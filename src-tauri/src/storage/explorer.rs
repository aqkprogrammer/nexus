use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use serde::Serialize;

use super::classify::{file_kind, FileKind};
use super::scanner::Node;
use crate::error::{NexusError, Result};
use crate::util::{allocated_size, path_string, run_with_timeout};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_hidden: bool,
    /// Directory sizes come from the last scan and are `None` until measured.
    pub size: Option<u64>,
    pub files: Option<u64>,
    pub kind: FileKind,
    pub modified: Option<i64>,
    pub accessed: Option<i64>,
    pub created: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
    /// Entries that could not be read and were skipped.
    pub skipped: u32,
}

fn secs(time: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    time.ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

pub fn entry_for(path: &Path, tree: Option<&Node>) -> Option<Entry> {
    let meta = fs::symlink_metadata(path).ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    let is_dir = meta.is_dir();
    let known = if is_dir { tree.and_then(|t| t.find(path)) } else { None };
    Some(Entry {
        is_hidden: name.starts_with('.'),
        path: path_string(path),
        is_dir,
        is_symlink: meta.file_type().is_symlink(),
        size: if is_dir { known.map(|n| n.size) } else { Some(allocated_size(&meta)) },
        files: if is_dir { known.map(|n| n.files) } else { None },
        kind: if is_dir { FileKind::Folder } else { file_kind(path) },
        modified: secs(meta.modified()),
        accessed: secs(meta.accessed()),
        created: secs(meta.created()),
        name,
    })
}

pub fn list(path: &Path, show_hidden: bool, tree: Option<&Node>) -> Result<Listing> {
    let read = fs::read_dir(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => NexusError::Permission(format!(
            "macOS denied access to {}. Grant access in System Settings > Privacy & Security.",
            path.display()
        )),
        _ => e.into(),
    })?;
    let mut skipped = 0;
    let mut entries = Vec::new();
    for item in read {
        let Ok(item) = item else {
            skipped += 1;
            continue;
        };
        match entry_for(&item.path(), tree) {
            Some(entry) if show_hidden || !entry.is_hidden => entries.push(entry),
            Some(_) => {}
            None => skipped += 1,
        }
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.size.unwrap_or(0)));
    Ok(Listing {
        path: path_string(path),
        parent: path.parent().map(path_string),
        entries,
        skipped,
    })
}

const SEARCH_LIMIT: usize = 300;

/// Name search under the approved roots: Spotlight for reach, the scan tree for hidden
/// and system-indexed gaps.
pub fn search(query: &str, roots: &[PathBuf], tree: Option<&Node>) -> Vec<Entry> {
    let query = query.trim();
    if query.len() < 2 {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();

    if let Some(tree) = tree {
        let needle = query.to_lowercase();
        let mut stack: Vec<(PathBuf, &Node)> =
            tree.children.iter().map(|c| (PathBuf::from(&c.name), c)).collect();
        while let Some((path, node)) = stack.pop() {
            if found.len() >= SEARCH_LIMIT {
                break;
            }
            if !node.aggregate && node.name.to_lowercase().contains(&needle) && seen.insert(path.clone()) {
                if let Some(entry) = entry_for(&path, Some(tree)) {
                    found.push(entry);
                }
            }
            for child in &node.children {
                if !child.aggregate {
                    stack.push((path.join(&child.name), child));
                }
            }
        }
    }

    for root in roots {
        if found.len() >= SEARCH_LIMIT {
            break;
        }
        let mut command = Command::new("/usr/bin/mdfind");
        command.arg("-onlyin").arg(root).arg("-name").arg(query);
        let Some(output) = run_with_timeout(command, Duration::from_secs(6)) else {
            continue;
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if found.len() >= SEARCH_LIMIT {
                break;
            }
            let path = PathBuf::from(line);
            if seen.insert(path.clone()) {
                if let Some(entry) = entry_for(&path, tree) {
                    found.push(entry);
                }
            }
        }
    }
    found.sort_by_key(|e| std::cmp::Reverse(e.size.unwrap_or(0)));
    found
}

/// Opens Finder with the item selected.
pub fn reveal(path: &Path) -> Result<()> {
    if !path.is_absolute() || fs::symlink_metadata(path).is_err() {
        return Err(NexusError::NotFound(format!("{} no longer exists.", path.display())));
    }
    Command::new("/usr/bin/open").arg("-R").arg(path).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn hides_dotfiles_until_asked() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".secret"), "x").unwrap();
        fs::write(dir.path().join("visible"), "x").unwrap();
        assert_eq!(list(dir.path(), false, None).unwrap().entries.len(), 1);
        assert_eq!(list(dir.path(), true, None).unwrap().entries.len(), 2);
    }

    #[test]
    fn directories_without_scan_data_have_unknown_size_not_zero() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let listing = list(dir.path(), false, None).unwrap();
        assert_eq!(listing.entries[0].size, None);
        assert!(listing.entries[0].is_dir);
    }

    #[test]
    fn symlinks_are_reported_as_links() {
        let dir = tempdir().unwrap();
        symlink("/usr", dir.path().join("link")).unwrap();
        let entry = &list(dir.path(), false, None).unwrap().entries[0];
        assert!(entry.is_symlink);
        assert!(!entry.is_dir);
    }

    #[test]
    fn missing_paths_are_errors_not_panics() {
        assert!(list(Path::new("/definitely/not/here"), false, None).is_err());
        assert!(reveal(Path::new("/definitely/not/here")).is_err());
    }

    #[test]
    fn single_character_queries_return_nothing() {
        assert!(search("a", &[], None).is_empty());
    }
}

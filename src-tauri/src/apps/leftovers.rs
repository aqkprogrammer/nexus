//! Finds Library entries named after a bundle ID that no installed or running app claims.
//! Matches are candidates for review, never automatic removals.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::util::path_string;

#[derive(Debug, Clone)]
pub struct Leftover {
    pub path: String,
    pub name: String,
}

const LOCATIONS: &[&str] = &[
    "Library/Application Support",
    "Library/Caches",
    "Library/Containers",
    "Library/HTTPStorages",
    "Library/Logs",
    "Library/Preferences",
    "Library/Saved Application State",
    "Library/WebKit",
];

/// Vendors whose tools run without a bundle in /Applications, so absence proves nothing.
const IGNORED_PREFIXES: &[&str] = &["com.apple.", "group.com.apple.", "app.nexus."];

fn looks_like_bundle_id(stem: &str) -> bool {
    let parts: Vec<&str> = stem.split('.').collect();
    parts.len() >= 3
        && matches!(parts[0], "com" | "org" | "net" | "io" | "app" | "co" | "dev" | "ai" | "us" | "de" | "uk")
        && parts.iter().all(|p| {
            !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
}

/// True when `stem` could belong to an installed id: the same id, a helper beneath it,
/// or a parent of it. Sharing a vendor is enough, which keeps false positives down.
fn claimed(stem: &str, installed: &HashSet<String>) -> bool {
    let lower = stem.to_ascii_lowercase();
    let vendor: String = lower.split('.').take(2).collect::<Vec<_>>().join(".");
    installed.iter().any(|id| {
        id == &lower
            || lower.starts_with(&format!("{id}."))
            || id.starts_with(&format!("{lower}."))
            || id.split('.').take(2).collect::<Vec<_>>().join(".") == vendor
    })
}

pub fn find(home: &Path, installed_ids: &[String]) -> Vec<Leftover> {
    let installed: HashSet<String> = installed_ids.iter().map(|i| i.to_ascii_lowercase()).collect();
    let mut found = Vec::new();
    for location in LOCATIONS {
        let Ok(entries) = fs::read_dir(home.join(location)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name
                .strip_suffix(".plist")
                .or_else(|| name.strip_suffix(".savedState"))
                .unwrap_or(&name);
            if IGNORED_PREFIXES.iter().any(|p| stem.starts_with(p))
                || !looks_like_bundle_id(stem)
                || claimed(stem, &installed)
            {
                continue;
            }
            found.push(Leftover { path: path_string(&entry.path()), name });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn home_with(files: &[&str]) -> tempfile::TempDir {
        let home = tempdir().unwrap();
        for file in files {
            let path = home.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        home
    }

    #[test]
    fn reports_entries_with_no_installed_owner() {
        let home = home_with(&["Library/Preferences/com.gone.tool.plist"]);
        let found = find(home.path(), &ids(&["com.other.app"]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "com.gone.tool.plist");
    }

    #[test]
    fn installed_apps_helpers_and_vendor_siblings_are_not_orphans() {
        let home = home_with(&[
            "Library/Preferences/com.vendor.editor.plist",
            "Library/Caches/com.vendor.editor.helper",
            "Library/Preferences/com.vendor.updater.plist",
        ]);
        assert!(find(home.path(), &ids(&["com.vendor.editor"])).is_empty());
    }

    #[test]
    fn apple_and_non_bundle_names_are_never_candidates() {
        let home = home_with(&[
            "Library/Preferences/com.apple.finder.plist",
            "Library/Application Support/Some Folder",
            "Library/Preferences/settings.plist",
        ]);
        assert!(find(home.path(), &[]).is_empty());
    }

    #[test]
    fn missing_library_is_harmless() {
        let home = tempdir().unwrap();
        assert!(find(home.path(), &[]).is_empty());
    }
}

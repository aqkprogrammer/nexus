//! Keeps held results truthful after a cleanup. A rescan can take a minute, so the items a
//! run actually removed are subtracted from the last results straight away. Figures for
//! folded small items are estimates, which is why summaries record that they were adjusted.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::cleaner::CleanerReport;
use crate::duplicates::DuplicateReport;
use crate::storage::classify::Category;
use crate::storage::scanner::{Node, ScanResult, MIB};
use crate::util::now_ms;

/// Paths a cleanup really removed, with the size recorded in its plan.
pub struct Removed {
    sizes: HashMap<PathBuf, u64>,
}

impl Removed {
    pub fn new(items: impl IntoIterator<Item = (String, u64)>) -> Self {
        Self { sizes: items.into_iter().map(|(p, s)| (PathBuf::from(p), s)).collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.sizes.is_empty()
    }

    /// True for a removed path or anything beneath one.
    fn covers(&self, path: &Path) -> bool {
        path.ancestors().any(|a| self.sizes.contains_key(a))
    }
}

struct Taken {
    size: u64,
    files: u64,
    by_category: [u64; Category::ALL.len()],
}

fn tally(node: &Node, inherited: Category, out: &mut [u64; Category::ALL.len()]) {
    // "Smaller items" has no category of its own, so it counts towards its parent.
    let category = if node.aggregate { inherited } else { node.category };
    let inner: u64 = node.children.iter().map(|c| c.size).sum();
    out[category.index()] += node.size.saturating_sub(inner);
    for child in &node.children {
        tally(child, category, out);
    }
}

fn take(node: &mut Node, parts: &[String], size: u64) -> Option<Taken> {
    let (first, rest) = parts.split_first()?;
    let found = node.children.iter().position(|c| !c.aggregate && c.name == *first);
    let taken = match found {
        Some(index) if rest.is_empty() => {
            let child = node.children.remove(index);
            let mut by_category = [0; Category::ALL.len()];
            tally(&child, node.category, &mut by_category);
            Taken { size: child.size, files: child.files, by_category }
        }
        Some(index) => take(&mut node.children[index], rest, size)?,
        None => {
            // Too small to have its own node, so its bytes live in "Smaller items".
            if let Some(folded) = node.children.iter_mut().find(|c| c.aggregate) {
                folded.size = folded.size.saturating_sub(size);
                folded.files = folded.files.saturating_sub(1);
            }
            node.children.retain(|c| !c.aggregate || c.size > 0);
            let mut by_category = [0; Category::ALL.len()];
            by_category[node.category.index()] = size;
            Taken { size, files: 1, by_category }
        }
    };
    node.size = node.size.saturating_sub(taken.size);
    node.files = node.files.saturating_sub(taken.files);
    Some(taken)
}

pub fn scan(result: &mut ScanResult, removed: &Removed, home: &Path) {
    let mut taken_sizes = Vec::new();
    for (path, size) in &removed.sizes {
        let root = result.tree.children.iter().position(|r| {
            let name = Path::new(&r.name);
            path != name && path.starts_with(name)
        });
        let Some(index) = root else {
            continue;
        };
        let Ok(relative) = path.strip_prefix(&result.tree.children[index].name) else {
            continue;
        };
        let parts: Vec<String> =
            relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let Some(taken) = take(&mut result.tree.children[index], &parts, *size) else {
            continue;
        };
        result.tree.size = result.tree.size.saturating_sub(taken.size);
        result.tree.files = result.tree.files.saturating_sub(taken.files);
        let summary = &mut result.summary;
        summary.total_bytes = summary.total_bytes.saturating_sub(taken.size);
        summary.files = summary.files.saturating_sub(taken.files);
        for stat in &mut summary.categories {
            stat.bytes = stat.bytes.saturating_sub(taken.by_category[stat.category.index()]);
        }
        taken_sizes.push((path.clone(), taken.size, taken.files));
    }

    let summary = &mut result.summary;
    let cutoff = summary.finished_at / 1000 - i64::from(summary.old.cutoff_months) * 30 * 86_400;
    let downloads = home.join("Downloads");
    let (mut over_100, mut over_1g, mut old) = (summary.over_100_mb, summary.over_1_gb, summary.old.clone());
    summary.top_files.retain(|file| {
        let path = Path::new(&file.path);
        if !removed.covers(path) {
            return true;
        }
        if file.size >= 100 * MIB {
            over_100 = (over_100.0.saturating_sub(1), over_100.1.saturating_sub(file.size));
        }
        if file.size >= 1024 * MIB {
            over_1g = (over_1g.0.saturating_sub(1), over_1g.1.saturating_sub(file.size));
        }
        if file.modified > 0 && file.modified < cutoff {
            old.bytes = old.bytes.saturating_sub(file.size);
            old.files = old.files.saturating_sub(1);
            if path.starts_with(&downloads) {
                old.downloads_bytes = old.downloads_bytes.saturating_sub(file.size);
                old.downloads_files = old.downloads_files.saturating_sub(1);
            }
        }
        false
    });
    summary.over_100_mb = over_100;
    summary.over_1_gb = over_1g;
    summary.old = old;

    summary.hogs.retain(|hog| !removed.covers(Path::new(&hog.path)));
    for hog in &mut summary.hogs {
        let hog_path = Path::new(&hog.path);
        for (path, size, files) in &taken_sizes {
            if path.starts_with(hog_path) {
                hog.size = hog.size.saturating_sub(*size);
                hog.files = hog.files.saturating_sub(*files);
            }
        }
    }
    summary.adjusted_at = Some(now_ms());
}

pub fn cleaner(report: &mut CleanerReport, removed: &Removed) {
    for target in &mut report.targets {
        target.items.retain(|item| !removed.covers(Path::new(&item.path)));
        target.total = target.items.iter().map(|i| i.size).sum();
    }
    report.reclaimable = report.targets.iter().map(|t| t.total).sum();
    report.recommended = report.targets.iter().filter(|t| t.selected_by_default).map(|t| t.total).sum();
}

pub fn duplicates(report: &mut DuplicateReport, removed: &Removed) {
    for group in &mut report.groups {
        group.files.retain(|file| !removed.covers(Path::new(&file.path)));
        group.recoverable = group.size * (group.files.len() as u64).saturating_sub(1);
    }
    report.groups.retain(|group| group.files.len() > 1);
    report.recoverable = report.groups.iter().map(|g| g.recoverable).sum();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::CancelToken;
    use crate::storage::classify::HogKind;
    use crate::storage::scanner::{self, ScanOptions};
    use crate::util::path_string;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    /// Scans a fixture for real, so these tests use the shape the scanner produces.
    fn fixture() -> (TempDir, PathBuf, ScanResult) {
        let dir = tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        write(&home.join("Movies/clip.mov"), 120 << 20);
        write(&home.join("Documents/report.pdf"), 3 << 20);
        write(&home.join("Documents/note.txt"), 4096);
        write(&home.join("app/node_modules/a/big.js"), 12 << 20);
        write(&home.join("app/node_modules/b/big.js"), 12 << 20);
        let options = ScanOptions {
            roots: vec![home.clone()],
            exclusions: Vec::new(),
            home: home.clone(),
            old_file_months: 12,
            cache: None,
        };
        let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let (result, _) = scanner::run(options, &CancelToken::default(), &pool, |_| {});
        (dir, home, result)
    }

    fn removed(path: &Path, size: u64) -> Removed {
        Removed::new([(path_string(path), size)])
    }

    fn category_bytes(result: &ScanResult, category: Category) -> u64 {
        result.summary.categories.iter().find(|c| c.category == category).unwrap().bytes
    }

    #[test]
    fn a_removed_file_leaves_every_figure_that_counted_it() {
        let (_dir, home, mut result) = fixture();
        let clip = home.join("Movies/clip.mov");
        let size = result.tree.find(&clip).unwrap().size;
        let before = result.summary.clone();
        scan(&mut result, &removed(&clip, size), &home);

        assert_eq!(result.summary.total_bytes, before.total_bytes - size);
        assert_eq!(result.summary.files, before.files - 1);
        assert_eq!(result.tree.size, before.total_bytes - size);
        assert_eq!(result.summary.over_100_mb.0, before.over_100_mb.0 - 1);
        assert_eq!(category_bytes(&result, Category::Media), result.tree.find(&home.join("Movies")).unwrap().size);
        assert!(result.tree.find(&clip).is_none());
        assert!(result.summary.top_files.iter().all(|f| f.name != "clip.mov"));
        assert!(result.summary.hogs.iter().all(|h| h.kind != HogKind::LargeVideo));
        assert!(result.summary.adjusted_at.is_some());
    }

    #[test]
    fn removing_a_folder_takes_everything_beneath_it() {
        let (_dir, home, mut result) = fixture();
        let modules = home.join("app/node_modules");
        let size = result.tree.find(&modules).unwrap().size;
        let before = result.summary.total_bytes;
        scan(&mut result, &removed(&modules, size), &home);

        assert_eq!(result.summary.total_bytes, before - size);
        assert!(result.summary.hogs.iter().all(|h| h.kind != HogKind::NodeModules));
        assert!(result.summary.top_files.iter().all(|f| !f.path.contains("node_modules")));
    }

    #[test]
    fn removing_part_of_a_hog_shrinks_it_rather_than_dropping_it() {
        let (_dir, home, mut result) = fixture();
        let part = home.join("app/node_modules/a");
        let size = result.tree.find(&part).unwrap().size;
        let hog = |r: &ScanResult| r.summary.hogs.iter().find(|h| h.kind == HogKind::NodeModules).unwrap().size;
        let before = hog(&result);
        scan(&mut result, &removed(&part, size), &home);
        assert_eq!(hog(&result), before - size);
    }

    #[test]
    fn an_item_folded_into_smaller_items_still_comes_off_the_totals() {
        let (_dir, home, mut result) = fixture();
        let note = home.join("Documents/note.txt");
        assert!(result.tree.find(&note).is_none());
        let before = result.summary.total_bytes;
        let folder = result.tree.find(&home.join("Documents")).unwrap().size;
        scan(&mut result, &removed(&note, 4096), &home);
        assert_eq!(result.summary.total_bytes, before - 4096);
        assert_eq!(result.tree.find(&home.join("Documents")).unwrap().size, folder - 4096);
    }

    #[test]
    fn paths_outside_the_scan_change_nothing_and_figures_never_underflow() {
        let (_dir, home, mut result) = fixture();
        let before = result.summary.total_bytes;
        scan(&mut result, &removed(Path::new("/elsewhere/file"), u64::MAX), &home);
        assert_eq!(result.summary.total_bytes, before);
        scan(&mut result, &removed(&home.join("Documents/ghost.bin"), u64::MAX), &home);
        assert_eq!(result.summary.total_bytes, 0);
    }

    fn item(id: &str, path: &str, size: u64) -> crate::cleaner::CleanerItem {
        crate::cleaner::CleanerItem {
            id: id.into(),
            path: path.into(),
            name: path.into(),
            size,
            files: 1,
            modified: 0,
            is_dir: true,
        }
    }

    #[test]
    fn cleaner_totals_follow_what_is_left() {
        use crate::cleaner::catalog::{Group, Risk};
        use crate::cleaner::{CleanerReport, CleanerTarget, TrashStatus};
        let target = |id: &str, selected: bool, items| CleanerTarget {
            id: id.into(),
            group: Group::SystemJunk,
            title: id.into(),
            description: String::new(),
            effect: String::new(),
            risk: Risk::Low,
            selected_by_default: selected,
            policy: crate::safety::Policy::CleanupTarget,
            items,
            total: 0,
            app_running: None,
            unavailable: None,
        };
        let mut report = CleanerReport {
            scanned_at: 0,
            duration_ms: 0,
            completed: true,
            targets: vec![
                target("a", true, vec![item("a:0", "/c/one", 10), item("a:1", "/c/two/inner", 20)]),
                target("b", false, vec![item("b:0", "/c/three", 40)]),
            ],
            trash: TrashStatus { accessible: false, items: Vec::new(), total: 0, count: 0 },
            docker: Default::default(),
            reclaimable: 70,
            recommended: 30,
        };
        cleaner(&mut report, &Removed::new([("/c/two".to_owned(), 20)]));
        assert_eq!(report.targets[0].items.len(), 1);
        assert_eq!(report.recommended, 10);
        assert_eq!(report.reclaimable, 50);
    }

    #[test]
    fn a_duplicate_group_disappears_once_one_copy_is_left() {
        use crate::duplicates::{DuplicateFile, DuplicateGroup, DuplicateReport};
        use crate::storage::classify::FileKind;
        let file = |path: &str| DuplicateFile { path: path.into(), name: path.into(), modified: 0, created: 0 };
        let group = |id: &str, files| DuplicateGroup {
            id: id.into(),
            hash: id.into(),
            size: 100,
            kind: FileKind::Other,
            recoverable: 0,
            files,
        };
        let mut report = DuplicateReport {
            scanned_at: 0,
            duration_ms: 0,
            completed: true,
            roots: Vec::new(),
            files_indexed: 0,
            bytes_hashed: 0,
            inaccessible: 0,
            recoverable: 300,
            groups: vec![
                group("a", vec![file("/a1"), file("/a2")]),
                group("b", vec![file("/b1"), file("/b2"), file("/b3")]),
            ],
        };
        duplicates(&mut report, &Removed::new([("/a2".to_owned(), 100), ("/b3".to_owned(), 100)]));
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].id, "b");
        assert_eq!(report.recoverable, 100);
    }
}

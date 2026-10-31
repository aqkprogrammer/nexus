pub mod catalog;
pub mod docker;

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use serde::Serialize;

use crate::apps::leftovers::Leftover;
use crate::jobs::CancelToken;
use crate::storage::scanner::{measure, HogItem};
use crate::storage::classify::HogKind;
use crate::util::{allocated_size, now_ms, path_string};
use catalog::{should_skip, Group, Risk, Source, TargetDef};
use docker::DockerStatus;

const MAX_ITEMS_PER_TARGET: usize = 1500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerItem {
    pub id: String,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub modified: i64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerTarget {
    pub id: String,
    pub group: Group,
    pub title: String,
    pub description: String,
    pub effect: String,
    pub risk: Risk,
    pub selected_by_default: bool,
    pub policy: crate::safety::Policy,
    pub items: Vec<CleanerItem>,
    pub total: u64,
    pub app_running: Option<String>,
    /// Set when the location exists but macOS refused access.
    pub unavailable: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashStatus {
    pub accessible: bool,
    pub items: Vec<CleanerItem>,
    pub total: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerReport {
    pub scanned_at: i64,
    pub duration_ms: i64,
    pub completed: bool,
    pub targets: Vec<CleanerTarget>,
    pub trash: TrashStatus,
    pub docker: DockerStatus,
    /// Bytes across all targets, including ones not selected by default.
    pub reclaimable: u64,
    /// Bytes across low-risk targets selected by default.
    pub recommended: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerProgress {
    pub percent: f64,
    pub phase: String,
    pub current: String,
    pub found_bytes: u64,
    pub findings: Vec<CleanerFinding>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerFinding {
    pub title: String,
    pub bytes: u64,
}

fn unix_secs(time: std::io::Result<SystemTime>) -> i64 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn measure_entry(path: &Path, token: &CancelToken) -> Option<(u64, u64, i64, bool)> {
    let meta = fs::symlink_metadata(path).ok()?;
    let modified = unix_secs(meta.modified());
    if meta.is_dir() {
        let (size, files) = measure(path, token);
        Some((size + allocated_size(&meta), files, modified, true))
    } else {
        Some((allocated_size(&meta), 1, modified, false))
    }
}

fn candidates(source: &Source) -> (Vec<PathBuf>, Option<String>) {
    let list_children = |dir: &Path| -> (Vec<PathBuf>, Option<String>) {
        match fs::read_dir(dir) {
            Ok(entries) => (entries.flatten().map(|e| e.path()).collect(), None),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => (
                Vec::new(),
                Some(format!("macOS denied access to {}.", dir.display())),
            ),
            Err(_) => (Vec::new(), None),
        }
    };
    match source {
        Source::Children(dir) => list_children(dir),
        Source::Item(path) => {
            (fs::symlink_metadata(path).is_ok().then(|| path.clone()).into_iter().collect(), None)
        }
        Source::ProfileDirs(root, names) => {
            let (profiles, denied) = list_children(root);
            let found = profiles
                .iter()
                .flat_map(|profile| names.iter().map(move |n| profile.join(n)))
                .filter(|p| p.is_dir())
                .collect();
            (found, denied)
        }
    }
}

/// Measures one catalog target. Pure enough to test against a fake home directory.
pub fn scan_target(def: &TargetDef, running: &HashSet<String>, uid: u32, token: &CancelToken) -> CleanerTarget {
    let now = now_ms() / 1000;
    let cutoff = now - i64::from(def.min_age_days) * 86_400;
    let mut paths = Vec::new();
    let mut unavailable = None;
    for source in &def.sources {
        let (found, denied) = candidates(source);
        paths.extend(found);
        unavailable = unavailable.or(denied);
    }
    let seen: HashSet<PathBuf> = paths.iter().cloned().collect();
    let mut items: Vec<CleanerItem> = seen
        .into_par_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            if should_skip(&name, def.skip) || token.is_cancelled() {
                return None;
            }
            let meta = fs::symlink_metadata(&path).ok()?;
            if def.owned_only && meta.uid() != uid {
                return None;
            }
            // A symlink's own timestamp says nothing about its target, so age the link itself.
            if def.min_age_days > 0 && unix_secs(meta.modified()) > cutoff {
                return None;
            }
            let (size, files, modified, is_dir) = measure_entry(&path, token)?;
            (size > 0 || !is_dir).then(|| CleanerItem {
                id: String::new(),
                path: path_string(&path),
                name,
                size,
                files,
                modified,
                is_dir,
            })
        })
        .collect();
    items.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
    items.truncate(MAX_ITEMS_PER_TARGET);
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("{}:{index}", def.id);
    }
    CleanerTarget {
        id: def.id.to_owned(),
        group: def.group,
        title: def.title.to_owned(),
        description: def.description.to_owned(),
        effect: def.effect.to_owned(),
        risk: def.risk,
        selected_by_default: def.selected_by_default,
        policy: crate::safety::Policy::CleanupTarget,
        total: items.iter().map(|i| i.size).sum(),
        items,
        app_running: def.app.filter(|a| running.contains(*a)).map(str::to_owned),
        unavailable,
    }
}

pub fn scan_trash(home: &Path, token: &CancelToken) -> TrashStatus {
    let Ok(entries) = fs::read_dir(home.join(".Trash")) else {
        return TrashStatus { accessible: false, items: Vec::new(), total: 0, count: 0 };
    };
    let paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    let mut items: Vec<CleanerItem> = paths
        .into_par_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            if name == ".DS_Store" {
                return None;
            }
            let (size, files, modified, is_dir) = measure_entry(&path, token)?;
            Some(CleanerItem { id: String::new(), path: path_string(&path), name, size, files, modified, is_dir })
        })
        .collect();
    items.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("trash:{index}");
    }
    TrashStatus {
        accessible: true,
        total: items.iter().map(|i| i.size).sum(),
        count: items.len() as u64,
        items,
    }
}

fn leftovers_target(id: &str, title: &str, description: &str, group: Vec<Leftover>, token: &CancelToken) -> CleanerTarget {
    let mut items: Vec<CleanerItem> = group
        .into_par_iter()
        .filter_map(|l| {
            let path = PathBuf::from(&l.path);
            let (size, files, modified, is_dir) = measure_entry(&path, token)?;
            Some(CleanerItem { id: String::new(), name: l.name, path: l.path, size, files, modified, is_dir })
        })
        .collect();
    items.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
    items.truncate(MAX_ITEMS_PER_TARGET);
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("{id}:{index}");
    }
    CleanerTarget {
        id: id.to_owned(),
        group: Group::Leftovers,
        title: title.to_owned(),
        description: description.to_owned(),
        effect: "These always go to the Trash so you can restore anything an installed tool still needs.".into(),
        risk: Risk::Medium,
        selected_by_default: false,
        policy: crate::safety::Policy::Leftover,
        total: items.iter().map(|i| i.size).sum(),
        items,
        app_running: None,
        unavailable: None,
    }
}

fn build_artifacts_target(hogs: &[HogItem]) -> Option<CleanerTarget> {
    let mut items: Vec<CleanerItem> = hogs
        .iter()
        .filter(|h| matches!(h.kind, HogKind::BuildArtifact))
        .filter(|h| fs::symlink_metadata(&h.path).is_ok())
        .map(|h| CleanerItem {
            id: String::new(),
            path: h.path.clone(),
            name: h.path.clone(),
            size: h.size,
            files: h.files,
            modified: h.modified,
            is_dir: h.is_dir,
        })
        .collect();
    if items.is_empty() {
        return None;
    }
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("build-artifacts:{index}");
    }
    Some(CleanerTarget {
        id: "build-artifacts".into(),
        group: Group::Developer,
        title: "Temporary build artifacts".into(),
        description: "Rebuildable output folders such as target, dist and .next, found by the last storage scan.".into(),
        effect: "The next build recreates these. They always go to the Trash first.".into(),
        risk: Risk::Medium,
        selected_by_default: false,
        policy: crate::safety::Policy::UserSelected,
        total: items.iter().map(|i| i.size).sum(),
        items,
        app_running: None,
        unavailable: None,
    })
}

pub struct ScanInput {
    pub home: PathBuf,
    pub defs: Vec<TargetDef>,
    pub running: HashSet<String>,
    pub leftovers: Vec<Leftover>,
    pub hogs: Vec<HogItem>,
}

pub fn scan(
    input: ScanInput,
    token: &CancelToken,
    pool: &rayon::ThreadPool,
    on_progress: impl Fn(CleanerProgress),
) -> CleanerReport {
    let started = now_ms();
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    let steps = input.defs.len() as f64 + 4.0;
    // A possible leftover needs review, so it never also appears under a preselected target.
    let leftover_paths: HashSet<&str> = input.leftovers.iter().map(|l| l.path.as_str()).collect();
    let mut targets = Vec::with_capacity(input.defs.len() + 3);
    let mut found = 0u64;
    let mut done = 0.0;
    let report = |percent: f64, phase: &str, current: &str, found: u64, findings: Vec<CleanerFinding>| {
        on_progress(CleanerProgress {
            percent,
            phase: phase.to_owned(),
            current: current.to_owned(),
            found_bytes: found,
            findings,
        });
    };

    for def in &input.defs {
        if token.is_cancelled() {
            break;
        }
        report(done / steps * 100.0, "Analyzing cleanup locations", def.title, found, Vec::new());
        let mut target = pool.install(|| scan_target(def, &input.running, uid, token));
        target.items.retain(|item| !leftover_paths.contains(item.path.as_str()));
        target.total = target.items.iter().map(|i| i.size).sum();
        done += 1.0;
        found += target.total;
        let finding = (target.total > 0)
            .then(|| CleanerFinding { title: target.title.clone(), bytes: target.total });
        report(done / steps * 100.0, "Analyzing cleanup locations", def.title, found, finding.into_iter().collect());
        targets.push(target);
    }

    report(done / steps * 100.0, "Checking application leftovers", "Library folders", found, Vec::new());
    let (preferences, other): (Vec<_>, Vec<_>) =
        input.leftovers.iter().cloned().partition(|l| l.name.ends_with(".plist"));
    let (caches, support): (Vec<_>, Vec<_>) =
        other.into_iter().partition(|l| l.path.contains("/Library/Caches/"));
    let groups = [
        ("orphan-support", "Unused application support files", "Support data and containers whose bundle ID matches no installed app.", support),
        ("orphan-prefs", "Orphaned preference files", "Settings files left behind by apps that are no longer installed.", preferences),
        ("orphan-caches", "Leftover application caches", "Caches for apps that are no longer installed.", caches),
    ];
    for (id, title, description, group) in groups {
        let target = pool.install(|| leftovers_target(id, title, description, group, token));
        done += 1.0;
        found += target.total;
        let finding = (target.total > 0)
            .then(|| CleanerFinding { title: target.title.clone(), bytes: target.total });
        report(done / steps * 100.0, "Checking application leftovers", title, found, finding.into_iter().collect());
        targets.push(target);
    }
    if let Some(target) = build_artifacts_target(&input.hogs) {
        found += target.total;
        targets.push(target);
    }

    report(done / steps * 100.0, "Measuring Trash and Docker", "Trash", found, Vec::new());
    let trash = pool.install(|| scan_trash(&input.home, token));
    let docker = docker::status();
    report(100.0, "Finishing", "", found, Vec::new());

    let reclaimable = targets.iter().map(|t| t.total).sum();
    let recommended = targets.iter().filter(|t| t.selected_by_default).map(|t| t.total).sum();
    CleanerReport {
        scanned_at: now_ms(),
        duration_ms: now_ms() - started,
        completed: !token.is_cancelled(),
        targets,
        trash,
        docker,
        reclaimable,
        recommended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    fn def(sources: Vec<Source>, skip: &'static [&'static str], min_age_days: u32) -> TargetDef {
        TargetDef {
            id: "t",
            group: Group::SystemJunk,
            title: "T",
            description: "",
            effect: "",
            risk: Risk::Low,
            selected_by_default: true,
            sources,
            skip,
            min_age_days,
            owned_only: false,
            app: Some("Safari"),
        }
    }

    fn uid() -> u32 {
        // SAFETY: getuid has no preconditions.
        unsafe { libc::getuid() }
    }

    fn write(path: &Path, size: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![1u8; size]).unwrap();
    }

    #[test]
    fn lists_children_as_items_and_honours_skips() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("com.vendor.app/cache.bin"), 8192);
        write(&dir.path().join("com.apple.dock/keep.bin"), 8192);
        let target = scan_target(
            &def(vec![Source::Children(dir.path().into())], &["com.apple.*"], 0),
            &HashSet::new(),
            uid(),
            &CancelToken::default(),
        );
        assert_eq!(target.items.len(), 1);
        assert_eq!(target.items[0].name, "com.vendor.app");
        assert_eq!(target.items[0].files, 1);
        assert!(target.total >= 8192);
    }

    #[test]
    fn minimum_age_leaves_fresh_items_alone() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("fresh.tmp"), 4096);
        let target = scan_target(
            &def(vec![Source::Children(dir.path().into())], &[], 3),
            &HashSet::new(),
            uid(),
            &CancelToken::default(),
        );
        assert!(target.items.is_empty());
    }

    #[test]
    fn missing_locations_are_empty_not_errors() {
        let target = scan_target(
            &def(vec![Source::Children("/no/such/dir".into()), Source::Item("/no/such/file".into())], &[], 0),
            &HashSet::new(),
            uid(),
            &CancelToken::default(),
        );
        assert!(target.items.is_empty());
        assert!(target.unavailable.is_none());
    }

    #[test]
    fn symlink_items_are_measured_as_links() {
        let dir = tempdir().unwrap();
        let big = tempdir().unwrap();
        write(&big.path().join("huge.bin"), 1 << 21);
        symlink(big.path(), dir.path().join("link")).unwrap();
        let target = scan_target(
            &def(vec![Source::Children(dir.path().into())], &[], 0),
            &HashSet::new(),
            uid(),
            &CancelToken::default(),
        );
        assert_eq!(target.items.len(), 1);
        assert!(target.items[0].size < 1 << 16);
        assert!(!target.items[0].is_dir);
    }

    #[test]
    fn running_owner_app_is_surfaced() {
        let running: HashSet<String> = ["Safari".to_owned()].into();
        let target = scan_target(&def(Vec::new(), &[], 0), &running, uid(), &CancelToken::default());
        assert_eq!(target.app_running.as_deref(), Some("Safari"));
    }

    #[test]
    fn profile_dirs_only_pick_named_folders() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("Default/Cache/data"), 4096);
        write(&dir.path().join("Default/Cookies"), 4096);
        write(&dir.path().join("Default/History"), 4096);
        let target = scan_target(
            &def(vec![Source::ProfileDirs(dir.path().into(), &["Cache", "GPUCache"])], &[], 0),
            &HashSet::new(),
            uid(),
            &CancelToken::default(),
        );
        assert_eq!(target.items.len(), 1);
        assert!(target.items[0].path.ends_with("Default/Cache"));
    }

    #[test]
    fn a_leftover_is_only_listed_for_review_never_as_preselected_cache() {
        let home = tempdir().unwrap();
        let caches = home.path().join("Library/Caches");
        write(&caches.join("com.gone.tool/data.bin"), 8192);
        write(&caches.join("KeptApp/data.bin"), 8192);
        let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let input = ScanInput {
            home: home.path().to_path_buf(),
            defs: vec![def(vec![Source::Children(caches.clone())], &[], 0)],
            running: HashSet::new(),
            leftovers: vec![Leftover {
                path: crate::util::path_string(&caches.join("com.gone.tool")),
                name: "com.gone.tool".into(),
            }],
            hogs: Vec::new(),
        };
        let report = scan(input, &CancelToken::default(), &pool, |_| {});
        let preselected = &report.targets[0];
        assert_eq!(preselected.items.len(), 1);
        assert_eq!(preselected.items[0].name, "KeptApp");
        let review = report.targets.iter().find(|t| t.id == "orphan-caches").unwrap();
        assert_eq!(review.items.len(), 1);
        assert!(!review.selected_by_default);
        assert_eq!(report.recommended, preselected.total);
    }

    #[test]
    fn trash_is_reported_with_totals() {
        let home = tempdir().unwrap();
        write(&home.path().join(".Trash/old.zip"), 8192);
        write(&home.path().join(".Trash/.DS_Store"), 8192);
        let trash = scan_trash(home.path(), &CancelToken::default());
        assert!(trash.accessible);
        assert_eq!(trash.count, 1);
        assert!(trash.total >= 8192);
    }

    #[test]
    fn unreadable_trash_is_flagged_rather_than_shown_as_empty() {
        let home = tempdir().unwrap();
        let trash = scan_trash(home.path(), &CancelToken::default());
        assert!(!trash.accessible);
    }
}

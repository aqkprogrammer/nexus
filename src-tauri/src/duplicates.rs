//! Finds byte-identical files in three narrowing passes: size, a partial hash of the head
//! and tail, then a full BLAKE3 hash of what is left.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::macos::fs::MetadataExt as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use parking_lot::Mutex;
use rayon::prelude::*;
use serde::Serialize;

use crate::jobs::CancelToken;
use crate::storage::classify::{file_kind, FileKind};
use crate::util::{now_ms, path_string};

const SAMPLE: usize = 16 * 1024;
const CHUNK: usize = 1024 * 1024;
/// iCloud and File Provider placeholder: reading it would trigger a download.
const SF_DATALESS: u32 = 0x4000_0000;
const SKIPPED_DIRS: &[&str] = &[".git", "node_modules", ".Trash", "Library", ".npm", ".cargo", ".rustup"];
/// Package formats whose insides must stay intact, so they are treated as single opaque items.
const OPAQUE_EXTENSIONS: &[&str] = &[
    "app", "photoslibrary", "fcpbundle", "musiclibrary", "tvlibrary", "imovielibrary", "framework",
    "xcodeproj", "xcworkspace", "sparsebundle", "pages", "numbers", "key",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub path: String,
    pub name: String,
    pub modified: i64,
    pub created: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub hash: String,
    pub size: u64,
    pub kind: FileKind,
    pub recoverable: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub scanned_at: i64,
    pub duration_ms: i64,
    pub completed: bool,
    pub roots: Vec<String>,
    pub files_indexed: u64,
    pub bytes_hashed: u64,
    pub inaccessible: u64,
    pub recoverable: u64,
    pub groups: Vec<DuplicateGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProgress {
    pub phase: String,
    pub files: u64,
    pub candidates: u64,
    pub bytes_hashed: u64,
    pub bytes_total: u64,
    pub current: String,
}

pub struct DuplicateOptions {
    pub roots: Vec<PathBuf>,
    pub exclusions: Vec<PathBuf>,
    pub min_size: u64,
}

#[derive(Clone)]
struct Candidate {
    path: PathBuf,
    size: u64,
    identity: (u64, u64),
}

struct Walk<'a> {
    options: &'a DuplicateOptions,
    token: &'a CancelToken,
    by_size: Mutex<HashMap<u64, Vec<Candidate>>>,
    files: AtomicU64,
    denied: AtomicU64,
    visited: Mutex<HashSet<(u64, u64)>>,
    current: Mutex<String>,
}

fn walk(ctx: &Walk<'_>, dir: &Path) {
    if ctx.token.is_cancelled() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        ctx.denied.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            ctx.denied.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        if meta.is_dir() {
            let name = entry.file_name();
            let opaque = crate::util::lower_ext(&path)
                .is_some_and(|e| OPAQUE_EXTENSIONS.contains(&e.as_str()));
            let skip = opaque
                || SKIPPED_DIRS.iter().any(|d| name == *d)
                || ctx.options.exclusions.iter().any(|e| path.starts_with(e));
            if !skip && ctx.visited.lock().insert((meta.dev(), meta.ino())) {
                subdirs.push(path);
            }
        } else if meta.is_file()
            && meta.len() >= ctx.options.min_size
            && meta.st_flags() & SF_DATALESS == 0
        {
            let count = ctx.files.fetch_add(1, Ordering::Relaxed);
            if count % 256 == 0 {
                *ctx.current.lock() = path_string(&path);
            }
            ctx.by_size.lock().entry(meta.len()).or_default().push(Candidate {
                path,
                size: meta.len(),
                identity: (meta.dev(), meta.ino()),
            });
        }
    }
    subdirs.par_iter().for_each(|d| walk(ctx, d));
}

fn partial_hash(candidate: &Candidate) -> Option<[u8; 32]> {
    let mut file = File::open(&candidate.path).ok()?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; SAMPLE];
    let read = file.read(&mut buffer).ok()?;
    hasher.update(&buffer[..read]);
    if candidate.size > (SAMPLE * 2) as u64 {
        file.seek(SeekFrom::End(-(SAMPLE as i64))).ok()?;
        let read = file.read(&mut buffer).ok()?;
        hasher.update(&buffer[..read]);
    }
    Some(*hasher.finalize().as_bytes())
}

fn full_hash(path: &Path, token: &CancelToken, hashed: &AtomicU64) -> Option<blake3::Hash> {
    let mut file = File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if token.is_cancelled() {
            return None;
        }
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            return Some(hasher.finalize());
        }
        hasher.update(&buffer[..read]);
        hashed.fetch_add(read as u64, Ordering::Relaxed);
    }
}

/// Groups candidates by a key, dropping groups that cannot contain a duplicate.
fn narrow<K: std::hash::Hash + Eq + Send>(
    candidates: Vec<Candidate>,
    key: impl Fn(&Candidate) -> Option<K> + Sync,
) -> Vec<(K, Vec<Candidate>)> {
    let keyed: Vec<(K, Candidate)> = candidates
        .into_par_iter()
        .filter_map(|c| key(&c).map(|k| (k, c)))
        .collect();
    let mut groups: HashMap<K, Vec<Candidate>> = HashMap::new();
    for (k, c) in keyed {
        groups.entry(k).or_default().push(c);
    }
    groups.into_iter().filter(|(_, g)| g.len() > 1).collect()
}

fn describe(path: &Path) -> Option<DuplicateFile> {
    let meta = fs::metadata(path).ok()?;
    let secs = |t: std::io::Result<std::time::SystemTime>| {
        t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
    };
    Some(DuplicateFile {
        path: path_string(path),
        name: path.file_name()?.to_string_lossy().into_owned(),
        modified: secs(meta.modified()),
        created: secs(meta.created()),
    })
}

pub fn scan(
    options: DuplicateOptions,
    token: &CancelToken,
    pool: &rayon::ThreadPool,
    on_progress: impl Fn(DuplicateProgress) + Send + Sync,
) -> DuplicateReport {
    let started = now_ms();
    let ctx = Walk {
        options: &options,
        token,
        by_size: Mutex::new(HashMap::new()),
        files: AtomicU64::new(0),
        denied: AtomicU64::new(0),
        visited: Mutex::new(HashSet::new()),
        current: Mutex::new(String::new()),
    };
    let hashed = AtomicU64::new(0);
    let total_to_hash = AtomicU64::new(0);
    let candidates_left = AtomicU64::new(0);
    let phase = Mutex::new("Indexing files");
    let done = std::sync::atomic::AtomicBool::new(false);

    let groups: Vec<(blake3::Hash, Vec<Candidate>)> = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut last = Instant::now() - Duration::from_secs(1);
            while !done.load(Ordering::Relaxed) {
                if last.elapsed() >= Duration::from_millis(120) {
                    last = Instant::now();
                    on_progress(DuplicateProgress {
                        phase: (*phase.lock()).to_owned(),
                        files: ctx.files.load(Ordering::Relaxed),
                        candidates: candidates_left.load(Ordering::Relaxed),
                        bytes_hashed: hashed.load(Ordering::Relaxed),
                        bytes_total: total_to_hash.load(Ordering::Relaxed),
                        current: ctx.current.lock().clone(),
                    });
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        });

        let result = pool.install(|| {
            options.roots.par_iter().for_each(|root| walk(&ctx, root));

            *phase.lock() = "Comparing candidates";
            let by_size = std::mem::take(&mut *ctx.by_size.lock());
            let same_size: Vec<Candidate> = by_size
                .into_values()
                .filter(|group| group.len() > 1)
                .flat_map(|group| {
                    // Hard links share one inode: they are the same file, not duplicates.
                    let mut seen = HashSet::new();
                    group.into_iter().filter(move |c| seen.insert(c.identity))
                })
                .collect();
            candidates_left.store(same_size.len() as u64, Ordering::Relaxed);

            let mut by_size_again: HashMap<u64, Vec<Candidate>> = HashMap::new();
            for candidate in same_size {
                by_size_again.entry(candidate.size).or_default().push(candidate);
            }
            let sampled: Vec<Candidate> = by_size_again
                .into_par_iter()
                .filter(|(_, g)| g.len() > 1)
                .flat_map(|(_, group)| {
                    narrow(group, partial_hash).into_par_iter().flat_map(|(_, g)| g)
                })
                .collect();

            *phase.lock() = "Verifying with BLAKE3";
            candidates_left.store(sampled.len() as u64, Ordering::Relaxed);
            total_to_hash.store(sampled.iter().map(|c| c.size).sum(), Ordering::Relaxed);
            narrow(sampled, |c| {
                *ctx.current.lock() = path_string(&c.path);
                full_hash(&c.path, token, &hashed)
            })
        });
        done.store(true, Ordering::Relaxed);
        result
    });

    let mut report_groups: Vec<DuplicateGroup> = groups
        .into_iter()
        .filter_map(|(hash, candidates)| {
            let size = candidates[0].size;
            let kind = file_kind(&candidates[0].path);
            let mut files: Vec<DuplicateFile> =
                candidates.iter().filter_map(|c| describe(&c.path)).collect();
            if files.len() < 2 {
                return None;
            }
            files.sort_by_key(|f| f.modified);
            let hex = hash.to_hex().to_string();
            Some(DuplicateGroup {
                id: hex[..16].to_owned(),
                recoverable: size * (files.len() as u64 - 1),
                hash: hex,
                size,
                kind,
                files,
            })
        })
        .collect();
    report_groups.sort_unstable_by_key(|x| std::cmp::Reverse(x.recoverable));

    DuplicateReport {
        scanned_at: now_ms(),
        duration_ms: now_ms() - started,
        completed: !token.is_cancelled(),
        roots: options.roots.iter().map(|r| path_string(r)).collect(),
        files_indexed: ctx.files.load(Ordering::Relaxed),
        bytes_hashed: hashed.load(Ordering::Relaxed),
        inaccessible: ctx.denied.load(Ordering::Relaxed),
        recoverable: report_groups.iter().map(|g| g.recoverable).sum(),
        groups: report_groups,
    }
}

/// Refuses a selection that would remove every copy of any group.
pub fn check_selection(report: &DuplicateReport, selected: &[String]) -> Result<Vec<(String, u64)>, String> {
    let chosen: HashSet<&str> = selected.iter().map(String::as_str).collect();
    let mut accepted = Vec::new();
    for group in &report.groups {
        let picked: Vec<&DuplicateFile> =
            group.files.iter().filter(|f| chosen.contains(f.path.as_str())).collect();
        if picked.len() == group.files.len() {
            return Err(format!(
                "Every copy of {} is selected. Keep at least one copy.",
                group.files[0].name
            ));
        }
        accepted.extend(picked.iter().map(|f| (f.path.clone(), group.size)));
    }
    if accepted.len() != chosen.len() {
        return Err("The selection includes a file that is not part of this scan.".to_owned());
    }
    Ok(accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn pool() -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap()
    }

    fn run(root: &Path) -> DuplicateReport {
        let options = DuplicateOptions {
            roots: vec![root.to_path_buf()],
            exclusions: Vec::new(),
            min_size: 1,
        };
        scan(options, &CancelToken::default(), &pool(), |_| {})
    }

    fn write(path: &Path, content: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn groups_identical_files_and_computes_recoverable_space() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a/one.bin"), &[1; 5000]);
        write(&dir.path().join("b/two.bin"), &[1; 5000]);
        write(&dir.path().join("c/three.bin"), &[1; 5000]);
        write(&dir.path().join("unique.bin"), &[2; 5000]);
        let report = run(dir.path());
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 3);
        assert_eq!(report.recoverable, 10_000);
    }

    #[test]
    fn same_size_different_content_is_not_a_duplicate() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), &[1; 4096]);
        write(&dir.path().join("b.bin"), &[2; 4096]);
        assert!(run(dir.path()).groups.is_empty());
    }

    #[test]
    fn files_differing_only_in_the_middle_are_told_apart() {
        let dir = tempdir().unwrap();
        let mut a = vec![0u8; SAMPLE * 4];
        let mut b = a.clone();
        a[SAMPLE * 2] = 1;
        b[SAMPLE * 2] = 2;
        write(&dir.path().join("a.bin"), &a);
        write(&dir.path().join("b.bin"), &b);
        assert!(run(dir.path()).groups.is_empty());
    }

    #[test]
    fn hard_links_are_not_duplicates() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), &[1; 5000]);
        fs::hard_link(dir.path().join("a.bin"), dir.path().join("b.bin")).unwrap();
        assert!(run(dir.path()).groups.is_empty());
    }

    #[test]
    fn symlinks_are_not_followed() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), &[1; 5000]);
        std::os::unix::fs::symlink(dir.path().join("a.bin"), dir.path().join("link.bin")).unwrap();
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();
        assert!(run(dir.path()).groups.is_empty());
    }

    #[test]
    fn opaque_packages_and_excluded_dirs_are_skipped() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("keep.bin"), &[1; 5000]);
        write(&dir.path().join("Thing.app/Contents/copy.bin"), &[1; 5000]);
        write(&dir.path().join("node_modules/copy.bin"), &[1; 5000]);
        assert!(run(dir.path()).groups.is_empty());
    }

    #[test]
    fn selecting_every_copy_is_refused() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), &[1; 5000]);
        write(&dir.path().join("b.bin"), &[1; 5000]);
        let report = run(dir.path());
        let all: Vec<String> = report.groups[0].files.iter().map(|f| f.path.clone()).collect();
        assert!(check_selection(&report, &all).is_err());
        assert_eq!(check_selection(&report, &all[..1]).unwrap().len(), 1);
    }

    #[test]
    fn selecting_a_path_outside_the_report_is_refused() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), &[1; 5000]);
        write(&dir.path().join("b.bin"), &[1; 5000]);
        let report = run(dir.path());
        assert!(check_selection(&report, &["/etc/hosts".to_owned()]).is_err());
    }

    #[test]
    fn cancellation_returns_a_partial_report() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), &[1; 5000]);
        let token = CancelToken::default();
        token.cancel();
        let options = DuplicateOptions { roots: vec![dir.path().into()], exclusions: Vec::new(), min_size: 1 };
        let report = scan(options, &token, &pool(), |_| {});
        assert!(!report.completed);
    }
}

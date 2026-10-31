//! Parallel disk walker. Width is bounded by the rayon pool, memory by keeping only the
//! largest entries per directory, and symlinks are never followed.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::classify::{
    file_kind, hog_for_dir, hog_for_file, Category, FileFacts, FileKind, HogKind, Locations,
};
use crate::db::DirCacheEntry;
use crate::jobs::CancelToken;
use crate::util::{allocated_size, now_ms, path_string};

const MIB: u64 = 1024 * 1024;
/// Children retained per directory; the remainder collapses into one aggregate node.
const MAX_CHILDREN: usize = 96;
/// Items below this stay out of the tree unless their parent is small enough to list.
const MIN_NODE_SIZE: u64 = MIB;
const TOP_FILES: usize = 1500;
const HOGS_PER_KIND: usize = 120;
/// Directories with at least this many files are worth caching.
const CACHE_MIN_FILES: u64 = 48;
/// A directory holding a file this large must always be re-read so it stays in the top lists.
const CACHE_MAX_FILE: u64 = 10 * MIB;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub modified: i64,
    pub kind: FileKind,
    pub category: Category,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub aggregate: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub denied: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: i64,
    pub accessed: i64,
    pub kind: FileKind,
    pub category: Category,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HogItem {
    pub path: String,
    pub name: String,
    pub kind: HogKind,
    pub size: u64,
    pub files: u64,
    pub modified: i64,
    pub accessed: i64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStat {
    pub category: Category,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OldFileStat {
    pub cutoff_months: u32,
    pub bytes: u64,
    pub files: u64,
    pub downloads_bytes: u64,
    pub downloads_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub started_at: i64,
    pub finished_at: i64,
    pub roots: Vec<String>,
    pub completed: bool,
    pub total_bytes: u64,
    pub files: u64,
    pub directories: u64,
    pub inaccessible: u64,
    pub reused_directories: u64,
    pub categories: Vec<CategoryStat>,
    pub over_100_mb: (u64, u64),
    pub over_1_gb: (u64, u64),
    pub old: OldFileStat,
    pub top_files: Vec<FileItem>,
    pub hogs: Vec<HogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub summary: ScanSummary,
    pub tree: Node,
}

pub struct ScanOptions {
    pub roots: Vec<PathBuf>,
    pub exclusions: Vec<PathBuf>,
    pub home: PathBuf,
    pub old_file_months: u32,
    /// Prior directory listings; `None` forces a full read of every directory.
    pub cache: Option<HashMap<String, DirCacheEntry>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
    pub inaccessible: u64,
    pub current: String,
    pub discoveries: Vec<Discovery>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub path: String,
    pub size: u64,
}

struct Ctx<'a> {
    options: &'a ScanOptions,
    token: &'a CancelToken,
    locations: Locations,
    now_secs: i64,
    old_cutoff: i64,
    files: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
    denied: AtomicU64,
    reused: AtomicU64,
    over_100: (AtomicU64, AtomicU64),
    over_1g: (AtomicU64, AtomicU64),
    old: [AtomicU64; 4],
    categories: [AtomicU64; Category::ALL.len()],
    top: Mutex<Vec<FileItem>>,
    top_floor: AtomicU64,
    hogs: Mutex<Vec<HogItem>>,
    hardlinks: Mutex<HashSet<(u64, u64)>>,
    visited: Mutex<HashSet<(u64, u64)>>,
    current: Mutex<String>,
    discoveries: Mutex<Vec<Discovery>>,
    fresh_cache: Mutex<Vec<(String, DirCacheEntry)>>,
}

fn unix_secs(time: std::io::Result<SystemTime>) -> i64 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn mtime_ns(meta: &fs::Metadata) -> i64 {
    meta.mtime().saturating_mul(1_000_000_000).saturating_add(meta.mtime_nsec())
}

impl Ctx<'_> {
    fn excluded(&self, path: &Path) -> bool {
        self.options.exclusions.iter().any(|e| path.starts_with(e))
    }

    fn add_category(&self, category: Category, bytes: u64) {
        self.categories[category.index()].fetch_add(bytes, Ordering::Relaxed);
    }

    fn note_file(&self, path: &Path, name: &str, meta: &fs::Metadata, size: u64, kind: FileKind, category: Category) {
        let modified = unix_secs(meta.modified());
        let accessed = unix_secs(meta.accessed());
        if size >= 100 * MIB {
            self.over_100.0.fetch_add(1, Ordering::Relaxed);
            self.over_100.1.fetch_add(size, Ordering::Relaxed);
            self.discoveries.lock().push(Discovery { path: path_string(path), size });
        }
        if size >= 1024 * MIB {
            self.over_1g.0.fetch_add(1, Ordering::Relaxed);
            self.over_1g.1.fetch_add(size, Ordering::Relaxed);
        }
        let in_downloads = self.locations.is_downloads(path);
        if modified > 0 && modified < self.old_cutoff {
            self.old[0].fetch_add(size, Ordering::Relaxed);
            self.old[1].fetch_add(1, Ordering::Relaxed);
            if in_downloads {
                self.old[2].fetch_add(size, Ordering::Relaxed);
                self.old[3].fetch_add(1, Ordering::Relaxed);
            }
        }
        if size >= self.top_floor.load(Ordering::Relaxed).max(10 * MIB) {
            let mut top = self.top.lock();
            top.push(FileItem {
                path: path_string(path),
                name: name.to_owned(),
                size,
                modified,
                accessed,
                kind,
                category,
            });
            if top.len() >= TOP_FILES * 2 {
                top.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
                top.truncate(TOP_FILES);
                self.top_floor.store(top[top.len() - 1].size, Ordering::Relaxed);
            }
        }
        let facts = FileFacts {
            size,
            age_days: (self.now_secs - modified.max(0)) / 86_400,
            in_downloads,
        };
        if let Some(hog) = hog_for_file(path, name, kind, &facts) {
            self.hogs.lock().push(HogItem {
                path: path_string(path),
                name: name.to_owned(),
                kind: hog,
                size,
                files: 1,
                modified,
                accessed,
                is_dir: false,
            });
        }
    }
}

struct DirListing {
    direct_size: u64,
    direct_files: u64,
    largest_file: u64,
    newest: i64,
    subdirs: Vec<(PathBuf, String)>,
    /// Large files kept as leaf nodes.
    leaves: Vec<Node>,
    by_category: [u64; Category::ALL.len()],
}

fn read_listing(ctx: &Ctx<'_>, path: &Path, context: Option<Category>, dev: u64) -> Option<DirListing> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            ctx.denied.fetch_add(1, Ordering::Relaxed);
            return None;
        }
    };
    let mut listing = DirListing {
        direct_size: 0,
        direct_files: 0,
        largest_file: 0,
        newest: 0,
        subdirs: Vec::new(),
        leaves: Vec::new(),
        by_category: [0; Category::ALL.len()],
    };

    for entry in entries.flatten() {
        if ctx.token.is_cancelled() {
            break;
        }
        let child = entry.path();
        // lstat: a symlink is measured as itself and never traversed.
        let Ok(meta) = fs::symlink_metadata(&child) else {
            ctx.denied.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if meta.is_dir() {
            if meta.dev() == dev && !ctx.excluded(&child) {
                listing.subdirs.push((child, name));
            }
            continue;
        }
        if !meta.is_file() && !meta.file_type().is_symlink() {
            continue;
        }
        if meta.nlink() > 1 && !ctx.hardlinks.lock().insert((meta.dev(), meta.ino())) {
            continue;
        }
        let size = allocated_size(&meta);
        let kind = if meta.is_file() { file_kind(&child) } else { FileKind::Other };
        let category = ctx.locations.category_for_file(context, kind);
        listing.direct_size += size;
        listing.direct_files += 1;
        listing.largest_file = listing.largest_file.max(size);
        listing.by_category[category.index()] += size;
        let modified = unix_secs(meta.modified());
        listing.newest = listing.newest.max(modified);

        let total = ctx.files.fetch_add(1, Ordering::Relaxed);
        ctx.bytes.fetch_add(size, Ordering::Relaxed);
        if total % 2048 == 0 {
            *ctx.current.lock() = path_string(&child);
        }
        ctx.note_file(&child, &name, &meta, size, kind, category);
        if size >= MIN_NODE_SIZE {
            listing.leaves.push(Node {
                name,
                size,
                files: 1,
                modified,
                kind,
                category,
                aggregate: false,
                denied: false,
                children: Vec::new(),
            });
        }
    }
    Some(listing)
}

fn cached_listing(ctx: &Ctx<'_>, path: &Path, meta: &fs::Metadata, dev: u64) -> Option<DirListing> {
    let cache = ctx.options.cache.as_ref()?;
    let key = path.to_str()?;
    let entry = cache.get(key)?;
    if entry.mtime_ns != mtime_ns(meta) {
        return None;
    }
    let mut by_category = [0u64; Category::ALL.len()];
    let rollup: Vec<(Category, u64)> = serde_json::from_str(&entry.categories).ok()?;
    for (category, bytes) in rollup {
        by_category[category.index()] += bytes;
    }
    // Subdirectories are still visited, so a change deeper down is always seen.
    let subdirs = entry
        .subdirs
        .iter()
        .filter_map(|name| {
            let child = path.join(name);
            let child_meta = fs::symlink_metadata(&child).ok()?;
            (child_meta.is_dir() && child_meta.dev() == dev && !ctx.excluded(&child))
                .then(|| (child, name.clone()))
        })
        .collect();
    ctx.reused.fetch_add(1, Ordering::Relaxed);
    ctx.files.fetch_add(entry.direct_files, Ordering::Relaxed);
    ctx.bytes.fetch_add(entry.direct_size, Ordering::Relaxed);
    Some(DirListing {
        direct_size: entry.direct_size,
        direct_files: entry.direct_files,
        largest_file: 0,
        newest: meta.mtime(),
        subdirs,
        leaves: Vec::new(),
        by_category,
    })
}

fn remember_listing(ctx: &Ctx<'_>, path: &Path, meta: &fs::Metadata, listing: &DirListing) {
    if listing.direct_files < CACHE_MIN_FILES || listing.largest_file >= CACHE_MAX_FILE {
        return;
    }
    let Some(key) = path.to_str() else {
        return;
    };
    let rollup: Vec<(Category, u64)> = Category::ALL
        .iter()
        .map(|c| (*c, listing.by_category[c.index()]))
        .filter(|(_, b)| *b > 0)
        .collect();
    ctx.fresh_cache.lock().push((
        key.to_owned(),
        DirCacheEntry {
            mtime_ns: mtime_ns(meta),
            direct_size: listing.direct_size,
            direct_files: listing.direct_files,
            subdirs: listing.subdirs.iter().map(|(_, n)| n.clone()).collect(),
            categories: serde_json::to_string(&rollup).unwrap_or_else(|_| "[]".into()),
        },
    ));
}

fn scan_dir(ctx: &Ctx<'_>, path: &Path, name: String, context: Option<Category>, in_hog: bool) -> Node {
    let mut node = Node {
        name,
        size: 0,
        files: 0,
        modified: 0,
        kind: FileKind::Folder,
        category: context.unwrap_or(Category::Other),
        aggregate: false,
        denied: false,
        children: Vec::new(),
    };
    if ctx.token.is_cancelled() {
        return node;
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        node.denied = true;
        ctx.denied.fetch_add(1, Ordering::Relaxed);
        return node;
    };
    // Directory hard links exist on old HFS+ backups; the visited set breaks any cycle.
    if !ctx.visited.lock().insert((meta.dev(), meta.ino())) {
        return node;
    }
    let dev = meta.dev();
    let context = ctx.locations.context_for(path, &node.name, context);
    node.category = context.unwrap_or(Category::Other);
    ctx.dirs.fetch_add(1, Ordering::Relaxed);

    let listing = match cached_listing(ctx, path, &meta, dev) {
        Some(listing) => listing,
        None => {
            let Some(listing) = read_listing(ctx, path, context, dev) else {
                node.denied = true;
                return node;
            };
            remember_listing(ctx, path, &meta, &listing);
            listing
        }
    };
    for category in Category::ALL {
        ctx.add_category(category, listing.by_category[category.index()]);
    }

    let dir_size = allocated_size(&meta);
    ctx.bytes.fetch_add(dir_size, Ordering::Relaxed);
    ctx.add_category(node.category, dir_size);

    let siblings: HashSet<&str> = listing.subdirs.iter().map(|(_, n)| n.as_str()).collect();
    let hog_marker = |p: &Path, n: &str| {
        hog_for_dir(p, n, |sibling| siblings.contains(sibling) || p.with_file_name(sibling).exists())
    };
    let hog_map: Vec<Option<HogKind>> = listing
        .subdirs
        .iter()
        .map(|(p, n)| if in_hog { None } else { hog_marker(p, n) })
        .collect();

    let mut children: Vec<Node> = listing
        .subdirs
        .into_par_iter()
        .zip(hog_map.into_par_iter())
        .map(|((child, child_name), hog)| {
            let child_node = scan_dir(ctx, &child, child_name, context, in_hog || hog.is_some());
            if let Some(kind) = hog {
                if child_node.size >= 20 * MIB {
                    let meta = fs::symlink_metadata(&child).ok();
                    ctx.hogs.lock().push(HogItem {
                        path: path_string(&child),
                        name: child_node.name.clone(),
                        kind,
                        size: child_node.size,
                        files: child_node.files,
                        modified: child_node.modified,
                        accessed: meta.map_or(0, |m| unix_secs(m.accessed())),
                        is_dir: true,
                    });
                }
            }
            child_node
        })
        .collect();

    node.size = dir_size + listing.direct_size + children.iter().map(|c| c.size).sum::<u64>();
    node.files = listing.direct_files + children.iter().map(|c| c.files).sum::<u64>();
    node.modified = children.iter().map(|c| c.modified).fold(listing.newest, i64::max);

    children.extend(listing.leaves);
    node.children = compact_children(children, node.size, node.files);
    node
}

/// Keeps the largest children and folds the rest into a single "smaller items" node,
/// which is what bounds memory on trees with millions of files.
fn compact_children(mut children: Vec<Node>, total: u64, total_files: u64) -> Vec<Node> {
    if total < MIN_NODE_SIZE {
        return Vec::new();
    }
    children.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
    let keep = children
        .iter()
        .take(MAX_CHILDREN)
        .take_while(|c| c.size >= MIN_NODE_SIZE)
        .count();
    children.truncate(keep);
    let kept_size: u64 = children.iter().map(|c| c.size).sum();
    let kept_files: u64 = children.iter().map(|c| c.files).sum();
    let rest = total.saturating_sub(kept_size);
    if rest > 0 {
        children.push(Node {
            name: "Smaller items".into(),
            size: rest,
            files: total_files.saturating_sub(kept_files),
            modified: 0,
            kind: FileKind::Other,
            category: Category::Other,
            aggregate: true,
            denied: false,
            children: Vec::new(),
        });
    }
    children
}

fn finish_hogs(mut hogs: Vec<HogItem>) -> Vec<HogItem> {
    hogs.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
    let mut counts: HashMap<HogKind, usize> = HashMap::new();
    hogs.retain(|h| {
        let count = counts.entry(h.kind).or_default();
        *count += 1;
        *count <= HOGS_PER_KIND
    });
    hogs
}

/// Runs a scan on the caller's rayon pool. `on_progress` fires roughly eight times a second.
pub fn run(
    options: ScanOptions,
    token: &CancelToken,
    pool: &rayon::ThreadPool,
    on_progress: impl Fn(ScanProgress) + Send + Sync,
) -> (ScanResult, Vec<(String, DirCacheEntry)>) {
    let started_at = now_ms();
    let now_secs = started_at / 1000;
    let ctx = Ctx {
        locations: Locations::new(options.home.clone()),
        options: &options,
        token,
        now_secs,
        old_cutoff: now_secs - i64::from(options.old_file_months) * 30 * 86_400,
        files: AtomicU64::new(0),
        dirs: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        denied: AtomicU64::new(0),
        reused: AtomicU64::new(0),
        over_100: Default::default(),
        over_1g: Default::default(),
        old: Default::default(),
        categories: Default::default(),
        top: Mutex::new(Vec::new()),
        top_floor: AtomicU64::new(0),
        hogs: Mutex::new(Vec::new()),
        hardlinks: Mutex::new(HashSet::new()),
        visited: Mutex::new(HashSet::new()),
        current: Mutex::new(String::new()),
        discoveries: Mutex::new(Vec::new()),
        fresh_cache: Mutex::new(Vec::new()),
    };

    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let children = std::thread::scope(|scope| {
        let (ctx, on_progress) = (&ctx, &on_progress);
        scope.spawn(move || {
            while done_rx.recv_timeout(Duration::from_millis(120)).is_err() {
                on_progress(ScanProgress {
                    files: ctx.files.load(Ordering::Relaxed),
                    directories: ctx.dirs.load(Ordering::Relaxed),
                    bytes: ctx.bytes.load(Ordering::Relaxed),
                    inaccessible: ctx.denied.load(Ordering::Relaxed),
                    current: ctx.current.lock().clone(),
                    discoveries: std::mem::take(&mut *ctx.discoveries.lock()),
                });
            }
        });
        let nodes: Vec<Node> = pool.install(|| {
            options
                .roots
                .par_iter()
                .map(|root| scan_dir(ctx, root, path_string(root), None, false))
                .collect()
        });
        let _ = done_tx.send(());
        nodes
    });

    let mut top_files = std::mem::take(&mut *ctx.top.lock());
    top_files.sort_unstable_by_key(|x| std::cmp::Reverse(x.size));
    top_files.truncate(TOP_FILES);

    let total_bytes: u64 = children.iter().map(|c| c.size).sum();
    let files: u64 = children.iter().map(|c| c.files).sum();
    let summary = ScanSummary {
        started_at,
        finished_at: now_ms(),
        roots: options.roots.iter().map(|r| path_string(r)).collect(),
        completed: !token.is_cancelled(),
        total_bytes,
        files,
        directories: ctx.dirs.load(Ordering::Relaxed),
        inaccessible: ctx.denied.load(Ordering::Relaxed),
        reused_directories: ctx.reused.load(Ordering::Relaxed),
        categories: Category::ALL
            .iter()
            .map(|c| CategoryStat {
                category: *c,
                bytes: ctx.categories[c.index()].load(Ordering::Relaxed),
            })
            .collect(),
        over_100_mb: (ctx.over_100.0.load(Ordering::Relaxed), ctx.over_100.1.load(Ordering::Relaxed)),
        over_1_gb: (ctx.over_1g.0.load(Ordering::Relaxed), ctx.over_1g.1.load(Ordering::Relaxed)),
        old: OldFileStat {
            cutoff_months: options.old_file_months,
            bytes: ctx.old[0].load(Ordering::Relaxed),
            files: ctx.old[1].load(Ordering::Relaxed),
            downloads_bytes: ctx.old[2].load(Ordering::Relaxed),
            downloads_files: ctx.old[3].load(Ordering::Relaxed),
        },
        top_files,
        hogs: finish_hogs(std::mem::take(&mut *ctx.hogs.lock())),
    };
    let tree = Node {
        name: "Scanned locations".into(),
        size: total_bytes,
        files,
        modified: children.iter().map(|c| c.modified).max().unwrap_or(0),
        kind: FileKind::Folder,
        category: Category::Other,
        aggregate: false,
        denied: false,
        children,
    };
    let cache = std::mem::take(&mut *ctx.fresh_cache.lock());
    (ScanResult { summary, tree }, cache)
}

/// Fast size-only walk used to measure a single directory on demand.
pub fn measure(path: &Path, token: &CancelToken) -> (u64, u64) {
    let Ok(entries) = fs::read_dir(path) else {
        return (0, 0);
    };
    let mut subdirs = Vec::new();
    let (mut size, mut files) = (0u64, 0u64);
    for entry in entries.flatten() {
        if token.is_cancelled() {
            break;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            size += allocated_size(&meta);
            subdirs.push(entry.path());
        } else {
            size += allocated_size(&meta);
            files += 1;
        }
    }
    let (sub_size, sub_files) = subdirs
        .par_iter()
        .map(|d| measure(d, token))
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    (size + sub_size, files + sub_files)
}

impl Node {
    /// Resolves an absolute path against the retained tree.
    pub fn find(&self, path: &Path) -> Option<&Node> {
        let root = self.children.iter().find_map(|c| {
            let relative = path.strip_prefix(&c.name).ok()?;
            Some((c, relative))
        });
        let (mut node, relative) = root?;
        for part in relative.components() {
            let part = part.as_os_str().to_string_lossy();
            node = node.children.iter().find(|c| c.name == part)?;
        }
        Some(node)
    }

    /// Clone limited to `depth` levels below this node.
    pub fn truncated(&self, depth: u32) -> Node {
        let mut copy = Node { children: Vec::new(), ..self.clone_shallow() };
        if depth > 0 {
            copy.children = self.children.iter().map(|c| c.truncated(depth - 1)).collect();
        }
        copy
    }

    fn clone_shallow(&self) -> Node {
        Node {
            name: self.name.clone(),
            size: self.size,
            files: self.files,
            modified: self.modified,
            kind: self.kind,
            category: self.category,
            aggregate: self.aggregate,
            denied: self.denied,
            children: Vec::new(),
        }
    }

    /// Copy for persistence: shallow levels in full, deep levels only when substantial.
    pub fn pruned(&self, depth: u32) -> Node {
        let mut copy = self.clone_shallow();
        copy.children = self
            .children
            .iter()
            .filter(|c| depth < 3 || c.size >= 16 * MIB)
            .map(|c| c.pruned(depth + 1))
            .collect();
        copy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    fn options(root: &Path) -> ScanOptions {
        ScanOptions {
            roots: vec![root.canonicalize().unwrap()],
            exclusions: Vec::new(),
            home: root.canonicalize().unwrap(),
            old_file_months: 12,
            cache: None,
        }
    }

    fn pool() -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new().num_threads(2).build().unwrap()
    }

    fn scan(options: ScanOptions) -> (ScanResult, Vec<(String, DirCacheEntry)>) {
        run(options, &CancelToken::default(), &pool(), |_| {})
    }

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn totals_match_files_on_disk() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a/one.bin"), 8192);
        write(&dir.path().join("a/b/two.bin"), 8192);
        let (result, _) = scan(options(dir.path()));
        assert!(result.summary.completed);
        assert_eq!(result.summary.files, 2);
        assert!(result.summary.total_bytes >= 16_384);
        assert_eq!(result.summary.directories, 3);
    }

    #[test]
    fn symlink_cycles_terminate_and_are_not_followed() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("real/file.bin"), 4096);
        symlink(dir.path(), dir.path().join("real/loop")).unwrap();
        symlink("/usr", dir.path().join("outside")).unwrap();
        let (result, _) = scan(options(dir.path()));
        // The file plus two symlinks measured as themselves; /usr was never entered.
        assert_eq!(result.summary.files, 3);
        assert_eq!(result.summary.directories, 2);
    }

    #[test]
    fn hard_links_are_counted_once() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), 1 << 20);
        fs::hard_link(dir.path().join("a.bin"), dir.path().join("b.bin")).unwrap();
        let (result, _) = scan(options(dir.path()));
        assert_eq!(result.summary.files, 1);
    }

    #[test]
    fn sparse_files_report_allocated_size() {
        let dir = tempdir().unwrap();
        let file = fs::File::create(dir.path().join("sparse.raw")).unwrap();
        file.set_len(1 << 32).unwrap();
        let (result, _) = scan(options(dir.path()));
        assert!(result.summary.total_bytes < 1 << 24, "got {}", result.summary.total_bytes);
    }

    #[test]
    fn exclusions_are_skipped() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("keep/a.bin"), 4096);
        write(&dir.path().join("skip/b.bin"), 4096);
        let mut opts = options(dir.path());
        opts.exclusions.push(dir.path().canonicalize().unwrap().join("skip"));
        assert_eq!(scan(opts).0.summary.files, 1);
    }

    #[test]
    fn cancellation_stops_early_and_is_reported() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("a.bin"), 4096);
        let token = CancelToken::default();
        token.cancel();
        let (result, _) = run(options(dir.path()), &token, &pool(), |_| {});
        assert!(!result.summary.completed);
        assert_eq!(result.summary.files, 0);
    }

    #[test]
    fn unreadable_directories_are_counted_not_fatal() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        write(&dir.path().join("ok.bin"), 4096);
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let (result, _) = scan(options(dir.path()));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result.summary.inaccessible, 1);
        assert_eq!(result.summary.files, 1);
    }

    #[test]
    fn nested_node_modules_is_reported_once() {
        let dir = tempdir().unwrap();
        let outer = dir.path().join("app/node_modules");
        write(&outer.join("big.js"), 21 << 20);
        write(&outer.join("dep/node_modules/inner.js"), 21 << 20);
        let (result, _) = scan(options(dir.path()));
        let hogs: Vec<_> =
            result.summary.hogs.iter().filter(|h| h.kind == HogKind::NodeModules).collect();
        assert_eq!(hogs.len(), 1);
        assert!(hogs[0].size >= 42 << 20);
    }

    #[test]
    fn cache_reuses_unchanged_directories_and_notices_changes() {
        let dir = tempdir().unwrap();
        for i in 0..60 {
            write(&dir.path().join(format!("many/f{i}.bin")), 4096);
        }
        let (first, cache) = scan(options(dir.path()));
        assert_eq!(cache.len(), 1);
        let cache: HashMap<_, _> = cache.into_iter().collect();

        let mut opts = options(dir.path());
        opts.cache = Some(cache);
        let (second, cache) = scan(opts);
        assert_eq!(second.summary.reused_directories, 1);
        assert_eq!(second.summary.files, first.summary.files);
        assert_eq!(second.summary.total_bytes, first.summary.total_bytes);
        assert!(cache.is_empty());

        let (_, cache) = scan(options(dir.path()));
        std::thread::sleep(Duration::from_millis(10));
        write(&dir.path().join("many/new.bin"), 4096);
        let mut opts = options(dir.path());
        opts.cache = Some(cache.into_iter().collect());
        let (third, _) = scan(opts);
        assert_eq!(third.summary.reused_directories, 0);
        assert_eq!(third.summary.files, 61);
    }

    #[test]
    fn wide_directories_collapse_into_an_aggregate_without_losing_bytes() {
        let children: Vec<Node> = (0..300)
            .map(|i| Node {
                name: format!("f{i}"),
                size: 2 * MIB + i,
                files: 1,
                modified: 0,
                kind: FileKind::Other,
                category: Category::Other,
                aggregate: false,
                denied: false,
                children: Vec::new(),
            })
            .collect();
        let total: u64 = children.iter().map(|c| c.size).sum();
        let compact = compact_children(children, total, 300);
        assert_eq!(compact.len(), MAX_CHILDREN + 1);
        assert!(compact.last().unwrap().aggregate);
        assert_eq!(compact.iter().map(|c| c.size).sum::<u64>(), total);
    }

    #[test]
    fn find_resolves_absolute_paths_in_the_tree() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("x/y/big.bin"), 2 << 20);
        let root = dir.path().canonicalize().unwrap();
        let (result, _) = scan(options(dir.path()));
        let node = result.tree.find(&root.join("x/y")).unwrap();
        assert_eq!(node.files, 1);
        assert!(result.tree.find(&root.join("missing")).is_none());
    }
}

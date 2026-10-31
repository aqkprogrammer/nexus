-- NEXUS local database. Everything here stays on this Mac.

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- One row per completed storage, cleaner or duplicate scan.
CREATE TABLE scans (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    kind          TEXT    NOT NULL CHECK (kind IN ('storage', 'cleaner', 'duplicates')),
    started_at    INTEGER NOT NULL,
    finished_at   INTEGER NOT NULL,
    status        TEXT    NOT NULL CHECK (status IN ('completed', 'cancelled', 'failed')),
    roots         TEXT    NOT NULL DEFAULT '[]',
    total_bytes   INTEGER NOT NULL DEFAULT 0,
    files         INTEGER NOT NULL DEFAULT 0,
    reclaimable   INTEGER NOT NULL DEFAULT 0,
    inaccessible  INTEGER NOT NULL DEFAULT 0,
    disk_used     INTEGER,
    disk_total    INTEGER,
    health_score  REAL,
    summary       TEXT    NOT NULL DEFAULT '{}'
);
CREATE INDEX idx_scans_kind_time ON scans (kind, finished_at DESC);

-- Serialized, pruned result of the latest storage scan so the map survives a restart.
CREATE TABLE scan_trees (
    scan_id INTEGER PRIMARY KEY REFERENCES scans (id) ON DELETE CASCADE,
    tree    BLOB NOT NULL
);

-- Directory listing cache used to avoid re-reading unchanged large directories.
CREATE TABLE dir_cache (
    path        TEXT PRIMARY KEY,
    mtime_ns    INTEGER NOT NULL,
    direct_size INTEGER NOT NULL,
    direct_files INTEGER NOT NULL,
    subdirs     TEXT    NOT NULL,
    categories  TEXT    NOT NULL,
    seen_at     INTEGER NOT NULL
);

CREATE TABLE cleanup_runs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    source        TEXT    NOT NULL,
    started_at    INTEGER NOT NULL,
    finished_at   INTEGER NOT NULL,
    dry_run       INTEGER NOT NULL,
    mode          TEXT    NOT NULL,
    status        TEXT    NOT NULL,
    bytes_freed   INTEGER NOT NULL DEFAULT 0,
    bytes_trashed INTEGER NOT NULL DEFAULT 0,
    items_ok      INTEGER NOT NULL DEFAULT 0,
    items_failed  INTEGER NOT NULL DEFAULT 0,
    items_skipped INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_cleanup_runs_time ON cleanup_runs (finished_at DESC);

-- Per-item audit log. Written for dry runs as well.
CREATE TABLE cleanup_items (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id  INTEGER NOT NULL REFERENCES cleanup_runs (id) ON DELETE CASCADE,
    path    TEXT    NOT NULL,
    size    INTEGER NOT NULL,
    action  TEXT    NOT NULL,
    outcome TEXT    NOT NULL,
    detail  TEXT
);
CREATE INDEX idx_cleanup_items_run ON cleanup_items (run_id);

-- Five-minute telemetry rollups used for long-term trends.
CREATE TABLE metric_samples (
    ts           INTEGER PRIMARY KEY,
    cpu          REAL NOT NULL,
    memory_used  INTEGER NOT NULL,
    memory_total INTEGER NOT NULL,
    disk_used    INTEGER NOT NULL,
    disk_total   INTEGER NOT NULL,
    battery      REAL,
    health       REAL NOT NULL,
    storage_score REAL NOT NULL,
    memory_score REAL NOT NULL,
    cpu_score    REAL NOT NULL,
    battery_score REAL,
    performance_score REAL NOT NULL
);

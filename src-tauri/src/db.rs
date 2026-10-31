use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::Result;

const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

pub struct Db {
    conn: Mutex<Connection>,
}

pub struct ScanRecord<'a> {
    pub kind: &'a str,
    pub started_at: i64,
    pub finished_at: i64,
    pub status: &'a str,
    pub roots: &'a [String],
    pub total_bytes: u64,
    pub files: u64,
    pub reclaimable: u64,
    pub inaccessible: u64,
    pub disk_used: Option<u64>,
    pub disk_total: Option<u64>,
    pub health_score: Option<f64>,
    pub summary: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRow {
    pub id: i64,
    pub kind: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub status: String,
    pub roots: Vec<String>,
    pub total_bytes: u64,
    pub files: u64,
    pub reclaimable: u64,
    pub inaccessible: u64,
    pub disk_used: Option<u64>,
    pub disk_total: Option<u64>,
    pub health_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRunRow {
    pub id: i64,
    pub source: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub dry_run: bool,
    pub mode: String,
    pub status: String,
    pub bytes_freed: u64,
    pub bytes_trashed: u64,
    pub items_ok: u64,
    pub items_failed: u64,
    pub items_skipped: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItemRow {
    pub path: String,
    pub size: u64,
    pub action: String,
    pub outcome: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricRow {
    pub ts: i64,
    pub cpu: f64,
    pub memory_used: u64,
    pub memory_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub battery: Option<f64>,
    pub health: f64,
    pub storage_score: f64,
    pub memory_score: f64,
    pub cpu_score: f64,
    pub battery_score: Option<f64>,
    pub performance_score: f64,
}

pub struct DirCacheEntry {
    pub mtime_ns: i64,
    pub direct_size: u64,
    pub direct_files: u64,
    pub subdirs: Vec<String>,
    /// JSON rollup of direct file bytes by storage category.
    pub categories: String,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::from_connection(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )?;
        migrate(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .lock()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn insert_scan(&self, scan: &ScanRecord<'_>) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO scans (kind, started_at, finished_at, status, roots, total_bytes, files,
                                reclaimable, inaccessible, disk_used, disk_total, health_score, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                scan.kind,
                scan.started_at,
                scan.finished_at,
                scan.status,
                serde_json::to_string(scan.roots)?,
                scan.total_bytes as i64,
                scan.files as i64,
                scan.reclaimable as i64,
                scan.inaccessible as i64,
                scan.disk_used.map(|v| v as i64),
                scan.disk_total.map(|v| v as i64),
                scan.health_score,
                scan.summary.to_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_scans(&self, kind: Option<&str>, limit: u32) -> Result<Vec<ScanRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, started_at, finished_at, status, roots, total_bytes, files, reclaimable,
                    inaccessible, disk_used, disk_total, health_score
             FROM scans WHERE (?1 IS NULL OR kind = ?1) ORDER BY finished_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![kind, limit], |r| {
            let roots: String = r.get(5)?;
            Ok(ScanRow {
                id: r.get(0)?,
                kind: r.get(1)?,
                started_at: r.get(2)?,
                finished_at: r.get(3)?,
                status: r.get(4)?,
                roots: serde_json::from_str(&roots).unwrap_or_default(),
                total_bytes: r.get::<_, i64>(6)? as u64,
                files: r.get::<_, i64>(7)? as u64,
                reclaimable: r.get::<_, i64>(8)? as u64,
                inaccessible: r.get::<_, i64>(9)? as u64,
                disk_used: r.get::<_, Option<i64>>(10)?.map(|v| v as u64),
                disk_total: r.get::<_, Option<i64>>(11)?.map(|v| v as u64),
                health_score: r.get(12)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn save_tree(&self, scan_id: i64, tree: &[u8]) -> Result<()> {
        let conn = self.conn.lock();
        // Only the newest tree is kept: it is a convenience cache, not history.
        conn.execute("DELETE FROM scan_trees", [])?;
        conn.execute(
            "INSERT INTO scan_trees (scan_id, tree) VALUES (?1, ?2)",
            params![scan_id, tree],
        )?;
        Ok(())
    }

    pub fn latest_tree(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .lock()
            .query_row("SELECT tree FROM scan_trees ORDER BY scan_id DESC LIMIT 1", [], |r| r.get(0))
            .optional()?)
    }

    pub fn load_dir_cache(&self) -> Result<std::collections::HashMap<String, DirCacheEntry>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT path, mtime_ns, direct_size, direct_files, subdirs, categories FROM dir_cache")?;
        let rows = stmt.query_map([], |r| {
            let subdirs: String = r.get(4)?;
            Ok((
                r.get::<_, String>(0)?,
                DirCacheEntry {
                    mtime_ns: r.get(1)?,
                    direct_size: r.get::<_, i64>(2)? as u64,
                    direct_files: r.get::<_, i64>(3)? as u64,
                    subdirs: serde_json::from_str(&subdirs).unwrap_or_default(),
                    categories: r.get(5)?,
                },
            ))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn replace_dir_cache(&self, entries: &[(String, DirCacheEntry)], now: i64) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM dir_cache", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO dir_cache (path, mtime_ns, direct_size, direct_files, subdirs, categories, seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (path, entry) in entries {
                stmt.execute(params![
                    path,
                    entry.mtime_ns,
                    entry.direct_size as i64,
                    entry.direct_files as i64,
                    serde_json::to_string(&entry.subdirs)?,
                    entry.categories,
                    now
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_cleanup_run(
        &self,
        run: &CleanupRunRow,
        items: &[CleanupItemRow],
    ) -> Result<i64> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO cleanup_runs (source, started_at, finished_at, dry_run, mode, status,
                                       bytes_freed, bytes_trashed, items_ok, items_failed, items_skipped)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run.source,
                run.started_at,
                run.finished_at,
                run.dry_run,
                run.mode,
                run.status,
                run.bytes_freed as i64,
                run.bytes_trashed as i64,
                run.items_ok as i64,
                run.items_failed as i64,
                run.items_skipped as i64
            ],
        )?;
        let run_id = tx.last_insert_rowid();
        {
            let mut stmt = tx.prepare(
                "INSERT INTO cleanup_items (run_id, path, size, action, outcome, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for item in items {
                stmt.execute(params![
                    run_id,
                    item.path,
                    item.size as i64,
                    item.action,
                    item.outcome,
                    item.detail
                ])?;
            }
        }
        tx.commit()?;
        Ok(run_id)
    }

    pub fn list_cleanup_runs(&self, limit: u32) -> Result<Vec<CleanupRunRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source, started_at, finished_at, dry_run, mode, status, bytes_freed,
                    bytes_trashed, items_ok, items_failed, items_skipped
             FROM cleanup_runs ORDER BY finished_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(CleanupRunRow {
                id: r.get(0)?,
                source: r.get(1)?,
                started_at: r.get(2)?,
                finished_at: r.get(3)?,
                dry_run: r.get(4)?,
                mode: r.get(5)?,
                status: r.get(6)?,
                bytes_freed: r.get::<_, i64>(7)? as u64,
                bytes_trashed: r.get::<_, i64>(8)? as u64,
                items_ok: r.get::<_, i64>(9)? as u64,
                items_failed: r.get::<_, i64>(10)? as u64,
                items_skipped: r.get::<_, i64>(11)? as u64,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn cleanup_items(&self, run_id: i64) -> Result<Vec<CleanupItemRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT path, size, action, outcome, detail FROM cleanup_items WHERE run_id = ?1 ORDER BY size DESC",
        )?;
        let rows = stmt.query_map([run_id], |r| {
            Ok(CleanupItemRow {
                path: r.get(0)?,
                size: r.get::<_, i64>(1)? as u64,
                action: r.get(2)?,
                outcome: r.get(3)?,
                detail: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn insert_metric(&self, row: &MetricRow) -> Result<()> {
        self.conn.lock().execute(
            "INSERT OR REPLACE INTO metric_samples
               (ts, cpu, memory_used, memory_total, disk_used, disk_total, battery, health,
                storage_score, memory_score, cpu_score, battery_score, performance_score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                row.ts,
                row.cpu,
                row.memory_used as i64,
                row.memory_total as i64,
                row.disk_used as i64,
                row.disk_total as i64,
                row.battery,
                row.health,
                row.storage_score,
                row.memory_score,
                row.cpu_score,
                row.battery_score,
                row.performance_score
            ],
        )?;
        Ok(())
    }

    pub fn list_metrics(&self, since_ms: i64) -> Result<Vec<MetricRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT ts, cpu, memory_used, memory_total, disk_used, disk_total, battery, health,
                    storage_score, memory_score, cpu_score, battery_score, performance_score
             FROM metric_samples WHERE ts >= ?1 ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map([since_ms], |r| {
            Ok(MetricRow {
                ts: r.get(0)?,
                cpu: r.get(1)?,
                memory_used: r.get::<_, i64>(2)? as u64,
                memory_total: r.get::<_, i64>(3)? as u64,
                disk_used: r.get::<_, i64>(4)? as u64,
                disk_total: r.get::<_, i64>(5)? as u64,
                battery: r.get(6)?,
                health: r.get(7)?,
                storage_score: r.get(8)?,
                memory_score: r.get(9)?,
                cpu_score: r.get(10)?,
                battery_score: r.get(11)?,
                performance_score: r.get(12)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Removes history older than the retention window. The audit log is kept with it.
    pub fn prune(&self, retention_days: u32) -> Result<()> {
        let cutoff = crate::util::now_ms() - i64::from(retention_days) * 86_400_000;
        let conn = self.conn.lock();
        conn.execute("DELETE FROM metric_samples WHERE ts < ?1", [cutoff])?;
        conn.execute("DELETE FROM scans WHERE finished_at < ?1", [cutoff])?;
        conn.execute("DELETE FROM cleanup_runs WHERE finished_at < ?1", [cutoff])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn.lock().execute_batch(
            "DELETE FROM metric_samples; DELETE FROM scans; DELETE FROM cleanup_runs; DELETE FROM dir_cache;",
        )?;
        Ok(())
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (index + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, finished_at: i64) -> CleanupRunRow {
        CleanupRunRow {
            id: 0,
            source: source.into(),
            started_at: finished_at - 10,
            finished_at,
            dry_run: true,
            mode: "trash".into(),
            status: "completed".into(),
            bytes_freed: 0,
            bytes_trashed: 42,
            items_ok: 1,
            items_failed: 0,
            items_skipped: 0,
        }
    }

    #[test]
    fn migrations_are_idempotent_and_versioned() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexus.db");
        drop(Db::open(&path).unwrap());
        let db = Db::open(&path).unwrap();
        let version: i64 =
            db.conn.lock().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn settings_round_trip_and_overwrite() {
        let db = Db::in_memory().unwrap();
        assert_eq!(db.get_setting("a").unwrap(), None);
        db.set_setting("a", "1").unwrap();
        db.set_setting("a", "2").unwrap();
        assert_eq!(db.get_setting("a").unwrap().as_deref(), Some("2"));
    }

    #[test]
    fn cleanup_runs_keep_item_audit_log() {
        let db = Db::in_memory().unwrap();
        let id = db
            .insert_cleanup_run(
                &run("cleaner", 1_000),
                &[CleanupItemRow {
                    path: "/tmp/x".into(),
                    size: 42,
                    action: "trash".into(),
                    outcome: "would-trash".into(),
                    detail: None,
                }],
            )
            .unwrap();
        assert_eq!(db.list_cleanup_runs(10).unwrap()[0].bytes_trashed, 42);
        assert_eq!(db.cleanup_items(id).unwrap()[0].outcome, "would-trash");
    }

    #[test]
    fn prune_drops_only_expired_history() {
        let db = Db::in_memory().unwrap();
        let now = crate::util::now_ms();
        db.insert_cleanup_run(&run("old", now - 40 * 86_400_000), &[]).unwrap();
        db.insert_cleanup_run(&run("new", now), &[]).unwrap();
        db.prune(30).unwrap();
        let runs = db.list_cleanup_runs(10).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].source, "new");
    }
}

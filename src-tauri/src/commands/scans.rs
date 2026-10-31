use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::notify;
use crate::advisor::gb;
use crate::apps::{self, leftovers, AppInfo, AssociatedItem};
use crate::cleaner::{self, CleanerReport};
use crate::db::ScanRecord;
use crate::duplicates::{self, DuplicateOptions, DuplicateReport};
use crate::error::{NexusError, Result};
use crate::jobs::{io_pool, CancelToken, JobGuard};
use crate::state::AppState;
use crate::storage::explorer::{self, Entry, Listing};
use crate::storage::scanner::{self, Node, ScanOptions, ScanResult, ScanSummary};
use crate::util::{home_dir, now_ms};

fn health_score(state: &AppState) -> Option<f64> {
    state.telemetry.latest().map(|s| s.health.overall)
}

fn exclusions(state: &AppState) -> Vec<PathBuf> {
    state.settings.read().exclusions.iter().map(PathBuf::from).collect()
}

fn spawn_job(name: &str, work: impl FnOnce() + Send + 'static) -> Result<()> {
    std::thread::Builder::new().name(format!("nexus-{name}")).spawn(work)?;
    Ok(())
}

fn fail(app: &AppHandle, event: &str, error: &NexusError) {
    let _ = app.emit(event, serde_json::json!({ "error": error.to_string() }));
}

#[tauri::command(async)]
pub fn start_storage_scan(app: AppHandle, state: State<'_, AppState>, deep: bool) -> Result<()> {
    begin_storage_scan(&app, &state, deep)
}

/// Shared by the command and the scheduled scan. Read-only, so it can run unattended.
pub fn begin_storage_scan(app: &AppHandle, state: &AppState, deep: bool) -> Result<()> {
    let roots: Vec<PathBuf> = state
        .approved_roots()
        .into_iter()
        .filter_map(|r| r.canonicalize().ok())
        .collect();
    if roots.is_empty() {
        return Err(NexusError::Permission("Approve at least one folder before scanning.".into()));
    }
    let guard = state.jobs.start("storage")?;
    let app = app.clone();
    spawn_job("storage", move || {
        if let Err(error) = run_storage_scan(&app, roots, deep, guard) {
            fail(&app, "storage://done", &error);
        }
    })
}

fn run_storage_scan(app: &AppHandle, roots: Vec<PathBuf>, deep: bool, guard: JobGuard) -> Result<()> {
    let state = app.state::<AppState>();
    let cache = if deep { None } else { state.db.load_dir_cache().ok() };
    let (old_file_months, exclusions) = {
        let settings = state.settings.read();
        (settings.old_file_months, settings.exclusions.iter().map(PathBuf::from).collect())
    };
    let options = ScanOptions {
        roots,
        exclusions,
        home: home_dir(),
        old_file_months,
        cache,
    };
    let pool = io_pool()?;
    let progress_app = app.clone();
    let (result, cache_entries) = scanner::run(options, &guard.token, &pool, move |progress| {
        let _ = progress_app.emit("storage://progress", progress);
    });

    if result.summary.completed {
        if !cache_entries.is_empty() {
            let _ = state.db.replace_dir_cache(&cache_entries, now_ms());
        }
        let volume = state.telemetry.latest().map(|s| s.volume);
        let summary = &result.summary;
        let id = state.db.insert_scan(&ScanRecord {
            kind: "storage",
            started_at: summary.started_at,
            finished_at: summary.finished_at,
            status: "completed",
            roots: &summary.roots,
            total_bytes: summary.total_bytes,
            files: summary.files,
            reclaimable: 0,
            inaccessible: summary.inaccessible,
            disk_used: volume.as_ref().map(|v| v.used),
            disk_total: volume.as_ref().map(|v| v.total),
            health_score: health_score(&state),
            summary: serde_json::json!({ "categories": summary.categories }),
        })?;
        let persisted = ScanResult { summary: summary.clone(), tree: result.tree.pruned(0) };
        state.db.save_tree(id, &serde_json::to_vec(&persisted)?)?;
        notify(
            app,
            "Storage scan complete",
            &format!("{} analyzed across {} files.", gb(summary.total_bytes), summary.files),
        );
        *state.storage.write() = Some(Arc::new(result));
    }
    let completed = state.storage.read().as_ref().map(|r| r.summary.clone());
    drop(guard);
    let _ = app.emit("storage://done", serde_json::json!({ "summary": completed }));
    Ok(())
}

#[tauri::command(async)]
pub fn get_storage_summary(state: State<'_, AppState>) -> Option<ScanSummary> {
    state.storage.read().as_ref().map(|r| r.summary.clone())
}

#[tauri::command(async)]
pub fn get_storage_tree(state: State<'_, AppState>, path: Option<String>, depth: u32) -> Option<Node> {
    let guard = state.storage.read();
    let result = guard.as_ref()?;
    let node = match path {
        Some(p) => result.tree.find(Path::new(&p))?,
        None => &result.tree,
    };
    Some(node.truncated(depth.min(3)))
}

#[tauri::command(async)]
pub fn list_directory(state: State<'_, AppState>, path: String, show_hidden: bool) -> Result<Listing> {
    let resolved = state.ensure_approved(Path::new(&path))?;
    let guard = state.storage.read();
    explorer::list(&resolved, show_hidden, guard.as_ref().map(|r| &r.tree))
}

#[tauri::command(async)]
pub fn measure_directory(state: State<'_, AppState>, path: String) -> Result<(u64, u64)> {
    let resolved = state.ensure_approved(Path::new(&path))?;
    let pool = io_pool()?;
    Ok(pool.install(|| scanner::measure(&resolved, &CancelToken::default())))
}

#[tauri::command(async)]
pub fn search_files(state: State<'_, AppState>, query: String) -> Vec<Entry> {
    let guard = state.storage.read();
    explorer::search(&query, &state.approved_roots(), guard.as_ref().map(|r| &r.tree))
}

#[tauri::command(async)]
pub fn reveal_in_finder(path: String) -> Result<()> {
    explorer::reveal(Path::new(&path))
}

#[tauri::command(async)]
pub fn start_cleaner_scan(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let guard = state.jobs.start("cleaner")?;
    let app_handle = app.clone();
    spawn_job("cleaner", move || {
        if let Err(error) = run_cleaner_scan(&app_handle, guard) {
            fail(&app_handle, "cleaner://done", &error);
        }
    })
}

fn installed_ids(apps: &[AppInfo]) -> Vec<String> {
    apps.iter().filter_map(|a| a.bundle_id.clone()).collect()
}

fn run_cleaner_scan(app: &AppHandle, guard: JobGuard) -> Result<()> {
    let state = app.state::<AppState>();
    let started = now_ms();
    let home = home_dir();
    let running = state.running_apps();
    let installed = apps::list(&home, &running);
    let hogs = state
        .storage
        .read()
        .as_ref()
        .map(|r| r.summary.hogs.clone())
        .unwrap_or_default();
    let input = cleaner::ScanInput {
        leftovers: leftovers::find(&home, &installed_ids(&installed)),
        home,
        defs: state.catalog(),
        running,
        hogs,
    };
    let pool = io_pool()?;
    let report = cleaner::scan(input, &guard.token, &pool, |progress| {
        let _ = app.emit("cleaner://progress", progress);
    });

    if report.completed {
        let volume = state.telemetry.latest().map(|s| s.volume);
        state.db.insert_scan(&ScanRecord {
            kind: "cleaner",
            started_at: started,
            finished_at: now_ms(),
            status: "completed",
            roots: &[],
            total_bytes: report.reclaimable + report.trash.total,
            files: report.targets.iter().map(|t| t.items.len() as u64).sum(),
            reclaimable: report.reclaimable,
            inaccessible: report.targets.iter().filter(|t| t.unavailable.is_some()).count() as u64,
            disk_used: volume.as_ref().map(|v| v.used),
            disk_total: volume.as_ref().map(|v| v.total),
            health_score: health_score(&state),
            summary: serde_json::json!({ "recommended": report.recommended }),
        })?;
        notify(app, "Cleaner scan complete", &format!("{} reclaimable found.", gb(report.reclaimable)));
    }
    let report = Arc::new(report);
    if report.completed {
        *state.cleaner.write() = Some(report.clone());
    }
    drop(guard);
    let _ = app.emit("cleaner://done", serde_json::json!({ "report": report }));
    Ok(())
}

#[tauri::command(async)]
pub fn get_cleaner_report(state: State<'_, AppState>) -> Option<Arc<CleanerReport>> {
    state.cleaner.read().clone()
}

#[tauri::command(async)]
pub fn start_duplicate_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    roots: Vec<String>,
    excluded: Vec<String>,
    min_size: u64,
) -> Result<()> {
    if roots.is_empty() {
        return Err(NexusError::Invalid("Choose at least one folder to scan.".into()));
    }
    let resolved = roots
        .iter()
        .map(|r| state.ensure_approved(Path::new(r)))
        .collect::<Result<Vec<_>>>()?;
    let mut all_excluded = exclusions(&state);
    all_excluded.extend(excluded.iter().map(PathBuf::from));
    let guard = state.jobs.start("duplicates")?;
    let options = DuplicateOptions { roots: resolved, exclusions: all_excluded, min_size: min_size.max(1) };
    let app_handle = app.clone();
    spawn_job("duplicates", move || {
        if let Err(error) = run_duplicate_scan(&app_handle, options, guard) {
            fail(&app_handle, "duplicates://done", &error);
        }
    })
}

fn run_duplicate_scan(app: &AppHandle, options: DuplicateOptions, guard: JobGuard) -> Result<()> {
    let state = app.state::<AppState>();
    let started = now_ms();
    let pool = io_pool()?;
    let progress_app = app.clone();
    let report = duplicates::scan(options, &guard.token, &pool, move |progress| {
        let _ = progress_app.emit("duplicates://progress", progress);
    });
    if report.completed {
        let volume = state.telemetry.latest().map(|s| s.volume);
        state.db.insert_scan(&ScanRecord {
            kind: "duplicates",
            started_at: started,
            finished_at: now_ms(),
            status: "completed",
            roots: &report.roots,
            total_bytes: report.bytes_hashed,
            files: report.files_indexed,
            reclaimable: report.recoverable,
            inaccessible: report.inaccessible,
            disk_used: volume.as_ref().map(|v| v.used),
            disk_total: volume.as_ref().map(|v| v.total),
            health_score: health_score(&state),
            summary: serde_json::json!({ "groups": report.groups.len() }),
        })?;
        notify(
            app,
            "Duplicate scan complete",
            &format!("{} groups, {} recoverable.", report.groups.len(), gb(report.recoverable)),
        );
    }
    let report = Arc::new(report);
    if report.completed {
        *state.duplicates.write() = Some(report.clone());
    }
    drop(guard);
    let _ = app.emit("duplicates://done", serde_json::json!({ "report": report }));
    Ok(())
}

#[tauri::command(async)]
pub fn get_duplicate_report(state: State<'_, AppState>) -> Option<Arc<DuplicateReport>> {
    state.duplicates.read().clone()
}

#[tauri::command(async)]
pub fn image_preview(state: State<'_, AppState>, path: String) -> Result<Option<String>> {
    let resolved = state.ensure_approved(Path::new(&path))?;
    Ok(apps::thumbnail_data_url(&resolved, &state.cache_dir.join("previews")))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSizeEvent {
    pub path: String,
    pub size: Option<u64>,
    pub data_size: Option<u64>,
}

/// Returns the list immediately and measures sizes in the background.
#[tauri::command(async)]
pub fn list_apps(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<AppInfo>> {
    let home = home_dir();
    let running = state.running_apps();
    let mut apps_list = apps::list(&home, &running);
    let known = state.apps.read();
    for entry in &mut apps_list {
        if let Some(previous) = known.iter().find(|k| k.path == entry.path) {
            entry.size = previous.size;
            entry.data_size = previous.data_size;
        }
    }
    drop(known);
    *state.apps.write() = apps_list.clone();
    if let Ok(guard) = state.jobs.start("apps") {
        let mut measuring = apps_list.clone();
        let app_handle = app.clone();
        spawn_job("apps", move || {
            let pool = match io_pool() {
                Ok(pool) => pool,
                Err(error) => return fail(&app_handle, "apps://done", &error),
            };
            pool.install(|| {
                apps::measure_all(&mut measuring, &home, &guard.token, |info| {
                    let _ = app_handle.emit(
                        "apps://size",
                        AppSizeEvent { path: info.path.clone(), size: info.size, data_size: info.data_size },
                    );
                });
            });
            *app_handle.state::<AppState>().apps.write() = measuring;
            drop(guard);
            let _ = app_handle.emit("apps://done", serde_json::json!({}));
        })?;
    }
    Ok(apps_list)
}

#[tauri::command(async)]
pub fn app_icon(app: AppHandle, state: State<'_, AppState>, path: String) -> Option<String> {
    if !path.ends_with(".app") {
        return None;
    }
    let requested = path.clone();
    apps::icon_data_url(&path, &state.cache_dir.join("icons"), move || {
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(apps::workspace_icon_tiff(&requested));
        })
        .ok()?;
        rx.recv_timeout(std::time::Duration::from_secs(3)).ok().flatten()
    })
}

#[tauri::command(async)]
pub fn app_associated(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<AssociatedItem>> {
    let info = state
        .apps
        .read()
        .iter()
        .find(|a| a.path == path)
        .cloned()
        .ok_or_else(|| NexusError::NotFound("That application is no longer listed.".into()))?;
    let pool = io_pool()?;
    Ok(pool.install(|| {
        apps::associated(info.bundle_id.as_deref(), &info.name, &home_dir(), &CancelToken::default())
    }))
}

pub fn restore_last_scan(state: &AppState) {
    let restored = state
        .db
        .latest_tree()
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<ScanResult>(&bytes).ok());
    if let Some(result) = restored {
        *state.storage.write() = Some(Arc::new(result));
    }
}

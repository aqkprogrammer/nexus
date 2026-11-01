pub mod advisor;
pub mod apps;
pub mod cleaner;
pub mod commands;
pub mod db;
#[cfg(debug_assertions)]
mod e2e;
pub mod duplicates;
pub mod error;
pub mod jobs;
pub mod plan;
pub mod reconcile;
pub mod report;
pub mod safety;
pub mod settings;
pub mod state;
pub mod storage;
pub mod system;
pub mod util;

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use tauri::Manager;

use commands::{cleanup, scans, system as sys};
use db::Db;
use jobs::Jobs;
use settings::Settings;
use state::AppState;
use system::telemetry::{self, Telemetry};

/// Hourly check for the optional scheduled scan. Scans are read-only.
fn spawn_scheduler(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3600));
        let state = app.state::<AppState>();
        let settings = state.settings.read().clone();
        let _ = state.db.prune(settings.retention_days);
        let every_ms: i64 = match settings.scan_frequency.as_str() {
            "daily" => 86_400_000,
            "weekly" => 7 * 86_400_000,
            _ => continue,
        };
        let last = state
            .db
            .list_scans(Some("storage"), 1)
            .ok()
            .and_then(|rows| rows.first().map(|r| r.finished_at))
            .unwrap_or(0);
        if util::now_ms() - last >= every_ms {
            let _ = scans::begin_storage_scan(&app, &state, false);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let db = Arc::new(Db::open(&data_dir.join("nexus.db"))?);
            let settings = Arc::new(RwLock::new(Settings::load(&db)?));
            let _ = db.prune(settings.read().retention_days);
            let telemetry = Telemetry::new();

            let state = AppState {
                db: db.clone(),
                settings: settings.clone(),
                telemetry: telemetry.clone(),
                jobs: Jobs::default(),
                storage: RwLock::new(None),
                cleaner: RwLock::new(None),
                duplicates: RwLock::new(None),
                apps: RwLock::new(Vec::new()),
                overview: RwLock::new(None),
                plans: Mutex::new(HashMap::new()),
                next_plan: AtomicU64::new(0),
                data_dir,
                cache_dir,
            };
            scans::restore_last_scan(&state);
            app.manage(state);
            telemetry::spawn(handle.clone(), telemetry, db, settings);
            spawn_scheduler(handle);
            #[cfg(debug_assertions)]
            e2e::start(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::approve_root,
            commands::revoke_root,
            commands::get_permissions,
            commands::open_privacy_settings,
            commands::cancel_job,
            commands::job_running,
            sys::get_overview,
            sys::get_telemetry,
            sys::get_history,
            sys::get_metric_trends,
            sys::list_processes,
            sys::terminate_process,
            sys::open_activity_monitor,
            sys::get_advisor_report,
            sys::list_scans,
            sys::list_cleanup_runs,
            sys::get_cleanup_items,
            sys::clear_history,
            sys::export_report,
            scans::start_storage_scan,
            scans::get_storage_summary,
            scans::get_storage_tree,
            scans::list_directory,
            scans::measure_directory,
            scans::search_files,
            scans::reveal_in_finder,
            scans::start_cleaner_scan,
            scans::get_cleaner_report,
            scans::start_duplicate_scan,
            scans::get_duplicate_report,
            scans::image_preview,
            scans::list_apps,
            scans::app_icon,
            scans::app_associated,
            cleanup::prepare_cleanup,
            cleanup::prepare_empty_trash,
            cleanup::prepare_paths_plan,
            cleanup::prepare_duplicate_plan,
            cleanup::prepare_uninstall,
            cleanup::discard_plan,
            cleanup::execute_plan,
            #[cfg(debug_assertions)]
            e2e::e2e_log,
            #[cfg(debug_assertions)]
            e2e::e2e_exit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NEXUS");
}

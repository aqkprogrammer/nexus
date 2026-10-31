use serde::Serialize;
use tauri::{AppHandle, State};

use crate::advisor::{self, AdvisorReport, Facts, LocalRules};
use crate::db::{CleanupItemRow, CleanupRunRow, MetricRow, ScanRow};
use crate::error::{NexusError, Result};
use crate::report::HealthExport;
use crate::state::AppState;
use crate::system::overview::{self, SystemOverview};
use crate::system::processes::{self, ProcessGroup, ProcessInfo};
use crate::system::telemetry::{HistoryPoint, TelemetrySample};
use crate::util::now_ms;

#[tauri::command(async)]
pub fn get_overview(state: State<'_, AppState>) -> SystemOverview {
    if let Some(cached) = state.overview.read().clone() {
        return cached;
    }
    let fresh = overview::collect();
    *state.overview.write() = Some(fresh.clone());
    fresh
}

#[tauri::command(async)]
pub fn get_telemetry(state: State<'_, AppState>) -> Option<TelemetrySample> {
    state.telemetry.latest()
}

#[tauri::command(async)]
pub fn get_history(state: State<'_, AppState>) -> Vec<HistoryPoint> {
    state.telemetry.history()
}

#[tauri::command(async)]
pub fn get_metric_trends(state: State<'_, AppState>, days: u32) -> Result<Vec<MetricRow>> {
    state.db.list_metrics(now_ms() - i64::from(days) * 86_400_000)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    pub processes: Vec<ProcessInfo>,
    pub groups: Vec<ProcessGroup>,
}

#[tauri::command(async)]
pub fn list_processes(state: State<'_, AppState>) -> ProcessSnapshot {
    let processes = state.telemetry.processes.list();
    let groups = processes::groups(&processes);
    ProcessSnapshot { processes, groups }
}

#[tauri::command(async)]
pub fn terminate_process(state: State<'_, AppState>, pid: u32, start_time: u64, force: bool) -> Result<()> {
    state.telemetry.processes.terminate(pid, start_time, force)
}

#[tauri::command(async)]
pub fn open_activity_monitor() -> Result<()> {
    processes::open_activity_monitor()
}

fn build_advisor(state: &AppState) -> AdvisorReport {
    let sample = state.telemetry.latest();
    let storage = state.storage.read().clone();
    let cleaner = state.cleaner.read().clone();
    let duplicates = state.duplicates.read().clone();
    let groups = processes::groups(&state.telemetry.processes.list());
    advisor::evaluate(
        &Facts {
            sample: sample.as_ref(),
            storage: storage.as_ref().map(|s| &s.summary),
            cleaner: cleaner.as_deref(),
            duplicates: duplicates.as_deref(),
            process_groups: &groups,
        },
        &LocalRules,
    )
}

#[tauri::command(async)]
pub fn get_advisor_report(state: State<'_, AppState>) -> AdvisorReport {
    build_advisor(&state)
}

#[tauri::command(async)]
pub fn list_scans(state: State<'_, AppState>, kind: Option<String>, limit: u32) -> Result<Vec<ScanRow>> {
    state.db.list_scans(kind.as_deref(), limit)
}

#[tauri::command(async)]
pub fn list_cleanup_runs(state: State<'_, AppState>, limit: u32) -> Result<Vec<CleanupRunRow>> {
    state.db.list_cleanup_runs(limit)
}

#[tauri::command(async)]
pub fn get_cleanup_items(state: State<'_, AppState>, run_id: i64) -> Result<Vec<CleanupItemRow>> {
    state.db.cleanup_items(run_id)
}

#[tauri::command(async)]
pub fn clear_history(state: State<'_, AppState>) -> Result<()> {
    state.db.clear_history()
}

/// Writes to a path the user just chose in the native save dialog.
#[tauri::command(async)]
pub fn export_report(_app: AppHandle, state: State<'_, AppState>, path: String) -> Result<()> {
    let overview = state.overview.read().clone();
    let export = HealthExport::new(
        overview,
        state.telemetry.latest(),
        build_advisor(&state),
        state.db.list_scans(None, 200)?,
        state.db.list_cleanup_runs(200)?,
    );
    let body = if path.ends_with(".json") {
        serde_json::to_string_pretty(&export)?
    } else if path.ends_with(".md") {
        export.to_markdown()
    } else {
        return Err(NexusError::Invalid("Reports are saved as .md or .json.".into()));
    };
    std::fs::write(path, body)?;
    Ok(())
}

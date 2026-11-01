use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use super::notify;
use crate::advisor::gb;
use crate::apps;
use crate::cleaner::catalog::Risk;
use crate::cleaner::{self, docker};
use crate::duplicates;
use crate::error::{NexusError, Result};
use crate::jobs::CancelToken;
use crate::plan::{self, Action, ExecResult, Plan, PlanInput, PlanMeta};
use crate::reconcile::{self, Removed};
use crate::safety::Policy;
use crate::state::AppState;
use crate::storage::scanner::{measure, ScanResult};
use crate::util::{allocated_size, home_dir};

fn store_plan(state: &AppState, meta: PlanMeta, inputs: Vec<PlanInput>) -> Plan {
    let plan = plan::build(meta, inputs, &state.safety_context());
    let mut plans = state.plans.lock();
    // Only the newest previews stay live; older ones would have to be regenerated anyway.
    if plans.len() >= 8 {
        plans.clear();
    }
    plans.insert(plan.id, plan.clone());
    plan
}

fn dry_run(state: &AppState, requested: bool) -> bool {
    requested || state.settings.read().dry_run
}

/// Turns selected cleaner item ids into a plan. Paths come from the held scan, not the UI.
#[tauri::command(async)]
pub fn prepare_cleanup(
    state: State<'_, AppState>,
    item_ids: Vec<String>,
    docker_ids: Vec<String>,
    dry: bool,
) -> Result<Plan> {
    let report = state
        .cleaner
        .read()
        .clone()
        .ok_or_else(|| NexusError::Invalid("Run a cleaner scan first.".into()))?;
    let wanted: HashSet<&str> = item_ids.iter().map(String::as_str).collect();
    let mode = state.settings.read().cleanup_mode.clone();
    let mut inputs = Vec::new();
    let mut warnings = Vec::new();

    for target in &report.targets {
        let chosen: Vec<_> = target.items.iter().filter(|i| wanted.contains(i.id.as_str())).collect();
        if chosen.is_empty() {
            continue;
        }
        if let Some(app) = &target.app_running {
            warnings.push(format!("{app} is running. Quit it first for the cleanest result."));
        }
        let action = plan::resolve_action(target.policy, target.risk, &mode);
        inputs.extend(chosen.into_iter().map(|item| PlanInput {
            path: item.path.clone(),
            size: item.size,
            action,
            policy: target.policy,
            risk: target.risk,
            group: target.title.clone(),
        }));
    }
    for id in &docker_ids {
        let resource = report
            .docker
            .resources
            .iter()
            .find(|r| r.id == *id)
            .ok_or_else(|| NexusError::Invalid(format!("Unknown Docker resource {id}.")))?;
        inputs.push(PlanInput {
            path: id.clone(),
            size: resource.reclaimable,
            action: Action::DockerPrune,
            policy: Policy::CleanupTarget,
            risk: if id == "volumes" { Risk::High } else { Risk::Low },
            group: format!("Docker: {}", resource.label),
        });
    }
    if inputs.is_empty() {
        return Err(NexusError::Invalid("Select at least one item.".into()));
    }
    let meta = PlanMeta {
        id: state.next_plan_id(),
        source: "cleaner".into(),
        title: "Smart Cleaner".into(),
        dry_run: dry_run(&state, dry),
        warnings,
    };
    Ok(store_plan(&state, meta, inputs))
}

#[tauri::command(async)]
pub fn prepare_empty_trash(state: State<'_, AppState>, dry: bool) -> Result<Plan> {
    let report = state.cleaner.read().clone();
    let trash = report
        .map(|r| r.trash.clone())
        .ok_or_else(|| NexusError::Invalid("Run a cleaner scan first.".into()))?;
    let inputs = if trash.accessible {
        trash
            .items
            .iter()
            .map(|item| PlanInput {
                path: item.path.clone(),
                size: item.size,
                action: Action::Delete,
                policy: Policy::CleanupTarget,
                risk: Risk::Medium,
                group: "Trash".into(),
            })
            .collect()
    } else {
        vec![PlanInput {
            path: "Trash".into(),
            size: 0,
            action: Action::FinderEmptyTrash,
            policy: Policy::CleanupTarget,
            risk: Risk::Medium,
            group: "Trash".into(),
        }]
    };
    let meta = PlanMeta {
        id: state.next_plan_id(),
        source: "trash".into(),
        title: "Empty Trash".into(),
        dry_run: dry_run(&state, dry),
        warnings: vec!["Emptying the Trash is permanent and cannot be undone.".into()],
    };
    Ok(store_plan(&state, meta, inputs))
}

fn path_size(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => measure(path, &CancelToken::default()).0 + allocated_size(&meta),
        Ok(meta) => allocated_size(&meta),
        Err(_) => 0,
    }
}

/// Items the user picked in an explorer or hogs view. These only ever go to the Trash.
#[tauri::command(async)]
pub fn prepare_paths_plan(
    state: State<'_, AppState>,
    source: String,
    title: String,
    paths: Vec<String>,
    dry: bool,
) -> Result<Plan> {
    if paths.is_empty() {
        return Err(NexusError::Invalid("Select at least one item.".into()));
    }
    let inputs = paths
        .into_iter()
        .map(|path| PlanInput {
            size: path_size(Path::new(&path)),
            path,
            action: Action::Trash,
            policy: Policy::UserSelected,
            risk: Risk::Medium,
            group: title.clone(),
        })
        .collect();
    let meta = PlanMeta {
        id: state.next_plan_id(),
        source,
        title,
        dry_run: dry_run(&state, dry),
        warnings: Vec::new(),
    };
    Ok(store_plan(&state, meta, inputs))
}

#[tauri::command(async)]
pub fn prepare_duplicate_plan(state: State<'_, AppState>, paths: Vec<String>, dry: bool) -> Result<Plan> {
    let report = state
        .duplicates
        .read()
        .clone()
        .ok_or_else(|| NexusError::Invalid("Run a duplicate scan first.".into()))?;
    let accepted = duplicates::check_selection(&report, &paths).map_err(NexusError::Safety)?;
    if accepted.is_empty() {
        return Err(NexusError::Invalid("Select at least one duplicate.".into()));
    }
    let inputs = accepted
        .into_iter()
        .map(|(path, size)| PlanInput {
            path,
            size,
            action: Action::Trash,
            policy: Policy::UserSelected,
            risk: Risk::Medium,
            group: "Duplicate files".into(),
        })
        .collect();
    let meta = PlanMeta {
        id: state.next_plan_id(),
        source: "duplicates".into(),
        title: "Duplicate Finder".into(),
        dry_run: dry_run(&state, dry),
        warnings: vec!["Files saved as APFS clones share storage, so actual savings can be lower.".into()],
    };
    Ok(store_plan(&state, meta, inputs))
}

/// Uninstall preview: the bundle plus only the associated items the user ticked.
#[tauri::command(async)]
pub fn prepare_uninstall(
    state: State<'_, AppState>,
    app_path: String,
    associated: Vec<String>,
    dry: bool,
) -> Result<Plan> {
    let info = state
        .apps
        .read()
        .iter()
        .find(|a| a.path == app_path)
        .cloned()
        .ok_or_else(|| NexusError::NotFound("That application is no longer listed.".into()))?;
    if info.is_system {
        return Err(NexusError::Safety("Apps that ship with macOS cannot be removed.".into()));
    }
    if info.running {
        return Err(NexusError::Invalid(format!("Quit {} before uninstalling it.", info.name)));
    }
    let known = apps::associated(info.bundle_id.as_deref(), &info.name, &home_dir(), &CancelToken::default());
    let mut inputs = vec![PlanInput {
        size: path_size(Path::new(&info.path)),
        path: info.path.clone(),
        action: Action::Trash,
        policy: Policy::Application,
        risk: Risk::Medium,
        group: "Application".into(),
    }];
    for path in associated {
        // Only items NEXUS itself associated with this app can be included.
        let item = known
            .iter()
            .find(|k| k.path == path)
            .ok_or_else(|| NexusError::Safety(format!("{path} is not associated with {}.", info.name)))?;
        inputs.push(PlanInput {
            path,
            size: item.size,
            action: Action::Trash,
            policy: Policy::Application,
            risk: Risk::Medium,
            group: item.label.clone(),
        });
    }
    let meta = PlanMeta {
        id: state.next_plan_id(),
        source: "uninstall".into(),
        title: format!("Uninstall {}", info.name),
        dry_run: dry_run(&state, dry),
        warnings: Vec::new(),
    };
    Ok(store_plan(&state, meta, inputs))
}

#[tauri::command(async)]
pub fn discard_plan(state: State<'_, AppState>, plan_id: u64) {
    state.plans.lock().remove(&plan_id);
}

/// Brings every held result in line with what a run removed, so screens stay right
/// without a rescan. The Trash is measured again because it just changed.
fn refresh_results(app: &AppHandle, state: &AppState, removed: &Removed, docker_changed: bool) {
    state.telemetry.request_volume_refresh();
    let home = home_dir();

    let held_scan = state.storage.read().clone();
    let summary = held_scan.filter(|_| !removed.is_empty()).map(|current| {
        let mut next = (*current).clone();
        reconcile::scan(&mut next, removed, &home);
        let persisted = ScanResult { summary: next.summary.clone(), tree: next.tree.pruned(0) };
        if let (Ok(rows), Ok(bytes)) = (state.db.list_scans(Some("storage"), 1), serde_json::to_vec(&persisted)) {
            if let Some(row) = rows.first() {
                let _ = state.db.save_tree(row.id, &bytes);
            }
        }
        let next = Arc::new(next);
        *state.storage.write() = Some(next.clone());
        next.summary.clone()
    });

    let held_cleaner = state.cleaner.read().clone();
    let cleaner = held_cleaner.map(|current| {
        let mut next = (*current).clone();
        reconcile::cleaner(&mut next, removed);
        next.trash = cleaner::scan_trash(&home, &CancelToken::default());
        if docker_changed {
            next.docker = docker::status();
        }
        let next = Arc::new(next);
        *state.cleaner.write() = Some(next.clone());
        next
    });

    let held_duplicates = state.duplicates.read().clone();
    let duplicates = held_duplicates.map(|current| {
        let mut next = (*current).clone();
        reconcile::duplicates(&mut next, removed);
        let next = Arc::new(next);
        *state.duplicates.write() = Some(next.clone());
        next
    });

    let _ = app.emit(
        "cleanup://reconciled",
        serde_json::json!({ "summary": summary, "cleaner": cleaner, "duplicates": duplicates }),
    );
}

/// `confirmed` must be sent explicitly; a plan never runs from a preview call alone.
#[tauri::command(async)]
pub fn execute_plan(app: AppHandle, state: State<'_, AppState>, plan_id: u64, confirmed: bool) -> Result<ExecResult> {
    if !confirmed {
        return Err(NexusError::Safety("Cleanup requires explicit confirmation.".into()));
    }
    let plan = state
        .plans
        .lock()
        .remove(&plan_id)
        .ok_or_else(|| NexusError::NotFound("This preview is no longer valid. Review it again.".into()))?;
    let guard = state.jobs.start("cleanup")?;
    let ctx = state.safety_context();
    let progress_app = app.clone();
    let execution = plan::execute(&plan, &ctx, &guard.token, move |progress| {
        let _ = progress_app.emit("cleanup://progress", progress);
        #[cfg(debug_assertions)]
        crate::e2e::slow_down();
    })?;
    let run_id = state.db.insert_cleanup_run(&execution.run, &execution.log)?;
    let removed = Removed::new(
        execution
            .log
            .iter()
            .filter(|item| matches!(item.outcome.as_str(), "trashed" | "deleted"))
            .map(|item| (item.path.clone(), item.size)),
    );
    let docker_changed = execution.log.iter().any(|item| item.outcome == "pruned");
    let run = execution.run;
    if !run.dry_run && run.items_ok > 0 {
        refresh_results(&app, &state, &removed, docker_changed);
        let title = if run.status == "cancelled" { "Cleanup stopped" } else { "Cleanup complete" };
        notify(
            &app,
            title,
            &format!("{} freed, {} moved to Trash.", gb(run.bytes_freed), gb(run.bytes_trashed)),
        );
    }
    Ok(ExecResult {
        run_id,
        dry_run: run.dry_run,
        status: run.status,
        bytes_freed: run.bytes_freed,
        bytes_trashed: run.bytes_trashed,
        items_ok: run.items_ok,
        items_failed: run.items_failed,
        items_skipped: run.items_skipped,
        failures: execution.failures,
    })
}

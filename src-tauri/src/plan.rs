//! Destructive work happens in two steps. The UI asks for a `Plan`, shows it, and confirms
//! it by id. The UI never supplies paths at execution time, so what runs is exactly what
//! was validated and shown.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cleaner::catalog::Risk;
use crate::cleaner::docker;
use crate::db::{CleanupItemRow, CleanupRunRow};
use crate::error::{NexusError, Result};
use crate::jobs::CancelToken;
use crate::safety::{validate, Policy, SafetyContext};
use crate::util::{now_ms, run_with_timeout};

const PLAN_TTL_MS: i64 = 15 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Move to the Trash. Reversible.
    Trash,
    /// Remove permanently.
    Delete,
    /// Run one Docker prune through Docker's own CLI.
    DockerPrune,
    /// Ask Finder to empty the Trash, for when NEXUS cannot read it directly.
    FinderEmptyTrash,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub path: String,
    pub size: u64,
    pub action: Action,
    pub policy: Policy,
    pub risk: Risk,
    pub group: String,
    /// Why this item will not be touched, when it failed validation.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: u64,
    pub source: String,
    pub title: String,
    pub dry_run: bool,
    pub created_at: i64,
    pub items: Vec<PlanItem>,
    pub trash_bytes: u64,
    pub delete_bytes: u64,
    pub blocked_count: u32,
    pub warnings: Vec<String>,
}

pub struct PlanInput {
    pub path: String,
    pub size: u64,
    pub action: Action,
    pub policy: Policy,
    pub risk: Risk,
    pub group: String,
}

pub struct PlanMeta {
    pub id: u64,
    pub source: String,
    pub title: String,
    pub dry_run: bool,
    pub warnings: Vec<String>,
}

/// Permanent deletion is only ever available for low-risk catalogued cleanup locations.
pub fn resolve_action(policy: Policy, risk: Risk, cleanup_mode: &str) -> Action {
    if policy == Policy::CleanupTarget && risk == Risk::Low && cleanup_mode == "delete" {
        Action::Delete
    } else {
        Action::Trash
    }
}

pub fn build(meta: PlanMeta, inputs: Vec<PlanInput>, ctx: &SafetyContext) -> Plan {
    let items: Vec<PlanItem> = inputs
        .into_iter()
        .map(|input| {
            let needs_path = matches!(input.action, Action::Trash | Action::Delete);
            let blocked = if needs_path {
                validate(Path::new(&input.path), input.policy, ctx).err().map(|e| e.to_string())
            } else {
                None
            };
            PlanItem {
                path: input.path,
                size: input.size,
                action: input.action,
                policy: input.policy,
                risk: input.risk,
                group: input.group,
                blocked,
            }
        })
        .collect();
    let sum = |action: fn(Action) -> bool| -> u64 {
        items
            .iter()
            .filter(|i| i.blocked.is_none() && action(i.action))
            .map(|i| i.size)
            .sum()
    };
    let trash_bytes = sum(|a| a == Action::Trash);
    let delete_bytes = sum(|a| a != Action::Trash);
    Plan {
        id: meta.id,
        source: meta.source,
        title: meta.title,
        dry_run: meta.dry_run,
        created_at: now_ms(),
        blocked_count: items.iter().filter(|i| i.blocked.is_some()).count() as u32,
        items,
        trash_bytes,
        delete_bytes,
        warnings: meta.warnings,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub run_id: i64,
    pub dry_run: bool,
    pub status: String,
    pub bytes_freed: u64,
    pub bytes_trashed: u64,
    pub items_ok: u64,
    pub items_failed: u64,
    pub items_skipped: u64,
    pub failures: Vec<ExecFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecFailure {
    pub path: String,
    pub reason: String,
}

fn trash_item(path: &Path) -> Result<()> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context
        .delete(path)
        .map_err(|e| NexusError::Unavailable(format!("Could not move to Trash: {e}")))
}

fn delete_item(path: &Path) -> Result<()> {
    // lstat so a symlink is unlinked rather than followed. remove_dir_all also never
    // follows links, and it does not cross into a link's target.
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn finder_empty_trash() -> Result<()> {
    let mut command = Command::new("/usr/bin/osascript");
    command.args(["-e", "tell application \"Finder\" to empty trash"]);
    match run_with_timeout(command, Duration::from_secs(120)) {
        Some(output) if output.status.success() => Ok(()),
        Some(output) => Err(NexusError::Permission(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        )),
        None => Err(NexusError::Unavailable("Finder did not respond.".into())),
    }
}

struct Outcome {
    label: &'static str,
    detail: Option<String>,
}

fn apply(item: &PlanItem, ctx: &SafetyContext, dry_run: bool) -> Result<Outcome> {
    let target: Option<PathBuf> = match item.action {
        // Revalidate at the last moment: the disk may have changed since the plan was shown.
        Action::Trash | Action::Delete => {
            Some(validate(Path::new(&item.path), item.policy, ctx)?)
        }
        Action::DockerPrune | Action::FinderEmptyTrash => None,
    };
    if dry_run {
        let label = match item.action {
            Action::Trash => "would-trash",
            Action::Delete | Action::FinderEmptyTrash => "would-delete",
            Action::DockerPrune => "would-prune",
        };
        return Ok(Outcome { label, detail: None });
    }
    match (item.action, target) {
        (Action::Trash, Some(path)) => {
            trash_item(&path)?;
            Ok(Outcome { label: "trashed", detail: None })
        }
        (Action::Delete, Some(path)) => {
            delete_item(&path)?;
            Ok(Outcome { label: "deleted", detail: None })
        }
        (Action::DockerPrune, _) => {
            docker::prune(&item.path)?;
            Ok(Outcome { label: "pruned", detail: Some("Docker reported success".into()) })
        }
        (Action::FinderEmptyTrash, _) => {
            finder_empty_trash()?;
            Ok(Outcome { label: "deleted", detail: Some("Emptied by Finder".into()) })
        }
        _ => Err(NexusError::Invalid("plan item has no target path".into())),
    }
}

pub struct Execution {
    pub run: CleanupRunRow,
    pub log: Vec<CleanupItemRow>,
    pub failures: Vec<ExecFailure>,
}

pub fn execute(
    plan: &Plan,
    ctx: &SafetyContext,
    token: &CancelToken,
    on_progress: impl Fn(ExecProgress),
) -> Result<Execution> {
    if now_ms() - plan.created_at > PLAN_TTL_MS {
        return Err(NexusError::Invalid(
            "This preview has expired. Review it again before continuing.".into(),
        ));
    }
    let started_at = now_ms();
    let total = plan.items.len();
    let mut run = CleanupRunRow {
        id: 0,
        source: plan.source.clone(),
        started_at,
        finished_at: 0,
        dry_run: plan.dry_run,
        mode: if plan.delete_bytes > 0 { "delete" } else { "trash" }.to_owned(),
        status: "completed".to_owned(),
        bytes_freed: 0,
        bytes_trashed: 0,
        items_ok: 0,
        items_failed: 0,
        items_skipped: 0,
    };
    let mut log = Vec::with_capacity(total);
    let mut failures = Vec::new();

    for (index, item) in plan.items.iter().enumerate() {
        on_progress(ExecProgress {
            done: index,
            total,
            current: item.path.clone(),
            bytes: run.bytes_freed + run.bytes_trashed,
        });
        let action = serde_json::to_value(item.action)?.as_str().unwrap_or("").to_owned();
        let mut record = |outcome: &str, detail: Option<String>| {
            log.push(CleanupItemRow {
                path: item.path.clone(),
                size: item.size,
                action: action.clone(),
                outcome: outcome.to_owned(),
                detail,
            });
        };
        if token.is_cancelled() {
            run.status = "cancelled".to_owned();
            run.items_skipped += 1;
            record("skipped", Some("Cancelled".into()));
            continue;
        }
        if let Some(reason) = &item.blocked {
            run.items_skipped += 1;
            record("blocked", Some(reason.clone()));
            continue;
        }
        match apply(item, ctx, plan.dry_run) {
            Ok(outcome) => {
                run.items_ok += 1;
                if item.action == Action::Trash {
                    run.bytes_trashed += item.size;
                } else {
                    run.bytes_freed += item.size;
                }
                record(outcome.label, outcome.detail);
            }
            Err(NexusError::NotFound(reason)) => {
                run.items_skipped += 1;
                record("gone", Some(reason));
            }
            Err(error) => {
                run.items_failed += 1;
                let reason = error.to_string();
                failures.push(ExecFailure { path: item.path.clone(), reason: reason.clone() });
                record("failed", Some(reason));
            }
        }
    }
    on_progress(ExecProgress {
        done: total,
        total,
        current: String::new(),
        bytes: run.bytes_freed + run.bytes_trashed,
    });
    run.finished_at = now_ms();
    if run.status == "completed" && run.items_failed > 0 {
        run.status = "partial".to_owned();
    }
    Ok(Execution { run, log, failures })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::{tempdir, TempDir};

    struct Fixture {
        _dir: TempDir,
        home: PathBuf,
        ctx: SafetyContext,
    }

    fn fixture() -> Fixture {
        let dir = tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap().join("home");
        fs::create_dir_all(home.join("Library/Caches")).unwrap();
        fs::create_dir_all(home.join("Documents")).unwrap();
        let ctx = SafetyContext {
            home: home.clone(),
            approved_roots: vec![home.join("Documents")],
            cleanup_roots: vec![home.join("Library/Caches")],
            cleanup_items: Vec::new(),
            nexus_data: None,
        };
        Fixture { _dir: dir, home, ctx }
    }

    fn meta(dry_run: bool) -> PlanMeta {
        PlanMeta {
            id: 1,
            source: "test".into(),
            title: "Test".into(),
            dry_run,
            warnings: Vec::new(),
        }
    }

    fn delete_input(path: &Path, size: u64) -> PlanInput {
        PlanInput {
            path: path.to_string_lossy().into_owned(),
            size,
            action: Action::Delete,
            policy: Policy::CleanupTarget,
            risk: Risk::Low,
            group: "Caches".into(),
        }
    }

    fn cache_file(f: &Fixture, name: &str) -> PathBuf {
        let path = f.home.join("Library/Caches").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "data").unwrap();
        path
    }

    fn run(plan: &Plan, ctx: &SafetyContext) -> Execution {
        execute(plan, ctx, &CancelToken::default(), |_| {}).unwrap()
    }

    /// Puts one tiny file in the real Trash, so it stays out of the normal test run.
    #[test]
    #[ignore = "writes to the user's Trash"]
    fn trash_action_moves_the_item_out_of_place() {
        let dir = tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("nexus-trash-selftest.txt");
        fs::write(&path, "NEXUS self-test. Safe to delete.").unwrap();
        trash_item(&path).unwrap();
        assert!(fs::symlink_metadata(&path).is_err());
    }

    #[test]
    fn dry_run_changes_nothing_but_still_logs_every_item() {
        let f = fixture();
        let path = cache_file(&f, "a.bin");
        let plan = build(meta(true), vec![delete_input(&path, 4)], &f.ctx);
        let exec = run(&plan, &f.ctx);
        assert!(path.exists());
        assert_eq!(exec.run.items_ok, 1);
        assert!(exec.run.dry_run);
        assert_eq!(exec.log[0].outcome, "would-delete");
    }

    #[test]
    fn execution_removes_files_and_directories() {
        let f = fixture();
        let file = cache_file(&f, "a.bin");
        let dir_file = cache_file(&f, "folder/nested/b.bin");
        let folder = f.home.join("Library/Caches/folder");
        let plan = build(meta(false), vec![delete_input(&file, 4), delete_input(&folder, 4)], &f.ctx);
        let exec = run(&plan, &f.ctx);
        assert!(!file.exists() && !dir_file.exists() && !folder.exists());
        assert_eq!(exec.run.bytes_freed, 8);
        assert_eq!(exec.run.status, "completed");
    }

    #[test]
    fn blocked_items_are_recorded_and_never_touched() {
        let f = fixture();
        let doc = f.home.join("Documents/keep.txt");
        fs::write(&doc, "precious").unwrap();
        let plan = build(meta(false), vec![delete_input(&doc, 8)], &f.ctx);
        assert_eq!(plan.blocked_count, 1);
        assert_eq!(plan.delete_bytes, 0);
        let exec = run(&plan, &f.ctx);
        assert!(doc.exists());
        assert_eq!(exec.run.items_skipped, 1);
        assert_eq!(exec.log[0].outcome, "blocked");
    }

    #[test]
    fn a_path_swapped_for_a_symlink_after_planning_only_loses_the_link() {
        let f = fixture();
        let path = cache_file(&f, "swap");
        let doc = f.home.join("Documents/keep.txt");
        fs::write(&doc, "precious").unwrap();
        let plan = build(meta(false), vec![delete_input(&path, 4)], &f.ctx);
        fs::remove_file(&path).unwrap();
        symlink(&doc, &path).unwrap();
        run(&plan, &f.ctx);
        assert!(doc.exists());
        assert!(fs::symlink_metadata(&path).is_err());
    }

    #[test]
    fn a_parent_swapped_for_a_symlink_after_planning_is_refused() {
        let f = fixture();
        let inner = cache_file(&f, "dir/file.bin");
        let plan = build(meta(false), vec![delete_input(&inner, 4)], &f.ctx);
        let doc = f.home.join("Documents/file.bin");
        fs::write(&doc, "precious").unwrap();
        fs::remove_dir_all(f.home.join("Library/Caches/dir")).unwrap();
        symlink(f.home.join("Documents"), f.home.join("Library/Caches/dir")).unwrap();
        let exec = run(&plan, &f.ctx);
        assert!(doc.exists());
        assert_eq!(exec.run.items_failed, 1);
    }

    #[test]
    fn items_that_vanished_are_skipped_not_failed() {
        let f = fixture();
        let path = cache_file(&f, "gone.bin");
        let plan = build(meta(false), vec![delete_input(&path, 4)], &f.ctx);
        fs::remove_file(&path).unwrap();
        let exec = run(&plan, &f.ctx);
        assert_eq!(exec.run.items_failed, 0);
        assert_eq!(exec.run.items_skipped, 1);
        assert_eq!(exec.log[0].outcome, "gone");
    }

    #[test]
    fn cancellation_stops_before_the_next_item() {
        let f = fixture();
        let path = cache_file(&f, "a.bin");
        let plan = build(meta(false), vec![delete_input(&path, 4)], &f.ctx);
        let token = CancelToken::default();
        token.cancel();
        let exec = execute(&plan, &f.ctx, &token, |_| {}).unwrap();
        assert!(path.exists());
        assert_eq!(exec.run.status, "cancelled");
    }

    #[test]
    fn expired_plans_are_refused() {
        let f = fixture();
        let path = cache_file(&f, "a.bin");
        let mut plan = build(meta(false), vec![delete_input(&path, 4)], &f.ctx);
        plan.created_at -= PLAN_TTL_MS + 1;
        assert!(execute(&plan, &f.ctx, &CancelToken::default(), |_| {}).is_err());
        assert!(path.exists());
    }

    #[test]
    fn permanent_delete_needs_low_risk_cleanup_and_the_delete_setting() {
        assert_eq!(resolve_action(Policy::CleanupTarget, Risk::Low, "delete"), Action::Delete);
        assert_eq!(resolve_action(Policy::CleanupTarget, Risk::Low, "trash"), Action::Trash);
        assert_eq!(resolve_action(Policy::CleanupTarget, Risk::Medium, "delete"), Action::Trash);
        assert_eq!(resolve_action(Policy::Leftover, Risk::Low, "delete"), Action::Trash);
        assert_eq!(resolve_action(Policy::UserSelected, Risk::Low, "delete"), Action::Trash);
        assert_eq!(resolve_action(Policy::Application, Risk::Low, "delete"), Action::Trash);
    }

    #[test]
    fn totals_exclude_blocked_items() {
        let f = fixture();
        let ok = cache_file(&f, "ok.bin");
        let doc = f.home.join("Documents/no.txt");
        fs::write(&doc, "x").unwrap();
        let plan = build(meta(true), vec![delete_input(&ok, 10), delete_input(&doc, 100)], &f.ctx);
        assert_eq!(plan.delete_bytes, 10);
        assert_eq!(plan.blocked_count, 1);
    }
}

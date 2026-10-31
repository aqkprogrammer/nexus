use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use crate::apps::AppInfo;
use crate::cleaner::{catalog, CleanerReport};
use crate::db::Db;
use crate::duplicates::DuplicateReport;
use crate::error::{NexusError, Result};
use crate::jobs::Jobs;
use crate::plan::Plan;
use crate::safety::SafetyContext;
use crate::settings::Settings;
use crate::storage::scanner::ScanResult;
use crate::system::overview::SystemOverview;
use crate::system::telemetry::Telemetry;
use crate::util::home_dir;

pub struct AppState {
    pub db: Arc<Db>,
    pub settings: Arc<RwLock<Settings>>,
    pub telemetry: Arc<Telemetry>,
    pub jobs: Jobs,
    pub storage: RwLock<Option<Arc<ScanResult>>>,
    pub cleaner: RwLock<Option<Arc<CleanerReport>>>,
    pub duplicates: RwLock<Option<Arc<DuplicateReport>>>,
    pub apps: RwLock<Vec<AppInfo>>,
    pub overview: RwLock<Option<SystemOverview>>,
    pub plans: Mutex<HashMap<u64, Plan>>,
    pub next_plan: AtomicU64,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl AppState {
    pub fn next_plan_id(&self) -> u64 {
        self.next_plan.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn catalog(&self) -> Vec<catalog::TargetDef> {
        let min_age = self.settings.read().temp_min_age_days;
        catalog::targets(&home_dir(), &std::env::temp_dir(), min_age)
    }

    pub fn safety_context(&self) -> SafetyContext {
        let home = home_dir();
        let (cleanup_roots, cleanup_items) = catalog::cleanup_bounds(&self.catalog(), &home);
        SafetyContext {
            home: home.canonicalize().unwrap_or(home),
            approved_roots: self.approved_roots(),
            cleanup_roots,
            cleanup_items,
            nexus_data: Some(self.data_dir.clone()),
        }
    }

    pub fn approved_roots(&self) -> Vec<PathBuf> {
        self.settings.read().approved_roots.iter().map(PathBuf::from).collect()
    }

    /// Read access is limited to folders the user approved, same as deletion.
    pub fn ensure_approved(&self, path: &Path) -> Result<PathBuf> {
        let resolved = path.canonicalize()?;
        if self
            .approved_roots()
            .iter()
            .any(|r| r.canonicalize().is_ok_and(|root| resolved.starts_with(root)))
        {
            Ok(resolved)
        } else {
            Err(NexusError::Permission(format!(
                "{} is outside the folders you approved. Add it to continue.",
                path.display()
            )))
        }
    }

    pub fn running_apps(&self) -> HashSet<String> {
        self.telemetry.processes.list().into_iter().filter_map(|p| p.app).collect()
    }
}

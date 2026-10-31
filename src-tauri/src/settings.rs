use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{NexusError, Result};
use crate::util::{home_dir, path_string};

const KEY: &str = "app";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub onboarded: bool,
    pub theme: String,
    pub accent: String,
    pub animation: String,
    /// Folders the user has explicitly allowed NEXUS to read.
    pub approved_roots: Vec<String>,
    pub exclusions: Vec<String>,
    pub scan_frequency: String,
    pub notifications: bool,
    pub cleanup_mode: String,
    pub dry_run: bool,
    pub temp_min_age_days: u32,
    pub old_file_months: u32,
    pub record_history: bool,
    pub retention_days: u32,
    pub launch_at_login: bool,
    pub ai_provider: String,
    pub ai_model: String,
    pub ai_endpoint: String,
    pub ai_share_aggregates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            theme: "nexus".into(),
            accent: "cyan".into(),
            animation: "full".into(),
            approved_roots: Vec::new(),
            exclusions: vec![
                path_string(&home_dir().join("Library/Mobile Documents")),
                path_string(&home_dir().join("Library/CloudStorage")),
            ],
            scan_frequency: "manual".into(),
            notifications: true,
            cleanup_mode: "trash".into(),
            dry_run: false,
            temp_min_age_days: 3,
            old_file_months: 12,
            record_history: true,
            retention_days: 180,
            launch_at_login: false,
            ai_provider: "none".into(),
            ai_model: String::new(),
            ai_endpoint: String::new(),
            ai_share_aggregates: false,
        }
    }
}

impl Settings {
    pub fn load(db: &Db) -> Result<Self> {
        Ok(match db.get_setting(KEY)? {
            Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            None => Self::default(),
        })
    }

    pub fn save(&self, db: &Db) -> Result<()> {
        self.validate()?;
        db.set_setting(KEY, &serde_json::to_string(self)?)
    }

    fn validate(&self) -> Result<()> {
        if !matches!(self.cleanup_mode.as_str(), "trash" | "delete") {
            return Err(NexusError::Invalid("cleanup mode must be trash or delete".into()));
        }
        if !matches!(self.scan_frequency.as_str(), "manual" | "daily" | "weekly") {
            return Err(NexusError::Invalid("unknown scan frequency".into()));
        }
        if !(1..=3650).contains(&self.retention_days) {
            return Err(NexusError::Invalid("retention must be between 1 and 3650 days".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative() {
        let s = Settings::default();
        assert_eq!(s.cleanup_mode, "trash");
        assert_eq!(s.ai_provider, "none");
        assert!(!s.ai_share_aggregates);
        assert!(s.approved_roots.is_empty());
    }

    #[test]
    fn unknown_fields_and_partial_json_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"theme":"arctic","legacy":1}"#).unwrap();
        assert_eq!(s.theme, "arctic");
        assert_eq!(s.cleanup_mode, "trash");
    }

    #[test]
    fn rejects_invalid_cleanup_mode() {
        let db = Db::in_memory().unwrap();
        let s = Settings { cleanup_mode: "shred".into(), ..Settings::default() };
        assert!(s.save(&db).is_err());
    }
}

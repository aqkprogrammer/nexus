//! IPC surface. Short reads use `#[tauri::command(async)]` so they stay off the main thread;
//! long jobs return immediately and report through events.

pub mod cleanup;
pub mod scans;
pub mod system;

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;

use crate::error::{NexusError, Result};
use crate::settings::Settings;
use crate::state::AppState;
use crate::util::{home_dir, path_string};

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let enabled = app.state::<AppState>().settings.read().notifications;
    if enabled {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.read().clone()
}

#[tauri::command(async)]
pub fn save_settings(state: State<'_, AppState>, mut settings: Settings) -> Result<Settings> {
    // Approval changes only through approve_root and revoke_root.
    settings.approved_roots = state.settings.read().approved_roots.clone();
    settings.save(&state.db)?;
    *state.settings.write() = settings.clone();
    Ok(settings)
}

#[tauri::command(async)]
pub fn approve_root(state: State<'_, AppState>, path: String) -> Result<Settings> {
    let resolved = PathBuf::from(&path).canonicalize()?;
    if !resolved.is_dir() {
        return Err(NexusError::Invalid(format!("{path} is not a folder.")));
    }
    if resolved == Path::new("/") {
        return Err(NexusError::Safety(
            "Approve specific folders instead of the whole disk.".into(),
        ));
    }
    let mut settings = state.settings.read().clone();
    let text = path_string(&resolved);
    if !settings.approved_roots.contains(&text) {
        settings.approved_roots.push(text);
    }
    settings.save(&state.db)?;
    *state.settings.write() = settings.clone();
    Ok(settings)
}

#[tauri::command(async)]
pub fn revoke_root(state: State<'_, AppState>, path: String) -> Result<Settings> {
    let mut settings = state.settings.read().clone();
    settings.approved_roots.retain(|r| *r != path);
    settings.save(&state.db)?;
    *state.settings.write() = settings.clone();
    Ok(settings)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub full_disk_access: String,
    pub home: String,
    pub suggested_roots: Vec<String>,
}

/// Probes a location macOS protects with Full Disk Access. Reading it never shows a prompt.
pub fn full_disk_access_state(home: &Path) -> &'static str {
    for probe in ["Library/Safari", "Library/Mail", ".Trash"] {
        match fs::read_dir(home.join(probe)) {
            Ok(_) => return "granted",
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return "denied",
            Err(_) => {}
        }
    }
    "unknown"
}

#[tauri::command(async)]
pub fn get_permissions() -> Permissions {
    let home = home_dir();
    Permissions {
        full_disk_access: full_disk_access_state(&home).to_owned(),
        suggested_roots: vec![path_string(&home), "/Applications".to_owned()],
        home: path_string(&home),
    }
}

#[tauri::command(async)]
pub fn open_privacy_settings() -> Result<()> {
    std::process::Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn()?;
    Ok(())
}

#[tauri::command(async)]
pub fn cancel_job(state: State<'_, AppState>, kind: String) -> bool {
    state.jobs.cancel(&kind)
}

#[tauri::command(async)]
pub fn job_running(state: State<'_, AppState>, kind: String) -> bool {
    state.jobs.is_running(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn an_accessible_probe_means_granted() {
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join("Library/Safari")).unwrap();
        assert_eq!(full_disk_access_state(home.path()), "granted");
    }

    #[test]
    fn no_probe_locations_means_unknown_not_denied() {
        let home = tempdir().unwrap();
        assert_eq!(full_disk_access_state(home.path()), "unknown");
    }
}

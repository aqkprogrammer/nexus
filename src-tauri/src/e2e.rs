//! Debug-only end-to-end driver. Absent from release builds. When NEXUS_E2E_SCRIPT names a
//! file, it is evaluated in the real webview so a test can exercise the UI and the real
//! Rust commands together. See `e2e/run.sh`, which always uses a throwaway HOME.

use std::time::Duration;

use tauri::{App, AppHandle, Manager};

pub fn start(app: &App) {
    let Ok(path) = std::env::var("NEXUS_E2E_SCRIPT") else {
        return;
    };
    let script = std::fs::read_to_string(path).expect("e2e script must be readable");
    let window = app.get_webview_window("main").expect("main window exists");
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        window.eval(&script).expect("e2e script must evaluate");
    });
}

/// Gives a test time to press Stop during a cleanup that would otherwise finish instantly.
pub fn slow_down() {
    let delay = std::env::var("NEXUS_E2E_SLOW_MS").ok().and_then(|v| v.parse().ok());
    if let Some(ms) = delay {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

#[tauri::command]
pub fn e2e_log(line: String) {
    println!("{line}");
}

#[tauri::command]
pub fn e2e_exit(app: AppHandle, code: i32) {
    app.exit(code);
}

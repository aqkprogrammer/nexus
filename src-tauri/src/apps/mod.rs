pub mod leftovers;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use base64::Engine;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSString;
use rayon::prelude::*;
use serde::Serialize;

use crate::jobs::CancelToken;
use crate::storage::scanner::measure;
use crate::util::{path_string, run_with_timeout};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub path: String,
    pub bundle_id: Option<String>,
    pub version: Option<String>,
    pub size: Option<u64>,
    pub data_size: Option<u64>,
    pub modified: Option<i64>,
    pub is_system: bool,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociatedItem {
    pub path: String,
    pub label: String,
    pub size: u64,
    pub files: u64,
    pub is_dir: bool,
}

const ASSOCIATED_LOCATIONS: &[(&str, &str)] = &[
    ("Library/Application Support", "Application data"),
    ("Library/Caches", "Cache"),
    ("Library/Containers", "Sandbox container"),
    ("Library/Group Containers", "Shared container"),
    ("Library/HTTPStorages", "Web storage"),
    ("Library/Logs", "Logs"),
    ("Library/Preferences", "Preferences"),
    ("Library/Saved Application State", "Saved window state"),
    ("Library/WebKit", "Web cache"),
];

fn bundle_info(path: &Path) -> Option<AppInfo> {
    let meta = fs::metadata(path).ok()?;
    let plist = plist::Value::from_file(path.join("Contents/Info.plist")).ok();
    let dict = plist.as_ref().and_then(|p| p.as_dictionary());
    let text = |key: &str| {
        dict.and_then(|d| d.get(key)).and_then(|v| v.as_string()).map(str::to_owned)
    };
    let file_name = path.file_stem()?.to_string_lossy().into_owned();
    Some(AppInfo {
        name: text("CFBundleDisplayName").or_else(|| text("CFBundleName")).unwrap_or(file_name),
        path: path_string(path),
        bundle_id: text("CFBundleIdentifier"),
        version: text("CFBundleShortVersionString").or_else(|| text("CFBundleVersion")),
        size: None,
        data_size: None,
        modified: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
        is_system: path.starts_with("/System"),
        running: false,
    })
}

fn collect_bundles(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        if path.extension().is_some_and(|e| e == "app") {
            out.push(path);
        } else if depth > 0 {
            collect_bundles(&path, depth - 1, out);
        }
    }
}

/// Every application bundle in the standard locations, including vendor subfolders.
pub fn list(home: &Path, running: &HashSet<String>) -> Vec<AppInfo> {
    let mut bundles = Vec::new();
    collect_bundles(Path::new("/Applications"), 2, &mut bundles);
    collect_bundles(&home.join("Applications"), 2, &mut bundles);
    collect_bundles(Path::new("/System/Applications"), 1, &mut bundles);
    let mut apps: Vec<AppInfo> = bundles
        .par_iter()
        .filter_map(|p| bundle_info(p))
        .map(|mut app| {
            app.running = running.contains(&app.name)
                || Path::new(&app.path).file_stem().is_some_and(|s| running.contains(&*s.to_string_lossy()));
            app
        })
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Fills in bundle and data sizes, reporting each app as it completes.
pub fn measure_all(apps: &mut [AppInfo], home: &Path, token: &CancelToken, on_each: impl Fn(&AppInfo) + Sync) {
    apps.par_iter_mut().for_each(|app| {
        if token.is_cancelled() {
            return;
        }
        app.size = Some(measure(Path::new(&app.path), token).0);
        let data = associated(app.bundle_id.as_deref(), &app.name, home, token);
        app.data_size = Some(data.iter().map(|d| d.size).sum());
        on_each(app);
    });
}

fn names_match(entry: &str, bundle_id: Option<&str>, app_name: &str) -> bool {
    let stem = entry
        .strip_suffix(".plist")
        .or_else(|| entry.strip_suffix(".savedState"))
        .or_else(|| entry.strip_suffix(".binarycookies"))
        .unwrap_or(entry);
    if let Some(id) = bundle_id {
        // Match the exact id or a dotted child of it, not a shared vendor prefix.
        if stem == id || stem.starts_with(&format!("{id}.")) {
            return true;
        }
        if let Some(group) = stem.split_once('.').map(|(_, rest)| rest) {
            if group == id {
                return true;
            }
        }
    }
    entry == app_name
}

/// Support files that belong to an app, matched by bundle id or exact app name.
pub fn associated(bundle_id: Option<&str>, app_name: &str, home: &Path, token: &CancelToken) -> Vec<AssociatedItem> {
    let mut items = Vec::new();
    for (location, label) in ASSOCIATED_LOCATIONS {
        let dir = home.join(location);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let by_name_allowed = *location == "Library/Application Support" || *location == "Library/Caches";
            let matched = if by_name_allowed {
                names_match(&name, bundle_id, app_name)
            } else {
                names_match(&name, bundle_id, "\0")
            };
            if !matched {
                continue;
            }
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            let (size, files) = if meta.is_dir() {
                measure(&path, token)
            } else {
                (crate::util::allocated_size(&meta), 1)
            };
            items.push(AssociatedItem {
                path: path_string(&path),
                label: (*label).to_owned(),
                size,
                files,
                is_dir: meta.is_dir(),
            });
        }
    }
    items.sort_by_key(|x| std::cmp::Reverse(x.size));
    items
}

fn bundle_icon_file(path: &Path) -> Option<PathBuf> {
    let plist = plist::Value::from_file(path.join("Contents/Info.plist")).ok()?;
    let name = plist.as_dictionary()?.get("CFBundleIconFile")?.as_string()?;
    let file = if name.ends_with(".icns") { name.to_owned() } else { format!("{name}.icns") };
    let icon = path.join("Contents/Resources").join(file);
    icon.exists().then_some(icon)
}

/// Renders an app's icon through NSWorkspace. AppKit resolves icons lazily and returns a
/// placeholder off the main thread, so this must run there.
pub fn workspace_icon_tiff(path: &str) -> Option<Vec<u8>> {
    let workspace = NSWorkspace::sharedWorkspace();
    let image = workspace.iconForFile(&NSString::from_str(path));
    Some(image.TIFFRepresentation()?.to_vec())
}

fn to_png(source: &Path, out: &Path) -> bool {
    let mut command = Command::new("/usr/bin/sips");
    command.args(["-s", "format", "png", "-Z", "96"]).arg(source).arg("--out").arg(out);
    run_with_timeout(command, Duration::from_secs(10)).is_some_and(|o| o.status.success())
}

/// App icon as a small PNG data URL. Most bundles ship an .icns file; apps that keep their
/// icon only in an asset catalog, including macOS's own, go through `fallback`.
pub fn icon_data_url(path: &str, cache_dir: &Path, fallback: impl FnOnce() -> Option<Vec<u8>>) -> Option<String> {
    fs::create_dir_all(cache_dir).ok()?;
    let key = blake3::hash(path.as_bytes()).to_hex();
    let png = cache_dir.join(format!("v2-{}.png", &key[..24]));
    if !png.exists() {
        let from_bundle = bundle_icon_file(Path::new(path)).is_some_and(|icns| to_png(&icns, &png));
        if !from_bundle {
            let tiff = cache_dir.join(format!("v2-{}.tiff", &key[..24]));
            fs::write(&tiff, fallback()?).ok()?;
            let converted = to_png(&tiff, &png);
            let _ = fs::remove_file(&tiff);
            if !converted {
                return None;
            }
        }
    }
    let bytes = fs::read(&png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Image preview as a JPEG data URL, generated by sips into a throwaway file.
pub fn thumbnail_data_url(path: &Path, cache_dir: &Path) -> Option<String> {
    fs::create_dir_all(cache_dir).ok()?;
    let key = blake3::hash(path.as_os_str().as_encoded_bytes()).to_hex();
    let out = cache_dir.join(format!("{}.jpg", &key[..24]));
    let mut command = Command::new("/usr/bin/sips");
    command.args(["-s", "format", "jpeg", "-Z", "320"]).arg(path).arg("--out").arg(&out);
    let output = run_with_timeout(command, Duration::from_secs(15));
    let bytes = output.filter(|o| o.status.success()).and_then(|_| fs::read(&out).ok());
    let _ = fs::remove_file(&out);
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes?)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn matches_exact_ids_and_children_but_not_vendor_siblings() {
        let id = Some("com.vendor.editor");
        assert!(names_match("com.vendor.editor", id, "Editor"));
        assert!(names_match("com.vendor.editor.plist", id, "Editor"));
        assert!(names_match("com.vendor.editor.helper", id, "Editor"));
        assert!(names_match("TEAMID.com.vendor.editor", id, "Editor"));
        assert!(!names_match("com.vendor.other", id, "Editor"));
        assert!(!names_match("com.vendor", id, "Editor"));
    }

    #[test]
    fn associated_finds_support_files_with_sizes() {
        let home = tempdir().unwrap();
        let support = home.path().join("Library/Application Support/Editor");
        fs::create_dir_all(&support).unwrap();
        fs::write(support.join("data.bin"), vec![0u8; 8192]).unwrap();
        let prefs = home.path().join("Library/Preferences");
        fs::create_dir_all(&prefs).unwrap();
        fs::write(prefs.join("com.vendor.editor.plist"), "x").unwrap();
        fs::write(prefs.join("com.other.app.plist"), "x").unwrap();
        let items = associated(Some("com.vendor.editor"), "Editor", home.path(), &CancelToken::default());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "Application data");
        assert!(items[0].size >= 8192);
    }

    #[test]
    fn an_app_name_alone_does_not_match_inside_preferences() {
        let home = tempdir().unwrap();
        let prefs = home.path().join("Library/Preferences");
        fs::create_dir_all(&prefs).unwrap();
        fs::write(prefs.join("Editor"), "x").unwrap();
        assert!(associated(None, "Editor", home.path(), &CancelToken::default()).is_empty());
    }

    #[test]
    fn reads_bundle_metadata_from_info_plist() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("Tool.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        let mut info = plist::Dictionary::new();
        info.insert("CFBundleName".into(), "Tool".into());
        info.insert("CFBundleIdentifier".into(), "com.vendor.tool".into());
        info.insert("CFBundleShortVersionString".into(), "2.1".into());
        plist::Value::Dictionary(info).to_file_xml(app.join("Contents/Info.plist")).unwrap();
        let parsed = bundle_info(&app).unwrap();
        assert_eq!(parsed.bundle_id.as_deref(), Some("com.vendor.tool"));
        assert_eq!(parsed.version.as_deref(), Some("2.1"));
    }

    #[test]
    fn finds_the_icon_named_in_info_plist_with_or_without_extension() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("Tool.app");
        fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        fs::write(app.join("Contents/Resources/Tool.icns"), "x").unwrap();
        let mut info = plist::Dictionary::new();
        info.insert("CFBundleIconFile".into(), "Tool".into());
        plist::Value::Dictionary(info).to_file_xml(app.join("Contents/Info.plist")).unwrap();
        assert!(bundle_icon_file(&app).unwrap().ends_with("Tool.icns"));
    }

    #[test]
    fn broken_bundles_still_list_by_file_name() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("Broken.app");
        fs::create_dir_all(&app).unwrap();
        let parsed = bundle_info(&app).unwrap();
        assert_eq!(parsed.name, "Broken");
        assert!(parsed.bundle_id.is_none());
    }
}

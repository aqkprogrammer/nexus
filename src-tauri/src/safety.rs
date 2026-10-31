//! The last line of defence before anything leaves the disk. Every destructive path is
//! checked here when a plan is built and again immediately before it is touched.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NexusError, Result};

/// How a path came to be a deletion candidate, which decides what it must satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Policy {
    /// Found by the cleaner inside a catalogued cleanup location.
    CleanupTarget,
    /// Reverse-DNS leftover of an uninstalled app, directly inside a Library folder.
    Leftover,
    /// Picked by the user in an explorer, duplicates or hogs view.
    UserSelected,
    /// An application bundle or its support data, in the uninstall flow.
    Application,
}

pub struct SafetyContext {
    pub home: PathBuf,
    pub approved_roots: Vec<PathBuf>,
    pub cleanup_roots: Vec<PathBuf>,
    /// Exact items allowed even though their parent is not a cleanup root.
    pub cleanup_items: Vec<PathBuf>,
    pub nexus_data: Option<PathBuf>,
}

const SYSTEM_PREFIXES: &[&str] = &[
    "/System", "/usr", "/bin", "/sbin", "/etc", "/var", "/dev", "/opt", "/cores", "/Library",
    "/Network", "/.vol",
];

/// Inside /private only the per-user temp areas are ever reachable.
const PRIVATE_ALLOWED: &[&str] = &["/private/tmp", "/private/var/folders"];

/// Standard home folders. Their contents may be cleaned but the folders themselves never.
const HOME_ANCHORS: &[&str] = &[
    "Desktop", "Documents", "Downloads", "Library", "Movies", "Music", "Pictures", "Public",
    "Applications", ".Trash", ".ssh", ".config", ".local",
];

/// Credentials, personal databases and browser identity: never candidates, under any policy.
const HOME_PROTECTED: &[&str] = &[
    ".ssh", ".gnupg", ".aws", ".kube", ".docker/config.json", ".netrc",
    "Library/Keychains", "Library/Cookies", "Library/Mail", "Library/Messages", "Library/Safari",
    "Library/Accounts", "Library/Calendars", "Library/Reminders", "Library/Photos",
    "Library/Passes", "Library/PersonalizationPortrait", "Library/Sharing", "Library/Suggestions",
    "Library/Application Support/AddressBook", "Library/Application Support/com.apple.TCC",
    "Library/Application Support/MobileSync", "Library/Application Support/1Password",
    "Library/Application Support/Bitwarden",
    "Library/Group Containers/group.com.apple.notes",
];

/// Browser profile roots. Inside these, only recognised cache folders may be touched, so
/// passwords, cookies, history and sessions cannot be reached by construction.
const BROWSER_PROFILES: &[&str] = &[
    "Library/Application Support/Google/Chrome",
    "Library/Application Support/Firefox",
    "Library/Application Support/Microsoft Edge",
    "Library/Application Support/BraveSoftware",
    "Library/Application Support/Arc",
    "Library/Containers/com.apple.Safari",
];

const BROWSER_CACHE_DIRS: &[&str] = &[
    "Cache", "Code Cache", "GPUCache", "ShaderCache", "DawnCache", "DawnGraphiteCache",
    "DawnWebGPUCache", "GrShaderCache", "CacheStorage", "cache2", "startupCache", "Caches",
];

const LEFTOVER_PARENTS: &[&str] = &[
    "Library/Application Support", "Library/Preferences", "Library/Caches", "Library/Containers",
    "Library/Group Containers", "Library/Saved Application State", "Library/HTTPStorages",
    "Library/WebKit", "Library/Logs",
];

const APP_DATA_PARENTS: &[&str] = &[
    "Library/Application Support", "Library/Preferences", "Library/Caches", "Library/Containers",
    "Library/Group Containers", "Library/Saved Application State", "Library/HTTPStorages",
    "Library/WebKit", "Library/Logs", "Library/LaunchAgents",
];

/// A protected path with a human-readable reason, or `None` when no absolute rule applies.
pub fn protection_reason(path: &Path, ctx: &SafetyContext) -> Option<String> {
    if path == Path::new("/") {
        return Some("The root of the disk is protected.".into());
    }
    if path.starts_with("/Volumes") && path.components().count() <= 3 {
        return Some("Whole volumes are protected.".into());
    }
    if path == Path::new("/Applications") || path == Path::new("/Users") {
        return Some("System folders are protected.".into());
    }
    if path.starts_with("/private") && !PRIVATE_ALLOWED.iter().any(|p| path.starts_with(p)) {
        return Some("Operating system data is protected.".into());
    }
    if SYSTEM_PREFIXES.iter().any(|p| path.starts_with(p)) && !path.starts_with(&ctx.home) {
        return Some("macOS system files are protected.".into());
    }
    if let Some(data) = &ctx.nexus_data {
        if path.starts_with(data) || data.starts_with(path) {
            return Some("NEXUS will not remove its own data.".into());
        }
    }
    if path == ctx.home || ctx.home.starts_with(path) {
        return Some("Your home folder is protected.".into());
    }
    if let Ok(relative) = path.strip_prefix(&ctx.home) {
        if HOME_ANCHORS.iter().any(|a| relative == Path::new(a)) {
            return Some("Standard home folders are protected.".into());
        }
        if HOME_PROTECTED.iter().any(|p| relative.starts_with(p)) {
            return Some("Personal data and credentials are protected.".into());
        }
        if let Some(root) = BROWSER_PROFILES.iter().find(|p| relative.starts_with(p)) {
            let inside = relative.strip_prefix(root).unwrap_or(relative);
            let is_cache = inside
                .components()
                .any(|c| BROWSER_CACHE_DIRS.iter().any(|d| c.as_os_str() == *d));
            if !is_cache {
                return Some(
                    "Browser passwords, cookies, history and sessions are protected.".into(),
                );
            }
        }
    }
    None
}

fn reject_traversal(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(NexusError::Safety(format!("{} is not an absolute path.", path.display())));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Err(NexusError::Safety(format!("{} contains relative segments.", path.display())));
    }
    Ok(())
}

/// Resolves every directory above the item while leaving the item itself unresolved, so a
/// symlink is acted on as a link and can never redirect the operation elsewhere.
pub fn resolve_parent(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| NexusError::Safety("The disk root cannot be a target.".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| NexusError::Safety(format!("{} has no file name.", path.display())))?;
    Ok(parent.canonicalize()?.join(name))
}

fn strictly_inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_reverse_dns(name: &str) -> bool {
    let stem = name.strip_suffix(".plist").or_else(|| name.strip_suffix(".savedState")).unwrap_or(name);
    let parts: Vec<&str> = stem.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

/// Accepts a path only if it exists, resolves safely and satisfies its policy.
/// Returns the path with its parents resolved, which is the path that must be acted on.
pub fn validate(path: &Path, policy: Policy, ctx: &SafetyContext) -> Result<PathBuf> {
    reject_traversal(path)?;
    fs::symlink_metadata(path).map_err(|_| {
        NexusError::NotFound(format!("{} no longer exists.", path.display()))
    })?;
    let resolved = resolve_parent(path)?;
    if let Some(reason) = protection_reason(&resolved, ctx) {
        return Err(NexusError::Safety(reason));
    }
    let home = canonical(&ctx.home);
    let allowed = match policy {
        Policy::CleanupTarget => {
            ctx.cleanup_items.iter().any(|i| canonical_item(i) == resolved)
                || ctx.cleanup_roots.iter().any(|r| strictly_inside(&resolved, &canonical(r)))
        }
        Policy::Leftover => {
            let parent = resolved.parent().unwrap_or(&resolved);
            let name = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("");
            is_reverse_dns(name)
                && !name.starts_with("com.apple.")
                && LEFTOVER_PARENTS.iter().any(|p| parent == home.join(p))
        }
        Policy::UserSelected => {
            ctx.approved_roots.iter().any(|r| strictly_inside(&resolved, &canonical(r)))
        }
        Policy::Application => {
            let parent = resolved.parent().unwrap_or(&resolved);
            let is_bundle = resolved.extension().is_some_and(|e| e == "app")
                && (parent == Path::new("/Applications") || parent == home.join("Applications"));
            let is_data = APP_DATA_PARENTS.iter().any(|p| parent == home.join(p));
            is_bundle || is_data
        }
    };
    if !allowed {
        return Err(NexusError::Safety(format!(
            "{} is outside the locations NEXUS may change.",
            path.display()
        )));
    }
    Ok(resolved)
}

fn canonical_item(item: &Path) -> PathBuf {
    resolve_parent(item).unwrap_or_else(|_| item.to_path_buf())
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
        for sub in ["Library/Caches", "Documents", "Downloads", "Library/Preferences"] {
            fs::create_dir_all(home.join(sub)).unwrap();
        }
        let ctx = SafetyContext {
            home: home.clone(),
            approved_roots: vec![home.join("Documents"), home.join("Downloads")],
            cleanup_roots: vec![home.join("Library/Caches")],
            cleanup_items: Vec::new(),
            nexus_data: Some(home.join("Library/Application Support/nexus")),
        };
        Fixture { _dir: dir, home, ctx }
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }

    #[test]
    fn allows_items_inside_cleanup_roots() {
        let f = fixture();
        let item = f.home.join("Library/Caches/com.example.app/data");
        touch(&item);
        assert_eq!(validate(&item, Policy::CleanupTarget, &f.ctx).unwrap(), item);
    }

    #[test]
    fn refuses_the_cleanup_root_itself() {
        let f = fixture();
        assert!(validate(&f.home.join("Library/Caches"), Policy::CleanupTarget, &f.ctx).is_err());
    }

    #[test]
    fn refuses_paths_outside_every_allowed_location() {
        let f = fixture();
        let doc = f.home.join("Documents/thesis.pdf");
        touch(&doc);
        assert!(validate(&doc, Policy::CleanupTarget, &f.ctx).is_err());
    }

    #[test]
    fn refuses_system_paths_under_every_policy() {
        let f = fixture();
        for path in ["/System/Library", "/usr/bin/true", "/bin/ls", "/etc/hosts", "/Library/Preferences"] {
            for policy in [Policy::CleanupTarget, Policy::UserSelected, Policy::Leftover, Policy::Application] {
                assert!(validate(Path::new(path), policy, &f.ctx).is_err(), "{path} {policy:?}");
            }
        }
    }

    #[test]
    fn refuses_home_and_standard_folders() {
        let f = fixture();
        for sub in ["", "Documents", "Downloads", "Library"] {
            let path = if sub.is_empty() { f.home.clone() } else { f.home.join(sub) };
            assert!(protection_reason(&path, &f.ctx).is_some(), "{sub}");
        }
    }

    #[test]
    fn refuses_parent_directory_traversal() {
        let f = fixture();
        let sneaky = f.home.join("Library/Caches/../../Documents/thesis.pdf");
        touch(&f.home.join("Documents/thesis.pdf"));
        assert!(matches!(
            validate(&sneaky, Policy::CleanupTarget, &f.ctx),
            Err(NexusError::Safety(_))
        ));
    }

    #[test]
    fn refuses_relative_paths() {
        let f = fixture();
        assert!(validate(Path::new("Library/Caches/x"), Policy::CleanupTarget, &f.ctx).is_err());
    }

    #[test]
    fn a_symlinked_parent_cannot_escape_the_cleanup_root() {
        let f = fixture();
        let secret = f.home.join("Documents/secret.txt");
        touch(&secret);
        symlink(f.home.join("Documents"), f.home.join("Library/Caches/escape")).unwrap();
        let through_link = f.home.join("Library/Caches/escape/secret.txt");
        assert!(validate(&through_link, Policy::CleanupTarget, &f.ctx).is_err());
    }

    #[test]
    fn a_symlink_item_is_treated_as_the_link_not_its_target() {
        let f = fixture();
        symlink("/usr", f.home.join("Library/Caches/link")).unwrap();
        let link = f.home.join("Library/Caches/link");
        assert_eq!(validate(&link, Policy::CleanupTarget, &f.ctx).unwrap(), link);
    }

    #[test]
    fn credentials_are_protected_even_inside_approved_roots() {
        let mut f = fixture();
        f.ctx.approved_roots.push(f.home.clone());
        let key = f.home.join(".ssh/id_ed25519");
        touch(&key);
        let keychain = f.home.join("Library/Keychains/login.keychain-db");
        touch(&keychain);
        assert!(validate(&key, Policy::UserSelected, &f.ctx).is_err());
        assert!(validate(&keychain, Policy::UserSelected, &f.ctx).is_err());
    }

    #[test]
    fn browser_identity_data_is_unreachable_but_caches_are_not() {
        let mut f = fixture();
        let chrome = f.home.join("Library/Application Support/Google/Chrome");
        f.ctx.cleanup_roots.push(chrome.clone());
        for sensitive in ["Default/Cookies", "Default/Login Data", "Default/History", "Default/Sessions/x"] {
            let path = chrome.join(sensitive);
            touch(&path);
            assert!(validate(&path, Policy::CleanupTarget, &f.ctx).is_err(), "{sensitive}");
        }
        let cache = chrome.join("Default/Code Cache/js/abc");
        touch(&cache);
        assert!(validate(&cache, Policy::CleanupTarget, &f.ctx).is_ok());
    }

    #[test]
    fn exact_items_do_not_open_up_their_siblings() {
        let mut f = fixture();
        let allowed = f.home.join(".npm/_cacache");
        let sibling = f.home.join(".npm/_logs");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        f.ctx.cleanup_items.push(allowed.clone());
        assert!(validate(&allowed, Policy::CleanupTarget, &f.ctx).is_ok());
        assert!(validate(&sibling, Policy::CleanupTarget, &f.ctx).is_err());
    }

    #[test]
    fn user_selection_is_limited_to_approved_roots() {
        let f = fixture();
        let inside = f.home.join("Downloads/a.dmg");
        let outside = f.home.join("Library/Preferences/x.plist");
        touch(&inside);
        touch(&outside);
        assert!(validate(&inside, Policy::UserSelected, &f.ctx).is_ok());
        assert!(validate(&outside, Policy::UserSelected, &f.ctx).is_err());
    }

    #[test]
    fn leftovers_must_look_like_third_party_bundle_ids() {
        let f = fixture();
        let prefs = f.home.join("Library/Preferences");
        let orphan = prefs.join("com.vendor.gone.plist");
        let apple = prefs.join("com.apple.finder.plist");
        let plain = prefs.join("settings.plist");
        for p in [&orphan, &apple, &plain] {
            touch(p);
        }
        assert!(validate(&orphan, Policy::Leftover, &f.ctx).is_ok());
        assert!(validate(&apple, Policy::Leftover, &f.ctx).is_err());
        assert!(validate(&plain, Policy::Leftover, &f.ctx).is_err());
    }

    #[test]
    fn leftovers_must_sit_directly_in_a_library_folder() {
        let f = fixture();
        let nested = f.home.join("Library/Preferences/deep/com.vendor.gone.plist");
        touch(&nested);
        assert!(validate(&nested, Policy::Leftover, &f.ctx).is_err());
    }

    #[test]
    fn nexus_never_removes_its_own_data() {
        let mut f = fixture();
        let support = f.home.join("Library/Application Support");
        f.ctx.cleanup_roots.push(support.clone());
        let db = support.join("nexus/nexus.db");
        touch(&db);
        assert!(validate(&db, Policy::CleanupTarget, &f.ctx).is_err());
    }

    #[test]
    fn missing_paths_are_reported_as_gone() {
        let f = fixture();
        let gone = f.home.join("Library/Caches/gone");
        assert!(matches!(validate(&gone, Policy::CleanupTarget, &f.ctx), Err(NexusError::NotFound(_))));
    }

    #[test]
    fn application_policy_covers_bundles_and_direct_support_entries_only() {
        let f = fixture();
        let bundle = f.home.join("Applications/Tool.app");
        fs::create_dir_all(&bundle).unwrap();
        let support = f.home.join("Library/Caches/com.vendor.tool");
        fs::create_dir_all(&support).unwrap();
        let doc = f.home.join("Documents/Tool.app");
        fs::create_dir_all(&doc).unwrap();
        assert!(validate(&bundle, Policy::Application, &f.ctx).is_ok());
        assert!(validate(&support, Policy::Application, &f.ctx).is_ok());
        assert!(validate(&doc, Policy::Application, &f.ctx).is_err());
    }
}

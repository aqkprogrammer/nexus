//! The complete list of places the cleaner is allowed to look. Nothing outside this
//! catalog can become a cleanup item.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    SystemJunk,
    Leftovers,
    Browser,
    Developer,
}

pub enum Source {
    /// Every direct child of this directory is a separate item.
    Children(PathBuf),
    /// This exact path is one item.
    Item(PathBuf),
    /// `root/*/name` for each listed name, used for browser profiles.
    ProfileDirs(PathBuf, &'static [&'static str]),
}

pub struct TargetDef {
    pub id: &'static str,
    pub group: Group,
    pub title: &'static str,
    pub description: &'static str,
    pub effect: &'static str,
    pub risk: Risk,
    pub selected_by_default: bool,
    pub sources: Vec<Source>,
    /// Names skipped when listing children: exact, or a prefix when ending in `*`.
    pub skip: &'static [&'static str],
    pub min_age_days: u32,
    /// Only items owned by this user, for shared locations such as /tmp.
    pub owned_only: bool,
    /// App that should be closed first; the UI warns while it is running.
    pub app: Option<&'static str>,
}

const CHROMIUM_CACHES: &[&str] = &["Cache", "Code Cache", "GPUCache", "DawnCache", "DawnGraphiteCache", "GrShaderCache", "ShaderCache"];

/// Caches handled by their own target, plus anything macOS manages itself.
const GENERIC_CACHE_SKIPS: &[&str] = &[
    "com.apple.*", "CloudKit", "FamilyCircle", "GeoServices", "Google", "Firefox", "BraveSoftware",
    "Microsoft Edge", "Arc", "company.thebrowser.Browser", "Homebrew", "pip", "Yarn", "pnpm", "CocoaPods",
    "go-build", "app.nexus.platform", "nexus",
];

fn item_paths(home: &Path, paths: &[&str]) -> Vec<Source> {
    paths.iter().map(|p| Source::Item(home.join(p))).collect()
}

pub fn targets(home: &Path, temp_dir: &Path, temp_min_age_days: u32) -> Vec<TargetDef> {
    let lib = home.join("Library");
    let caches = lib.join("Caches");
    let support = lib.join("Application Support");
    vec![
        TargetDef {
            id: "app-caches",
            group: Group::SystemJunk,
            title: "Application caches",
            description: "Data apps saved in ~/Library/Caches to speed themselves up.",
            effect: "Apps rebuild these automatically. The first launch afterwards may be slower.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![Source::Children(caches.clone())],
            skip: GENERIC_CACHE_SKIPS,
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "user-logs",
            group: Group::SystemJunk,
            title: "Log files",
            description: "Diagnostic logs written by apps to ~/Library/Logs.",
            effect: "Removes history that is only useful when troubleshooting. New logs are written as needed.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![Source::Children(lib.join("Logs"))],
            skip: &["DiagnosticReports", "com.apple.*"],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "crash-reports",
            group: Group::SystemJunk,
            title: "Crash reports",
            description: "Reports created when an app quit unexpectedly.",
            effect: "You lose the record of past crashes, which developers may ask for when debugging.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![Source::Children(lib.join("Logs/DiagnosticReports"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "old-diagnostics",
            group: Group::SystemJunk,
            title: "Old diagnostic files",
            description: "Crash reporter data older than 30 days.",
            effect: "Clears stale reporter data. Recent reports are left alone.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![Source::Children(support.join("CrashReporter"))],
            skip: &[],
            min_age_days: 30,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "temp-user",
            group: Group::SystemJunk,
            title: "Temporary files",
            description: "Your private temporary folder, limited to items untouched for several days.",
            effect: "Apps normally recreate what they need. Recently used items are skipped.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![Source::Children(temp_dir.to_path_buf())],
            // Dot entries are lock files and sockets for running apps, whatever their age.
            skip: &["com.apple.*", "TemporaryItems", ".*"],
            min_age_days: temp_min_age_days,
            owned_only: true,
            app: None,
        },
        TargetDef {
            id: "temp-shared",
            group: Group::SystemJunk,
            title: "Shared temporary files",
            description: "Your own files in /tmp that have not changed for over a week.",
            effect: "Review first: long-running tools sometimes keep working files here.",
            risk: Risk::Medium,
            selected_by_default: false,
            sources: vec![Source::Children(PathBuf::from("/private/tmp"))],
            skip: &["com.apple.*", "powerlog"],
            min_age_days: 7,
            owned_only: true,
            app: None,
        },
        TargetDef {
            id: "safari-cache",
            group: Group::Browser,
            title: "Safari cache",
            description: "Web content Safari stored to load pages faster.",
            effect: "Pages reload from the network once. History, cookies and passwords are not touched.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![
                Source::Item(caches.join("com.apple.Safari")),
                Source::Item(lib.join("Containers/com.apple.Safari/Data/Library/Caches")),
            ],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: Some("Safari"),
        },
        TargetDef {
            id: "chrome-cache",
            group: Group::Browser,
            title: "Chrome cache",
            description: "Web and code caches for each Chrome profile.",
            effect: "Pages reload from the network once. Passwords, cookies, history and sessions are not touched.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![
                Source::Children(caches.join("Google/Chrome")),
                Source::ProfileDirs(support.join("Google/Chrome"), CHROMIUM_CACHES),
            ],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: Some("Google Chrome"),
        },
        TargetDef {
            id: "firefox-cache",
            group: Group::Browser,
            title: "Firefox cache",
            description: "Web content stored by each Firefox profile.",
            effect: "Pages reload from the network once. Passwords, cookies, history and sessions are not touched.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![Source::Children(caches.join("Firefox/Profiles"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: Some("Firefox"),
        },
        TargetDef {
            id: "chromium-cache",
            group: Group::Browser,
            title: "Edge, Brave and Arc caches",
            description: "Browser temporary files for other Chromium-based browsers.",
            effect: "Pages reload from the network once. Saved logins and history are not touched.",
            risk: Risk::Low,
            selected_by_default: true,
            sources: vec![
                Source::Children(caches.join("Microsoft Edge")),
                Source::Children(caches.join("BraveSoftware/Brave-Browser")),
                Source::Children(caches.join("Arc")),
                Source::ProfileDirs(support.join("Microsoft Edge"), CHROMIUM_CACHES),
                Source::ProfileDirs(support.join("BraveSoftware/Brave-Browser"), CHROMIUM_CACHES),
            ],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "xcode-derived",
            group: Group::Developer,
            title: "Xcode DerivedData",
            description: "Intermediate build products and indexes for each Xcode project.",
            effect: "The next build of each project takes longer. Source code is not touched.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![Source::Children(lib.join("Developer/Xcode/DerivedData"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: Some("Xcode"),
        },
        TargetDef {
            id: "simulator-caches",
            group: Group::Developer,
            title: "iOS simulator caches",
            description: "Cached dyld and runtime data used by the simulators.",
            effect: "Simulators rebuild these on next boot. Installed simulator apps stay put.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![Source::Children(lib.join("Developer/CoreSimulator/Caches"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: Some("Simulator"),
        },
        TargetDef {
            id: "npm-cache",
            group: Group::Developer,
            title: "npm cache",
            description: "Downloaded package tarballs in ~/.npm/_cacache.",
            effect: "npm downloads packages again the next time they are installed.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: item_paths(home, &[".npm/_cacache"]),
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "pnpm-cache",
            group: Group::Developer,
            title: "pnpm cache",
            description: "pnpm's metadata cache. The content-addressed store is left alone because projects hard-link into it.",
            effect: "pnpm fetches package metadata again on the next install.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![Source::Item(caches.join("pnpm"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "yarn-cache",
            group: Group::Developer,
            title: "Yarn cache",
            description: "Packages cached by Yarn.",
            effect: "Yarn downloads packages again the next time they are installed.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![Source::Item(caches.join("Yarn"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "pip-cache",
            group: Group::Developer,
            title: "pip cache",
            description: "Python wheels and HTTP responses cached by pip.",
            effect: "pip downloads packages again the next time they are installed.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![Source::Item(caches.join("pip"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "cargo-cache",
            group: Group::Developer,
            title: "Cargo build cache",
            description: "Downloaded crate archives and extracted sources.",
            effect: "Cargo downloads crates again on the next build. Project target folders are listed in Space Hogs.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: item_paths(home, &[".cargo/registry/cache", ".cargo/registry/src"]),
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "homebrew-cache",
            group: Group::Developer,
            title: "Homebrew downloads",
            description: "Bottles and source archives Homebrew kept after installing.",
            effect: "Reinstalling a formula downloads it again.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![Source::Item(caches.join("Homebrew"))],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
        TargetDef {
            id: "other-dev-caches",
            group: Group::Developer,
            title: "Gradle, CocoaPods and Go caches",
            description: "Build tool caches that can be recreated from the network.",
            effect: "The next build downloads or recompiles what it needs.",
            risk: Risk::Low,
            selected_by_default: false,
            sources: vec![
                Source::Item(home.join(".gradle/caches")),
                Source::Item(caches.join("CocoaPods")),
                Source::Item(caches.join("go-build")),
            ],
            skip: &[],
            min_age_days: 0,
            owned_only: false,
            app: None,
        },
    ]
}

/// Bounds used by the safety layer, derived from the same catalog as the scan.
pub fn cleanup_bounds(defs: &[TargetDef], home: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut roots = vec![home.join(".Trash")];
    let mut items = Vec::new();
    for def in defs {
        for source in &def.sources {
            match source {
                Source::Children(dir) | Source::ProfileDirs(dir, _) => roots.push(dir.clone()),
                Source::Item(path) => items.push(path.clone()),
            }
        }
    }
    (roots, items)
}

pub fn should_skip(name: &str, skip: &[&str]) -> bool {
    skip.iter().any(|rule| match rule.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *rule,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_ids_are_unique() {
        let defs = targets(Path::new("/h"), Path::new("/t"), 3);
        let mut ids: Vec<_> = defs.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), defs.len());
    }

    #[test]
    fn nothing_above_low_risk_is_selected_by_default() {
        for def in targets(Path::new("/h"), Path::new("/t"), 3) {
            if def.selected_by_default {
                assert_eq!(def.risk, Risk::Low, "{}", def.id);
            }
        }
    }

    #[test]
    fn developer_caches_are_opt_in() {
        for def in targets(Path::new("/h"), Path::new("/t"), 3) {
            if def.group == Group::Developer {
                assert!(!def.selected_by_default, "{}", def.id);
            }
        }
    }

    #[test]
    fn no_target_reaches_documents_or_downloads() {
        let (roots, items) = cleanup_bounds(&targets(Path::new("/h"), Path::new("/t"), 3), Path::new("/h"));
        for path in roots.iter().chain(&items) {
            assert!(!path.starts_with("/h/Documents") && !path.starts_with("/h/Downloads"));
            assert_ne!(path, Path::new("/h"));
        }
    }

    #[test]
    fn temp_cleanup_leaves_system_items_and_app_sockets_alone() {
        let defs = targets(Path::new("/h"), Path::new("/t"), 3);
        let temp = defs.iter().find(|d| d.id == "temp-user").unwrap();
        assert!(should_skip("com.apple.WebKit.Networking", temp.skip));
        assert!(should_skip(".com.google.Chrome.abc123", temp.skip));
        assert!(should_skip("TemporaryItems", temp.skip));
        assert!(!should_skip("old-export.tmp", temp.skip));
    }

    #[test]
    fn skip_rules_support_exact_and_prefix() {
        assert!(should_skip("com.apple.dock", GENERIC_CACHE_SKIPS));
        assert!(should_skip("Homebrew", GENERIC_CACHE_SKIPS));
        assert!(!should_skip("com.spotify.client", GENERIC_CACHE_SKIPS));
    }
}

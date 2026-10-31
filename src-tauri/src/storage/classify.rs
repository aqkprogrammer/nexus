use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::util::lower_ext;

/// Where bytes belong in the storage overview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Applications,
    Documents,
    Downloads,
    Media,
    Developer,
    Caches,
    Other,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Applications,
        Category::Documents,
        Category::Downloads,
        Category::Media,
        Category::Developer,
        Category::Caches,
        Category::Other,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// What a file is, independent of where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    Folder,
    Video,
    Audio,
    Image,
    Document,
    Archive,
    Installer,
    DiskImage,
    Code,
    Application,
    Other,
}

pub fn file_kind(path: &Path) -> FileKind {
    let Some(ext) = lower_ext(path) else {
        return FileKind::Other;
    };
    match ext.as_str() {
        "mp4" | "mov" | "m4v" | "mkv" | "avi" | "webm" | "wmv" | "flv" | "mpg" | "mpeg" | "prores" => {
            FileKind::Video
        }
        "mp3" | "m4a" | "aac" | "wav" | "flac" | "aiff" | "ogg" | "opus" => FileKind::Audio,
        "jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "webp" | "tiff" | "tif" | "bmp" | "raw"
        | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd" | "svg" => FileKind::Image,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "pages" | "numbers" | "key" | "txt"
        | "rtf" | "md" | "csv" | "epub" => FileKind::Document,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" => FileKind::Archive,
        "pkg" | "mpkg" | "msi" | "exe" | "deb" | "rpm" | "xip" => FileKind::Installer,
        "dmg" | "iso" | "img" | "sparseimage" | "sparsebundle" | "qcow2" | "vmdk" | "vdi" | "vhd"
        | "vhdx" | "raw.img" => FileKind::DiskImage,
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "swift" | "c" | "cc" | "cpp" | "h" | "hpp"
        | "java" | "kt" | "rb" | "sh" | "json" | "toml" | "yaml" | "yml" | "html" | "css" => {
            FileKind::Code
        }
        "app" => FileKind::Application,
        _ => FileKind::Other,
    }
}

/// Directories whose contents always count as developer files.
const DEVELOPER_DIRS: &[&str] = &[
    "node_modules", "DerivedData", ".cargo", ".rustup", ".gradle", ".m2", ".npm", ".pnpm-store",
    ".yarn", ".nvm", ".pyenv", ".venv", "venv", "Pods", ".docker", ".terraform", ".git",
];

pub struct Locations {
    home: PathBuf,
}

impl Locations {
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    pub fn is_downloads(&self, path: &Path) -> bool {
        path.starts_with(self.home.join("Downloads"))
    }

    /// Category a directory imposes on everything beneath it, if any.
    pub fn context_for(&self, path: &Path, name: &str, parent: Option<Category>) -> Option<Category> {
        if parent == Some(Category::Caches) {
            return parent;
        }
        if let Ok(relative) = path.strip_prefix(&self.home) {
            match relative.to_str() {
                Some("Library/Caches") => return Some(Category::Caches),
                Some("Library/Developer") => return Some(Category::Developer),
                Some("Downloads") => return Some(Category::Downloads),
                Some("Documents" | "Desktop") => return Some(Category::Documents),
                Some("Pictures" | "Movies" | "Music") => return Some(Category::Media),
                Some("Applications") => return Some(Category::Applications),
                _ => {}
            }
        }
        if path == Path::new("/Applications") {
            return Some(Category::Applications);
        }
        if DEVELOPER_DIRS.contains(&name) {
            return Some(Category::Developer);
        }
        parent
    }

    pub fn category_for_file(&self, context: Option<Category>, kind: FileKind) -> Category {
        context.unwrap_or(match kind {
            FileKind::Video | FileKind::Audio | FileKind::Image => Category::Media,
            FileKind::Document => Category::Documents,
            FileKind::Application => Category::Applications,
            _ => Category::Other,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HogKind {
    LargeVideo,
    OldInstaller,
    ZipArchive,
    DiskImage,
    VirtualMachine,
    Docker,
    XcodeDerivedData,
    NodeModules,
    BuildArtifact,
    ProjectBackup,
    UnusedDownload,
    AiModels,
}

const VM_BUNDLES: &[&str] = &["utm", "vmwarevm", "pvm", "vbox"];
const BUILD_DIRS: &[&str] = &[".next", ".nuxt", ".turbo", ".svelte-kit", ".parcel-cache", "__pycache__"];

/// Classifies a directory as a known space hog. `has_sibling` reports names beside it.
pub fn hog_for_dir(path: &Path, name: &str, has_sibling: impl Fn(&str) -> bool) -> Option<HogKind> {
    let text = path.to_string_lossy();
    if name == "node_modules" {
        return Some(HogKind::NodeModules);
    }
    if name == "DerivedData" && text.contains("Developer/Xcode") {
        return Some(HogKind::XcodeDerivedData);
    }
    if lower_ext(path).is_some_and(|e| VM_BUNDLES.contains(&e.as_str())) {
        return Some(HogKind::VirtualMachine);
    }
    if text.ends_with("Containers/com.docker.docker/Data/vms") || text.ends_with(".docker/desktop/vms") {
        return Some(HogKind::Docker);
    }
    if text.ends_with(".ollama/models") || text.ends_with(".cache/huggingface") || text.ends_with(".lmstudio/models") {
        return Some(HogKind::AiModels);
    }
    let build_like = (name == "target" && has_sibling("Cargo.toml"))
        || ((name == "build" || name == "dist")
            && (has_sibling("package.json") || has_sibling("build.gradle") || has_sibling("Makefile")))
        || BUILD_DIRS.contains(&name);
    if build_like {
        return Some(HogKind::BuildArtifact);
    }
    if is_backup_name(name) {
        return Some(HogKind::ProjectBackup);
    }
    None
}

fn is_backup_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("backup") || lower.ends_with(".bak") || lower.ends_with(".old") || lower.ends_with("_old")
}

pub struct FileFacts {
    pub size: u64,
    pub age_days: i64,
    pub in_downloads: bool,
}

const MIB: u64 = 1024 * 1024;

pub fn hog_for_file(path: &Path, name: &str, kind: FileKind, facts: &FileFacts) -> Option<HogKind> {
    let hog = match kind {
        FileKind::Video if facts.size >= 100 * MIB => Some(HogKind::LargeVideo),
        FileKind::Installer if facts.size >= 20 * MIB => Some(HogKind::OldInstaller),
        FileKind::DiskImage => {
            let vm_disk = lower_ext(path)
                .is_some_and(|e| matches!(e.as_str(), "qcow2" | "vmdk" | "vdi" | "vhd" | "vhdx"));
            if vm_disk && facts.size >= 100 * MIB {
                Some(HogKind::VirtualMachine)
            } else if name.eq_ignore_ascii_case("Docker.raw") {
                Some(HogKind::Docker)
            } else if facts.size >= 50 * MIB {
                Some(HogKind::DiskImage)
            } else {
                None
            }
        }
        FileKind::Archive if facts.size >= 50 * MIB => Some(HogKind::ZipArchive),
        _ => None,
    };
    hog.or_else(|| {
        if name.eq_ignore_ascii_case("Docker.raw") {
            return Some(HogKind::Docker);
        }
        if facts.size >= 50 * MIB && is_backup_name(name) {
            return Some(HogKind::ProjectBackup);
        }
        (facts.in_downloads && facts.size >= 10 * MIB && facts.age_days >= 180)
            .then_some(HogKind::UnusedDownload)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(size: u64) -> FileFacts {
        FileFacts { size, age_days: 0, in_downloads: false }
    }

    #[test]
    fn extensions_map_to_kinds_case_insensitively() {
        assert_eq!(file_kind(Path::new("a.MOV")), FileKind::Video);
        assert_eq!(file_kind(Path::new("a.dmg")), FileKind::DiskImage);
        assert_eq!(file_kind(Path::new("noext")), FileKind::Other);
    }

    #[test]
    fn context_flows_down_and_caches_win() {
        let loc = Locations::new("/Users/u".into());
        let caches = loc.context_for(Path::new("/Users/u/Library/Caches"), "Caches", None);
        assert_eq!(caches, Some(Category::Caches));
        let nested = loc.context_for(Path::new("/Users/u/Library/Caches/x/node_modules"), "node_modules", caches);
        assert_eq!(nested, Some(Category::Caches));
        let docs = loc.context_for(Path::new("/Users/u/Documents"), "Documents", None);
        let project = loc.context_for(Path::new("/Users/u/Documents/p/node_modules"), "node_modules", docs);
        assert_eq!(project, Some(Category::Developer));
    }

    #[test]
    fn loose_media_counts_as_media_but_location_wins() {
        let loc = Locations::new("/Users/u".into());
        assert_eq!(loc.category_for_file(None, FileKind::Video), Category::Media);
        assert_eq!(
            loc.category_for_file(Some(Category::Downloads), FileKind::Video),
            Category::Downloads
        );
    }

    #[test]
    fn target_is_only_a_build_artifact_beside_cargo_toml() {
        let path = Path::new("/p/target");
        assert_eq!(hog_for_dir(path, "target", |n| n == "Cargo.toml"), Some(HogKind::BuildArtifact));
        assert_eq!(hog_for_dir(path, "target", |_| false), None);
    }

    #[test]
    fn detects_named_hogs() {
        assert_eq!(hog_for_dir(Path::new("/p/node_modules"), "node_modules", |_| false), Some(HogKind::NodeModules));
        assert_eq!(hog_for_dir(Path::new("/u/Library/Developer/Xcode/DerivedData"), "DerivedData", |_| false), Some(HogKind::XcodeDerivedData));
        assert_eq!(hog_for_dir(Path::new("/vms/ubuntu.utm"), "ubuntu.utm", |_| false), Some(HogKind::VirtualMachine));
        assert_eq!(hog_for_dir(Path::new("/u/.ollama/models"), "models", |_| false), Some(HogKind::AiModels));
    }

    #[test]
    fn size_thresholds_keep_small_files_out() {
        let p = Path::new("/x/a.zip");
        assert_eq!(hog_for_file(p, "a.zip", FileKind::Archive, &facts(MIB)), None);
        assert_eq!(hog_for_file(p, "a.zip", FileKind::Archive, &facts(60 * MIB)), Some(HogKind::ZipArchive));
    }

    #[test]
    fn old_downloads_are_flagged_only_when_old() {
        let p = Path::new("/d/a.bin");
        let old = FileFacts { size: 20 * MIB, age_days: 200, in_downloads: true };
        let new = FileFacts { size: 20 * MIB, age_days: 2, in_downloads: true };
        assert_eq!(hog_for_file(p, "a.bin", FileKind::Other, &old), Some(HogKind::UnusedDownload));
        assert_eq!(hog_for_file(p, "a.bin", FileKind::Other, &new), None);
    }
}

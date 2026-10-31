use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::util::{run_with_timeout, sysctl_i64, sysctl_string};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: Option<String>,
    pub file_system: Option<String>,
    pub protocol: Option<String>,
    pub smart_status: Option<String>,
    pub solid_state: Option<bool>,
    pub encrypted: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemOverview {
    pub model_name: Option<String>,
    pub model_identifier: Option<String>,
    pub chip: Option<String>,
    pub architecture: String,
    pub cpu_brand: Option<String>,
    pub physical_cores: Option<u32>,
    pub logical_cores: Option<u32>,
    pub performance_cores: Option<u32>,
    pub efficiency_cores: Option<u32>,
    pub memory_total: u64,
    pub os_name: String,
    pub os_version: Option<String>,
    pub os_build: Option<String>,
    pub kernel: Option<String>,
    pub host_name: Option<String>,
    pub boot_time: Option<u64>,
    pub disk: DiskInfo,
}

pub fn collect() -> SystemOverview {
    let hardware = hardware_profile();
    let field = |key: &str| hardware[key].as_str().map(str::to_owned);
    let core_count = |name: &str| sysctl_i64(name).filter(|v| *v > 0).map(|v| v as u32);
    let os_version = sysinfo::System::os_version().ok();
    let memory_total = sysinfo::System::new()
        .map(|mut sys| {
            sys.refresh_memory();
            sys.total_memory()
        })
        .unwrap_or(0);

    SystemOverview {
        model_name: field("machine_name"),
        model_identifier: field("machine_model").or_else(|| sysctl_string("hw.model")),
        chip: field("chip_type").or_else(|| field("cpu_type")),
        architecture: std::env::consts::ARCH.to_owned(),
        cpu_brand: sysctl_string("machdep.cpu.brand_string"),
        physical_cores: core_count("hw.physicalcpu"),
        logical_cores: core_count("hw.logicalcpu"),
        performance_cores: core_count("hw.perflevel0.physicalcpu"),
        efficiency_cores: core_count("hw.perflevel1.physicalcpu"),
        memory_total,
        os_name: marketing_os_name(os_version.as_deref()),
        os_version,
        os_build: sysctl_string("kern.osversion"),
        kernel: sysinfo::System::kernel_version().ok(),
        host_name: sysinfo::System::host_name().ok(),
        boot_time: sysinfo::System::boot_time().ok(),
        disk: disk_info(),
    }
}

fn marketing_os_name(version: Option<&str>) -> String {
    let major = version.and_then(|v| v.split('.').next()).and_then(|v| v.parse::<u32>().ok());
    match major {
        Some(13) => "macOS Ventura",
        Some(14) => "macOS Sonoma",
        Some(15) => "macOS Sequoia",
        Some(26) => "macOS Tahoe",
        _ => "macOS",
    }
    .to_owned()
}

fn hardware_profile() -> serde_json::Value {
    let mut command = Command::new("/usr/sbin/system_profiler");
    command.args(["-json", "SPHardwareDataType"]);
    run_with_timeout(command, Duration::from_secs(15))
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|v| v["SPHardwareDataType"].as_array().and_then(|a| a.first().cloned()))
        .unwrap_or(serde_json::Value::Null)
}

fn diskutil(target: &str) -> Option<plist::Dictionary> {
    let mut command = Command::new("/usr/sbin/diskutil");
    command.args(["info", "-plist", target]);
    let output = run_with_timeout(command, Duration::from_secs(10))?;
    plist::Value::from_reader(std::io::Cursor::new(output.stdout)).ok()?.into_dictionary()
}

/// "disk0s2" becomes "disk0". Whole-disk records hold the physical drive's name.
fn whole_disk(device: &str) -> Option<&str> {
    let digits = device.strip_prefix("disk")?;
    let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
    (end > 0).then(|| &device[..4 + end])
}

fn disk_info() -> DiskInfo {
    // The sealed system volume reports a snapshot name, so read the data volume instead.
    let volume = diskutil("/System/Volumes/Data").or_else(|| diskutil("/"));
    let physical = volume
        .as_ref()
        .and_then(|v| v.get("APFSPhysicalStores")?.as_array()?.first()?.as_dictionary()?.get("APFSPhysicalStore")?.as_string())
        .and_then(whole_disk)
        .and_then(diskutil);
    let text = |dict: Option<&plist::Dictionary>, key: &str| {
        dict.and_then(|d| d.get(key))
            .and_then(|v| v.as_string())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let flag = |key: &str| volume.as_ref().and_then(|d| d.get(key)).and_then(|v| v.as_boolean());
    DiskInfo {
        name: text(physical.as_ref(), "MediaName").or_else(|| text(volume.as_ref(), "VolumeName")),
        file_system: text(volume.as_ref(), "FilesystemName").or_else(|| text(volume.as_ref(), "FilesystemType")),
        protocol: text(volume.as_ref(), "BusProtocol"),
        smart_status: text(physical.as_ref(), "SMARTStatus").or_else(|| text(volume.as_ref(), "SMARTStatus")),
        solid_state: flag("SolidState"),
        encrypted: flag("FileVault").or_else(|| flag("Encryption")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_resolve_to_their_whole_disk() {
        assert_eq!(whole_disk("disk0s2"), Some("disk0"));
        assert_eq!(whole_disk("disk12"), Some("disk12"));
        assert_eq!(whole_disk("nvme0"), None);
        assert_eq!(whole_disk("disk"), None);
    }

    #[test]
    fn unknown_os_versions_stay_generic_rather_than_guessed() {
        assert_eq!(marketing_os_name(Some("15.1")), "macOS Sequoia");
        assert_eq!(marketing_os_name(Some("99.0")), "macOS");
        assert_eq!(marketing_os_name(None), "macOS");
    }
}

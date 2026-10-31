pub mod battery;
pub mod health;
pub mod iokit;
pub mod memory;
pub mod overview;
pub mod processes;
pub mod telemetry;

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSnapshot {
    pub mount_point: String,
    pub total: u64,
    /// Includes purgeable space macOS would release on demand, as Finder reports it.
    pub available: u64,
    pub used: u64,
}

/// The user data volume, which is what "storage" means on a modern sealed-system Mac.
pub fn data_volume() -> VolumeSnapshot {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else {
        return VolumeSnapshot::default();
    };
    let disk = ["/System/Volumes/Data", "/"]
        .iter()
        .find_map(|m| disks.iter().find(|d| d.mount_point().as_os_str() == *m));
    match disk {
        Some(d) => {
            let total = d.total_space();
            let available = d.available_space();
            VolumeSnapshot {
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                total,
                available,
                used: total.saturating_sub(available),
            }
        }
        None => VolumeSnapshot::default(),
    }
}

/// Cumulative bytes read and written across all block devices since boot.
pub fn disk_io_totals() -> Option<(u64, u64)> {
    let drivers = iokit::services("IOBlockStorageDriver");
    let mut seen = false;
    let (mut read, mut written) = (0u64, 0u64);
    for driver in drivers {
        if let Some(stats) = driver.dict("Statistics") {
            read += stats.i64("Bytes (Read)").unwrap_or(0).max(0) as u64;
            written += stats.i64("Bytes (Write)").unwrap_or(0).max(0) as u64;
            seen = true;
        }
    }
    seen.then_some((read, written))
}

/// Foundation's thermal state: the only supported thermal signal on every Mac.
pub fn thermal_state() -> &'static str {
    use objc2_foundation::{NSProcessInfo, NSProcessInfoThermalState as T};
    match NSProcessInfo::processInfo().thermalState() {
        T::Nominal => "nominal",
        T::Fair => "fair",
        T::Serious => "serious",
        T::Critical => "critical",
        _ => "unknown",
    }
}

/// Average of the die sensors the OS exposes, or `None` when this Mac reports none.
pub fn cpu_temperature(components: &mut sysinfo::Components) -> Option<f32> {
    components.refresh(true);
    let readings: Vec<f32> = components
        .iter()
        .filter(|c| {
            let label = c.label().to_ascii_lowercase();
            label.contains("cpu")
                || label.contains("tdie")
                || label.contains("pacc")
                || label.contains("eacc")
                || label.contains("soc mtr")
        })
        .filter_map(|c| c.temperature())
        .filter(|t| (10.0..=125.0).contains(t))
        .collect();
    if readings.is_empty() {
        None
    } else {
        Some(readings.iter().sum::<f32>() / readings.len() as f32)
    }
}

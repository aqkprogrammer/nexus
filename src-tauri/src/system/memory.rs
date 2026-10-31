use serde::Serialize;

use crate::util::sysctl_i64;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    pub total: u64,
    /// App + wired + compressed, matching Activity Monitor's "Memory Used".
    pub used: u64,
    pub app: u64,
    pub wired: u64,
    pub compressed: u64,
    /// File-backed and purgeable memory macOS can reclaim instantly. Not "wasted".
    pub cached: u64,
    pub free: u64,
    pub available: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    /// normal, warning or critical, from the kernel's own pressure signal.
    pub pressure: String,
    /// Kernel estimate of free memory headroom, 0–100.
    pub pressure_level: Option<u8>,
}

pub fn snapshot(sys: &sysinfo::System) -> MemorySnapshot {
    let total = sys.total_memory();
    let mut snap = MemorySnapshot {
        total,
        swap_used: sys.used_swap(),
        swap_total: sys.total_swap(),
        ..Default::default()
    };

    let mut stat: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: stat is a correctly sized vm_statistics64 and count is its word length.
    let rc = unsafe {
        #[allow(deprecated)]
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            (&mut stat as *mut libc::vm_statistics64).cast(),
            &mut count,
        )
    };
    if rc == 0 {
        // SAFETY: sysconf has no preconditions.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        let pages = |n: u64| n.saturating_mul(page);
        snap.app = pages(u64::from(stat.internal_page_count).saturating_sub(u64::from(stat.purgeable_count)));
        snap.wired = pages(u64::from(stat.wire_count));
        snap.compressed = pages(u64::from(stat.compressor_page_count));
        snap.cached = pages(u64::from(stat.external_page_count) + u64::from(stat.purgeable_count));
        snap.free = pages(u64::from(stat.free_count).saturating_sub(u64::from(stat.speculative_count)));
        snap.used = snap.app + snap.wired + snap.compressed;
    } else {
        snap.used = sys.used_memory();
    }
    snap.available = total.saturating_sub(snap.used);

    snap.pressure = pressure_name(sysctl_i64("kern.memorystatus_vm_pressure_level")).to_owned();
    snap.pressure_level = sysctl_i64("kern.memorystatus_level").map(|v| v.clamp(0, 100) as u8);
    snap
}

fn pressure_name(level: Option<i64>) -> &'static str {
    match level {
        Some(4) => "critical",
        Some(2) => "warning",
        Some(_) => "normal",
        None => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_kernel_pressure_levels() {
        assert_eq!(pressure_name(Some(1)), "normal");
        assert_eq!(pressure_name(Some(2)), "warning");
        assert_eq!(pressure_name(Some(4)), "critical");
        assert_eq!(pressure_name(None), "unknown");
    }

    #[test]
    fn live_snapshot_is_internally_consistent() {
        let mut sys = sysinfo::System::new().unwrap();
        sys.refresh_memory();
        let m = snapshot(&sys);
        assert!(m.total > 0);
        assert!(m.used > 0 && m.used <= m.total);
        assert_eq!(m.available, m.total - m.used);
        assert!(m.cached < m.total);
    }
}

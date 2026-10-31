use std::collections::HashMap;
use std::ffi::{c_int, c_void};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

use crate::error::{NexusError, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub app: Option<String>,
    pub exe: Option<String>,
    pub cpu: f32,
    /// Physical footprint when readable (Activity Monitor's figure), otherwise RSS.
    pub memory: u64,
    pub disk_read: u64,
    pub disk_write: u64,
    pub status: String,
    pub start_time: u64,
    pub run_time: u64,
    pub owned: bool,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroup {
    pub name: String,
    pub processes: u32,
    pub cpu: f32,
    pub memory: u64,
}

pub struct ProcessTable {
    sys: Mutex<(Option<System>, Option<Instant>)>,
    uid: u32,
}

const PROTECTED_NAMES: &[&str] = &[
    "launchd", "kernel_task", "WindowServer", "loginwindow", "Finder", "Dock", "SystemUIServer",
    "ControlCenter", "NotificationCenter", "Spotlight", "mds", "mds_stores", "coreaudiod",
    "bluetoothd", "cfprefsd", "distnoted", "securityd", "trustd", "tccd", "opendirectoryd",
    "syslogd", "logd", "powerd", "configd", "diskarbitrationd", "fseventsd", "sysmond",
];

const PROTECTED_PREFIXES: &[&str] =
    &["/System/", "/usr/libexec/", "/usr/sbin/", "/sbin/", "/bin/", "/usr/bin/"];

/// True when NEXUS must never offer to stop this process, whoever owns it.
pub fn is_protected(pid: u32, name: &str, exe: Option<&str>) -> bool {
    pid <= 1
        || pid == std::process::id()
        || PROTECTED_NAMES.contains(&name)
        || exe.is_some_and(|e| PROTECTED_PREFIXES.iter().any(|p| e.starts_with(p)))
}

/// Name of the outermost .app bundle an executable lives in.
pub fn app_name_from_exe(exe: &str) -> Option<String> {
    exe.split('/')
        .find(|segment| segment.ends_with(".app"))
        .map(|bundle| bundle.trim_end_matches(".app").to_owned())
}

fn status_name(status: ProcessStatus) -> &'static str {
    match status {
        ProcessStatus::Run => "Running",
        ProcessStatus::Sleep | ProcessStatus::Idle => "Sleeping",
        ProcessStatus::Stop => "Stopped",
        ProcessStatus::Zombie => "Zombie",
        _ => "Unknown",
    }
}

// Mirrors `struct rusage_info_v2` in <sys/resource.h>.
#[repr(C)]
#[derive(Default)]
struct RusageInfoV2 {
    uuid: [u8; 16],
    user_time: u64,
    system_time: u64,
    pkg_idle_wkups: u64,
    interrupt_wkups: u64,
    pageins: u64,
    wired_size: u64,
    resident_size: u64,
    phys_footprint: u64,
    proc_start_abstime: u64,
    proc_exit_abstime: u64,
    child_user_time: u64,
    child_system_time: u64,
    child_pkg_idle_wkups: u64,
    child_interrupt_wkups: u64,
    child_pageins: u64,
    child_elapsed_abstime: u64,
    diskio_bytesread: u64,
    diskio_byteswritten: u64,
}

extern "C" {
    fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *mut c_void) -> c_int;
}

/// Readable for our own processes; other users' processes fall back to RSS.
fn phys_footprint(pid: u32) -> Option<u64> {
    let mut info = RusageInfoV2::default();
    // SAFETY: flavor 2 is RUSAGE_INFO_V2, which writes exactly one RusageInfoV2.
    let rc = unsafe { proc_pid_rusage(pid as c_int, 2, (&mut info as *mut RusageInfoV2).cast()) };
    (rc == 0 && info.phys_footprint > 0).then_some(info.phys_footprint)
}

pub fn process_count() -> Option<u32> {
    // SAFETY: a null buffer asks only for the number of pids.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    (count > 0).then_some(count as u32)
}

impl Default for ProcessTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessTable {
    pub fn new() -> Self {
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        Self { sys: Mutex::new((System::new().ok(), None)), uid }
    }

    pub fn list(&self) -> Vec<ProcessInfo> {
        let mut guard = self.sys.lock();
        let (sys, last) = &mut *guard;
        let Some(sys) = sys else {
            return Vec::new();
        };
        // sysinfo needs two samples for CPU%, and a burst of callers should share one.
        if last.is_none_or(|t| t.elapsed() > Duration::from_millis(800)) {
            let kind = ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_user(UpdateKind::OnlyIfNotSet)
                .with_exe(UpdateKind::OnlyIfNotSet);
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
            *last = Some(Instant::now());
        }

        sys.processes()
            .values()
            .filter(|p| p.thread_kind().is_none())
            .map(|p| {
                let pid = p.pid().as_u32();
                let name = p.name().to_string_lossy().into_owned();
                let exe = p.exe().map(|e| e.to_string_lossy().into_owned());
                let disk = p.disk_usage();
                let owned = p.user_id().is_some_and(|u| **u == self.uid);
                ProcessInfo {
                    pid,
                    parent_pid: p.parent().map(Pid::as_u32),
                    app: exe.as_deref().and_then(app_name_from_exe),
                    protected: is_protected(pid, &name, exe.as_deref()),
                    name,
                    exe,
                    cpu: p.cpu_usage(),
                    memory: phys_footprint(pid).unwrap_or_else(|| p.memory()),
                    disk_read: disk.read_bytes,
                    disk_write: disk.written_bytes,
                    status: status_name(p.status()).to_owned(),
                    start_time: p.start_time(),
                    run_time: p.run_time(),
                    owned,
                }
            })
            .collect()
    }

    /// Sends SIGTERM (or SIGKILL when forced) after re-checking eligibility against live state.
    /// `start_time` guards against the pid having been reused since the user confirmed.
    pub fn terminate(&self, pid: u32, start_time: u64, force: bool) -> Result<()> {
        let info = self
            .list()
            .into_iter()
            .find(|p| p.pid == pid && p.start_time == start_time)
            .ok_or_else(|| NexusError::NotFound("That process has already exited.".into()))?;
        if info.protected {
            return Err(NexusError::Safety(format!(
                "{} is a protected system process and cannot be terminated.",
                info.name
            )));
        }
        if !info.owned {
            return Err(NexusError::Safety(format!(
                "{} belongs to another user and cannot be terminated.",
                info.name
            )));
        }
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        // SAFETY: plain kill(2); pid was just verified to be this user's live process.
        if unsafe { libc::kill(pid as i32, signal) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
}

pub fn groups(processes: &[ProcessInfo]) -> Vec<ProcessGroup> {
    let mut map: HashMap<String, ProcessGroup> = HashMap::new();
    for p in processes {
        let name = p.app.clone().unwrap_or_else(|| p.name.clone());
        let entry = map.entry(name.clone()).or_insert(ProcessGroup {
            name,
            processes: 0,
            cpu: 0.0,
            memory: 0,
        });
        entry.processes += 1;
        entry.cpu += p.cpu;
        entry.memory += p.memory;
    }
    let mut list: Vec<_> = map.into_values().collect();
    list.sort_by_key(|x| std::cmp::Reverse(x.memory));
    list
}

pub fn open_activity_monitor() -> Result<()> {
    std::process::Command::new("/usr/bin/open")
        .args(["-a", "Activity Monitor"])
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_processes_are_protected() {
        assert!(is_protected(1, "launchd", None));
        assert!(is_protected(500, "WindowServer", None));
        assert!(is_protected(500, "helper", Some("/System/Library/CoreServices/helper")));
        assert!(is_protected(500, "zsh", Some("/bin/zsh")));
        assert!(is_protected(std::process::id(), "nexus", None));
    }

    #[test]
    fn user_apps_are_eligible() {
        let exe = "/Applications/Slack.app/Contents/MacOS/Slack";
        assert!(!is_protected(9999, "Slack", Some(exe)));
    }

    #[test]
    fn finds_the_outermost_bundle() {
        let helper =
            "/Applications/Google Chrome.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper";
        assert_eq!(app_name_from_exe(helper).as_deref(), Some("Google Chrome"));
        assert_eq!(app_name_from_exe("/usr/bin/python3"), None);
    }

    #[test]
    fn groups_sum_helpers_under_their_app() {
        let p = |name: &str, app: Option<&str>, memory| ProcessInfo {
            pid: 10,
            parent_pid: None,
            name: name.into(),
            app: app.map(str::to_owned),
            exe: None,
            cpu: 1.0,
            memory,
            disk_read: 0,
            disk_write: 0,
            status: "Running".into(),
            start_time: 0,
            run_time: 0,
            owned: true,
            protected: false,
        };
        let list = [
            p("Chrome", Some("Google Chrome"), 100),
            p("Helper", Some("Google Chrome"), 300),
            p("node", None, 50),
        ];
        let g = groups(&list);
        assert_eq!(g[0].name, "Google Chrome");
        assert_eq!(g[0].processes, 2);
        assert_eq!(g[0].memory, 400);
    }

    #[test]
    fn terminate_refuses_mismatched_identity() {
        let table = ProcessTable::new();
        let err = table.terminate(std::process::id(), 1, false).unwrap_err();
        assert!(matches!(err, NexusError::NotFound(_)));
    }

    #[test]
    fn terminate_refuses_self_even_with_correct_identity() {
        let table = ProcessTable::new();
        let me = table.list().into_iter().find(|p| p.pid == std::process::id()).unwrap();
        let err = table.terminate(me.pid, me.start_time, false).unwrap_err();
        assert!(matches!(err, NexusError::Safety(_)));
    }

    #[test]
    fn footprint_is_readable_for_our_own_process() {
        assert!(phys_footprint(std::process::id()).is_some());
    }
}

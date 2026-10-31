use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Space a file really occupies on disk. `len()` overstates sparse files such as Docker.raw.
pub fn allocated_size(meta: &std::fs::Metadata) -> u64 {
    meta.blocks().saturating_mul(512)
}

pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn sysctl_string(name: &str) -> Option<String> {
    let name = CString::new(name).ok()?;
    let mut size = 0usize;
    // SAFETY: a null output pointer asks sysctl for the required length.
    let rc = unsafe {
        libc::sysctlbyname(name.as_ptr(), std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0)
    };
    if rc != 0 || size == 0 {
        return None;
    }
    let mut buffer = vec![0u8; size];
    // SAFETY: buffer holds `size` writable bytes and size is passed by reference.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    CStr::from_bytes_until_nul(&buffer)
        .ok()
        .map(|s| s.to_string_lossy().into_owned())
}

pub fn sysctl_i64(name: &str) -> Option<i64> {
    let name = CString::new(name).ok()?;
    let mut value = 0i64;
    let mut size = std::mem::size_of::<i64>();
    // SAFETY: value is an 8-byte buffer; the kernel writes `size` bytes, at most 8.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&mut value as *mut i64).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    // 4-byte sysctls fill only the low half of `value` on little-endian machines.
    Some(if size == 4 { value as i32 as i64 } else { value })
}

/// Runs a command with a hard deadline so a hung helper cannot wedge a worker thread.
pub fn run_with_timeout(mut command: Command, timeout: Duration) -> Option<std::process::Output> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let deadline = Instant::now() + timeout;
    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(result) => result.ok(),
        Err(_) => {
            // SAFETY: plain kill(2) on the child we spawned; a stale pid only fails.
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            None
        }
    }
}

pub fn lower_ext(path: &Path) -> Option<String> {
    path.extension()
        .map(|e| String::from_utf8_lossy(e.as_bytes()).to_ascii_lowercase())
}

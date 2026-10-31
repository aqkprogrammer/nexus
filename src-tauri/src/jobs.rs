use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::error::{NexusError, Result};

#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// One long-running job per kind. Holding the guard keeps the slot; dropping it frees it.
#[derive(Clone, Default)]
pub struct Jobs {
    running: Arc<Mutex<HashMap<String, CancelToken>>>,
}

pub struct JobGuard {
    jobs: Jobs,
    kind: String,
    pub token: CancelToken,
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.jobs.running.lock().remove(&self.kind);
    }
}

impl Jobs {
    pub fn start(&self, kind: &str) -> Result<JobGuard> {
        let mut running = self.running.lock();
        if running.contains_key(kind) {
            return Err(NexusError::Busy(format!("a {kind} job is already running")));
        }
        let token = CancelToken::default();
        running.insert(kind.to_owned(), token.clone());
        Ok(JobGuard { jobs: self.clone(), kind: kind.to_owned(), token })
    }

    pub fn cancel(&self, kind: &str) -> bool {
        match self.running.lock().get(kind) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn is_running(&self, kind: &str) -> bool {
        self.running.lock().contains_key(kind)
    }
}

/// Worker pool for filesystem jobs: bounded width, and utility QoS so the UI and
/// foreground apps stay responsive during a whole-disk scan.
pub fn io_pool() -> Result<rayon::ThreadPool> {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    rayon::ThreadPoolBuilder::new()
        .num_threads(cores.clamp(2, 8))
        .stack_size(16 * 1024 * 1024)
        .thread_name(|i| format!("nexus-io-{i}"))
        .start_handler(|_| {
            // SAFETY: changes the QoS class of the calling thread only.
            unsafe {
                libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
            }
        })
        .build()
        .map_err(|e| NexusError::Unavailable(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_job_of_the_same_kind_is_refused_until_the_first_ends() {
        let jobs = Jobs::default();
        let guard = jobs.start("storage").unwrap();
        assert!(jobs.start("storage").is_err());
        assert!(jobs.start("cleaner").is_ok());
        drop(guard);
        assert!(jobs.start("storage").is_ok());
    }

    #[test]
    fn cancel_reaches_the_running_job() {
        let jobs = Jobs::default();
        let guard = jobs.start("duplicates").unwrap();
        assert!(jobs.cancel("duplicates"));
        assert!(guard.token.is_cancelled());
        assert!(!jobs.cancel("nothing"));
    }
}

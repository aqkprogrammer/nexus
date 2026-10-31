use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use sysinfo::{Components, CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tauri::{AppHandle, Emitter};

use super::battery::{self, AppleBatteryProfile, BatterySnapshot};
use super::health::{self, HealthInput, HealthReport};
use super::memory::{self, MemorySnapshot};
use super::processes::{process_count, ProcessTable};
use super::{cpu_temperature, data_volume, disk_io_totals, thermal_state, VolumeSnapshot};
use crate::db::{Db, MetricRow};
use crate::settings::Settings;
use crate::util::now_ms;

const TICK: Duration = Duration::from_secs(2);
const HISTORY_LEN: usize = 300;
const ROLLUP_EVERY: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySample {
    pub ts: i64,
    pub cpu: f64,
    pub per_core: Vec<f32>,
    pub load_average: [f64; 3],
    pub memory: MemorySnapshot,
    pub disk_read_bps: Option<f64>,
    pub disk_write_bps: Option<f64>,
    pub volume: VolumeSnapshot,
    pub battery: Option<BatterySnapshot>,
    pub cpu_temperature: Option<f32>,
    pub thermal_state: String,
    pub process_count: Option<u32>,
    pub uptime: u64,
    pub health: HealthReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    pub ts: i64,
    pub cpu: f64,
    pub memory_used: u64,
    pub disk_read_bps: Option<f64>,
    pub disk_write_bps: Option<f64>,
    pub disk_used: u64,
    pub disk_available: u64,
    pub battery: Option<f64>,
    pub temperature: Option<f32>,
    pub process_count: Option<u32>,
    pub cycle_count: Option<u32>,
    pub uptime: u64,
    pub health: f64,
}

pub struct Telemetry {
    latest: RwLock<Option<TelemetrySample>>,
    history: Mutex<VecDeque<HistoryPoint>>,
    pub processes: ProcessTable,
}

impl Telemetry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            latest: RwLock::new(None),
            history: Mutex::new(VecDeque::with_capacity(HISTORY_LEN)),
            processes: ProcessTable::new(),
        })
    }

    pub fn latest(&self) -> Option<TelemetrySample> {
        self.latest.read().clone()
    }

    pub fn history(&self) -> Vec<HistoryPoint> {
        self.history.lock().iter().cloned().collect()
    }

    fn record(&self, sample: TelemetrySample) {
        let mut history = self.history.lock();
        if history.len() == HISTORY_LEN {
            history.pop_front();
        }
        history.push_back(HistoryPoint {
            ts: sample.ts,
            cpu: sample.cpu,
            memory_used: sample.memory.used,
            disk_read_bps: sample.disk_read_bps,
            disk_write_bps: sample.disk_write_bps,
            disk_used: sample.volume.used,
            disk_available: sample.volume.available,
            battery: sample.battery.as_ref().map(|b| b.percent),
            temperature: sample.cpu_temperature,
            process_count: sample.process_count,
            cycle_count: sample.battery.as_ref().and_then(|b| b.cycle_count),
            uptime: sample.uptime,
            health: sample.health.overall,
        });
        *self.latest.write() = Some(sample);
    }

    fn cpu_average(&self, current: f64) -> f64 {
        let history = self.history.lock();
        let recent = history.iter().rev().take(29).map(|p| p.cpu);
        let (sum, count) = recent.fold((current, 1.0), |(s, n), v| (s + v, n + 1.0));
        sum / count
    }
}

/// The sampler is the app's heartbeat. Expensive probes run on slower cadences so that
/// watching the machine does not meaningfully load it.
pub fn spawn(
    app: AppHandle,
    telemetry: Arc<Telemetry>,
    db: Arc<Db>,
    settings: Arc<RwLock<Settings>>,
) {
    std::thread::Builder::new()
        .name("nexus-telemetry".into())
        .spawn(move || {
            // SAFETY: changes the QoS class of this thread only.
            unsafe {
                libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
            }
            let refresh = RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_usage())
                .with_memory(MemoryRefreshKind::everything());
            let Ok(mut sys) = System::new_with_specifics(refresh) else {
                eprintln!("NEXUS: system metrics are unavailable on this Mac");
                return;
            };
            let mut components = Components::new_with_refreshed_list().ok();
            let mut profile = AppleBatteryProfile::default();
            let mut profile_at: Option<Instant> = None;
            let mut temperature = None;
            let mut temperature_at: Option<Instant> = None;
            let mut volume = data_volume();
            let mut volume_at = Instant::now();
            let mut bat: Option<BatterySnapshot> = None;
            let mut bat_at: Option<Instant> = None;
            let mut last_io = disk_io_totals().map(|t| (t, Instant::now()));
            let mut rollup_at: Option<Instant> = None;
            let due = |at: Option<Instant>, every: Duration| at.is_none_or(|t| t.elapsed() >= every);

            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
            loop {
                sys.refresh_cpu_usage();
                sys.refresh_memory();

                if due(profile_at, Duration::from_secs(3600)) {
                    profile = battery::apple_profile();
                    profile_at = Some(Instant::now());
                }
                if due(bat_at, Duration::from_secs(20)) {
                    bat = battery::read(&profile);
                    bat_at = Some(Instant::now());
                }
                if due(temperature_at, Duration::from_secs(10)) {
                    temperature = components.as_mut().and_then(cpu_temperature);
                    temperature_at = Some(Instant::now());
                }
                if volume_at.elapsed() >= Duration::from_secs(15) {
                    volume = data_volume();
                    volume_at = Instant::now();
                }

                let (disk_read_bps, disk_write_bps) = match disk_io_totals() {
                    Some(totals) => {
                        let now = Instant::now();
                        let rates = last_io.map(|((r0, w0), t0)| {
                            let dt = now.duration_since(t0).as_secs_f64().max(0.001);
                            (
                                totals.0.saturating_sub(r0) as f64 / dt,
                                totals.1.saturating_sub(w0) as f64 / dt,
                            )
                        });
                        last_io = Some((totals, now));
                        (rates.map(|r| r.0), rates.map(|r| r.1))
                    }
                    None => (None, None),
                };

                let cpu = f64::from(sys.global_cpu_usage());
                let per_core = sys.cpus().iter().map(|c| c.usage()).collect::<Vec<_>>();
                let memory = memory::snapshot(&sys);
                let load = System::load_average().map_or([0.0; 3], |l| [l.one, l.five, l.fifteen]);
                let thermal = thermal_state();
                let cores = per_core.len().max(1) as f64;
                let report = health::evaluate(&HealthInput {
                    volume: &volume,
                    memory: &memory,
                    cpu_average: telemetry.cpu_average(cpu),
                    load_per_core: load[0] / cores,
                    thermal_state: thermal,
                    battery: bat.as_ref(),
                });

                let sample = TelemetrySample {
                    ts: now_ms(),
                    cpu,
                    per_core,
                    load_average: load,
                    memory,
                    disk_read_bps,
                    disk_write_bps,
                    volume: volume.clone(),
                    battery: bat.clone(),
                    cpu_temperature: temperature,
                    thermal_state: thermal.to_owned(),
                    process_count: process_count(),
                    uptime: System::uptime().unwrap_or(0),
                    health: report,
                };

                if settings.read().record_history && due(rollup_at, ROLLUP_EVERY) {
                    rollup_at = Some(Instant::now());
                    let _ = db.insert_metric(&rollup(&sample));
                }

                let _ = app.emit("telemetry://tick", &sample);
                telemetry.record(sample);
                std::thread::sleep(TICK);
            }
        })
        .expect("telemetry thread must start");
}

fn rollup(s: &TelemetrySample) -> MetricRow {
    MetricRow {
        ts: s.ts,
        cpu: s.cpu,
        memory_used: s.memory.used,
        memory_total: s.memory.total,
        disk_used: s.volume.used,
        disk_total: s.volume.total,
        battery: s.battery.as_ref().map(|b| b.percent),
        health: s.health.overall,
        storage_score: s.health.storage.score,
        memory_score: s.health.memory.score,
        cpu_score: s.health.cpu.score,
        battery_score: s.health.battery.as_ref().map(|b| b.score),
        performance_score: s.health.performance.score,
    }
}

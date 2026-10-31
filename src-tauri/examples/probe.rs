//! Read-only diagnostic: prints what the native collectors see on this Mac.
//! `cargo run --example probe -- /path/to/scan [/more/paths]`

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use nexus_lib::apps::{self, leftovers};
use nexus_lib::cleaner::{self, catalog};
use nexus_lib::jobs::{io_pool, CancelToken};
use nexus_lib::storage::scanner::{self, ScanOptions};
use nexus_lib::system::{self, battery, health, memory, overview, processes};
use nexus_lib::util::home_dir;

fn main() {
    let overview = overview::collect();
    println!("overview  {}", serde_json::to_string_pretty(&overview).unwrap());

    let refresh = sysinfo::RefreshKind::nothing()
        .with_cpu(sysinfo::CpuRefreshKind::nothing().with_usage())
        .with_memory(sysinfo::MemoryRefreshKind::everything());
    let mut sys = sysinfo::System::new_with_specifics(refresh).unwrap();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let memory = memory::snapshot(&sys);
    println!("memory    {}", serde_json::to_string(&memory).unwrap());
    let volume = system::data_volume();
    println!("volume    {}", serde_json::to_string(&volume).unwrap());
    let profile = battery::apple_profile();
    let bat = battery::read(&profile);
    println!("battery   {}", serde_json::to_string(&bat).unwrap());
    println!("thermal   {}", system::thermal_state());
    let temp = sysinfo::Components::new_with_refreshed_list().ok().as_mut().and_then(system::cpu_temperature);
    println!("cpu temp  {temp:?}");
    let t0 = system::disk_io_totals();
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("disk io   {t0:?} -> {:?}", system::disk_io_totals());
    println!("processes {:?}", processes::process_count());
    let report = health::evaluate(&health::HealthInput {
        volume: &volume,
        memory: &memory,
        cpu_average: f64::from(sys.global_cpu_usage()),
        load_per_core: 0.3,
        thermal_state: system::thermal_state(),
        battery: bat.as_ref(),
    });
    println!("health    {:.0} {}", report.overall, report.status);

    let table = processes::ProcessTable::new();
    table.list();
    std::thread::sleep(std::time::Duration::from_millis(900));
    let list = table.list();
    let groups = processes::groups(&list);
    println!("procs     {} listed, top apps:", list.len());
    for g in groups.iter().take(4) {
        println!("          {:<28} {:>3} procs {:>7.1}% {:>9}", g.name, g.processes, g.cpu, nexus_lib::advisor::gb(g.memory));
    }

    let pool = io_pool().unwrap();
    let roots: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if !roots.is_empty() {
        let started = Instant::now();
        let options = ScanOptions {
            roots,
            exclusions: nexus_lib::settings::Settings::default().exclusions.iter().map(PathBuf::from).collect(),
            home: home_dir(),
            old_file_months: 12,
            cache: None,
        };
        let (result, cache) = scanner::run(options, &CancelToken::default(), &pool, |_| {});
        let s = &result.summary;
        println!(
            "scan      {} in {:.2?}: {} files, {} dirs, {} denied, {} hogs, {} top files, {} cacheable dirs",
            nexus_lib::advisor::gb(s.total_bytes), started.elapsed(), s.files, s.directories, s.inaccessible,
            s.hogs.len(), s.top_files.len(), cache.len()
        );
        println!("tree json {} KB pruned", serde_json::to_vec(&result.tree.pruned(0)).unwrap().len() / 1024);
    }

    let home = home_dir();
    let started = Instant::now();
    let installed = apps::list(&home, &HashSet::new());
    let ids: Vec<String> = installed.iter().filter_map(|a| a.bundle_id.clone()).collect();
    println!("apps      {} found in {:.2?}", installed.len(), started.elapsed());
    let started = Instant::now();
    let leftovers = leftovers::find(&home, &ids);
    let input = cleaner::ScanInput {
        home: home.clone(),
        defs: catalog::targets(&home, &std::env::temp_dir(), 3),
        running: HashSet::new(),
        leftovers,
        hogs: Vec::new(),
    };
    let report = cleaner::scan(input, &CancelToken::default(), &pool, |_| {});
    println!("cleaner   done in {:.2?}: reclaimable {}, recommended {}", started.elapsed(), nexus_lib::advisor::gb(report.reclaimable), nexus_lib::advisor::gb(report.recommended));
    for t in report.targets.iter().filter(|t| t.total > 0 || t.unavailable.is_some()) {
        println!("          {:<34} {:>9} {:>5} items {}", t.title, nexus_lib::advisor::gb(t.total), t.items.len(), t.unavailable.as_deref().unwrap_or(""));
    }
    println!("trash     accessible={} total={} count={}", report.trash.accessible, nexus_lib::advisor::gb(report.trash.total), report.trash.count);
    println!("docker    installed={} running={} {}", report.docker.installed, report.docker.running, report.docker.message);
}

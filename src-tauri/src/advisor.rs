//! NEXUS AI. Recommendations come from deterministic rules over locally collected facts.
//! The engine only ever proposes: every action routes to a screen where the user confirms.

use serde::Serialize;

use crate::cleaner::catalog::Risk;
use crate::cleaner::CleanerReport;
use crate::duplicates::DuplicateReport;
use crate::storage::classify::HogKind;
use crate::storage::scanner::ScanSummary;
use crate::system::processes::ProcessGroup;
use crate::system::telemetry::TelemetrySample;

const GB: f64 = 1_000_000_000.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub issue: String,
    pub why: String,
    pub impact: String,
    pub estimate: String,
    pub estimate_bytes: Option<u64>,
    pub action: String,
    pub risk: Risk,
    pub severity: u8,
    pub action_label: String,
    pub route: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorReport {
    pub generated_at: i64,
    pub engine: String,
    pub summary: String,
    pub recommendations: Vec<Recommendation>,
    pub missing_data: Vec<String>,
}

pub struct Facts<'a> {
    pub sample: Option<&'a TelemetrySample>,
    pub storage: Option<&'a ScanSummary>,
    pub cleaner: Option<&'a CleanerReport>,
    pub duplicates: Option<&'a DuplicateReport>,
    pub process_groups: &'a [ProcessGroup],
}

/// Extension point for an optional LLM. Implementations receive recommendations only,
/// never file contents, and may reword them but cannot add actions.
pub trait AdvisorProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn narrate(&self, report: &AdvisorReport) -> Option<String>;
}

pub struct LocalRules;

impl AdvisorProvider for LocalRules {
    fn name(&self) -> &'static str {
        "Local rules"
    }

    fn narrate(&self, _report: &AdvisorReport) -> Option<String> {
        None
    }
}

pub fn gb(bytes: u64) -> String {
    let value = bytes as f64 / GB;
    if value >= 1.0 {
        format!("{value:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / 1_000_000.0)
    }
}

/// Memory is measured in binary units, as Activity Monitor and Apple's specs do.
fn gib(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
}

fn storage_rules(facts: &Facts<'_>, out: &mut Vec<Recommendation>) {
    if let Some(volume) = facts.sample.map(|s| &s.volume).filter(|v| v.total > 0) {
        let free = volume.available as f64 / volume.total as f64 * 100.0;
        if free < 15.0 {
            let critical = free < 10.0;
            out.push(Recommendation {
                id: "low-disk".into(),
                domain: "storage".into(),
                title: if critical { "Disk is nearly full" } else { "Disk space is getting low" }.into(),
                issue: format!(
                    "Your available storage is {free:.0}% ({} free of {}).",
                    gb(volume.available),
                    gb(volume.total)
                ),
                why: "macOS needs free space for swap, updates and APFS snapshots. Performance and app stability drop when it runs short.".into(),
                impact: "Reclaiming space restores room for virtual memory and system updates.".into(),
                estimate: "Depends on what you review".into(),
                estimate_bytes: None,
                action: "Review large media files and the prioritized cleanup list.".into(),
                risk: Risk::Low,
                severity: if critical { 95 } else { 75 },
                action_label: "Open Optimization Center".into(),
                route: "optimize".into(),
            });
        }
    }

    let Some(storage) = facts.storage else {
        return;
    };
    if storage.old.downloads_bytes as f64 >= GB {
        let months = storage.old.cutoff_months;
        out.push(Recommendation {
            id: "old-downloads".into(),
            domain: "storage".into(),
            title: "Old files in Downloads".into(),
            issue: format!(
                "Your Downloads folder contains {} across {} files that have not been modified in more than {months} months.",
                gb(storage.old.downloads_bytes),
                storage.old.downloads_files
            ),
            why: "Downloads tends to collect installers and one-off files. Age alone does not make a file unnecessary, so these need your judgement.".into(),
            impact: "Frees space with no effect on installed apps.".into(),
            estimate: format!("Up to {}", gb(storage.old.downloads_bytes)),
            estimate_bytes: Some(storage.old.downloads_bytes),
            action: "Review these files and keep anything you still need.".into(),
            risk: Risk::Medium,
            severity: 55,
            action_label: "Review in Space Hogs".into(),
            route: "hogs".into(),
        });
    }

    let hog_total = |kind: HogKind| -> u64 {
        storage.hogs.iter().filter(|h| h.kind == kind).map(|h| h.size).sum()
    };
    let derived = hog_total(HogKind::XcodeDerivedData);
    if derived as f64 >= 2.0 * GB {
        out.push(Recommendation {
            id: "derived-data".into(),
            domain: "developer".into(),
            title: "Xcode DerivedData is large".into(),
            issue: format!("Xcode DerivedData is consuming {}.", gb(derived)),
            why: "Xcode rebuilds this data, and stale projects keep their build products indefinitely.".into(),
            impact: "The next build of affected projects will take longer.".into(),
            estimate: format!("Up to {}", gb(derived)),
            estimate_bytes: Some(derived),
            action: "You can review and remove selected build caches.".into(),
            risk: Risk::Low,
            severity: 50,
            action_label: "Open Smart Cleaner".into(),
            route: "cleaner".into(),
        });
    }

    let modules = hog_total(HogKind::NodeModules);
    if modules as f64 >= 5.0 * GB {
        out.push(Recommendation {
            id: "node-modules".into(),
            domain: "developer".into(),
            title: "node_modules folders add up".into(),
            issue: format!("node_modules folders are consuming {}.", gb(modules)),
            why: "Each project keeps its own dependency tree, including projects you no longer work on.".into(),
            impact: "A project needs a fresh install before it runs again.".into(),
            estimate: format!("Up to {}", gb(modules)),
            estimate_bytes: Some(modules),
            action: "Remove these from inactive projects only.".into(),
            risk: Risk::Medium,
            severity: 45,
            action_label: "Review in Space Hogs".into(),
            route: "hogs".into(),
        });
    }

    let (count, bytes) = storage.over_1_gb;
    if count >= 3 {
        out.push(Recommendation {
            id: "huge-files".into(),
            domain: "storage".into(),
            title: "Very large files found".into(),
            issue: format!("{count} files over 1 GB are using {}.", gb(bytes)),
            why: "A handful of very large files often accounts for much of a full disk.".into(),
            impact: "Moving or removing even one can free significant space.".into(),
            estimate: format!("Up to {}", gb(bytes)),
            estimate_bytes: Some(bytes),
            action: "Check whether each is still needed, or move it to external storage.".into(),
            risk: Risk::Medium,
            severity: 40,
            action_label: "Inspect largest files".into(),
            route: "storage".into(),
        });
    }
}

fn cleaner_rules(facts: &Facts<'_>, out: &mut Vec<Recommendation>) {
    let Some(cleaner) = facts.cleaner else {
        return;
    };
    if cleaner.recommended as f64 >= 0.5 * GB {
        out.push(Recommendation {
            id: "safe-cleanup".into(),
            domain: "storage".into(),
            title: "Low-risk cleanup available".into(),
            issue: format!("{} of caches, logs and temporary files can be cleared.", gb(cleaner.recommended)),
            why: "Apps and browsers recreate this data when they need it.".into(),
            impact: "Reclaims space. Some apps may be briefly slower on their next launch.".into(),
            estimate: gb(cleaner.recommended),
            estimate_bytes: Some(cleaner.recommended),
            action: "Review the categories and run the cleanup.".into(),
            risk: Risk::Low,
            severity: 60,
            action_label: "Open Smart Cleaner".into(),
            route: "cleaner".into(),
        });
    }
    if cleaner.trash.total as f64 >= GB {
        out.push(Recommendation {
            id: "trash".into(),
            domain: "storage".into(),
            title: "Trash is holding space".into(),
            issue: format!("The Trash contains {} across {} items.", gb(cleaner.trash.total), cleaner.trash.count),
            why: "Files in the Trash still occupy the disk until it is emptied.".into(),
            impact: "Emptying is permanent. Check for anything you may want back first.".into(),
            estimate: gb(cleaner.trash.total),
            estimate_bytes: Some(cleaner.trash.total),
            action: "Review the Trash contents, then empty it.".into(),
            risk: Risk::Medium,
            severity: 52,
            action_label: "Review Trash".into(),
            route: "cleaner".into(),
        });
    }
    let docker: u64 = cleaner.docker.resources.iter().filter(|r| r.id != "volumes").map(|r| r.reclaimable).sum();
    if docker as f64 >= 2.0 * GB {
        out.push(Recommendation {
            id: "docker".into(),
            domain: "developer".into(),
            title: "Docker has reclaimable space".into(),
            issue: format!("Docker reports {} reclaimable, excluding volumes.", gb(docker)),
            why: "Build cache and stopped containers accumulate over time.".into(),
            impact: "Images and layers are rebuilt or pulled again on demand.".into(),
            estimate: gb(docker),
            estimate_bytes: Some(docker),
            action: "Preview Docker's reclaimable resources in the cleaner.".into(),
            risk: Risk::Low,
            severity: 44,
            action_label: "Open Smart Cleaner".into(),
            route: "cleaner".into(),
        });
    }
}

fn duplicate_rules(facts: &Facts<'_>, out: &mut Vec<Recommendation>) {
    let Some(report) = facts.duplicates.filter(|d| d.recoverable as f64 >= 0.5 * GB) else {
        return;
    };
    out.push(Recommendation {
        id: "duplicates".into(),
        domain: "storage".into(),
        title: "Duplicate files found".into(),
        issue: format!(
            "{} groups of identical files are wasting {}.",
            report.groups.len(),
            gb(report.recoverable)
        ),
        why: "Exact copies take full space unless they are APFS clones, which NEXUS cannot distinguish.".into(),
        impact: "Removing extra copies frees up to this amount.".into(),
        estimate: format!("Up to {}", gb(report.recoverable)),
        estimate_bytes: Some(report.recoverable),
        action: "Choose which copies to keep in each group.".into(),
        risk: Risk::Medium,
        severity: 48,
        action_label: "Open Duplicate Finder".into(),
        route: "duplicates".into(),
    });
}

fn performance_rules(facts: &Facts<'_>, out: &mut Vec<Recommendation>) {
    if let Some(sample) = facts.sample {
        if matches!(sample.memory.pressure.as_str(), "warning" | "critical") {
            let top = facts.process_groups.first();
            out.push(Recommendation {
                id: "memory-pressure".into(),
                domain: "performance".into(),
                title: "Memory is under pressure".into(),
                issue: format!(
                    "macOS reports {} memory pressure{}.",
                    sample.memory.pressure,
                    top.map(|g| format!("; {} is the largest user", g.name)).unwrap_or_default()
                ),
                why: "Under pressure macOS compresses memory and swaps to disk, which slows everything down.".into(),
                impact: "Closing heavy apps relieves pressure immediately.".into(),
                estimate: "Immediate responsiveness gain".into(),
                estimate_bytes: None,
                action: "Review the heaviest apps in the performance monitor.".into(),
                risk: Risk::Low,
                severity: if sample.memory.pressure == "critical" { 90 } else { 70 },
                action_label: "Open Performance Monitor".into(),
                route: "performance".into(),
            });
        }
        if matches!(sample.thermal_state.as_str(), "serious" | "critical") {
            out.push(Recommendation {
                id: "thermal".into(),
                domain: "performance".into(),
                title: "Your Mac is running hot".into(),
                issue: format!("The system thermal state is {}.", sample.thermal_state),
                why: "macOS throttles the CPU to shed heat, reducing performance.".into(),
                impact: "Reducing sustained load lets the system recover.".into(),
                estimate: "Restores full CPU speed".into(),
                estimate_bytes: None,
                action: "Check for runaway processes and give the Mac airflow.".into(),
                risk: Risk::Low,
                severity: 80,
                action_label: "Open Performance Monitor".into(),
                route: "performance".into(),
            });
        }
    }
    if let Some(group) = facts
        .process_groups
        .iter()
        .find(|g| g.memory >= 3 * 1_073_741_824 && g.processes > 1)
    {
        out.push(Recommendation {
            id: format!("heavy-app-{}", group.name.to_lowercase().replace(' ', "-")),
            domain: "performance".into(),
            title: format!("{} is using a lot of memory", group.name),
            issue: format!(
                "{} is currently consuming {} of RAM across {} processes.",
                group.name,
                gib(group.memory),
                group.processes
            ),
            why: "Multi-process apps hide their total footprint behind many small entries.".into(),
            impact: "Closing unused windows or tabs returns memory to other apps.".into(),
            estimate: format!("Up to {} of RAM", gib(group.memory)),
            estimate_bytes: None,
            action: "Close what you are not using in that app.".into(),
            risk: Risk::Low,
            severity: 35,
            action_label: "Open Performance Monitor".into(),
            route: "performance".into(),
        });
    }
}

fn battery_rules(facts: &Facts<'_>, out: &mut Vec<Recommendation>) {
    let Some(battery) = facts.sample.and_then(|s| s.battery.as_ref()) else {
        return;
    };
    let degraded = battery.max_capacity_percent.is_some_and(|c| c < 80.0)
        || battery
            .condition
            .as_deref()
            .is_some_and(|c| !c.eq_ignore_ascii_case("good") && !c.eq_ignore_ascii_case("normal"));
    if degraded {
        out.push(Recommendation {
            id: "battery-wear".into(),
            domain: "battery".into(),
            title: "Battery capacity is reduced".into(),
            issue: format!(
                "macOS reports condition \"{}\" with maximum capacity {}.",
                battery.condition.as_deref().unwrap_or("Unavailable"),
                battery
                    .max_capacity_percent
                    .map_or("Unavailable".to_owned(), |c| format!("{c:.0}%"))
            ),
            why: "Batteries wear chemically with cycles and age. No software can restore lost capacity.".into(),
            impact: "Shorter time on battery. Apple Support can advise on service options.".into(),
            estimate: "Not recoverable through software".into(),
            estimate_bytes: None,
            action: "Review high-energy apps to make the most of current capacity.".into(),
            risk: Risk::Low,
            severity: 42,
            action_label: "Open Battery Intelligence".into(),
            route: "battery".into(),
        });
    }
}

pub fn evaluate(facts: &Facts<'_>, provider: &dyn AdvisorProvider) -> AdvisorReport {
    let mut recommendations = Vec::new();
    storage_rules(facts, &mut recommendations);
    cleaner_rules(facts, &mut recommendations);
    duplicate_rules(facts, &mut recommendations);
    performance_rules(facts, &mut recommendations);
    battery_rules(facts, &mut recommendations);
    recommendations.sort_by_key(|x| std::cmp::Reverse(x.severity));

    let mut missing_data = Vec::new();
    if facts.storage.is_none() {
        missing_data.push("Run a storage scan so NEXUS can analyze large files and folders.".to_owned());
    }
    if facts.cleaner.is_none() {
        missing_data.push("Run Smart Cleaner so NEXUS can estimate reclaimable caches.".to_owned());
    }
    if facts.duplicates.is_none() {
        missing_data.push("Run Duplicate Finder to include duplicate files.".to_owned());
    }

    let reclaimable: u64 = recommendations.iter().filter_map(|r| r.estimate_bytes).sum();
    let summary = match recommendations.len() {
        0 => "No issues detected in the data NEXUS has collected.".to_owned(),
        n if reclaimable > 0 => format!(
            "{n} recommendation{} ready, covering up to {} of storage.",
            if n == 1 { "" } else { "s" },
            gb(reclaimable)
        ),
        n => format!("{n} recommendation{} ready.", if n == 1 { "" } else { "s" }),
    };
    let mut report = AdvisorReport {
        generated_at: crate::util::now_ms(),
        engine: provider.name().to_owned(),
        summary,
        recommendations,
        missing_data,
    };
    if let Some(text) = provider.narrate(&report) {
        report.summary = text;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::scanner::{CategoryStat, HogItem, OldFileStat};
    use crate::system::battery::BatterySnapshot;
    use crate::system::health::{self, HealthInput};
    use crate::system::memory::MemorySnapshot;
    use crate::system::VolumeSnapshot;

    fn empty_facts() -> Facts<'static> {
        Facts { sample: None, storage: None, cleaner: None, duplicates: None, process_groups: &[] }
    }

    fn sample(free: u64, pressure: &str, battery: Option<BatterySnapshot>) -> TelemetrySample {
        let volume = VolumeSnapshot { mount_point: "/".into(), total: 100 * 1_000_000_000, available: free * 1_000_000_000, used: 0 };
        let memory = MemorySnapshot { total: 100, pressure: pressure.into(), ..Default::default() };
        let health = health::evaluate(&HealthInput {
            volume: &volume,
            memory: &memory,
            cpu_average: 5.0,
            load_per_core: 0.1,
            thermal_state: "nominal",
            battery: None,
        });
        TelemetrySample {
            ts: 0,
            cpu: 5.0,
            per_core: Vec::new(),
            load_average: [0.0; 3],
            memory,
            disk_read_bps: None,
            disk_write_bps: None,
            volume,
            battery,
            cpu_temperature: None,
            thermal_state: "nominal".into(),
            process_count: None,
            uptime: 0,
            health,
        }
    }

    fn summary(hogs: Vec<HogItem>, old_downloads: u64) -> ScanSummary {
        ScanSummary {
            started_at: 0,
            finished_at: 0,
            roots: Vec::new(),
            completed: true,
            total_bytes: 0,
            files: 0,
            directories: 0,
            inaccessible: 0,
            reused_directories: 0,
            adjusted_at: None,
            categories: Vec::<CategoryStat>::new(),
            over_100_mb: (0, 0),
            over_1_gb: (0, 0),
            old: OldFileStat { cutoff_months: 12, downloads_bytes: old_downloads, downloads_files: 40, ..Default::default() },
            top_files: Vec::new(),
            hogs,
        }
    }

    fn hog(kind: HogKind, size: u64) -> HogItem {
        HogItem { path: "/x".into(), name: "x".into(), kind, size, files: 1, modified: 0, accessed: 0, is_dir: true }
    }

    #[test]
    fn with_no_data_it_asks_for_scans_instead_of_inventing_findings() {
        let report = evaluate(&empty_facts(), &LocalRules);
        assert!(report.recommendations.is_empty());
        assert_eq!(report.missing_data.len(), 3);
    }

    #[test]
    fn low_disk_is_flagged_and_outranks_routine_items() {
        let s = sample(8, "normal", None);
        let summary = summary(Vec::new(), 18_400_000_000);
        let facts = Facts { sample: Some(&s), storage: Some(&summary), ..empty_facts() };
        let report = evaluate(&facts, &LocalRules);
        assert_eq!(report.recommendations[0].id, "low-disk");
        assert!(report.recommendations[0].issue.contains("8%"));
    }

    #[test]
    fn old_downloads_wording_quotes_measured_size_and_needs_review() {
        let summary = summary(Vec::new(), 18_400_000_000);
        let facts = Facts { storage: Some(&summary), ..empty_facts() };
        let rec = evaluate(&facts, &LocalRules).recommendations.remove(0);
        assert!(rec.issue.contains("18.4 GB"));
        assert_eq!(rec.risk, Risk::Medium);
    }

    #[test]
    fn derived_data_threshold_applies() {
        let small = summary(vec![hog(HogKind::XcodeDerivedData, 1_000_000_000)], 0);
        let big = summary(vec![hog(HogKind::XcodeDerivedData, 8_200_000_000)], 0);
        let none = evaluate(&Facts { storage: Some(&small), ..empty_facts() }, &LocalRules);
        let some = evaluate(&Facts { storage: Some(&big), ..empty_facts() }, &LocalRules);
        assert!(none.recommendations.is_empty());
        assert!(some.recommendations[0].issue.contains("8.2 GB"));
    }

    #[test]
    fn memory_rule_uses_pressure_not_usage() {
        let calm = sample(50, "normal", None);
        let tight = sample(50, "critical", None);
        assert!(evaluate(&Facts { sample: Some(&calm), ..empty_facts() }, &LocalRules).recommendations.is_empty());
        let r = evaluate(&Facts { sample: Some(&tight), ..empty_facts() }, &LocalRules);
        assert_eq!(r.recommendations[0].id, "memory-pressure");
    }

    #[test]
    fn heavy_multiprocess_apps_are_named_with_their_total() {
        let groups = [ProcessGroup { name: "Chrome".into(), processes: 22, cpu: 4.0, memory: 3_650_722_201 }];
        let facts = Facts { process_groups: &groups, ..empty_facts() };
        let rec = evaluate(&facts, &LocalRules).recommendations.remove(0);
        assert!(rec.issue.contains("Chrome is currently consuming 3.4 GB"));
    }

    #[test]
    fn battery_advice_never_promises_recovery() {
        let battery = BatterySnapshot {
            max_capacity_percent: Some(72.0),
            condition: Some("Service Recommended".into()),
            ..Default::default()
        };
        let s = sample(50, "normal", Some(battery));
        let rec = evaluate(&Facts { sample: Some(&s), ..empty_facts() }, &LocalRules)
            .recommendations
            .remove(0);
        assert_eq!(rec.id, "battery-wear");
        assert!(rec.estimate.contains("Not recoverable"));
        assert!(rec.estimate_bytes.is_none());
    }

    #[test]
    fn every_recommendation_routes_to_a_screen_rather_than_acting() {
        let summary = summary(vec![hog(HogKind::NodeModules, 9_000_000_000)], 18_400_000_000);
        let s = sample(5, "warning", None);
        let facts = Facts { sample: Some(&s), storage: Some(&summary), ..empty_facts() };
        for rec in evaluate(&facts, &LocalRules).recommendations {
            assert!(["optimize", "hogs", "cleaner", "storage", "performance", "duplicates", "battery"]
                .contains(&rec.route.as_str()));
        }
    }

    struct Narrator;
    impl AdvisorProvider for Narrator {
        fn name(&self) -> &'static str {
            "Test"
        }
        fn narrate(&self, _: &AdvisorReport) -> Option<String> {
            Some("Rewritten".into())
        }
    }

    #[test]
    fn a_provider_can_reword_the_summary_but_not_the_findings() {
        let s = sample(5, "normal", None);
        let facts = Facts { sample: Some(&s), ..empty_facts() };
        let local = evaluate(&facts, &LocalRules);
        let narrated = evaluate(&facts, &Narrator);
        assert_eq!(narrated.summary, "Rewritten");
        assert_eq!(narrated.recommendations.len(), local.recommendations.len());
    }
}

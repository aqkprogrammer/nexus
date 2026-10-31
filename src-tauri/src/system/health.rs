//! NEXUS's own heuristic health score. It is not an Apple diagnostic.

use serde::Serialize;

use super::battery::BatterySnapshot;
use super::memory::MemorySnapshot;
use super::VolumeSnapshot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthComponent {
    pub score: f64,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub overall: f64,
    pub status: String,
    pub storage: HealthComponent,
    pub memory: HealthComponent,
    pub cpu: HealthComponent,
    pub battery: Option<HealthComponent>,
    pub performance: HealthComponent,
}

pub struct HealthInput<'a> {
    pub volume: &'a VolumeSnapshot,
    pub memory: &'a MemorySnapshot,
    /// Mean CPU usage over the last minute, 0–100.
    pub cpu_average: f64,
    pub load_per_core: f64,
    pub thermal_state: &'a str,
    pub battery: Option<&'a BatterySnapshot>,
}

fn status_for(score: f64) -> &'static str {
    match score {
        s if s >= 85.0 => "Excellent",
        s if s >= 70.0 => "Good",
        s if s >= 50.0 => "Fair",
        s if s >= 30.0 => "Attention",
        _ => "Critical",
    }
}

fn component(score: f64, detail: String) -> HealthComponent {
    let score = score.clamp(0.0, 100.0).round();
    HealthComponent { score, status: status_for(score).to_owned(), detail }
}

/// Piecewise-linear mapping through ascending (input, score) knots.
fn lerp(value: f64, knots: &[(f64, f64)]) -> f64 {
    let first = knots[0];
    if value <= first.0 {
        return first.1;
    }
    for pair in knots.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if value <= x1 {
            return y0 + (value - x0) / (x1 - x0) * (y1 - y0);
        }
    }
    knots[knots.len() - 1].1
}

fn storage(volume: &VolumeSnapshot) -> HealthComponent {
    if volume.total == 0 {
        return component(50.0, "Disk capacity could not be read.".into());
    }
    let free = volume.available as f64 / volume.total as f64 * 100.0;
    let score = lerp(free, &[(2.0, 5.0), (5.0, 30.0), (10.0, 60.0), (15.0, 80.0), (25.0, 100.0)]);
    component(score, format!("{free:.0}% of the disk is available."))
}

fn memory(memory: &MemorySnapshot) -> HealthComponent {
    let base: f64 = match memory.pressure.as_str() {
        "critical" => 20.0,
        "warning" => 55.0,
        _ => 100.0,
    };
    let swap_ratio = if memory.total > 0 {
        memory.swap_used as f64 / memory.total as f64
    } else {
        0.0
    };
    let score = base - lerp(swap_ratio, &[(0.1, 0.0), (0.5, 15.0), (1.0, 30.0)]);
    let detail = match memory.pressure.as_str() {
        "unknown" => "Memory pressure could not be read.".to_owned(),
        level => format!("Memory pressure is {level}."),
    };
    component(score, detail)
}

fn cpu(average: f64) -> HealthComponent {
    let score = lerp(average, &[(50.0, 100.0), (75.0, 80.0), (90.0, 55.0), (100.0, 35.0)]);
    component(score, format!("Averaging {average:.0}% over the last minute."))
}

fn battery(b: &BatterySnapshot) -> Option<HealthComponent> {
    let capacity = b.max_capacity_percent?;
    let condition_ok = b.condition.as_deref().is_none_or(|c| {
        matches!(c.to_ascii_lowercase().as_str(), "good" | "normal")
    });
    let mut score = lerp(capacity, &[(60.0, 15.0), (80.0, 60.0), (90.0, 85.0), (97.0, 100.0)]);
    if !condition_ok {
        score = score.min(45.0);
    }
    Some(component(score, format!("Maximum capacity is {capacity:.0}% of design.")))
}

fn performance(load_per_core: f64, thermal_state: &str) -> HealthComponent {
    let thermal_cap: f64 = match thermal_state {
        "critical" => 15.0,
        "serious" => 45.0,
        "fair" => 80.0,
        _ => 100.0,
    };
    let score = lerp(load_per_core, &[(0.8, 100.0), (1.5, 80.0), (3.0, 45.0)]).min(thermal_cap);
    component(
        score,
        format!("Thermal state is {thermal_state}; load is {load_per_core:.2} per core."),
    )
}

pub fn evaluate(input: &HealthInput<'_>) -> HealthReport {
    let storage = storage(input.volume);
    let memory = memory(input.memory);
    let cpu = cpu(input.cpu_average);
    let battery = input.battery.and_then(battery);
    let performance = performance(input.load_per_core, input.thermal_state);

    // A Mac without a battery is scored on the remaining signals, not penalized.
    let mut weighted = vec![
        (&storage, 0.30),
        (&memory, 0.25),
        (&cpu, 0.15),
        (&performance, 0.15),
    ];
    if let Some(b) = &battery {
        weighted.push((b, 0.15));
    }
    let weight: f64 = weighted.iter().map(|(_, w)| w).sum();
    let mut overall = weighted.iter().map(|(c, w)| c.score * w).sum::<f64>() / weight;
    // One failing subsystem must not be hidden by four healthy ones.
    let worst = weighted.iter().map(|(c, _)| c.score).fold(100.0, f64::min);
    overall = overall.min(worst + 35.0).round();

    HealthReport {
        overall,
        status: status_for(overall).to_owned(),
        storage,
        memory,
        cpu,
        battery,
        performance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(free_pct: u64) -> VolumeSnapshot {
        VolumeSnapshot {
            mount_point: "/".into(),
            total: 100,
            available: free_pct,
            used: 100 - free_pct,
        }
    }

    fn mem(pressure: &str) -> MemorySnapshot {
        MemorySnapshot { total: 100, pressure: pressure.into(), ..Default::default() }
    }

    fn input<'a>(
        volume: &'a VolumeSnapshot,
        memory: &'a MemorySnapshot,
        battery: Option<&'a BatterySnapshot>,
    ) -> HealthInput<'a> {
        HealthInput {
            volume,
            memory,
            cpu_average: 10.0,
            load_per_core: 0.2,
            thermal_state: "nominal",
            battery,
        }
    }

    #[test]
    fn healthy_mac_scores_near_the_top() {
        let (v, m) = (volume(50), mem("normal"));
        let r = evaluate(&input(&v, &m, None));
        assert_eq!(r.overall, 100.0);
        assert_eq!(r.status, "Excellent");
    }

    #[test]
    fn a_missing_battery_is_not_penalized() {
        let (v, m) = (volume(50), mem("normal"));
        assert!(evaluate(&input(&v, &m, None)).battery.is_none());
        assert_eq!(evaluate(&input(&v, &m, None)).overall, 100.0);
    }

    #[test]
    fn a_nearly_full_disk_drags_the_overall_score_down() {
        let (v, m) = (volume(3), mem("normal"));
        let r = evaluate(&input(&v, &m, None));
        assert!(r.storage.score < 25.0);
        assert!(r.overall <= r.storage.score + 35.0);
    }

    #[test]
    fn memory_pressure_is_what_counts_not_cached_memory() {
        let v = volume(50);
        let mut m = mem("normal");
        m.cached = 90;
        assert_eq!(memory(&m).score, 100.0);
        assert!(evaluate(&input(&v, &mem("critical"), None)).memory.score <= 20.0);
    }

    #[test]
    fn thermal_pressure_caps_performance() {
        assert!(performance(0.1, "serious").score <= 45.0);
        assert_eq!(performance(0.1, "nominal").score, 100.0);
    }

    #[test]
    fn an_unhealthy_battery_condition_is_capped() {
        let b = BatterySnapshot {
            max_capacity_percent: Some(99.0),
            condition: Some("Service Recommended".into()),
            ..Default::default()
        };
        assert!(battery(&b).unwrap().score <= 45.0);
    }

    #[test]
    fn unknown_capacity_means_no_score_rather_than_a_guess() {
        assert!(battery(&BatterySnapshot::default()).is_none());
    }

    #[test]
    fn lerp_interpolates_and_clamps() {
        let knots = [(0.0, 0.0), (10.0, 100.0)];
        assert_eq!(lerp(-5.0, &knots), 0.0);
        assert_eq!(lerp(5.0, &knots), 50.0);
        assert_eq!(lerp(50.0, &knots), 100.0);
    }
}

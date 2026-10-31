use std::fmt::Write as _;

use serde::Serialize;

use crate::advisor::{gb, AdvisorReport};
use crate::db::{CleanupRunRow, ScanRow};
use crate::system::overview::SystemOverview;
use crate::system::telemetry::TelemetrySample;
use crate::util::now_ms;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthExport {
    pub generated_at: i64,
    pub disclaimer: &'static str,
    pub overview: Option<SystemOverview>,
    pub sample: Option<TelemetrySample>,
    pub advisor: AdvisorReport,
    pub scans: Vec<ScanRow>,
    pub cleanups: Vec<CleanupRunRow>,
}

pub const DISCLAIMER: &str =
    "The health score is NEXUS's own heuristic. It is not an official Apple diagnostic.";

impl HealthExport {
    pub fn new(
        overview: Option<SystemOverview>,
        sample: Option<TelemetrySample>,
        advisor: AdvisorReport,
        scans: Vec<ScanRow>,
        cleanups: Vec<CleanupRunRow>,
    ) -> Self {
        Self { generated_at: now_ms(), disclaimer: DISCLAIMER, overview, sample, advisor, scans, cleanups }
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# NEXUS System Health Report\n\n");
        let date = chrono::DateTime::from_timestamp_millis(self.generated_at)
            .map_or_else(String::new, |d| d.format("%Y-%m-%d %H:%M UTC").to_string());
        let _ = writeln!(out, "Generated {date}\n\n> {}\n", self.disclaimer);

        if let Some(o) = &self.overview {
            out.push_str("## System\n\n");
            let unknown = || "Unavailable".to_owned();
            let _ = writeln!(out, "- Model: {}", o.model_name.clone().unwrap_or_else(unknown));
            let _ = writeln!(out, "- Chip: {}", o.chip.clone().unwrap_or_else(unknown));
            let _ = writeln!(
                out,
                "- OS: {} {}",
                o.os_name,
                o.os_version.clone().unwrap_or_else(unknown)
            );
            let _ = writeln!(out, "- Memory: {}\n", gb(o.memory_total));
        }

        if let Some(s) = &self.sample {
            let h = &s.health;
            out.push_str("## Health\n\n| Area | Score | Status |\n| --- | --- | --- |\n");
            let _ = writeln!(out, "| Overall | {:.0} | {} |", h.overall, h.status);
            let _ = writeln!(out, "| Storage | {:.0} | {} |", h.storage.score, h.storage.status);
            let _ = writeln!(out, "| Memory | {:.0} | {} |", h.memory.score, h.memory.status);
            let _ = writeln!(out, "| CPU | {:.0} | {} |", h.cpu.score, h.cpu.status);
            match &h.battery {
                Some(b) => {
                    let _ = writeln!(out, "| Battery | {:.0} | {} |", b.score, b.status);
                }
                None => out.push_str("| Battery | Unavailable | Unavailable |\n"),
            }
            let _ = writeln!(out, "| Performance | {:.0} | {} |\n", h.performance.score, h.performance.status);
            let _ = writeln!(
                out,
                "Storage: {} used, {} available of {}.\n",
                gb(s.volume.used),
                gb(s.volume.available),
                gb(s.volume.total)
            );
        }

        out.push_str("## Recommendations\n\n");
        if self.advisor.recommendations.is_empty() {
            out.push_str("None at this time.\n\n");
        }
        for rec in &self.advisor.recommendations {
            let _ = writeln!(out, "### {}\n\n{}\n\n- Why it matters: {}\n- Impact: {}\n- Estimate: {}\n- Risk: {:?}\n", rec.title, rec.issue, rec.why, rec.impact, rec.estimate, rec.risk);
        }

        if !self.cleanups.is_empty() {
            out.push_str("## Cleanup history\n\n| When | Source | Freed | To Trash | Dry run |\n| --- | --- | --- | --- | --- |\n");
            for run in self.cleanups.iter().take(25) {
                let when = chrono::DateTime::from_timestamp_millis(run.finished_at)
                    .map_or_else(String::new, |d| d.format("%Y-%m-%d").to_string());
                let _ = writeln!(
                    out,
                    "| {when} | {} | {} | {} | {} |",
                    run.source,
                    gb(run.bytes_freed),
                    gb(run.bytes_trashed),
                    if run.dry_run { "yes" } else { "no" }
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advisor() -> AdvisorReport {
        AdvisorReport {
            generated_at: 0,
            engine: "Local rules".into(),
            summary: String::new(),
            recommendations: Vec::new(),
            missing_data: Vec::new(),
        }
    }

    #[test]
    fn report_states_that_the_score_is_a_heuristic() {
        let md = HealthExport::new(None, None, advisor(), Vec::new(), Vec::new()).to_markdown();
        assert!(md.contains("not an official Apple diagnostic"));
    }

    #[test]
    fn report_serializes_to_json_without_data() {
        let export = HealthExport::new(None, None, advisor(), Vec::new(), Vec::new());
        let value = serde_json::to_value(&export).unwrap();
        assert!(value["overview"].is_null());
    }
}

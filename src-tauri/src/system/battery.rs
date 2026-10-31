use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use super::iokit;
use crate::util::run_with_timeout;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatterySnapshot {
    pub percent: f64,
    pub is_charging: bool,
    pub external_power: bool,
    pub fully_charged: bool,
    pub power_source: String,
    pub cycle_count: Option<u32>,
    pub design_capacity_mah: Option<u32>,
    pub max_capacity_mah: Option<u32>,
    pub max_capacity_percent: Option<f64>,
    pub condition: Option<String>,
    pub temperature_c: Option<f64>,
    pub voltage_v: Option<f64>,
    pub power_w: Option<f64>,
    pub time_remaining_min: Option<u32>,
}

/// Apple's own condition and maximum-capacity figures, which IOKit does not expose.
#[derive(Debug, Clone, Default)]
pub struct AppleBatteryProfile {
    pub condition: Option<String>,
    pub max_capacity_percent: Option<f64>,
}

pub fn read(profile: &AppleBatteryProfile) -> Option<BatterySnapshot> {
    let services = iokit::services("AppleSmartBattery");
    let props = services.first()?;
    if props.bool("BatteryInstalled") == Some(false) {
        return None;
    }
    let current = props.i64("CurrentCapacity")? as f64;
    let max = props.i64("MaxCapacity").filter(|v| *v > 0)? as f64;
    let external = props.bool("ExternalConnected").unwrap_or(false);
    // Newer macOS moved most values into BatteryData, and some onto the pack child node.
    let data = props.dict("BatteryData");
    let pack = iokit::services("AppleSmartBatteryPack");
    let pack_data = pack.first().and_then(|p| p.dict("BatteryData"));
    let lookup = |key: &str| {
        props
            .i64(key)
            .or_else(|| data.as_ref().and_then(|d| d.i64(key)))
            .or_else(|| pack_data.as_ref().and_then(|d| d.i64(key)))
    };
    let design = lookup("DesignCapacity").filter(|v| *v > 0);
    let raw_max = lookup("NominalChargeCapacity")
        .or_else(|| lookup("AppleRawMaxCapacity"))
        .filter(|v| *v > 0);
    let voltage = props.i64("Voltage").map(|v| v as f64 / 1000.0);
    let amperage = props.i64("InstantAmperage").or_else(|| props.i64("Amperage"));
    let derived_percent = match (raw_max, design) {
        (Some(r), Some(d)) => Some((r as f64 / d as f64 * 100.0).min(100.0)),
        _ => None,
    };

    Some(BatterySnapshot {
        percent: (current / max * 100.0).clamp(0.0, 100.0),
        is_charging: props.bool("IsCharging").unwrap_or(false),
        external_power: external,
        fully_charged: props.bool("FullyCharged").unwrap_or(false),
        power_source: if external { "Power adapter" } else { "Battery" }.to_owned(),
        cycle_count: props.i64("CycleCount").map(|v| v as u32),
        design_capacity_mah: design.map(|v| v as u32),
        max_capacity_mah: raw_max.map(|v| v as u32),
        max_capacity_percent: profile.max_capacity_percent.or(derived_percent),
        condition: profile.condition.clone(),
        temperature_c: lookup("Temperature")
            .or_else(|| lookup("VirtualTemperature"))
            .filter(|v| *v > 0)
            .map(|v| v as f64 / 100.0),
        voltage_v: voltage,
        power_w: match (voltage, amperage) {
            (Some(v), Some(a)) => Some(v * a as f64 / 1000.0),
            _ => None,
        },
        time_remaining_min: props
            .i64("TimeRemaining")
            .filter(|v| *v > 0 && *v < 65_535 && !external)
            .map(|v| v as u32),
    })
}

/// Slow (it shells out to system_profiler), so callers cache the result.
pub fn apple_profile() -> AppleBatteryProfile {
    let mut command = Command::new("/usr/sbin/system_profiler");
    command.args(["-json", "SPPowerDataType"]);
    run_with_timeout(command, Duration::from_secs(15))
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .map(|v| parse_profile(&v))
        .unwrap_or_default()
}

fn parse_profile(value: &serde_json::Value) -> AppleBatteryProfile {
    let info = value["SPPowerDataType"]
        .as_array()
        .and_then(|items| items.iter().find_map(|i| i.get("sppower_battery_health_info")));
    let Some(info) = info else {
        return AppleBatteryProfile::default();
    };
    AppleBatteryProfile {
        condition: info["sppower_battery_health"].as_str().map(str::to_owned),
        max_capacity_percent: info["sppower_battery_health_maximum_capacity"]
            .as_str()
            .and_then(|s| s.trim_end_matches('%').trim().parse().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_profiler_power_json() {
        let v = serde_json::json!({"SPPowerDataType": [
            {"_name": "spbattery_information", "sppower_battery_health_info": {
                "sppower_battery_cycle_count": 212,
                "sppower_battery_health": "Good",
                "sppower_battery_health_maximum_capacity": "91%"
            }}
        ]});
        let p = parse_profile(&v);
        assert_eq!(p.condition.as_deref(), Some("Good"));
        assert_eq!(p.max_capacity_percent, Some(91.0));
    }

    #[test]
    fn desktop_macs_have_no_profile() {
        let v = serde_json::json!({"SPPowerDataType": [{"_name": "sppower_ac_charger_information"}]});
        let p = parse_profile(&v);
        assert!(p.condition.is_none() && p.max_capacity_percent.is_none());
    }
}

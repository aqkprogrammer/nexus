//! Docker cleanup always goes through Docker's own CLI, so the daemon stays consistent.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;

use crate::error::{NexusError, Result};
use crate::util::{home_dir, run_with_timeout};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerResource {
    pub id: String,
    pub label: String,
    pub total: String,
    pub active: String,
    pub size: u64,
    pub reclaimable: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerStatus {
    pub installed: bool,
    pub running: bool,
    pub message: String,
    pub resources: Vec<DockerResource>,
}

fn binary() -> Option<PathBuf> {
    // Prune acts on a whole daemon, so a test run may only ever talk to its stand-in.
    #[cfg(debug_assertions)]
    if std::env::var_os("NEXUS_E2E_SCRIPT").is_some() {
        return std::env::var_os("NEXUS_E2E_DOCKER").map(PathBuf::from);
    }
    // Apps launched from Finder do not inherit the shell's PATH.
    [
        PathBuf::from("/usr/local/bin/docker"),
        PathBuf::from("/opt/homebrew/bin/docker"),
        home_dir().join(".docker/bin/docker"),
        home_dir().join(".orbstack/bin/docker"),
        PathBuf::from("/Applications/Docker.app/Contents/Resources/bin/docker"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

/// Parses Docker's human sizes such as "1.25GB" or "512.3kB". Docker uses base 1000.
pub fn parse_size(text: &str) -> u64 {
    let text = text.trim();
    let split = text.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number.trim().parse().unwrap_or(0.0);
    let scale = match unit.trim().to_ascii_lowercase().as_str() {
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        _ => 1.0,
    };
    (value * scale) as u64
}

fn resource_id(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "Images" => Some(("images", "Unused images")),
        "Containers" => Some(("containers", "Stopped containers")),
        "Local Volumes" => Some(("volumes", "Unused volumes")),
        "Build Cache" => Some(("builder", "Build cache")),
        _ => None,
    }
}

pub fn parse_df(output: &str) -> Vec<DockerResource> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|row| {
            let (id, label) = resource_id(row["Type"].as_str()?)?;
            let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
            let number = |key: &str| match &row[key] {
                serde_json::Value::Number(n) => n.to_string(),
                other => other.as_str().unwrap_or_default().to_owned(),
            };
            Some(DockerResource {
                id: id.to_owned(),
                label: label.to_owned(),
                total: number("TotalCount"),
                active: number("Active"),
                size: parse_size(&text("Size")),
                reclaimable: parse_size(text("Reclaimable").split('(').next().unwrap_or("")),
            })
        })
        .collect()
}

pub fn status() -> DockerStatus {
    let Some(docker) = binary() else {
        return DockerStatus { message: "Docker is not installed.".into(), ..Default::default() };
    };
    let mut command = Command::new(docker);
    command.args(["system", "df", "--format", "{{json .}}"]);
    match run_with_timeout(command, Duration::from_secs(12)) {
        Some(output) if output.status.success() => DockerStatus {
            installed: true,
            running: true,
            message: String::new(),
            resources: parse_df(&String::from_utf8_lossy(&output.stdout)),
        },
        _ => DockerStatus {
            installed: true,
            running: false,
            message: "Docker is installed but its daemon is not running. Start Docker to preview reclaimable space.".into(),
            resources: Vec::new(),
        },
    }
}

/// Runs one prune. `id` comes from `DockerResource::id`.
pub fn prune(id: &str) -> Result<()> {
    let args: &[&str] = match id {
        "builder" => &["builder", "prune", "-f"],
        "images" => &["image", "prune", "-f"],
        "containers" => &["container", "prune", "-f"],
        "volumes" => &["volume", "prune", "-f"],
        _ => return Err(NexusError::Invalid(format!("unknown Docker resource {id}"))),
    };
    let docker = binary().ok_or_else(|| NexusError::Unavailable("Docker is not installed.".into()))?;
    let mut command = Command::new(docker);
    command.args(args);
    match run_with_timeout(command, Duration::from_secs(300)) {
        Some(output) if output.status.success() => Ok(()),
        Some(output) => Err(NexusError::Unavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        )),
        None => Err(NexusError::Unavailable("Docker did not finish in time.".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_sizes() {
        assert_eq!(parse_size("1.5GB"), 1_500_000_000);
        assert_eq!(parse_size("512.5kB"), 512_500);
        assert_eq!(parse_size("0B"), 0);
        assert_eq!(parse_size(""), 0);
    }

    #[test]
    fn parses_system_df_lines() {
        let out = concat!(
            r#"{"Active":"2","Reclaimable":"1.2GB (40%)","Size":"3GB","TotalCount":"7","Type":"Images"}"#,
            "\n",
            r#"{"Active":"0","Reclaimable":"500MB","Size":"500MB","TotalCount":"12","Type":"Build Cache"}"#,
            "\n",
            "not json\n"
        );
        let resources = parse_df(out);
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].id, "images");
        assert_eq!(resources[0].reclaimable, 1_200_000_000);
        assert_eq!(resources[1].id, "builder");
        assert_eq!(resources[1].size, 500_000_000);
    }

    #[test]
    fn prune_rejects_unknown_scopes() {
        assert!(prune("system --all").is_err());
    }
}

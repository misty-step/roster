use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use roster_core::ResolvedAgent;
use serde::Serialize;
use std::{env, fs, path::PathBuf};

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub schema_version: String,
    pub started_at: String,
    pub finished_at: String,
    pub agent: String,
    pub role: String,
    pub harness: String,
    pub model: String,
    pub workspace: PathBuf,
    pub config: PathBuf,
    pub bundle: Option<PathBuf>,
    pub exit_code: Option<i32>,
}

pub fn state_root() -> Result<PathBuf> {
    if let Some(path) = env::var_os("ROSTER_STATE_DIR") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/state/roster"))
}

pub fn record(
    agent: &ResolvedAgent,
    workspace: PathBuf,
    bundle: Option<PathBuf>,
    started_at: DateTime<Utc>,
    exit_code: Option<i32>,
) -> Result<PathBuf> {
    let receipt = Receipt {
        schema_version: "roster.receipt.v1".to_owned(),
        started_at: started_at.to_rfc3339(),
        finished_at: Utc::now().to_rfc3339(),
        agent: agent.name.clone(),
        role: agent.role.clone(),
        harness: agent.harness.to_string(),
        model: agent.model.clone(),
        workspace,
        config: agent.config_path.clone(),
        bundle,
        exit_code,
    };
    let root = state_root()?.join("receipts");
    fs::create_dir_all(&root)?;
    let path = root.join(format!(
        "{}-{}-{}.yaml",
        started_at.format("%Y%m%dT%H%M%S%.3fZ"),
        std::process::id(),
        receipt.agent
    ));
    fs::write(&path, serde_yaml::to_string(&receipt)?)?;
    Ok(path)
}

pub fn print_recent(limit: usize) -> Result<()> {
    let directory = state_root()?.join("receipts");
    if !directory.is_dir() {
        return Ok(());
    }
    let mut paths = fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    println!("receipts:");
    for path in paths.iter().rev().take(limit) {
        println!("  - {}", path.display());
    }
    Ok(())
}

#[derive(Serialize)]
struct AuthorityReceipt<'a> {
    schema_version: &'static str,
    requested_at: String,
    agent: &'a str,
    capability: &'a str,
    provider: &'a str,
    exit_code: Option<i32>,
}

pub fn record_authority(
    agent: &str,
    capability: &str,
    provider: &str,
    exit_code: Option<i32>,
) -> Result<PathBuf> {
    let requested_at = Utc::now();
    let receipt = AuthorityReceipt {
        schema_version: "roster.authority_receipt.v1",
        requested_at: requested_at.to_rfc3339(),
        agent,
        capability,
        provider,
        exit_code,
    };
    let directory = state_root()?.join("authority");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "{}-{}-{}.yaml",
        requested_at.format("%Y%m%dT%H%M%S%.3fZ"),
        std::process::id(),
        agent
    ));
    fs::write(&path, serde_yaml::to_string(&receipt)?)?;
    Ok(path)
}

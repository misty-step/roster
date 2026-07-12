mod check;
mod doctor;
mod sync;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use roster_core::{
    Models, PowderCardSnapshot, PowderClaim, Roster, render_bb_agent, render_brief,
    render_claude_agent, render_codex_agent, render_omp_agent, render_show,
};
use serde_json::Value;
use std::path::PathBuf;
use sync::Catalog;

#[derive(Debug, Parser)]
#[command(name = "roster")]
#[command(about = "Materialize roster agent declarations for harnesses")]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    List,
    Show {
        agent: String,
    },
    Materialize {
        agent: String,
        #[arg(long, value_enum)]
        harness: Harness,
    },
    Brief {
        agent: String,
        #[arg(long)]
        card: Option<String>,
        #[arg(long = "add-skill")]
        add_skills: Vec<String>,
        #[arg(long = "add-mcp")]
        add_mcps: Vec<String>,
    },
    Sync {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long)]
        disable: bool,
        #[arg(long, value_enum, default_value = "full")]
        catalog: Catalog,
        #[arg(long = "all-agents")]
        all_agents: bool,
    },
    /// Gate the primitives catalog: frontmatter shape, referenced-path
    /// existence, skills-index/disk parity, conflict markers.
    Check,
    /// Inspect Tier 1 harness projections without changing the machine.
    Doctor {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Run bounded external runtime/MCP/QMD/process/disk probes.
        #[arg(long)]
        live: bool,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum Harness {
    Claude,
    Codex,
    Bb,
    Omp,
}

fn main() -> Result<()> {
    roster_canary::init("roster", "roster");
    roster_canary::init_tracing();
    roster_canary::install_panic_hook();

    let cli = Cli::parse();
    roster_canary::check_in();

    let result = run(cli);
    if let Err(error) = &result {
        roster_canary::report_error("roster.run.failed", &format!("{error:?}"));
    }
    // ONE unconditional flush before `main` returns, on both the success and
    // error paths. `roster` is a short-lived CLI: `report_error`/`check_in`
    // spawn the send off the hot path, so without a flush the process exits
    // (and, on `Err`, prints the error and exits 1) before the send reaches
    // the network -- the exact short-lived-CLI race where a handled error is
    // reported in code but never lands at the hub. `flush` joins every
    // in-flight send (check-in + error), each bounded by the reporter's
    // per-attempt timeout, so the send completes before exit.
    roster_canary::flush();
    result
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::List => {
            let roster = Roster::load(&cli.root)?;
            for agent in roster.agents() {
                println!(
                    "{}\t{}\t{}\t{}",
                    agent.role.name,
                    agent.role.model_policy.preferred.model,
                    agent.role.model_policy.preferred.reasoning,
                    agent.role.description
                );
            }
        }
        Command::Show { agent } => {
            let roster = Roster::load(&cli.root)?;
            let agent = find_agent(&roster, &agent)?;
            print!("{}", render_show(agent));
        }
        Command::Materialize { agent, harness } => {
            let roster = Roster::load(&cli.root)?;
            let agent = find_agent(&roster, &agent)?;
            match harness {
                Harness::Claude => {
                    let models = Models::load(&cli.root)?;
                    print!(
                        "{}",
                        render_claude_agent(agent, &models).map_err(|error| anyhow!(error))?
                    );
                }
                Harness::Codex => print!("{}", render_codex_agent(agent)),
                Harness::Bb => {
                    let models = Models::load(&cli.root)?;
                    print!(
                        "{}",
                        render_bb_agent(agent, &models).map_err(|error| anyhow!(error))?
                    );
                }
                Harness::Omp => print!(
                    "{}",
                    render_omp_agent(agent).map_err(|error| anyhow!(error))?
                ),
            }
        }
        Command::Brief {
            agent,
            card,
            add_skills,
            add_mcps,
        } => {
            let roster = Roster::load(&cli.root)?;
            let agent = find_agent(&roster, &agent)?;
            let card = card
                .as_deref()
                .map(fetch_powder_card)
                .transpose()
                .context("failed to fetch Powder card")?;
            print!(
                "{}",
                render_brief(agent, &add_skills, &add_mcps, card.as_ref())
            );
        }
        Command::Sync {
            home,
            disable,
            catalog,
            all_agents,
        } => sync::run(&cli.root, home, disable, catalog, all_agents)?,
        Command::Check => {
            if !check::run(&cli.root)? {
                // `process::exit` never returns to `main`'s post-`run` report
                // path, so this arm reports+flushes itself before exiting --
                // otherwise a real `roster check` failure would go silently
                // unreported to Canary.
                roster_canary::report_error(
                    "roster.check.failed",
                    "roster check reported violations",
                );
                roster_canary::flush();
                std::process::exit(1);
            }
        }
        Command::Doctor { home, json, live } => {
            if !doctor::run(&cli.root, home, json, live)? {
                // Like `check`, doctor emits its machine/human report before
                // returning the failure status. Flush telemetry explicitly
                // because process::exit bypasses main's post-run path.
                roster_canary::report_error(
                    "roster.doctor.failed",
                    "roster doctor reported failures",
                );
                roster_canary::flush();
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

pub(crate) fn find_agent<'a>(roster: &'a Roster, name: &str) -> Result<&'a roster_core::Agent> {
    roster
        .agent(name)
        .ok_or_else(|| anyhow!("unknown agent {name:?}"))
}

fn fetch_powder_card(id: &str) -> Result<PowderCardSnapshot> {
    let base_url = std::env::var("POWDER_API_BASE_URL")
        .context("POWDER_API_BASE_URL is required for --card")?;
    let api_key =
        std::env::var("POWDER_API_KEY").context("POWDER_API_KEY is required for --card")?;
    let url = format!("{}/api/v1/cards/{}", base_url.trim_end_matches('/'), id);
    let response: Value = ureq::get(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call()
        .with_context(|| format!("GET {url}"))?
        .into_json()
        .with_context(|| format!("decode Powder response for {id}"))?;

    let card = response.get("card").unwrap_or(&response);
    let title = card
        .get("title")
        .and_then(Value::as_str)
        .context("card.title is required")?
        .to_string();
    let body = card
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let acceptance = card
        .get("acceptance")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .or_else(|| {
            card.get("criteria").and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("text").and_then(Value::as_str))
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    let status = card
        .get("status")
        .and_then(Value::as_str)
        .context("card.status is required")?
        .to_string();
    let updated_at = card
        .get("updated_at")
        .and_then(Value::as_i64)
        .context("card.updated_at is required")?;
    let claim = match card.get("claim") {
        None | Some(Value::Null) => None,
        Some(claim) => Some(PowderClaim {
            agent: claim
                .get("agent")
                .and_then(Value::as_str)
                .context("card.claim.agent is required when claim is present")?
                .to_string(),
            run_id: claim
                .get("run_id")
                .and_then(Value::as_str)
                .context("card.claim.run_id is required when claim is present")?
                .to_string(),
            expires_at: claim
                .get("expires_at")
                .and_then(Value::as_i64)
                .context("card.claim.expires_at is required when claim is present")?,
        }),
    };

    Ok(PowderCardSnapshot {
        id: id.to_string(),
        title,
        body,
        acceptance,
        status,
        updated_at,
        fetched_at: chrono::Utc::now().timestamp(),
        claim,
    })
}

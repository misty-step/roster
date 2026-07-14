mod adapter;
mod check;
mod picker;
mod process;
mod receipt;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use roster_core::{Harness, ResolvedAgent, Roster, RosterError};
use std::{env, fs, io::IsTerminal, path::PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "roster",
    version,
    about = "Compose and launch exact agent environments"
)]
struct Cli {
    /// Use one explicit config instead of nearest-config discovery.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Workspace used for config discovery and as the launched agent's cwd.
    #[arg(long, global = true)]
    cwd: Option<PathBuf>,
    /// Explicit Roster library checkout for the catalog gate.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a deliberately small local configuration.
    Init {
        #[arg(long)]
        source: PathBuf,
    },
    /// List launchable agent definitions in the effective config.
    List,
    /// Show one agent definition and its exact resolved composition.
    Show { agent: String },
    /// Resolve one agent into an immutable bundle without launching it.
    Resolve {
        #[command(flatten)]
        selection: AgentSelection,
        #[arg(long)]
        output: PathBuf,
    },
    /// Launch one named agent or one explicit ephemeral composition.
    Dispatch {
        #[command(flatten)]
        selection: AgentSelection,
        #[arg(long)]
        dry_run: bool,
        /// Retain the runtime bundle after the Harness exits.
        #[arg(long)]
        keep_bundle: bool,
    },
    /// Explain effective config, selected primitives, and recent receipts.
    Inspect { agent: Option<String> },
    /// Launch a raw, context-free Harness for repairing Roster itself.
    Rescue {
        harness: Harness,
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate the effective graph and the public primitive catalog.
    Check,
    /// Internal in-session authority adapter seam.
    #[command(hide = true)]
    Authority {
        #[command(subcommand)]
        command: AuthorityCommand,
    },
}

#[derive(Debug, Args)]
struct AgentSelection {
    /// Launch this named agent definition and its complete role.
    #[arg(
        value_name = "AGENT",
        required_unless_present_any = ["using", "default_harness"],
        conflicts_with_all = ["using", "default_harness"]
    )]
    agent: Option<String>,
    /// Launch the effective config's explicitly declared default for this Harness.
    #[arg(
        long = "default",
        value_name = "HARNESS",
        conflicts_with_all = ["agent", "using"]
    )]
    default_harness: Option<Harness>,
    /// Borrow only this agent's Harness, model, reasoning, and native arguments.
    #[arg(
        long,
        value_name = "AGENT",
        required_unless_present_any = ["agent", "default_harness"],
        conflicts_with_all = ["agent", "default_harness"],
        requires_all = ["as_name", "purpose", "include"]
    )]
    using: Option<String>,
    /// Runtime name for an ephemeral role.
    #[arg(long = "as", value_name = "NAME", requires = "using")]
    as_name: Option<String>,
    /// Concise purpose injected as the ephemeral role description.
    #[arg(long, value_name = "TEXT", requires = "using")]
    purpose: Option<String>,
    /// Exact primitive or pack to include; repeat for the complete composition.
    #[arg(long, value_name = "IDENTITY", requires = "using")]
    include: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum AuthorityCommand {
    /// Ask the configured provider for one narrowly named capability.
    Request { capability: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let cwd = cli
        .cwd
        .unwrap_or(env::current_dir().context("read current directory")?);
    match cli.command {
        None => {
            let roster = load(&cli.config, &cwd)?;
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                print_agents(&roster);
                return Ok(());
            }
            if let Some(agent) = picker::pick(&roster)? {
                adapter::dispatch(&roster.resolve(&agent)?, &cwd, false, false)?;
            }
        }
        Some(Command::Init { source }) => init(&cwd, &source)?,
        Some(Command::List) => print_agents(&load(&cli.config, &cwd)?),
        Some(Command::Show { agent }) => {
            let roster = load(&cli.config, &cwd)?;
            print_resolved(&roster.resolve(&agent)?);
        }
        Some(Command::Resolve { selection, output }) => {
            let roster = load_selection(&cli.config, &cwd, &selection)?;
            let manifest = resolve_selection(&roster, &selection)?.write_bundle(&output, &cwd)?;
            print!("{}", serde_yaml::to_string(&manifest)?);
        }
        Some(Command::Dispatch {
            selection,
            dry_run,
            keep_bundle,
        }) => {
            let roster = load_selection(&cli.config, &cwd, &selection)?;
            adapter::dispatch(
                &resolve_selection(&roster, &selection)?,
                &cwd,
                dry_run,
                keep_bundle,
            )?;
        }
        Some(Command::Inspect { agent }) => {
            let roster = load(&cli.config, &cwd)?;
            println!("config: {}", roster.config_path().display());
            if let Some(agent) = agent {
                print_resolved(&roster.resolve(&agent)?);
            } else {
                print_agents(&roster);
                receipt::print_recent(10)?;
            }
        }
        Some(Command::Rescue { harness, dry_run }) => adapter::rescue(harness, &cwd, dry_run)?,
        Some(Command::Check) => {
            let roster = load_for_check(&cli.config, &cwd)?;
            if let Some(roster) = &roster {
                for name in roster.agents().keys() {
                    roster
                        .resolve(name)
                        .with_context(|| format!("resolve agent {name:?}"))?;
                }
            }
            let roots = match cli.root {
                Some(root) => vec![root],
                None => match &roster {
                    Some(roster) => roster
                        .source_roots()
                        .filter(|root| root.join("primitives/skills/skills-index.yaml").is_file())
                        .map(PathBuf::from)
                        .collect(),
                    None if cwd.join("primitives/skills/skills-index.yaml").is_file() => {
                        vec![cwd.clone()]
                    }
                    None => Vec::new(),
                },
            };
            if roots.is_empty() {
                bail!("no source exposes the public primitive catalog; pass --root explicitly");
            }
            let mut checked = std::collections::BTreeSet::new();
            for root in roots {
                let root = root.canonicalize().with_context(|| {
                    format!("canonicalize primitive catalog root {}", root.display())
                })?;
                if checked.insert(root.clone()) && !check::run(&root)? {
                    bail!("primitive catalog check failed for {}", root.display());
                }
            }
            if let Some(roster) = roster {
                println!(
                    "roster graph: ok ({} agents from {})",
                    roster.agents().len(),
                    roster.config_path().display()
                );
            } else {
                println!("roster graph: skipped (no effective config)");
            }
        }
        Some(Command::Authority {
            command: AuthorityCommand::Request { capability },
        }) => request_authority(&cli.config, &cwd, &capability)?,
    }
    Ok(())
}

fn resolve_selection(roster: &Roster, selection: &AgentSelection) -> Result<ResolvedAgent> {
    if let Some(agent) = &selection.agent {
        return roster.resolve(agent).map_err(Into::into);
    }
    if let Some(harness) = selection.default_harness {
        return roster
            .resolve(roster.default_agent(harness)?)
            .map_err(Into::into);
    }
    roster
        .resolve_ad_hoc(
            selection.using.as_deref().context("missing --using")?,
            selection.as_name.as_deref().context("missing --as")?,
            selection.purpose.as_deref().context("missing --purpose")?,
            &selection.include,
        )
        .map_err(Into::into)
}

fn load(config: &Option<PathBuf>, cwd: &std::path::Path) -> Result<Roster> {
    let runtime_config = env::var_os("ROSTER_CONFIG").map(PathBuf::from);
    match config.as_ref().or(runtime_config.as_ref()) {
        Some(path) => Roster::load_config(path).map_err(Into::into),
        None => Roster::discover(cwd).map_err(Into::into),
    }
}

fn load_selection(
    config: &Option<PathBuf>,
    cwd: &std::path::Path,
    selection: &AgentSelection,
) -> Result<Roster> {
    if selection.default_harness.is_some() && config.is_none() {
        Roster::discover(cwd).map_err(Into::into)
    } else {
        load(config, cwd)
    }
}

fn load_for_check(config: &Option<PathBuf>, cwd: &std::path::Path) -> Result<Option<Roster>> {
    let runtime_config = env::var_os("ROSTER_CONFIG").map(PathBuf::from);
    if config.is_some() || runtime_config.is_some() {
        return load(config, cwd).map(Some);
    }
    match Roster::discover(cwd) {
        Ok(roster) => Ok(Some(roster)),
        Err(RosterError::ConfigNotFound { .. } | RosterError::HomeNotSet) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn request_authority(
    config: &Option<PathBuf>,
    cwd: &std::path::Path,
    capability: &str,
) -> Result<()> {
    if capability.trim().is_empty() || capability.starts_with('-') {
        bail!("capability must be a non-empty descriptive name");
    }
    let roster = load(config, cwd)?;
    let provider = roster
        .authority()
        .context("no authority provider is configured; operation denied")?;
    let agent = env::var("ROSTER_AGENT").unwrap_or_else(|_| "unknown".to_owned());
    let authority_env = [("ROSTER_AUTHORITY_AGENT".to_owned(), agent.clone())]
        .into_iter()
        .collect();
    let status = process::isolated(&provider.command, &authority_env)
        .args(&provider.args)
        .arg(capability)
        .status()
        .with_context(|| format!("invoke authority provider {}", provider.command))?;
    receipt::record_authority(
        &agent,
        capability,
        std::path::Path::new(&provider.command)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("provider"),
        status.code(),
    )?;
    if !status.success() {
        bail!("authority request denied or unavailable ({status})");
    }
    Ok(())
}

fn print_agents(roster: &Roster) {
    for (name, agent) in roster.agents() {
        match roster.resolve(name) {
            Ok(_) => println!(
                "{name}\t{}\t{}\t{}",
                agent.harness, agent.model, agent.description
            ),
            Err(error) => println!(
                "{name}\t{}\t{}\t{}\tDISABLED: {error}",
                agent.harness, agent.model, agent.description
            ),
        }
    }
}

fn print_resolved(agent: &ResolvedAgent) {
    println!("name: {}", agent.name);
    println!("binding: {}", agent.binding);
    println!("description: {}", agent.description);
    println!("role: {}", agent.role);
    println!("harness: {}", agent.harness);
    println!("model: {}", agent.model);
    if let Some(reasoning) = &agent.reasoning {
        println!("reasoning: {reasoning}");
    }
    println!("guidance:");
    for item in &agent.guidance {
        println!("  - {}", item.identity);
    }
    println!("skills:");
    for item in &agent.skills {
        println!("  - {}", item.identity);
    }
    println!("mcps:");
    for item in &agent.mcps {
        println!("  - {}", item.identity);
    }
}

fn init(cwd: &std::path::Path, source: &std::path::Path) -> Result<()> {
    let directory = cwd.join(".roster");
    let path = directory.join("config.yaml");
    if path.exists() {
        return Err(anyhow!("refusing to overwrite {}", path.display()));
    }
    fs::create_dir_all(&directory)?;
    let body = format!(
        "schema_version: roster.config.v1\ndefaults:\n  codex: amos\nsources:\n  core: {}\nagents:\n  amos:\n    description: Default Codex orchestrator\n    role: core/role:orchestrator\n    model: gpt-5.6\n    reasoning: high\n    harness: codex\n    args: []\n",
        source.display()
    );
    fs::write(&path, body)?;
    println!("created {}", path.display());
    Ok(())
}

use crate::receipt;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use roster_core::{Harness, ResolvedAgent, ResolvedMcp};
use serde_json::{Map, Value, json};
use signal_hook::iterator::Signals;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{Read, Write},
    os::unix::fs::symlink,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Invocation {
    env: BTreeMap<String, String>,
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
    bundle: Option<RunBundle>,
}

#[derive(Debug)]
struct RunBundle {
    path: PathBuf,
    keep: bool,
    cleaned: bool,
}

impl RunBundle {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            keep: false,
            cleaned: false,
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        if !self.keep && !self.cleaned && self.path.exists() {
            fs::remove_dir_all(&self.path)?;
        }
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for RunBundle {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub fn dispatch(
    agent: &ResolvedAgent,
    workspace: &Path,
    dry_run: bool,
    keep_bundle: bool,
) -> Result<()> {
    let mut invocation = prepare(agent, workspace)?;
    invocation.bundle.as_mut().expect("dispatch bundle").keep = keep_bundle;
    if dry_run {
        print_invocation(&invocation);
        return Ok(());
    }
    eprintln!("Preparing {} ({})…", agent.name, agent.harness);
    std::io::stderr().flush()?;
    preflight(agent, workspace, &invocation)?;
    eprintln!("Launching {} ({})…", agent.name, agent.harness);
    std::io::stderr().flush()?;
    let started_at = Utc::now();
    let status = run_invocation(&invocation)?;
    let path = receipt::record(
        agent,
        workspace.to_path_buf(),
        keep_bundle.then(|| {
            invocation
                .bundle
                .as_ref()
                .expect("dispatch bundle")
                .path
                .join("bundle")
        }),
        started_at,
        status.code(),
    )?;
    invocation
        .bundle
        .as_mut()
        .expect("dispatch bundle")
        .cleanup()?;
    eprintln!("roster receipt: {}", path.display());
    propagate_exit(status)
}

pub fn rescue(harness: Harness, workspace: &Path, dry_run: bool) -> Result<()> {
    let mut environment = BTreeMap::new();
    let args = match harness {
        Harness::Codex => {
            let home = receipt::state_root()?.join("rescue/codex");
            if home.exists() {
                fs::remove_dir_all(&home)?;
            }
            fs::create_dir_all(&home)?;
            bridge(&home, "auth.json", &real_home()?.join(".codex/auth.json"))?;
            environment.insert("CODEX_HOME".to_owned(), home.display().to_string());
            vec![
                "--disable".into(),
                "apps".into(),
                "-C".into(),
                workspace.display().to_string(),
            ]
        }
        Harness::Claude => vec!["--safe-mode".into()],
        Harness::Omp => {
            let home = receipt::state_root()?.join("rescue/omp/agent");
            if home.exists() {
                fs::remove_dir_all(&home)?;
            }
            fs::create_dir_all(&home)?;
            fs::write(home.join("mcp.json"), b"{\"mcpServers\":{}}\n")?;
            let isolation = home
                .parent()
                .expect("OMP rescue parent")
                .join("isolation.yml");
            fs::write(&isolation, omp_isolation())?;
            environment.insert("PI_CODING_AGENT_DIR".to_owned(), home.display().to_string());
            vec![
                "--no-skills".into(),
                "--no-rules".into(),
                "--no-extensions".into(),
                "--config".into(),
                isolation.display().to_string(),
                "--cwd".into(),
                workspace.display().to_string(),
            ]
        }
    };
    let invocation = Invocation {
        env: environment,
        command: harness.command().to_owned(),
        args,
        cwd: workspace.to_path_buf(),
        bundle: None,
    };
    if dry_run {
        print_invocation(&invocation);
        return Ok(());
    }
    let status = run_invocation(&invocation)?;
    propagate_exit(status)
}

fn prepare(agent: &ResolvedAgent, workspace: &Path) -> Result<Invocation> {
    let run_root = receipt::state_root()?.join("runs").join(format!(
        "{}-{}-{}",
        Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
        std::process::id(),
        agent.name
    ));
    let guard = RunBundle::new(run_root.clone());
    let bundle = run_root.join("bundle");
    agent.write_bundle(&bundle, workspace)?;
    let mut invocation = match agent.harness {
        Harness::Codex => prepare_codex(agent, workspace, &bundle, &run_root)?,
        Harness::Claude => prepare_claude(agent, workspace, &bundle, &run_root)?,
        Harness::Omp => prepare_omp(agent, workspace, &bundle, &run_root)?,
    };
    invocation
        .env
        .insert("ROSTER_AGENT".to_owned(), agent.name.clone());
    invocation.env.insert(
        "ROSTER_CONFIG".to_owned(),
        agent.config_path.display().to_string(),
    );
    invocation.args.extend(agent.args.clone());
    invocation.bundle = Some(guard);
    Ok(invocation)
}

fn prepare_codex(
    agent: &ResolvedAgent,
    workspace: &Path,
    bundle: &Path,
    run_root: &Path,
) -> Result<Invocation> {
    let home = run_root.join("projection/codex");
    fs::create_dir_all(&home)?;
    let native_home = real_home()?.join(".codex");
    for name in [
        "auth.json",
        "sessions",
        "archived_sessions",
        "history.jsonl",
        "session_index.jsonl",
    ] {
        bridge(&home, name, &native_home.join(name))?;
    }
    symlink_if_present(&bundle.join("AGENTS.md"), &home.join("AGENTS.md"))?;
    symlink_if_present(&bundle.join("skills"), &home.join("skills"))?;
    fs::write(home.join("config.toml"), codex_config(agent, workspace)?)?;
    Ok(Invocation {
        env: BTreeMap::from([
            ("CODEX_HOME".to_owned(), home.display().to_string()),
            (
                "CODEX_SQLITE_HOME".to_owned(),
                native_home.display().to_string(),
            ),
        ]),
        command: "codex".to_owned(),
        args: vec![
            "--strict-config".into(),
            "--disable".into(),
            "apps".into(),
            "-m".into(),
            agent.model.clone(),
            "-C".into(),
            workspace.display().to_string(),
        ],
        cwd: workspace.to_path_buf(),
        bundle: None,
    })
}

fn prepare_claude(
    agent: &ResolvedAgent,
    workspace: &Path,
    bundle: &Path,
    run_root: &Path,
) -> Result<Invocation> {
    let projection = run_root.join("projection/claude");
    let plugin = projection.join("root-plugin");
    fs::create_dir_all(plugin.join(".claude-plugin"))?;
    fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        serde_json::to_vec_pretty(&json!({
            "name": format!("roster-{}", agent.name),
            "description": agent.description,
            "version": env!("CARGO_PKG_VERSION"),
            "author": {"name": "Roster"}
        }))?,
    )?;
    symlink_if_present(&bundle.join("skills"), &plugin.join("skills"))?;
    let mcp_path = projection.join("root-mcp.json");
    fs::write(
        &mcp_path,
        serde_json::to_vec_pretty(&mcp_json(&agent.mcps))?,
    )?;
    let mut args = vec![
        "--setting-sources=".into(),
        "--system-prompt-file".into(),
        bundle.join("AGENTS.md").display().to_string(),
        "--plugin-dir-no-mcp".into(),
        plugin.display().to_string(),
        "--strict-mcp-config".into(),
        "--mcp-config".into(),
        mcp_path.display().to_string(),
        "--model".into(),
        agent.model.clone(),
        "--name".into(),
        agent.name.clone(),
    ];
    if let Some(reasoning) = &agent.reasoning {
        args.extend(["--effort".into(), reasoning.clone()]);
    }
    Ok(Invocation {
        env: BTreeMap::new(),
        command: "claude".to_owned(),
        args,
        cwd: workspace.to_path_buf(),
        bundle: None,
    })
}

fn prepare_omp(
    agent: &ResolvedAgent,
    workspace: &Path,
    bundle: &Path,
    run_root: &Path,
) -> Result<Invocation> {
    let projection = run_root.join("projection/omp");
    let home = projection.join("agent");
    fs::create_dir_all(&home)?;
    symlink_if_present(&bundle.join("skills"), &home.join("skills"))?;
    fs::write(
        home.join("mcp.json"),
        serde_json::to_vec_pretty(&mcp_json(&agent.mcps))?,
    )?;
    let isolation = projection.join("isolation.yml");
    fs::write(&isolation, omp_isolation())?;
    let system_prompt =
        fs::read_to_string(bundle.join("AGENTS.md")).context("read resolved OMP instructions")?;
    let mut args = vec![
        "--cwd".into(),
        workspace.display().to_string(),
        "--model".into(),
        agent.model.clone(),
        "--system-prompt".into(),
        system_prompt,
        "--append-system-prompt".into(),
        String::new(),
        "--skills".into(),
        agent
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>()
            .join(","),
        "--no-rules".into(),
        "--no-extensions".into(),
    ];
    args.extend(["--config".into(), isolation.display().to_string()]);
    if let Some(reasoning) = &agent.reasoning {
        args.extend(["--thinking".into(), reasoning.clone()]);
    }
    let sessions = real_home()?.join(".omp/agent/sessions");
    if sessions.is_dir() {
        args.extend(["--session-dir".into(), sessions.display().to_string()]);
    }
    Ok(Invocation {
        env: BTreeMap::from([("PI_CODING_AGENT_DIR".to_owned(), home.display().to_string())]),
        command: "omp".to_owned(),
        args,
        cwd: workspace.to_path_buf(),
        bundle: None,
    })
}

fn omp_isolation() -> &'static str {
    "setupVersion: 1\nmcp:\n  enableProjectConfig: false\n  discoveryMode: false\ntools:\n  discoveryMode: off\ndisabledProviders:\n  - agents-md\n  - claude\n  - claude-plugins\n  - cline\n  - codex\n  - cursor\n  - gemini\n  - github\n  - mcp-json\n  - omp-plugins\n  - opencode\n  - ssh-json\n  - vscode\n  - windsurf\n"
}

fn codex_config(agent: &ResolvedAgent, workspace: &Path) -> Result<String> {
    let mut document = format!("model = {:?}\n", agent.model);
    if let Some(reasoning) = &agent.reasoning {
        document.push_str(&format!("model_reasoning_effort = {:?}\n", reasoning));
    }
    if let Some((project, trust_level)) = codex_project_trust(workspace)? {
        document.push_str(&format!(
            "\n[projects.{}]\ntrust_level = {:?}\n",
            toml_key(&project),
            trust_level
        ));
    }
    for item in &agent.mcps {
        document.push_str(&format!("\n[mcp_servers.{}]\n", toml_key(&item.id)));
        match item.transport.as_deref() {
            Some("stdio") => document.push_str(&format!(
                "command = {:?}\nargs = {}\n",
                item.command.as_deref().context("stdio MCP command")?,
                serde_json::to_string(&item.args)?
            )),
            Some("http") => document.push_str(&format!(
                "url = {:?}\n",
                item.url.as_deref().context("http MCP URL")?
            )),
            other => bail!("unsupported MCP transport {other:?} for {}", item.id),
        }
    }
    for name in project_mcp_names(workspace)? {
        if !agent.mcps.iter().any(|item| item.id == name) {
            document.push_str(&format!(
                "\n[mcp_servers.{}]\nenabled = false\n",
                toml_key(&name)
            ));
        }
    }
    for path in external_skill_paths(workspace)? {
        document.push_str(&format!(
            "\n[[skills.config]]\npath = {:?}\nenabled = false\n",
            path.display().to_string()
        ));
    }
    Ok(document)
}

fn codex_project_trust(workspace: &Path) -> Result<Option<(String, String)>> {
    let path = real_home()?.join(".codex/config.toml");
    if !path.is_file() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("read Codex trust state from {}", path.display()))?;
    let value: toml::Value = toml::from_str(&contents)
        .with_context(|| format!("parse Codex trust state from {}", path.display()))?;
    let Some(projects) = value.get("projects").and_then(toml::Value::as_table) else {
        return Ok(None);
    };
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut matches = projects
        .iter()
        .filter_map(|(project, settings)| {
            let project_path = PathBuf::from(project);
            let comparable = project_path
                .canonicalize()
                .unwrap_or_else(|_| project_path.clone());
            let trust_level = settings.get("trust_level")?.as_str()?;
            workspace.starts_with(&comparable).then(|| {
                (
                    comparable.components().count(),
                    project.clone(),
                    trust_level.to_owned(),
                )
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(depth, _, _)| *depth);
    Ok(matches
        .pop()
        .map(|(_, project, trust_level)| (project, trust_level)))
}

fn project_mcp_names(workspace: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let home = real_home()?;
    for directory in workspace.ancestors() {
        if directory == home {
            break;
        }
        let path = directory.join(".codex/config.toml");
        if !path.is_file() {
            continue;
        }
        let contents =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let value: toml::Value =
            toml::from_str(&contents).with_context(|| format!("parse {}", path.display()))?;
        if let Some(table) = value.get("mcp_servers").and_then(toml::Value::as_table) {
            names.extend(table.keys().cloned());
        }
    }
    Ok(names)
}

fn external_skill_paths(workspace: &Path) -> Result<BTreeSet<PathBuf>> {
    let home = real_home()?;
    let mut roots = vec![home.join(".agents/skills")];
    for directory in workspace.ancestors() {
        if directory == home {
            break;
        }
        roots.push(directory.join(".agents/skills"));
        roots.push(directory.join(".codex/skills"));
    }
    let mut paths = BTreeSet::new();
    for root in roots {
        collect_skill_files(&root, &mut paths)?;
    }
    Ok(paths)
}

fn collect_skill_files(directory: &Path, output: &mut BTreeSet<PathBuf>) -> Result<()> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_skill_files(&path, output)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            output.insert(path);
        }
    }
    Ok(())
}

fn mcp_json(mcps: &[ResolvedMcp]) -> Value {
    let mut servers = Map::new();
    for item in mcps {
        let value = match item.transport.as_deref() {
            Some("stdio") => json!({"command": item.command, "args": item.args}),
            Some("http") => json!({"type": "http", "url": item.url}),
            _ => json!({"disabled": true}),
        };
        servers.insert(item.id.clone(), value);
    }
    json!({"mcpServers": servers})
}

fn preflight(agent: &ResolvedAgent, workspace: &Path, invocation: &Invocation) -> Result<()> {
    match agent.harness {
        Harness::Codex => {
            require_version(invocation, &["--version"], &["codex-cli 0.144.3"])?;
            let output = Command::new("codex")
                .args(["mcp", "--disable", "apps", "list", "--json"])
                .current_dir(workspace)
                .envs(&invocation.env)
                .output()
                .context("inspect isolated Codex MCP catalog")?;
            if !output.status.success() {
                bail!(
                    "Codex isolation preflight failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            let value: Value = serde_json::from_slice(&output.stdout)?;
            let actual = value
                .as_array()
                .into_iter()
                .flatten()
                .filter(|entry| entry.get("enabled").and_then(Value::as_bool) == Some(true))
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .collect::<BTreeSet<_>>();
            let expected = agent.mcps.iter().map(|item| item.id.as_str()).collect();
            if actual != expected {
                bail!("Codex MCP isolation drift: expected {expected:?}, loaded {actual:?}");
            }
        }
        Harness::Claude => preflight_claude(agent, invocation)?,
        Harness::Omp => preflight_omp(agent, invocation)?,
    }
    Ok(())
}

fn preflight_claude(agent: &ResolvedAgent, invocation: &Invocation) -> Result<()> {
    require_version(invocation, &["--version"], &["2.1.207"])?;
    let plugin = invocation_arg(invocation, "--plugin-dir-no-mcp")?;
    let output = Command::new("claude")
        .args(["plugin", "validate", "--strict", plugin])
        .current_dir(&invocation.cwd)
        .envs(&invocation.env)
        .output()
        .context("validate isolated Claude plugin")?;
    if !output.status.success() {
        bail!(
            "Claude isolation preflight failed: {}",
            bounded_stderr(&output.stderr)
        );
    }
    let mcp_path = invocation_arg(invocation, "--mcp-config")?;
    let value: Value = serde_json::from_slice(
        &fs::read(mcp_path).with_context(|| format!("read Claude MCP projection {mcp_path}"))?,
    )?;
    let actual = value
        .get("mcpServers")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|servers| servers.keys().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let expected = agent.mcps.iter().map(|item| item.id.as_str()).collect();
    if actual != expected {
        bail!("Claude MCP projection drift: expected {expected:?}, wrote {actual:?}");
    }
    Ok(())
}

fn preflight_omp(agent: &ResolvedAgent, invocation: &Invocation) -> Result<()> {
    require_version(invocation, &["--version"], &["omp v16.4.4", "omp/16.4.4"])?;
    let output = omp_rpc_state(invocation, !agent.mcps.is_empty())?;
    let state = output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|frame| {
            frame.get("command").and_then(Value::as_str) == Some("get_state")
                && frame.get("success").and_then(Value::as_bool) == Some(true)
        })
        .context("OMP isolation probe returned no get_state response")?;
    let data = state.get("data").context("OMP get_state omitted data")?;
    let model = data
        .pointer("/model/id")
        .and_then(Value::as_str)
        .context("OMP get_state omitted model id")?;
    if model != agent.model {
        bail!(
            "OMP model isolation drift: expected {:?}, loaded {model:?}",
            agent.model
        );
    }
    let prompt_segments = data
        .get("systemPrompt")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let expected_prompt = invocation_arg(invocation, "--system-prompt")?;
    let expected_normalized = expected_prompt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let instructions_loaded = prompt_segments.iter().any(|segment| {
        let actual_normalized = segment.split_whitespace().collect::<Vec<_>>().join(" ");
        actual_normalized == expected_normalized
            || actual_normalized
                .strip_prefix(&expected_normalized)
                .is_some_and(|suffix| suffix.starts_with(' '))
    });
    if !instructions_loaded {
        bail!("OMP instruction isolation drift: resolved AGENTS.md was semantically altered");
    }
    let prompt = prompt_segments.join("\n");
    let actual_skills = prompt
        .split("<skill name=\"")
        .skip(1)
        .filter_map(|tail| tail.split_once('\"').map(|(name, _)| name))
        .collect::<BTreeSet<_>>();
    let expected_skills = agent
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<BTreeSet<_>>();
    if actual_skills != expected_skills {
        bail!("OMP skill isolation drift: expected {expected_skills:?}, loaded {actual_skills:?}");
    }

    let tools = data
        .get("dumpTools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if tools.contains(&"search_tool_bm25") {
        bail!("OMP isolation drift: ambient tool discovery remained enabled");
    }
    let mut observed_mcps = BTreeSet::new();
    for tool in tools.into_iter().filter(|tool| tool.starts_with("mcp__")) {
        let matched = agent
            .mcps
            .iter()
            .find(|mcp| tool.starts_with(&format!("mcp__{}_", mcp.id.replace('-', "_"))));
        let Some(mcp) = matched else {
            bail!("OMP MCP isolation drift: loaded undeclared tool {tool:?}");
        };
        observed_mcps.insert(mcp.id.as_str());
    }
    let expected_mcps = agent.mcps.iter().map(|mcp| mcp.id.as_str()).collect();
    if observed_mcps != expected_mcps {
        bail!("OMP MCP isolation drift: expected {expected_mcps:?}, connected {observed_mcps:?}");
    }
    Ok(())
}

fn omp_rpc_state(invocation: &Invocation, wait_for_mcps: bool) -> Result<String> {
    let mut command = Command::new(&invocation.command);
    command
        .args(&invocation.args)
        .args(["--mode", "rpc", "--no-session"])
        .envs(&invocation.env)
        .current_dir(&invocation.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().context("start OMP isolation probe")?;
    let mut stdout = child.stdout.take().context("capture OMP probe stdout")?;
    let mut stderr = child.stderr.take().context("capture OMP probe stderr")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    thread::sleep(Duration::from_secs(if wait_for_mcps { 5 } else { 2 }));
    let mut stdin = child.stdin.take().context("open OMP probe stdin")?;
    stdin.write_all(b"{\"id\":\"roster-preflight\",\"type\":\"get_state\"}\n")?;
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("OMP isolation probe timed out");
        }
        thread::sleep(Duration::from_millis(50));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("OMP probe stdout reader panicked"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("OMP probe stderr reader panicked"))?;
    if !status.success() {
        bail!(
            "OMP isolation preflight failed: {}",
            bounded_stderr(&stderr)
        );
    }
    String::from_utf8(stdout).context("OMP isolation probe emitted non-UTF-8 output")
}

fn require_version(invocation: &Invocation, args: &[&str], supported: &[&str]) -> Result<()> {
    let output = Command::new(&invocation.command)
        .args(args)
        .current_dir(&invocation.cwd)
        .envs(&invocation.env)
        .output()
        .with_context(|| format!("probe {} version", invocation.command))?;
    if !output.status.success() {
        bail!(
            "{} version probe failed: {}",
            invocation.command,
            bounded_stderr(&output.stderr)
        );
    }
    let actual = String::from_utf8_lossy(&output.stdout);
    if !supported
        .iter()
        .any(|expected| actual.trim().starts_with(expected))
    {
        bail!(
            "{} adapter supports {supported:?}; live version is {:?}",
            invocation.command,
            actual.trim()
        );
    }
    Ok(())
}

fn invocation_arg<'a>(invocation: &'a Invocation, flag: &str) -> Result<&'a str> {
    invocation
        .args
        .iter()
        .position(|argument| argument == flag)
        .and_then(|index| invocation.args.get(index + 1))
        .map(String::as_str)
        .with_context(|| format!("{flag} missing from {} projection", invocation.command))
}

fn bounded_stderr(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let start = text.len().saturating_sub(2_000);
    text.get(start..).unwrap_or(&text).trim().to_owned()
}

fn bridge(destination_root: &Path, name: &str, source: &Path) -> Result<()> {
    if source.exists() {
        symlink(source, destination_root.join(name))?;
    }
    Ok(())
}

fn symlink_if_present(source: &Path, destination: &Path) -> Result<()> {
    if source.exists() {
        symlink(source, destination)?;
    }
    Ok(())
}

fn real_home() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

fn toml_key(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        value.to_owned()
    } else {
        format!("{value:?}")
    }
}

fn print_invocation(invocation: &Invocation) {
    print!(
        "cd {} && ",
        shell_word(&invocation.cwd.display().to_string())
    );
    for (key, value) in &invocation.env {
        print!("{key}={} ", shell_word(value));
    }
    print!("{}", invocation.command);
    for argument in &invocation.args {
        print!(" {}", shell_word(argument));
    }
    println!();
}

fn shell_word(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._-=:,".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{value}'", value = value.replace('\'', "'\\''"))
    }
}

fn run_invocation(invocation: &Invocation) -> Result<ExitStatus> {
    let mut command = Command::new(&invocation.command);
    command
        .args(&invocation.args)
        .envs(&invocation.env)
        .current_dir(&invocation.cwd);
    let child_pid = Arc::new(AtomicI32::new(0));
    let mut signals = Signals::new([libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT])?;
    let signal_handle = signals.handle();
    let forwarded_pid = Arc::clone(&child_pid);
    let forwarder = thread::spawn(move || {
        for signal in signals.forever() {
            let pid = forwarded_pid.load(Ordering::SeqCst);
            if pid > 0 {
                // SAFETY: pid comes from Child and signal is from the bounded
                // signal-hook list above.
                unsafe {
                    libc::kill(pid, signal);
                }
            }
        }
    });
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            signal_handle.close();
            let _ = forwarder.join();
            return Err(error).with_context(|| format!("launch {}", invocation.command));
        }
    };
    child_pid.store(child.id() as i32, Ordering::SeqCst);
    let status = child.wait()?;
    child_pid.store(0, Ordering::SeqCst);
    signal_handle.close();
    let _ = forwarder.join();
    Ok(status)
}

fn propagate_exit(status: ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }
    if let Some(signal) = status.signal() {
        // SAFETY: wait(2) reported this exact terminating signal.
        unsafe {
            libc::raise(signal);
        }
    }
    std::process::exit(status.code().unwrap_or(1));
}

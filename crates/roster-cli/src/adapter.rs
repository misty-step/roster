use crate::receipt;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use roster_core::{Harness, ResolvedAgent, ResolvedMcp};
use serde_json::{Map, Value, json};
use signal_hook::iterator::Signals;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::fs::symlink,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
        mpsc,
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
    let started_at = Utc::now();
    if let Err(preflight_error) = preflight(agent, workspace, &invocation) {
        let receipt_path = match receipt::record(
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
            None,
        ) {
            Ok(path) => path,
            Err(receipt_error) => {
                return Err(anyhow::anyhow!(
                    "{preflight_error:#}; failed to record preflight receipt: {receipt_error:#}"
                ));
            }
        };
        if let Err(cleanup_error) = invocation
            .bundle
            .as_mut()
            .expect("dispatch bundle")
            .cleanup()
        {
            return Err(anyhow::anyhow!(
                "{preflight_error:#}; failed to clean dispatch projection after preflight: {cleanup_error:#}"
            ));
        }
        eprintln!("roster receipt: {}", receipt_path.display());
        return Err(preflight_error);
    }
    eprintln!("Launching {} ({})…", agent.name, agent.harness);
    std::io::stderr().flush()?;
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
    fs::copy(bundle.join("AGENTS.md"), home.join("AGENTS.md"))
        .context("copy Codex instruction projection")?;
    copy_projection_tree(&bundle.join("skills"), &home.join("skills"))?;
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
    copy_projection_tree(&bundle.join("skills"), &plugin.join("skills"))?;
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
    copy_projection_tree(&bundle.join("skills"), &home.join("skills"))?;
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
    let mut document = format!("model = {:?}\nproject_doc_max_bytes = 0\n", agent.model);
    if let Some(reasoning) = &agent.reasoning {
        document.push_str(&format!("model_reasoning_effort = {:?}\n", reasoning));
    }
    let project = codex_project_root(workspace);
    document.push_str(&format!(
        "\n[projects.{}]\ntrust_level = \"untrusted\"\n",
        toml_key(&project.display().to_string())
    ));
    document.push_str("\n[skills.bundled]\nenabled = false\n");
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
    for path in external_skill_paths(workspace)? {
        document.push_str(&format!(
            "\n[[skills.config]]\npath = {:?}\nenabled = false\n",
            path.display().to_string()
        ));
    }
    Ok(document)
}

fn codex_project_root(workspace: &Path) -> PathBuf {
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    for directory in workspace.ancestors() {
        if directory.join(".git").exists() {
            return directory.to_path_buf();
        }
    }
    workspace
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
    let mut visited = BTreeSet::new();
    for root in roots {
        collect_skill_files(&root, &mut paths, &mut visited)?;
    }
    Ok(paths)
}

fn collect_skill_files(
    directory: &Path,
    output: &mut BTreeSet<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    if !directory.is_dir() {
        return Ok(());
    }
    let directory = directory.canonicalize()?;
    if !visited.insert(directory.clone()) {
        return Ok(());
    }
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_skill_files(&path, output, visited)?;
        } else if file_type.is_symlink() {
            let Ok(target) = path.canonicalize() else {
                continue;
            };
            if target.is_dir() {
                collect_skill_files(&target, output, visited)?;
            }
        } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            output.insert(path.canonicalize()?);
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
            let actual_skills = codex_enabled_skills(invocation, workspace)?;
            let skill_root = PathBuf::from(
                invocation
                    .env
                    .get("CODEX_HOME")
                    .context("CODEX_HOME missing from Codex projection")?,
            )
            .join("skills");
            let mut expected_skills = agent
                .skills
                .iter()
                .map(|skill| {
                    let path = skill_root.join(&skill.name).join("SKILL.md");
                    Ok((
                        skill.name.clone(),
                        path.canonicalize().with_context(|| {
                            format!("canonicalize projected skill {}", path.display())
                        })?,
                        "user".to_owned(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            expected_skills.sort();
            if actual_skills != expected_skills {
                bail!(
                    "Codex skill isolation drift: expected {expected_skills:?}, loaded {actual_skills:?}"
                );
            }
            let output = Command::new(&invocation.command)
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

fn codex_enabled_skills(
    invocation: &Invocation,
    workspace: &Path,
) -> Result<Vec<(String, PathBuf, String)>> {
    let mut child = Command::new(&invocation.command)
        .args(["app-server", "--strict-config", "--listen", "stdio://"])
        .current_dir(workspace)
        .envs(&invocation.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start strict Codex skill inventory probe")?;
    let mut stdin = child.stdin.take().context("open Codex probe stdin")?;
    let stdout = child.stdout.take().context("capture Codex probe stdout")?;
    let mut stderr = child.stderr.take().context("capture Codex probe stderr")?;
    let (sender, receiver) = mpsc::channel();
    let stdout_reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let frames = [
        json!({
            "method": "initialize",
            "id": "roster-init",
            "params": {
                "clientInfo": {"name": "roster", "title": "Roster", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }
        }),
        json!({"method": "initialized", "params": {}}),
        json!({
            "method": "skills/list",
            "id": "roster-skills",
            "params": {"cwds": [workspace.display().to_string()], "forceReload": true}
        }),
    ];
    let write_result = (|| -> Result<()> {
        for frame in frames {
            serde_json::to_writer(&mut stdin, &frame)?;
            stdin.write_all(b"\n")?;
        }
        stdin.flush()?;
        Ok(())
    })();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut response = None;
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => {
                let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if frame.get("id").and_then(Value::as_str) == Some("roster-skills") {
                    response = Some(frame);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.try_wait()?.is_some() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stdin);
    let mut timed_out = false;
    let shutdown_deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= shutdown_deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(25));
    };
    let _ = stdout_reader.join();
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Codex probe stderr reader panicked"))?;
    if timed_out {
        bail!("Codex strict configuration preflight timed out");
    }
    if !status.success() {
        bail!(
            "Codex strict configuration preflight failed: {}",
            bounded_stderr(&stderr)
        );
    }
    write_result?;
    let response =
        response.context("Codex skill isolation probe returned no skills/list response")?;
    if let Some(error) = response.get("error") {
        bail!("Codex skill isolation preflight failed: {error}");
    }
    let entries = response
        .pointer("/result/data")
        .and_then(Value::as_array)
        .context("Codex skills/list omitted result.data")?;
    let mut enabled = Vec::new();
    for entry in entries {
        let errors = entry
            .get("errors")
            .and_then(Value::as_array)
            .context("Codex skills/list omitted errors")?;
        if !errors.is_empty() {
            bail!(
                "Codex skill isolation preflight reported errors: {}",
                serde_json::to_string(errors)?
            );
        }
        for skill in entry
            .get("skills")
            .and_then(Value::as_array)
            .context("Codex skills/list omitted skills")?
        {
            if skill.get("enabled").and_then(Value::as_bool) == Some(true) {
                let name = skill
                    .get("name")
                    .and_then(Value::as_str)
                    .context("Codex enabled skill omitted name")?
                    .to_owned();
                let path = PathBuf::from(
                    skill
                        .get("path")
                        .and_then(Value::as_str)
                        .context("Codex enabled skill omitted path")?,
                );
                let path = path
                    .canonicalize()
                    .with_context(|| format!("canonicalize enabled skill {}", path.display()))?;
                let scope = skill
                    .get("scope")
                    .and_then(Value::as_str)
                    .context("Codex enabled skill omitted scope")?
                    .to_owned();
                enabled.push((name, path, scope));
            }
        }
    }
    enabled.sort();
    Ok(enabled)
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

fn copy_projection_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    if !source.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_projection_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)?;
        } else {
            bail!(
                "unsupported file type in Codex skill projection: {}",
                source_path.display()
            );
        }
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

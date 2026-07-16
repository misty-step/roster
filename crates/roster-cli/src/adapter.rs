use crate::{process, receipt};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use roster_core::{BundleManifest, Harness, ResolvedAgent, ResolvedMcp};
use serde_json::{Map, Value, json};
use signal_hook::iterator::Signals;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::fs::{PermissionsExt, symlink},
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
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
    disabled_skill_paths: BTreeSet<PathBuf>,
    bundle: Option<RunBundle>,
}

#[derive(Debug)]
struct RunBundle {
    path: PathBuf,
    manifest: Option<BundleManifest>,
    keep: bool,
    cleaned: bool,
}

impl RunBundle {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            manifest: None,
            keep: false,
            cleaned: false,
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        if !self.cleaned && self.path.exists() {
            if self.keep {
                self.redact_projected_mcp_configs()?;
            } else {
                fs::remove_dir_all(&self.path)?;
            }
        }
        self.cleaned = true;
        Ok(())
    }

    fn redact_projected_mcp_configs(&self) -> Result<()> {
        for relative in [
            "projection/claude/root-mcp.json",
            "projection/omp/agent/mcp.json",
        ] {
            let path = self.path.join(relative);
            if !path.is_file() {
                continue;
            }
            let mut document: Value = serde_json::from_slice(
                &fs::read(&path)
                    .with_context(|| format!("read projected MCP config {}", path.display()))?,
            )
            .with_context(|| format!("parse projected MCP config {}", path.display()))?;
            let Some(servers) = document
                .get_mut("mcpServers")
                .and_then(Value::as_object_mut)
            else {
                continue;
            };
            for server in servers.values_mut() {
                let Some(env_map) = server.get_mut("env").and_then(Value::as_object_mut) else {
                    continue;
                };
                for value in env_map.values_mut() {
                    *value = Value::String("<redacted by roster>".to_owned());
                }
            }
            fs::write(&path, serde_json::to_vec_pretty(&document)?)?;
        }
        Ok(())
    }

    fn manifest(&self) -> &BundleManifest {
        self.manifest
            .as_ref()
            .expect("dispatch bundle manifest must exist after preparation")
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
        eprintln!("Dry run only: live adapter preflight was not executed.");
        print_invocation(&invocation);
        return Ok(());
    }
    eprintln!("Preparing {} ({})…", agent.name, agent.harness);
    std::io::stderr().flush()?;
    let started_at = Utc::now();
    let harness_version = observe_version(&invocation);
    eprintln!("Adapter target: {} {harness_version}", agent.harness);
    if let Err(preflight_error) = preflight(agent, workspace, &invocation) {
        let receipt_path = match receipt::record(
            invocation
                .bundle
                .as_ref()
                .expect("dispatch bundle")
                .manifest(),
            keep_bundle.then(|| PathBuf::from("$ROSTER_BUNDLE")),
            started_at,
            &harness_version,
            false,
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
        invocation
            .bundle
            .as_ref()
            .expect("dispatch bundle")
            .manifest(),
        keep_bundle.then(|| PathBuf::from("$ROSTER_BUNDLE")),
        started_at,
        &harness_version,
        true,
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
            bridge_omp_credential_store(&home)?;
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
        disabled_skill_paths: BTreeSet::new(),
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
    let mut guard = RunBundle::new(run_root.clone());
    let bundle = run_root.join("bundle");
    guard.manifest = Some(agent.write_bundle(&bundle, workspace)?);
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
        "themes",
    ] {
        bridge(&home, name, &native_home.join(name))?;
    }
    fs::copy(bundle.join("AGENTS.md"), home.join("AGENTS.md"))
        .context("copy Codex instruction projection")?;
    copy_projection_tree(&bundle.join("skills"), &home.join("skills"))?;
    let disabled_skill_paths = external_skill_paths(workspace)?;
    fs::write(
        home.join("config.toml"),
        codex_config(
            agent,
            workspace,
            &native_home.join("config.toml"),
            &disabled_skill_paths,
        )?,
    )?;
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
        disabled_skill_paths,
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
    write_private_json(&mcp_path, &mcp_json(&agent.mcps))?;
    let settings_path = projection.join("settings.json");
    fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&claude_presentation_settings())?,
    )?;
    let mut args = vec![
        "--setting-sources=".into(),
        "--settings".into(),
        settings_path.display().to_string(),
        "--system-prompt-file".into(),
        bundle.join("AGENTS.md").display().to_string(),
        "--plugin-dir".into(),
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
        disabled_skill_paths: BTreeSet::new(),
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
    // Reuse the operator credential store. OMP already multi-opens one
    // agent.db across interactive sessions; a forked empty store cannot
    // resolve API keys or OAuth. Projection still owns mcp.json, skills,
    // and the isolation overlay — never ambient config or discovery.
    bridge_omp_credential_store(&home)?;
    copy_projection_tree(&bundle.join("skills"), &home.join("skills"))?;
    let mcp_path = home.join("mcp.json");
    write_private_json(&mcp_path, &mcp_json(&agent.mcps))?;
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
        disabled_skill_paths: BTreeSet::new(),
        bundle: None,
    })
}

fn omp_isolation() -> &'static str {
    "setupVersion: 1\nmcp:\n  enableProjectConfig: false\n  discoveryMode: false\ntools:\n  discoveryMode: off\ndisabledProviders:\n  - agents-md\n  - claude\n  - claude-plugins\n  - cline\n  - codex\n  - cursor\n  - gemini\n  - github\n  - mcp-json\n  - omp-plugins\n  - opencode\n  - ssh-json\n  - vscode\n  - windsurf\n"
}

fn codex_config(
    agent: &ResolvedAgent,
    workspace: &Path,
    native_config: &Path,
    disabled_skill_paths: &BTreeSet<PathBuf>,
) -> Result<String> {
    let mut document = format!("model = {:?}\nproject_doc_max_bytes = 0\n", agent.model);
    if let Some(reasoning) = &agent.reasoning {
        document.push_str(&format!("model_reasoning_effort = {:?}\n", reasoning));
    }
    if let Some(tui) = codex_presentation_config(native_config) {
        document.push_str(&format!("\n{tui}"));
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
            Some("stdio") => {
                document.push_str(&format!(
                    "command = {:?}\nargs = {}\n",
                    item.command.as_deref().context("stdio MCP command")?,
                    serde_json::to_string(&item.args)?
                ));
                if !item.env_refs.is_empty() {
                    document.push_str(&format!(
                        "env_vars = {}\n",
                        serde_json::to_string(&item.env_refs)?
                    ));
                }
            }
            Some("http") => document.push_str(&format!(
                "url = {:?}\n",
                item.url.as_deref().context("http MCP URL")?
            )),
            other => bail!("unsupported MCP transport {other:?} for {}", item.id),
        }
    }
    for path in disabled_skill_paths {
        document.push_str(&format!(
            "\n[[skills.config]]\npath = {:?}\nenabled = false\n",
            path.display().to_string()
        ));
    }
    Ok(document)
}

// Native config crosses the isolation boundary only through these structural
// presentation allowlists; the source files themselves are never forwarded.
fn claude_presentation_settings() -> Value {
    let Ok(home) = real_home() else {
        return json!({});
    };
    let Ok(bytes) = fs::read(home.join(".claude/settings.json")) else {
        return json!({});
    };
    let Ok(native) = serde_json::from_slice::<Value>(&bytes) else {
        return json!({});
    };
    let Some(native) = native.as_object() else {
        return json!({});
    };
    let mut projected = Map::new();
    if let Some(tui) = native
        .get("tui")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "default" | "fullscreen"))
    {
        projected.insert("tui".to_owned(), Value::String(tui.to_owned()));
    }
    if let Some(status_line) = native.get("statusLine").and_then(claude_status_line) {
        projected.insert("statusLine".to_owned(), status_line);
    }
    Value::Object(projected)
}

fn claude_status_line(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some("command") {
        return None;
    }
    let command = object.get("command").and_then(Value::as_str)?;
    let mut projected = Map::new();
    projected.insert("type".to_owned(), Value::String("command".to_owned()));
    projected.insert("command".to_owned(), Value::String(command.to_owned()));
    if let Some(padding) = object
        .get("padding")
        .and_then(Value::as_u64)
        .map(|value| Value::Number(value.into()))
    {
        projected.insert("padding".to_owned(), padding);
    }
    if let Some(refresh_interval) = object
        .get("refreshInterval")
        .and_then(Value::as_u64)
        .filter(|value| *value >= 1)
        .map(|value| Value::Number(value.into()))
    {
        projected.insert("refreshInterval".to_owned(), refresh_interval);
    }
    if let Some(hide_vim_mode_indicator) = object
        .get("hideVimModeIndicator")
        .and_then(Value::as_bool)
        .map(Value::Bool)
    {
        projected.insert("hideVimModeIndicator".to_owned(), hide_vim_mode_indicator);
    }
    Some(Value::Object(projected))
}

fn codex_presentation_config(native_config: &Path) -> Option<String> {
    let source = fs::read_to_string(native_config).ok()?;
    let native: toml::Value = toml::from_str(&source).ok()?;
    let tui = native.get("tui")?.as_table()?;
    let mut fields = Vec::new();
    if let Some(theme) = tui.get("theme").and_then(toml::Value::as_str) {
        fields.push(format!("theme = {}", toml::Value::String(theme.to_owned())));
    }
    if let Some(status_line) = toml_string_array(tui.get("status_line")) {
        fields.push(format!("status_line = {status_line}"));
    }
    if let Some(status_line_use_colors) = tui
        .get("status_line_use_colors")
        .and_then(toml::Value::as_bool)
    {
        fields.push(format!("status_line_use_colors = {status_line_use_colors}"));
    }
    (!fields.is_empty()).then(|| format!("[tui]\n{}\n", fields.join("\n")))
}

fn toml_string_array(value: Option<&toml::Value>) -> Option<String> {
    let items = value?.as_array()?;
    let items = items
        .iter()
        .map(toml::Value::as_str)
        .collect::<Option<Vec<_>>>()?;
    Some(
        toml::Value::Array(
            items
                .into_iter()
                .map(|item| toml::Value::String(item.to_owned()))
                .collect(),
        )
        .to_string(),
    )
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
            Some("stdio") => {
                let mut server = Map::new();
                server.insert(
                    "command".to_owned(),
                    Value::String(item.command.clone().unwrap_or_default()),
                );
                server.insert("args".to_owned(), json!(item.args));
                // Tier 1 adapters spawn stdio MCP servers with only the
                // declared env map (plus a minimal host baseline). Ambient
                // process env is not inherited for secrets — promote
                // ROSTER_CHILD_ENV_* values.
                let mut env_map = Map::new();
                for env_ref in &item.env_refs {
                    if let Some(value) = resolve_mcp_env_value(env_ref) {
                        env_map.insert(env_ref.clone(), Value::String(value));
                    }
                }
                if !env_map.is_empty() {
                    server.insert("env".to_owned(), Value::Object(env_map));
                }
                Value::Object(server)
            }
            Some("http") => json!({"type": "http", "url": item.url}),
            _ => json!({"disabled": true}),
        };
        servers.insert(item.id.clone(), value);
    }
    json!({"mcpServers": servers})
}
fn write_private_json(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn resolve_mcp_env_value(env_ref: &str) -> Option<String> {
    let child_key = format!("ROSTER_CHILD_ENV_{env_ref}");
    env::var(&child_key).ok().filter(|value| !value.is_empty())
}

fn preflight(agent: &ResolvedAgent, workspace: &Path, invocation: &Invocation) -> Result<()> {
    match agent.harness {
        Harness::Codex => {
            require_declared_mcp_child_env(agent)?;
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
            let output = process::isolated(&invocation.command, &invocation.env)
                .args(["mcp", "--disable", "apps", "list", "--json"])
                .current_dir(workspace)
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
    let mut child = process::isolated(&invocation.command, &invocation.env)
        .args(["app-server", "--strict-config", "--listen", "stdio://"])
        .current_dir(workspace)
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
        let unexpected_errors = errors
            .iter()
            .filter(|error| {
                let Some(path) = error.get("path").and_then(Value::as_str) else {
                    return true;
                };
                let path = PathBuf::from(path);
                let path = path.canonicalize().unwrap_or(path);
                !invocation.disabled_skill_paths.contains(&path)
            })
            .collect::<Vec<_>>();
        if !unexpected_errors.is_empty() {
            bail!(
                "Codex skill isolation preflight reported errors: {}",
                serde_json::to_string(&unexpected_errors)?
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
    require_declared_mcp_child_env(agent)?;
    require_accepted_arguments(invocation)?;
    let plugin = invocation_arg(invocation, "--plugin-dir")?;
    let output = process::isolated(&invocation.command, &invocation.env)
        .args(["plugin", "validate", "--strict", plugin])
        .current_dir(&invocation.cwd)
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
    require_declared_mcp_child_env(agent)?;
    verify_omp_projected_mcps(agent, invocation)?;
    // omp/17 RPC get_state.dumpTools does not surface MCP tools even when the
    // server is declared and connectable. Preflight therefore proves model,
    // instructions, skills, and structural MCP projection — not dumpTools
    // membership. Live MCP tool activation is harness-owned after launch.
    let (data, probe_stderr) = omp_probe_inventory(invocation)?;
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
    let mut undeclared = Vec::new();
    for tool in tools
        .iter()
        .copied()
        .filter(|tool| tool.starts_with("mcp__"))
    {
        if declared_mcp_for_tool(tool, agent).is_none() {
            undeclared.push(tool);
        }
    }
    if !undeclared.is_empty() {
        bail!(
            "OMP MCP isolation drift: loaded undeclared tool(s) {undeclared:?}; probe stderr: {}",
            bounded_text(&probe_stderr)
        );
    }
    Ok(())
}

fn verify_omp_projected_mcps(agent: &ResolvedAgent, invocation: &Invocation) -> Result<()> {
    if agent.mcps.is_empty() {
        return Ok(());
    }
    let home = invocation
        .env
        .get("PI_CODING_AGENT_DIR")
        .context("PI_CODING_AGENT_DIR missing from OMP projection")?;
    let path = PathBuf::from(home).join("mcp.json");
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read projected OMP MCP config {}", path.display()))?;
    let document: Value =
        serde_json::from_str(&raw).context("projected OMP mcp.json is not valid JSON")?;
    let servers = document
        .get("mcpServers")
        .and_then(Value::as_object)
        .context("projected OMP mcp.json missing mcpServers")?;
    for mcp in &agent.mcps {
        let server = servers.get(&mcp.id).with_context(|| {
            format!(
                "projected OMP mcp.json omitted declared MCP {:?}; present {:?}",
                mcp.id,
                servers.keys().cloned().collect::<Vec<_>>()
            )
        })?;
        if mcp.transport.as_deref() == Some("stdio") {
            let command = server
                .get("command")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .with_context(|| format!("OMP MCP {:?} missing command in projection", mcp.id))?;
            if which_command(command).is_none() {
                bail!(
                    "OMP MCP {:?} command {command:?} is not on PATH for the isolated child",
                    mcp.id
                );
            }
            if !mcp.env_refs.is_empty() {
                let env_map = server
                    .get("env")
                    .and_then(Value::as_object)
                    .with_context(|| {
                        format!(
                            "OMP MCP {:?} requires env {:?} in projection but env map is missing",
                            mcp.id, mcp.env_refs
                        )
                    })?;
                for env_ref in &mcp.env_refs {
                    let value = env_map.get(env_ref).and_then(Value::as_str).unwrap_or("");
                    if value.is_empty() {
                        bail!(
                            "OMP MCP {:?} projection env missing non-empty {env_ref}; export ROSTER_CHILD_ENV_{env_ref}",
                            mcp.id
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn which_command(command: &str) -> Option<PathBuf> {
    if command.contains(std::path::MAIN_SEPARATOR) {
        let path = PathBuf::from(command);
        return path.is_file().then_some(path);
    }
    let path = env::var_os("PATH")?;
    env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(command);
        candidate.is_file().then_some(candidate)
    })
}

fn require_declared_mcp_child_env(agent: &ResolvedAgent) -> Result<()> {
    let mut missing = Vec::new();
    for mcp in &agent.mcps {
        for env_ref in &mcp.env_refs {
            // Isolated children only receive SAFE_PARENT_ENV plus explicit
            // ROSTER_CHILD_ENV_* promotions. Ambient MCP credentials do not pass.
            let child_key = format!("ROSTER_CHILD_ENV_{env_ref}");
            if env::var(&child_key)
                .ok()
                .filter(|value| !value.is_empty())
                .is_none()
            {
                missing.push(format!("{env_ref} (export {child_key})"));
            }
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    bail!(
        "MCP child environment incomplete before launch: missing {}. Roster does not read vaults; promote values with ROSTER_CHILD_ENV_<NAME>.",
        missing.join(", ")
    );
}

fn declared_mcp_for_tool<'a>(tool: &str, agent: &'a ResolvedAgent) -> Option<&'a ResolvedMcp> {
    agent.mcps.iter().find(|mcp| {
        let normalized = mcp.id.replace('-', "_");
        tool.starts_with(&format!("mcp__{normalized}_"))
            || tool.starts_with(&format!("mcp__{normalized}__"))
    })
}

fn omp_state_ready(data: &Value) -> bool {
    data.get("systemPrompt")
        .and_then(Value::as_array)
        .is_some_and(|segments| segments.iter().any(|segment| segment.as_str().is_some()))
}

fn omp_probe_inventory(invocation: &Invocation) -> Result<(Value, String)> {
    let mut command = process::isolated(&invocation.command, &invocation.env);
    command
        .args(&invocation.args)
        .args(["--mode", "rpc", "--no-session"])
        .current_dir(&invocation.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().context("start OMP isolation probe")?;
    let mut stdout = child.stdout.take().context("capture OMP probe stdout")?;
    let mut stderr = child.stderr.take().context("capture OMP probe stderr")?;
    let (line_tx, line_rx) = mpsc::channel::<Result<String, String>>();
    let stdout_reader = thread::spawn(move || {
        let mut reader = BufReader::new(&mut stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if line_tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = line_tx.send(Err(error.to_string()));
                    break;
                }
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let mut stdin = child.stdin.take().context("open OMP probe stdin")?;
    let timeout = Duration::from_secs(10);
    let deadline = Instant::now() + timeout;
    let mut last_state: Option<Value> = None;
    let mut last_tools: Vec<String> = Vec::new();
    let mut request = 0u32;
    let mut pending_id: Option<String> = None;
    let result = loop {
        if Instant::now() >= deadline {
            break Err(anyhow::anyhow!(
                "OMP isolation probe timed out after {timeout:?} waiting for inventory (last tools {last_tools:?})"
            ));
        }
        if let Some(status) = child.try_wait()? {
            break Err(anyhow::anyhow!(
                "OMP isolation probe exited early ({status})"
            ));
        }
        if pending_id.is_none() {
            request = request.saturating_add(1);
            let id = format!("roster-preflight-{request}");
            if let Err(error) = (|| -> Result<()> {
                writeln!(
                    stdin,
                    "{{\"id\":{id_json},\"type\":\"get_state\"}}",
                    id_json = serde_json::to_string(&id)
                        .unwrap_or_else(|_| "\"roster-preflight\"".into())
                )?;
                stdin.flush()?;
                Ok(())
            })() {
                break Err(error.context("write OMP get_state request"));
            }
            pending_id = Some(id);
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(250));
        match line_rx.recv_timeout(wait) {
            Ok(Ok(line)) => {
                let Some(state) = parse_omp_get_state_line(&line, pending_id.as_deref()) else {
                    continue;
                };
                pending_id = None;
                let data = match state.get("data").cloned() {
                    Some(data) => data,
                    None => {
                        break Err(anyhow::anyhow!("OMP get_state omitted data"));
                    }
                };
                last_tools = data
                    .get("dumpTools")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
                    .collect();
                if omp_state_ready(&data) {
                    last_state = Some(data);
                    break Ok(());
                }
                last_state = Some(data);
            }
            Ok(Err(error)) => break Err(anyhow::anyhow!("OMP probe stdout read failed: {error}")),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Re-issue get_state on the next loop if still pending too long,
                // or if the previous response never arrived.
                if pending_id.is_some() && request > 0 {
                    // Keep waiting until deadline; do not flood the runtime.
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err(anyhow::anyhow!(
                    "OMP isolation probe closed stdout before ready inventory"
                ));
            }
        }
    };

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_reader.join();
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("OMP probe stderr reader panicked"))?;
    let stderr = String::from_utf8_lossy(&stderr).into_owned();
    match result {
        Ok(()) => {
            let data = last_state.context("OMP isolation probe produced no get_state data")?;
            Ok((data, stderr))
        }
        Err(error) => Err(anyhow::anyhow!(
            "{error}; probe stderr: {}",
            bounded_text(&stderr)
        )),
    }
}

fn parse_omp_get_state_line(line: &str, request_id: Option<&str>) -> Option<Value> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let frame = serde_json::from_str::<Value>(trimmed).ok()?;
    if frame.get("command").and_then(Value::as_str) != Some("get_state") {
        return None;
    }
    if frame.get("success").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    if let (Some(expected), Some(id)) = (request_id, frame.get("id").and_then(Value::as_str))
        && id != expected
        && !id.starts_with("roster-preflight")
    {
        return None;
    }
    Some(frame)
}

fn bounded_text(text: &str) -> String {
    let start = text.len().saturating_sub(2_000);
    text.get(start..).unwrap_or(text).trim().to_owned()
}

fn observe_version(invocation: &Invocation) -> String {
    let output = process::isolated(&invocation.command, &invocation.env)
        .arg("--version")
        .current_dir(&invocation.cwd)
        .output();
    let Ok(output) = output else {
        return "version unavailable".to_owned();
    };
    if !output.status.success() {
        return format!("version unavailable (exit {:?})", output.status.code());
    }
    let version = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if version.is_empty() {
        return "version unavailable (empty output)".to_owned();
    }
    let lower = version.to_ascii_lowercase();
    let unsafe_word = lower
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| {
            matches!(
                word,
                "api" | "bearer" | "credential" | "key" | "password" | "secret" | "token"
            )
        });
    let safe = version.len() <= 200
        && version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || " ._+-()".contains(character))
        && version.chars().any(|character| character.is_ascii_digit())
        && !unsafe_word;
    if safe {
        version
    } else {
        "version unavailable (unsafe output)".to_owned()
    }
}

fn require_accepted_arguments(invocation: &Invocation) -> Result<()> {
    const SENTINEL: &str = "--roster-invalid-probe";
    let output = process::isolated(&invocation.command, &invocation.env)
        .args(&invocation.args)
        .arg(SENTINEL)
        .current_dir(&invocation.cwd)
        .output()
        .with_context(|| format!("probe {} argument parser", invocation.command))?;
    let stderr = bounded_stderr(&output.stderr);
    let first_diagnostic = stderr.lines().find(|line| !line.trim().is_empty());
    if output.status.success() || !first_diagnostic.is_some_and(|line| line.contains(SENTINEL)) {
        bail!("Claude adapter arguments were rejected before launch: {stderr}");
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
    bounded_text(&String::from_utf8_lossy(bytes))
}

fn bridge_omp_credential_store(home: &Path) -> Result<()> {
    let native = real_home()?.join(".omp/agent/agent.db");
    bridge(home, "agent.db", &native)?;
    Ok(())
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
        "cd {} && env -i ",
        shell_word(&invocation.cwd.display().to_string())
    );
    for (key, value) in process::visible_parent_environment() {
        print!("{key}={} ", shell_word(&value));
    }
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
    let mut command = process::isolated(&invocation.command, &invocation.env);
    command.args(&invocation.args).current_dir(&invocation.cwd);
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

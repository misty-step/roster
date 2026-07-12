//! Read-only inspection of the Tier 1 harness projections.
//!
//! Structural checks render the live declarations and compare exact projected
//! bytes. Optional live checks invoke bounded, read-only harness diagnostics.

use anyhow::{Context, Result, anyhow, bail};
use roster_core::{
    Mcp, Models, Roster, render_claude_agent, render_codex_agent, render_home_doctrine,
    render_omp_agent,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::sync::{codex_registration, managed_markdown};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const SCHEMA_VERSION: &str = "roster.doctor.v1";
const MANIFEST_REL: &str = ".roster/orchestrator/manifest.json";
const DOCTRINE_SOURCE_REL: &str = ".roster/orchestrator/home-doctrine.md";
const LEGACY_CODEX_CONFIG_REL: &str = ".codex/config/config.toml";

struct HarnessSpec {
    name: &'static str,
    binary: &'static str,
    native_config: &'static [&'static str],
    doctrine: &'static str,
    orchestrator: &'static str,
}

const CLAUDE_NATIVE_CONFIG: &[&str] = &[".claude/settings.json", ".claude.json"];
const CODEX_NATIVE_CONFIG: &[&str] = &[".codex/config.toml"];
const OMP_NATIVE_CONFIG: &[&str] = &[
    ".omp/agent/config.yml",
    ".omp/agent/config.yaml",
    ".omp/agent/mcp.json",
];

const HARNESSES: &[HarnessSpec] = &[
    HarnessSpec {
        name: "claude",
        binary: "claude",
        native_config: CLAUDE_NATIVE_CONFIG,
        doctrine: ".claude/CLAUDE.md",
        orchestrator: ".claude/agents/orchestrator.md",
    },
    HarnessSpec {
        name: "codex",
        binary: "codex",
        native_config: CODEX_NATIVE_CONFIG,
        doctrine: ".codex/AGENTS.md",
        orchestrator: ".roster/orchestrator/codex-roles/orchestrator.toml",
    },
    HarnessSpec {
        name: "omp",
        binary: "omp",
        native_config: OMP_NATIVE_CONFIG,
        doctrine: ".omp/agent/AGENTS.md",
        orchestrator: ".omp/agent/agents/orchestrator.md",
    },
];

pub fn run(root: &Path, home: Option<PathBuf>, json_output: bool, live: bool) -> Result<bool> {
    let home = home
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
        .context("HOME is required for roster doctor; pass --home to choose an inspection root")?;
    if home.as_os_str().is_empty() {
        bail!("HOME is required for roster doctor; pass --home to choose an inspection root");
    }
    let resolved_root = root
        .canonicalize()
        .with_context(|| format!("resolve roster root {}", root.display()))?;
    let roster = Roster::load(&resolved_root).context("load roster declarations")?;
    let models = Models::load(&resolved_root).context("load model bindings")?;
    let orchestrator = roster
        .agent("orchestrator")
        .context("roster has no orchestrator declaration")?;
    let expected_doctrine = managed_markdown(&render_home_doctrine(&resolved_root, orchestrator)?);
    let expected_agents = BTreeMap::from([
        (
            "claude",
            managed_markdown(
                &render_claude_agent(orchestrator, &models).map_err(|error| anyhow!(error))?,
            ),
        ),
        ("codex", render_codex_agent(orchestrator)),
        (
            "omp",
            managed_markdown(&render_omp_agent(orchestrator).map_err(|error| anyhow!(error))?),
        ),
    ]);
    let expected_codex_registration = codex_registration(
        orchestrator,
        &home.join(".roster/orchestrator/codex-roles/orchestrator.toml"),
    );

    let (manifest, mut failures) = inspect_manifest(&home);
    let mut warnings = Vec::new();
    let mut harnesses = BTreeMap::new();

    for spec in HARNESSES {
        let (report, harness_failures, harness_warnings) = inspect_harness(
            &home,
            spec,
            &expected_doctrine,
            expected_agents[spec.name].as_str(),
            &expected_codex_registration,
        );
        harnesses.insert(spec.name, report);
        failures.extend(harness_failures);
        warnings.extend(harness_warnings);
    }

    if fs::symlink_metadata(home.join(LEGACY_CODEX_CONFIG_REL)).is_ok() {
        warnings.push(format!(
            "codex: legacy nested Codex config at {LEGACY_CODEX_CONFIG_REL} is present; current Codex uses .codex/config.toml"
        ));
    }

    let (mcp_policy, mcp_failures, mcp_warnings) = inspect_mcp_policy(&home, roster.mcps());
    failures.extend(mcp_failures);
    warnings.extend(mcp_warnings);

    let live_report = if live {
        let (report, live_failures, live_warnings) = inspect_live(&home);
        failures.extend(live_failures);
        warnings.extend(live_warnings);
        report
    } else {
        json!({"enabled": false})
    };

    let ok = failures.is_empty();
    let degraded = !warnings.is_empty() || !failures.is_empty();
    let status = if !ok {
        "failed"
    } else if degraded {
        "degraded"
    } else {
        "ok"
    };
    let report = json!({
        "schema_version": SCHEMA_VERSION,
        "status": status,
        "ok": ok,
        "degraded": degraded,
        "source": {
            "root": root.display().to_string(),
            "home": home.display().to_string(),
        },
        "manifest": manifest,
        "harnesses": harnesses,
        "mcp_policy": mcp_policy,
        "live": live_report,
        "warnings": warnings,
        "failures": failures,
    });

    if json_output {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_human(&report);
    }
    Ok(ok)
}

fn inspect_harness(
    home: &Path,
    spec: &HarnessSpec,
    expected_doctrine: &str,
    expected_agent: &str,
    expected_codex_registration: &str,
) -> (Value, Vec<String>, Vec<String>) {
    let binary_path = find_binary(spec.binary);
    let binary_found = binary_path.is_some();
    let native_paths = spec
        .native_config
        .iter()
        .copied()
        .filter(|relative| home.join(relative).is_file())
        .collect::<Vec<_>>();
    let doctrine_path = home.join(spec.doctrine);
    let doctrine_meta = fs::symlink_metadata(&doctrine_path).ok();
    let doctrine_source = home.join(DOCTRINE_SOURCE_REL);
    let doctrine_present = doctrine_meta.as_ref().is_some_and(|metadata| {
        metadata.file_type().is_symlink()
            && doctrine_path.exists()
            && fs::canonicalize(&doctrine_path).ok() == fs::canonicalize(&doctrine_source).ok()
            && fs::read_to_string(&doctrine_source)
                .ok()
                .is_some_and(|contents| contents == expected_doctrine)
    });
    let doctrine_kind = doctrine_meta.map(|metadata| {
        if metadata.file_type().is_symlink() {
            "symlink"
        } else if metadata.is_file() {
            "file"
        } else {
            "other"
        }
    });

    let orchestrator_path = home.join(spec.orchestrator);
    let orchestrator_present = fs::metadata(&orchestrator_path)
        .ok()
        .is_some_and(|metadata| metadata.is_file())
        && fs::read_to_string(&orchestrator_path)
            .ok()
            .is_some_and(|contents| contents == expected_agent);

    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    if !binary_found {
        warnings.push(format!(
            "{}: binary {:?} was not found on PATH",
            spec.name, spec.binary
        ));
    }
    if binary_found && native_paths.is_empty() {
        warnings.push(format!(
            "{}: native config is missing (expected {})",
            spec.name,
            spec.native_config.join(" or ")
        ));
    }
    if binary_found && !doctrine_present {
        failures.push(format!(
            "{}: doctrine projection is missing or broken at {}",
            spec.name, spec.doctrine
        ));
    }
    if binary_found && !orchestrator_present {
        failures.push(format!(
            "{}: default orchestrator projection is missing or lacks named evidence at {}",
            spec.name, spec.orchestrator
        ));
    }
    if spec.name == "codex" && binary_found {
        let config = fs::read_to_string(home.join(CODEX_NATIVE_CONFIG[0])).unwrap_or_default();
        let registration_present = managed_block_contents(
            &config,
            "# >>> roster sync: codex agents v1",
            "# <<< roster sync: codex agents v1",
        )
        .is_some_and(|contents| contents == expected_codex_registration.trim_end());
        if !registration_present {
            failures.push(
                "codex: native orchestrator registration is missing or drifted in .codex/config.toml"
                    .to_string(),
            );
        }
    }

    let report = json!({
        "binary": {
            "name": spec.binary,
            "found": binary_found,
            "path": binary_path.map(|path| path.display().to_string()),
        },
        "native_config": {
            "present": !native_paths.is_empty(),
            "paths": native_paths,
            "expected": spec.native_config,
        },
        "doctrine": {
            "path": spec.doctrine,
            "present": doctrine_present,
            "kind": doctrine_kind,
        },
        "default_orchestrator": {
            "path": spec.orchestrator,
            "present": orchestrator_present,
            "evidence": if orchestrator_present {
                "named projection"
            } else {
                "missing"
            },
        },
    });
    (report, failures, warnings)
}

fn managed_block_contents<'a>(
    contents: &'a str,
    start_marker: &str,
    end_marker: &str,
) -> Option<&'a str> {
    let start = contents.find(start_marker)? + start_marker.len();
    let rest = contents.get(start..)?.strip_prefix('\n')?;
    let end = rest.find(end_marker)?;
    rest.get(..end)?.strip_suffix('\n')
}

fn inspect_manifest(home: &Path) -> (Value, Vec<String>) {
    let manifest_path = home.join(MANIFEST_REL);
    let mut failures = Vec::new();
    let mut listed = BTreeMap::<String, bool>::new();
    let mut invalid = Vec::new();

    let Ok(contents) = fs::read_to_string(&manifest_path) else {
        failures.push(format!("manifest missing or unreadable: {MANIFEST_REL}"));
        return (
            json!({
                "path": MANIFEST_REL,
                "present": manifest_path.exists(),
                "valid": false,
                "integrity": "missing",
                "listed_paths": empty_path_report(),
            }),
            failures,
        );
    };
    let Ok(value) = serde_json::from_str::<Value>(&contents) else {
        failures.push(format!("manifest is not valid JSON: {MANIFEST_REL}"));
        return (
            json!({
                "path": MANIFEST_REL,
                "present": true,
                "valid": false,
                "integrity": "broken",
                "listed_paths": empty_path_report(),
            }),
            failures,
        );
    };

    for (field, symlink) in [
        ("files", false),
        ("symlinks", true),
        ("managed_blocks", false),
    ] {
        let Some(items) = value.get(field) else {
            if field == "files" {
                failures.push(format!("manifest missing {field} array: {MANIFEST_REL}"));
            }
            continue;
        };
        let Some(items) = items.as_array() else {
            failures.push(format!("manifest {field} is not an array: {MANIFEST_REL}"));
            continue;
        };
        for item in items {
            let Some(relative) = item.as_str() else {
                failures.push(format!("manifest {field} contains a non-string path"));
                continue;
            };
            if !is_safe_relative_path(relative) {
                invalid.push(relative.to_string());
                failures.push(format!("manifest contains unsafe listed path: {relative}"));
                continue;
            }
            if symlink {
                // `roster sync` records every managed path in `files` and
                // repeats symlink paths in `symlinks` so rollback knows the
                // ownership/type. The symlinks list is authoritative here.
                listed.insert(relative.to_string(), true);
            } else {
                listed.entry(relative.to_string()).or_insert(false);
            }
        }
    }

    let mut present = Vec::new();
    let mut missing = Vec::new();
    let mut wrong_type = Vec::new();
    for (relative, expects_symlink) in &listed {
        let path = home.join(relative);
        let Some(metadata) = fs::symlink_metadata(&path).ok() else {
            missing.push(relative.clone());
            failures.push(format!("manifest path missing: {relative}"));
            continue;
        };
        let valid = if *expects_symlink {
            metadata.file_type().is_symlink() && path.exists()
        } else {
            metadata.is_file()
        };
        if !valid {
            wrong_type.push(relative.clone());
            failures.push(format!(
                "manifest path has wrong type or broken target: {relative}"
            ));
            continue;
        }
        present.push(relative.clone());
    }

    let path_report = json!({
        "total": listed.len() + invalid.len(),
        "present": present.len(),
        "missing": missing,
        "invalid": invalid,
        "wrong_type": wrong_type,
    });
    (
        json!({
            "path": MANIFEST_REL,
            "present": true,
            "valid": failures.is_empty(),
            "schema_version": value.get("schema_version").and_then(Value::as_str),
            "integrity": if failures.is_empty() { "ok" } else { "broken" },
            "listed_paths": path_report,
        }),
        failures,
    )
}

fn empty_path_report() -> Value {
    json!({
        "total": 0,
        "present": 0,
        "missing": [],
        "invalid": [],
        "wrong_type": [],
    })
}

fn is_safe_relative_path(relative: &str) -> bool {
    let path = Path::new(relative);
    !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
}

fn find_binary(binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(binary))
        .find(|candidate| {
            fs::metadata(candidate)
                .ok()
                .is_some_and(|metadata| metadata.is_file() && is_executable(&metadata))
        })
}

fn inspect_mcp_policy(home: &Path, mcps: &[Mcp]) -> (Value, Vec<String>, Vec<String>) {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let disabled = mcps
        .iter()
        .filter(|mcp| mcp.status == "disabled")
        .map(|mcp| mcp.id.as_str())
        .collect::<Vec<_>>();

    let claude = read_json(home.join(".claude.json"), &mut warnings);
    let omp = read_json(home.join(".omp/agent/mcp.json"), &mut warnings);
    let codex = fs::read_to_string(home.join(".codex/config.toml")).unwrap_or_default();
    let omp_disabled = omp
        .as_ref()
        .and_then(|value| value.get("disabledServers"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();

    let mut active_disabled = Vec::new();
    let mut suppressed = Vec::new();
    for id in &disabled {
        if claude
            .as_ref()
            .is_some_and(|value| claude_config_has_mcp_server(value, id))
        {
            active_disabled.push(format!("claude:{id}"));
        }
        if codex_mcp_is_active(&codex, id) {
            active_disabled.push(format!("codex:{id}"));
        }
        if omp.as_ref().is_some_and(|value| {
            top_level_mcp_servers(value).is_some_and(|servers| servers.contains_key(*id))
        }) {
            if omp_disabled.contains(id) {
                suppressed.push(format!("omp:{id}"));
            } else {
                active_disabled.push(format!("omp:{id}"));
            }
        }
    }
    for binding in &active_disabled {
        failures.push(format!(
            "MCP policy drift: registry-disabled server remains active at {binding}"
        ));
    }

    let mut status_counts = BTreeMap::<&str, usize>::new();
    for mcp in mcps {
        *status_counts.entry(mcp.status.as_str()).or_default() += 1;
    }
    (
        json!({
            "catalog_entries": mcps.len(),
            "status_counts": status_counts,
            "disabled_ids": disabled,
            "active_disabled": active_disabled,
            "suppressed": suppressed,
        }),
        failures,
        warnings,
    )
}

fn read_json(path: PathBuf, warnings: &mut Vec<String>) -> Option<Value> {
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => {
            warnings.push(format!("MCP policy could not read {}", path.display()));
            return None;
        }
    };
    match serde_json::from_str(&contents) {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push(format!(
                "MCP policy found invalid JSON at {}",
                path.display()
            ));
            None
        }
    }
}

fn top_level_mcp_servers(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value.get("mcpServers").and_then(Value::as_object)
}

fn claude_config_has_mcp_server(value: &Value, id: &str) -> bool {
    // Claude user-scoped MCPs are top-level; local-scoped MCPs are stored
    // under `projects.<path>.mcpServers`. There is no disabled flag for these
    // entries: `claude mcp remove` removes them. The distinct
    // `disabledMcpjsonServers` setting applies to project `.mcp.json` files,
    // not the entries represented in this user config.
    top_level_mcp_servers(value).is_some_and(|servers| servers.contains_key(id))
        || value
            .get("projects")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|projects| projects.values())
            .filter_map(top_level_mcp_servers)
            .any(|servers| servers.contains_key(id))
}

fn codex_mcp_is_active(contents: &str, id: &str) -> bool {
    let headers = [
        format!("[mcp_servers.{id}]"),
        format!("[mcp_servers.\"{id}\"]"),
    ];
    let mut in_section = false;
    let mut found = false;
    let mut enabled = true;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if in_section {
                break;
            }
            in_section = headers.iter().any(|header| line == header);
            found |= in_section;
            continue;
        }
        if in_section && line == "enabled = false" {
            enabled = false;
        }
    }
    found && enabled
}

fn inspect_live(home: &Path) -> (Value, Vec<String>, Vec<String>) {
    const HARNESS_PROBE_TIMEOUT: Duration = Duration::from_secs(40);
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut probes = BTreeMap::new();
    let mut runtime_versions = BTreeMap::new();

    for (name, binary) in [("claude", "claude"), ("codex", "codex"), ("omp", "omp")] {
        let probe = run_bounded(home, binary, &["--version"], Duration::from_secs(5));
        runtime_versions.insert(name, extract_semver(&probe.stdout));
    }

    let probe_specs = [
        ("codex", "codex", &["doctor", "--json"][..], true),
        ("claude_mcp", "claude", &["mcp", "list"][..], true),
        ("omp_config", "omp", &["config", "path"][..], true),
        ("qmd", "qmd", &["status"][..], false),
    ];
    let completed = thread::scope(|scope| {
        probe_specs
            .into_iter()
            .map(|(name, binary, args, required)| {
                scope.spawn(move || {
                    (
                        name,
                        binary,
                        required,
                        run_bounded(home, binary, args, HARNESS_PROBE_TIMEOUT),
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("bounded probe thread"))
            .collect::<Vec<_>>()
    });
    for (name, binary, required, probe) in completed {
        let claude_failed = if name == "claude_mcp" {
            probe.stdout.matches("Failed to connect").count()
        } else {
            0
        };
        let claude_auth = if name == "claude_mcp" {
            probe.stdout.matches("Needs authentication").count()
        } else {
            0
        };
        if probe.timed_out {
            failures.push(format!(
                "live probe {name} timed out after {} seconds",
                HARNESS_PROBE_TIMEOUT.as_secs()
            ));
        } else if probe.found && probe.exit_code != Some(0) {
            failures.push(format!(
                "live probe {name} exited {}",
                probe
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "without a code".to_string())
            ));
        } else if !probe.found && required {
            failures.push(format!("live probe {name} could not find {binary} on PATH"));
        } else if !probe.found {
            warnings.push(format!(
                "optional live probe {name} skipped; {binary} not on PATH"
            ));
        }
        if claude_failed > 0 {
            warnings.push(format!(
                "Claude MCP health reports {claude_failed} failed external connection(s)"
            ));
        }
        if claude_auth > 0 {
            warnings.push(format!(
                "Claude MCP health reports {claude_auth} connection(s) awaiting authentication"
            ));
        }
        let mut report = probe.as_json();
        if name == "claude_mcp" {
            report["reported_failed_connections"] = json!(claude_failed);
            report["reported_auth_required"] = json!(claude_auth);
        }
        probes.insert(name, report);
    }

    let disk = run_bounded(
        home,
        "df",
        &["-Pk", &home.to_string_lossy()],
        Duration::from_secs(5),
    );
    let disk_percent = disk
        .stdout
        .lines()
        .last()
        .and_then(|line| line.split_whitespace().nth(4))
        .and_then(|value| value.trim_end_matches('%').parse::<u8>().ok());
    if let Some(percent) = disk_percent.filter(|percent| *percent >= 90) {
        warnings.push(format!(
            "home volume is {percent}% used; review workstation disk pressure"
        ));
    }

    let launchd = run_bounded(
        home,
        "sh",
        &[
            "-c",
            "launchctl print gui/$(id -u) 2>/dev/null | awk '/state = failed/{n++} /last exit code = [1-9]/{n++} END{print n+0}'",
        ],
        Duration::from_secs(5),
    );
    let launchd_failures = launchd.stdout.trim().parse::<usize>().ok();
    if let Some(count) = launchd_failures.filter(|count| *count > 0) {
        warnings.push(format!(
            "launchd reports {count} failed/nonzero service rows"
        ));
    }

    let mcp_processes = run_bounded(
        home,
        "sh",
        &[
            "-c",
            "pgrep -fl 'mcp|model-context-protocol' 2>/dev/null | wc -l | tr -d ' '",
        ],
        Duration::from_secs(5),
    );
    let mcp_process_count = mcp_processes.stdout.trim().parse::<usize>().ok();

    (
        json!({
            "enabled": true,
            "runtime_versions": runtime_versions,
            "probes": probes,
            "disk": {
                "used_percent": disk_percent,
                "probe": disk.as_json(),
            },
            "launchd": {
                "failed_rows": launchd_failures,
                "probe": launchd.as_json(),
            },
            "mcp_processes": {
                "count": mcp_process_count,
                "probe": mcp_processes.as_json(),
            },
        }),
        failures,
        warnings,
    )
}

fn extract_semver(contents: &str) -> Option<String> {
    contents
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_ascii_digit() && character != '.' && character != 'v'
            })
        })
        .map(|token| token.trim_start_matches('v'))
        .find(|token| {
            token.split('.').count() >= 3
                && token
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == '.')
        })
        .map(ToOwned::to_owned)
}

struct ProbeResult {
    found: bool,
    timed_out: bool,
    exit_code: Option<i32>,
    elapsed_ms: u128,
    stdout: String,
}

impl ProbeResult {
    fn as_json(&self) -> Value {
        json!({
            "found": self.found,
            "timed_out": self.timed_out,
            "exit_code": self.exit_code,
            "elapsed_ms": self.elapsed_ms,
        })
    }
}

fn run_bounded(home: &Path, binary: &str, args: &[&str], timeout: Duration) -> ProbeResult {
    let started = Instant::now();
    let mut command = Command::new(binary);
    command
        .args(args)
        .env("HOME", home)
        .env_remove("CODEX_HOME")
        .env_remove("PI_CONFIG_DIR")
        .env_remove("PI_CODING_AGENT_DIR")
        .env_remove("OMP_PROFILE")
        .env_remove("PI_PROFILE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    let Ok(mut child) = command.spawn() else {
        return ProbeResult {
            found: false,
            timed_out: false,
            exit_code: None,
            elapsed_ms: started.elapsed().as_millis(),
            stdout: String::new(),
        };
    };

    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| thread::spawn(move || read_bounded_stdout(stdout)));

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return ProbeResult {
                    found: true,
                    timed_out: false,
                    exit_code: status.code(),
                    elapsed_ms: started.elapsed().as_millis(),
                    stdout: join_stdout(stdout_reader),
                };
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let status = terminate_process_group(&mut child);
                return ProbeResult {
                    found: true,
                    timed_out: true,
                    exit_code: status.and_then(|value| value.code()),
                    elapsed_ms: started.elapsed().as_millis(),
                    stdout: join_stdout(stdout_reader),
                };
            }
            Err(_) => {
                let _ = terminate_process_group(&mut child);
                return ProbeResult {
                    found: true,
                    timed_out: false,
                    exit_code: None,
                    elapsed_ms: started.elapsed().as_millis(),
                    stdout: join_stdout(stdout_reader),
                };
            }
        }
    }
}

fn terminate_process_group(child: &mut std::process::Child) -> Option<std::process::ExitStatus> {
    #[cfg(unix)]
    unsafe {
        // The child leads its own process group. Killing the group closes
        // inherited stdout handles held by grandchildren, so joining the
        // bounded reader cannot outlive the timeout.
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    child.wait().ok()
}

fn read_bounded_stdout(mut stdout: ChildStdout) -> String {
    const RETAIN_LIMIT: usize = 64 * 1024;
    let mut retained = Vec::new();
    let mut chunk = [0_u8; 4096];
    while let Ok(read) = stdout.read(&mut chunk) {
        if read == 0 {
            break;
        }
        let remaining = RETAIN_LIMIT.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..read.min(remaining)]);
    }
    String::from_utf8_lossy(&retained).into_owned()
}

fn join_stdout(reader: Option<thread::JoinHandle<String>>) -> String {
    reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default()
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

fn print_human(report: &Value) {
    println!(
        "roster doctor: {}",
        report["status"].as_str().unwrap_or("failed")
    );
    println!(
        "source: root={} home={}",
        report["source"]["root"].as_str().unwrap_or("?"),
        report["source"]["home"].as_str().unwrap_or("?")
    );
    println!(
        "manifest: {}",
        report["manifest"]["integrity"].as_str().unwrap_or("broken")
    );
    for spec in HARNESSES {
        let harness = &report["harnesses"][spec.name];
        println!(
            "{}: binary={} native={} doctrine={} orchestrator={}",
            spec.name,
            if harness["binary"]["found"].as_bool().unwrap_or(false) {
                "found"
            } else {
                "missing"
            },
            if harness["native_config"]["present"]
                .as_bool()
                .unwrap_or(false)
            {
                "ok"
            } else {
                "missing"
            },
            if harness["doctrine"]["present"].as_bool().unwrap_or(false) {
                "ok"
            } else {
                "missing"
            },
            if harness["default_orchestrator"]["present"]
                .as_bool()
                .unwrap_or(false)
            {
                "ok"
            } else {
                "missing"
            },
        );
    }
    for warning in report["warnings"].as_array().into_iter().flatten() {
        if let Some(warning) = warning.as_str() {
            println!("WARN {warning}");
        }
    }
    for failure in report["failures"].as_array().into_iter().flatten() {
        if let Some(failure) = failure.as_str() {
            println!("FAIL {failure}");
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn bounded_probe_kills_descendants_that_hold_stdout_open() {
        let started = Instant::now();
        let result = run_bounded(
            Path::new("/tmp"),
            "sh",
            &["-c", "(sleep 60) & sleep 60"],
            Duration::from_millis(200),
        );
        assert!(result.timed_out);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

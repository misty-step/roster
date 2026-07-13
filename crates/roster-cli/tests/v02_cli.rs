use assert_cmd::Command;
use predicates::prelude::*;
use std::{
    fs, os::unix::fs::PermissionsExt, path::Path, process::Command as StdCommand, thread,
    time::Duration,
};

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, body).expect("fixture");
}

fn fixture(root: &Path) -> std::path::PathBuf {
    write(
        &root.join("source/roles/orchestrator.yaml"),
        "schema_version: roster.role.v2\nname: orchestrator\ndescription: Coordinate work.\ninclude:\n  - core/guidance:lead\n  - core/skill:deliver\n",
    );
    write(
        &root.join("source/primitives/guidance/lead.md"),
        "# Lead\n\nCoordinate from evidence.\n",
    );
    write(
        &root.join("source/primitives/skills/deliver/SKILL.md"),
        "---\nname: deliver\ndescription: Deliver work.\n---\n\nDeliver.\n",
    );
    write(
        &root.join("source/primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: fixture\nmcps: []\n",
    );
    let config = root.join("config.yaml");
    write(
        &config,
        &format!(
            "schema_version: roster.config.v1\nsources:\n  core: {}\nagents:\n  amos:\n    description: Codex lead\n    role: core/role:orchestrator\n    model: gpt-test\n    reasoning: high\n    harness: codex\n    args: [--search]\n    delegates: []\n",
            root.join("source").display()
        ),
    );
    config
}

#[test]
fn list_and_show_use_the_explicit_config() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    Command::cargo_bin("roster")
        .expect("bin")
        .args(["--config", config.to_str().unwrap(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "amos\tcodex\tgpt-test\tCodex lead",
        ));
    Command::cargo_bin("roster")
        .expect("bin")
        .args(["--config", config.to_str().unwrap(), "show", "amos"])
        .assert()
        .success()
        .stdout(predicate::str::contains("core/skill:deliver"));
}

#[test]
fn list_keeps_an_invalid_agent_visible_but_disabled() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let body = fs::read_to_string(&config).expect("config");
    write(
        &config,
        &format!(
            "{body}  broken:\n    description: Broken agent remains inspectable\n    role: core/role:missing\n    model: gpt-test\n    harness: codex\n    args: []\n    delegates: []\n"
        ),
    );
    Command::cargo_bin("roster")
        .expect("bin")
        .args(["--config", config.to_str().unwrap(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "amos\tcodex\tgpt-test\tCodex lead",
        ))
        .stdout(predicate::str::contains(
            "broken\tcodex\tgpt-test\tBroken agent remains inspectable\tDISABLED:",
        ));
}

#[test]
fn resolve_writes_an_exact_bundle_and_reports_its_manifest() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let bundle = temp.path().join("bundle");
    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "resolve",
            "amos",
            "--output",
            bundle.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("agent: amos"));
    assert!(bundle.join("skills/deliver/SKILL.md").is_file());
}

#[test]
fn dispatch_dry_run_is_transparent_and_rescue_has_no_roster_context() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let state = temp.path().join("state");
    Command::cargo_bin("roster")
        .expect("bin")
        .env("ROSTER_STATE_DIR", &state)
        .args([
            "--config",
            config.to_str().unwrap(),
            "dispatch",
            "amos",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("CODEX_HOME="))
        .stdout(predicate::str::contains("codex"))
        .stdout(predicate::str::contains("--disable apps"))
        .stdout(predicate::str::contains("gpt-test"));
    assert!(
        !state.join("runs").exists()
            || fs::read_dir(state.join("runs"))
                .expect("runs")
                .next()
                .is_none(),
        "dry run bundle must be ephemeral"
    );

    Command::cargo_bin("roster")
        .expect("bin")
        .args(["rescue", "claude", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude --safe-mode"))
        .stdout(predicate::str::contains("ROSTER").not());

    Command::cargo_bin("roster")
        .expect("bin")
        .env("ROSTER_STATE_DIR", &state)
        .args(["rescue", "omp", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PI_CODING_AGENT_DIR="))
        .stdout(predicate::str::contains("--no-skills"))
        .stdout(predicate::str::contains("--config"));
    assert!(
        fs::read_to_string(state.join("rescue/omp/isolation.yml"))
            .expect("OMP rescue isolation")
            .contains("setupVersion: 1"),
        "isolated OMP launches must bypass the first-run wizard"
    );
}

fn fake_codex(directory: &Path, launch: &str) {
    let path = directory.join("codex");
    write(
        &path,
        &format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'codex-cli 0.144.3'; exit 0; fi\nif [ \"$1\" = mcp ]; then printf '[]\\n'; exit 0; fi\n{launch}\n"
        ),
    );
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn fake_harness(directory: &Path, name: &str, script: &str) {
    let path = directory.join(name);
    write(&path, &format!("#!/bin/sh\n{script}\n"));
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

#[test]
fn dispatch_preserves_child_exit_code_and_cleans_failed_run() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_codex(&bin, "exit 42");
    let state = temp.path().join("state");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_STATE_DIR", &state)
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .code(42);
    assert!(
        !state.join("runs").exists()
            || fs::read_dir(state.join("runs"))
                .expect("runs")
                .next()
                .is_none(),
        "failed run bundle must be cleaned"
    );
}

#[test]
fn termination_signal_is_forwarded_to_the_harness_child() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let ready = temp.path().join("ready");
    let caught = temp.path().join("caught");
    fake_codex(
        &bin,
        "trap 'touch \"$FAKE_CAUGHT\"; exit 143' TERM\ntouch \"$FAKE_READY\"\nwhile :; do sleep 1; done",
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let roster = assert_cmd::cargo::cargo_bin!("roster");
    let mut child = StdCommand::new(roster)
        .env("PATH", path)
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .env("FAKE_READY", &ready)
        .env("FAKE_CAUGHT", &caught)
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .spawn()
        .expect("spawn roster");
    for _ in 0..100 {
        if ready.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(ready.exists(), "fake Harness never started");
    // SAFETY: child.id is the live Roster process created above.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let status = child.wait().expect("wait roster");
    assert_eq!(status.code(), Some(143));
    assert!(caught.exists(), "Harness child did not receive SIGTERM");
}

#[test]
fn claude_preflight_is_version_pinned_and_launches_in_the_selected_workspace() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let body = fs::read_to_string(&config)
        .expect("config")
        .replace("harness: codex", "harness: claude")
        .replace("args: [--search]", "args: []");
    write(&config, &body);
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_harness(
        &bin,
        "claude",
        "if [ \"$1\" = --version ]; then echo '2.1.207 (Claude Code)'; exit 0; fi\nif [ \"$1\" = plugin ]; then exit 0; fi\nprintf '%s' \"$PWD\" > \"$FAKE_PWD\"",
    );
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let observed = temp.path().join("observed-pwd");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("FAKE_PWD", &observed)
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .args([
            "--config",
            config.to_str().unwrap(),
            "--cwd",
            workspace.to_str().unwrap(),
            "dispatch",
            "amos",
        ])
        .assert()
        .success();
    assert_eq!(
        fs::canonicalize(fs::read_to_string(observed).expect("observed cwd"))
            .expect("canonical observed cwd"),
        workspace.canonicalize().expect("canonical workspace")
    );
}

#[test]
fn omp_preflight_reads_the_live_runtime_inventory() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let body = fs::read_to_string(&config)
        .expect("config")
        .replace("harness: codex", "harness: omp")
        .replace("args: [--search]", "args: []");
    write(&config, &body);
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_harness(
        &bin,
        "omp",
        r#"if [ "$1" = --version ]; then echo 'omp v16.4.4'; exit 0; fi
prompt=''
append_seen=false
append='undeclared'
rpc=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --system-prompt) shift; prompt="$1" ;;
    --append-system-prompt) shift; append_seen=true; append="$1" ;;
    --mode) shift; [ "$1" = rpc ] && rpc=true ;;
  esac
  shift
done
if $rpc; then
  $append_seen || exit 92
  [ -z "$append" ] || exit 93
  case "$prompt" in
    *"Coordinate from evidence."*) ;;
    *) exit 91 ;;
  esac
  read request
  printf '%s\n' '{"type":"ready"}'
  python3 -c 'import json,re,sys; normalized = re.sub(r"\n{3,}", "\n", sys.argv[1]); print(json.dumps({"id":"roster-preflight","type":"response","command":"get_state","success":True,"data":{"model":{"id":"gpt-test"},"thinkingLevel":"high","systemPrompt":[normalized + "\n# Runtime context","<skill name=\"deliver\">"],"dumpTools":[]}}))' "$prompt"
fi"#,
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .success();
}

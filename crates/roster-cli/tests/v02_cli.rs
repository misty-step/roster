use assert_cmd::Command;
use predicates::prelude::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
    process::Command as StdCommand,
    thread,
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
            "schema_version: roster.config.v1\nsources:\n  core: {}\nagents:\n  amos:\n    description: Codex lead\n    role: core/role:orchestrator\n    model: gpt-test\n    reasoning: high\n    harness: codex\n    args: [--search]\n",
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
fn dispatch_selects_an_explicit_harness_default() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let body = fs::read_to_string(&config).expect("config");
    write(
        &config,
        &body.replace("sources:\n", "defaults:\n  codex: amos\nsources:\n"),
    );

    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "dispatch",
            "--default",
            "codex",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ROSTER_AGENT=amos"))
        .stdout(predicate::str::contains(" codex --strict-config"));
}

#[test]
fn dispatch_default_fails_clearly_when_unconfigured() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "dispatch",
            "--default",
            "claude",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no default claude agent"));
}

#[test]
fn harness_defaults_always_use_nearest_config_discovery() {
    let temp = tempfile::tempdir().expect("temp");
    let inherited = fixture(temp.path());
    let inherited_body = fs::read_to_string(&inherited).expect("inherited config");
    write(
        &inherited,
        &inherited_body.replace("sources:\n", "defaults:\n  codex: amos\nsources:\n"),
    );
    let workspace = temp.path().join("work/r90/project");
    let local = temp.path().join("work/r90/.roster/config.yaml");
    write(
        &local,
        &inherited_body
            .replace("sources:\n", "defaults:\n  codex: odysseus\nsources:\n")
            .replace("  amos:\n", "  odysseus:\n"),
    );
    fs::create_dir_all(&workspace).expect("workspace");

    Command::cargo_bin("roster")
        .expect("bin")
        .env("ROSTER_CONFIG", &inherited)
        .args([
            "--cwd",
            workspace.to_str().unwrap(),
            "dispatch",
            "--default",
            "codex",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ROSTER_AGENT=odysseus"))
        .stdout(predicate::str::contains("ROSTER_AGENT=amos").not());

    let home = temp.path().join("home");
    let home_config = home.join(".roster/config.yaml");
    write(
        &home_config,
        &fs::read_to_string(&inherited).expect("home config"),
    );
    let configless = home.join("elsewhere");
    fs::create_dir_all(&configless).expect("home workspace");
    Command::cargo_bin("roster")
        .expect("bin")
        .env("HOME", &home)
        .env("ROSTER_CONFIG", &local)
        .args([
            "--cwd",
            configless.to_str().unwrap(),
            "dispatch",
            "--default",
            "codex",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ROSTER_AGENT=amos"));
}

#[test]
fn list_keeps_an_invalid_agent_visible_but_disabled() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let body = fs::read_to_string(&config).expect("config");
    write(
        &config,
        &format!(
            "{body}  broken:\n    description: Broken agent remains inspectable\n    role: core/role:missing\n    model: gpt-test\n    harness: codex\n    args: []\n"
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
        .stdout(predicate::str::contains("schema_version: roster.bundle.v2"))
        .stdout(predicate::str::contains("agent: amos"))
        .stdout(predicate::str::contains("binding: amos"))
        .stdout(predicate::str::contains("purpose: Codex lead"));
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
        .stderr(predicate::str::contains(
            "Dry run only: live adapter preflight was not executed.",
        ))
        .stdout(predicate::str::contains("&& env -i "))
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

    let home = temp.path().join("home");
    write(&home.join(".codex/auth.json"), "{}\n");
    Command::cargo_bin("roster")
        .expect("bin")
        .env("HOME", &home)
        .env("ROSTER_STATE_DIR", &state)
        .args(["rescue", "codex", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CODEX_HOME="))
        .stdout(predicate::str::contains("--disable apps"));
}

#[test]
fn rescue_uses_the_same_clean_declared_environment() {
    let temp = tempfile::tempdir().expect("temp");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_harness(
        &bin,
        "claude",
        r#"test -z "${SHOULD_NOT_SURVIVE:-}"
test -z "${OPENAI_API_KEY:-}"
test "$MINT_BASE_URL" = "http://mint.test"
test "$1" = --safe-mode"#,
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("SHOULD_NOT_SURVIVE", "ambient-canary")
        .env("OPENAI_API_KEY", "ambient-secret")
        .env("ROSTER_CHILD_ENV_MINT_BASE_URL", "http://mint.test")
        .args(["rescue", "claude"])
        .assert()
        .success();
}

#[test]
fn resolve_and_dispatch_accept_one_explicit_ad_hoc_composition() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let config_before = fs::read(&config).expect("config before");
    let bundle = temp.path().join("bundle");

    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "resolve",
            "--using",
            "amos",
            "--as",
            "one-off",
            "--purpose",
            "Verify one exact seam.",
            "--include",
            "core/skill:deliver",
            "--output",
            bundle.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("agent: one-off"))
        .stdout(predicate::str::contains("binding: amos"))
        .stdout(predicate::str::contains("role: ad-hoc"));
    let agents = fs::read_to_string(bundle.join("AGENTS.md")).expect("AGENTS.md");
    assert!(agents.contains("# one-off"));
    assert!(agents.contains("Verify one exact seam."));
    assert!(agents.contains("skills/deliver/SKILL.md"));
    assert!(!agents.contains("Coordinate work."));

    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "resolve",
            "--using",
            "amos",
            "--as",
            "one-off",
            "--purpose",
            "Verify one exact seam.",
            "--include",
            "core/skill:deliver",
            "--output",
            bundle.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "bundle destination already exists",
        ));

    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "dispatch",
            "--using",
            "amos",
            "--as",
            "one-off",
            "--purpose",
            "Verify one exact seam.",
            "--include",
            "core/skill:deliver",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ROSTER_AGENT=one-off"))
        .stdout(predicate::str::contains("gpt-test"));

    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            config.to_str().unwrap(),
            "dispatch",
            "--using",
            "amos",
            "--as",
            "one-off",
            "--purpose",
            "missing includes",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--include"));
    assert_eq!(fs::read(&config).expect("config after"), config_before);
    assert!(
        !temp.path().join("source/roles/one-off.yaml").exists(),
        "ad-hoc resolution must not persist a generated role"
    );
}

#[test]
fn ad_hoc_dispatch_receipt_retains_the_exact_effective_composition() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_codex(&bin, "exit 0");
    let state = temp.path().join("state");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_STATE_DIR", &state)
        .args([
            "--config",
            config.to_str().unwrap(),
            "--cwd",
            temp.path().to_str().unwrap(),
            "dispatch",
            "--using",
            "amos",
            "--as",
            "one-off",
            "--purpose",
            "Verify one exact seam.",
            "--include",
            "core/skill:deliver",
        ])
        .assert()
        .success();

    let receipt_path = fs::read_dir(state.join("receipts"))
        .expect("receipts")
        .next()
        .expect("one receipt")
        .expect("receipt entry")
        .path();
    let receipt = fs::read_to_string(receipt_path).expect("receipt body");
    assert!(receipt.contains("schema_version: roster.receipt.v2"));
    assert!(receipt.contains("agent: one-off"));
    assert!(receipt.contains("binding: amos"));
    assert!(receipt.contains("role: ad-hoc"));
    assert!(receipt.contains("purpose: Verify one exact seam."));
    assert!(receipt.contains("reasoning: high"));
    assert!(receipt.contains("- --search"));
    assert!(receipt.contains("identity: core/skill:deliver"));
    assert!(receipt.contains("ad-hoc/role:one-off"));
    assert!(receipt.contains("guidance: []"));
    assert!(receipt.contains("mcps: []"));
    assert!(receipt.contains("AGENTS.md: sha256:"));

    let receipt: serde_yaml::Value = serde_yaml::from_str(&receipt).expect("receipt yaml");
    assert!(receipt["bundle"].is_null());
    assert!(
        !state.join("runs").exists()
            || fs::read_dir(state.join("runs"))
                .expect("runs")
                .next()
                .is_none(),
        "default-deleted dispatch must retain evidence in the receipt, not the run tree"
    );
}

fn fake_codex(directory: &Path, launch: &str) {
    let path = directory.join("codex");
    write(
        &path,
        &r#"#!/bin/sh
test -z "${SHOULD_NOT_SURVIVE:-}" || exit 67
test -z "${OPENAI_API_KEY:-}" || exit 68
test -z "${OP_SERVICE_ACCOUNT_TOKEN:-}" || exit 69
test "${CODEX_HOME:-}" != /ambient-codex-home || exit 66
if [ -n "${EXPECT_MINT_BASE_URL:-}" ]; then
  test "${MINT_BASE_URL:-}" = "$EXPECT_MINT_BASE_URL" || exit 65
fi
if [ "$1" = --version ]; then
  echo 'codex-cli 9.9.9'
  exit 0
fi
if [ "$1" = app-server ]; then
  [ "$2" = --strict-config ] || exit 76
  [ "$3" = --listen ] || exit 77
  [ "$4" = stdio:// ] || exit 78
  [ -z "$FAKE_PREFLIGHT" ] || touch "$FAKE_PREFLIGHT"
  if [ -n "$FAKE_SYSTEM_SKILL" ]; then
    mkdir -p "$CODEX_HOME/skills/.system/intrinsic"
    printf '%s\n' intrinsic > "$CODEX_HOME/skills/.system/intrinsic/SKILL.md"
    printf '%s\n' projection-mutation >> "$CODEX_HOME/skills/deliver/SKILL.md"
    printf '%s\n' projection-agent-mutation >> "$CODEX_HOME/AGENTS.md"
  fi
  while IFS= read -r request; do
    case "$request" in
      *'"id":"roster-skills"'*)
        printf '{"id":"roster-skills","result":{"data":[{"cwd":"fixture","skills":[{"name":"deliver","path":"%s","scope":"user","enabled":true}],"errors":[]}]}}\n' \
          "$CODEX_HOME/skills/deliver/SKILL.md"
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = mcp ]; then
  printf '[]\n'
  exit 0
fi
__LAUNCH__
"#
        .replace("__LAUNCH__", launch),
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
fn codex_projection_preserves_native_state_and_blocks_project_config() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    fs::create_dir_all(workspace.join(".git")).expect("git marker");
    let selected_workspace = workspace.join("nested");
    fs::create_dir_all(&selected_workspace).expect("nested workspace");
    write(
        &workspace.join(".codex/config.toml"),
        "[features]\nretired_project_field = true\n",
    );
    let ambient_skill = temp.path().join("ambient/foreign/SKILL.md");
    write(
        &ambient_skill,
        "---\nname: foreign\ndescription: Ambient skill.\n---\n\nAmbient.\n",
    );
    fs::create_dir_all(workspace.join(".agents/skills")).expect("ambient skill root");
    symlink(
        ambient_skill.parent().expect("ambient skill parent"),
        workspace.join(".agents/skills/foreign"),
    )
    .expect("ambient skill symlink");
    let home = temp.path().join("home");
    let codex_home = home.join(".codex");
    fs::create_dir_all(codex_home.join("sessions")).expect("sessions");
    fs::create_dir_all(codex_home.join("archived_sessions")).expect("archive");
    write(&codex_home.join("auth.json"), "{}\n");
    write(&codex_home.join("sessions/global.jsonl"), "ambient\n");
    write(
        &codex_home.join("archived_sessions/global.jsonl"),
        "ambient\n",
    );
    write(&codex_home.join("history.jsonl"), "ambient\n");
    write(&codex_home.join("session_index.jsonl"), "ambient\n");
    let trust_config = format!(
        "[projects.\"{workspace}\"]\ntrust_level = \"trusted\"\n",
        workspace = workspace.display()
    );
    assert!(!trust_config.contains("\\\""), "{trust_config:?}");
    write(&codex_home.join("config.toml"), &trust_config);

    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_codex(
        &bin,
        r#"test -L "$CODEX_HOME/auth.json" || exit 70
test -L "$CODEX_HOME/sessions" || exit 71
test -L "$CODEX_HOME/archived_sessions" || exit 72
test -L "$CODEX_HOME/history.jsonl" || exit 73
test -L "$CODEX_HOME/session_index.jsonl" || exit 74
test "$CODEX_SQLITE_HOME" = "$HOME/.codex" || exit 75
grep -F 'trust_level = "untrusted"' "$CODEX_HOME/config.toml" >/dev/null || exit 79
grep -F 'project_doc_max_bytes = 0' "$CODEX_HOME/config.toml" >/dev/null || exit 80
run_root=$(dirname "$(dirname "$CODEX_HOME")")
test ! -e "$run_root/bundle/skills/.system" || exit 81
test -f "$CODEX_HOME/skills/.system/intrinsic/SKILL.md" || exit 82
test ! -L "$CODEX_HOME/skills/deliver" || exit 83
grep -F projection-mutation "$run_root/bundle/skills/deliver/SKILL.md" >/dev/null && exit 84
grep -F projection-agent-mutation "$run_root/bundle/AGENTS.md" >/dev/null && exit 85
cp "$CODEX_HOME/config.toml" "$FAKE_CONFIG""#,
    );
    let observed = temp.path().join("observed-config.toml");
    let strict_preflight = temp.path().join("strict-preflight");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("HOME", &home)
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_CONFIG", &observed)
        .env("ROSTER_CHILD_ENV_FAKE_PREFLIGHT", &strict_preflight)
        .env("ROSTER_CHILD_ENV_FAKE_SYSTEM_SKILL", "1")
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .args([
            "--config",
            config.to_str().unwrap(),
            "--cwd",
            selected_workspace.to_str().unwrap(),
            "dispatch",
            "amos",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Preparing amos (codex)"))
        .stderr(predicate::str::contains("Launching amos (codex)"));
    assert!(
        strict_preflight.is_file(),
        "dispatch must strict-parse the exact Codex projection before launch"
    );

    let observed = fs::read_to_string(observed).expect("observed config");
    let projected: toml::Value = toml::from_str(&observed).expect("valid projected config");
    let project_root = workspace
        .canonicalize()
        .expect("canonical workspace")
        .display()
        .to_string();
    assert_eq!(
        projected
            .get("projects")
            .and_then(toml::Value::as_table)
            .and_then(|projects| projects.get(&project_root))
            .and_then(|project| project.get("trust_level"))
            .and_then(toml::Value::as_str),
        Some("untrusted")
    );
    assert_eq!(
        projected
            .get("skills")
            .and_then(|skills| skills.get("bundled"))
            .and_then(|bundled| bundled.get("enabled"))
            .and_then(toml::Value::as_bool),
        Some(false)
    );
    let ambient_skill = ambient_skill
        .canonicalize()
        .expect("canonical ambient skill")
        .display()
        .to_string();
    assert!(
        projected
            .get("skills")
            .and_then(|skills| skills.get("config"))
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .any(|rule| {
                rule.get("path").and_then(toml::Value::as_str) == Some(ambient_skill.as_str())
                    && rule.get("enabled").and_then(toml::Value::as_bool) == Some(false)
            }),
        "symlinked ambient skills must be disabled by canonical identity"
    );
}

#[test]
fn codex_strict_configuration_failure_stops_before_launch() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).expect("workspace");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_harness(
        &bin,
        "codex",
        r#"if [ "$1" = --version ]; then echo 'codex-cli 0.144.3'; exit 0; fi
if [ "$1" = app-server ]; then echo 'unknown configuration field fixture' >&2; exit 81; fi
touch "$FAKE_LAUNCHED""#,
    );
    let launched = temp.path().join("launched");
    let state = temp.path().join("state");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_LAUNCHED", &launched)
        .env("ROSTER_STATE_DIR", &state)
        .args([
            "--config",
            config.to_str().unwrap(),
            "--cwd",
            workspace.to_str().unwrap(),
            "dispatch",
            "amos",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Codex strict configuration preflight failed: unknown configuration field fixture",
        ))
        .stderr(predicate::str::contains("Launching amos (codex)").not());
    assert!(
        !launched.exists(),
        "the Harness must not launch after strict configuration preflight failure"
    );
    let receipts = fs::read_dir(state.join("receipts"))
        .expect("failed preflight receipts")
        .collect::<Result<Vec<_>, _>>()
        .expect("receipt entries");
    assert_eq!(receipts.len(), 1, "failed preflight must write one receipt");
    let receipt = fs::read_to_string(receipts[0].path()).expect("preflight receipt");
    assert!(receipt.contains("agent: amos"));
    assert!(receipt.contains("harness_version: codex-cli 0.144.3"));
    assert!(receipt.contains("preflight_passed: false"));
    assert!(
        receipt.contains("exit_code: null"),
        "a Harness that never launched has no child exit code: {receipt}"
    );
}

#[test]
fn codex_skill_inventory_drift_stops_before_launch() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).expect("workspace");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_harness(
        &bin,
        "codex",
        r#"if [ "$1" = --version ]; then echo 'codex-cli 0.144.3'; exit 0; fi
if [ "$1" = app-server ]; then
  mkdir -p "$CODEX_HOME/skills/ambient"
  cp "$CODEX_HOME/skills/deliver/SKILL.md" "$CODEX_HOME/skills/ambient/SKILL.md"
  while IFS= read -r request; do
    case "$request" in
      *'"id":"roster-skills"'*)
        printf '{"id":"roster-skills","result":{"data":[{"cwd":"fixture","skills":[{"name":"ambient","path":"%s","scope":"user","enabled":true}],"errors":[]}]}}\n' \
          "$CODEX_HOME/skills/ambient/SKILL.md"
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = mcp ]; then printf '[]\n'; exit 0; fi
touch "$FAKE_LAUNCHED""#,
    );
    let launched = temp.path().join("launched");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_LAUNCHED", &launched)
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
        .failure()
        .stderr(predicate::str::contains("Codex skill isolation drift:"))
        .stderr(predicate::str::contains("ambient"))
        .stderr(predicate::str::contains("Launching amos (codex)").not());
    assert!(
        !launched.exists(),
        "the Harness must not launch with an unexpected enabled skill"
    );
}

#[test]
fn codex_skill_identity_collision_stops_before_launch() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(workspace.join(".git")).expect("workspace");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_harness(
        &bin,
        "codex",
        r#"if [ "$1" = --version ]; then echo 'codex-cli 0.144.3'; exit 0; fi
if [ "$1" = app-server ]; then
  mkdir -p "$CODEX_HOME/skills/ambient-deliver"
  cp "$CODEX_HOME/skills/deliver/SKILL.md" "$CODEX_HOME/skills/ambient-deliver/SKILL.md"
  while IFS= read -r request; do
    case "$request" in
      *'"id":"roster-skills"'*)
        printf '{"id":"roster-skills","result":{"data":[{"cwd":"fixture","skills":[{"name":"deliver","path":"%s","scope":"user","enabled":true},{"name":"deliver","path":"%s","scope":"user","enabled":true}],"errors":[]}]}}\n' \
          "$CODEX_HOME/skills/deliver/SKILL.md" \
          "$CODEX_HOME/skills/ambient-deliver/SKILL.md"
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = mcp ]; then printf '[]\n'; exit 0; fi
touch "$FAKE_LAUNCHED""#,
    );
    let launched = temp.path().join("launched");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_LAUNCHED", &launched)
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
        .failure()
        .stderr(predicate::str::contains("Codex skill isolation drift:"))
        .stderr(predicate::str::contains("Launching amos (codex)").not());
    assert!(
        !launched.exists(),
        "a colliding ambient skill must not pass as the declared skill"
    );
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
        .env("SHOULD_NOT_SURVIVE", "ambient-canary")
        .env("OPENAI_API_KEY", "ambient-secret")
        .env("OP_SERVICE_ACCOUNT_TOKEN", "ambient-vault-token")
        .env("ROSTER_CHILD_ENV_CODEX_HOME", "/ambient-codex-home")
        .env("ROSTER_CHILD_ENV_EXPECT_MINT_BASE_URL", "http://mint.test")
        .env("ROSTER_CHILD_ENV_MINT_BASE_URL", "http://mint.test")
        .env("ROSTER_STATE_DIR", &state)
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .code(42);
    let receipt = fs::read_dir(state.join("receipts"))
        .expect("receipts")
        .next()
        .expect("named receipt")
        .expect("receipt entry")
        .path();
    let receipt = fs::read_to_string(receipt).expect("named receipt body");
    assert!(receipt.contains("schema_version: roster.receipt.v2"));
    assert!(receipt.contains("harness_version: codex-cli 9.9.9"));
    assert!(receipt.contains("preflight_passed: true"));
    assert!(receipt.contains("agent: amos"));
    assert!(receipt.contains("binding: amos"));
    assert!(receipt.contains("role: orchestrator"));
    assert!(receipt.contains("purpose: Codex lead"));
    assert!(receipt.contains("identity: core/guidance:lead"));
    assert!(receipt.contains("AGENTS.md: sha256:"));
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
        .env("PATH", &path)
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .env("ROSTER_CHILD_ENV_FAKE_READY", &ready)
        .env("ROSTER_CHILD_ENV_FAKE_CAUGHT", &caught)
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
fn claude_preflight_accepts_new_versions_and_launches_in_the_selected_workspace() {
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
        "if [ \"$1\" = --version ]; then echo '9.9.9 (Claude Code)'; exit 0; fi\ncase \" $* \" in *' --roster-invalid-probe '*) echo \"error: unknown option '--roster-invalid-probe'\" >&2; exit 1;; esac\nif [ \"$1\" = plugin ]; then exit 0; fi\nprintf '%s' \"$PWD\" > \"$FAKE_PWD\"",
    );
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let observed = temp.path().join("observed-pwd");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_PWD", &observed)
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
fn claude_missing_required_capability_stops_before_launch() {
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
        "if [ \"$1\" = --version ]; then echo '9.9.9 (Claude Code)'; exit 0; fi\ncase \" $* \" in *' --strict-mcp-config '*) echo \"error: unknown option '--strict-mcp-config'\" >&2; echo \"error: unknown option '--roster-invalid-probe'\" >&2; exit 1;; esac\nif [ \"$1\" = plugin ]; then exit 0; fi\ntouch \"$FAKE_LAUNCHED\"",
    );
    let launched = temp.path().join("launched");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_LAUNCHED", &launched)
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Claude adapter arguments were rejected before launch: error: unknown option '--strict-mcp-config'",
        ))
        .stderr(predicate::str::contains("Launching amos (claude)").not());
    assert!(!launched.exists(), "unsupported Claude CLI must not launch");
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
        r#"if [ "$1" = --version ]; then echo 'omp v99.0.0'; exit 0; fi
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
  python3 -c 'import json,re,sys; normalized = re.sub(r"\n{3,}", "\n", sys.argv[1]); print(json.dumps({"id":"roster-preflight","type":"response","command":"get_state","success":True,"data":{"model":{"id":sys.argv[2]},"thinkingLevel":"high","systemPrompt":[normalized + "\n# Runtime context","<skill name=\"deliver\">"],"dumpTools":[]}}))' "$prompt" "${FAKE_MODEL:-gpt-test}"
fi"#,
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", &path)
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .success();

    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_CHILD_ENV_FAKE_MODEL", "undeclared-model")
        .env("ROSTER_STATE_DIR", temp.path().join("state"))
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("OMP model isolation drift"))
        .stderr(predicate::str::contains("Launching amos (omp)").not());
}

#[test]
fn bare_launch_is_pipe_friendly_and_init_refuses_overwrite() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    Command::cargo_bin("roster")
        .expect("bin")
        .args(["--config", config.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "amos\tcodex\tgpt-test\tCodex lead",
        ));

    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let source = temp.path().join("source");
    let args = [
        "--cwd",
        workspace.to_str().unwrap(),
        "init",
        "--source",
        source.to_str().unwrap(),
    ];
    Command::cargo_bin("roster")
        .expect("bin")
        .args(args)
        .assert()
        .success()
        .stdout(predicate::str::contains("created"));
    assert!(workspace.join(".roster/config.yaml").is_file());
    Command::cargo_bin("roster")
        .expect("bin")
        .args(args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing to overwrite"));
}

#[test]
fn inspect_reports_one_agent_and_recent_receipts() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let state = temp.path().join("state");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    fake_codex(&bin, "exit 0");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    Command::cargo_bin("roster")
        .expect("bin")
        .env("PATH", path)
        .env("ROSTER_STATE_DIR", &state)
        .args(["--config", config.to_str().unwrap(), "dispatch", "amos"])
        .assert()
        .success();

    Command::cargo_bin("roster")
        .expect("bin")
        .env("ROSTER_STATE_DIR", &state)
        .args(["--config", config.to_str().unwrap(), "inspect"])
        .assert()
        .success()
        .stdout(predicate::str::contains("receipts:"))
        .stdout(predicate::str::contains("- "));
    Command::cargo_bin("roster")
        .expect("bin")
        .args(["--config", config.to_str().unwrap(), "inspect", "amos"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: amos"));
}

#[test]
fn authority_requests_are_explicit_receipted_and_fail_closed() {
    let temp = tempfile::tempdir().expect("temp");
    let config = fixture(temp.path());
    let provider = temp.path().join("bin/authority-provider");
    fake_harness(
        provider.parent().expect("provider parent"),
        "authority-provider",
        r#"test -z "${SHOULD_NOT_SURVIVE:-}"
test -z "${OPENAI_API_KEY:-}"
printf '%s|%s|%s' "$ROSTER_AUTHORITY_AGENT" "$1" "$2" > "$AUTHORITY_OBSERVED"
[ "$2" = deploy ]"#,
    );
    let body = fs::read_to_string(&config).expect("config");
    write(
        &config,
        &format!(
            "{body}authority:\n  command: {:?}\n  args: [fixed]\n",
            provider.display().to_string()
        ),
    );
    let state = temp.path().join("state");
    let observed = temp.path().join("observed");
    let request = |capability: &str| {
        let mut command = Command::cargo_bin("roster").expect("bin");
        command
            .env("ROSTER_AGENT", "amos")
            .env("ROSTER_STATE_DIR", &state)
            .env("ROSTER_CHILD_ENV_AUTHORITY_OBSERVED", &observed)
            .env("SHOULD_NOT_SURVIVE", "ambient-canary")
            .env("OPENAI_API_KEY", "ambient-secret")
            .args([
                "--config",
                config.to_str().unwrap(),
                "authority",
                "request",
                capability,
            ]);
        command
    };
    request("deploy").assert().success();
    assert_eq!(
        fs::read_to_string(&observed).expect("observed"),
        "amos|fixed|deploy"
    );
    let receipt = fs::read_dir(state.join("authority"))
        .expect("authority receipts")
        .next()
        .expect("receipt")
        .expect("entry")
        .path();
    let receipt = fs::read_to_string(receipt).expect("receipt body");
    assert!(receipt.contains("roster.authority_receipt.v1"));
    assert!(receipt.contains("capability: deploy"));

    request("denied")
        .assert()
        .failure()
        .stderr(predicate::str::contains("denied or unavailable"));
    request("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("non-empty descriptive name"));

    let no_provider = fixture(&temp.path().join("plain"));
    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            no_provider.to_str().unwrap(),
            "authority",
            "request",
            "deploy",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no authority provider"));
}

#[test]
fn check_validates_an_explicit_config_graph_and_rejects_no_catalog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Command::cargo_bin("roster")
        .expect("bin")
        .args([
            "--config",
            root.join("examples/config.yaml").to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
            "check",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("roster graph: ok (12 agents"));

    let temp = tempfile::tempdir().expect("temp");
    Command::cargo_bin("roster")
        .expect("bin")
        .current_dir(temp.path())
        .env("HOME", temp.path())
        .env_remove("ROSTER_CONFIG")
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no source exposes"));
}

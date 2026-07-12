use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("mkdir fixture parent");
    fs::write(path, contents).expect("write fixture file");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn visit(path: &Path, base: &Path, out: &mut BTreeMap<String, String>) {
        let relative = path
            .strip_prefix(base)
            .expect("snapshot path under fixture")
            .display()
            .to_string();
        let metadata = fs::symlink_metadata(path).expect("snapshot metadata");
        if metadata.file_type().is_symlink() {
            out.insert(
                relative,
                format!(
                    "link:{}",
                    fs::read_link(path).expect("read symlink").display()
                ),
            );
        } else if metadata.is_dir() {
            for entry in fs::read_dir(path).expect("read snapshot directory") {
                visit(&entry.expect("snapshot entry").path(), base, out);
            }
        } else {
            out.insert(
                relative,
                format!(
                    "file:{}",
                    String::from_utf8_lossy(&fs::read(path).expect("read snapshot"))
                ),
            );
        }
    }

    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn fixture() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().expect("tempdir");
    let root = workspace_root();
    let home = temp.path().join("home");
    let bin = home.join("bin");
    for binary in ["claude", "codex", "omp"] {
        let path = bin.join(binary);
        write(&path, "fixture binary\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("make fixture binary executable");
        }
    }

    for path in [".claude/settings.json", ".omp/agent/config.yml"] {
        write(&home.join(path), "fixture config\n");
    }
    write(&home.join(".omp/agent/mcp.json"), "{}\n");
    let mut sync = Command::cargo_bin("roster").expect("roster binary");
    sync.current_dir(&root)
        .env_remove("CANARY_ENDPOINT")
        .env_remove("CANARY_API_KEY")
        .env_remove("CANARY_INGEST_KEY")
        .args(["sync", "--home"])
        .arg(&home)
        .args(["--catalog", "curated"])
        .assert()
        .success();

    (temp, root, home)
}

fn doctor(root: &Path, home: &Path, json: bool, live: bool) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("roster").expect("roster binary");
    command
        .env_remove("CANARY_ENDPOINT")
        .env_remove("CANARY_API_KEY")
        .env_remove("CANARY_INGEST_KEY")
        .env("PATH", home.join("bin"))
        .args([
            "--root",
            root.to_str().expect("root utf8"),
            "doctor",
            "--home",
            home.to_str().expect("home utf8"),
        ]);
    if json {
        command.arg("--json");
    }
    if live {
        command.arg("--live");
    }
    command.assert()
}

#[test]
fn doctor_json_reports_clean_tier1_fixture_without_writing() {
    let (_temp, root, home) = fixture();
    let before = snapshot(&home);

    let output = doctor(&root, &home, true, false)
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).expect("doctor JSON");

    assert_eq!(report["schema_version"], "roster.doctor.v1");
    assert_eq!(report["ok"], true);
    assert_eq!(report["degraded"], false);
    assert_eq!(report["source"]["root"], root.display().to_string());
    assert_eq!(report["source"]["home"], home.display().to_string());
    assert_eq!(report["manifest"]["present"], true);
    assert_eq!(report["manifest"]["integrity"], "ok");
    assert_eq!(report["failures"], json!([]));
    for harness in ["claude", "codex", "omp"] {
        assert_eq!(report["harnesses"][harness]["binary"]["found"], true);
        assert_eq!(
            report["harnesses"][harness]["native_config"]["present"],
            true
        );
        assert_eq!(report["harnesses"][harness]["doctrine"]["present"], true);
        assert_eq!(
            report["harnesses"][harness]["default_orchestrator"]["present"],
            true
        );
    }
    assert!(!String::from_utf8_lossy(&output).contains("fixture config"));
    assert_eq!(
        before,
        snapshot(&home),
        "doctor must not write the fixture HOME"
    );
}

#[test]
fn doctor_json_reports_broken_projection_nonzero_and_only_safe_findings() {
    let (_temp, root, home) = fixture();
    fs::remove_file(home.join(".roster/orchestrator/codex-roles/orchestrator.toml"))
        .expect("break codex role");
    fs::remove_file(home.join(".claude/settings.json")).expect("break Claude config");
    write(&home.join(".codex/config/config.toml"), "legacy config\n");
    let before = snapshot(&home);

    let assertion = doctor(&root, &home, true, false).failure();
    let output = assertion.get_output().stdout.clone();
    let report: Value = serde_json::from_slice(&output).expect("broken doctor JSON");

    assert_eq!(report["ok"], false);
    assert_eq!(report["degraded"], true);
    assert_eq!(report["manifest"]["integrity"], "broken");
    assert!(
        report["failures"]
            .as_array()
            .expect("failures array")
            .iter()
            .any(|finding| finding
                .as_str()
                .is_some_and(|finding| finding.contains("codex-roles/orchestrator.toml")))
    );
    assert!(
        report["warnings"]
            .as_array()
            .expect("warnings array")
            .iter()
            .any(|warning| warning
                .as_str()
                .is_some_and(|warning| warning.contains("nested Codex config")))
    );
    assert!(!String::from_utf8_lossy(&output).contains("legacy config"));
    assert_eq!(
        before,
        snapshot(&home),
        "broken doctor must still be read-only"
    );
}

#[test]
fn doctor_rejects_content_drift_and_unmanaged_doctrine_copy() {
    let (_temp, root, home) = fixture();
    write(
        &home.join(".omp/agent/agents/orchestrator.md"),
        "orchestrator but drifted\n",
    );
    let claude_doctrine = home.join(".claude/CLAUDE.md");
    fs::remove_file(&claude_doctrine).expect("remove managed doctrine link");
    let doctrine_contents = fs::read_to_string(home.join(".roster/orchestrator/home-doctrine.md"))
        .expect("read doctrine source");
    write(&claude_doctrine, &doctrine_contents);

    let report: Value = serde_json::from_slice(
        &doctor(&root, &home, true, false)
            .failure()
            .get_output()
            .stdout,
    )
    .expect("drift report");
    assert_eq!(
        report["harnesses"]["omp"]["default_orchestrator"]["present"],
        false
    );
    assert_eq!(report["harnesses"]["claude"]["doctrine"]["present"], false);
}

#[test]
fn doctor_fails_when_registry_disabled_mcp_remains_effectively_active() {
    let (_temp, root, home) = fixture();
    write(
        &home.join(".claude.json"),
        r#"{"mcpServers":{"context7":{"type":"http","url":"https://example.invalid"}}}"#,
    );

    let output = doctor(&root, &home, true, false)
        .failure()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).expect("MCP drift doctor JSON");
    assert_eq!(
        report["mcp_policy"]["active_disabled"],
        json!(["claude:context7"])
    );
    assert!(
        report["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding.as_str().unwrap().contains("registry-disabled"))
    );
    assert!(!String::from_utf8_lossy(&output).contains("example.invalid"));
}

#[test]
fn doctor_treats_absent_tier1_binaries_as_warning_only() {
    let (_temp, root, home) = fixture();
    for binary in ["claude", "codex", "omp"] {
        fs::remove_file(home.join("bin").join(binary)).expect("remove fixture binary");
    }

    let report: Value = serde_json::from_slice(
        &doctor(&root, &home, true, false)
            .success()
            .get_output()
            .stdout,
    )
    .expect("warning-only doctor JSON");
    assert_eq!(report["ok"], true);
    assert_eq!(report["degraded"], true);
    assert_eq!(report["failures"], json!([]));
    assert_eq!(
        report["warnings"].as_array().expect("warnings array").len(),
        3
    );
}

#[test]
fn doctor_human_output_is_compact() {
    let (_temp, root, home) = fixture();
    doctor(&root, &home, false, false)
        .success()
        .stdout(predicates::str::contains("roster doctor: ok"))
        .stdout(predicates::str::contains("manifest: ok"))
        .stdout(predicates::str::contains("claude: binary=found"))
        .stdout(predicates::str::contains("codex: binary=found"))
        .stdout(predicates::str::contains("omp: binary=found"))
        .stdout(predicates::str::contains("{ ").not());
}

#[test]
fn doctor_live_probes_are_bounded_and_do_not_echo_child_output() {
    let (_temp, root, home) = fixture();
    for binary in ["claude", "codex", "omp", "qmd"] {
        let path = home.join("bin").join(binary);
        write(
            &path,
            &format!(
                "#!/bin/sh\n[ \"$HOME\" = {:?} ] || exit 7\necho child-secret-shaped-output\nexit 0\n",
                home.display().to_string()
            ),
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("make live fixture executable");
        }
    }
    write(
        &home.join("bin/codex"),
        &format!(
            "#!/bin/sh\n[ \"$HOME\" = {:?} ] || exit 7\ni=0\nwhile [ \"$i\" -lt 20000 ]; do echo child-secret-shaped-output; i=$((i + 1)); done\nexit 0\n",
            home.display().to_string()
        ),
    );
    write(
        &home.join("bin/claude"),
        &format!(
            "#!/bin/sh\n[ \"$HOME\" = {:?} ] || exit 7\necho 'Failed to connect'\necho 'Needs authentication'\necho child-secret-shaped-output\nexit 0\n",
            home.display().to_string()
        ),
    );

    let output = doctor(&root, &home, true, true)
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).expect("live doctor JSON");
    assert_eq!(report["live"]["enabled"], true);
    for probe in ["claude_mcp", "codex", "omp_config", "qmd"] {
        assert_eq!(report["live"]["probes"][probe]["timed_out"], false);
        assert_eq!(report["live"]["probes"][probe]["exit_code"], 0);
    }
    assert_eq!(
        report["live"]["probes"]["claude_mcp"]["reported_failed_connections"],
        1
    );
    assert_eq!(
        report["live"]["probes"]["claude_mcp"]["reported_auth_required"],
        1
    );
    assert!(!String::from_utf8_lossy(&output).contains("child-secret-shaped-output"));
}

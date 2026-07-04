use assert_cmd::Command;
use predicates::prelude::*;
use std::{collections::BTreeMap, path::PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn roster_cmd() -> Command {
    let mut command = Command::cargo_bin("roster").expect("roster binary");
    command.current_dir(workspace_root());
    command
}

#[test]
fn list_prints_seed_agents() {
    roster_cmd()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("cerberus\tcodex-class\txhigh"))
        .stdout(predicate::str::contains("lead\tfable-class\tlow"))
        .stdout(predicate::str::contains("sweep\topenrouter-class\tmedium"));
}

#[test]
fn materialize_codex_prints_brief_header() {
    roster_cmd()
        .args(["materialize", "cerberus", "--harness", "codex"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Roster Brief: cerberus"))
        .stdout(predicate::str::contains("Read:"))
        .stdout(predicate::str::contains("Code-review master"));
}

#[test]
fn materialize_bb_prints_agent_binding() {
    roster_cmd()
        .args(["materialize", "cerberus", "--harness", "bb"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Generated from roster agent cerberus",
        ))
        .stdout(predicate::str::contains("harness = \"pi\""))
        .stdout(predicate::str::contains(
            "model = \"moonshotai/kimi-k2.7-code\"",
        ))
        .stdout(predicate::str::contains("role = \"cerberus\""))
        .stdout(predicate::str::contains("output_bytes_cap = 120000"))
        .stdout(predicate::str::contains("side_effect_policy = \"kill\""));
}

#[test]
fn materialize_claude_prints_native_subagent_frontmatter() {
    for (agent, expected_tools) in [
        ("lead", "Read, Write, Edit, Grep, Glob, Bash, WebSearch"),
        ("cerberus", "Read, Grep, Glob, Bash"),
    ] {
        let output = roster_cmd()
            .args(["materialize", agent, "--harness", "claude"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let output = String::from_utf8(output).expect("utf8 stdout");
        let frontmatter = frontmatter_fields(&output);

        assert_eq!(
            frontmatter.keys().map(String::as_str).collect::<Vec<_>>(),
            ["description", "model", "name", "tools"]
        );
        assert_eq!(frontmatter.get("name").map(String::as_str), Some(agent));
        assert!(
            frontmatter
                .get("description")
                .is_some_and(|description| !description.is_empty())
        );
        assert_eq!(frontmatter.get("model").map(String::as_str), Some("sonnet"));
        assert_eq!(
            frontmatter.get("tools").map(String::as_str),
            Some(expected_tools)
        );
        assert!(
            !["fable-class", "codex-class", "openrouter-class"]
                .contains(&frontmatter["model"].as_str())
        );
    }
}

fn frontmatter_fields(output: &str) -> BTreeMap<String, String> {
    let frontmatter = output
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .expect("claude subagent frontmatter");

    let mut fields = BTreeMap::new();
    for line in frontmatter.lines() {
        let (key, value) = line.split_once(':').expect("frontmatter key-value");
        assert!(
            fields
                .insert(key.to_string(), value.trim().to_string())
                .is_none(),
            "duplicate frontmatter key {key}"
        );
    }
    fields
}

#[test]
fn sync_is_p2_stub() {
    roster_cmd()
        .arg("sync")
        .assert()
        .failure()
        .stderr(predicate::str::contains("P2"));
}

use roster_core::Roster;
use std::{fs, path::PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn loads_seed_agents_from_repo() {
    let roster = Roster::load(workspace_root()).expect("seed roster loads");
    let names: Vec<_> = roster
        .agents()
        .iter()
        .map(|agent| agent.role.name.as_str())
        .collect();

    // Roster::load sorts agents by name (see roster-core/src/lib.rs), so this
    // list is alphabetical, not insertion order. Adding a new agent directory
    // is a mechanical edit here: insert its name in alphabetical position and
    // add an assertion block for it below, same shape as the existing agent
    // blocks (roster-911 landed exactly where the roster-908 comment
    // predicted: `builder` before `cerberus`, `verifier` after `sweep`).
    assert_eq!(
        names,
        [
            "builder",
            "cerberus",
            "oracle",
            "orchestrator",
            "sweep",
            "verifier"
        ]
    );

    let builder = roster.agent("builder").expect("builder exists");
    assert_eq!(builder.role.model_policy.preferred, "codex-class");
    assert_eq!(builder.role.permissions.filesystem, "workspace-write");
    assert_eq!(builder.role.mcps, ["powder"]);

    let cerberus = roster.agent("cerberus").expect("cerberus exists");
    assert!(cerberus.role.description.contains("Code-review master"));
    assert_eq!(cerberus.role.model_policy.preferred, "codex-class");
    assert_eq!(cerberus.role.model_policy.reasoning, "xhigh");
    assert!(
        cerberus
            .instructions
            .contains("Cerberus code-review master")
    );
    assert!(cerberus.role.mcps_contextual.is_empty());

    let orchestrator = roster.agent("orchestrator").expect("orchestrator exists");
    assert_eq!(orchestrator.role.mcps, ["powder"]);
    assert_eq!(
        orchestrator.role.mcps_contextual,
        ["qmd", "todoist", "bitterblossom", "glass"]
    );

    let oracle = roster.agent("oracle").expect("oracle exists");
    assert_eq!(oracle.role.model_policy.preferred, "openrouter-class");
    assert_eq!(oracle.role.model_policy.reasoning, "high");
    assert!(oracle.role.mcps.is_empty());
    assert_eq!(
        oracle.role.mcps_contextual,
        ["exa", "firecrawl", "context7"]
    );
    assert!(!oracle.role.subagent_rights.may_dispatch);
    assert!(oracle.instructions.contains("probe the cheap tier"));

    let verifier = roster.agent("verifier").expect("verifier exists");
    assert_eq!(verifier.role.permissions.filesystem, "read-only");
    assert_eq!(verifier.role.permissions.commands, "verification-only");
    assert!(verifier.role.mcps.is_empty());
    assert!(verifier.instructions.contains("never fix what you verify"));
}

#[test]
fn unknown_role_fields_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let agent_dir = temp.path().join("agents/bad");
    fs::create_dir_all(&agent_dir).expect("agent dir");
    fs::write(agent_dir.join("instructions.md"), "# Bad\n").expect("instructions");
    fs::write(
        agent_dir.join("role.yaml"),
        r#"schema_version: roster.role.v1
name: bad
description: Bad fixture
model_policy:
  preferred: codex-class
  fallbacks: []
  reasoning: high
permissions:
  filesystem: read-only
  commands: read-only
  network: none
  secrets: none
  mutations: none
skills: []
mcps: []
subagent_rights:
  may_dispatch: false
  may_spawn_subagents: false
  may_use_peer_harnesses: false
evidence_expectations: []
surprise: should fail
"#,
    )
    .expect("role");

    let error = Roster::load(temp.path()).expect_err("unknown field rejected");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn missing_instructions_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let agent_dir = temp.path().join("agents/bad");
    fs::create_dir_all(&agent_dir).expect("agent dir");
    fs::write(
        agent_dir.join("role.yaml"),
        r#"schema_version: roster.role.v1
name: bad
description: Bad fixture
model_policy:
  preferred: codex-class
  fallbacks: []
  reasoning: high
permissions:
  filesystem: read-only
  commands: read-only
  network: none
  secrets: none
  mutations: none
skills: []
mcps: []
subagent_rights:
  may_dispatch: false
  may_spawn_subagents: false
  may_use_peer_harnesses: false
evidence_expectations: []
"#,
    )
    .expect("role");

    let error = Roster::load(temp.path()).expect_err("missing instructions rejected");
    assert!(error.to_string().contains("instructions.md"), "{error}");
}

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
fn loads_one_catalog_and_resolves_every_role_mcp() {
    let roster = Roster::load(workspace_root()).expect("single MCP catalog loads");
    let ids: Vec<_> = roster.mcps().iter().map(|mcp| mcp.id.as_str()).collect();

    assert_eq!(
        ids,
        [
            "aesthetic",
            "bitterblossom",
            "canary",
            "context7",
            "conviction",
            "exa",
            "firecrawl",
            "glass",
            "landmark",
            "mint",
            "overmind",
            "powder",
            "qmd",
            "robinhood-trading",
            "todoist"
        ]
    );

    let context7 = roster.mcp("context7").expect("context7 is declared");
    assert_eq!(context7.status, "disabled");
    let reason = context7.reason.as_deref().expect("disabled MCP has reason");
    assert!(reason.contains("2026-07-11"), "{reason}");
    assert!(
        reason.contains("Unfold must not be polled after it returned Poll::Ready(None)"),
        "{reason}"
    );

    let powder = roster.mcp("powder").expect("powder is declared");
    assert_eq!(powder.source_repo.as_deref(), Some("misty-step/powder"));
    assert_eq!(
        powder.source_ref.as_deref(),
        Some("a20d4ecefcb6a16d595966177b48c47a87dfffc8")
    );
}

#[test]
fn unknown_mcp_registry_fields_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("agents")).expect("agents dir");
    fs::create_dir_all(temp.path().join("primitives/mcps")).expect("MCP directory");
    fs::write(
        temp.path().join("primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: test\nmcps: []\nsurprise: true\n",
    )
    .expect("registry");

    let error = Roster::load(temp.path()).expect_err("unknown registry field rejected");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn duplicate_mcp_ids_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("primitives/mcps")).expect("MCP directory");
    fs::create_dir(temp.path().join("agents")).expect("agents dir");
    fs::write(
        temp.path().join("primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: test\nmcps:\n  - id: duplicate\n    status: external\n  - id: duplicate\n    status: disabled\n    reason: test\n",
    )
    .expect("registry");

    let error = Roster::load(temp.path()).expect_err("duplicate MCP id rejected");
    assert!(
        error.to_string().contains("duplicate MCP id \"duplicate\""),
        "{error}"
    );
}

#[test]
fn unknown_required_or_contextual_mcp_references_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("primitives/mcps")).expect("MCP directory");
    let agent_dir = temp.path().join("agents/test-agent");
    fs::create_dir_all(&agent_dir).expect("agent directory");
    fs::write(agent_dir.join("instructions.md"), "# Test agent\n").expect("instructions");
    fs::write(
        agent_dir.join("role.yaml"),
        r#"schema_version: roster.role.v1
name: test-agent
description: Test agent
model_policy:
  preferred:
    model: gpt-5.6-luna
    reasoning: high
  fallbacks: []
permissions:
  filesystem: read-only
  commands: read-only
  network: none
  secrets: none
  mutations: none
skills: []
mcps: []
mcps_contextual:
  - missing
subagent_rights:
  may_dispatch: false
  may_spawn_subagents: false
  may_use_peer_harnesses: false
evidence_expectations: []
"#,
    )
    .expect("role");
    fs::write(
        temp.path().join("primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: test\nmcps:\n  - id: known\n    status: external\n",
    )
    .expect("registry");

    let error = Roster::load(temp.path()).expect_err("unknown MCP reference rejected");
    assert!(
        error
            .to_string()
            .contains("references unknown MCP \"missing\" in mcps_contextual"),
        "{error}"
    );

    let role_path = agent_dir.join("role.yaml");
    let role = fs::read_to_string(&role_path)
        .expect("read role")
        .replace("  - missing", "  - known");
    fs::write(&role_path, role).expect("rewrite role");
    fs::write(
        temp.path().join("primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: test\nmcps:\n  - id: known\n    status: disabled\n    reason: incident\n",
    )
    .expect("disabled registry");
    let error = Roster::load(temp.path()).expect_err("disabled MCP reference rejected");
    assert!(
        error
            .to_string()
            .contains("non-bindable status \"disabled\""),
        "{error}"
    );
}

#[test]
fn available_mcp_requires_a_complete_launch_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("primitives/mcps")).expect("MCP directory");
    fs::create_dir(temp.path().join("agents")).expect("agents dir");
    fs::write(
        temp.path().join("primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: test\nmcps:\n  - id: incomplete\n    status: available\n    transport: http\n",
    )
    .expect("registry");

    let error = Roster::load(temp.path()).expect_err("incomplete available MCP rejected");
    assert!(error.to_string().contains("empty mcps[].url"), "{error}");
}

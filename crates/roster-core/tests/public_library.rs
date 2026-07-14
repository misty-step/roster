use roster_core::Roster;
use std::path::PathBuf;

#[test]
fn every_example_agent_resolves_from_the_public_library() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");
    assert_eq!(roster.agents().len(), 11);
    for name in roster.agents().keys() {
        let resolved = roster
            .resolve(name)
            .unwrap_or_else(|error| panic!("resolve {name}: {error}"));
        assert!(!resolved.guidance.is_empty(), "{name} has no guidance");
        assert!(!resolved.skills.is_empty(), "{name} has no skills");
    }
}

#[test]
fn smith_resolves_the_focused_agent_engineering_surface() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");
    let smith = roster.resolve("smith").expect("resolve smith");
    let guidance = smith
        .guidance
        .iter()
        .map(|item| item.identity.as_str())
        .collect::<Vec<_>>();
    let skills = smith
        .skills
        .iter()
        .map(|item| item.identity.as_str())
        .collect::<Vec<_>>();
    let mcps = smith
        .mcps
        .iter()
        .map(|item| item.identity.as_str())
        .collect::<Vec<_>>();

    assert_eq!(smith.role, "agent-creator");
    assert_eq!(smith.model, "gpt-5.6-sol");
    assert_eq!(smith.reasoning.as_deref(), Some("high"));
    assert_eq!(smith.harness.to_string(), "codex");
    assert_eq!(
        smith.args,
        ["--search", "--dangerously-bypass-approvals-and-sandbox"]
    );
    assert_eq!(
        guidance,
        [
            "core/guidance:engineering",
            "core/guidance:work-ledger",
            "core/guidance:agent-creator",
            "core/guidance:delegation",
        ]
    );
    assert_eq!(
        skills,
        [
            "core/skill:orient",
            "core/skill:roster",
            "core/skill:powder",
            "core/skill:harness-engineering",
            "core/skill:skill-eval",
            "core/skill:eval-design",
            "core/skill:mcp-design",
            "core/skill:research",
        ]
    );
    assert_eq!(mcps, ["core/mcp:powder", "core/mcp:crucible"]);
}

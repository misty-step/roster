use roster_core::Roster;
use std::fs;
use std::path::PathBuf;

#[test]
fn every_example_agent_resolves_from_the_public_library() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");
    assert_eq!(roster.agents().len(), 12);
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

#[test]
fn estate_action_classes_materialize_without_granting_authority() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");

    for (class, guidance_identity) in [
        (
            "observe-plan",
            "core/guidance:estate-infrastructure-observe-plan",
        ),
        (
            "bounded-reversible",
            "core/guidance:estate-infrastructure-bounded-reversible",
        ),
        (
            "exact-plan-mutation",
            "core/guidance:estate-infrastructure-exact-plan-mutation",
        ),
    ] {
        let include = vec![format!("core/pack:estate-infrastructure-{class}")];
        let resolved = roster
            .resolve_ad_hoc(
                "hephaestus",
                &format!("estate-{class}"),
                &format!("Request Estate {class} work."),
                &include,
            )
            .unwrap_or_else(|error| panic!("resolve {class}: {error}"));

        assert_eq!(resolved.role, "ad-hoc");
        assert_eq!(resolved.guidance[0].identity, guidance_identity);
        assert_eq!(
            resolved.skills[0].identity,
            "core/skill:estate-infrastructure"
        );
        assert_eq!(
            resolved.guidance[0].via,
            [[
                format!("ad-hoc/role:estate-{class}"),
                format!("core/pack:estate-infrastructure-{class}"),
            ]]
        );
    }

    let include = vec!["core/pack:estate-infrastructure-exact-plan-mutation".to_owned()];
    let resolved = roster
        .resolve_ad_hoc(
            "hephaestus",
            "estate-exact-plan-proof",
            "Prove the public Estate exact-plan projection.",
            &include,
        )
        .expect("resolve Estate proof agent");
    let temp = tempfile::tempdir().expect("tempdir");
    let bundle = temp.path().join("bundle");
    let manifest = resolved
        .write_bundle(&bundle, temp.path())
        .expect("materialize Estate proof agent");
    let agents = fs::read_to_string(bundle.join("AGENTS.md")).expect("read AGENTS.md");
    let skill = fs::read_to_string(bundle.join("skills/estate-infrastructure/SKILL.md"))
        .expect("read Estate skill");

    assert_eq!(manifest.role, "ad-hoc");
    assert_eq!(
        manifest.guidance[0].identity,
        "core/guidance:estate-infrastructure-exact-plan-mutation"
    );
    assert_eq!(
        manifest.skills[0].identity,
        "core/skill:estate-infrastructure"
    );
    assert!(agents.contains("The declaration grants nothing"));
    assert!(agents.contains("skills/estate-infrastructure/SKILL.md"));
    assert!(skill.contains("standards/vendor-inventory.toml"));
    assert!(skill.contains("not runtime identity or Estate approval"));
    assert!(skill.contains("ad-hoc role may prove this projection"));

    let projected = format!("{agents}\n{skill}").to_ascii_lowercase();
    for forbidden in [
        "sanctum",
        "bearer ",
        "private_key",
        "access_token",
        "__mint.",
    ] {
        assert!(
            !projected.contains(forbidden),
            "public Estate projection contains forbidden private material: {forbidden}"
        );
    }
}

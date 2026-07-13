use roster_core::Roster;
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

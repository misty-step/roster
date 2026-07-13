use roster_core::{Harness, Roster, discover_config};
use std::{fs, path::Path, str::FromStr};

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, body).expect("write fixture");
}

fn source(root: &Path, id: &str, tracker: &str) {
    write(
        &root.join("roles/orchestrator.yaml"),
        &format!(
            r#"schema_version: roster.role.v2
name: orchestrator
description: Coordinate {id} work
include:
  - {id}/guidance:identity
  - {id}/skill:deliver
  - {id}/mcp:{tracker}
"#
        ),
    );
    write(
        &root.join("primitives/guidance/identity.md"),
        &format!("# {id}\n\nOperate as {id}.\n"),
    );
    write(
        &root.join("primitives/skills/deliver/SKILL.md"),
        "---\nname: deliver\ndescription: Deliver one change.\n---\n\nDeliver it.\n",
    );
    write(
        &root.join("primitives/mcps/registry.yaml"),
        &format!(
            r#"schema_version: roster.mcp_registry.v1
provenance: fixture
mcps:
  - id: {tracker}
    status: available
    transport: stdio
    command: {tracker}-mcp
"#
        ),
    );
}

fn config(path: &Path, source_id: &str, source_path: &Path, agent: &str, harness: &str) {
    write(
        path,
        &format!(
            r#"schema_version: roster.config.v1
sources:
  {source_id}: {}
agents:
  {agent}:
    description: {source_id} lead
    role: {source_id}/role:orchestrator
    model: test/model
    reasoning: high
    harness: {harness}
    args: []
    delegates: []
"#,
            source_path.display()
        ),
    );
}

#[test]
fn nearest_config_replaces_home_instead_of_merging() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("home");
    let work = temp.path().join("work/r90/project");
    let misty = temp.path().join("misty-source");
    let r90 = temp.path().join("r90-source");
    source(&misty, "misty", "powder");
    source(&r90, "r90", "habitat");
    config(
        &home.join(".roster/config.yaml"),
        "misty",
        &misty,
        "amos",
        "codex",
    );
    config(
        &temp.path().join("work/r90/.roster/config.yaml"),
        "r90",
        &r90,
        "athena",
        "claude",
    );
    fs::create_dir_all(&work).expect("work dir");

    let found = discover_config(&work, &home).expect("discover local config");
    assert_eq!(found, temp.path().join("work/r90/.roster/config.yaml"));
    let roster = Roster::load_config(&found).expect("load scoped roster");
    assert!(roster.agent("athena").is_some());
    assert!(roster.agent("amos").is_none());

    let resolved = roster.resolve("athena").expect("resolve r90 agent");
    assert_eq!(resolved.harness, Harness::Claude);
    assert_eq!(resolved.mcps[0].id, "habitat");
    assert!(resolved.mcps.iter().all(|mcp| mcp.id != "powder"));
}

#[test]
fn bundle_contains_only_the_resolved_role_primitives() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("home");
    let source_root = temp.path().join("source");
    source(&source_root, "misty", "powder");
    config(
        &home.join(".roster/config.yaml"),
        "misty",
        &source_root,
        "amos",
        "codex",
    );
    let roster = Roster::load_config(home.join(".roster/config.yaml")).expect("load");
    let resolved = roster.resolve("amos").expect("resolve");
    write(
        &source_root.join("primitives/skills/deliver/scripts/__pycache__/generated.pyc"),
        "generated",
    );
    write(
        &source_root.join("primitives/skills/deliver/.env.example.tmpl"),
        "SAFE_TEMPLATE=1",
    );
    let bundle = temp.path().join("bundle");
    write(
        &temp.path().join("AGENTS.md"),
        "# Fixture project\n\nProject truth.\n",
    );
    let manifest = resolved
        .write_bundle(&bundle, temp.path())
        .expect("write bundle");

    assert!(bundle.join("AGENTS.md").is_file());
    assert!(bundle.join("skills/deliver/SKILL.md").is_file());
    assert!(bundle.join("mcps.yaml").is_file());
    assert!(bundle.join("manifest.yaml").is_file());
    assert_eq!(manifest.skills.len(), 1);
    assert_eq!(manifest.mcps.len(), 1);
    assert_eq!(manifest.mcps[0].identity, "misty/mcp:powder");
    assert_eq!(manifest.reasoning.as_deref(), Some("high"));
    assert!(manifest.args.is_empty());
    assert!(manifest.delegates.is_empty());
    assert!(!bundle.join("skills/deliver/scripts/__pycache__").exists());
    assert!(bundle.join("skills/deliver/.env.example.tmpl").is_file());
    assert_eq!(manifest.context.len(), 1);
    assert!(manifest.context[0].sha256.starts_with("sha256:"));
    assert!(
        fs::read_to_string(bundle.join("AGENTS.md"))
            .expect("agents")
            .contains("Project truth.")
    );
    assert!(
        !fs::read_to_string(bundle.join("AGENTS.md"))
            .expect("agents")
            .contains("habitat")
    );
}

#[test]
fn secret_shaped_files_and_symlinks_are_rejected_before_bundle_creation() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    config(
        &temp.path().join("config.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load");
    let resolved = roster.resolve("amos").expect("resolve");
    let secret = root.join("primitives/skills/deliver/.env.local");
    write(&secret, "SECRET=1");
    let secret_bundle = temp.path().join("secret-bundle");
    let error = resolved
        .write_bundle(&secret_bundle, temp.path())
        .expect_err("secret-shaped file must fail");
    assert!(error.to_string().contains("secret-shaped"), "{error}");
    assert!(!secret_bundle.exists());

    fs::remove_file(secret).expect("remove fixture secret");
    symlink(
        "/etc/hosts",
        root.join("primitives/skills/deliver/escaped-link"),
    )
    .expect("create fixture symlink");
    let symlink_bundle = temp.path().join("symlink-bundle");
    let error = resolved
        .write_bundle(&symlink_bundle, temp.path())
        .expect_err("symlink must fail");
    assert!(error.to_string().contains("symlink"), "{error}");
    assert!(!symlink_bundle.exists());
}

#[test]
fn unknown_agent_fields_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path().join("source");
    source(&source_root, "core", "powder");
    write(
        &temp.path().join("config.yaml"),
        &format!(
            r#"schema_version: roster.config.v1
sources:
  core: {}
agents:
  broken:
    description: broken
    role: core/role:orchestrator
    model: test/model
    harness: codex
    primitives: [core/skill:deliver]
"#,
            source_root.display()
        ),
    );

    let error =
        Roster::load_config(temp.path().join("config.yaml")).expect_err("unknown field must fail");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn path_shaped_agent_names_are_rejected_before_dispatch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path().join("source");
    source(&source_root, "core", "powder");
    write(
        &temp.path().join("config.yaml"),
        &format!(
            "schema_version: roster.config.v1\nsources:\n  core: {}\nagents:\n  ../escape:\n    description: unsafe\n    role: core/role:orchestrator\n    model: test/model\n    harness: codex\n",
            source_root.display()
        ),
    );
    let error = Roster::load_config(temp.path().join("config.yaml"))
        .expect_err("path-shaped agent name must fail");
    assert!(error.to_string().contains("unsafe agent name"), "{error}");
}

#[test]
fn same_short_skill_name_from_two_sources_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let left = temp.path().join("left");
    let right = temp.path().join("right");
    source(&left, "left", "powder");
    source(&right, "right", "habitat");
    write(
        &left.join("roles/collision.yaml"),
        "schema_version: roster.role.v2\nname: collision\ndescription: collision\ninclude:\n  - left/skill:deliver\n  - right/skill:deliver\n",
    );
    write(
        &temp.path().join("config.yaml"),
        &format!(
            "schema_version: roster.config.v1\nsources:\n  left: {}\n  right: {}\nagents:\n  broken:\n    description: broken\n    role: left/role:collision\n    model: test/model\n    harness: codex\n",
            left.display(),
            right.display()
        ),
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load graph");
    let error = roster.resolve("broken").expect_err("collision must fail");
    assert!(
        error.to_string().contains("skill name collision"),
        "{error}"
    );
}

#[test]
fn same_projected_mcp_name_from_two_sources_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let left = temp.path().join("left");
    let right = temp.path().join("right");
    source(&left, "left", "tracker");
    source(&right, "right", "tracker");
    write(
        &left.join("roles/collision.yaml"),
        "schema_version: roster.role.v2\nname: collision\ndescription: collision\ninclude:\n  - left/mcp:tracker\n  - right/mcp:tracker\n",
    );
    write(
        &temp.path().join("config.yaml"),
        &format!(
            "schema_version: roster.config.v1\nsources:\n  left: {}\n  right: {}\nagents:\n  broken:\n    description: broken\n    role: left/role:collision\n    model: test/model\n    harness: codex\n",
            left.display(),
            right.display()
        ),
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load graph");
    let error = roster
        .resolve("broken")
        .expect_err("MCP collision must fail");
    assert!(error.to_string().contains("MCP name collision"), "{error}");
}

#[test]
fn duplicate_skill_index_names_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    write(
        &root.join("primitives/skills/skills-index.yaml"),
        "schema_version: roster.skills_index.v1\nphase: test\nnote: test\nskills:\n  - name: deliver\n    path: primitives/skills/deliver/SKILL.md\n  - name: deliver\n    path: primitives/skills/other/SKILL.md\n",
    );
    config(
        &temp.path().join("config.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load graph");
    let error = roster
        .resolve("amos")
        .expect_err("duplicate index name must fail");
    assert!(
        error.to_string().contains("duplicate skill name"),
        "{error}"
    );
}

#[test]
fn skill_index_paths_cannot_escape_the_declared_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    fs::remove_dir_all(root.join("primitives/skills/deliver")).expect("remove direct fixture");
    write(
        &temp.path().join("outside/SKILL.md"),
        "---\nname: deliver\ndescription: escaped\n---\n",
    );
    write(
        &root.join("primitives/skills/skills-index.yaml"),
        "schema_version: roster.skills_index.v1\nphase: test\nnote: test\nskills:\n  - name: deliver\n    path: ../../outside/SKILL.md\n",
    );
    config(
        &temp.path().join("config.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load graph");
    let error = roster.resolve("amos").expect_err("escaping path must fail");
    assert!(error.to_string().contains("escapes"), "{error}");
}

#[test]
fn unsafe_harness_arguments_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    config(
        &temp.path().join("config.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let path = temp.path().join("config.yaml");
    let body = fs::read_to_string(&path)
        .expect("config")
        .replace("args: []", "args: [--api-key, SUPERSECRET]");
    write(&path, &body);
    let error = Roster::load_config(&path).expect_err("credential flag must fail");
    assert!(error.to_string().contains("unsafe"), "{error}");
    assert!(!error.to_string().contains("SUPERSECRET"));
}

#[test]
fn duplicate_and_incomplete_mcp_declarations_are_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    config(
        &temp.path().join("config.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let registry = root.join("primitives/mcps/registry.yaml");
    write(
        &registry,
        "schema_version: roster.mcp_registry.v1\nprovenance: fixture\nmcps:\n  - id: powder\n    status: available\n    transport: stdio\n    command: powder\n  - id: powder\n    status: available\n    transport: http\n    url: https://example.test\n",
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load");
    let error = roster.resolve("amos").expect_err("duplicate MCP must fail");
    assert!(error.to_string().contains("duplicate MCP"), "{error}");

    write(
        &registry,
        "schema_version: roster.mcp_registry.v1\nprovenance: fixture\nmcps:\n  - id: powder\n    status: available\n    transport: http\n",
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load");
    let error = roster
        .resolve("amos")
        .expect_err("incomplete MCP must fail");
    assert!(
        error
            .to_string()
            .contains("requires stdio command or http URL")
    );
}

#[test]
fn manifest_records_registry_and_inclusion_chain() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    write(
        &root.join("packs/ledger.yaml"),
        "schema_version: roster.pack.v1\nname: ledger\ninclude:\n  - core/mcp:powder\n",
    );
    write(
        &root.join("packs/operations.yaml"),
        "schema_version: roster.pack.v1\nname: operations\ninclude:\n  - core/mcp:powder\n",
    );
    write(
        &root.join("roles/orchestrator.yaml"),
        "schema_version: roster.role.v2\nname: orchestrator\ndescription: test\ninclude:\n  - core/pack:ledger\n  - core/pack:operations\n",
    );
    config(
        &temp.path().join("config.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let roster = Roster::load_config(temp.path().join("config.yaml")).expect("load");
    let resolved = roster.resolve("amos").expect("resolve");
    let bundle = temp.path().join("bundle");
    let manifest = resolved.write_bundle(&bundle, temp.path()).expect("bundle");
    assert!(
        manifest.mcps[0]
            .source
            .ends_with("primitives/mcps/registry.yaml")
    );
    assert_eq!(
        manifest.mcps[0].via,
        [
            ["core/role:orchestrator", "core/pack:ledger"],
            ["core/role:orchestrator", "core/pack:operations"]
        ]
    );
}

#[test]
fn discovery_imports_and_public_metadata_are_observable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().join("home");
    let config_dir = home.join(".roster");
    let source_root = temp.path().join("source");
    source(&source_root, "core", "powder");
    let imported = config_dir.join("base.yaml");
    config(&imported, "core", &source_root, "amos", "codex");
    let mut body = fs::read_to_string(&imported)
        .expect("base config")
        .replace("delegates: []", "delegates: [reviewer]");
    body.push_str(
        "  reviewer:\n    description: Independent reviewer\n    role: core/role:orchestrator\n    model: test/model\n    harness: claude\n    args: []\n    delegates: []\nauthority:\n  command: authority-provider\n  args: [request]\n",
    );
    write(&imported, &body);
    let config_path = config_dir.join("config.yaml");
    write(
        &config_path,
        "schema_version: roster.config.v1\nimports: [base.yaml]\nsources: {}\nagents: {}\n",
    );
    let workspace = temp.path().join("workspace/project");
    fs::create_dir_all(&workspace).expect("workspace");

    let found = discover_config(&workspace, &home).expect("home fallback");
    assert_eq!(found, config_path);
    let roster = Roster::load_config(&found).expect("imported roster");
    assert_eq!(roster.config_path(), found);
    assert_eq!(roster.source_roots().collect::<Vec<_>>(), [&source_root]);
    assert_eq!(
        roster.authority().expect("authority").command,
        "authority-provider"
    );
    let markdown = roster.resolve("amos").expect("resolve").agents_markdown();
    assert!(markdown.contains("## Delegation"));
    assert!(markdown.contains("`reviewer`"));
}

#[test]
fn imports_fail_closed_on_cycles_and_duplicates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");

    let cycle = temp.path().join("cycle");
    write(
        &cycle.join("a.yaml"),
        "schema_version: roster.config.v1\nimports: [b.yaml]\nsources: {}\nagents: {}\n",
    );
    write(
        &cycle.join("b.yaml"),
        "schema_version: roster.config.v1\nimports: [a.yaml]\nsources: {}\nagents: {}\n",
    );
    let error = Roster::load_config(cycle.join("a.yaml")).expect_err("cycle must fail");
    assert!(error.to_string().contains("config import cycle"), "{error}");

    let duplicate_source = temp.path().join("duplicate-source");
    config(
        &duplicate_source.join("base.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    write(
        &duplicate_source.join("config.yaml"),
        &format!(
            "schema_version: roster.config.v1\nimports: [base.yaml]\nsources:\n  core: {}\nagents: {{}}\n",
            root.display()
        ),
    );
    let error = Roster::load_config(duplicate_source.join("config.yaml"))
        .expect_err("duplicate source must fail");
    assert!(error.to_string().contains("duplicate source"), "{error}");

    let duplicate_agent = temp.path().join("duplicate-agent");
    config(
        &duplicate_agent.join("base.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    write(
        &duplicate_agent.join("config.yaml"),
        "schema_version: roster.config.v1\nimports: [base.yaml]\nsources: {}\nagents:\n  amos:\n    description: duplicate\n    role: core/role:orchestrator\n    model: test/model\n    harness: codex\n",
    );
    let error = Roster::load_config(duplicate_agent.join("config.yaml"))
        .expect_err("duplicate agent must fail");
    assert!(error.to_string().contains("duplicate agent"), "{error}");

    let duplicate_authority = temp.path().join("duplicate-authority");
    config(
        &duplicate_authority.join("base.yaml"),
        "core",
        &root,
        "amos",
        "codex",
    );
    let base = fs::read_to_string(duplicate_authority.join("base.yaml")).expect("base");
    write(
        &duplicate_authority.join("base.yaml"),
        &format!("{base}authority:\n  command: first\n"),
    );
    write(
        &duplicate_authority.join("config.yaml"),
        "schema_version: roster.config.v1\nimports: [base.yaml]\nsources: {}\nagents: {}\nauthority:\n  command: second\n",
    );
    let error = Roster::load_config(duplicate_authority.join("config.yaml"))
        .expect_err("duplicate authority must fail");
    assert!(error.to_string().contains("duplicate authority"), "{error}");
}

#[test]
fn config_and_harness_arguments_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    let path = temp.path().join("config.yaml");
    let document = |sources: &str, agents: &str| {
        format!("schema_version: roster.config.v1\nsources: {sources}\nagents: {agents}\n")
    };
    let agent = |description: &str, harness: &str, args: &str, delegates: &str| {
        format!(
            "\n  amos:\n    description: {description:?}\n    role: core/role:orchestrator\n    model: test/model\n    harness: {harness}\n    args: {args}\n    delegates: {delegates}\n"
        )
    };
    let sources = format!("\n  core: {}", root.display());
    let cases = [
        (
            "{}".to_owned(),
            agent("lead", "codex", "[]", "[]"),
            "no sources",
        ),
        (sources.clone(), "{}".to_owned(), "no agents"),
        (
            format!("\n  Unsafe: {}", root.display()),
            agent("lead", "codex", "[]", "[]"),
            "unsafe source name",
        ),
        (
            sources.clone(),
            agent("", "codex", "[]", "[]"),
            "incomplete agent",
        ),
        (
            sources.clone(),
            agent("lead", "codex", "[]", "[../escape]"),
            "unsafe delegate name",
        ),
        (
            sources.clone(),
            agent("lead", "codex", "[]", "[missing]"),
            "unknown delegate",
        ),
        (
            sources.clone(),
            agent("lead", "codex", "[--sandbox]", "[]"),
            "requires a value",
        ),
        (
            sources.clone(),
            agent("lead", "codex", "[--sandbox, invalid]", "[]"),
            "invalid --sandbox value",
        ),
        (
            sources.clone(),
            agent("lead", "claude", "[--model, other]", "[]"),
            "unsafe or topology-changing",
        ),
        (
            sources.clone(),
            agent("lead", "omp", "[--model, other]", "[]"),
            "unsafe or topology-changing",
        ),
    ];
    for (sources, agents, expected) in cases {
        write(&path, &document(&sources, &agents));
        let error = Roster::load_config(&path).expect_err(expected);
        assert!(error.to_string().contains(expected), "{expected}: {error}");
    }

    write(
        &path,
        &format!(
            "schema_version: roster.config.v1\nsources:{sources}\nagents:\n  codex:\n    description: codex\n    role: core/role:orchestrator\n    model: test/model\n    harness: codex\n    args: [--sandbox, read-only, --ask-for-approval, never]\n  claude:\n    description: claude\n    role: core/role:orchestrator\n    model: test/model\n    harness: claude\n    args: [--permission-mode, plan]\n  omp:\n    description: omp\n    role: core/role:orchestrator\n    model: test/model\n    harness: omp\n    args: [--approval-mode, write]\n"
        ),
    );
    let roster = Roster::load_config(&path).expect("allowlisted Harness arguments");
    assert_eq!(roster.agents().len(), 3);
    let error = Harness::from_str("mystery").expect_err("unsupported Harness must fail");
    assert!(error.to_string().contains("unsupported Harness"));
}

#[test]
fn resolution_rejects_semantic_role_and_pack_drift() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("source");
    source(&root, "core", "powder");
    let path = temp.path().join("config.yaml");
    config(&path, "core", &root, "amos", "codex");

    let body = fs::read_to_string(&path)
        .expect("config")
        .replace("core/role:orchestrator", "core/pack:orchestrator");
    write(&path, &body);
    let roster = Roster::load_config(&path).expect("load wrong kind");
    let error = roster.resolve("amos").expect_err("role kind must fail");
    assert!(
        error.to_string().contains("must identify a role"),
        "{error}"
    );

    config(&path, "core", &root, "amos", "codex");
    write(
        &root.join("roles/orchestrator.yaml"),
        "schema_version: roster.role.v2\nname: other\ndescription: drift\ninclude: []\n",
    );
    let roster = Roster::load_config(&path).expect("load role drift");
    let error = roster
        .resolve("amos")
        .expect_err("role name drift must fail");
    assert!(error.to_string().contains("declares role"), "{error}");

    write(
        &root.join("roles/orchestrator.yaml"),
        "schema_version: roster.role.v2\nname: orchestrator\ndescription: drift\ninclude: [core/role:other]\n",
    );
    let roster = Roster::load_config(&path).expect("load nested role");
    let error = roster.resolve("amos").expect_err("nested role must fail");
    assert!(
        error.to_string().contains("roles cannot include roles"),
        "{error}"
    );

    write(
        &root.join("roles/orchestrator.yaml"),
        "schema_version: roster.role.v2\nname: orchestrator\ndescription: cycle\ninclude: [core/pack:a]\n",
    );
    write(
        &root.join("packs/a.yaml"),
        "schema_version: roster.pack.v1\nname: a\ninclude: [core/pack:b]\n",
    );
    write(
        &root.join("packs/b.yaml"),
        "schema_version: roster.pack.v1\nname: b\ninclude: [core/pack:a]\n",
    );
    let roster = Roster::load_config(&path).expect("load pack cycle");
    let error = roster.resolve("amos").expect_err("pack cycle must fail");
    assert!(error.to_string().contains("pack cycle"), "{error}");

    write(
        &root.join("packs/a.yaml"),
        "schema_version: roster.pack.v1\nname: wrong\ninclude: []\n",
    );
    let roster = Roster::load_config(&path).expect("load pack drift");
    let error = roster
        .resolve("amos")
        .expect_err("pack name drift must fail");
    assert!(error.to_string().contains("declares pack"), "{error}");

    write(
        &root.join("roles/orchestrator.yaml"),
        "schema_version: roster.role.v2\nname: orchestrator\ndescription: inactive\ninclude: [core/mcp:powder]\n",
    );
    write(
        &root.join("primitives/mcps/registry.yaml"),
        "schema_version: roster.mcp_registry.v1\nprovenance: fixture\nmcps:\n  - id: powder\n    status: disabled\n",
    );
    let roster = Roster::load_config(&path).expect("load inactive MCP");
    let error = roster.resolve("amos").expect_err("inactive MCP must fail");
    assert!(
        error.to_string().contains("non-launchable status"),
        "{error}"
    );
}

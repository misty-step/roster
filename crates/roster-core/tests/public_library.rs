use roster_core::Roster;
use std::fs;
use std::path::PathBuf;

#[test]
fn public_library_contains_no_operator_identifiers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .expect("list tracked public-library files");
    assert!(output.status.success(), "git ls-files failed");

    let mut findings = Vec::new();
    for raw_path in output.stdout.split(|byte| *byte == 0) {
        if raw_path.is_empty() {
            continue;
        }
        let path = PathBuf::from(String::from_utf8_lossy(raw_path).as_ref());
        let Ok(content) = fs::read_to_string(root.join(&path)) else {
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            let labels = public_library_privacy_findings(line);
            for label in labels {
                findings.push(format!("{}:{}: {label}", path.display(), line_index + 1));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "public library contains operator-specific material:\n{}",
        findings.join("\n")
    );
}

fn public_library_privacy_findings(line: &str) -> Vec<&'static str> {
    let mut findings = Vec::new();
    let macos_home_prefix = concat!("/", "Users", "/");
    let tailnet_suffix = concat!(".ts", ".net");

    if line.contains(macos_home_prefix) {
        findings.push("absolute macOS home path");
    }
    if line.contains(tailnet_suffix) {
        findings.push("private Tailnet hostname");
    }
    if email_addresses(line).any(|email| !allowed_public_email(email)) {
        findings.push("non-allowlisted email address");
    }

    findings
}

fn email_addresses(line: &str) -> impl Iterator<Item = &str> {
    line.match_indices('@').filter_map(|(at, _)| {
        let bytes = line.as_bytes();
        let mut start = at;
        while start > 0 && is_email_local_byte(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = at + 1;
        while end < bytes.len() && is_email_domain_byte(bytes[end]) {
            end += 1;
        }

        let email = &line[start..end];
        let domain = email.split_once('@')?.1;
        let looks_like_format_string = email[..at - start].contains('%');
        (!looks_like_format_string
            && start < at
            && domain.contains('.')
            && domain
                .rsplit_once('.')
                .is_some_and(|(_, suffix)| suffix.chars().all(|ch| ch.is_ascii_alphabetic())))
        .then_some(email)
    })
}

fn is_email_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn is_email_domain_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

fn allowed_public_email(email: &str) -> bool {
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    domain == "example.com"
        || domain == "users.noreply.github.com"
        || (local == "hey" && domain == "herdr.dev")
}

#[test]
fn public_library_privacy_oracle_is_structural() {
    let home = concat!("/", "Users", "/", "someone-else", "/work");
    let tailnet = concat!("service.example", ".ts", ".net");
    let private_email = concat!("operator", "@", "personal.dev");

    assert_eq!(
        public_library_privacy_findings(home),
        ["absolute macOS home path"]
    );
    assert_eq!(
        public_library_privacy_findings(tailnet),
        ["private Tailnet hostname"]
    );
    assert_eq!(
        public_library_privacy_findings(private_email),
        ["non-allowlisted email address"]
    );
    assert!(public_library_privacy_findings("person@example.com").is_empty());
    assert!(public_library_privacy_findings("%s@github.com").is_empty());
}

#[test]
fn every_example_agent_resolves_from_the_public_library() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");
    assert_eq!(roster.agents().len(), 13);
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

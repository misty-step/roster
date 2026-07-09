use assert_cmd::Command;
use predicates::prelude::*;
use std::{fs, path::Path};
use tempfile::TempDir;

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn write(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, contents).expect("write fixture file");
}

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn check_fails_on_broken_frontmatter_and_dead_reference() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();

    write(
        root,
        "primitives/skills/skills-index.yaml",
        "schema_version: roster.skills_index.v1\nskills: []\n",
    );
    write(
        root,
        "primitives/skills/broken/SKILL.md",
        "---\ndescription: only a description, no name\n---\nbody\n",
    );
    write(
        root,
        "primitives/skills/dead-ref/SKILL.md",
        "---\nname: dead-ref\ndescription: cites a path that does not exist\nargument-hint: \"[x]\"\n---\n\nSee `primitives/shared/references/does-not-exist.md` for detail.\n",
    );

    git(root, &["init", "-q"]);
    git(
        root,
        &["config", "user.email", "roster-check-test@example.com"],
    );
    git(root, &["config", "user.name", "roster-check-test"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "fixture"]);

    Command::cargo_bin("roster")
        .expect("roster binary")
        .env_remove("CANARY_ENDPOINT")
        .env_remove("CANARY_API_KEY")
        .env_remove("CANARY_INGEST_KEY")
        .args(["--root", root.to_str().expect("utf8 path"), "check"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing/empty name"))
        .stdout(predicate::str::contains("dead reference"));
}

#[test]
fn check_warns_but_passes_on_past_due_review_date() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();

    write(
        root,
        "primitives/skills/skills-index.yaml",
        "schema_version: roster.skills_index.v1\nskills:\n  - name: stale-index\n    path: primitives/skills/stale-index/SKILL.md\n",
    );
    write(
        root,
        "primitives/skills/stale-index/SKILL.md",
        "---\nname: stale-index\ndescription: skill with a dated reference file\nargument-hint: \"[x]\"\n---\nbody\n",
    );
    write(
        root,
        "primitives/skills/stale-index/references/facts.md",
        "---\nmodel_reference_review_due: 2020-01-01\nlast_researched: 2019-12-01\n---\n\n# Facts\n",
    );

    git(root, &["init", "-q"]);
    git(
        root,
        &["config", "user.email", "roster-check-test@example.com"],
    );
    git(root, &["config", "user.name", "roster-check-test"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "fixture"]);

    Command::cargo_bin("roster")
        .expect("roster binary")
        .env_remove("CANARY_ENDPOINT")
        .env_remove("CANARY_API_KEY")
        .env_remove("CANARY_INGEST_KEY")
        .args(["--root", root.to_str().expect("utf8 path"), "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "WARN primitives/skills/stale-index/references/facts.md: model_reference_review_due 2020-01-01 is past due",
        ));
}

#[test]
fn check_accepts_consistent_external_provenance_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    write(
        root,
        "primitives/skills/skills-index.yaml",
        "schema_version: roster.skills_index.v1\nskills: []\n",
    );
    write(
        root,
        "primitives/skills/.external/registry.yaml",
        "sources:\n  - repo: example/skills\n    pin: 0123456789abcdef0123456789abcdef01234567\n    alias_prefix: example-\n    include: [motion]\n",
    );
    write(
        root,
        "primitives/skills/.external/example-motion/SKILL.md",
        "---\nname: motion\ndescription: Motion guidance.\n---\nbody\n",
    );
    write(
        root,
        "primitives/skills/.external/example-motion/.sync-meta.json",
        r#"{"repo":"example/skills","sha":"0123456789abcdef0123456789abcdef01234567","src_path_suffix":"motion"}"#,
    );

    git(root, &["init", "-q"]);
    git(root, &["add", "-A"]);

    Command::cargo_bin("roster")
        .expect("roster binary")
        .args(["--root", root.to_str().expect("utf8 path"), "check"])
        .assert()
        .success();
}

#[test]
fn check_reports_external_alias_receipt_and_declaration_drift() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    write(
        root,
        "primitives/skills/skills-index.yaml",
        "schema_version: roster.skills_index.v1\nskills: []\n",
    );
    write(
        root,
        "primitives/skills/.external/registry.yaml",
        "sources:\n  - repo: example/skills\n    pin: 0123456789abcdef0123456789abcdef01234567\n    alias_prefix: example-\n    include: [motion, missing]\n  - repo: collision/skills\n    pin: fedcba9876543210fedcba9876543210fedcba98\n    alias_prefix: example-\n    include: [motion]\n",
    );
    write(
        root,
        "primitives/skills/.external/example-motion/SKILL.md",
        "---\nname: motion\ndescription: Motion guidance.\n---\nbody\n",
    );
    write(
        root,
        "primitives/skills/.external/example-motion/.sync-meta.json",
        r#"{"repo":"wrong/repo","sha":"wrong-pin","src_path_suffix":"wrong-name"}"#,
    );
    write(
        root,
        "primitives/skills/.external/undeclared/SKILL.md",
        "---\nname: undeclared\ndescription: Undeclared skill.\n---\nbody\n",
    );

    git(root, &["init", "-q"]);
    git(root, &["add", "-A"]);

    Command::cargo_bin("roster")
        .expect("roster binary")
        .args(["--root", root.to_str().expect("utf8 path"), "check"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("alias collision example-motion"))
        .stdout(predicate::str::contains(
            "declared alias example-missing has no vendored SKILL.md",
        ))
        .stdout(predicate::str::contains("repo mismatch"))
        .stdout(predicate::str::contains("sha mismatch"))
        .stdout(predicate::str::contains("src_path_suffix mismatch"))
        .stdout(predicate::str::contains(
            "undeclared: vendored skill is not declared",
        ));
}

#[test]
fn check_passes_on_the_real_repo() {
    Command::cargo_bin("roster")
        .expect("roster binary")
        .current_dir(workspace_root())
        .env_remove("CANARY_ENDPOINT")
        .env_remove("CANARY_API_KEY")
        .env_remove("CANARY_INGEST_KEY")
        .arg("check")
        .assert()
        .success();
}

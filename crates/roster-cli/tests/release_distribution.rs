use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{fs, path::PathBuf, process::Command};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        pair => panic!("unsupported release-test host {pair:?}"),
    }
}

#[test]
fn release_archive_installs_and_drives_the_public_cold_start() {
    let root = repository_root();
    let temp = tempfile::tempdir().expect("tempdir");
    let dist = temp.path().join("dist");
    fs::create_dir_all(&dist).expect("dist directory");
    let binary = assert_cmd::cargo::cargo_bin("roster");

    Command::new(root.join("scripts/package-release"))
        .args([
            binary.as_os_str(),
            "0.2.0".as_ref(),
            host_target().as_ref(),
            dist.as_os_str(),
        ])
        .env("SOURCE_DATE_EPOCH", "1784116800")
        .assert()
        .success();

    let archive = dist.join(format!("roster-v0.2.0-{}.tar.gz", host_target()));
    assert!(archive.is_file(), "missing {}", archive.display());
    let second_dist = temp.path().join("second-dist");
    fs::create_dir_all(&second_dist).expect("second dist directory");
    Command::new(root.join("scripts/package-release"))
        .args([
            binary.as_os_str(),
            "0.2.0".as_ref(),
            host_target().as_ref(),
            second_dist.as_os_str(),
        ])
        .env("SOURCE_DATE_EPOCH", "1784116800")
        .assert()
        .success();
    assert_eq!(
        fs::read(&archive).expect("first archive"),
        fs::read(second_dist.join(archive.file_name().expect("archive name")))
            .expect("second archive"),
        "release packaging is not reproducible"
    );
    Command::new(root.join("scripts/package-release"))
        .current_dir(temp.path())
        .args([
            binary.as_os_str(),
            "0.2.0".as_ref(),
            host_target().as_ref(),
            "relative-dist".as_ref(),
        ])
        .assert()
        .success();
    assert!(
        temp.path()
            .join("relative-dist")
            .join(archive.file_name().expect("archive name"))
            .is_file()
    );
    Command::new("tar")
        .args([
            "-xzf".as_ref(),
            archive.as_os_str(),
            "-C".as_ref(),
            temp.path().as_os_str(),
        ])
        .assert()
        .success();

    let package = temp.path().join(format!("roster-v0.2.0-{}", host_target()));
    assert!(package.join("LICENSE").is_file());
    let release_manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(package.join("release-manifest.json")).expect("release manifest"),
    )
    .expect("valid release manifest");
    assert_eq!(release_manifest["version"], "0.2.0");
    assert_eq!(release_manifest["target"], host_target());
    assert_eq!(
        release_manifest["library"],
        serde_json::Value::String("share/roster".into())
    );
    assert!(!package.join("share/roster/.git").exists());
    assert!(!package.join("share/roster/primitives/.DS_Store").exists());
    assert_eq!(
        fs::read_to_string(package.join("share/roster/VERSION")).expect("version marker"),
        "0.2.0\n"
    );

    let prefix = temp.path().join("prefix");
    Command::new(package.join("install.sh"))
        .args(["--prefix".as_ref(), prefix.as_os_str()])
        .assert()
        .success();

    let installed = prefix.join("bin/roster");
    Command::new(&installed)
        .env_remove("ROSTER_CONFIG")
        .arg("--version")
        .assert()
        .success()
        .stdout("roster 0.2.0\n");

    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(&workspace).expect("workspace");
    Command::new(&installed)
        .env_remove("ROSTER_CONFIG")
        .env("HOME", &home)
        .args(["--cwd".as_ref(), workspace.as_os_str(), "init".as_ref()])
        .assert()
        .success();

    let config = fs::read_to_string(workspace.join(".roster/config.yaml")).expect("config");
    assert!(
        config.contains(&prefix.join("share/roster").display().to_string()),
        "installed source missing from config:\n{config}"
    );

    Command::new(&installed)
        .env_remove("ROSTER_CONFIG")
        .env("HOME", &home)
        .args(["--cwd".as_ref(), workspace.as_os_str(), "check".as_ref()])
        .assert()
        .success()
        .stdout(predicate::str::contains("roster graph: ok"));
    Command::new(&installed)
        .env_remove("ROSTER_CONFIG")
        .env("HOME", &home)
        .args([
            "--cwd".as_ref(),
            workspace.as_os_str(),
            "show".as_ref(),
            "amos".as_ref(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("role: starter"))
        .stdout(predicate::str::contains("mcps:\n"));

    let bundle = temp.path().join("bundle");
    Command::new(&installed)
        .env_remove("ROSTER_CONFIG")
        .env("HOME", &home)
        .args([
            "--cwd".as_ref(),
            workspace.as_os_str(),
            "resolve".as_ref(),
            "amos".as_ref(),
            "--output".as_ref(),
            bundle.as_os_str(),
        ])
        .assert()
        .success();
    let manifest = fs::read_to_string(bundle.join("manifest.yaml")).expect("manifest");
    assert!(!manifest.contains(&root.display().to_string()));
    assert!(!bundle.join("mcps").exists());

    // Same-version reinstall proves repair and installer idempotency. A later
    // cross-version rollback uses this identical path with an older archive.
    fs::write(&installed, "damaged").expect("damage installed binary");
    Command::new(package.join("install.sh"))
        .args(["--prefix".as_ref(), prefix.as_os_str()])
        .assert()
        .success();
    Command::new(&installed)
        .env_remove("ROSTER_CONFIG")
        .arg("--version")
        .assert()
        .success()
        .stdout("roster 0.2.0\n");
}

#[test]
fn release_workflow_keeps_version_intelligence_provenance_and_live_replay() {
    let root = repository_root();
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).expect("release changelog");
    let release_section = changelog
        .split("## [0.2.0]")
        .nth(1)
        .expect("release section [0.2.0] not found");
    let release_section = release_section
        .split("## [")
        .next()
        .expect("release section body");
    assert!(
        release_section.lines().any(|line| line.starts_with("- ")),
        "release section [0.2.0] needs evidence bullets"
    );
    Command::new(root.join("scripts/check-release-version"))
        .arg("v0.2.0")
        .assert()
        .success()
        .stdout("release version: 0.2.0\n");
    Command::new(root.join("scripts/check-release-version"))
        .arg("v0.2.1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("tag is 0.2.1"));
    Command::new(root.join("scripts/check-release-version"))
        .arg("v0.2.0-rc.1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid tag"));

    let workflow =
        fs::read_to_string(root.join(".github/workflows/release.yml")).expect("release workflow");
    serde_yaml::from_str::<serde_yaml::Value>(&workflow).expect("valid workflow YAML");
    for required in [
        "scripts/check-release-version",
        "landmark run --provider local",
        "landmark synthesize",
        "--version \"${GITHUB_REF_NAME#v}\"",
        "actions/attest@v4",
        "landmark-synthesis-quality.txt)\" = valid",
        "--notes-file dist/landmark-release-notes.md",
        "gh attestation verify",
        "roster --cwd \"$workspace\" check",
        "roster --cwd \"$workspace\" resolve amos",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER: rust-lld",
        "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER: rust-lld",
        "accept:\n    needs: build",
        "publish:\n    needs: [validate, build, accept]",
        "cold-start:\n    needs: publish",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow lost {required}"
        );
    }
    assert!(
        !workflow.contains("apt-get"),
        "release builds must not depend on apt"
    );
}

#[test]
fn release_shell_surfaces_keep_portable_failure_boundaries() {
    let root = repository_root();
    let package = fs::read_to_string(root.join("scripts/package-release")).expect("packager");
    assert!(package.contains("export COPYFILE_DISABLE=1\n"));
    assert!(package.contains("cp -X \"$1\" \"$2\" 2>/dev/null || cp \"$1\" \"$2\""));
    assert!(package.contains("work=$(mktemp -d)\n"));
    assert!(package.contains("> \"$work/files.txt\"\n"));
    assert!(package.contains("done < \"$work/files.txt\"\n"));

    let landmark = fs::read_to_string(root.join("scripts/fetch-landmark")).expect("fetcher");
    assert!(landmark.contains("checksums=$(mktemp)\n"));

    let installer = fs::read_to_string(root.join("packaging/install.sh")).expect("installer");
    assert!(installer.contains("elif [ \"$library_installed\" -eq 1 ]; then"));
    assert!(installer.contains("elif [ \"$binary_installed\" -eq 1 ]; then"));

    let get_started = fs::read_to_string(root.join("site/get-started.html")).expect("site docs");
    assert!(get_started.contains("*) echo \"unsupported host\" &gt;&amp;2; exit 1 ;;"));

    let releasing = fs::read_to_string(root.join("docs/RELEASING.md")).expect("release docs");
    assert!(releasing.contains("404) ;;"));
    assert!(releasing.contains("could not prove v0.2.0 is unpublished"));
}

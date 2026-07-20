use roster_core::Roster;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn public_library_contains_no_operator_identifiers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
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
fn public_library_boundary_is_behavioral_and_graph_complete() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().expect("boundary tempdir");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");
    let amos = roster.resolve("amos").expect("resolve public orchestrator");
    let artifact_bundle = temp.path().join("artifact-bundle");
    let artifact_manifest = amos
        .write_bundle(&artifact_bundle, temp.path())
        .expect("materialize public Artifact projection");
    let artifact_create = artifact_bundle.join("skills/artifact/scripts/artifact_create.py");
    let artifact_serve = artifact_bundle.join("skills/artifact/scripts/artifact_serve.py");
    assert!(artifact_create.is_file(), "materialized renderer missing");
    assert!(artifact_serve.is_file(), "materialized server missing");
    assert!(
        artifact_bundle
            .join("skills/artifact/scripts/artifact_fs.py")
            .is_file(),
        "descriptor filesystem module missing from materialized Artifact"
    );
    let artifact_root = temp.path().join("artifacts");
    fs::create_dir_all(&artifact_root).expect("artifact root");
    fs::write(artifact_root.join("index.html"), "public artifact").expect("artifact page");
    fs::create_dir(artifact_root.join("redirect-me")).expect("redirect fixture directory");
    fs::write(
        artifact_root.join("redirect-me/index.html"),
        "redirect target",
    )
    .expect("redirect fixture page");
    fs::write(artifact_root.join("%ZZ"), "malformed escape sentinel")
        .expect("malformed escape fixture");
    #[cfg(unix)]
    {
        let outside = temp.path().join("outside-secret.txt");
        fs::write(&outside, "outside secret").expect("outside fixture");
        std::os::unix::fs::symlink(&outside, artifact_root.join("escape"))
            .expect("artifact escape symlink");
        std::os::unix::fs::symlink(
            artifact_root.join("index.html"),
            artifact_root.join("internal-link"),
        )
        .expect("artifact internal symlink");
        std::os::unix::fs::symlink(&outside, artifact_root.join(".roster-not-found"))
            .expect("hostile not-found sentinel");
    }

    let port = TcpListener::bind(("127.0.0.1", 0))
        .expect("reserve loopback port")
        .local_addr()
        .expect("loopback address")
        .port();
    let mut server = Command::new("python3")
        .arg(&artifact_serve)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--root",
            artifact_root.to_str().expect("artifact root utf8"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch artifact server");
    wait_for_loopback(port);

    let (status, body) = http_exchange(port, "GET", "/index.html", "");
    assert_eq!(status, 200);
    assert!(body.contains("public artifact"));
    let (status, body) = http_exchange(port, "GET", "/", "");
    assert_eq!(status, 200);
    assert!(body.contains("public artifact"));
    let (status, _) = http_exchange(port, "GET", "/%00", "");
    assert_eq!(status, 404);
    for malformed_target in ["/%ZZ", "/%A"] {
        let (status, body) = http_exchange(port, "GET", malformed_target, "");
        assert_eq!(status, 404);
        assert!(!body.contains("malformed escape sentinel"));
    }
    let (status, body) = http_exchange(port, "GET", "//attacker.example", "");
    assert_eq!(status, 404);
    assert!(!body.contains("Location: //attacker.example"));
    for non_origin_target in ["http://attacker.example/index.html", "attacker.example:80"] {
        let (status, body) = http_exchange(port, "GET", non_origin_target, "");
        assert!(!(200..400).contains(&status));
        assert!(!body.contains("public artifact"));
        assert!(!body.contains("Location:"));
    }
    let (status, body) = http_exchange(port, "GET", "/redirect-me?mode=proof", "");
    assert_eq!(status, 301);
    assert!(body.contains("Location: /redirect-me/?mode=proof"));
    assert!(!body.contains("Location: //"));
    let (status, body) = http_exchange(port, "GET", "////redirect-me?mode=proof", "");
    assert_eq!(status, 301);
    assert!(body.contains("Location: /redirect-me/?mode=proof"));
    assert!(!body.contains("Location: //"));
    #[cfg(unix)]
    {
        let (status, body) = http_exchange(port, "GET", "/escape", "");
        assert_eq!(status, 404);
        assert!(!body.contains("outside secret"));
        let (status, body) = http_exchange(port, "GET", "/internal-link", "");
        assert_eq!(status, 404);
        assert!(!body.contains("public artifact"));
        let (status, body) = http_exchange(port, "GET", "/missing", "");
        assert_eq!(status, 404);
        assert!(!body.contains("outside secret"));

        let fifo = artifact_root.join("blocking-fifo");
        let mkfifo = Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("create non-regular artifact fixture");
        assert!(mkfifo.success());
        let (status, _) = http_exchange(port, "GET", "/blocking-fifo", "");
        assert_eq!(status, 404);
    }
    for method in ["POST", "PUT", "DELETE"] {
        let (status, _) = http_exchange(port, method, "/mutated.txt", "mutation");
        assert!(
            !(200..300).contains(&status),
            "{method} unexpectedly mutated artifacts"
        );
    }
    assert!(!artifact_root.join("mutated.txt").exists());
    server.kill().expect("stop artifact server");
    server.wait().expect("reap artifact server");

    let non_loopback = Command::new("python3")
        .arg(&artifact_serve)
        .args([
            "--host",
            "0.0.0.0",
            "--port",
            "0",
            "--root",
            artifact_root.to_str().expect("artifact root utf8"),
        ])
        .output()
        .expect("reject non-loopback artifact server");
    assert!(!non_loopback.status.success());

    #[cfg(unix)]
    {
        let root_link = temp.path().join("artifact-root-link");
        std::os::unix::fs::symlink(&artifact_root, &root_link).expect("artifact root symlink");
        let symlinked_root = Command::new("python3")
            .arg(&artifact_serve)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                "0",
                "--root",
                root_link.to_str().expect("root link utf8"),
            ])
            .output()
            .expect("reject symlinked artifact root");
        assert!(!symlinked_root.status.success());
    }

    let body_file = temp.path().join("report.md");
    fs::write(&body_file, "# Local report\n").expect("report body");
    let outbound_spy = TcpListener::bind(("127.0.0.1", 0)).expect("bind outbound spy");
    outbound_spy
        .set_nonblocking(true)
        .expect("make outbound spy nonblocking");
    let base_url = format!(
        "http://127.0.0.1:{}",
        outbound_spy.local_addr().expect("spy address").port()
    );
    let create = Command::new("python3")
        .arg(&artifact_create)
        .args([
            "--title",
            "Local report",
            "--slug",
            "local-report",
            "--body-file",
            body_file.to_str().expect("body utf8"),
            "--root",
            artifact_root.to_str().expect("artifact root utf8"),
            "--base-url",
            &base_url,
        ])
        .output()
        .expect("run local artifact renderer");
    assert!(
        create.status.success(),
        "artifact renderer failed: {create:?}"
    );
    assert!(String::from_utf8_lossy(&create.stdout).contains("\"local_only\": true"));
    assert!(!String::from_utf8_lossy(&create.stderr).contains("publish"));
    assert!(artifact_root.join("a/local-report/index.html").is_file());
    assert!(matches!(outbound_spy.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

    let rendered_port = TcpListener::bind(("127.0.0.1", 0))
        .expect("reserve rendered artifact port")
        .local_addr()
        .expect("rendered loopback address")
        .port();
    let mut rendered_server = Command::new("python3")
        .arg(&artifact_serve)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &rendered_port.to_string(),
            "--root",
            artifact_root.to_str().expect("artifact root utf8"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch rendered artifact server");
    wait_for_loopback(rendered_port);
    for path in ["/a/local-report/", "/a/index/", "/a/index/index.json"] {
        let (status, _) = http_exchange(rendered_port, "GET", path, "");
        assert_eq!(status, 200, "rendered Artifact route failed: {path}");
    }
    rendered_server
        .kill()
        .expect("stop rendered artifact server");
    rendered_server
        .wait()
        .expect("reap rendered artifact server");

    for external_url in [
        "https://attacker.example",
        "http://169.254.169.254",
        "javascript:alert(1)",
    ] {
        let rejected = Command::new("python3")
            .arg(&artifact_create)
            .args([
                "--title",
                "Rejected report",
                "--slug",
                "rejected-report",
                "--body-file",
                body_file.to_str().expect("body utf8"),
                "--root",
                artifact_root.to_str().expect("artifact root utf8"),
                "--base-url",
                external_url,
            ])
            .output()
            .expect("reject external artifact URL");
        assert!(
            !rejected.status.success(),
            "accepted external artifact URL {external_url}"
        );
    }
    #[cfg(unix)]
    {
        let body_link = temp.path().join("body-link.md");
        std::os::unix::fs::symlink(&body_file, &body_link).expect("body symlink");
        let symlinked_input = Command::new("python3")
            .arg(&artifact_create)
            .args([
                "--title",
                "Symlinked report",
                "--slug",
                "symlinked-report",
                "--body-file",
                body_link.to_str().expect("body link utf8"),
                "--root",
                artifact_root.to_str().expect("artifact root utf8"),
            ])
            .output()
            .expect("reject symlinked artifact input");
        assert!(!symlinked_input.status.success());

        let outside_output = temp.path().join("outside-output");
        fs::create_dir(&outside_output).expect("outside output directory");
        let escaped_slug = artifact_root.join("a/escape-render");
        std::os::unix::fs::symlink(&outside_output, &escaped_slug)
            .expect("artifact output symlink");
        let symlinked_output = Command::new("python3")
            .arg(&artifact_create)
            .args([
                "--title",
                "Escaped report",
                "--slug",
                "escape-render",
                "--body-file",
                body_file.to_str().expect("body utf8"),
                "--root",
                artifact_root.to_str().expect("artifact root utf8"),
            ])
            .output()
            .expect("reject symlinked artifact output");
        assert!(!symlinked_output.status.success());
        assert!(!outside_output.join("index.html").exists());

        let outside_page = temp.path().join("outside-page.html");
        fs::write(&outside_page, "outside page sentinel").expect("outside page fixture");
        let leaf_render = artifact_root.join("a/leaf-render");
        fs::create_dir(&leaf_render).expect("leaf render directory");
        std::os::unix::fs::symlink(&outside_page, leaf_render.join("index.html"))
            .expect("page leaf symlink");
        let reject_page_leaf = Command::new("python3")
            .arg(&artifact_create)
            .args([
                "--title",
                "Leaf-safe report",
                "--slug",
                "leaf-render",
                "--body-file",
                body_file.to_str().expect("body utf8"),
                "--root",
                artifact_root.to_str().expect("artifact root utf8"),
            ])
            .output()
            .expect("reject symlink page leaf");
        assert!(!reject_page_leaf.status.success(), "{reject_page_leaf:?}");
        assert_eq!(
            fs::read_to_string(&outside_page).expect("outside page unchanged"),
            "outside page sentinel"
        );
        assert!(
            fs::symlink_metadata(leaf_render.join("index.html"))
                .expect("rendered page metadata")
                .file_type()
                .is_symlink()
        );

        let registry = artifact_root.join("a/index");
        let outside_registry_json = temp.path().join("outside-registry.json");
        let outside_registry_html = temp.path().join("outside-registry.html");
        fs::write(&outside_registry_json, "registry json sentinel")
            .expect("outside registry JSON fixture");
        fs::write(&outside_registry_html, "registry html sentinel")
            .expect("outside registry HTML fixture");
        fs::remove_file(registry.join("index.json")).expect("remove registry JSON leaf");
        fs::remove_file(registry.join("index.html")).expect("remove registry HTML leaf");
        std::os::unix::fs::symlink(&outside_registry_json, registry.join("index.json"))
            .expect("registry JSON leaf symlink");
        std::os::unix::fs::symlink(&outside_registry_html, registry.join("index.html"))
            .expect("registry HTML leaf symlink");
        let reindex = Command::new("python3")
            .arg(&artifact_create)
            .args([
                "--reindex",
                "--root",
                artifact_root.to_str().expect("artifact root utf8"),
            ])
            .output()
            .expect("reject symlink registry leaves");
        assert!(!reindex.status.success(), "{reindex:?}");
        assert_eq!(
            fs::read_to_string(&outside_registry_json).expect("outside registry JSON unchanged"),
            "registry json sentinel"
        );
        assert_eq!(
            fs::read_to_string(&outside_registry_html).expect("outside registry HTML unchanged"),
            "registry html sentinel"
        );
        for leaf in ["index.json", "index.html"] {
            assert!(
                fs::symlink_metadata(registry.join(leaf))
                    .expect("registry leaf metadata")
                    .file_type()
                    .is_symlink()
            );
        }
    }

    let sprite_script =
        fs::read_to_string(root.join("primitives/skills/sprites/scripts/sprite-lane"))
            .expect("sprite script");
    let sprite_docs = format!(
        "{}\n{}",
        fs::read_to_string(root.join("primitives/skills/sprites/SKILL.md")).expect("sprite skill"),
        fs::read_to_string(root.join("primitives/skills/sprites/references/provisioning.md"))
            .expect("sprite provisioning")
    );
    assert!(sprite_script.contains("sprite_cli exec"));
    assert!(sprite_script.contains("cmd_prepare"));
    assert!(sprite_script.contains("launch_owner\": \"external"));
    assert!(sprite_script.contains("$HOME/.config/git"));
    assert!(sprite_script.contains("$HOME/.omp"));
    assert!(sprite_script.contains("stage_card.py"));
    assert!(
        artifact_bundle
            .join("skills/sprites/scripts/stage_card.py")
            .is_file(),
        "materialized Sprite omitted descriptor-safe card staging"
    );
    assert!(sprite_script.contains("env -i"));
    assert!(sprite_docs.contains("credential-free"));
    assert!(sprite_docs.contains("external"));
    assert!(!sprite_script.contains("OPENAI_API_KEY"));
    assert!(!sprite_script.contains("ANTHROPIC_API_KEY"));
    assert!(!sprite_script.contains("codex exec"));
    assert!(!sprite_script.contains("codex --"));
    assert!(!sprite_script.contains("--env"));
    assert!(!sprite_script.contains("gh auth token"));
    assert!(!sprite_script.contains("credential.helper store"));

    assert!(!roster.agents().contains_key("simons"));
    let exact_args = [
        (
            "amos",
            vec![
                "--search",
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "on-request",
            ],
        ),
        ("kaylee", vec!["--permission-mode", "acceptEdits"]),
        ("urza", vec!["--approval-mode", "write"]),
        (
            "hephaestus",
            vec![
                "--search",
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "on-request",
            ],
        ),
        (
            "cerberus",
            vec![
                "--search",
                "--sandbox",
                "read-only",
                "--ask-for-approval",
                "never",
            ],
        ),
        (
            "scully",
            vec![
                "--search",
                "--sandbox",
                "read-only",
                "--ask-for-approval",
                "never",
            ],
        ),
        ("eames", vec!["--permission-mode", "acceptEdits"]),
        (
            "ripley",
            vec![
                "--search",
                "--sandbox",
                "read-only",
                "--ask-for-approval",
                "never",
            ],
        ),
        (
            "tars",
            vec![
                "--search",
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "never",
            ],
        ),
        (
            "magellan",
            vec![
                "--search",
                "--sandbox",
                "read-only",
                "--ask-for-approval",
                "never",
            ],
        ),
        ("solomon", vec!["--permission-mode", "plan"]),
        (
            "smith",
            vec![
                "--search",
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "on-request",
            ],
        ),
    ];
    for (name, expected) in exact_args {
        assert_eq!(
            roster.agents()[name].args,
            expected,
            "{name} public example args drifted"
        );
    }
    for (name, agent) in roster.agents() {
        let args = agent.args.join(" ").to_ascii_lowercase().replace('=', " ");
        for forbidden in [
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-skip-permissions",
            "--approval-mode yolo",
            "--permission-mode bypasspermissions",
            "danger-full-access",
            "bypasspermissions",
            "--yolo",
            "full-auto",
        ] {
            assert!(
                !args.contains(forbidden),
                "{name} contains forbidden arg {forbidden}"
            );
        }
    }
    for path in ["examples/config.yaml", "README.md"] {
        let content = fs::read_to_string(root.join(path))
            .expect("read canonical public example surface")
            .to_ascii_lowercase()
            .replace('=', " ");
        for forbidden in [
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-skip-permissions",
            "--approval-mode yolo",
            "--permission-mode bypasspermissions",
            "danger-full-access",
            "bypasspermissions",
            "--yolo",
            "full-auto",
        ] {
            assert!(
                !content.contains(forbidden),
                "{path} contains forbidden arg {forbidden}"
            );
        }
    }

    let simons_graph = [
        "roles/simons.yaml",
        "packs/simons.yaml",
        "primitives/guidance/simons.md",
    ];
    for path in simons_graph {
        let content = fs::read_to_string(root.join(path))
            .expect("read Simons declaration")
            .to_ascii_lowercase();
        for forbidden in [
            "autonomous",
            "brokerage",
            "place orders",
            "live order",
            "live-order",
            "order.live",
            "broker writes",
            "portfolio-operator",
            "dexter",
        ] {
            assert!(
                !content.contains(forbidden),
                "{path} grants authority with {forbidden}"
            );
        }
    }
    let simons_config = temp.path().join("simons-config.yaml");
    fs::write(
        &simons_config,
        format!(
            "schema_version: roster.config.v1\nsources:\n  core: {}\nagents:\n  simons:\n    description: Research-only Simons graph.\n    role: core/role:simons\n    model: gpt-5.6-sol\n    harness: codex\n    args: []\n",
            root.display()
        ),
    )
    .expect("write Simons graph config");
    let simons = Roster::load_config(simons_config)
        .expect("load Simons graph config")
        .resolve("simons")
        .expect("resolve complete Simons role graph");
    assert_eq!(simons.guidance.len(), 1);
    assert_eq!(simons.guidance[0].identity, "core/guidance:simons");
    assert_eq!(simons.skills.len(), 1);
    assert_eq!(simons.skills[0].identity, "core/skill:orient");
    assert!(simons.mcps.is_empty());
    let simons_bundle = temp.path().join("simons-bundle");
    let simons_manifest = simons
        .write_bundle(&simons_bundle, temp.path())
        .expect("materialize research-only Simons bundle");
    let simons_agents = fs::read_to_string(simons_bundle.join("AGENTS.md"))
        .expect("read materialized Simons guidance")
        .to_ascii_lowercase();
    assert!(simons_agents.contains("research-only"));
    assert!(simons_agents.contains("do not access or operate a financial account"));
    for forbidden in [
        "place orders",
        "live order",
        "live-order",
        "broker writes",
        "portfolio-operator",
        "dexter",
    ] {
        assert!(
            !simons_agents.contains(forbidden),
            "materialized Simons guidance grants authority with {forbidden}"
        );
    }
    assert!(
        simons_manifest
            .files
            .keys()
            .any(|path| path == std::path::Path::new("skills/orient/SKILL.md")),
        "materialized Simons bundle omitted its only skill"
    );
    assert!(
        simons_manifest
            .files
            .keys()
            .all(|path| !path.to_string_lossy().contains("public-equity-investing")),
        "materialized Simons bundle gained an undeclared execution surface"
    );

    let artifact_files = artifact_manifest
        .files
        .keys()
        .filter(|path| path.starts_with("skills/artifact"))
        .collect::<Vec<_>>();
    assert!(!artifact_files.is_empty(), "amos did not project Artifact");
    for relative in artifact_files {
        let Ok(content) = fs::read_to_string(artifact_bundle.join(relative)) else {
            continue;
        };
        let content = content.to_ascii_lowercase();
        for forbidden in [
            "powder",
            "workbench-",
            "scratchpad",
            "retro bridge",
            "hermes",
        ] {
            assert!(
                !content.contains(forbidden),
                "materialized Artifact contains private workflow term {forbidden}: {}",
                relative.display()
            );
        }
    }
    let sprite_files = artifact_manifest
        .files
        .keys()
        .filter(|path| path.starts_with("skills/sprites"))
        .collect::<Vec<_>>();
    assert!(!sprite_files.is_empty(), "amos did not project Sprite");
    for relative in sprite_files {
        let Ok(content) = fs::read_to_string(artifact_bundle.join(relative)) else {
            continue;
        };
        let content = content.to_ascii_lowercase();
        for forbidden in [
            "glass",
            "powder",
            "workbench-",
            "scratchpad",
            "retro bridge",
            "openai_api_key",
            "anthropic_api_key",
            "codex exec",
            "codex --",
        ] {
            assert!(
                !content.contains(forbidden),
                "materialized Sprite contains forbidden term {forbidden}: {}",
                relative.display()
            );
        }
    }
    let bundle_agents = fs::read_to_string(artifact_bundle.join("AGENTS.md"))
        .expect("read canonical public bundle guidance")
        .to_ascii_lowercase();
    assert!(
        !bundle_agents.contains("simons"),
        "research-only Simons role leaked into the canonical public agent"
    );
    assert!(
        artifact_manifest.files.keys().all(|path| !path
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("simons")),
        "research-only Simons files leaked into the canonical public projection"
    );
}

fn wait_for_loopback(port: u16) {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("artifact server did not bind loopback port {port}");
}

fn http_exchange(port: u16, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect artifact server");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set HTTP timeout");
    let request = format!(
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .expect("write HTTP request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read HTTP response");
    let response = String::from_utf8_lossy(&response);
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .expect("HTTP status");
    (status, response.into_owned())
}

#[cfg(unix)]
#[test]
fn artifact_helpers_reject_concurrent_parent_symlink_swaps() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().expect("artifact race tempdir");
    let bundle = temp.path().join("bundle");
    Roster::load_config(root.join("examples/config.yaml"))
        .expect("load public config for Artifact race")
        .resolve("amos")
        .expect("resolve public Artifact race bundle")
        .write_bundle(&bundle, temp.path())
        .expect("materialize Artifact race bundle");
    let renderer = bundle.join("skills/artifact/scripts/artifact_create.py");
    let server_script = bundle.join("skills/artifact/scripts/artifact_serve.py");
    assert!(
        bundle
            .join("skills/artifact/scripts/artifact_fs.py")
            .is_file(),
        "materialized race bundle omitted descriptor helper"
    );

    let input_path = temp.path().join("input-parent");
    let parked_input = temp.path().join("input-parent-parked");
    let outside_input = temp.path().join("outside-input");
    fs::create_dir(&input_path).expect("safe input parent");
    fs::create_dir(&outside_input).expect("outside input parent");
    fs::write(input_path.join("report.md"), "SAFE_INPUT_SENTINEL\n").expect("safe input");
    fs::write(outside_input.join("report.md"), "OUTSIDE_INPUT_SENTINEL\n").expect("outside input");
    let output_root = temp.path().join("rendered");
    fs::create_dir(&output_root).expect("race output root");

    let stop_input = Arc::new(AtomicBool::new(false));
    let input_flag = Arc::clone(&stop_input);
    let input_path_thread = input_path.clone();
    let parked_input_thread = parked_input.clone();
    let outside_input_thread = outside_input.clone();
    let input_swapper = thread::spawn(move || {
        while !input_flag.load(Ordering::Relaxed) {
            fs::rename(&input_path_thread, &parked_input_thread).expect("park safe input");
            std::os::unix::fs::symlink(&outside_input_thread, &input_path_thread)
                .expect("swap input symlink");
            thread::sleep(Duration::from_micros(250));
            fs::remove_file(&input_path_thread).expect("remove input symlink");
            fs::rename(&parked_input_thread, &input_path_thread).expect("restore safe input");
            thread::sleep(Duration::from_micros(250));
        }
    });

    let mut safe_renders = 0;
    for index in 0..40 {
        let slug = format!("race-input-{index}");
        let result = Command::new("python3")
            .arg(&renderer)
            .args([
                "--title",
                "Input race",
                "--slug",
                &slug,
                "--body-file",
                input_path
                    .join("report.md")
                    .to_str()
                    .expect("race input utf8"),
                "--root",
                output_root.to_str().expect("race output utf8"),
            ])
            .output()
            .expect("run renderer during input swap");
        if result.status.success() {
            safe_renders += 1;
            let page = fs::read_to_string(output_root.join("a").join(&slug).join("index.html"))
                .expect("read race render");
            assert!(page.contains("SAFE_INPUT_SENTINEL"));
            assert!(!page.contains("OUTSIDE_INPUT_SENTINEL"));
        }
    }
    stop_input.store(true, Ordering::Relaxed);
    input_swapper.join().expect("join input swapper");
    assert!(
        safe_renders > 0,
        "input race never exercised a successful read"
    );

    let serve_root = temp.path().join("serve-root");
    let serve_path = serve_root.join("race");
    let parked_serve = serve_root.join("race-parked");
    let outside_serve = temp.path().join("outside-serve");
    fs::create_dir_all(&serve_path).expect("safe served directory");
    fs::create_dir(&outside_serve).expect("outside served directory");
    fs::write(serve_path.join("index.html"), "SAFE_SERVER_SENTINEL").expect("safe served page");
    fs::write(outside_serve.join("index.html"), "OUTSIDE_SERVER_SENTINEL")
        .expect("outside served page");

    let port = TcpListener::bind(("127.0.0.1", 0))
        .expect("reserve race server port")
        .local_addr()
        .expect("race server address")
        .port();
    let mut server = Command::new("python3")
        .arg(&server_script)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--root",
            serve_root.to_str().expect("serve root utf8"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch race artifact server");
    wait_for_loopback(port);

    let stop_serve = Arc::new(AtomicBool::new(false));
    let serve_flag = Arc::clone(&stop_serve);
    let serve_path_thread = serve_path.clone();
    let parked_serve_thread = parked_serve.clone();
    let outside_serve_thread = outside_serve.clone();
    let serve_swapper = thread::spawn(move || {
        while !serve_flag.load(Ordering::Relaxed) {
            fs::rename(&serve_path_thread, &parked_serve_thread).expect("park served directory");
            std::os::unix::fs::symlink(&outside_serve_thread, &serve_path_thread)
                .expect("swap served symlink");
            thread::sleep(Duration::from_micros(250));
            fs::remove_file(&serve_path_thread).expect("remove served symlink");
            fs::rename(&parked_serve_thread, &serve_path_thread).expect("restore served directory");
            thread::sleep(Duration::from_micros(250));
        }
    });

    let mut safe_responses = 0;
    for _ in 0..100 {
        let (status, body) = http_exchange(port, "GET", "/race/", "");
        assert!(
            status == 200 || status == 404,
            "unexpected race status {status}"
        );
        assert!(!body.contains("OUTSIDE_SERVER_SENTINEL"));
        if status == 200 {
            safe_responses += 1;
            assert!(body.contains("SAFE_SERVER_SENTINEL"));
        }
    }
    stop_serve.store(true, Ordering::Relaxed);
    serve_swapper.join().expect("join serve swapper");
    server.kill().expect("stop race artifact server");
    server.wait().expect("reap race artifact server");
    assert!(
        safe_responses > 0,
        "server race never exercised a successful descriptor read"
    );
}

#[cfg(unix)]
#[test]
fn artifact_renderer_holds_root_and_registry_descriptors_for_the_transaction() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().expect("artifact descriptor tempdir");
    let bundle = temp.path().join("bundle");
    Roster::load_config(root.join("examples/config.yaml"))
        .expect("load public config for Artifact descriptor test")
        .resolve("amos")
        .expect("resolve public Artifact descriptor bundle")
        .write_bundle(&bundle, temp.path())
        .expect("materialize Artifact descriptor bundle");
    let renderer = bundle.join("skills/artifact/scripts/artifact_create.py");
    let helper = bundle.join("skills/artifact/scripts/artifact_fs.py");

    let render_root = temp.path().join("render-root");
    let parked_root = temp.path().join("render-root-parked");
    let outside_root = temp.path().join("outside-root");
    let returned_outside = temp.path().join("outside-root-returned");
    fs::create_dir(&render_root).expect("safe render root");
    fs::create_dir_all(outside_root.join("a/held-root")).expect("outside artifact directory");
    fs::create_dir_all(outside_root.join("a/index")).expect("outside registry directory");
    fs::write(
        outside_root.join("a/held-root/index.html"),
        "OUTSIDE_PAGE_SENTINEL",
    )
    .expect("outside page sentinel");
    fs::write(
        outside_root.join("a/index/index.json"),
        "OUTSIDE_REGISTRY_JSON_SENTINEL",
    )
    .expect("outside registry JSON sentinel");
    fs::write(
        outside_root.join("a/index/index.html"),
        "OUTSIDE_REGISTRY_HTML_SENTINEL",
    )
    .expect("outside registry HTML sentinel");

    let mut child = Command::new("python3")
        .arg(&renderer)
        .args([
            "--title",
            "Held root",
            "--slug",
            "held-root",
            "--root",
            render_root.to_str().expect("render root utf8"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn descriptor-held renderer");
    let mut input = child.stdin.take().expect("renderer stdin");
    input
        .write_all(&vec![b'x'; 4 * 1024 * 1024])
        .expect("fill renderer input after root open");

    fs::rename(&render_root, &parked_root).expect("park validated render root");
    fs::rename(&outside_root, &render_root).expect("replace render root pathname");
    drop(input);
    let output = child.wait_with_output().expect("finish held-root render");
    fs::rename(&render_root, &returned_outside).expect("recover outside root");
    fs::rename(&parked_root, &render_root).expect("restore safe render root");

    assert!(
        output.status.success(),
        "descriptor-held render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(render_root.join("a/held-root/index.html").is_file());
    let safe_registry =
        fs::read_to_string(render_root.join("a/index/index.json")).expect("safe registry JSON");
    assert!(safe_registry.contains("held-root"));
    assert_eq!(
        fs::read_to_string(returned_outside.join("a/held-root/index.html"))
            .expect("outside page unchanged"),
        "OUTSIDE_PAGE_SENTINEL"
    );
    assert_eq!(
        fs::read_to_string(returned_outside.join("a/index/index.json"))
            .expect("outside registry JSON unchanged"),
        "OUTSIDE_REGISTRY_JSON_SENTINEL"
    );
    assert_eq!(
        fs::read_to_string(returned_outside.join("a/index/index.html"))
            .expect("outside registry HTML unchanged"),
        "OUTSIDE_REGISTRY_HTML_SENTINEL"
    );

    let safe_listing = temp.path().join("safe-listing");
    let parked_listing = temp.path().join("safe-listing-parked");
    let outside_listing = temp.path().join("outside-listing");
    fs::create_dir(&safe_listing).expect("safe listing directory");
    fs::create_dir(&outside_listing).expect("outside listing directory");
    fs::write(safe_listing.join("SAFE_NAME"), "safe").expect("safe name");
    fs::write(outside_listing.join("OUTSIDE_NAME"), "outside").expect("outside name");
    let aba_probe = r#"
import os
import sys

sys.path.insert(0, os.path.dirname(sys.argv[1]))
import artifact_fs

safe, outside, parked = sys.argv[2:]
directory_fd = artifact_fs.open_directory(safe)
original_listdir = os.listdir

def exchange_during_listdir(target):
    os.rename(safe, parked)
    os.rename(outside, safe)
    try:
        return original_listdir(target)
    finally:
        os.rename(safe, outside)
        os.rename(parked, safe)

artifact_fs.os.listdir = exchange_during_listdir
try:
    names = artifact_fs.list_names_checked(directory_fd)
    if sys.platform == 'darwin':
        artifact_fs._NATIVE_LISTDIR_FD = False
        os.lseek(directory_fd, 0, os.SEEK_SET)
        fallback_names = artifact_fs.list_names_checked(directory_fd)
        assert fallback_names == ['SAFE_NAME'], fallback_names
finally:
    os.close(directory_fd)
assert names == ['SAFE_NAME'], names
"#;
    let aba = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(aba_probe)
        .arg(&helper)
        .arg(&safe_listing)
        .arg(&outside_listing)
        .arg(&parked_listing)
        .output()
        .expect("run registry ABA probe");
    assert!(
        aba.status.success(),
        "registry enumeration accepted ABA names: {}",
        String::from_utf8_lossy(&aba.stderr)
    );

    let transaction_root = temp.path().join("artifact-transactions");
    fs::create_dir(&transaction_root).expect("artifact transaction root");
    let transaction_probe = r#"
import errno
import os
import stat
import sys

sys.path.insert(0, os.path.dirname(sys.argv[1]))
import artifact_fs

root = sys.argv[2]
outside = os.path.join(root, 'outside')
with open(outside, 'w', encoding='utf-8') as output:
    output.write('OUTSIDE_SENTINEL')
directory_fd = artifact_fs.open_directory(root)
with open(os.path.join(root, 'target'), 'w', encoding='utf-8') as output:
    output.write('OLD')
expected = artifact_fs.regular_identity_at(directory_fd, 'target')
transaction_name, transaction_fd = artifact_fs._open_transaction_directory(directory_fd)
temporary = 'candidate'
candidate_fd = artifact_fs.open_at(
    transaction_fd, temporary, artifact_fs.WRITE_FLAGS, 0o600
)
os.write(candidate_fd, b'NEW')
os.fsync(candidate_fd)
os.close(candidate_fd)

original_identity = artifact_fs.regular_identity_at
armed = True
def replace_rollback_source(directory, name, missing_ok=False):
    global armed
    if armed and directory == transaction_fd and name == temporary:
        armed = False
        artifact_fs.unlink_at(transaction_fd, temporary)
        os.symlink(outside, temporary, dir_fd=transaction_fd)
    return original_identity(directory, name, missing_ok=missing_ok)

artifact_fs.regular_identity_at = replace_rollback_source
try:
    artifact_fs.guarded_replace(
        transaction_fd, temporary, directory_fd, 'target', expected
    )
except artifact_fs.AtomicWritePostCommitError as error:
    assert error.outcome == 'COMMITTED/DURABILITY_UNKNOWN'
    assert 'rollback source validation' in str(error)
else:
    raise AssertionError('hostile rollback source was accepted')
finally:
    artifact_fs.regular_identity_at = original_identity

target_stat = os.lstat(os.path.join(root, 'target'))
assert stat.S_ISREG(target_stat.st_mode), target_stat.st_mode
assert open(os.path.join(root, 'target'), encoding='utf-8').read() == 'NEW'
assert open(outside, encoding='utf-8').read() == 'OUTSIDE_SENTINEL'

# Once the first exchange crosses the commit point, a mismatched displaced
# inode must never be exchanged back from a re-resolvable transaction name.
with open(os.path.join(root, 'rollback-target'), 'w', encoding='utf-8') as output:
    output.write('ROLLBACK_OLD')
rollback_expected = artifact_fs.regular_identity_at(directory_fd, 'rollback-target')
rollback_transaction_name, rollback_transaction_fd = artifact_fs._open_transaction_directory(
    directory_fd
)
rollback_temporary = 'candidate'
rollback_candidate_fd = artifact_fs.open_at(
    rollback_transaction_fd, rollback_temporary, artifact_fs.WRITE_FLAGS, 0o600
)
os.write(rollback_candidate_fd, b'ROLLBACK_NEW')
os.fsync(rollback_candidate_fd)
os.close(rollback_candidate_fd)

original_rename = artifact_fs.rename_at_with_flags
exchange_calls = 0
def replace_after_validation_before_rollback(
    source_directory_fd, source, destination_directory_fd, destination, flags
):
    global exchange_calls
    if flags == artifact_fs._RENAME_EXCHANGE:
        exchange_calls += 1
        if exchange_calls == 2:
            artifact_fs.unlink_at(source_directory_fd, source)
            os.symlink(outside, source, dir_fd=source_directory_fd)
    return original_rename(
        source_directory_fd, source, destination_directory_fd, destination, flags
    )

artifact_fs.rename_at_with_flags = replace_after_validation_before_rollback
try:
    artifact_fs.guarded_replace(
        rollback_transaction_fd,
        rollback_temporary,
        directory_fd,
        'rollback-target',
        (rollback_expected[0], rollback_expected[1] + 1),
    )
except artifact_fs.AtomicWritePostCommitError as error:
    assert error.outcome == 'COMMITTED/DURABILITY_UNKNOWN'
else:
    raise AssertionError('mismatched displaced inode was rolled back')
finally:
    artifact_fs.rename_at_with_flags = original_rename

assert exchange_calls == 1, exchange_calls
rollback_target_stat = os.lstat(os.path.join(root, 'rollback-target'))
assert stat.S_ISREG(rollback_target_stat.st_mode), rollback_target_stat.st_mode
assert open(os.path.join(root, 'rollback-target'), encoding='utf-8').read() == 'ROLLBACK_NEW'
assert open(outside, encoding='utf-8').read() == 'OUTSIDE_SENTINEL'
assert artifact_fs.regular_identity_at(
    rollback_transaction_fd, rollback_temporary
) == rollback_expected
os.close(rollback_transaction_fd)

original_fsync = artifact_fs.os.fsync
def fail_destination_directory_fsync(file_fd):
    if file_fd == directory_fd and stat.S_ISDIR(os.fstat(file_fd).st_mode):
        raise OSError(errno.EINVAL, 'injected destination directory fsync failure')
    return original_fsync(file_fd)

artifact_fs.os.fsync = fail_destination_directory_fsync
try:
    artifact_fs.atomic_write_text(directory_fd, 'durability', 'COMMITTED')
except artifact_fs.AtomicWritePostCommitError as error:
    assert error.outcome == 'COMMITTED/DURABILITY_UNKNOWN'
    assert 'directory fsync' in str(error)
else:
    raise AssertionError('post-commit fsync failure looked like success')
finally:
    artifact_fs.os.fsync = original_fsync

assert open(os.path.join(root, 'durability'), encoding='utf-8').read() == 'COMMITTED'
assert any(name.startswith('.artifact-txn-') for name in os.listdir(root))

recovery_before = {
    name for name in os.listdir(root) if name.startswith('.artifact-recovery-')
}
parent_sync_calls = 0
def fail_final_cleanup_fsync(file_fd):
    global parent_sync_calls
    if file_fd == directory_fd and stat.S_ISDIR(os.fstat(file_fd).st_mode):
        parent_sync_calls += 1
        if parent_sync_calls == 3:
            raise OSError(errno.EIO, 'injected final cleanup fsync failure')
    return original_fsync(file_fd)

artifact_fs.os.fsync = fail_final_cleanup_fsync
try:
    artifact_fs.atomic_write_text(
        directory_fd, 'cleanup-durability', 'CLEANUP_COMMITTED'
    )
except artifact_fs.AtomicWritePostCommitError as error:
    assert error.outcome == 'COMMITTED/DURABILITY_UNKNOWN'
    assert error.phase == 'transaction cleanup fsync', error.phase
else:
    raise AssertionError('final cleanup fsync failure looked like success')
finally:
    artifact_fs.os.fsync = original_fsync

assert parent_sync_calls == 3, parent_sync_calls
assert open(
    os.path.join(root, 'cleanup-durability'), encoding='utf-8'
).read() == 'CLEANUP_COMMITTED'
recovery_after = {
    name for name in os.listdir(root) if name.startswith('.artifact-recovery-')
}
new_recovery = recovery_after - recovery_before
assert len(new_recovery) == 1, new_recovery
recovery_name = new_recovery.pop()
transaction_name = recovery_name.removeprefix('.artifact-recovery-')
assert not os.path.exists(os.path.join(root, transaction_name)), transaction_name
assert os.path.isdir(os.path.join(root, recovery_name)), recovery_name
os.close(transaction_fd)
os.close(directory_fd)
"#;
    let transaction_result = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(transaction_probe)
        .arg(&helper)
        .arg(&transaction_root)
        .output()
        .expect("run Artifact transaction probe");
    assert!(
        transaction_result.status.success(),
        "Artifact transaction boundary regressed: {}",
        String::from_utf8_lossy(&transaction_result.stderr)
    );
}

#[cfg(unix)]
#[test]
fn sprite_lane_behaviorally_bounds_credentials_shells_and_receipts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().expect("sprite tempdir");
    let bundle = temp.path().join("bundle");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");
    roster
        .resolve("amos")
        .expect("resolve public orchestrator")
        .write_bundle(&bundle, temp.path())
        .expect("materialize public Sprite primitive");
    let script = bundle.join("skills/sprites/scripts/sprite-lane");
    let staging_helper = bundle.join("skills/sprites/scripts/stage_card.py");
    let provider_snapshot_log = temp.path().join("provider-snapshot.path");
    let staging_source = fs::read_to_string(&staging_helper).expect("read staging helper");
    let provider_snapshot_log_python = provider_snapshot_log
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    let staging_source = staging_source.replacen(
        "            return f\"{root}|{token}\"\n",
        &format!(
            "            if kind == \'provider\':\n                with open(\'{provider_snapshot_log_python}\', \'w\', encoding=\'utf-8\') as proof:\n                    proof.write(root)\n            return f\"{{root}}|{{token}}\"\n"
        ),
        1,
    );
    let transient_receipt_marker = temp.path().join("fail-terminal-receipt-once");
    let transient_receipt_marker_python = transient_receipt_marker
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    let staging_source = staging_source.replacen(
        "def descriptor_install(directory, leaf, data):\n",
        &format!(
            "def descriptor_install(directory, leaf, data):\n    transient_marker = \'{transient_receipt_marker_python}\'\n    if os.path.exists(transient_marker) and b\'\"state\": \"prepared\"\' in data:\n        os.unlink(transient_marker)\n        raise OSError(errno.EIO, \'injected one-shot terminal receipt failure\')\n"
        ),
        1,
    );
    fs::write(&staging_helper, staging_source).expect("instrument provider snapshot cleanup");
    let local_state = temp.path().join("local-state");
    let parked_local_state = temp.path().join("local-state-parked");
    let outside_local_state = temp.path().join("outside-local-state");
    fs::create_dir(&local_state).expect("safe local state directory");
    fs::create_dir(&outside_local_state).expect("outside local state directory");
    fs::write(
        outside_local_state.join("sentinel"),
        "OUTSIDE_STATE_SENTINEL\n",
    )
    .expect("outside local state sentinel");
    let state_probe = r#"
import errno
import os
import stat
import sys

sys.path.insert(0, os.path.dirname(sys.argv[1]))
import stage_card

safe, parked, outside = sys.argv[2:]
original_install = stage_card.install_fd
swapped = False

def swap_parent_then_install(source_fd, directory_fd, leaf):
    global swapped
    if not swapped:
        swapped = True
        os.rename(safe, parked)
        os.rename(outside, safe)
        try:
            return original_install(source_fd, directory_fd, leaf)
        finally:
            os.rename(safe, outside)
            os.rename(parked, safe)
    return original_install(source_fd, directory_fd, leaf)

stage_card.install_fd = swap_parent_then_install
stage_card.descriptor_install(safe, 'owner', b'SAFE_OWNER\n')
assert open(os.path.join(safe, 'owner'), 'rb').read() == b'SAFE_OWNER\n'
assert os.listdir(outside) == ['sentinel'], os.listdir(outside)

outside_leaf = os.path.join(outside, 'leaf-sentinel')
with open(outside_leaf, 'wb') as output:
    output.write(b'OUTSIDE_LEAF\n')
os.symlink(outside_leaf, os.path.join(safe, 'receipt'))
stage_card.install_fd = original_install
try:
    stage_card.descriptor_install(safe, 'receipt', b'SAFE_RECEIPT\n')
except OSError:
    pass
else:
    raise AssertionError('symlink local-state destination did not fail closed')
assert os.path.islink(os.path.join(safe, 'receipt'))
os.unlink(os.path.join(safe, 'receipt'))
stage_card.descriptor_install(safe, 'receipt', b'SAFE_RECEIPT\n')
assert open(os.path.join(safe, 'receipt'), 'rb').read() == b'SAFE_RECEIPT\n'
assert open(outside_leaf, 'rb').read() == b'OUTSIDE_LEAF\n'

original_unlink = stage_card.unlink_at
def fail_recovery_cleanup(directory_fd, leaf):
    if leaf.startswith('.receipt.recovery.'):
        raise OSError(errno.EIO, 'injected recovery cleanup failure')
    return original_unlink(directory_fd, leaf)

stage_card.unlink_at = fail_recovery_cleanup
try:
    stage_card.descriptor_install(safe, 'receipt', b'COMMITTED_RECEIPT\n')
except stage_card.CommittedCleanupDebt as error:
    assert 'retained recovery path' in str(error)
else:
    raise AssertionError('committed cleanup debt looked like success')
finally:
    stage_card.unlink_at = original_unlink
assert open(os.path.join(safe, 'receipt'), 'rb').read() == b'COMMITTED_RECEIPT\n'
recovery = [name for name in os.listdir(safe) if name.startswith('.receipt.recovery.')]
assert len(recovery) == 1, recovery
os.unlink(os.path.join(safe, recovery[0]))

original_fsync = stage_card.os.fsync
def fail_directory_fsync(file_fd):
    if stat.S_ISDIR(os.fstat(file_fd).st_mode):
        raise OSError(errno.EIO, 'injected directory fsync failure')
    return original_fsync(file_fd)

stage_card.os.fsync = fail_directory_fsync
try:
    stage_card.descriptor_install(safe, 'terminal', b'TERMINAL\n')
except stage_card.DurabilityUnknown as error:
    assert 'injected directory fsync failure' in str(error)
else:
    raise AssertionError('post-install fsync failure lacked durability-unknown outcome')
finally:
    stage_card.os.fsync = original_fsync
assert open(os.path.join(safe, 'terminal'), 'rb').read() == b'TERMINAL\n'
"#;
    let state_probe_output = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(state_probe)
        .arg(&staging_helper)
        .arg(&local_state)
        .arg(&parked_local_state)
        .arg(&outside_local_state)
        .output()
        .expect("run descriptor-held local state probe");
    assert!(
        state_probe_output.status.success(),
        "local state helper escaped or obscured commit state: {}",
        String::from_utf8_lossy(&state_probe_output.stderr)
    );
    assert!(
        staging_helper.is_file(),
        "materialized staging helper missing"
    );
    assert!(script.is_file(), "materialized Sprite script missing");
    assert!(
        fs::metadata(&script)
            .expect("materialized Sprite metadata")
            .permissions()
            .mode()
            & 0o111
            != 0,
        "materialized Sprite script is not executable"
    );

    let fake_bin = temp.path().join("bin");
    let hostile_bin = temp.path().join("hostile-bin");
    let homes = temp.path().join("homes");
    let sprites = temp.path().join("sprites");
    let snapshots = temp.path().join("snapshots");
    let receipts = temp.path().join("receipts");
    let owners = temp.path().join("owners");
    let local_home = temp.path().join("local-home");
    for directory in [
        &fake_bin,
        &hostile_bin,
        &homes,
        &sprites,
        &snapshots,
        &local_home,
    ] {
        fs::create_dir_all(directory).expect("sprite fixture directory");
    }
    let log = temp.path().join("sprite.log");
    let hostile_log = temp.path().join("hostile.log");
    let hostile_bash_env = temp.path().join("hostile-bash-env");
    fs::write(
        &hostile_bash_env,
        format!(
            "printf 'BASH_ENV_SOURCED\\n' >> '{}'\n",
            hostile_log.display()
        ),
    )
    .expect("hostile BASH_ENV");

    let legacy_home = homes.join("legacy-lane");
    fs::create_dir_all(legacy_home.join(".codex")).expect("legacy Codex directory");
    fs::write(legacy_home.join(".codex/auth.json"), "legacy-secret").expect("legacy credential");
    fs::write(legacy_home.join(".sprite-lane-golden"), "v1 legacy\n").expect("legacy marker");
    fs::write(sprites.join("legacy-lane"), "").expect("legacy Sprite record");

    let fake_sprite = r##"#!/usr/bin/env bash
set -euo pipefail
base="__FIXTURE_BASE__"
log="$base/sprite.log"
homes="$base/homes"
sprites="$base/sprites"
snapshots="$base/snapshots"
fake_bin="$base/bin"
for name in GITHUB_TOKEN GH_TOKEN GIT_CONFIG_COUNT HTTPS_PROXY HTTP_PROXY ALL_PROXY SSH_AUTH_SOCK BASH_ENV; do
  [[ -z "${!name-}" ]] || { printf 'AMBIENT_LOCAL_ENV:%s\n' "$name" >> "$log"; exit 80; }
done
printf 'SPRITE' >> "$log"
for arg in "$@"; do printf '\t%s' "$arg" >> "$log"; done
printf '\n' >> "$log"

case "${1-}" in
list)
  while [[ -e "$base/pause-list" ]]; do sleep 0.01; done
  for file in "$sprites"/*; do [[ -f "$file" ]] || continue; basename "$file"; done
  ;;
create)
  name="${@: -1}"
  rm -rf "$homes/$name" "$snapshots/$name"
  mkdir -p "$homes/$name/.codex" "$homes/$name/.omp/agent/sessions" \
    "$homes/$name/.config/git" "$snapshots/$name"
  printf 'seeded-credential\n' > "$homes/$name/.codex/auth.json"
  printf 'seeded-session\n' > "$homes/$name/.omp/agent/sessions/session.json"
  printf '[credential]\n  helper = store\n' > "$homes/$name/.config/git/config"
  if [[ "$name" == config-link ]]; then
    mkdir -p "$base/outside-config"
    printf 'OUTSIDE_CONFIG_SENTINEL\n' > "$base/outside-config/sentinel"
    rm -rf "$homes/$name/.config"
    ln -s "$base/outside-config" "$homes/$name/.config"
  fi
  if [[ "$name" == lanes-link ]]; then
    mkdir -p "$base/outside-lanes"
    printf 'OUTSIDE_LANES_SENTINEL\n' > "$base/outside-lanes/sentinel"
    ln -s "$base/outside-lanes" "$homes/$name/lanes"
  fi
  : > "$sprites/$name"
  ;;
checkpoint)
  action="${2-}"
  shift 2
  sprite=""
  comment=""
  checkpoint_id=""
  while (($#)); do
    case "$1" in
      -s) sprite="$2"; shift 2 ;;
      --comment) comment="$2"; shift 2 ;;
      *) checkpoint_id="$1"; shift ;;
    esac
  done
  home="$homes/$sprite"
  store="$snapshots/$sprite"
  case "$action" in
  create)
    [[ ! -e "$base/checkpoint-fail" ]] || exit 94
    marker="$(cat "$home/.sprite-lane-golden")"
    [[ "$comment" == "sprite-lane golden ${marker}" ]] || exit 95
    count="$(find "$store" -maxdepth 1 -type d -name 'checkpoint-*' | wc -l | tr -d ' ')"
    id="checkpoint-$((count + 1))-$$"
    mkdir -p "$store/$id/home"
    cp -R "$home/." "$store/$id/home/"
    printf '%s\n' "$comment" > "$store/$id.comment"
    printf 'CHECKPOINT_CREATE\t%s\t%s\n' "$id" "$comment" >> "$log"
    while [[ -e "$base/pause-checkpoint-create" ]]; do sleep 0.01; done
    printf '%s\n' "$id"
    ;;
  list)
    if [[ -e "$base/postcommit-fail" && -f "$base/owners/$sprite.owner" && -f "$home/.sprite-lane-golden" ]]; then
      local_marker="$(cat "$base/owners/$sprite.owner")"
      remote_marker="$(cat "$home/.sprite-lane-golden")"
      checkpoint_count="$(find "$store" -maxdepth 1 -type f -name 'checkpoint-*.comment' | wc -l | tr -d ' ')"
      if [[ "$local_marker" == "$remote_marker" && "$checkpoint_count" -gt 1 ]]; then
        exit 93
      fi
    fi
    for comment_file in "$store"/checkpoint-*.comment; do
      [[ -f "$comment_file" ]] || continue
      id="$(basename "$comment_file" .comment)"
      printf '%s %s\n' "$id" "$(cat "$comment_file")"
    done
    ;;
  delete)
    id="$checkpoint_id"
    printf 'CHECKPOINT_DELETE\t%s\n' "$id" >> "$log"
    while [[ -e "$base/pause-checkpoint-delete" ]]; do sleep 0.01; done
    rm -rf "$store/$id" "$store/$id.comment"
    ;;
  *) exit 96 ;;
  esac
  ;;
restore)
  shift
  sprite=""
  while (($#)); do
    case "$1" in -s) sprite="$2"; shift 2 ;; *) id="$1"; shift ;; esac
  done
  [[ ! -e "$base/restore-fail" ]] || exit 41
  if [[ -e "$base/pause-rollback-restore" && -f "$base/owners/$sprite.owner" \
      && -f "$homes/$sprite/.sprite-lane-golden" ]]; then
    local_marker="$(cat "$base/owners/$sprite.owner")"
    remote_marker="$(cat "$homes/$sprite/.sprite-lane-golden")"
    if [[ "$local_marker" != "$remote_marker" ]]; then
      printf 'ROLLBACK_RESTORE_WAIT\t%s\n' "$id" >> "$log"
      while [[ -e "$base/pause-rollback-restore" ]]; do sleep 0.01; done
    fi
  fi
  if [[ -e "$base/rollback-restore-fail" && -f "$base/owners/$sprite.owner" \
      && -f "$homes/$sprite/.sprite-lane-golden" ]]; then
    local_marker="$(cat "$base/owners/$sprite.owner")"
    remote_marker="$(cat "$homes/$sprite/.sprite-lane-golden")"
    [[ "$local_marker" == "$remote_marker" ]] || exit 42
  fi
  store="$snapshots/$sprite/$id/home"
  [[ -d "$store" ]] || exit 97
  rm -rf "$homes/$sprite"
  mkdir -p "$homes/$sprite"
  cp -R "$store/." "$homes/$sprite/"
  ;;
exec)
  shift
  sprite=""
  workdir=""
  file_pair=""
  while [[ "${1-}" != -- ]]; do
    case "${1-}" in
      -s) sprite="$2"; shift 2 ;;
      --dir) workdir="$2"; shift 2 ;;
      --file) file_pair="$2"; shift 2 ;;
      --env) printf 'SECRET_TRANSPORT_USED\n' >> "$log"; exit 98 ;;
      *) exit 99 ;;
    esac
  done
  shift
  home="$homes/$sprite"
  if [[ -e "$base/swap-lanes-before-handoff" \
      && ( -n "$file_pair" || " $* " == *"card.md"* ) ]]; then
    mv "$home/lanes" "$base/parked-remote-lanes"
    ln -s "$base/outside-remote-lanes" "$home/lanes"
  fi
  if [[ -n "$file_pair" ]]; then
    source="${file_pair%%:*}"
    destination="${file_pair#*:}"
    destination="${destination//\/home\/sprite/$home}"
    mkdir -p "$(dirname "$destination")"
    cp "$source" "$destination"
  fi
  command=("$@")
  for i in "${!command[@]}"; do
    command[$i]="${command[$i]//\/home\/sprite/$home}"
    command[$i]="${command[$i]//\/usr\/local\/bin:\/usr\/bin:\/bin/$fake_bin:/usr/bin:/bin}"
  done
  if [[ -n "$workdir" ]]; then
    workdir="${workdir//\/home\/sprite/$home}"
    mkdir -p "$workdir"
    cd "$workdir"
  fi
  HOME="$home" PATH="$fake_bin:/usr/bin:/bin" BASH_ENV="$base/hostile-bash-env" \
    GITHUB_TOKEN=ambient-github GH_TOKEN=ambient-gh GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0=url.https://attacker.example/.insteadOf \
    GIT_CONFIG_VALUE_0=https://github.com/ HTTPS_PROXY=http://proxy.invalid \
    SSH_AUTH_SOCK="$base/ssh-agent.sock" "${command[@]}"
  ;;
*) exit 100 ;;
esac
"##
    .replace(
        "__FIXTURE_BASE__",
        temp.path().to_str().expect("fixture base utf8"),
    );
    let fake_sprite_path = fake_bin.join("sprite");
    write_executable(&fake_sprite_path, &fake_sprite);

    let pinned_provider = Command::new("/usr/bin/python3")
        .arg(&staging_helper)
        .args([
            "broker-start",
            "provider",
            fake_sprite_path.to_str().expect("provider utf8"),
        ])
        .output()
        .expect("snapshot provider executable");
    assert!(
        pinned_provider.status.success(),
        "provider snapshot failed: {}",
        String::from_utf8_lossy(&pinned_provider.stderr)
    );
    let pinned_provider_handle = String::from_utf8(pinned_provider.stdout)
        .expect("snapshot handle utf8")
        .trim()
        .to_owned();
    let pinned_provider_root = PathBuf::from(
        pinned_provider_handle
            .split_once('|')
            .expect("snapshot recovery handle")
            .0,
    );
    let original_provider_path = fake_bin.join("sprite-original");
    fs::rename(&fake_sprite_path, &original_provider_path).expect("park original provider");
    write_executable(
        &fake_sprite_path,
        &format!(
            "#!/bin/sh\nprintf 'REPLACEMENT_PROVIDER_RAN\\n' >> '{}'\nexit 78\n",
            hostile_log.display()
        ),
    );
    let published_snapshot = pinned_provider_root.join("snapshot");
    let held_snapshot = pinned_provider_root.join("snapshot-held");
    fs::rename(&published_snapshot, &held_snapshot).expect("park published snapshot");
    write_executable(
        &published_snapshot,
        &format!(
            "#!/bin/sh\nprintf 'REPLACEMENT_SNAPSHOT_RAN\\n' >> '{}'\nexit 77\n",
            hostile_log.display()
        ),
    );
    let pinned_execution = Command::new("/usr/bin/python3")
        .arg(&staging_helper)
        .args(["broker-exec", &pinned_provider_handle, "-", "--", "list"])
        .env_clear()
        .env("HOME", &local_home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("execute pinned provider snapshot");
    assert!(
        pinned_execution.status.success(),
        "pinned provider did not preserve selected executable"
    );
    assert!(
        !hostile_log.exists(),
        "replacement provider gained authority"
    );
    fs::remove_file(&fake_sprite_path).expect("remove replacement provider");
    fs::rename(&original_provider_path, &fake_sprite_path).expect("restore original provider");
    fs::remove_file(&published_snapshot).expect("remove replacement snapshot");
    fs::rename(&held_snapshot, &published_snapshot).expect("restore held snapshot path");
    let discard_provider = Command::new("/usr/bin/python3")
        .arg(&staging_helper)
        .args(["broker-discard", &pinned_provider_handle])
        .output()
        .expect("discard provider broker");
    assert!(
        discard_provider.status.success(),
        "provider broker cleanup failed: {}",
        String::from_utf8_lossy(&discard_provider.stderr)
    );
    assert!(
        !pinned_provider_root.exists(),
        "provider broker recovery root leaked"
    );

    let broker_card = temp.path().join("broker-card.md");
    fs::write(&broker_card, "BROKER_CARD_SENTINEL\n").expect("broker card");
    let broker_authentication_probe = r#"
import json
import os
import socket
import sys
import threading

sys.path.insert(0, os.path.dirname(sys.argv[1]))
import stage_card

handle = stage_card.start_broker(sys.argv[2], 'card')
root, token = handle.rsplit('|', 1)
parked = root + '.parked'
listener = None
observed = {}
restored = False
try:
    os.rename(root, parked)
    os.mkdir(root, 0o700)
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(os.path.join(root, 'broker.sock'))
    listener.listen(1)

    def fake_broker():
        connection, _ = listener.accept()
        try:
            observed.update(stage_card._receive_frame(connection))
            stage_card._send_frame(connection, {
                'nonce': observed.get('nonce'),
                'response': {'ok': True, 'data': ''},
                'mac': '0' * 64,
            })
        finally:
            connection.close()

    thread = threading.Thread(target=fake_broker)
    thread.start()
    try:
        stage_card.broker_read(handle)
    except PermissionError:
        pass
    else:
        raise AssertionError('forged broker response was accepted')
    thread.join(5)
    assert not thread.is_alive(), 'fake broker did not receive a request'
    assert set(observed) == {'nonce', 'request', 'mac'}, observed
    assert token not in json.dumps(observed, sort_keys=True)
    assert 'token' not in observed['request']
finally:
    if listener is not None:
        listener.close()
    fake_socket = os.path.join(root, 'broker.sock')
    if os.path.exists(fake_socket):
        os.unlink(fake_socket)
    if os.path.isdir(root):
        os.rmdir(root)
    if os.path.isdir(parked):
        os.rename(parked, root)
        restored = True
    if restored:
        stage_card.broker_discard(handle)
"#;
    let broker_authentication = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(broker_authentication_probe)
        .arg(&staging_helper)
        .arg(&broker_card)
        .output()
        .expect("run broker socket authentication probe");
    assert!(
        broker_authentication.status.success(),
        "broker socket substitution was not rejected: {}",
        String::from_utf8_lossy(&broker_authentication.stderr)
    );

    write_executable(
        &hostile_bin.join("bash"),
        &format!(
            "#!/bin/sh\nprintf 'HOSTILE_PATH_BASH\\n' >> '{}'\nexit 79\n",
            hostile_log.display()
        ),
    );
    let hostile_startup_log = temp.path().join("hostile-startup.log");
    write_executable(
        &hostile_bin.join("dirname"),
        &format!(
            "#!/bin/sh\nprintf 'HOSTILE_PATH_RAN\\n' >> '{}'\nexec /usr/bin/dirname \"$@\"\n",
            hostile_startup_log.display()
        ),
    );

    let fake_git = r##"#!/usr/bin/env bash
set -euo pipefail
base="$(cd "$(dirname "$0")/.." && pwd -P)"
log="$base/sprite.log"
printf 'GIT' >> "$log"
for arg in "$@"; do printf '\t%s' "$arg" >> "$log"; done
printf '\n' >> "$log"
if [[ "${1-}" == check-ref-format ]]; then exec /usr/bin/git "$@"; fi
[[ " $* " == *" clone "* ]] || exit 0
[[ "${GIT_CONFIG_NOSYSTEM-}" == 1 && "${GIT_CONFIG_GLOBAL-}" == /dev/null ]] || exit 81
[[ "${GIT_TERMINAL_PROMPT-}" == 0 && "${GIT_ASKPASS-}" == /bin/false ]] || exit 82
for name in GITHUB_TOKEN GH_TOKEN GIT_CONFIG_COUNT HTTPS_PROXY HTTP_PROXY ALL_PROXY SSH_AUTH_SOCK; do
  [[ -z "${!name-}" ]] || exit 83
done
[[ ! -e "$HOME/.config/git" ]] || exit 84
case "$*" in
  *owner/clone-fail.git*) exit 31 ;;
  *owner/receipt-fail-primary.git*)
    mv "$base/receipts" "$base/receipts-disabled"
    : > "$base/receipts"
    exit 37
    ;;
  *owner/receipt-fail-success.git*)
    mv "$base/receipts" "$base/receipts-disabled"
    : > "$base/receipts"
    ;;
  *owner/receipt-fail-once.git*)
    : > "$base/fail-terminal-receipt-once"
    ;;
esac
destination="${@: -1}"
mkdir -p "$destination/.git"
"##;
    write_executable(&fake_bin.join("git"), fake_git);

    let path = format!(
        "{}:{}:{}",
        hostile_bin.display(),
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let make_command = |args: &[String]| {
        let mut command = Command::new(&script);
        command
            .arg("--provider-cli")
            .arg(&fake_sprite_path)
            .args(args)
            .env("PATH", &path)
            .env("HOME", &local_home)
            .env("SPRITE_LANE_RECEIPTS", &receipts)
            .env("SPRITE_LANE_OWNERS", &owners)
            .env("GITHUB_TOKEN", "ambient-github")
            .env("GH_TOKEN", "ambient-gh")
            .env("GIT_CONFIG_COUNT", "1")
            .env("HTTPS_PROXY", "http://proxy.invalid")
            .env("SSH_AUTH_SOCK", temp.path().join("ssh-agent.sock"))
            .env("SHELLOPTS", "xtrace")
            .env(
                "BASH_FUNC_env%%",
                format!(
                    "() {{ printf 'FUNCTION_INTERCEPTED\\n' >> '{}'; }}",
                    hostile_log.display()
                ),
            );
        command
    };
    let run = |args: &[String]| make_command(args).output().expect("run sprite-lane");
    let prepare_args = |repo: &str, card: &std::path::Path| {
        vec![
            "prepare".into(),
            "lane-1".into(),
            "--repo".into(),
            repo.into(),
            "--card".into(),
            card.to_str().expect("card utf8").into(),
        ]
    };

    let direct_start = Command::new(&script)
        .args(["--provider-cli"])
        .arg(&fake_sprite_path)
        .arg("unknown-command")
        .env("BASH_ENV", &hostile_bash_env)
        .env("PATH", &path)
        .env("HOME", &local_home)
        .output()
        .expect("exercise hardened shebang");
    assert!(!direct_start.status.success());
    assert!(!hostile_log.exists(), "hostile shell startup was evaluated");

    let forged_clean_start = Command::new(&script)
        .args(["--provider-cli"])
        .arg(&fake_sprite_path)
        .arg("unknown-command")
        .env("SPRITE_LANE_CLEAN_START", "v1")
        .env("PATH", &path)
        .env("HOME", &local_home)
        .output()
        .expect("exercise forged clean-start marker");
    assert!(!forged_clean_start.status.success());
    assert!(
        !hostile_startup_log.exists(),
        "caller-forged clean-start marker retained hostile PATH"
    );

    let outside_card = temp.path().join("outside-invalid-card.md");
    let invalid_card = temp.path().join("invalid-card-link.md");
    fs::write(&outside_card, "INVALID CARD SENTINEL\n").expect("outside invalid card");
    std::os::unix::fs::symlink(&outside_card, &invalid_card).expect("invalid card symlink");
    let invalid_prepare = run(&prepare_args("owner/invalid-card", &invalid_card));
    assert!(!invalid_prepare.status.success());
    let leaked_provider = PathBuf::from(
        fs::read_to_string(&provider_snapshot_log)
            .expect("provider snapshot path")
            .trim(),
    );
    assert!(
        !leaked_provider.exists(),
        "provider snapshot leaked after card staging failure: {}",
        leaked_provider.display()
    );

    let legacy_bake = run(&["bake".into(), "legacy-lane".into()]);
    assert!(!legacy_bake.status.success());
    assert_eq!(
        fs::read_to_string(legacy_home.join(".codex/auth.json")).expect("legacy preserved"),
        "legacy-secret"
    );

    let config_link_bake = run(&["bake".into(), "config-link".into()]);
    assert!(!config_link_bake.status.success());
    assert_eq!(
        fs::read_to_string(temp.path().join("outside-config/sentinel"))
            .expect("outside config preserved"),
        "OUTSIDE_CONFIG_SENTINEL\n"
    );
    let lanes_link_bake = run(&["bake".into(), "lanes-link".into()]);
    assert!(!lanes_link_bake.status.success());
    assert_eq!(
        fs::read_to_string(temp.path().join("outside-lanes/sentinel"))
            .expect("outside lanes preserved"),
        "OUTSIDE_LANES_SENTINEL\n"
    );

    fs::write(sprites.join("laneX1"), "").expect("regex-confusable Sprite record");
    let literal_name_bake = run(&["bake".into(), "lane.1".into()]);
    assert!(
        literal_name_bake.status.success(),
        "literal Sprite inventory matching failed: {literal_name_bake:?}"
    );
    assert!(
        sprites.join("lane.1").is_file(),
        "regex-confusable inventory entry suppressed Sprite creation"
    );

    let first_bake = run(&["bake".into(), "lane-1".into()]);
    assert!(
        first_bake.status.success(),
        "first bake failed: {first_bake:?}"
    );
    let lane_home = homes.join("lane-1");
    for forbidden in [
        ".codex",
        ".claude",
        ".omp",
        ".config/gh",
        ".config/git",
        ".ssh",
    ] {
        assert!(
            !lane_home.join(forbidden).exists(),
            "unsafe bake path survived: {forbidden}"
        );
    }
    assert_eq!(
        fs::read_to_string(lane_home.join(".gitconfig")).expect("neutral Git config"),
        "[user]\n  name = sprite-lane\n  email = sprite-lane@users.noreply.github.com\n"
    );
    let original_marker =
        fs::read_to_string(lane_home.join(".sprite-lane-golden")).expect("initial remote marker");
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner")).expect("initial local witness"),
        original_marker
    );

    let outside_remote_lanes = temp.path().join("outside-remote-lanes");
    fs::create_dir(&outside_remote_lanes).expect("outside remote handoff directory");
    fs::write(
        outside_remote_lanes.join("sentinel"),
        "OUTSIDE_HANDOFF_SENTINEL\n",
    )
    .expect("outside remote handoff sentinel");
    let remote_race_card = temp.path().join("remote-race-card.md");
    fs::write(&remote_race_card, "REMOTE_RACE_CARD_SENTINEL\n").expect("remote race card");
    fs::write(temp.path().join("swap-lanes-before-handoff"), "")
        .expect("arm remote handoff parent swap");
    let remote_parent_swap = run(&prepare_args("owner/remote-parent-race", &remote_race_card));
    assert!(
        !remote_parent_swap.status.success(),
        "remote handoff followed a swapped lanes parent"
    );
    assert_eq!(
        fs::read_dir(&outside_remote_lanes)
            .expect("outside remote handoff contents")
            .count(),
        1,
        "remote handoff wrote outside the Sprite home"
    );
    fs::remove_file(lane_home.join("lanes")).expect("remove swapped remote lanes symlink");
    fs::rename(
        temp.path().join("parked-remote-lanes"),
        lane_home.join("lanes"),
    )
    .expect("restore anchored remote lanes directory");
    fs::remove_file(temp.path().join("swap-lanes-before-handoff"))
        .expect("disarm remote handoff parent swap");

    let race_parent = temp.path().join("card-parent");
    let parked_parent = temp.path().join("card-parent-parked");
    let outside_parent = temp.path().join("outside-card-parent");
    fs::create_dir(&race_parent).expect("safe card parent");
    fs::create_dir(&outside_parent).expect("outside card parent");
    fs::write(race_parent.join("card.md"), "SAFE_CARD_SENTINEL\n").expect("safe race card");
    fs::write(outside_parent.join("card.md"), "OUTSIDE_CARD_SENTINEL\n")
        .expect("outside race card");
    let stop_card_race = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let race_flag = std::sync::Arc::clone(&stop_card_race);
    let race_parent_thread = race_parent.clone();
    let parked_parent_thread = parked_parent.clone();
    let outside_parent_thread = outside_parent.clone();
    let card_swapper = thread::spawn(move || {
        while !race_flag.load(std::sync::atomic::Ordering::Relaxed) {
            fs::rename(&race_parent_thread, &parked_parent_thread).expect("park safe card parent");
            std::os::unix::fs::symlink(&outside_parent_thread, &race_parent_thread)
                .expect("swap card parent symlink");
            thread::sleep(Duration::from_micros(250));
            fs::remove_file(&race_parent_thread).expect("remove swapped card parent");
            fs::rename(&parked_parent_thread, &race_parent_thread)
                .expect("restore safe card parent");
            thread::sleep(Duration::from_micros(250));
        }
    });
    let mut safe_card_prepares = 0;
    for _ in 0..20 {
        let race_args = prepare_args("owner/race-repo", &race_parent.join("card.md"));
        let result = run(&race_args);
        if result.status.success() {
            safe_card_prepares += 1;
            let remote_card = fs::read_dir(lane_home.join("lanes"))
                .expect("race lane root")
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("card.md"))
                .find(|path| path.is_file())
                .expect("successful prepare uploaded a card");
            let staged = fs::read_to_string(remote_card).expect("read staged race card");
            assert!(staged.contains("SAFE_CARD_SENTINEL"));
            assert!(!staged.contains("OUTSIDE_CARD_SENTINEL"));
        }
    }
    stop_card_race.store(true, std::sync::atomic::Ordering::Relaxed);
    card_swapper.join().expect("join card swapper");
    assert!(
        safe_card_prepares > 0,
        "card race never reached a safe window"
    );

    let card = temp.path().join("lane-card.md");
    fs::write(&card, "Prepare this public lane.\n").expect("lane card");

    fs::write(temp.path().join("pause-list"), "").expect("pause provider inventory");
    let interrupted_args = prepare_args("owner/interrupted", &card);
    let mut interrupted_command = make_command(&interrupted_args);
    interrupted_command
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut interrupted_child = interrupted_command
        .spawn()
        .expect("spawn interruptible preparation");
    let mut preparing_receipt = None;
    for _ in 0..200 {
        if let Ok(entries) = fs::read_dir(&receipts) {
            preparing_receipt = entries.filter_map(Result::ok).find_map(|entry| {
                if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                    return None;
                }
                let text = fs::read_to_string(entry.path()).ok()?;
                (text.contains("github.com/owner/interrupted")
                    && text.contains("\"state\": \"preparing\""))
                .then_some(entry.path())
            });
        }
        if preparing_receipt.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    let preparing_receipt = preparing_receipt.expect("preparing receipt became durable");
    let signal_status = Command::new("/bin/kill")
        .args(["-TERM", &interrupted_child.id().to_string()])
        .status()
        .expect("signal preparation");
    assert!(signal_status.success());
    fs::remove_file(temp.path().join("pause-list")).expect("release provider inventory");
    let interrupted_status = interrupted_child
        .wait()
        .expect("reap interrupted preparation");
    assert_eq!(interrupted_status.code(), Some(143));
    let interrupted_receipt =
        fs::read_to_string(preparing_receipt).expect("read terminal interrupted receipt");
    assert!(interrupted_receipt.contains("\"state\": \"interrupted\""));
    assert!(interrupted_receipt.contains("\"exit_code\": 143"));

    let creates_before_cleanup_signal = fs::read_to_string(&log)
        .expect("checkpoint log before prepare cleanup signal")
        .lines()
        .filter(|line| line.starts_with("CHECKPOINT_CREATE\t"))
        .count();
    let deletes_before_cleanup_signal = fs::read_to_string(&log)
        .expect("checkpoint log before prepare cleanup delete")
        .lines()
        .filter(|line| line.starts_with("CHECKPOINT_DELETE\t"))
        .count();
    fs::write(temp.path().join("pause-checkpoint-create"), "")
        .expect("pause prepare checkpoint create");
    fs::write(temp.path().join("pause-checkpoint-delete"), "")
        .expect("pause prepare rollback delete");
    let cleanup_signal_args = vec![
        "prepare".into(),
        "cleanup-signal".into(),
        "--repo".into(),
        "owner/cleanup-signal".into(),
        "--card".into(),
        card.to_str().expect("cleanup signal card utf8").into(),
    ];
    let mut cleanup_signal_command = make_command(&cleanup_signal_args);
    cleanup_signal_command
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut cleanup_signal_child = cleanup_signal_command
        .spawn()
        .expect("spawn prepare cleanup signal probe");
    let mut cleanup_candidate_exists = false;
    for _ in 0..1000 {
        let creates = fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with("CHECKPOINT_CREATE\t"))
            .count();
        if creates > creates_before_cleanup_signal {
            cleanup_candidate_exists = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    if !cleanup_candidate_exists {
        fs::remove_file(temp.path().join("pause-checkpoint-create")).ok();
        fs::remove_file(temp.path().join("pause-checkpoint-delete")).ok();
        let output = cleanup_signal_child
            .wait_with_output()
            .expect("reap failed cleanup signal probe");
        panic!(
            "prepare cleanup candidate was never created: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let first_cleanup_signal = Command::new("/bin/kill")
        .args(["-TERM", &cleanup_signal_child.id().to_string()])
        .status()
        .expect("signal prepare before cleanup");
    assert!(first_cleanup_signal.success());
    fs::remove_file(temp.path().join("pause-checkpoint-create"))
        .expect("release prepare checkpoint create");
    let mut cleanup_delete_started = false;
    for _ in 0..400 {
        let deletes = fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with("CHECKPOINT_DELETE\t"))
            .count();
        if deletes > deletes_before_cleanup_signal {
            cleanup_delete_started = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        cleanup_delete_started,
        "prepare rollback delete never started"
    );
    let second_cleanup_signal = Command::new("/bin/kill")
        .args(["-TERM", &cleanup_signal_child.id().to_string()])
        .status()
        .expect("repeat signal during prepare cleanup");
    assert!(second_cleanup_signal.success());
    fs::remove_file(temp.path().join("pause-checkpoint-delete"))
        .expect("release prepare rollback delete");
    assert_eq!(
        cleanup_signal_child
            .wait()
            .expect("reap repeated-signal prepare")
            .code(),
        Some(143)
    );
    let cleanup_signal_receipt = fs::read_dir(&receipts)
        .expect("receipt directory after repeated signal")
        .filter_map(Result::ok)
        .find_map(|entry| {
            let text = fs::read_to_string(entry.path()).ok()?;
            (text.contains("github.com/owner/cleanup-signal")).then_some(text)
        })
        .expect("repeated-signal prepare receipt");
    assert!(cleanup_signal_receipt.contains("\"state\": \"interrupted\""));
    assert!(cleanup_signal_receipt.contains("\"exit_code\": 143"));

    let valid = prepare_args("https://github.com/owner/repo.git", &card);
    let first_prepare = run(&valid);
    assert!(
        first_prepare.status.success(),
        "valid preparation failed: {first_prepare:?}"
    );
    fs::create_dir_all(lane_home.join(".config/git")).expect("poison XDG Git directory");
    fs::write(
        lane_home.join(".config/git/config"),
        "[url \"https://attacker.example/\"]\n  insteadOf = https://github.com/\n",
    )
    .expect("poison XDG Git config");
    fs::write(lane_home.join("live-poison"), "prior lane state").expect("live poison");
    let second_prepare = run(&valid);
    assert!(
        second_prepare.status.success(),
        "repeat preparation failed: {second_prepare:?}"
    );
    assert!(!lane_home.join(".config/git").exists());
    assert!(!lane_home.join("live-poison").exists());

    fs::write(temp.path().join("restore-fail"), "").expect("restore failure trigger");
    let restore_failure = run(&prepare_args("owner/restore-fail", &card));
    assert_eq!(restore_failure.status.code(), Some(41));
    fs::remove_file(temp.path().join("restore-fail")).expect("remove restore trigger");

    let clone_failure = run(&prepare_args("owner/clone-fail", &card));
    assert_eq!(clone_failure.status.code(), Some(31));

    let transient_receipt_failure = run(&prepare_args("owner/receipt-fail-once", &card));
    assert!(
        transient_receipt_failure.status.success(),
        "terminal receipt retry lost committed preparation: {transient_receipt_failure:?}"
    );
    assert!(!transient_receipt_marker.exists());
    assert!(
        String::from_utf8_lossy(&transient_receipt_failure.stderr)
            .contains("final receipt write failed")
    );
    let retried_receipt = fs::read_dir(&receipts)
        .expect("receipts after transient failure")
        .filter_map(Result::ok)
        .find_map(|entry| {
            let text = fs::read_to_string(entry.path()).ok()?;
            (text.contains("github.com/owner/receipt-fail-once")).then_some(text)
        })
        .expect("retried prepared receipt");
    assert!(retried_receipt.contains("\"state\": \"prepared\""));
    assert!(retried_receipt.contains("\"exit_code\": 0"));

    let receipt_failure_success = run(&prepare_args("owner/receipt-fail-success", &card));
    assert_eq!(receipt_failure_success.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&receipt_failure_success.stderr)
            .contains("terminal receipt retry failed")
    );
    fs::remove_file(&receipts).expect("remove sabotaged receipt path");
    fs::rename(temp.path().join("receipts-disabled"), &receipts)
        .expect("restore receipt directory");
    let receipt_failure_primary = run(&prepare_args("owner/receipt-fail-primary", &card));
    assert_eq!(
        receipt_failure_primary.status.code(),
        Some(37),
        "primary failure was masked: {receipt_failure_primary:?}"
    );
    assert!(
        String::from_utf8_lossy(&receipt_failure_primary.stderr)
            .contains("terminal receipt retry failed")
    );
    let recovery_receipt = fs::read_dir(temp.path().join("receipts-disabled"))
        .expect("receipt recovery directory")
        .filter_map(Result::ok)
        .find_map(|entry| {
            let text = fs::read_to_string(entry.path()).ok()?;
            (text.contains("github.com/owner/receipt-fail-primary")).then_some(text)
        })
        .expect("durable preparing receipt recovery evidence");
    assert!(recovery_receipt.contains("\"state\": \"preparing\""));
    fs::remove_file(&receipts).expect("remove sabotaged receipt path again");
    fs::rename(temp.path().join("receipts-disabled"), &receipts)
        .expect("restore receipt directory again");

    let receipts_json = fs::read_dir(&receipts)
        .expect("receipt directory")
        .filter_map(Result::ok)
        .map(|entry| {
            assert_eq!(
                entry.path().extension().and_then(|value| value.to_str()),
                Some("json")
            );
            let text = fs::read_to_string(entry.path()).expect("read receipt");
            assert!(!text.to_ascii_lowercase().contains("secret"));
            serde_json::from_str::<serde_json::Value>(&text).expect("valid atomic receipt")
        })
        .collect::<Vec<_>>();
    let prepared = receipts_json
        .iter()
        .filter(|receipt| receipt["source"] == "github.com/owner/repo")
        .collect::<Vec<_>>();
    assert_eq!(prepared.len(), 2);
    assert!(prepared.iter().all(|receipt| {
        receipt["state"] == "prepared"
            && receipt["harness"].is_null()
            && receipt["launch_owner"] == "external"
            && receipt["remote_log"].is_null()
    }));
    let restore_receipt = receipts_json
        .iter()
        .find(|receipt| receipt["source"] == "github.com/owner/restore-fail")
        .expect("restore failure receipt");
    assert_eq!(restore_receipt["state"], "setup_failed");
    assert_eq!(restore_receipt["exit_code"], 41);
    let clone_receipt = receipts_json
        .iter()
        .find(|receipt| receipt["source"] == "github.com/owner/clone-fail")
        .expect("clone failure receipt");
    assert_eq!(clone_receipt["state"], "setup_failed");
    assert_eq!(clone_receipt["exit_code"], 31);

    fs::write(temp.path().join("checkpoint-fail"), "").expect("checkpoint failure trigger");
    let failed_replacement = run(&["bake".into(), "lane-1".into()]);
    assert_eq!(failed_replacement.status.code(), Some(94));
    fs::remove_file(temp.path().join("checkpoint-fail")).expect("remove checkpoint trigger");
    assert_eq!(
        fs::read_to_string(lane_home.join(".sprite-lane-golden"))
            .expect("remote marker after failed replacement"),
        original_marker
    );
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner"))
            .expect("local witness after failed replacement"),
        original_marker
    );
    let status_after_failure = run(&["status".into(), "lane-1".into()]);
    assert!(status_after_failure.status.success());
    assert!(String::from_utf8_lossy(&status_after_failure.stdout).contains("owned clean v3"));

    fs::write(temp.path().join("postcommit-fail"), "").expect("commit verification trigger");
    let failed_commit = run(&["bake".into(), "lane-1".into()]);
    assert_eq!(failed_commit.status.code(), Some(1));
    fs::remove_file(temp.path().join("postcommit-fail")).expect("remove commit trigger");
    assert_eq!(
        fs::read_to_string(lane_home.join(".sprite-lane-golden"))
            .expect("remote marker after failed commit"),
        original_marker
    );
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner")).expect("local witness after failed commit"),
        original_marker
    );
    let checkpoints_after_failed_commit = fs::read_dir(snapshots.join("lane-1"))
        .expect("checkpoint inventory after failed commit")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().and_then(|value| value.to_str()) == Some("comment")
        })
        .count();
    assert_eq!(
        checkpoints_after_failed_commit, 1,
        "failed replacement candidate was not retired after rollback"
    );
    let status_after_failed_commit = run(&["status".into(), "lane-1".into()]);
    assert!(status_after_failed_commit.status.success());
    assert!(String::from_utf8_lossy(&status_after_failed_commit.stdout).contains("owned clean v3"));

    let replacement = run(&["bake".into(), "lane-1".into()]);
    assert!(
        replacement.status.success(),
        "replacement bake failed: {replacement:?}"
    );
    let replacement_marker =
        fs::read_to_string(lane_home.join(".sprite-lane-golden")).expect("replacement marker");
    assert_ne!(replacement_marker, original_marker);
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner")).expect("replacement witness"),
        replacement_marker
    );
    let checkpoint_comments = fs::read_dir(snapshots.join("lane-1"))
        .expect("checkpoint inventory")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().and_then(|value| value.to_str()) == Some("comment")
        })
        .count();
    assert_eq!(
        checkpoint_comments, 1,
        "prior checkpoint was not retired after commit"
    );

    let deletes_before_late_signal = fs::read_to_string(&log)
        .expect("checkpoint log before late signal")
        .lines()
        .filter(|line| line.starts_with("CHECKPOINT_DELETE\t"))
        .count();
    fs::write(temp.path().join("pause-checkpoint-delete"), "")
        .expect("pause prior checkpoint cleanup");
    let mut late_signal_command = make_command(&["bake".into(), "lane-1".into()]);
    late_signal_command
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let late_signal_bake = late_signal_command
        .spawn()
        .expect("spawn post-commit signal bake");
    let mut cleanup_started = false;
    for _ in 0..400 {
        let deletes = fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with("CHECKPOINT_DELETE\t"))
            .count();
        if deletes > deletes_before_late_signal {
            cleanup_started = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(cleanup_started, "prior checkpoint cleanup never started");
    let signal_status = Command::new("/bin/kill")
        .args(["-TERM", &late_signal_bake.id().to_string()])
        .status()
        .expect("signal post-commit cleanup");
    assert!(signal_status.success());
    fs::remove_file(temp.path().join("pause-checkpoint-delete"))
        .expect("release prior checkpoint cleanup");
    let late_signal_output = late_signal_bake
        .wait_with_output()
        .expect("reap post-commit signal bake");
    assert!(
        late_signal_output.status.success(),
        "post-commit signal reported an uncommitted bake: {}",
        String::from_utf8_lossy(&late_signal_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&late_signal_output.stderr)
            .contains("signal arrived after bake commit")
    );
    let committed_marker = fs::read_to_string(lane_home.join(".sprite-lane-golden"))
        .expect("post-signal committed marker");
    assert_ne!(committed_marker, replacement_marker);
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner")).expect("post-signal committed witness"),
        committed_marker
    );
    let checkpoints_after_late_signal = fs::read_dir(snapshots.join("lane-1"))
        .expect("checkpoint inventory after post-commit signal")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().and_then(|value| value.to_str()) == Some("comment")
        })
        .count();
    assert_eq!(checkpoints_after_late_signal, 1);

    let checkpoint_creates_before_signal = fs::read_to_string(&log)
        .expect("checkpoint log before signal")
        .lines()
        .filter(|line| line.starts_with("CHECKPOINT_CREATE\t"))
        .count();
    fs::write(temp.path().join("pause-checkpoint-create"), "").expect("pause candidate checkpoint");
    let mut signaled_bake_command = make_command(&["bake".into(), "lane-1".into()]);
    signaled_bake_command
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut signaled_bake = signaled_bake_command
        .spawn()
        .expect("spawn interruptible replacement bake");
    let mut candidate_exists = false;
    for _ in 0..400 {
        let creates = fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with("CHECKPOINT_CREATE\t"))
            .count();
        if creates > checkpoint_creates_before_signal {
            candidate_exists = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(candidate_exists, "replacement candidate was never created");
    fs::write(temp.path().join("pause-rollback-restore"), "")
        .expect("pause signal rollback cleanup");
    let signal_status = Command::new("/bin/kill")
        .args(["-TERM", &signaled_bake.id().to_string()])
        .status()
        .expect("signal replacement bake");
    assert!(signal_status.success());
    fs::remove_file(temp.path().join("pause-checkpoint-create"))
        .expect("release candidate checkpoint");
    let mut rollback_cleanup_started = false;
    for _ in 0..400 {
        if fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .any(|line| line.starts_with("ROLLBACK_RESTORE_WAIT\t"))
        {
            rollback_cleanup_started = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        rollback_cleanup_started,
        "signal rollback cleanup never reached restore"
    );
    let second_signal = Command::new("/bin/kill")
        .args(["-TERM", &signaled_bake.id().to_string()])
        .status()
        .expect("repeat signal during rollback cleanup");
    assert!(second_signal.success());
    fs::remove_file(temp.path().join("pause-rollback-restore"))
        .expect("release signal rollback cleanup");
    assert_eq!(
        signaled_bake.wait().expect("reap signaled bake").code(),
        Some(143)
    );
    assert_eq!(
        fs::read_to_string(lane_home.join(".sprite-lane-golden"))
            .expect("remote marker restored after signal"),
        committed_marker
    );
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner"))
            .expect("local witness retained after signal"),
        committed_marker
    );
    let checkpoints_after_signal = fs::read_dir(snapshots.join("lane-1"))
        .expect("checkpoint inventory after signal")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().and_then(|value| value.to_str()) == Some("comment")
        })
        .count();
    assert_eq!(checkpoints_after_signal, 1);
    let status_after_signal = run(&["status".into(), "lane-1".into()]);
    assert!(String::from_utf8_lossy(&status_after_signal.stdout).contains("owned clean v3"));

    fs::write(temp.path().join("checkpoint-fail"), "").expect("primary rollback trigger");
    fs::write(temp.path().join("rollback-restore-fail"), "").expect("rollback restore trigger");
    let failed_rollback = run(&["bake".into(), "lane-1".into()]);
    assert_eq!(failed_rollback.status.code(), Some(94));
    let rollback_error = String::from_utf8_lossy(&failed_rollback.stderr);
    assert!(rollback_error.contains("rollback incomplete"));
    assert!(rollback_error.contains("preserving the primary bake failure"));
    assert_eq!(
        fs::read_to_string(owners.join("lane-1.owner"))
            .expect("old witness retained after rollback failure"),
        committed_marker
    );
    let status_after_rollback_failure = run(&["status".into(), "lane-1".into()]);
    assert!(status_after_rollback_failure.status.success());
    assert!(
        String::from_utf8_lossy(&status_after_rollback_failure.stdout)
            .contains("bake: NOT OWNED/VALID")
    );
    fs::remove_file(temp.path().join("checkpoint-fail")).expect("clear primary trigger");
    fs::remove_file(temp.path().join("rollback-restore-fail")).expect("clear rollback trigger");

    let log_text = fs::read_to_string(&log).expect("Sprite command log");
    assert!(!log_text.contains("AMBIENT_LOCAL_ENV"));
    assert!(!log_text.contains("SECRET_TRANSPORT_USED"));
    assert!(!log_text.contains("DIRTY_GIT_ENV"));
    assert!(!hostile_log.exists(), "hostile BASH_ENV or function ran");
    let clone_destinations = log_text
        .lines()
        .filter(|line| line.starts_with("GIT\t") && line.contains("\tclone\t"))
        .filter_map(|line| line.rsplit('\t').next())
        .collect::<Vec<_>>();
    assert!(clone_destinations.len() >= 2);
    assert!(
        clone_destinations.iter().all(|path| *path == "."),
        "clone escaped the descriptor-anchored lane cwd: {clone_destinations:?}"
    );
}

#[cfg(unix)]
fn write_executable(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod executable");
}

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
fn orchestrator_resolves_the_fleet_feed_mcp_for_each_tier_one_harness() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");

    for agent in ["amos", "kaylee", "urza"] {
        let resolved = roster
            .resolve(agent)
            .unwrap_or_else(|error| panic!("resolve {agent}: {error}"));
        let mcps = resolved
            .mcps
            .iter()
            .map(|mcp| mcp.identity.as_str())
            .collect::<Vec<_>>();
        assert!(
            mcps.contains(&"core/mcp:overmind"),
            "{agent} must carry the fleet Feed MCP: {mcps:?}"
        );
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
        [
            "--search",
            "--sandbox",
            "workspace-write",
            "--ask-for-approval",
            "on-request",
        ]
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
fn estate_intents_materialize_provider_native_composition() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roster = Roster::load_config(root.join("examples/config.yaml")).expect("load example");

    let estate_files = |directory: &str| {
        let mut names = fs::read_dir(root.join(directory))
            .unwrap_or_else(|error| panic!("read public {directory}: {error}"))
            .map(|entry| entry.expect("read public entry").path())
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| name.starts_with("estate-infrastructure-"))
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(
        estate_files("primitives/guidance"),
        [
            "estate-infrastructure-manage.md".to_owned(),
            "estate-infrastructure-observe-plan.md".to_owned(),
        ]
    );
    assert_eq!(
        estate_files("packs"),
        [
            "estate-infrastructure-manage.yaml".to_owned(),
            "estate-infrastructure-observe-plan.yaml".to_owned(),
        ]
    );

    for (intent, guidance_identity) in [
        (
            "observe-plan",
            "core/guidance:estate-infrastructure-observe-plan",
        ),
        ("manage", "core/guidance:estate-infrastructure-manage"),
    ] {
        let include = vec![format!("core/pack:estate-infrastructure-{intent}")];
        let resolved = roster
            .resolve_ad_hoc(
                "hephaestus",
                &format!("estate-{intent}"),
                &format!("Request Estate {intent} work."),
                &include,
            )
            .unwrap_or_else(|error| panic!("resolve {intent}: {error}"));
        let expected_role = format!("ad-hoc/role:estate-{intent}");
        let expected_pack = format!("core/pack:estate-infrastructure-{intent}");

        assert_eq!(resolved.role, "ad-hoc");
        assert_eq!(resolved.guidance.len(), 1);
        assert_eq!(resolved.skills.len(), 1);
        assert_eq!(
            resolved
                .guidance
                .iter()
                .map(|item| item.identity.as_str())
                .collect::<Vec<_>>(),
            [guidance_identity]
        );
        assert_eq!(
            resolved
                .skills
                .iter()
                .map(|item| item.identity.as_str())
                .collect::<Vec<_>>(),
            ["core/skill:estate-infrastructure"]
        );
        assert_eq!(
            resolved.guidance[0].via,
            [[expected_role.clone(), expected_pack.clone()]]
        );
        assert_eq!(resolved.skills[0].via, [[expected_role, expected_pack]]);
    }

    let include = vec!["core/pack:estate-infrastructure-manage".to_owned()];
    let resolved = roster
        .resolve_ad_hoc(
            "hephaestus",
            "estate-manage-proof",
            "Prove the public Estate provider-native manage projection.",
            &include,
        )
        .expect("resolve Estate manage proof agent");
    let temp = tempfile::tempdir().expect("tempdir");
    let bundle = temp.path().join("bundle");
    let manifest = resolved
        .write_bundle(&bundle, temp.path())
        .expect("materialize Estate manage proof agent");
    let agents = fs::read_to_string(bundle.join("AGENTS.md")).expect("read AGENTS.md");
    let skill = fs::read_to_string(bundle.join("skills/estate-infrastructure/SKILL.md"))
        .expect("read Estate skill");

    assert_eq!(manifest.role, "ad-hoc");
    assert_eq!(manifest.guidance.len(), 1);
    assert_eq!(manifest.skills.len(), 1);
    assert_eq!(
        manifest
            .guidance
            .iter()
            .map(|item| item.identity.as_str())
            .collect::<Vec<_>>(),
        ["core/guidance:estate-infrastructure-manage"]
    );
    assert_eq!(
        manifest
            .skills
            .iter()
            .map(|item| item.identity.as_str())
            .collect::<Vec<_>>(),
        ["core/skill:estate-infrastructure"]
    );
    assert_eq!(
        manifest.guidance[0].via,
        [[
            "ad-hoc/role:estate-manage-proof".to_owned(),
            "core/pack:estate-infrastructure-manage".to_owned(),
        ]]
    );
    assert_eq!(
        manifest.skills[0].via,
        [[
            "ad-hoc/role:estate-manage-proof".to_owned(),
            "core/pack:estate-infrastructure-manage".to_owned(),
        ]]
    );
    let normalized_agents = agents.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized_agents.contains(
            "Manage only from a saved exact OpenTofu/provider plan whose resource scope and digest have been checked"
        )
    );
    assert!(
        normalized_agents
            .contains("Expired standards or exceptions and missing evidence fail closed.")
    );
    assert!(normalized_agents.contains("secret-free evidence pointer"));
    let normalized_skill = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(normalized_skill.contains("ordinary OpenTofu/provider tooling"));
    assert!(agents.contains("skills/estate-infrastructure/SKILL.md"));
    assert!(normalized_skill.contains("estate map"));
    assert!(normalized_skill.contains("estate resource <id>"));
    assert!(normalized_skill.contains("saved exact OpenTofu/provider plan"));
    assert!(normalized_skill.contains("exact `owner_repo` product identity"));
    assert!(normalized_skill.contains("That graph is the routing table"));
    assert!(
        normalized_skill
            .contains("do not copy Misty Step's private topology into a public product repository")
    );
    assert!(
        normalized_skill
            .contains("obtain explicit operator approval of that exact plan and digest")
    );
    assert!(normalized_skill.contains("scoped Mint/provider credential"));
    assert!(normalized_skill.contains("Roster authority-provider receipt"));
    assert!(normalized_skill.contains("provider readback"));
    assert!(
        normalized_skill.contains("secret-free evidence pointer for Estate's next reconciliation")
    );

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

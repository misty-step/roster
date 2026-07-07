//! Real-binary proof that a HANDLED `roster` error reaches Canary before the
//! process exits. This is the regression guard for the short-lived-CLI race:
//! `report_error` spawns the send off the hot path, so a missing flush would
//! let the process exit (printing the error, returning 1) before the send
//! lands -- reported in code, invisible at the hub. A unit test that calls
//! `report_error` + `flush` in-process cannot catch that: only spawning the
//! actual binary and watching the wire proves `main` flushes on the `Err`
//! path. Mirrors the mock-listener shape already used by `PowderStub` in
//! `cli.rs`, but reads the full request body (not just headers).

use assert_cmd::cargo::CommandCargoExt;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// Read one full HTTP request (headers + Content-Length body) from a mock
/// connection, reply 200, and return the raw request text.
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("set read timeout");
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(read) = stream.read(&mut buffer) {
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&request);
        let Some(header_end) = text.find("\r\n\r\n") else {
            continue;
        };
        let content_length = text
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
    let response = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}";
    let _ = stream.write_all(response.as_bytes());
    String::from_utf8_lossy(&request).into_owned()
}

fn body_json(request: &str) -> serde_json::Value {
    let raw = request.split("\r\n\r\n").nth(1).unwrap_or_default();
    serde_json::from_str(raw).expect("mock request body is valid json")
}

#[test]
fn handled_error_reaches_canary_before_process_exits() {
    // Nonblocking so the accept loop can poll against a wall-clock deadline
    // instead of blocking forever if the binary never connects.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock canary");
    let address = listener.local_addr().expect("mock canary address");
    listener
        .set_nonblocking(true)
        .expect("nonblocking mock listener");

    // `show <missing>` loads the real roster (so `Roster::load` succeeds) then
    // fails in `find_agent`, exercising `main`'s `Err` arm -- the exact
    // command the audit fired by hand. `env_remove` pins service/monitor to
    // the built-in `roster` identity regardless of the caller's environment.
    let mut child = Command::cargo_bin("roster")
        .expect("roster binary")
        .arg("--root")
        .arg(workspace_root())
        .args(["show", "__definitely_missing_agent__"])
        .env("CANARY_ENDPOINT", format!("http://{address}"))
        .env("CANARY_API_KEY", "test-key")
        .env_remove("CANARY_INGEST_KEY")
        .env_remove("CANARY_SERVICE")
        .env_remove("CANARY_MONITOR")
        .env_remove("CANARY_ENVIRONMENT")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn roster");

    // The CLI opens two connections (check-in + error) on separate threads;
    // order is not guaranteed, so collect until the error POST arrives or the
    // deadline trips.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut requests = Vec::new();
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = read_request(&mut stream);
                let is_error = request.starts_with("POST /api/v1/errors");
                requests.push(request);
                if is_error {
                    break;
                }
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("mock accept failed: {error}"),
        }
    }

    let status = child.wait().expect("wait roster");
    assert!(
        !status.success(),
        "roster show <missing> must exit non-zero"
    );

    let error_request = requests
        .iter()
        .find(|request| request.starts_with("POST /api/v1/errors"))
        .unwrap_or_else(|| {
            panic!(
                "roster must POST /api/v1/errors before exiting; got: {:?}",
                requests
                    .iter()
                    .map(|r| r.lines().next().unwrap_or_default())
                    .collect::<Vec<_>>()
            )
        });

    assert!(error_request.contains("Authorization: Bearer test-key"));
    let body = body_json(error_request);
    assert_eq!(body["service"], "roster");
    assert_eq!(body["error_class"], "roster.run.failed");
    assert_eq!(body["severity"], "error");
    assert!(
        body["message"]
            .as_str()
            .expect("message is a string")
            .contains("unknown agent"),
        "message was: {}",
        body["message"]
    );
}

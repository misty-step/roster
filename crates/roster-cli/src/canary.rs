//! Fire-and-forget Canary self-reporter. No creds => silent no-op.
//! A Canary outage never blocks, slows, or panics `roster`.
//!
//! `CANARY_ENDPOINT` + (`CANARY_API_KEY` or `CANARY_INGEST_KEY`) gate every
//! send; unset either and every call below is a no-op. `roster` is a
//! short-lived CLI, not a long-running service, so there is no background
//! health loop: each send runs on its own thread off the hot path, and
//! [`flush`] joins any in-flight sends (bounded by [`SEND_TIMEOUT`] per
//! attempt) before the process exits, so a fired proof event actually
//! reaches the network instead of being dropped on exit.

use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

const SERVICE: &str = "roster";
const MONITOR: &str = "roster";
const TTL_MS: u64 = 120_000;
const SEND_TIMEOUT: Duration = Duration::from_secs(3);

fn config() -> Option<(String, String)> {
    let endpoint = std::env::var("CANARY_ENDPOINT").ok()?;
    let key = std::env::var("CANARY_API_KEY")
        .or_else(|_| std::env::var("CANARY_INGEST_KEY"))
        .ok()?;
    (!endpoint.trim().is_empty() && !key.trim().is_empty())
        .then(|| (endpoint.trim_end_matches('/').to_owned(), key))
}

fn service() -> String {
    std::env::var("CANARY_SERVICE")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| SERVICE.to_owned())
}

fn pending() -> &'static Mutex<Vec<JoinHandle<()>>> {
    static PENDING: OnceLock<Mutex<Vec<JoinHandle<()>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(Vec::new()))
}

/// Report a handled or unhandled error. Safe to call anywhere; a no-op
/// without `CANARY_ENDPOINT`/`CANARY_API_KEY` (or `CANARY_INGEST_KEY`).
pub fn report_error(error_class: &str, message: &str) {
    let Some((endpoint, key)) = config() else {
        return;
    };
    let environment =
        std::env::var("CANARY_ENVIRONMENT").unwrap_or_else(|_| "production".to_owned());
    let body = serde_json::json!({
        "service": service(),
        "error_class": error_class,
        "message": message.chars().take(4096).collect::<String>(),
        "severity": "error",
        "environment": environment,
    });
    spawn_send(endpoint, key, "/api/v1/errors", body);
}

/// One check-in per CLI invocation. A no-op without
/// `CANARY_ENDPOINT`/`CANARY_API_KEY` (or `CANARY_INGEST_KEY`).
pub fn check_in() {
    let Some((endpoint, key)) = config() else {
        return;
    };
    let body = serde_json::json!({
        "monitor": MONITOR,
        "status": "alive",
        "summary": concat!(env!("CARGO_PKG_NAME"), " run"),
        "ttl_ms": TTL_MS,
    });
    spawn_send(endpoint, key, "/api/v1/check-ins", body);
}

/// Block until any in-flight sends finish (each bounded by [`SEND_TIMEOUT`]
/// per attempt). Call before process exit: `roster` is short-lived and would
/// otherwise race a detached send thread to process teardown.
pub fn flush() {
    let Ok(mut handles) = pending().lock() else {
        return;
    };
    for handle in handles.drain(..) {
        let _ = handle.join();
    }
}

fn spawn_send(endpoint: String, key: String, path: &'static str, body: serde_json::Value) {
    let spawned = std::thread::Builder::new()
        .name("canary-report".into())
        .spawn(move || {
            let agent = ureq::AgentBuilder::new().timeout(SEND_TIMEOUT).build();
            let url = format!("{endpoint}{path}");
            let auth = format!("Bearer {key}");
            for _ in 0..2 {
                // one retry, then give up silently
                let ok = agent
                    .post(&url)
                    .set("Authorization", &auth)
                    .send_json(&body)
                    .is_ok();
                if ok {
                    break;
                }
            }
        });
    let Ok(handle) = spawned else {
        return;
    };
    let Ok(mut handles) = pending().lock() else {
        return;
    };
    handles.push(handle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Instant;

    /// Serializes every test in this module: they all mutate the same
    /// process-global `CANARY_*` env vars, which race under the default
    /// multi-threaded test runner.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn clear_canary_env() {
        for var in [
            "CANARY_ENDPOINT",
            "CANARY_API_KEY",
            "CANARY_INGEST_KEY",
            "CANARY_SERVICE",
            "CANARY_ENVIRONMENT",
        ] {
            // SAFETY: serialized by ENV_GUARD; no other thread reads/writes
            // these vars concurrently within this test binary.
            unsafe { std::env::remove_var(var) };
        }
    }

    /// Accepts exactly one connection, captures the raw HTTP request text,
    /// and replies 200. Returns the endpoint URL and a channel yielding the
    /// captured request once received.
    fn serve_once() -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock listener");
        let address = listener.local_addr().expect("mock listener address");
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
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
            let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
            let response = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}";
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{address}"), receiver)
    }

    fn body_of(request: &str) -> serde_json::Value {
        let raw = request.split("\r\n\r\n").nth(1).unwrap_or_default();
        serde_json::from_str(raw).expect("mock request body is valid json")
    }

    #[test]
    fn check_in_posts_to_check_ins_endpoint() {
        let _guard = lock_env();
        clear_canary_env();
        let (endpoint, received) = serve_once();
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &endpoint);
            std::env::set_var("CANARY_API_KEY", "test-key");
        }

        check_in();
        flush();

        let request = received
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server received a request");
        clear_canary_env();

        assert!(request.starts_with("POST /api/v1/check-ins"));
        assert!(request.contains("Authorization: Bearer test-key"));
        let body = body_of(&request);
        assert_eq!(body["monitor"], "roster");
        assert_eq!(body["status"], "alive");
        assert_eq!(body["ttl_ms"], 120_000);
    }

    #[test]
    fn report_error_posts_to_errors_endpoint() {
        let _guard = lock_env();
        clear_canary_env();
        let (endpoint, received) = serve_once();
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &endpoint);
            std::env::set_var("CANARY_API_KEY", "test-key");
            std::env::set_var("CANARY_ENVIRONMENT", "test");
        }

        report_error("roster.run.failed", "boom");
        flush();

        let request = received
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server received a request");
        clear_canary_env();

        assert!(request.starts_with("POST /api/v1/errors"));
        assert!(request.contains("Authorization: Bearer test-key"));
        let body = body_of(&request);
        assert_eq!(body["service"], "roster");
        assert_eq!(body["error_class"], "roster.run.failed");
        assert_eq!(body["message"], "boom");
        assert_eq!(body["severity"], "error");
        assert_eq!(body["environment"], "test");
    }

    #[test]
    fn dead_port_returns_without_hang_or_panic() {
        let _guard = lock_env();
        clear_canary_env();
        // Bind then immediately drop: reserves a free port with nothing
        // listening, so the connect fails fast (connection refused) instead
        // of flaking on an arbitrary fixed port.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind throwaway listener");
        let address = listener.local_addr().expect("throwaway listener address");
        drop(listener);
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", format!("http://{address}"));
            std::env::set_var("CANARY_API_KEY", "test-key");
        }

        let started = Instant::now();
        report_error("roster.run.failed", "unreachable");
        flush();
        let elapsed = started.elapsed();
        clear_canary_env();

        assert!(
            elapsed < Duration::from_secs(10),
            "flush took {elapsed:?}, expected a bounded failure, not a hang"
        );
    }

    #[test]
    fn missing_credentials_is_silent_noop() {
        let _guard = lock_env();
        clear_canary_env();

        let started = Instant::now();
        check_in();
        report_error("roster.run.failed", "no creds");
        flush();
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "no-op path should return immediately, took {elapsed:?}"
        );
    }
}

//! Fire-and-forget Canary self-reporter shared by every roster binary
//! (`roster`, `roster-api`, `roster-mcp`).
//!
//! No blanket `#![forbid(unsafe_code)]` here (unlike `roster-mcp`/`roster-api`):
//! the test module below uses `unsafe { std::env::set_var/remove_var }` to
//! serialize `CANARY_*` env mutation across tests (2024 edition requires
//! `unsafe` for those calls). Non-test code stays entirely safe.
//!
//! `CANARY_ENDPOINT` + (`CANARY_API_KEY` or `CANARY_INGEST_KEY`) gate every
//! send; unset either and every call below is a no-op. A Canary outage never
//! blocks, slows, or panics the host process: each send runs on its own
//! thread off the hot path, bounded by [`SEND_TIMEOUT`] per attempt.
//!
//! Comprehensive coverage (not just a hand-wired call at `main`'s `Err` arm)
//! comes from three pieces a binary wires once at startup, in order:
//!
//! 1. [`init`] -- sets this binary's default `service`/`monitor` identity
//!    (`CANARY_SERVICE`/`CANARY_MONITOR` env still win when set).
//! 2. [`init_tracing`] -- installs a `tracing` subscriber with [`CanaryLayer`]
//!    registered, so any `tracing::error!` anywhere in the process (or its
//!    library dependencies) is auto-forwarded with zero per-site wiring.
//! 3. [`install_panic_hook`] -- reports `<service>.panic` and flushes before
//!    the default panic hook runs.
//!
//! Long-running services additionally call [`start_health_loop`] in *every*
//! `serve`/`mcp`/daemon entry point; one-shot CLIs call [`check_in`] once per
//! invocation instead.

use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

const DEFAULT_SERVICE: &str = "roster";
const DEFAULT_MONITOR: &str = "roster";
const CHECKIN_INTERVAL: Duration = Duration::from_secs(60);
const TTL_MS: u64 = 120_000;
const SEND_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
struct Identity {
    service: &'static str,
    monitor: &'static str,
}

static IDENTITY: OnceLock<Identity> = OnceLock::new();

/// Set this binary's default `service`/`monitor` identity. Call once, as
/// early as possible in `main`, before [`init_tracing`],
/// [`install_panic_hook`], or [`start_health_loop`].
///
/// `CANARY_SERVICE`/`CANARY_MONITOR` env vars still override this at every
/// call site (operator escape hatch). Safe to call more than once or not at
/// all -- falls back to `"roster"`/`"roster"`; only the first call wins.
pub fn init(service: &'static str, monitor: &'static str) {
    let _ = IDENTITY.set(Identity { service, monitor });
}

fn identity() -> Identity {
    IDENTITY.get().copied().unwrap_or(Identity {
        service: DEFAULT_SERVICE,
        monitor: DEFAULT_MONITOR,
    })
}

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
        .unwrap_or_else(|| identity().service.to_owned())
}

fn monitor() -> String {
    std::env::var("CANARY_MONITOR")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| identity().monitor.to_owned())
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

/// Heartbeat. A no-op without `CANARY_ENDPOINT`/`CANARY_API_KEY` (or
/// `CANARY_INGEST_KEY`). CLIs call this once per invocation; services get a
/// continuous version via [`start_health_loop`].
pub fn check_in() {
    let Some((endpoint, key)) = config() else {
        return;
    };
    let body = serde_json::json!({
        "monitor": monitor(),
        "status": "alive",
        "summary": format!("{} heartbeat", service()),
        "ttl_ms": TTL_MS,
    });
    spawn_send(endpoint, key, "/api/v1/check-ins", body);
}

/// Services only: fire a check-in immediately, then every 60s from a named
/// background thread (TTL 120s). Call this in *every* long-running bootstrap
/// -- each `serve`/`mcp`/daemon entry point, not just `main` -- otherwise the
/// process reads as falsely overdue once it outlives one TTL window.
pub fn start_health_loop() {
    if config().is_none() {
        return;
    }
    check_in();
    let _ = std::thread::Builder::new()
        .name("canary-health".into())
        .spawn(|| {
            loop {
                std::thread::sleep(CHECKIN_INTERVAL);
                check_in();
            }
        });
}

/// Block until any in-flight sends finish (each bounded by [`SEND_TIMEOUT`]
/// per attempt). Call before process exit on a short-lived binary, and
/// inside the panic hook, so a fired proof event actually reaches the
/// network instead of being dropped on exit/unwind.
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

/// Install a `tracing` subscriber: a stderr fmt layer (respecting
/// `RUST_LOG`, default `warn`) plus [`CanaryLayer`], so every
/// `tracing::error!` anywhere in the process -- or a library it calls into
/// -- lands in Canary with zero per-site wiring. Call once, near the top of
/// `main`, before [`install_panic_hook`]. Idempotent: a second call is a
/// silent no-op rather than a panic (`try_init`), so it is safe to call from
/// more than one entry point in the same process (e.g. tests).
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    // `with_writer(stderr)` is load-bearing, not cosmetic: `roster-mcp`
    // speaks JSON-RPC over stdout, one object per line. The default fmt
    // layer writer is stdout -- left at the default, a single
    // `tracing::error!` interleaves a log line into that stream and breaks
    // every client's line-delimited JSON parser.
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")));
    let _ = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(CanaryLayer)
        .try_init();
}

/// Tracing targets whose `ERROR`-level events must never auto-forward,
/// because something else on the same code path already reports them.
///
/// `tower_http::catch_panic` is `tower_http`'s own `tracing::error!` inside
/// `CatchPanicLayer` -- it fires for the *same* panic that
/// [`install_panic_hook`]'s panic hook already reports as `<service>.panic`
/// (a panic hook always runs on unwind, regardless of who catches it). Left
/// unfiltered, [`CanaryLayer`] would auto-forward it too, doubling every
/// caught-panic report. Deny only this exact target -- other `tower_http`
/// targets (request tracing, etc.) still forward normally.
const DENIED_TARGETS: &[&str] = &["tower_http::catch_panic"];

/// A `tracing_subscriber` [`Layer`] that forwards every `ERROR`-level event
/// to [`report_error`]. Register it once via [`init_tracing`] (or directly)
/// and "app logging" becomes error capture: any `tracing::error!(...)`
/// anywhere in the app or its libraries reaches Canary with zero per-site
/// wiring. [`DENIED_TARGETS`] excludes the small set of targets that would
/// otherwise duplicate a report made elsewhere on the same path.
pub struct CanaryLayer;

impl<S: Subscriber> Layer<S> for CanaryLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if config().is_none() || *event.metadata().level() != Level::ERROR {
            return;
        }
        let target = event.metadata().target();
        if DENIED_TARGETS.contains(&target) {
            return;
        }
        let mut msg = String::new();
        event.record(&mut Visitor(&mut msg));
        let class = format!("{}.{}", service(), target);
        report_error(&class, &redact(&msg));
    }
}

struct Visitor<'a>(&'a mut String);
impl tracing::field::Visit for Visitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(&format!("{}={:?}", field.name(), value));
    }
}

/// Scrub obviously secret-shaped substrings before a log line leaves the
/// process. Defense in depth for the auto-forwarded tracing path: no call
/// site should ever put a real credential into a tracing field, but a
/// forwarded `error!` is a leak surface this crate did not create, so it
/// gets a floor. Redacts `Bearer <token>` occurrences (case-insensitive);
/// identity otherwise.
fn redact(message: &str) -> String {
    const MARKER: &str = "earer ";
    let mut result = String::with_capacity(message.len());
    let mut rest = message;
    loop {
        let Some(marker_at) = rest.find(MARKER) else {
            result.push_str(rest);
            break;
        };
        // Require the char just before "earer " to be a 'B'/'b' so this only
        // matches "Bearer "/"bearer ", not an arbitrary "...earer " substring.
        if marker_at == 0 || !matches!(rest.as_bytes()[marker_at - 1], b'B' | b'b') {
            let keep_to = marker_at + MARKER.len();
            result.push_str(&rest[..keep_to]);
            rest = &rest[keep_to..];
            continue;
        }
        let token_start = marker_at + MARKER.len();
        result.push_str(&rest[..token_start]);
        result.push_str("[REDACTED]");
        let token_end = rest[token_start..]
            .find(char::is_whitespace)
            .map(|offset| token_start + offset)
            .unwrap_or(rest.len());
        rest = &rest[token_end..];
    }
    result
}

/// Install a panic hook that reports `<service>.panic` (payload + location)
/// and flushes before the default hook runs. A no-op without
/// `CANARY_ENDPOINT`/`CANARY_API_KEY` (or `CANARY_INGEST_KEY`). Call once,
/// near the top of `main`.
///
/// For Axum services, pair this with `tower_http::catch_panic::CatchPanicLayer`
/// so a panicking handler both reports here *and* returns a 500 instead of
/// silently killing the worker task -- this hook alone still fires either
/// way, since a panic hook always runs on unwind regardless of who catches it.
pub fn install_panic_hook() {
    if config().is_none() {
        return;
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".to_owned());
        report_error(&format!("{}.panic", service()), &format!("{msg} @ {loc}"));
        flush(); // best-effort before the process dies
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Instant;

    /// Serializes every test in this module: they all mutate the same
    /// process-global `CANARY_*` env vars (and, for the panic-hook test, the
    /// process-global panic hook), which race under the default
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
            "CANARY_MONITOR",
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
        assert_eq!(body["status"], "alive");
        assert_eq!(body["ttl_ms"], 120_000);
    }

    #[test]
    fn check_in_honors_monitor_env_override() {
        let _guard = lock_env();
        clear_canary_env();
        let (endpoint, received) = serve_once();
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &endpoint);
            std::env::set_var("CANARY_API_KEY", "test-key");
            std::env::set_var("CANARY_MONITOR", "roster-api");
        }

        check_in();
        flush();

        let request = received
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server received a request");
        clear_canary_env();

        let body = body_of(&request);
        assert_eq!(body["monitor"], "roster-api");
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
            std::env::set_var("CANARY_SERVICE", "roster-test");
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
        assert_eq!(body["service"], "roster-test");
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

    #[test]
    fn canary_layer_forwards_tracing_error_to_report_error() {
        use tracing_subscriber::layer::SubscriberExt;

        let _guard = lock_env();
        clear_canary_env();
        let (endpoint, received) = serve_once();
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &endpoint);
            std::env::set_var("CANARY_API_KEY", "test-key");
            std::env::set_var("CANARY_SERVICE", "roster-layer-test");
        }

        let subscriber = tracing_subscriber::registry().with(CanaryLayer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(target: "roster_canary::tests", "boom from layer");
            // INFO must NOT be forwarded -- only ERROR crosses the layer.
            tracing::info!(target: "roster_canary::tests", "should not be reported");
        });
        flush();

        let request = received
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server received a request forwarded by CanaryLayer");
        clear_canary_env();

        assert!(request.starts_with("POST /api/v1/errors"));
        let body = body_of(&request);
        assert_eq!(body["service"], "roster-layer-test");
        assert_eq!(
            body["error_class"],
            "roster-layer-test.roster_canary::tests"
        );
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("boom from layer"),
            "message was: {}",
            body["message"]
        );
    }

    #[test]
    fn canary_layer_denies_tower_http_catch_panic_target_only() {
        use tracing_subscriber::layer::SubscriberExt;

        let _guard = lock_env();
        clear_canary_env();

        // `tower_http::catch_panic` must never auto-forward, even though
        // CanaryLayer is otherwise wired to forward every ERROR event: the
        // panic hook already reports the same panic as `<service>.panic`,
        // and double-forwarding would double every caught-panic report.
        // Spawn a one-shot accept watcher rather than serving a real
        // response -- if the denylist regresses and a connection actually
        // arrives, that alone proves the bug; nothing needs to read the
        // request body.
        let denied_listener =
            TcpListener::bind("127.0.0.1:0").expect("bind denied-target listener");
        let denied_endpoint = format!(
            "http://{}",
            denied_listener.local_addr().expect("denied listener addr")
        );
        let (saw_connection_tx, saw_connection_rx) = mpsc::channel();
        std::thread::spawn(move || {
            if denied_listener.accept().is_ok() {
                let _ = saw_connection_tx.send(());
            }
        });
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &denied_endpoint);
            std::env::set_var("CANARY_API_KEY", "test-key");
        }
        let subscriber = tracing_subscriber::registry().with(CanaryLayer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(target: "tower_http::catch_panic", "Service panicked: kaboom");
        });
        flush();
        assert!(
            saw_connection_rx
                .recv_timeout(Duration::from_millis(500))
                .is_err(),
            "tower_http::catch_panic must never auto-forward -- the panic hook already reports it"
        );

        // A *different* tower_http target must still forward normally,
        // proving the deny list is scoped to catch_panic only.
        let (endpoint, received) = serve_once();
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &endpoint);
        }
        let subscriber = tracing_subscriber::registry().with(CanaryLayer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(target: "tower_http::trace", "unrelated tower_http error");
        });
        flush();
        let request = received
            .recv_timeout(Duration::from_secs(5))
            .expect("non-denied tower_http target must still forward");
        clear_canary_env();

        assert!(request.starts_with("POST /api/v1/errors"));
        let body = body_of(&request);
        assert_eq!(body["error_class"], "roster.tower_http::trace");
    }

    #[test]
    fn panic_hook_reports_service_scoped_panic_class() {
        let _guard = lock_env();
        clear_canary_env();
        let (endpoint, received) = serve_once();
        // SAFETY: serialized by ENV_GUARD.
        unsafe {
            std::env::set_var("CANARY_ENDPOINT", &endpoint);
            std::env::set_var("CANARY_API_KEY", "test-key");
            std::env::set_var("CANARY_SERVICE", "roster-panic-test");
        }

        install_panic_hook();
        // `catch_unwind` still runs the installed panic *hook* (that's how
        // hooks work -- they fire on unwind regardless of who catches it);
        // this just stops the unwind from taking down the test process.
        let result = std::panic::catch_unwind(|| {
            panic!("kaboom-payload");
        });
        assert!(result.is_err());
        // `take_hook` always resets to the default hook afterward, so this
        // also un-installs our hook and prevents it leaking into later tests.
        let _ = std::panic::take_hook();

        let request = received
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server received a request from the panic hook");
        clear_canary_env();

        assert!(request.starts_with("POST /api/v1/errors"));
        let body = body_of(&request);
        assert_eq!(body["service"], "roster-panic-test");
        assert_eq!(body["error_class"], "roster-panic-test.panic");
        assert!(
            body["message"].as_str().unwrap().contains("kaboom-payload"),
            "message was: {}",
            body["message"]
        );
    }

    #[test]
    fn redact_scrubs_bearer_tokens_case_insensitively() {
        assert_eq!(
            redact("Authorization: Bearer abc123XYZ status=401"),
            "Authorization: Bearer [REDACTED] status=401"
        );
        assert_eq!(
            redact("retrying with bearer sekret-token then giving up"),
            "retrying with bearer [REDACTED] then giving up"
        );
        assert_eq!(redact("no secrets here"), "no secrets here");
        assert_eq!(
            redact("two: Bearer aaa and Bearer bbb"),
            "two: Bearer [REDACTED] and Bearer [REDACTED]"
        );
    }
}

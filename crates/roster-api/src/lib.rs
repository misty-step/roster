#![forbid(unsafe_code)]

//! The roster HTTP API face. Every route is a thin translation into
//! `roster_mcp::call_tool` -- the same dispatcher the MCP server and (via
//! `roster-core`) the CLI already exercise -- so "same semantics as the
//! CLI" holds by construction rather than by a second implementation to
//! keep in sync.

mod ui;

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::Html,
    routing::get,
};
use roster_core::{Models, Roster, SubagentPool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tower_http::catch_panic::CatchPanicLayer;

#[derive(Clone)]
struct AppState {
    root: Arc<PathBuf>,
}

/// Build the router for one roster checkout.
///
/// `root` is fixed once at server construction, unlike the CLI's `--root`
/// flag or the MCP server's `ROSTER_ROOT` env var. Those are trusted local
/// invocations; an HTTP server may be reached by callers who should not get
/// to choose which directory on this host gets read, so root is a
/// server-startup concern here, never a per-request one.
pub fn router(root: PathBuf) -> Router {
    let state = AppState {
        root: Arc::new(root),
    };
    let router = Router::new()
        .route("/", get(agents_page))
        .route("/health", get(health))
        .route("/v1/agents", get(list_agents))
        .route("/v1/agents/{agent}", get(show_agent))
        .route("/v1/agents/{agent}/brief", get(brief_agent))
        .route("/v1/agents/{agent}/materialize", get(materialize_agent));
    // Debug-only proof route: compiled out of `--release` builds (Fly
    // deploys build release), so it never exists in production. Lets a
    // local `cargo run`/`cargo test` (both debug builds) force a real panic
    // to verify `install_panic_hook()` + `CatchPanicLayer` together --
    // report to Canary AND return 500 instead of killing the worker task.
    #[cfg(debug_assertions)]
    let router = router.route("/debug/panic", get(debug_panic));
    router
        // Catches a panicking handler so it reports (the panic hook still
        // fires on unwind regardless of who catches it) and returns 500
        // instead of silently killing the worker task.
        .layer(CatchPanicLayer::new())
        .with_state(state)
}

#[cfg(debug_assertions)]
async fn debug_panic() -> StatusCode {
    panic!("roster-api debug panic route fired");
}

/// The persistent roster UI (roster-928): reads the live checkout fresh on
/// every request -- `Roster::load`/`Models::load`/`SubagentPool::load` are
/// no-cache, so a `role.yaml` edit on disk shows up on the next reload with
/// no regenerate/republish step.
async fn agents_page(State(state): State<AppState>) -> (StatusCode, Html<String>) {
    let root = state.root.as_path();
    let roster = match Roster::load(root) {
        Ok(roster) => roster,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<pre>failed to load roster: {error}</pre>")),
            );
        }
    };
    let models = Models::load(root).unwrap_or(Models {
        schema_version: String::new(),
        models: Default::default(),
    });
    let pool = SubagentPool::load(root).unwrap_or(SubagentPool {
        schema_version: String::new(),
        pool: Vec::new(),
    });
    (
        StatusCode::OK,
        Html(ui::render_agents_page(root, &roster, &models, &pool)),
    )
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn list_agents(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    dispatch("list", json!({ "root": root_str(&state) }))
}

async fn show_agent(
    State(state): State<AppState>,
    AxumPath(agent): AxumPath<String>,
) -> (StatusCode, Json<Value>) {
    dispatch("show", json!({ "root": root_str(&state), "agent": agent }))
}

#[derive(Debug, Default, Deserialize)]
struct BriefParams {
    /// Comma-separated, matching this face's own convention rather than
    /// repeated-key array syntax (`?a=1&a=2`), which plain `serde_urlencoded`
    /// does not deserialize into a `Vec` without an extra dependency.
    add_skill: Option<String>,
    add_mcp: Option<String>,
}

async fn brief_agent(
    State(state): State<AppState>,
    AxumPath(agent): AxumPath<String>,
    Query(params): Query<BriefParams>,
) -> (StatusCode, Json<Value>) {
    dispatch(
        "brief",
        json!({
            "root": root_str(&state),
            "agent": agent,
            "add_skills": split_csv(params.add_skill),
            "add_mcps": split_csv(params.add_mcp),
        }),
    )
}

#[derive(Debug, Deserialize)]
struct MaterializeParams {
    harness: Option<String>,
}

async fn materialize_agent(
    State(state): State<AppState>,
    AxumPath(agent): AxumPath<String>,
    Query(params): Query<MaterializeParams>,
) -> (StatusCode, Json<Value>) {
    dispatch(
        "materialize",
        json!({
            "root": root_str(&state),
            "agent": agent,
            "harness": params.harness.unwrap_or_default(),
        }),
    )
}

fn split_csv(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn root_str(state: &AppState) -> String {
    state.root.to_string_lossy().into_owned()
}

/// Route every verb through `roster_mcp::call_tool` and map its
/// `Result<Value, String>` onto an HTTP status. `call_tool` errors are
/// plain strings, not structured error codes, so the mapping below matches
/// on the known validation-shaped messages the MCP tests already pin
/// (`unknown agent`, `unknown harness`, `... is required`); anything else
/// (e.g. a corrupt or missing roster checkout) is a server-side problem,
/// not a caller mistake.
fn dispatch(name: &str, args: Value) -> (StatusCode, Json<Value>) {
    match roster_mcp::call_tool(name, &args) {
        Ok(value) => (StatusCode::OK, Json(value)),
        Err(message) => {
            let status = status_for(&message);
            if status.is_server_error() {
                // `tracing::error!` here (rather than a direct
                // `report_error` call) is captured automatically by
                // `CanaryLayer` registered in `main` -- this is the Axum 5xx
                // mapping the comprehensive-coverage pattern asks every
                // standing service to wire. 4xx client mistakes (unknown
                // agent, missing param) are not incidents and stay unreported.
                tracing::error!(tool = name, %status, "roster-api tool call failed: {message}");
            }
            (status, Json(json!({ "error": message })))
        }
    }
}

fn status_for(message: &str) -> StatusCode {
    if message.starts_with("unknown agent") || message.starts_with("unknown tool") {
        StatusCode::NOT_FOUND
    } else if message.starts_with("unknown harness") || message.ends_with("is required") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    async fn call(router: Router, uri: &str) -> (StatusCode, Value) {
        let response = router
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    #[tokio::test]
    async fn health_reports_ok() {
        let (status, body) = call(router(workspace_root()), "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
    }

    /// A panicking handler must not kill the whole service: `CatchPanicLayer`
    /// converts the unwind into a 500 response. This only proves the HTTP
    /// contract in-process; whether the panic *hook* also reported
    /// `roster-api.panic` to Canary is proven separately at the hub (the
    /// hook only installs from `main`, not from this library-level test).
    #[tokio::test]
    async fn debug_panic_route_returns_500_via_catch_panic_layer() {
        let (status, _body) = call(router(workspace_root()), "/debug/panic").await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn list_matches_mcp_tool_call() {
        let (status, body) = call(router(workspace_root()), "/v1/agents").await;
        assert_eq!(status, StatusCode::OK);
        let expected = roster_mcp::call_tool("list", &json!({"root": workspace_root()})).unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn show_matches_mcp_tool_call() {
        let (status, body) = call(router(workspace_root()), "/v1/agents/orchestrator").await;
        assert_eq!(status, StatusCode::OK);
        let expected = roster_mcp::call_tool(
            "show",
            &json!({"root": workspace_root(), "agent": "orchestrator"}),
        )
        .unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn show_unknown_agent_is_not_found() {
        let (status, body) = call(router(workspace_root()), "/v1/agents/nope-nobody").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("unknown agent"));
    }

    #[tokio::test]
    async fn brief_splits_comma_separated_overrides() {
        let (status, body) = call(
            router(workspace_root()),
            "/v1/agents/sweep/brief?add_skill=extra-skill&add_mcp=extra-mcp",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let expected = roster_mcp::call_tool(
            "brief",
            &json!({
                "root": workspace_root(),
                "agent": "sweep",
                "add_skills": ["extra-skill"],
                "add_mcps": ["extra-mcp"],
            }),
        )
        .unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn brief_with_no_overrides_matches_mcp_tool_call() {
        let (status, body) = call(router(workspace_root()), "/v1/agents/sweep/brief").await;
        assert_eq!(status, StatusCode::OK);
        let expected = roster_mcp::call_tool(
            "brief",
            &json!({"root": workspace_root(), "agent": "sweep", "add_skills": [], "add_mcps": []}),
        )
        .unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn materialize_matches_mcp_tool_call_per_harness() {
        let (status, body) = call(
            router(workspace_root()),
            "/v1/agents/cerberus/materialize?harness=codex",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let expected = roster_mcp::call_tool(
            "materialize",
            &json!({"root": workspace_root(), "agent": "cerberus", "harness": "codex"}),
        )
        .unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn materialize_unknown_harness_is_bad_request() {
        let (status, body) = call(
            router(workspace_root()),
            "/v1/agents/cerberus/materialize?harness=unknown",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("unknown harness"));
    }

    #[tokio::test]
    async fn materialize_missing_harness_is_bad_request() {
        let (status, body) =
            call(router(workspace_root()), "/v1/agents/cerberus/materialize").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().ends_with("is required"));
    }

    #[test]
    fn status_for_covers_known_shapes() {
        assert_eq!(status_for("unknown agent \"x\""), StatusCode::NOT_FOUND);
        assert_eq!(status_for("unknown tool: x"), StatusCode::NOT_FOUND);
        assert_eq!(
            status_for("unknown harness \"x\"; expected claude, codex, or bb"),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(status_for("agent is required"), StatusCode::BAD_REQUEST);
        assert_eq!(
            status_for("No such file or directory (os error 2)"),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn split_csv_trims_and_drops_empties() {
        assert_eq!(
            split_csv(Some(" a , b ,,c".to_string())),
            vec!["a", "b", "c"]
        );
        assert_eq!(split_csv(None), Vec::<String>::new());
    }
}

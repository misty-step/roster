use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

struct Server {
    child: Child,
    base_url: String,
}

impl Server {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_roster-api"))
            .args([
                "--root",
                workspace_root().to_str().expect("utf8 root"),
                "--bind",
                "127.0.0.1",
                "--port",
                "0",
            ])
            .stderr(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn roster-api");

        let stderr = child.stderr.take().expect("stderr");
        let mut line = String::new();
        BufReader::new(stderr)
            .read_line(&mut line)
            .expect("read startup line");
        let addr = line
            .trim()
            .rsplit("http://")
            .next()
            .expect("startup line names the bound address")
            .to_string();

        Self {
            child,
            base_url: format!("http://{addr}"),
        }
    }

    /// Consume the server with a graceful SIGTERM and confirm it actually
    /// exits cleanly, rather than just firing a signal and hoping.
    fn shutdown(mut self) -> std::process::ExitStatus {
        let pid = self.child.id().to_string();
        Command::new("kill")
            .arg(&pid)
            .status()
            .expect("send SIGTERM");
        self.child.wait().expect("wait for graceful exit")
    }

    fn get(&self, path: &str) -> ureq::Response {
        let url = format!("{}{path}", self.base_url);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match ureq::get(&url).call() {
                Ok(response) => return response,
                Err(ureq::Error::Status(_, response)) => return response,
                Err(error) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                    let _ = error;
                }
                Err(error) => panic!("GET {url} never succeeded: {error}"),
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Safety net for panic paths only -- the happy path calls
        // `shutdown()` explicitly. Skip if it already exited (e.g. via
        // `shutdown()`) so this doesn't fire a redundant signal at a
        // recycled PID.
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        // SIGTERM, not `Child::kill` (SIGKILL): a killed process never
        // returns from `main` and never flushes coverage instrumentation
        // data, which would silently zero out this binary's coverage even
        // though this test genuinely exercises it end to end.
        let _ = Command::new("kill")
            .arg(self.child.id().to_string())
            .status();
        let _ = self.child.wait();
    }
}

#[test]
fn server_binds_serves_health_list_show_brief_and_materialize() {
    let server = Server::spawn();

    let page = server.get("/");
    assert_eq!(page.status(), 200);
    let page = page.into_string().expect("agents page html");
    assert!(page.contains("<meta name=\"viewport\""));
    assert!(page.contains("orchestrator"));
    assert!(page.contains("agents declared"));

    let health: Value = server.get("/health").into_json().expect("health json");
    assert_eq!(health["status"], "ok");

    let list = server.get("/v1/agents");
    assert_eq!(list.status(), 200);
    let list: Value = list.into_json().expect("list json");
    assert!(list["structuredContent"]["agents"].is_array());

    let show: Value = server
        .get("/v1/agents/orchestrator")
        .into_json()
        .expect("show json");
    assert_eq!(show["structuredContent"]["agent"], "orchestrator");

    let brief = server.get("/v1/agents/sweep/brief?add_skill=extra-skill");
    assert_eq!(brief.status(), 200);
    let brief: Value = brief.into_json().expect("brief json");
    assert_eq!(brief["structuredContent"]["added_skills"][0], "extra-skill");

    let materialize = server.get("/v1/agents/cerberus/materialize?harness=codex");
    assert_eq!(materialize.status(), 200);

    let missing_harness = server.get("/v1/agents/cerberus/materialize");
    assert_eq!(missing_harness.status(), 400);

    let unknown_agent = server.get("/v1/agents/nope-nobody");
    assert_eq!(unknown_agent.status(), 404);

    assert!(
        server.shutdown().success(),
        "server should exit 0 on SIGTERM"
    );
}

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tokio::net::TcpListener;

#[derive(Debug, Parser)]
#[command(name = "roster-api")]
#[command(about = "Serve roster's core verbs (list/show/brief/materialize) over HTTP")]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Loopback by default -- this reads the caller's roster checkout, and
    /// nothing scopes access once a port is reachable off-host.
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
    #[arg(long, default_value_t = 4101)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    roster_canary::init("roster-api", "roster-api");
    roster_canary::init_tracing();
    roster_canary::install_panic_hook();
    // Standing service: fires a check-in immediately, then every 60s for as
    // long as this process runs -- a one-shot `check_in()` would go falsely
    // overdue once the server outlives one TTL window.
    roster_canary::start_health_loop();

    let cli = Cli::parse();
    let addr = format!("{}:{}", cli.bind, cli.port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    let bound = listener.local_addr().context("local_addr")?;
    eprintln!(
        "roster-api: serving {} on http://{bound}",
        cli.root.display()
    );
    axum::serve(listener, roster_api::router(cli.root))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve")
}

/// Exit cleanly on SIGTERM/Ctrl-C rather than being killed. A `SIGKILL`
/// never returns from `main`, so it also never flushes coverage
/// instrumentation data -- graceful shutdown is what makes the server
/// integration test's exercise of this binary actually count.
async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.ok().unwrap_or(()) };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

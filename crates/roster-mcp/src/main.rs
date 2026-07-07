#![forbid(unsafe_code)]

use std::io::{self, BufRead, Write};

use serde_json::Value;

fn main() {
    roster_canary::init("roster-mcp", "roster-mcp");
    roster_canary::init_tracing();
    roster_canary::install_panic_hook();
    // The MCP stdio loop below is a standing service for as long as the
    // client keeps the process alive -- a one-shot `check_in()` would go
    // falsely overdue once the process outlives one TTL window, so this
    // needs the continuous health loop, not just a startup ping.
    roster_canary::start_health_loop();

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }

        let request = match serde_json::from_str::<Value>(&line) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("roster-mcp: invalid json: {error}");
                continue;
            }
        };

        if let Some(response) = roster_mcp::handle_json_rpc(&request)
            && let Ok(line) = serde_json::to_string(&response)
        {
            let _ = writeln!(stdout, "{line}");
            let _ = stdout.flush();
        }
    }
}

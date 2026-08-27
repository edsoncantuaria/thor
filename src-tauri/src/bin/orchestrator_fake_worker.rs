//! Deterministic stand-in for a real agent CLI, used only by `tests/orchestrator.rs` so the
//! orchestrator's failover/timeout/token-budget paths can be exercised without a real Codex or
//! OpenCode install. Behavior is entirely env-var driven (never argv — the orchestrator always
//! appends the task spec as a trailing arg, which this binary ignores) so there's no shell or
//! quoting involved in spawning it, which keeps it identical across Windows/macOS/Linux.
//!
//! Modes:
//! - Default (one-shot): print `THOR_FAKE_TEXT`, optionally sleep `THOR_FAKE_SLEEP_MS` first,
//!   exit with `THOR_FAKE_EXIT_CODE`.
//! - `THOR_FAKE_MODE=codex_rpc`: speaks just enough of Codex's app-server JSON-RPC protocol to
//!   drive a real `thread/start` → `turn/started` → `thread/tokenUsage/updated` →
//!   `turn/completed` sequence, reporting `THOR_FAKE_TOKENS` as the total.

use std::io::{self, BufRead, Write};

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn codex_rpc_mode() {
    let tokens = env_u64("THOR_FAKE_TOKENS", 0);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if value.get("id").and_then(serde_json::Value::as_i64) == Some(2) {
            let messages = [
                serde_json::json!({ "id": 2, "result": { "thread": { "id": "fake-thread-1" } } }),
                serde_json::json!({ "method": "turn/started", "params": { "turn": { "id": "turn-1" } } }),
                serde_json::json!({
                    "method": "thread/tokenUsage/updated",
                    "params": { "tokenUsage": { "totalTokens": tokens } }
                }),
                serde_json::json!({ "method": "turn/completed", "params": {} }),
            ];
            for message in messages {
                let _ = writeln!(stdout, "{message}");
            }
            let _ = stdout.flush();
            return;
        }
    }
}

fn main() {
    if std::env::var("THOR_FAKE_MODE").as_deref() == Ok("codex_rpc") {
        codex_rpc_mode();
        return;
    }

    let text = std::env::var("THOR_FAKE_TEXT").unwrap_or_default();
    let sleep_ms = env_u64("THOR_FAKE_SLEEP_MS", 0);
    let exit_code: i32 = std::env::var("THOR_FAKE_EXIT_CODE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    if sleep_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    }
    print!("{text}");
    std::process::exit(exit_code);
}

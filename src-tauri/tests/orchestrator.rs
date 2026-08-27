//! Drives the delegation core through the same MCP entry point Claude Code uses.
//!
//! The core is compiled directly rather than linked from `thor_lib`: a Rust test binary carries
//! no application manifest, so linking the GUI stack makes it fail to start on Windows.
//!
//! Worker tests spawn real Codex processes and are ignored by default:
//! `cargo test --test orchestrator -- --ignored --test-threads=1`

#[path = "../src/orchestrator_core.rs"]
mod orchestrator_core;

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use orchestrator_core::{handle_mcp_body, Core, Launcher, AGENT_CODEX};

fn rpc(core: &Core, id: u32, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let raw = handle_mcp_body(core, &body.to_string()).expect("a response");
    serde_json::from_str(&raw).expect("valid json")
}

fn call(core: &Core, name: &str, arguments: Value) -> Value {
    let response = rpc(
        core,
        10,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    );
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool text")
        .to_string();
    if response["result"]["isError"] == json!(true) {
        return json!({ "error": text });
    }
    serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text }))
}

fn codex_launcher() -> Launcher {
    let output = Command::new("where")
        .arg("codex")
        .output()
        .expect("where codex");
    let found = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| line.to_ascii_lowercase().ends_with(".cmd"))
        .map(ToOwned::to_owned)
        .expect("codex on PATH");
    Launcher::codex_app_server(PathBuf::from(found))
}

fn workspace(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("alethe-orch-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("workspace");
    dir
}

/// Deterministic stand-in for a real agent CLI (`src/bin/orchestrator_fake_worker.rs`), driven
/// entirely by env vars — no real Codex/OpenCode install, no shell, so it behaves identically on
/// every CI platform. `env` is `(key, value)` pairs applied on top of a plain one-shot launcher.
fn fake_launcher(label: &str, env: &[(&str, &str)]) -> Launcher {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_orchestrator_fake_worker"));
    let mut launcher = Launcher::one_shot(label.to_string(), path, Vec::new(), None, None);
    launcher
        .env
        .extend(env.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    launcher
}

fn fake_codex_launcher(tokens: u64) -> Launcher {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_orchestrator_fake_worker"));
    let mut launcher = Launcher::codex_app_server(path);
    launcher
        .env
        .push(("THOR_FAKE_MODE".into(), "codex_rpc".into()));
    launcher
        .env
        .push(("THOR_FAKE_TOKENS".into(), tokens.to_string()));
    launcher
}

struct PeakWatcher {
    peak: Arc<Mutex<usize>>,
    stop: Arc<Mutex<bool>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl PeakWatcher {
    fn start(core: Core) -> Self {
        let peak = Arc::new(Mutex::new(0usize));
        let stop = Arc::new(Mutex::new(false));
        let sampled = Arc::clone(&peak);
        let stopped = Arc::clone(&stop);
        let handle = thread::spawn(move || loop {
            let (running, _) = core.counts();
            {
                let mut peak = sampled.lock().expect("peak");
                *peak = (*peak).max(running);
            }
            if *stopped.lock().expect("stop") {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        });
        Self {
            peak,
            stop,
            handle: Some(handle),
        }
    }

    fn finish(mut self) -> usize {
        *self.stop.lock().expect("stop") = true;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let peak = *self.peak.lock().expect("peak");
        peak
    }
}

#[test]
fn the_handshake_advertises_every_tool() {
    let core = Core::default();
    let initialized = rpc(&core, 1, "initialize", json!({}));
    assert_eq!(initialized["result"]["serverInfo"]["name"], json!("thor"));

    let listed = rpc(&core, 2, "tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();

    for expected in [
        "thor_delegate",
        "thor_check",
        "thor_status",
        "thor_steer",
        "thor_send",
        "thor_cancel",
        "thor_release",
        "thor_diff",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn a_notification_gets_no_response_body() {
    let core = Core::default();
    let body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert!(handle_mcp_body(&core, body).is_none());
}

#[test]
fn delegating_nothing_is_an_error() {
    let core = Core::default();
    let result = call(&core, "thor_delegate", json!({ "tasks": [] }));
    assert!(
        result["error"]
            .as_str()
            .unwrap_or_default()
            .contains("at least one"),
        "{result}"
    );
}

#[test]
fn steering_an_unknown_job_is_refused() {
    let core = Core::default();
    let result = call(
        &core,
        "thor_steer",
        json!({ "jobId": "job-99", "message": "turn left" }),
    );
    assert!(
        result["error"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown job"),
        "{result}"
    );
}

#[test]
fn checking_with_no_work_returns_at_once() {
    let core = Core::default();
    let result = call(&core, "thor_check", json!({ "wait": true }));
    assert_eq!(result["workersStillBusy"], json!(0), "{result}");
    assert_eq!(
        result["deliveries"].as_array().expect("deliveries").len(),
        0
    );
}

#[test]
fn a_job_fails_cleanly_when_no_launcher_is_configured() {
    let core = Core::default();
    let dir = workspace("nolauncher");
    let delegated = call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "tasks": ["anything"] }),
    );
    assert_eq!(delegated["accepted"], json!(1), "{delegated}");

    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 5000 }),
    );
    let deliveries = checked["deliveries"].as_array().expect("deliveries");
    assert_eq!(deliveries.len(), 1, "{checked}");
    assert_eq!(deliveries[0]["outcome"], json!("failed"));
    assert!(
        deliveries[0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("bucket"),
        "{checked}"
    );
    assert_eq!(checked["workersStillBusy"], json!(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_observer_sees_every_state_change() {
    let core = Core::default();
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorder = Arc::clone(&seen);
    core.set_observer(Arc::new(move |snapshot| {
        recorder.lock().expect("seen").push(snapshot);
    }));

    let dir = workspace("observer");
    call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "tasks": ["anything"] }),
    );

    let snapshots = seen.lock().expect("seen");
    assert!(!snapshots.is_empty(), "the observer was never called");
    let last = snapshots.last().expect("a snapshot");
    assert!(last["jobs"].as_array().is_some_and(|jobs| !jobs.is_empty()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bucket_a_looks_exhausted_and_fails_over_to_bucket_b() {
    let core = Core::default();
    let mut a = fake_launcher(
        "a",
        &[
            ("THOR_FAKE_EXIT_CODE", "1"),
            ("THOR_FAKE_TEXT", "429 rate limit"),
        ],
    );
    a.fallback = Some("b".to_string());
    core.set_launcher("a", a);
    core.set_launcher("b", fake_launcher("b", &[("THOR_FAKE_TEXT", "ok")]));

    let dir = workspace("failover");
    let delegated = call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "bucket": "a", "tasks": ["anything"] }),
    );
    assert_eq!(delegated["accepted"], json!(1), "{delegated}");

    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 10000 }),
    );
    assert_eq!(checked["workersStillBusy"], json!(0), "{checked}");
    let deliveries = checked["deliveries"].as_array().expect("deliveries");
    assert!(
        deliveries.iter().any(|d| d["type"] == json!("failover")),
        "missing a failover delivery: {checked}"
    );
    assert!(
        deliveries
            .iter()
            .any(|d| d["type"] == json!("worker_done") && d["outcome"] == json!("succeeded")),
        "missing the final worker_done: {checked}"
    );

    let status = call(&core, "thor_status", json!({}));
    let jobs = status["jobs"].as_array().expect("jobs");
    assert_eq!(
        jobs[0]["bucket"],
        json!("b"),
        "job stayed on the exhausted bucket: {status}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failure_with_no_fallback_configured_just_fails() {
    let core = Core::default();
    core.set_launcher(
        "a",
        fake_launcher(
            "a",
            &[
                ("THOR_FAKE_EXIT_CODE", "1"),
                ("THOR_FAKE_TEXT", "429 rate limit"),
            ],
        ),
    );

    let dir = workspace("nofallback");
    call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "bucket": "a", "tasks": ["anything"] }),
    );
    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 10000 }),
    );
    let deliveries = checked["deliveries"].as_array().expect("deliveries");
    assert_eq!(deliveries.len(), 1, "{checked}");
    assert_eq!(deliveries[0]["type"], json!("worker_done"));
    assert_eq!(deliveries[0]["outcome"], json!("failed"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_generic_failure_does_not_trigger_failover() {
    let core = Core::default();
    let mut a = fake_launcher(
        "a",
        &[
            ("THOR_FAKE_EXIT_CODE", "1"),
            ("THOR_FAKE_TEXT", "file not found"),
        ],
    );
    a.fallback = Some("b".to_string());
    core.set_launcher("a", a);
    core.set_launcher("b", fake_launcher("b", &[("THOR_FAKE_TEXT", "ok")]));

    let dir = workspace("nofailover");
    call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "bucket": "a", "tasks": ["anything"] }),
    );
    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 10000 }),
    );
    let deliveries = checked["deliveries"].as_array().expect("deliveries");
    assert_eq!(deliveries.len(), 1, "{checked}");
    assert_eq!(deliveries[0]["type"], json!("worker_done"));
    assert_eq!(deliveries[0]["outcome"], json!("failed"));

    let status = call(&core, "thor_status", json!({}));
    let jobs = status["jobs"].as_array().expect("jobs");
    assert_eq!(
        jobs[0]["bucket"],
        json!("a"),
        "bucket changed without a quota signal: {status}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hung_worker_is_killed_after_its_timeout() {
    let core = Core::default();
    core.test_set_job_timeout_ms(300);
    core.set_launcher(
        "slow",
        fake_launcher("slow", &[("THOR_FAKE_SLEEP_MS", "5000")]),
    );

    let dir = workspace("timeout");
    call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "bucket": "slow", "tasks": ["anything"] }),
    );
    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 10000 }),
    );
    assert_eq!(checked["workersStillBusy"], json!(0), "{checked}");
    let deliveries = checked["deliveries"].as_array().expect("deliveries");
    assert_eq!(deliveries.len(), 1, "{checked}");
    assert_eq!(deliveries[0]["outcome"], json!("timeout"));
    assert!(
        deliveries[0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("exceeded"),
        "{checked}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_token_budget_blocks_further_delegation_once_reached() {
    let core = Core::default();
    core.set_launcher(AGENT_CODEX, fake_codex_launcher(500));
    core.set_token_budget(Some(100));

    let dir = workspace("budget");
    let delegated = call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "tasks": ["anything"] }),
    );
    assert_eq!(delegated["accepted"], json!(1), "{delegated}");

    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 10000 }),
    );
    assert_eq!(checked["workersStillBusy"], json!(0), "{checked}");

    let status = call(&core, "thor_status", json!({}));
    assert_eq!(status["tokensUsed"], json!(500), "{status}");

    let blocked = call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "tasks": ["more work"] }),
    );
    assert!(
        blocked["error"]
            .as_str()
            .unwrap_or_default()
            .contains("token budget"),
        "{blocked}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "spawns real codex workers"]
fn two_workers_overlap_and_check_waits_for_both() {
    let core = Core::default();
    core.set_launcher(AGENT_CODEX, codex_launcher());
    let dir = workspace("parallel");
    let watcher = PeakWatcher::start(core.clone());

    let delegated = call(
        &core,
        "thor_delegate",
        json!({
            "cwd": dir.to_string_lossy(),
            "tasks": [
                "Create a file ALPHA.txt whose entire content is the word ALPHA.",
                "Create a file BETA.txt whose entire content is the word BETA."
            ]
        }),
    );
    assert_eq!(delegated["accepted"], json!(2), "{delegated}");

    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 540000 }),
    );
    let peak = watcher.finish();

    assert_eq!(
        checked["workersStillBusy"],
        json!(0),
        "untilAllSettled returned early: {checked}"
    );
    assert_eq!(
        checked["deliveries"].as_array().expect("deliveries").len(),
        2,
        "both workers must land in one call: {checked}"
    );
    assert_eq!(peak, 2, "the workers never overlapped");
    assert!(
        dir.join("ALPHA.txt").exists(),
        "ALPHA.txt missing: {checked}"
    );
    assert!(dir.join("BETA.txt").exists(), "BETA.txt missing: {checked}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "spawns real codex workers"]
fn the_queue_never_breaches_the_concurrency_limit() {
    let core = Core::default();
    core.set_launcher(AGENT_CODEX, codex_launcher());
    core.set_concurrency_limit(2);
    let dir = workspace("queue");
    let watcher = PeakWatcher::start(core.clone());

    let tasks: Vec<String> = (1..=4)
        .map(|index| {
            format!("Create a file Q{index}.txt whose entire content is the number {index}.")
        })
        .collect();
    let delegated = call(
        &core,
        "thor_delegate",
        json!({ "cwd": dir.to_string_lossy(), "tasks": tasks }),
    );
    assert_eq!(delegated["accepted"], json!(4), "{delegated}");

    let (running, queued) = core.counts();
    assert!(running <= 2, "started {running} workers over the limit");
    assert_eq!(queued, 2, "the remainder must queue");

    let checked = call(
        &core,
        "thor_check",
        json!({ "wait": true, "timeoutMs": 600000 }),
    );
    let peak = watcher.finish();

    assert_eq!(peak, 2, "the limit was breached, peak was {peak}");
    assert_eq!(
        checked["deliveries"].as_array().expect("deliveries").len(),
        4,
        "every queued job must drain: {checked}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

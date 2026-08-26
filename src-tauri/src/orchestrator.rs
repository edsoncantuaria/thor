//! Tauri glue for the delegation core in `orchestrator_core`.
//!
//! The MCP server is hosted in-process over HTTP on the `agent_events` listener, so worker state
//! lives next to the UI instead of in a sidecar.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::cli_resolver;
use crate::orchestrator_core::{Core, Launcher, AGENT_CODEX, AGENT_OPENCODE};

const JOBS_EVENT: &str = "orchestrator://jobs";

#[derive(Default)]
pub struct OrchestratorState {
    core: Core,
    prepared: AtomicBool,
}

impl OrchestratorState {
    pub fn core(&self) -> &Core {
        &self.core
    }
}

/// Resolving launchers lazily keeps a missing CLI install from blocking app start; the failure
/// then surfaces as a delivery on the job that needed it (or, for `thor_delegate`, as an
/// immediate tool error). Resolving them scans PATH, so it happens once rather than on every
/// request. Codex and OpenCode are independent: either, both, or neither may be installed.
fn prepare(app: &AppHandle, state: &OrchestratorState) {
    if state.prepared.swap(true, Ordering::SeqCst) {
        return;
    }
    let core = state.core.clone();
    let handle = app.clone();
    core.set_observer(Arc::new(move |snapshot: Value| {
        let _ = handle.emit(JOBS_EVENT, snapshot);
    }));

    if let Some(program) = cli_resolver::find_windows_cli_launcher("codex") {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut launcher = Launcher::codex_app_server(PathBuf::from(program));
        #[cfg(windows)]
        launcher
            .env
            .push(("Path".to_string(), cli_resolver::rebuilt_path()));
        core.set_launcher(AGENT_CODEX, launcher);
    }

    if let Some(program) = cli_resolver::find_windows_cli_launcher("opencode") {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut launcher = Launcher::opencode_run(PathBuf::from(program));
        #[cfg(windows)]
        launcher
            .env
            .push(("Path".to_string(), cli_resolver::rebuilt_path()));
        core.set_launcher(AGENT_OPENCODE, launcher);
    }
}

pub fn handle_mcp_body(app: Option<&AppHandle>, state: &OrchestratorState, body: &str) -> Option<String> {
    if let Some(app) = app {
        prepare(app, state);
    }
    crate::orchestrator_core::handle_mcp_body(&state.core, body)
}

#[tauri::command]
pub fn orchestrator_mcp_config_path(app: AppHandle) -> Result<String, String> {
    prepare(&app, &app.state::<OrchestratorState>());
    let endpoint = crate::agent_events::agent_hooks_endpoint()?;
    let token = crate::agent_events::agent_hooks_token();
    let config = json!({
        "mcpServers": {
            "alethe": {
                "type": "http",
                "url": format!("{endpoint}/mcp"),
                "headers": { "X-Thor-Token": token }
            }
        }
    });
    let path = std::env::temp_dir().join("thor-orchestrator-mcp.json");
    let body = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;
    std::fs::write(&path, body).map_err(|error| format!("write_failed:{error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn orchestrator_jobs(state: tauri::State<'_, OrchestratorState>) -> Value {
    state.core.snapshot()
}

#[tauri::command]
pub fn orchestrator_set_concurrency(state: tauri::State<'_, OrchestratorState>, limit: usize) {
    state.core.set_concurrency_limit(limit);
}

/// A user-configured worker bucket, as saved in Preferences → Orchestrator. `protocol` is
/// `"appServer"` (Codex's persistent JSON-RPC mode) or `"oneShot"` (everything else: OpenCode,
/// Claude in print mode, Cursor's headless mode, ...). `command` is resolved on PATH the same way
/// every other agent CLI in Thor is, so a bucket that isn't installed yet just reports as such
/// instead of blocking the others.
fn parse_bucket(entry: &Value) -> Result<(String, Launcher, Value), String> {
    let id = entry
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("bucket id is required")?
        .to_string();
    let label = entry
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&id)
        .to_string();
    let command = entry
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("bucket \"{id}\" has no command"))?;
    let protocol = entry.get("protocol").and_then(Value::as_str).unwrap_or("oneShot");
    let default_model = entry
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let resolved = cli_resolver::find_windows_cli_launcher(command);
    // Kept unresolved rather than rejected outright: the bucket is still saved, and a job that
    // picks it fails with a clear "spawn failed" delivery instead of the whole config vanishing.
    let program = resolved.clone().unwrap_or_else(|| PathBuf::from(command));

    let mut launcher = if protocol == "appServer" {
        Launcher::codex_app_server(program)
    } else {
        let args = entry
            .get("args")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).map(ToOwned::to_owned).collect())
            .unwrap_or_default();
        let model_flag = entry
            .get("modelFlag")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| Some("--model".to_string()));
        Launcher::one_shot(label.clone(), program, args, model_flag, default_model.clone())
    };
    launcher.label = label;
    launcher.default_model = default_model;
    launcher.fallback = entry
        .get("fallback")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != id)
        .map(ToOwned::to_owned);

    let status = json!({
        "id": id,
        "resolved": resolved.is_some(),
        "path": resolved.map(|path| path.to_string_lossy().into_owned()),
    });
    Ok((id, launcher, status))
}

/// Replaces every user-configured bucket wholesale (Preferences saves the whole list each time).
/// Returns one status entry per input bucket — `{id, resolved, path}` on success, `{error}` on a
/// malformed entry — so the settings UI can show live PATH resolution without a round trip.
#[tauri::command]
pub fn orchestrator_set_buckets(
    app: AppHandle,
    state: tauri::State<'_, OrchestratorState>,
    buckets: Vec<Value>,
) -> Vec<Value> {
    prepare(&app, &state);
    let mut parsed = Vec::new();
    let mut statuses = Vec::new();
    for entry in &buckets {
        match parse_bucket(entry) {
            Ok((id, launcher, status)) => {
                statuses.push(status);
                parsed.push((id, launcher));
            }
            Err(error) => statuses.push(json!({ "error": error })),
        }
    }
    state.core().set_user_buckets(parsed);
    statuses
}

#[tauri::command]
pub fn orchestrator_list_buckets(app: AppHandle, state: tauri::State<'_, OrchestratorState>) -> Value {
    prepare(&app, &state);
    state.core().list_buckets()
}

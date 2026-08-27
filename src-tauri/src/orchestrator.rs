//! Tauri glue for the delegation core in `orchestrator_core`.
//!
//! The MCP server is hosted in-process over HTTP on the `agent_events` listener, so worker state
//! lives next to the UI instead of in a sidecar.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::cli_resolver;
use crate::orchestrator_core::{Core, Launcher, AGENT_CODEX, AGENT_OPENCODE};
use crate::worktrees::{self, WorktreeMode};

const JOBS_EVENT: &str = "orchestrator://jobs";
const ISOLATED_CHECKPOINT_EVENT: &str = "orchestrator://isolated-checkpoint";

/// Returns the requested `cwd` when `body` is a `tools/call` request for `thor_delegate` with
/// `arguments.isolate == true` and a non-empty `arguments.cwd`. `None` for anything else
/// (wrong method/tool, missing/false `isolate`, missing `cwd`, or unparseable JSON) — the caller
/// treats `None` as "forward unmodified, no isolation requested".
fn delegate_isolate_request(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    if value.get("method")?.as_str()? != "tools/call" {
        return None;
    }
    let params = value.get("params")?;
    if params.get("name")?.as_str()? != "thor_delegate" {
        return None;
    }
    let arguments = params.get("arguments")?;
    if !arguments.get("isolate")?.as_bool()? {
        return None;
    }
    let cwd = arguments.get("cwd")?.as_str()?;
    if cwd.is_empty() {
        return None;
    }
    Some(cwd.to_string())
}

/// Rewrites `arguments.cwd` in a `body` JSON-RPC message to `path`, leaving every other field
/// (including `isolate`) untouched. Falls back to returning `body` unchanged if it doesn't parse
/// as JSON — defensive only, since callers only reach here after `delegate_isolate_request`
/// already validated the shape.
fn with_rewritten_cwd(body: &str, path: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(body) else {
        return body.to_string();
    };
    if let Some(arguments) = value
        .get_mut("params")
        .and_then(|params| params.get_mut("arguments"))
        .and_then(Value::as_object_mut)
    {
        arguments.insert("cwd".to_string(), Value::String(path.to_string()));
    }
    value.to_string()
}

/// Pulls the delegated job ids out of a `thor_delegate` response's MCP envelope. The tool result
/// text is itself a JSON string (double-encoded); returns an empty `Vec` on any parse failure at
/// either layer, or on an error response.
fn extract_delegated_job_ids(response: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(response) else {
        return Vec::new();
    };
    let Some(text) = value
        .get("result")
        .and_then(|result| result.get("content"))
        .and_then(|content| content.get(0))
        .and_then(|first| first.get("text"))
        .and_then(Value::as_str)
    else {
        return Vec::new();
    };
    let Ok(inner) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Some(jobs) = inner.get("jobs").and_then(Value::as_array) else {
        return Vec::new();
    };
    jobs.iter()
        .filter_map(|job| job.get("id").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

/// Pulls `params.arguments.jobIds` out of a `thor_release` request. Empty `Vec` if anything fails
/// to parse; the tool-name check itself is left to the caller.
fn release_job_ids(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(job_ids) = value
        .get("params")
        .and_then(|params| params.get("arguments"))
        .and_then(|arguments| arguments.get("jobIds"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    job_ids
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

/// The `params.name` of a `tools/call` request, or `None` for anything else (wrong method,
/// unparseable JSON, missing fields).
fn tools_call_name(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    if value.get("method")?.as_str()? != "tools/call" {
        return None;
    }
    value
        .get("params")?
        .get("name")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// The `params.arguments.tasks` list of a `thor_delegate` request, in order — used to pair each
/// minted job id back to the instruction that produced it (the core creates jobs in the same
/// order `tasks` was given).
fn delegate_tasks(body: &str) -> Vec<String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            let tasks = value
                .get("params")?
                .get("arguments")?
                .get("tasks")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect();
            Some(tasks)
        })
        .unwrap_or_default()
}

/// A `tools/call` error response matching `orchestrator_core::handle_mcp_body`'s own error
/// shape, echoing the request's `id` back so the caller can still correlate it.
fn tool_error_response(body: &str, message: &str) -> String {
    let id = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("id").cloned())
        .unwrap_or(Value::Null);
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": format!("error: {message}") }],
            "isError": true
        }
    })
    .to_string()
}

/// One `thor_delegate` call's worktree, tracked from acceptance until `thor_release`. All jobs
/// minted by the same isolated call share one entry's worth of worktree info (they already share
/// bucket/model by the tool's own contract, so they share isolation too).
struct IsolatedJob {
    repo: String,
    worktree_agent_id: String,
    path: String,
    branch: String,
    spec: String,
}

#[derive(Default)]
pub struct OrchestratorState {
    core: Core,
    prepared: AtomicBool,
    isolated: Mutex<HashMap<String, IsolatedJob>>,
}

impl OrchestratorState {
    pub fn core(&self) -> &Core {
        &self.core
    }

    fn isolated(&self) -> MutexGuard<'_, HashMap<String, IsolatedJob>> {
        match self.isolated.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
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

pub fn handle_mcp_body(
    app: Option<&AppHandle>,
    state: &OrchestratorState,
    body: &str,
) -> Option<String> {
    if let Some(app) = app {
        prepare(app, state);
    }

    if let Some(cwd) = delegate_isolate_request(body) {
        return Some(handle_isolated_delegate(app, state, body, cwd));
    }

    if tools_call_name(body).as_deref() == Some("thor_release") {
        let job_ids = release_job_ids(body);
        if !job_ids.is_empty() {
            handle_isolated_release(app, state, &job_ids);
        }
    }

    crate::orchestrator_core::handle_mcp_body(&state.core, body)
}

/// Provisions a fresh, isolated git worktree for one `thor_delegate` call, rewrites the request's
/// `cwd` to point at it, forwards to the real delegation core, and tracks the returned job ids
/// against that worktree so `thor_release` can checkpoint-commit their work later. On a
/// provisioning failure (e.g. `cwd` isn't inside a git repository), returns a tool error instead
/// of silently falling back to un-isolated — isolation was explicitly requested.
fn handle_isolated_delegate(
    app: Option<&AppHandle>,
    state: &OrchestratorState,
    body: &str,
    cwd: String,
) -> String {
    let worktree_agent_id = format!("thor-delegate-{}", nanoid::nanoid!(8));
    let info = match worktrees::worktree_provision_inner(
        cwd.clone(),
        worktree_agent_id.clone(),
        WorktreeMode::GitWorktree,
    ) {
        Ok(info) => info,
        Err(error) => {
            return tool_error_response(
                body,
                &format!("isolate:true requires cwd to be inside a git repository: {error}"),
            );
        }
    };

    let rewritten = with_rewritten_cwd(body, &info.path);
    if let Some(app) = app {
        prepare(app, state);
    }
    let Some(response) = crate::orchestrator_core::handle_mcp_body(&state.core, &rewritten) else {
        return tool_error_response(body, "delegation produced no response");
    };

    let job_ids = extract_delegated_job_ids(&response);
    if !job_ids.is_empty() {
        let tasks = delegate_tasks(body);
        let mut isolated = state.isolated();
        for (index, job_id) in job_ids.into_iter().enumerate() {
            isolated.insert(
                job_id,
                IsolatedJob {
                    repo: cwd.clone(),
                    worktree_agent_id: worktree_agent_id.clone(),
                    path: info.path.clone(),
                    branch: info.branch.clone(),
                    spec: tasks.get(index).cloned().unwrap_or_default(),
                },
            );
        }
    }

    response
}

/// For every released job that was isolated: commits whatever it left pending in its worktree
/// (the checkpoint) and, if anything actually landed, emits a summary event the frontend turns
/// into a per-project Todo for human review. The worktree itself is left in place — reachable
/// through Thor's normal worktree list for manual follow-up — not torn down here.
fn handle_isolated_release(app: Option<&AppHandle>, state: &OrchestratorState, job_ids: &[String]) {
    for job_id in job_ids {
        let job = {
            let mut isolated = state.isolated();
            isolated.remove(job_id)
        };
        let Some(job) = job else { continue };

        let committed = worktrees::worktree_commit_pending_inner(
            job.repo.clone(),
            job.worktree_agent_id.clone(),
        )
        .unwrap_or(false);
        if !committed {
            continue;
        }

        let diff_summary = crate::git_control::checked_output(
            std::path::Path::new(&job.path),
            &["diff", "--stat", "HEAD~1..HEAD"],
        )
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .chars()
                .take(2000)
                .collect::<String>()
        })
        .unwrap_or_default();

        if let Some(app) = app {
            let _ = app.emit(
                ISOLATED_CHECKPOINT_EVENT,
                json!({
                    "jobId": job_id,
                    "cwd": job.repo,
                    "branch": job.branch,
                    "path": job.path,
                    "spec": job.spec,
                    "diffSummary": diff_summary,
                }),
            );
        }
    }
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
            },
            "thor": {
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

#[tauri::command]
pub fn orchestrator_set_job_timeout_secs(state: tauri::State<'_, OrchestratorState>, secs: u64) {
    state.core.set_job_timeout_secs(secs);
}

/// `budget` of `None` clears the cap (unlimited). Purely a spend backstop for the Codex
/// app-server protocol, the only bucket kind that reports token usage at all — one-shot buckets
/// (OpenCode, Claude in print mode, ...) stay unmetered and never trip it.
#[tauri::command]
pub fn orchestrator_set_token_budget(
    state: tauri::State<'_, OrchestratorState>,
    budget: Option<u64>,
) {
    state.core.set_token_budget(budget);
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
    let protocol = entry
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("oneShot");
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
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let model_flag = entry
            .get("modelFlag")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| Some("--model".to_string()));
        Launcher::one_shot(
            label.clone(),
            program,
            args,
            model_flag,
            default_model.clone(),
        )
    };
    launcher.label = label;
    launcher.default_model = default_model;
    launcher.fallback = entry
        .get("fallback")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != id)
        .map(ToOwned::to_owned);
    // The native relay hook: e.g. OPENAI_BASE_URL/OPENAI_API_KEY to point a CLI at an
    // OpenAI-compatible proxy instead of the vendor's own endpoint. One KEY=VALUE per line,
    // "#"-prefixed lines ignored, same convention as a .env file.
    launcher.env.extend(
        entry
            .get("env")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let (key, value) = line.split_once('=')?;
                let key = key.trim();
                if key.is_empty() {
                    return None;
                }
                Some((key.to_string(), value.trim().to_string()))
            }),
    );

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
pub fn orchestrator_list_buckets(
    app: AppHandle,
    state: tauri::State<'_, OrchestratorState>,
) -> Value {
    prepare(&app, &state);
    state.core().list_buckets()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegate_isolate_request_extracts_cwd_when_isolate_is_true() {
        let body = json!({
            "method": "tools/call",
            "params": {
                "name": "thor_delegate",
                "arguments": { "tasks": ["do it"], "cwd": "/repo", "isolate": true }
            }
        })
        .to_string();
        assert_eq!(delegate_isolate_request(&body), Some("/repo".to_string()));
    }

    #[test]
    fn delegate_isolate_request_is_none_when_isolate_false_or_absent() {
        let without_flag = json!({
            "method": "tools/call",
            "params": { "name": "thor_delegate", "arguments": { "tasks": ["x"], "cwd": "/repo" } }
        })
        .to_string();
        assert_eq!(delegate_isolate_request(&without_flag), None);

        let flag_false = json!({
            "method": "tools/call",
            "params": {
                "name": "thor_delegate",
                "arguments": { "tasks": ["x"], "cwd": "/repo", "isolate": false }
            }
        })
        .to_string();
        assert_eq!(delegate_isolate_request(&flag_false), None);
    }

    #[test]
    fn delegate_isolate_request_is_none_for_other_tools() {
        let body = json!({
            "method": "tools/call",
            "params": { "name": "thor_status", "arguments": { "isolate": true, "cwd": "/repo" } }
        })
        .to_string();
        assert_eq!(delegate_isolate_request(&body), None);
    }

    #[test]
    fn delegate_isolate_request_is_none_when_cwd_missing() {
        let body = json!({
            "method": "tools/call",
            "params": {
                "name": "thor_delegate",
                "arguments": { "tasks": ["x"], "isolate": true }
            }
        })
        .to_string();
        assert_eq!(delegate_isolate_request(&body), None);
    }

    #[test]
    fn delegate_isolate_request_is_none_for_garbage_input() {
        assert_eq!(delegate_isolate_request("not json at all"), None);
        assert_eq!(delegate_isolate_request(""), None);
    }

    #[test]
    fn with_rewritten_cwd_changes_only_cwd() {
        let body = json!({
            "method": "tools/call",
            "params": {
                "name": "thor_delegate",
                "arguments": { "tasks": ["a", "b"], "cwd": "/repo", "isolate": true }
            }
        })
        .to_string();

        let rewritten = with_rewritten_cwd(&body, "/tmp/worktree-123");
        let parsed: Value = serde_json::from_str(&rewritten).expect("valid json");

        assert_eq!(
            parsed["params"]["arguments"]["cwd"],
            json!("/tmp/worktree-123")
        );
        assert_eq!(parsed["params"]["arguments"]["tasks"], json!(["a", "b"]));
        assert_eq!(parsed["params"]["arguments"]["isolate"], json!(true));
    }

    #[test]
    fn with_rewritten_cwd_returns_original_on_unparseable_body() {
        assert_eq!(with_rewritten_cwd("not json", "/tmp/x"), "not json");
    }

    #[test]
    fn extract_delegated_job_ids_parses_well_formed_response() {
        let inner = json!({
            "accepted": 2,
            "jobs": [
                { "id": "job-01", "spec": "task one" },
                { "id": "job-02", "spec": "task two" }
            ]
        })
        .to_string();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": [{ "type": "text", "text": inner }] }
        })
        .to_string();

        assert_eq!(
            extract_delegated_job_ids(&response),
            vec!["job-01".to_string(), "job-02".to_string()]
        );
    }

    #[test]
    fn extract_delegated_job_ids_is_empty_for_malformed_response() {
        assert_eq!(extract_delegated_job_ids("not json"), Vec::<String>::new());

        let missing_text = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": [{ "type": "text" }] }
        })
        .to_string();
        assert_eq!(
            extract_delegated_job_ids(&missing_text),
            Vec::<String>::new()
        );

        let inner_not_json = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": [{ "type": "text", "text": "not valid json" }] }
        })
        .to_string();
        assert_eq!(
            extract_delegated_job_ids(&inner_not_json),
            Vec::<String>::new()
        );
    }

    #[test]
    fn extract_delegated_job_ids_is_empty_for_error_response() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": [{ "type": "text", "text": "spawn failed" }], "isError": true }
        })
        .to_string();
        assert_eq!(extract_delegated_job_ids(&response), Vec::<String>::new());
    }

    #[test]
    fn release_job_ids_parses_well_formed_request() {
        let body = json!({
            "method": "tools/call",
            "params": {
                "name": "thor_release",
                "arguments": { "jobIds": ["job-01", "job-02"] }
            }
        })
        .to_string();
        assert_eq!(
            release_job_ids(&body),
            vec!["job-01".to_string(), "job-02".to_string()]
        );
    }

    #[test]
    fn release_job_ids_is_empty_for_malformed_input() {
        assert_eq!(release_job_ids("not json"), Vec::<String>::new());

        let no_arguments = json!({
            "method": "tools/call",
            "params": { "name": "thor_release" }
        })
        .to_string();
        assert_eq!(release_job_ids(&no_arguments), Vec::<String>::new());

        let job_ids_not_array = json!({
            "method": "tools/call",
            "params": { "name": "thor_release", "arguments": { "jobIds": "job-01" } }
        })
        .to_string();
        assert_eq!(release_job_ids(&job_ids_not_array), Vec::<String>::new());
    }
}

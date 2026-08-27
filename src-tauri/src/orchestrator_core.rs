//! Delegation core: queue, workers and the MCP tool surface.
//!
//! Deliberately free of Tauri and of anything else in this crate. The app layer supplies a
//! launcher and an optional observer; everything else here is plain `std` + `serde_json`, which
//! is what lets `tests/orchestrator.rs` compile this file directly instead of linking the GUI
//! stack a Rust test binary cannot load.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

const DEFAULT_MAX_CONCURRENT: usize = 4;
const MAX_WAIT_MS: u64 = 600_000;
const REPLY_LIMIT: usize = 16_000;
/// Generous enough for a real coding task, bounded enough that a hung worker doesn't sit on a
/// concurrency slot forever. Overridable via `Core::set_job_timeout_secs`, clamped 1 min – 2 h.
const DEFAULT_JOB_TIMEOUT_SECS: u64 = 1800;
/// How often the watchdog thread sweeps for jobs that have overrun their timeout.
const WATCHDOG_INTERVAL: Duration = Duration::from_millis(1000);

pub const STATUS_QUEUED: &str = "queued";
pub const STATUS_RUNNING: &str = "running";
pub const STATUS_DONE: &str = "done";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const STATUS_RELEASED: &str = "released";

pub type Observer = Arc<dyn Fn(Value) + Send + Sync>;

/// Bucket id Thor seeds automatically when Codex/OpenCode are found on PATH. Any other id is a
/// worker bucket the user configured in Preferences → Orchestrator — nothing beyond these two
/// defaults is baked in, so Claude, Cursor, a second OpenCode pointed at a local Ollama model, or
/// anything else that runs from a CLI is just another bucket the lead can pick by id.
pub const AGENT_CODEX: &str = "codex";
pub const AGENT_OPENCODE: &str = "opencode";

/// Codex speaks a persistent JSON-RPC app-server protocol over stdin/stdout, so one worker can be
/// steered or sent follow-up turns mid-thread. Every other CLI is treated as one-shot: run it with
/// the task as the final argument, capture stdout, done — that covers `opencode run`, `claude -p`,
/// `cursor-agent -p` and anything else without a persistent app-server mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LauncherKind {
    CodexAppServer,
    OneShotRun,
}

/// How to start one worker bucket. The core never guesses a binary, a protocol, or an invocation
/// shape — every field here comes from either the PATH-scan defaults or the user's own config.
#[derive(Clone, Debug)]
pub struct Launcher {
    pub kind: LauncherKind,
    /// Display name only (surfaced to the lead via `thor_status` so it can pick sensibly).
    pub label: String,
    pub program: PathBuf,
    /// Base args before the model flag / task text. Fixed to `["app-server","--stdio"]` for
    /// `CodexAppServer` (that protocol IS that invocation); user-defined for `OneShotRun`.
    pub args: Vec<String>,
    /// Flag used to pass a model, e.g. `--model`. `None` means this bucket has no model override
    /// (the CLI has nothing to configure, or the model is already baked into `args`/the CLI's own
    /// config).
    pub model_flag: Option<String>,
    /// Used when `thor_delegate` doesn't override the model for a task.
    pub default_model: Option<String>,
    /// Bucket id to retry the same task on, automatically, if this one fails with what looks
    /// like a quota/rate-limit error (see `looks_like_quota_exhaustion`). User-configured, so
    /// nothing fails over unless the user built that chain — e.g. a "claude-sonnet" bucket
    /// falling back to a "gemini-flash" one.
    pub fallback: Option<String>,
    pub env: Vec<(String, String)>,
}

impl Launcher {
    pub fn codex_app_server(program: PathBuf) -> Self {
        Self {
            kind: LauncherKind::CodexAppServer,
            label: "Codex".into(),
            program,
            args: vec!["app-server".into(), "--stdio".into()],
            model_flag: None,
            default_model: None,
            fallback: None,
            env: Vec::new(),
        }
    }

    /// `opencode run` — non-interactive, exits when the task is done.
    pub fn opencode_run(program: PathBuf) -> Self {
        Self {
            kind: LauncherKind::OneShotRun,
            label: "OpenCode".into(),
            program,
            args: vec!["run".into()],
            model_flag: Some("--model".into()),
            default_model: None,
            fallback: None,
            env: Vec::new(),
        }
    }

    /// A user-configured one-shot bucket: any CLI, any base args, any model flag. This is the
    /// generic path Claude, Cursor, or a second differently-configured OpenCode go through.
    pub fn one_shot(
        label: String,
        program: PathBuf,
        args: Vec<String>,
        model_flag: Option<String>,
        default_model: Option<String>,
    ) -> Self {
        Self {
            kind: LauncherKind::OneShotRun,
            label,
            program,
            args,
            model_flag,
            default_model,
            fallback: None,
            env: Vec::new(),
        }
    }
}

/// Deliberately a keyword sweep rather than parsing exit codes or JSON error shapes, since every
/// CLI reports "you're out of quota" differently. Case-insensitive substring match against
/// whatever text the failed job produced (stderr/stdout for one-shot, the turn's reply for
/// Codex). False negatives just mean no failover (safe); a false positive triggers one extra
/// retry on a bucket the user explicitly chose as this one's fallback (safe, cheap).
fn looks_like_quota_exhaustion(text: &str) -> bool {
    let lower = text.to_lowercase();
    const MARKERS: [&str; 22] = [
        "rate limit",
        "rate_limit",
        "ratelimit",
        "429",
        "quota",
        "usage limit",
        "usage_limit",
        "resource_exhausted",
        "resource exhausted",
        "too many requests",
        "insufficient_quota",
        "overloaded",
        // Broader, provider-agnostic net: HTTP statuses and phrasing other providers use for the
        // same "back off and retry elsewhere" condition, not just OpenAI/Anthropic's own wording.
        "402",
        "403",
        "500",
        "502",
        "503",
        "529",
        "capacity",
        "try again later",
        "exceeded your",
        "billing",
    ];
    MARKERS.iter().any(|marker| lower.contains(marker))
}

/// Best-effort: the exact key spelling Codex's `tokenUsage` payload uses isn't pinned down
/// against a live app-server response in this codebase, so this checks a few plausible
/// spellings rather than committing to one fixed schema. An unrecognized shape just reports 0
/// — safe (undercounts, never trips the budget on a shape it doesn't understand).
fn extract_total_tokens(usage: &Value) -> u64 {
    let as_u64 = |key: &str| usage.get(key).and_then(Value::as_u64);
    if let Some(total) = as_u64("totalTokens")
        .or_else(|| as_u64("total_tokens"))
        .or_else(|| as_u64("total"))
    {
        return total;
    }
    let input = as_u64("inputTokens")
        .or_else(|| as_u64("input_tokens"))
        .or_else(|| as_u64("input"))
        .unwrap_or(0);
    let output = as_u64("outputTokens")
        .or_else(|| as_u64("output_tokens"))
        .or_else(|| as_u64("output"))
        .unwrap_or(0);
    input + output
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or_default()
}

struct Job {
    id: String,
    spec: String,
    cwd: String,
    bucket: String,
    model: Option<String>,
    /// Every bucket already attempted for this task, including the current one once it fails.
    /// Bounds automatic failover to a finite chain and stops a fallback cycle from looping.
    bucket_chain_tried: Vec<String>,
    /// Which protocol the currently (or most recently) running worker actually used. `None` while
    /// still queued. Set once at spawn time so `thor_steer`/`thor_send` can give an accurate
    /// answer even if the bucket's config changes after the job started.
    protocol_hint: Option<LauncherKind>,
    status: String,
    thread_id: Option<String>,
    active_turn_id: Option<String>,
    reply: String,
    plan: Vec<String>,
    diff: Option<String>,
    tokens: Option<Value>,
    /// Running total tokens this job's current bucket attempt has burned, parsed via
    /// `extract_total_tokens` from Codex's `thread/tokenUsage/updated`. Reset to 0 on failover
    /// along with the job's other transient per-attempt state — a failed-over attempt's spend
    /// isn't retained in the total (accepted simplification, see plan notes).
    tokens_total: u64,
    outcome: Option<String>,
    started_at: Option<u64>,
    ended_at: Option<u64>,
    child: Option<Arc<Mutex<Child>>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    next_request_id: i64,
}

impl Job {
    fn snapshot(&self) -> Value {
        let elapsed = match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => Some(end.saturating_sub(start) as f64 / 1000.0),
            (Some(start), None) => Some(now_ms().saturating_sub(start) as f64 / 1000.0),
            _ => None,
        };
        json!({
            "id": self.id,
            "spec": self.spec,
            "cwd": self.cwd,
            "bucket": self.bucket,
            "model": self.model,
            "status": self.status,
            "threadId": self.thread_id,
            "outcome": self.outcome,
            "seconds": elapsed,
            "plan": self.plan,
            "tokens": self.tokens,
            "hasDiff": self.diff.is_some(),
            "summary": self.reply.trim().chars().take(1200).collect::<String>(),
        })
    }

    fn settled(&self) -> bool {
        matches!(
            self.status.as_str(),
            STATUS_DONE | STATUS_FAILED | STATUS_CANCELLED | STATUS_RELEASED
        )
    }

    fn teardown(&mut self) {
        if let Some(child) = self.child.take() {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }
        self.stdin = None;
    }
}

struct Delivery {
    seq: u64,
    kind: String,
    job_id: String,
    outcome: Option<String>,
    text: String,
}

impl Delivery {
    fn to_value(&self) -> Value {
        json!({
            "seq": self.seq,
            "type": self.kind,
            "jobId": self.job_id,
            "outcome": self.outcome,
            "text": self.text,
        })
    }
}

#[derive(Default)]
struct Inner {
    jobs: HashMap<String, Job>,
    order: Vec<String>,
    queue: VecDeque<String>,
    deliveries: VecDeque<Delivery>,
    seq: u64,
    running: usize,
    max_concurrent: usize,
    job_counter: u64,
    job_timeout_ms: u64,
    /// `None` = unlimited (default — opt-in, doesn't change existing behavior). When set,
    /// `thor_delegate`/`thor_send` refuse to start new work once `tokens_used() >= budget`.
    token_budget: Option<u64>,
}

impl Inner {
    /// Sum of every job's `tokens_total` — computed on demand rather than maintained as a
    /// separately-mutated running counter, so there's no delta-tracking to get wrong (a job's
    /// `tokens_total` is already the source of truth, updated in `on_worker_message`).
    fn tokens_used(&self) -> u64 {
        self.jobs.values().map(|job| job.tokens_total).sum()
    }

    fn check_token_budget(&self) -> Result<(), String> {
        if let Some(budget) = self.token_budget {
            let used = self.tokens_used();
            if used >= budget {
                return Err(format!(
                    "token budget reached ({used}/{budget} tokens used) — release settled jobs or raise the budget before delegating more work"
                ));
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Value {
        let jobs: Vec<Value> = self
            .order
            .iter()
            .filter_map(|id| self.jobs.get(id))
            .map(Job::snapshot)
            .collect();
        json!({
            "jobs": jobs,
            "running": self.running,
            "queued": self.queue.len(),
            "concurrencyLimit": self.max_concurrent,
            "tokensUsed": self.tokens_used(),
            "tokenBudget": self.token_budget
        })
    }

    fn push_delivery(&mut self, kind: &str, job_id: &str, outcome: Option<String>, text: String) {
        self.seq += 1;
        let seq = self.seq;
        self.deliveries.push_back(Delivery {
            seq,
            kind: kind.to_string(),
            job_id: job_id.to_string(),
            outcome,
            text,
        });
    }
}

#[derive(Clone)]
pub struct Core {
    inner: Arc<Mutex<Inner>>,
    signal: Arc<Condvar>,
    /// PATH-scan defaults (codex/opencode), set once by `orchestrator.rs::prepare`.
    auto_buckets: Arc<Mutex<HashMap<String, Launcher>>>,
    /// User-configured buckets from Preferences → Orchestrator. Replaced wholesale on every save,
    /// and checked before `auto_buckets`, so a user bucket named "codex" overrides the default.
    user_buckets: Arc<Mutex<HashMap<String, Launcher>>>,
    observer: Arc<Mutex<Option<Observer>>>,
}

impl Default for Core {
    fn default() -> Self {
        let core = Self {
            inner: Arc::new(Mutex::new(Inner {
                max_concurrent: DEFAULT_MAX_CONCURRENT,
                job_timeout_ms: DEFAULT_JOB_TIMEOUT_SECS * 1000,
                ..Inner::default()
            })),
            signal: Arc::new(Condvar::new()),
            auto_buckets: Arc::new(Mutex::new(HashMap::new())),
            user_buckets: Arc::new(Mutex::new(HashMap::new())),
            observer: Arc::new(Mutex::new(None)),
        };
        core.spawn_watchdog();
        core
    }
}

fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn send_rpc(stdin: &Arc<Mutex<ChildStdin>>, value: &Value) -> Result<(), String> {
    let mut stdin = guard(stdin);
    serde_json::to_writer(&mut *stdin, value).map_err(|error| error.to_string())?;
    stdin.write_all(b"\n").map_err(|error| error.to_string())?;
    stdin.flush().map_err(|error| error.to_string())
}

fn job_rpc(inner: &mut Inner, job_id: &str, method: &str, params: Value) -> Result<(), String> {
    let job = inner
        .jobs
        .get_mut(job_id)
        .ok_or_else(|| format!("unknown job {job_id}"))?;
    let stdin = job
        .stdin
        .clone()
        .ok_or_else(|| format!("job {job_id} has no live worker"))?;
    job.next_request_id += 1;
    let id = job.next_request_id;
    send_rpc(
        &stdin,
        &json!({ "id": id, "method": method, "params": params }),
    )
}

impl Core {
    /// Registers a PATH-scan default bucket (codex/opencode). Never overrides a user bucket of
    /// the same id — `resolve_launcher` always checks user buckets first.
    pub fn set_launcher(&self, id: &str, launcher: Launcher) {
        guard(&self.auto_buckets).insert(id.to_string(), launcher);
    }

    /// Replaces every user-configured bucket wholesale — called on app boot and whenever
    /// Preferences → Orchestrator is saved. Does not touch the PATH-scan defaults.
    pub fn set_user_buckets(&self, buckets: Vec<(String, Launcher)>) {
        let mut map = guard(&self.user_buckets);
        map.clear();
        for (id, launcher) in buckets {
            map.insert(id, launcher);
        }
    }

    fn resolve_launcher(&self, id: &str) -> Option<Launcher> {
        if let Some(launcher) = guard(&self.user_buckets).get(id) {
            return Some(launcher.clone());
        }
        guard(&self.auto_buckets).get(id).cloned()
    }

    /// Every configured bucket, for `thor_status` (so the lead can discover real options
    /// instead of guessing an id) and for the Preferences UI's live status list.
    pub fn list_buckets(&self) -> Value {
        let auto = guard(&self.auto_buckets);
        let user = guard(&self.user_buckets);
        let mut ids: Vec<&String> = auto.keys().chain(user.keys()).collect();
        ids.sort();
        ids.dedup();
        let buckets: Vec<Value> = ids
            .into_iter()
            .filter_map(|id| {
                let launcher = user.get(id).or_else(|| auto.get(id))?;
                Some(json!({
                    "id": id,
                    "label": launcher.label,
                    "protocol": match launcher.kind {
                        LauncherKind::CodexAppServer => "appServer",
                        LauncherKind::OneShotRun => "oneShot",
                    },
                    "defaultModel": launcher.default_model,
                    "fallback": launcher.fallback,
                    "custom": user.contains_key(id),
                }))
            })
            .collect();
        json!({ "buckets": buckets })
    }

    pub fn set_observer(&self, observer: Observer) {
        *guard(&self.observer) = Some(observer);
    }

    pub fn set_concurrency_limit(&self, limit: usize) {
        guard(&self.inner).max_concurrent = limit.clamp(1, 16);
    }

    pub fn set_job_timeout_secs(&self, secs: u64) {
        guard(&self.inner).job_timeout_ms = secs.clamp(60, 7200) * 1000;
    }

    /// Bypasses the production clamp so tests can assert a kill within a couple seconds instead
    /// of waiting out the real 60s floor.
    #[cfg(test)]
    pub fn test_set_job_timeout_ms(&self, ms: u64) {
        guard(&self.inner).job_timeout_ms = ms;
    }

    pub fn set_token_budget(&self, budget: Option<u64>) {
        guard(&self.inner).token_budget = budget;
    }

    fn spawn_watchdog(&self) {
        let watchdog = self.clone();
        thread::spawn(move || loop {
            thread::sleep(WATCHDOG_INTERVAL);
            watchdog.sweep_timeouts();
        });
    }

    /// Kills and settles any `running` job that has overrun `job_timeout_ms`. Reuses `finish`'s
    /// existing teardown/delivery/notify plumbing — a timeout's failure text never matches
    /// `looks_like_quota_exhaustion`, so this naturally never triggers failover, which is
    /// correct: a hang isn't a quota signal.
    fn sweep_timeouts(&self) {
        let timeout_ms = guard(&self.inner).job_timeout_ms;
        let now = now_ms();
        let expired: Vec<String> = {
            let inner = guard(&self.inner);
            inner
                .jobs
                .values()
                .filter(|job| job.status == STATUS_RUNNING)
                .filter_map(|job| job.started_at.map(|start| (job.id.clone(), start)))
                .filter(|(_, start)| now.saturating_sub(*start) > timeout_ms)
                .map(|(id, _)| id)
                .collect()
        };
        for job_id in expired {
            self.finish(
                &job_id,
                STATUS_FAILED,
                Some("timeout".into()),
                format!(
                    "worker exceeded {}s without completing — killed",
                    timeout_ms / 1000
                ),
                true,
            );
        }
    }

    pub fn snapshot(&self) -> Value {
        guard(&self.inner).snapshot()
    }

    /// Running and queued counts, for tests and for the UI.
    pub fn counts(&self) -> (usize, usize) {
        let inner = guard(&self.inner);
        (inner.running, inner.queue.len())
    }

    fn notify(&self, inner: &Inner) {
        let observer = guard(&self.observer).clone();
        if let Some(observer) = observer {
            observer(inner.snapshot());
        }
    }

    fn spawn_worker(&self, job_id: &str) {
        let (cwd, spec, bucket, model) = {
            let mut inner = guard(&self.inner);
            let Some(job) = inner.jobs.get_mut(job_id) else {
                return;
            };
            job.status = STATUS_RUNNING.to_string();
            job.started_at = Some(now_ms());
            let tuple = (
                job.cwd.clone(),
                job.spec.clone(),
                job.bucket.clone(),
                job.model.clone(),
            );
            inner.running += 1;
            tuple
        };

        let Some(launcher) = self.resolve_launcher(&bucket) else {
            self.settle(
                job_id,
                STATUS_FAILED,
                "failed",
                &format!(
                    "no worker bucket configured with id \"{bucket}\" — check thor_status or add it in Preferences → Orchestrator"
                ),
            );
            return;
        };

        {
            let mut inner = guard(&self.inner);
            if let Some(job) = inner.jobs.get_mut(job_id) {
                job.protocol_hint = Some(launcher.kind);
            }
        }

        let effective_model = model.or_else(|| launcher.default_model.clone());

        match launcher.kind {
            LauncherKind::CodexAppServer => {
                self.spawn_codex_worker(job_id, &cwd, &spec, effective_model.as_deref(), &launcher)
            }
            LauncherKind::OneShotRun => self.spawn_one_shot_worker(
                job_id,
                &cwd,
                &spec,
                effective_model.as_deref(),
                &launcher,
            ),
        }
    }

    fn spawn_codex_worker(
        &self,
        job_id: &str,
        cwd: &str,
        spec: &str,
        model: Option<&str>,
        launcher: &Launcher,
    ) {
        let mut command = Command::new(&launcher.program);
        command
            .args(&launcher.args)
            .current_dir(PathBuf::from(cwd))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in &launcher.env {
            command.env(key, value);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.settle(
                    job_id,
                    STATUS_FAILED,
                    "failed",
                    &format!("worker spawn failed: {error}"),
                );
                return;
            }
        };

        let stdin = match child.stdin.take() {
            Some(stdin) => Arc::new(Mutex::new(stdin)),
            None => {
                let _ = child.kill();
                self.settle(job_id, STATUS_FAILED, "failed", "worker has no stdin");
                return;
            }
        };
        let stdout = child.stdout.take();
        let child = Arc::new(Mutex::new(child));

        {
            let mut inner = guard(&self.inner);
            if let Some(job) = inner.jobs.get_mut(job_id) {
                job.child = Some(Arc::clone(&child));
                job.stdin = Some(Arc::clone(&stdin));
            }
            self.notify(&inner);
        }

        let _ = send_rpc(
            &stdin,
            &json!({
                "id": 1,
                "method": "initialize",
                "params": { "clientInfo": { "name": "thor-orchestrator", "title": "Thor", "version": "1" } }
            }),
        );
        let _ = send_rpc(&stdin, &json!({ "method": "initialized" }));
        let mut thread_start_params = json!({
            "cwd": cwd, "approvalPolicy": "never", "sandbox": "workspace-write"
        });
        if let Some(model) = model {
            thread_start_params["model"] = json!(model);
        }
        let _ = send_rpc(
            &stdin,
            &json!({ "id": 2, "method": "thread/start", "params": thread_start_params }),
        );

        if let Some(stdout) = stdout {
            let core = self.clone();
            let owned_id = job_id.to_string();
            let spec = spec.to_string();
            let stdin = Arc::clone(&stdin);
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let Ok(message) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    core.on_worker_message(&owned_id, &stdin, &spec, &message);
                }
                core.finish(
                    &owned_id,
                    STATUS_FAILED,
                    Some("failed".into()),
                    "worker connection closed".into(),
                    true,
                );
            });
        }
    }

    /// OpenCode (and anything else one-shot) has no persistent RPC thread to steer or send more
    /// work to: the whole task is the initial prompt, and the whole reply is whatever it printed
    /// before exiting. `thor_steer`/`thor_send` reject jobs of this kind for that reason.
    fn spawn_one_shot_worker(
        &self,
        job_id: &str,
        cwd: &str,
        spec: &str,
        model: Option<&str>,
        launcher: &Launcher,
    ) {
        let mut command = Command::new(&launcher.program);
        command
            .args(&launcher.args)
            .current_dir(PathBuf::from(cwd))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &launcher.env {
            command.env(key, value);
        }
        if let (Some(flag), Some(model)) = (launcher.model_flag.as_deref(), model) {
            command.arg(flag).arg(model);
        }
        command.arg(spec);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.settle(
                    job_id,
                    STATUS_FAILED,
                    "failed",
                    &format!("worker spawn failed: {error}"),
                );
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(Mutex::new(child));

        {
            let mut inner = guard(&self.inner);
            if let Some(job) = inner.jobs.get_mut(job_id) {
                job.child = Some(Arc::clone(&child));
            }
            self.notify(&inner);
        }

        let core = self.clone();
        let owned_id = job_id.to_string();
        thread::spawn(move || {
            use std::io::Read;
            let mut out = String::new();
            if let Some(mut stdout) = stdout {
                let _ = stdout.read_to_string(&mut out);
            }
            let mut err = String::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_string(&mut err);
            }
            let status = guard(&child).wait();
            let ok = matches!(status, Ok(status) if status.success());
            let mut text = out.trim().to_string();
            if text.is_empty() {
                text = err.trim().to_string();
            }
            if text.len() > REPLY_LIMIT {
                let cut = text.len() - REPLY_LIMIT;
                text = text.split_off(cut);
            }
            core.finish(
                &owned_id,
                if ok { STATUS_DONE } else { STATUS_FAILED },
                Some(if ok {
                    "succeeded".into()
                } else {
                    "failed".into()
                }),
                text,
                true,
            );
        });
    }

    fn settle(&self, job_id: &str, status: &str, outcome: &str, text: &str) {
        self.finish(
            job_id,
            status,
            Some(outcome.to_string()),
            text.to_string(),
            true,
        );
    }

    fn on_worker_message(
        &self,
        job_id: &str,
        stdin: &Arc<Mutex<ChildStdin>>,
        spec: &str,
        message: &Value,
    ) {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = message.get("result").cloned().unwrap_or(Value::Null);

        if message.get("id").and_then(Value::as_i64) == Some(2) {
            let thread_id = result
                .get("thread")
                .and_then(|thread| thread.get("id"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            if let Some(thread_id) = thread_id {
                {
                    let mut inner = guard(&self.inner);
                    if let Some(job) = inner.jobs.get_mut(job_id) {
                        job.thread_id = Some(thread_id.clone());
                    }
                    self.notify(&inner);
                }
                let _ = send_rpc(
                    stdin,
                    &json!({
                        "id": 3,
                        "method": "turn/start",
                        "params": {
                            "threadId": thread_id,
                            "input": [{ "type": "text", "text": spec }],
                            "approvalPolicy": "never"
                        }
                    }),
                );
            }
            return;
        }

        if method.ends_with("requestApproval") {
            if let Some(id) = message.get("id") {
                let _ = send_rpc(
                    stdin,
                    &json!({ "id": id, "result": { "decision": "accept" } }),
                );
            }
            return;
        }

        let mut inner = guard(&self.inner);
        let Some(job) = inner.jobs.get_mut(job_id) else {
            return;
        };

        match method {
            "turn/started" => {
                job.active_turn_id = params
                    .get("turn")
                    .and_then(|turn| turn.get("id"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            "item/agentMessage/delta" => {
                if let Some(delta) = params.get("delta").and_then(Value::as_str) {
                    job.reply.push_str(delta);
                    if job.reply.len() > REPLY_LIMIT {
                        let cut = job.reply.len() - REPLY_LIMIT;
                        job.reply = job.reply.split_off(cut);
                    }
                }
                return;
            }
            "turn/plan/updated" => {
                job.plan = params
                    .get("plan")
                    .and_then(Value::as_array)
                    .map(|steps| {
                        steps
                            .iter()
                            .filter_map(|step| step.get("step").and_then(Value::as_str))
                            .map(ToOwned::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
            }
            "turn/diff/updated" => {
                job.diff = params
                    .get("diff")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            "thread/tokenUsage/updated" => {
                if let Some(usage) = params.get("tokenUsage") {
                    job.tokens_total = extract_total_tokens(usage);
                    job.tokens = Some(usage.clone());
                }
            }
            "turn/completed" | "turn/failed" => {
                let completed = method == "turn/completed";
                let summary = job.reply.trim().to_string();
                drop(inner);
                self.finish(
                    job_id,
                    if completed {
                        STATUS_DONE
                    } else {
                        STATUS_FAILED
                    },
                    Some(if completed {
                        "succeeded".into()
                    } else {
                        "failed".into()
                    }),
                    summary,
                    false,
                );
                return;
            }
            _ => return,
        }

        self.notify(&inner);
    }

    /// Requeues `job_id` on its bucket's configured fallback when the failure text looks like a
    /// quota/rate-limit error. Returns `false` (do the normal terminal-failure handling) when
    /// there's no fallback, the fallback was already tried for this job, or the text doesn't
    /// match — false negatives just mean no failover, which is always safe.
    fn maybe_failover(&self, job_id: &str, text: &str) -> bool {
        if !looks_like_quota_exhaustion(text) {
            return false;
        }
        let mut inner = guard(&self.inner);
        let Some(job) = inner.jobs.get_mut(job_id) else {
            return false;
        };
        if job.settled() {
            return false;
        }
        let Some(fallback) = self
            .resolve_launcher(&job.bucket)
            .and_then(|launcher| launcher.fallback)
        else {
            return false;
        };
        if job.bucket_chain_tried.contains(&fallback) {
            return false;
        }

        let previous = job.bucket.clone();
        job.bucket_chain_tried.push(previous.clone());
        job.bucket = fallback.clone();
        // A model string that meant something to the old bucket's CLI may mean nothing (or the
        // wrong thing) to the fallback's — fall through to the fallback bucket's own default.
        job.model = None;
        job.status = STATUS_QUEUED.to_string();
        job.thread_id = None;
        job.active_turn_id = None;
        job.reply.clear();
        job.plan.clear();
        job.diff = None;
        job.tokens = None;
        job.tokens_total = 0;
        job.outcome = None;
        job.started_at = None;
        job.ended_at = None;
        job.protocol_hint = None;
        job.teardown();

        inner.running = inner.running.saturating_sub(1);
        inner.push_delivery(
            "failover",
            job_id,
            Some("failover".into()),
            format!(
                "bucket \"{previous}\" looked exhausted (quota/rate limit) — retrying automatically on fallback bucket \"{fallback}\""
            ),
        );
        inner.queue.push_back(job_id.to_string());
        self.notify(&inner);
        drop(inner);
        self.signal.notify_all();
        true
    }

    /// `terminal` decides whether the worker process dies with the turn. A completed turn keeps
    /// it alive so `thor_send` can hand it more work on the same thread; cancelling kills it.
    fn finish(
        &self,
        job_id: &str,
        status: &str,
        outcome: Option<String>,
        text: String,
        terminal: bool,
    ) {
        if terminal && status == STATUS_FAILED && self.maybe_failover(job_id, &text) {
            self.drain_queue();
            return;
        }
        {
            let mut inner = guard(&self.inner);
            let Some(job) = inner.jobs.get_mut(job_id) else {
                return;
            };
            if job.settled() {
                return;
            }
            job.status = status.to_string();
            job.outcome = outcome.clone();
            job.ended_at = Some(now_ms());
            job.active_turn_id = None;
            if terminal {
                job.teardown();
            }
            inner.running = inner.running.saturating_sub(1);
            inner.push_delivery("worker_done", job_id, outcome, text);
            self.notify(&inner);
        }
        self.signal.notify_all();
        self.drain_queue();
    }

    fn drain_queue(&self) {
        loop {
            let next = {
                let mut inner = guard(&self.inner);
                if inner.running >= inner.max_concurrent {
                    None
                } else {
                    inner.queue.pop_front()
                }
            };
            let Some(job_id) = next else { break };
            self.spawn_worker(&job_id);
        }
    }
}

// ---------------------------------------------------------------------- tools

pub fn tools() -> Value {
    json!([
        {
            "name": "thor_delegate",
            "description": "Hand independent units of work to worker agents that Thor runs for you. Returns job ids immediately; the workers run in parallel. Delegate any unit that would make you read more than 5 files or that you estimate at over 2 minutes of your own work, and send every qualifying unit in ONE call so they run at the same time. Each task must be self contained. All tasks in one call share the same bucket and model — make a separate call to mix them. Call thor_status first if you don't already know which buckets are configured.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "One self contained instruction per worker."
                    },
                    "cwd": { "type": "string", "description": "Working directory. Defaults to the lead's directory." },
                    "bucket": {
                        "type": "string",
                        "description": "Which configured worker bucket runs the tasks — see thor_status for the live list. \"codex\" (default) keeps a live thread you can steer or send more work to. Every other bucket is one-shot: fire the task, get the final answer, no steer/send afterwards — pick a cheap one-shot bucket (e.g. a local Ollama-backed OpenCode) for simple, well-scoped work."
                    },
                    "model": {
                        "type": "string",
                        "description": "Overrides the bucket's default model for this call (ignored by buckets with no model flag, e.g. plain codex). Example: \"ollama/qwen2.5-coder:7b\"."
                    },
                    "isolate": {
                        "type": "boolean",
                        "description": "Run this call's tasks in a fresh, isolated git worktree instead of cwd directly — a clean checkpoint before the worker touches anything, and no risk of colliding with other work in the same directory. Requires cwd to be inside a git repository. All tasks in one call share the same worktree (they already share bucket/model); for full task-level isolation make separate calls."
                    }
                },
                "required": ["tasks"]
            }
        },
        {
            "name": "thor_check",
            "description": "Collect what workers reported. With wait set it blocks until they settle. Process every delivery it returns before calling it again. A \"failover\" delivery means the job hit an automatic bucket failover and is still running (not settled) — its final \"worker_done\" delivery comes later, so don't treat a failover as the job finishing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "wait": { "type": "boolean" },
                    "untilAllSettled": {
                        "type": "boolean",
                        "description": "Default true: block until every worker has settled, so you never report on a partial set. Set false only when you want to react to the first worker that finishes."
                    },
                    "timeoutMs": { "type": "number" }
                }
            }
        },
        {
            "name": "thor_status",
            "description": "Snapshot without blocking: every worker's status, elapsed time, current plan and token usage, plus the list of configured buckets (id, label, protocol, default model, fallback) you can pass to thor_delegate. If a bucket has a fallback and a worker fails with what looks like a quota/rate-limit error, Thor automatically retries the same task on the fallback bucket — watch for a \"failover\" delivery in thor_check, the job keeps its id but its bucket changes.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "thor_steer",
            "description": "Correct a worker while its turn is still running, without killing it or losing its context. Use this instead of cancelling when the worker is heading the wrong way.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "jobId": { "type": "string" },
                    "message": { "type": "string" }
                },
                "required": ["jobId", "message"]
            }
        },
        {
            "name": "thor_send",
            "description": "Give a settled worker more work on its existing thread, keeping everything it already learned.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "jobId": { "type": "string" },
                    "message": { "type": "string" }
                },
                "required": ["jobId", "message"]
            }
        },
        {
            "name": "thor_cancel",
            "description": "Interrupt running workers.",
            "inputSchema": {
                "type": "object",
                "properties": { "jobIds": { "type": "array", "items": { "type": "string" } } },
                "required": ["jobIds"]
            }
        },
        {
            "name": "thor_release",
            "description": "Let go of settled workers you have no more work for. Account for every worker you started: either send it more work or release it.",
            "inputSchema": {
                "type": "object",
                "properties": { "jobIds": { "type": "array", "items": { "type": "string" } } },
                "required": ["jobIds"]
            }
        },
        {
            "name": "thor_diff",
            "description": "Read the unified diff a worker has produced so far.",
            "inputSchema": {
                "type": "object",
                "properties": { "jobId": { "type": "string" } },
                "required": ["jobId"]
            }
        }
    ])
}

fn string_list(arguments: &Map<String, Value>, key: &str) -> Vec<String> {
    arguments
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn required_str(arguments: &Map<String, Value>, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{key} is required"))
}

pub fn call_tool(core: &Core, name: &str, arguments: &Map<String, Value>) -> Result<Value, String> {
    match name {
        "thor_delegate" => {
            let tasks = string_list(arguments, "tasks");
            if tasks.is_empty() {
                return Err("tasks must contain at least one instruction".into());
            }
            let cwd = arguments
                .get("cwd")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|path| path.to_string_lossy().into_owned())
                })
                .ok_or_else(|| "cwd is required".to_string())?;
            let bucket = arguments
                .get("bucket")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(AGENT_CODEX)
                .to_string();
            let model = arguments
                .get("model")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);

            let mut created = Vec::new();
            {
                let mut inner = guard(&core.inner);
                inner.check_token_budget()?;
                for spec in tasks {
                    inner.job_counter += 1;
                    let id = format!("job-{:02}", inner.job_counter);
                    inner.jobs.insert(
                        id.clone(),
                        Job {
                            id: id.clone(),
                            spec: spec.clone(),
                            cwd: cwd.clone(),
                            bucket: bucket.clone(),
                            model: model.clone(),
                            bucket_chain_tried: Vec::new(),
                            protocol_hint: None,
                            status: STATUS_QUEUED.to_string(),
                            thread_id: None,
                            active_turn_id: None,
                            reply: String::new(),
                            plan: Vec::new(),
                            diff: None,
                            tokens: None,
                            tokens_total: 0,
                            outcome: None,
                            started_at: None,
                            ended_at: None,
                            child: None,
                            stdin: None,
                            next_request_id: 10,
                        },
                    );
                    inner.order.push(id.clone());
                    inner.queue.push_back(id.clone());
                    created.push(json!({ "id": id, "spec": spec }));
                }
                core.notify(&inner);
            }
            core.drain_queue();

            let limit = guard(&core.inner).max_concurrent;
            Ok(json!({
                "accepted": created.len(),
                "runningInParallel": true,
                "concurrencyLimit": limit,
                "bucket": bucket,
                "jobs": created,
                "next": "call thor_check with wait true"
            }))
        }

        "thor_check" => {
            let wait = arguments
                .get("wait")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let until_all_settled = arguments
                .get("untilAllSettled")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let timeout = arguments
                .get("timeoutMs")
                .and_then(Value::as_u64)
                .unwrap_or(300_000)
                .min(MAX_WAIT_MS);

            let mut inner = guard(&core.inner);
            if wait {
                let deadline = Instant::now() + Duration::from_millis(timeout);
                loop {
                    let busy = inner.running > 0 || !inner.queue.is_empty();
                    if !busy {
                        break;
                    }
                    if !until_all_settled && !inner.deliveries.is_empty() {
                        break;
                    }
                    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                        break;
                    };
                    let (next, timed_out) = core
                        .signal
                        .wait_timeout(inner, remaining)
                        .map_err(|_| "orchestrator state poisoned".to_string())?;
                    inner = next;
                    if timed_out.timed_out() {
                        break;
                    }
                }
            }

            let mut deliveries = Vec::new();
            while let Some(delivery) = inner.deliveries.pop_front() {
                deliveries.push(delivery.to_value());
            }
            let pending = inner.running + inner.queue.len();
            Ok(json!({
                "deliveries": deliveries,
                "workersStillBusy": pending,
                "note": if pending > 0 {
                    "timed out with workers still running: call thor_check again"
                } else {
                    "every worker settled"
                }
            }))
        }

        "thor_status" => {
            let mut snapshot = core.snapshot();
            if let Value::Object(map) = &mut snapshot {
                map.insert("buckets".into(), core.list_buckets()["buckets"].clone());
            }
            Ok(snapshot)
        }

        "thor_steer" => {
            let job_id = required_str(arguments, "jobId")?;
            let message = required_str(arguments, "message")?;
            let mut inner = guard(&core.inner);
            let job = inner
                .jobs
                .get(&job_id)
                .ok_or_else(|| format!("unknown job {job_id}"))?;
            if job.protocol_hint == Some(LauncherKind::OneShotRun) {
                return Err(format!(
                    "job {job_id} is a one-shot {} worker: it has no live thread to steer",
                    job.bucket
                ));
            }
            let thread_id = job
                .thread_id
                .clone()
                .ok_or_else(|| format!("job {job_id} has no thread yet"))?;
            let turn_id = job
                .active_turn_id
                .clone()
                .ok_or_else(|| format!("job {job_id} has no running turn to steer"))?;
            job_rpc(
                &mut inner,
                &job_id,
                "turn/steer",
                json!({
                    "threadId": thread_id,
                    "input": [{ "type": "text", "text": message }],
                    "expectedTurnId": turn_id
                }),
            )?;
            Ok(json!({ "steered": job_id, "turnId": turn_id }))
        }

        "thor_send" => {
            let job_id = required_str(arguments, "jobId")?;
            let message = required_str(arguments, "message")?;
            let mut inner = guard(&core.inner);
            let job = inner
                .jobs
                .get(&job_id)
                .ok_or_else(|| format!("unknown job {job_id}"))?;
            if job.protocol_hint == Some(LauncherKind::OneShotRun) {
                return Err(format!(
                    "job {job_id} is a one-shot {} worker: release it and delegate a new task instead",
                    job.bucket
                ));
            }
            let thread_id = job
                .thread_id
                .clone()
                .ok_or_else(|| format!("job {job_id} has no thread"))?;
            if job.stdin.is_none() {
                return Err(format!(
                    "job {job_id} was released and cannot take more work"
                ));
            }
            if inner.running >= inner.max_concurrent {
                return Err(format!(
                    "concurrency limit {} reached, call thor_check first",
                    inner.max_concurrent
                ));
            }
            inner.check_token_budget()?;
            job_rpc(
                &mut inner,
                &job_id,
                "turn/start",
                json!({
                    "threadId": thread_id,
                    "input": [{ "type": "text", "text": message }],
                    "approvalPolicy": "never"
                }),
            )?;
            if let Some(job) = inner.jobs.get_mut(&job_id) {
                job.status = STATUS_RUNNING.to_string();
                job.outcome = None;
                job.ended_at = None;
                job.reply.clear();
            }
            inner.running += 1;
            core.notify(&inner);
            Ok(json!({ "sent": job_id }))
        }

        "thor_cancel" => {
            let ids = string_list(arguments, "jobIds");
            let mut cancelled = Vec::new();
            for job_id in ids {
                let payload = {
                    let inner = guard(&core.inner);
                    inner.jobs.get(&job_id).and_then(|job| {
                        match (job.thread_id.clone(), job.active_turn_id.clone()) {
                            (Some(thread_id), Some(turn_id)) => {
                                Some(json!({ "threadId": thread_id, "turnId": turn_id }))
                            }
                            _ => None,
                        }
                    })
                };
                if let Some(payload) = payload {
                    let mut inner = guard(&core.inner);
                    let _ = job_rpc(&mut inner, &job_id, "turn/interrupt", payload);
                }
                core.finish(
                    &job_id,
                    STATUS_CANCELLED,
                    Some("cancelled".into()),
                    "cancelled by the lead".into(),
                    true,
                );
                cancelled.push(job_id);
            }
            Ok(json!({ "cancelled": cancelled }))
        }

        "thor_release" => {
            let ids = string_list(arguments, "jobIds");
            let mut released = Vec::new();
            {
                let mut inner = guard(&core.inner);
                for job_id in &ids {
                    let Some(job) = inner.jobs.get_mut(job_id) else {
                        continue;
                    };
                    if job.status == STATUS_RUNNING {
                        continue;
                    }
                    job.teardown();
                    job.status = STATUS_RELEASED.to_string();
                    released.push(job_id.clone());
                }
                core.notify(&inner);
            }
            Ok(json!({ "released": released }))
        }

        "thor_diff" => {
            let job_id = required_str(arguments, "jobId")?;
            let inner = guard(&core.inner);
            let job = inner
                .jobs
                .get(&job_id)
                .ok_or_else(|| format!("unknown job {job_id}"))?;
            Ok(json!({ "jobId": job_id, "diff": job.diff.clone().unwrap_or_default() }))
        }

        other => Err(format!("unknown tool {other}")),
    }
}

// ------------------------------------------------------------------ transport

/// One JSON-RPC message in, one response out. `None` means the message was a notification and
/// the caller should answer 202 with no body.
pub fn handle_mcp_body(core: &Core, body: &str) -> Option<String> {
    let message: Value = serde_json::from_str(body).ok()?;
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    let response = match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("2025-06-18"),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "thor", "title": "Thor", "version": "1" }
            }
        }),
        "tools/list" => json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools() } }),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let empty = Map::new();
            let arguments = params
                .get("arguments")
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            match call_tool(core, name, arguments) {
                Ok(value) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": value.to_string() }] }
                }),
                Err(error) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("error: {error}") }],
                        "isError": true
                    }
                }),
            }
        }
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        other => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("unknown method {other}") }
        }),
    };

    Some(response.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_markers_match_known_provider_phrasings() {
        for text in [
            "Error: 429 Too Many Requests",
            "you have exceeded your current quota",
            "rate_limit_exceeded",
            "RESOURCE_EXHAUSTED: quota exceeded",
            "the model is currently overloaded, please try again later",
            "503 Service Unavailable",
            "billing hard limit reached",
        ] {
            assert!(looks_like_quota_exhaustion(text), "should match: {text}");
        }
    }

    #[test]
    fn unrelated_failures_do_not_match() {
        for text in [
            "file not found",
            "permission denied",
            "syntax error on line 12",
            "",
        ] {
            assert!(
                !looks_like_quota_exhaustion(text),
                "should not match: {text}"
            );
        }
    }

    #[test]
    fn extracts_a_direct_total_field() {
        assert_eq!(extract_total_tokens(&json!({ "totalTokens": 42 })), 42);
        assert_eq!(extract_total_tokens(&json!({ "total_tokens": 7 })), 7);
        assert_eq!(extract_total_tokens(&json!({ "total": 3 })), 3);
    }

    #[test]
    fn falls_back_to_summing_input_and_output() {
        assert_eq!(
            extract_total_tokens(&json!({ "inputTokens": 10, "outputTokens": 5 })),
            15
        );
        assert_eq!(
            extract_total_tokens(&json!({ "input_tokens": 100, "output_tokens": 50 })),
            150
        );
    }

    #[test]
    fn unknown_shapes_report_zero() {
        assert_eq!(extract_total_tokens(&json!({ "somethingElse": 1 })), 0);
        assert_eq!(extract_total_tokens(&json!({})), 0);
    }
}

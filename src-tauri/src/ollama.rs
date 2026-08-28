// Integração com Ollama: instalar, listar/baixar modelos, e subir/derrubar
// múltiplas instâncias do daemon `ollama serve` em portas separadas — cada
// instância roda o mesmo storage de modelos compartilhado (~/.ollama/models,
// não sobrescrevemos OLLAMA_MODELS), mas em processos e portas distintos, para
// permitir que sessões de agente diferentes conversem com dois daemons ao
// mesmo tempo em vez de disputar um único processo. O campo `model` de cada
// instância é só um rótulo de intenção (ex: "instância para llama3.2:3b") —
// o Ollama não restringe de fato quais modelos um daemon aceita servir.
//
// Cada instância é, na prática, colocada atrás de um proxy reverso do Thor:
// a porta pública (devolvida ao frontend, é a que qualquer chamador externo
// usa) nunca muda de comportamento; o `ollama serve` real fica numa porta
// interna que só o Thor conhece. O proxy existe só para poder observar as
// respostas (streaming, sem bufferizar) e somar tokens/requests reais — o
// Ollama não expõe nenhuma métrica cumulativa própria. Se o proxy não
// conseguir subir por algum motivo, caímos de volta para expor o `ollama
// serve` direto na porta pública (sem estatísticas de throughput, mas
// funcional).

use crate::git_control::repository_root;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use sysinfo::System;
use tauri::{AppHandle, Emitter};

const DEFAULT_PORT: u16 = 11434;
const INTERNAL_PORT_BASE: u16 = 41434;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct OllamaInstanceInfo {
    pub id: String,
    pub port: u16,
    pub model: String,
    pub pid: u32,
}

#[derive(Serialize, Clone, Debug)]
struct OllamaPullProgress {
    model: String,
    line: String,
    done: bool,
}

/// Estatísticas ao vivo de uma instância: consumo de recursos do processo
/// `ollama serve` (via `sysinfo`) e, quando a instância está atrás do proxy
/// (`proxied: true`), throughput real observado (tokens/requests). Quando não
/// proxied, os contadores ficam em zero e `tokens_per_second` é `None` — o
/// frontend deve esconder essa seção em vez de mostrar zero como se fosse dado
/// real.
#[derive(Serialize, Clone, Debug)]
pub struct OllamaInstanceStats {
    pub id: String,
    pub cpu_percent: f32,
    pub ram_mb: f64,
    pub gpu_percent: Option<f32>,
    pub proxied: bool,
    pub prompt_tokens_total: u64,
    pub eval_tokens_total: u64,
    pub requests_total: u64,
    pub tokens_per_second: Option<f32>,
}

#[derive(Default)]
struct ProxyCounters {
    prompt_tokens_total: AtomicU64,
    eval_tokens_total: AtomicU64,
    requests_total: AtomicU64,
}

struct RunningInstance {
    child: Child,
    /// Porta pública: a que `ollama_start_instance`/`ollama_list_instances` sempre
    /// devolveram, e a que qualquer chamador externo usa. Nunca muda de sentido.
    port: u16,
    model: String,
    /// `Some(porta interna)` quando o proxy está ativo (o `child` acima roda na porta
    /// interna, não na pública); `None` quando caímos no modo direto (fallback).
    internal_port: Option<u16>,
    proxy_stop: Option<Arc<AtomicBool>>,
    proxy_thread: Option<JoinHandle<()>>,
    counters: Arc<ProxyCounters>,
    started_at: Instant,
}

fn instances() -> &'static Mutex<HashMap<String, RunningInstance>> {
    static INSTANCES: OnceLock<Mutex<HashMap<String, RunningInstance>>> = OnceLock::new();
    INSTANCES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_port_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn next_free_port(starting_at: u16) -> Option<u16> {
    (starting_at..starting_at.saturating_add(200)).find(|p| is_port_free(*p))
}

fn internal_next_free_port() -> Option<u16> {
    next_free_port(INTERNAL_PORT_BASE)
}

#[tauri::command]
pub async fn ollama_is_installed() -> bool {
    tokio::task::spawn_blocking(|| which::which("ollama").is_ok())
        .await
        .unwrap_or(false)
}

#[tauri::command]
pub async fn ollama_install() -> Result<(), String> {
    tokio::task::spawn_blocking(install_inner).await.map_err(|e| e.to_string())?
}

fn install_inner() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("sh")
            .arg("-c")
            .arg("curl -fsSL https://ollama.com/install.sh | sh")
            .status()
            .map_err(|e| format!("falha ao executar o instalador: {e}"))?;
        if !status.success() {
            return Err(format!("instalador saiu com status {status}"));
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("sh")
            .arg("-c")
            .arg("brew install ollama")
            .status()
            .map_err(|e| format!("falha ao executar o Homebrew: {e}"))?;
        if !status.success() {
            return Err("instalação via Homebrew falhou — instale manualmente em https://ollama.com/download".into());
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err("instalação automática não suportada nesta plataforma — baixe em https://ollama.com/download".into())
    }
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    #[serde(default)]
    size: u64,
}

#[tauri::command]
pub async fn ollama_list_models(port: Option<u16>) -> Result<Vec<OllamaModelInfo>, String> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let url = format!("http://127.0.0.1:{port}/api/tags");
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("não foi possível falar com o Ollama em {url}: {e}"))?;
    let parsed: TagsResponse = response
        .json()
        .await
        .map_err(|e| format!("resposta inesperada do Ollama: {e}"))?;
    Ok(parsed
        .models
        .into_iter()
        .map(|m| OllamaModelInfo {
            name: m.name,
            size_bytes: m.size,
        })
        .collect())
}

/// A daemon reachable on the standard Ollama port that the Thor did not
/// start itself (e.g. a system-wide `ollama serve`, or one the user runs by
/// hand) — surfaced read-only in the Local AI panel so it's not invisible
/// just because Thor isn't the one managing it, without pretending Thor can
/// start/stop it.
#[derive(Serialize, Clone, Debug)]
pub struct ExternalOllamaInfo {
    pub port: u16,
    pub models: Vec<OllamaModelInfo>,
}

#[tauri::command]
pub async fn ollama_detect_external() -> Option<ExternalOllamaInfo> {
    let already_thor_managed = instances()
        .lock()
        .ok()
        .map(|map| map.values().any(|inst| inst.port == DEFAULT_PORT))
        .unwrap_or(false);
    if already_thor_managed {
        return None;
    }
    let models = ollama_list_models(Some(DEFAULT_PORT)).await.ok()?;
    Some(ExternalOllamaInfo {
        port: DEFAULT_PORT,
        models,
    })
}

#[tauri::command]
pub async fn ollama_pull_model(app: AppHandle, model: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || pull_inner(app, model))
        .await
        .map_err(|e| e.to_string())?
}

fn pull_inner(app: AppHandle, model: String) -> Result<(), String> {
    let mut child = Command::new("ollama")
        .arg("pull")
        .arg(&model)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("falha ao iniciar 'ollama pull {model}': {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let model_for_reader = model.clone();
        let app_for_reader = app.clone();
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = app_for_reader.emit(
                "ollama-pull-progress",
                OllamaPullProgress {
                    model: model_for_reader.clone(),
                    line,
                    done: false,
                },
            );
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("falha ao aguardar 'ollama pull {model}': {e}"))?;

    let _ = app.emit(
        "ollama-pull-progress",
        OllamaPullProgress {
            model: model.clone(),
            line: String::new(),
            done: true,
        },
    );

    if !status.success() {
        return Err(format!("'ollama pull {model}' saiu com status {status}"));
    }
    Ok(())
}

fn blocking_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::blocking::Client::new)
}

/// Envolve a resposta do `ollama serve` real: repassa os bytes para o cliente
/// exatamente como chegam (sem bufferizar a resposta inteira — streaming
/// continua funcionando para quem depende de tokens ao vivo) enquanto observa
/// as linhas NDJSON em busca do objeto final (`"done":true`), que carrega os
/// contadores cumulativos daquela chamada (`prompt_eval_count`/`eval_count`).
struct TeeCounting<R> {
    inner: R,
    counters: Arc<ProxyCounters>,
    line_buf: Vec<u8>,
    finished: bool,
}

impl<R: Read> TeeCounting<R> {
    fn new(inner: R, counters: Arc<ProxyCounters>) -> Self {
        Self {
            inner,
            counters,
            line_buf: Vec::new(),
            finished: false,
        }
    }

    fn scan(&mut self, chunk: &[u8]) {
        self.line_buf.extend_from_slice(chunk);
        while let Some(pos) = self.line_buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.line_buf.drain(..=pos).collect();
            self.try_parse_line(&line);
        }
    }

    fn try_parse_line(&mut self, line: &[u8]) {
        let Ok(text) = std::str::from_utf8(line) else {
            return;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            return;
        };
        let done = value.get("done").and_then(|v| v.as_bool()).unwrap_or(false);
        if !done {
            return;
        }
        let prompt_tokens = value
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let eval_tokens = value.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
        if prompt_tokens > 0 {
            self.counters
                .prompt_tokens_total
                .fetch_add(prompt_tokens, Ordering::Relaxed);
        }
        if eval_tokens > 0 {
            self.counters
                .eval_tokens_total
                .fetch_add(eval_tokens, Ordering::Relaxed);
        }
        self.counters.requests_total.fetch_add(1, Ordering::Relaxed);
    }
}

impl<R: Read> Read for TeeCounting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n == 0 {
            if !self.finished && !self.line_buf.is_empty() {
                let remainder = std::mem::take(&mut self.line_buf);
                self.try_parse_line(&remainder);
            }
            self.finished = true;
        } else {
            self.scan(&buf[..n]);
        }
        Ok(n)
    }
}

fn handle_proxy_request(mut request: tiny_http::Request, internal_port: u16, counters: Arc<ProxyCounters>) {
    let method_str = request.method().as_str().to_string();
    let url = request.url().to_string();

    let mut body = Vec::new();
    if let Err(e) = request.as_reader().read_to_end(&mut body) {
        let _ = request.respond(
            tiny_http::Response::from_string(format!("proxy: falha lendo corpo: {e}"))
                .with_status_code(502),
        );
        return;
    }

    let method = reqwest::Method::from_bytes(method_str.as_bytes()).unwrap_or(reqwest::Method::GET);
    let target = format!("http://127.0.0.1:{internal_port}{url}");
    let mut builder = blocking_client().request(method, target.as_str());
    for header in request.headers() {
        let name = header.field.as_str().as_str();
        if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        builder = builder.header(name, header.value.as_str());
    }
    if !body.is_empty() {
        builder = builder.body(body);
    }

    let upstream = match builder.send() {
        Ok(resp) => resp,
        Err(e) => {
            let _ = request.respond(
                tiny_http::Response::from_string(format!("proxy: ollama indisponível: {e}"))
                    .with_status_code(502),
            );
            return;
        }
    };

    let status = upstream.status().as_u16();
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let header = tiny_http::Header::from_bytes("Content-Type", content_type.as_bytes())
        .unwrap_or_else(|_| tiny_http::Header::from_bytes("Content-Type", "application/octet-stream").unwrap());

    let tee = TeeCounting::new(upstream, counters);
    let response = tiny_http::Response::new(tiny_http::StatusCode(status), vec![header], tee, None, None);
    let _ = request.respond(response);
}

fn proxy_loop(server: tiny_http::Server, internal_port: u16, counters: Arc<ProxyCounters>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match server.recv_timeout(Duration::from_millis(300)) {
            Ok(Some(request)) => {
                let counters = counters.clone();
                thread::spawn(move || handle_proxy_request(request, internal_port, counters));
            }
            Ok(None) => continue,
            Err(_) => break,
        }
    }
}

/// Tenta abrir o proxy na porta pública. O bind acontece de forma síncrona
/// aqui (não numa thread) para que o chamador saiba deterministicamente se
/// deu certo antes de decidir se cai no modo direto (fallback).
fn spawn_proxy(
    public_port: u16,
    internal_port: u16,
    counters: Arc<ProxyCounters>,
) -> Result<(Arc<AtomicBool>, JoinHandle<()>), String> {
    let server = tiny_http::Server::http(("127.0.0.1", public_port))
        .map_err(|e| format!("não foi possível abrir o proxy em {public_port}: {e}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop.clone();
    let handle = thread::spawn(move || proxy_loop(server, internal_port, counters, stop_for_thread));
    Ok((stop, handle))
}

fn gpu_utilization_percent() -> Option<f32> {
    static CACHE: OnceLock<Mutex<Option<(Instant, Option<f32>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((at, value)) = guard.as_ref() {
        if at.elapsed() < Duration::from_secs(2) {
            return *value;
        }
    }
    let value = read_nvidia_gpu_utilization();
    *guard = Some((Instant::now(), value));
    value
}

/// Melhor esforço: leitura de utilização da GPU inteira (não por processo —
/// atribuir uso de GPU a um processo específico de forma confiável exige
/// NVML, fora de escopo aqui). Se `nvidia-smi` não existir (ex: sem GPU
/// NVIDIA, ou AMD/Apple), devolve `None` silenciosamente — nunca um valor
/// inventado.
fn read_nvidia_gpu_utilization() -> Option<f32> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next()?.trim().parse::<f32>().ok()
}

fn shared_system() -> &'static Mutex<System> {
    static SYS: OnceLock<Mutex<System>> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(System::new()))
}

#[tauri::command]
pub fn ollama_list_instances() -> Vec<OllamaInstanceInfo> {
    let Ok(map) = instances().lock() else {
        return Vec::new();
    };
    map.iter()
        .map(|(id, inst)| OllamaInstanceInfo {
            id: id.clone(),
            port: inst.port,
            model: inst.model.clone(),
            pid: inst.child.id(),
        })
        .collect()
}

fn spawn_ollama_serve(bind_port: u16) -> Result<Child, String> {
    Command::new("ollama")
        .arg("serve")
        .env("OLLAMA_HOST", format!("127.0.0.1:{bind_port}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("falha ao iniciar 'ollama serve' na porta {bind_port}: {e}"))
}

#[tauri::command]
pub fn ollama_start_instance(model: String, port: Option<u16>) -> Result<OllamaInstanceInfo, String> {
    let public_port = match port {
        Some(p) if is_port_free(p) => p,
        Some(p) => return Err(format!("porta {p} já está em uso")),
        None => next_free_port(DEFAULT_PORT).ok_or("nenhuma porta livre encontrada")?,
    };

    let counters = Arc::new(ProxyCounters::default());
    let internal_port = internal_next_free_port();

    let (child, internal_port, proxy_stop, proxy_thread) = match internal_port {
        Some(internal_port) => {
            let real_child = spawn_ollama_serve(internal_port)?;
            match spawn_proxy(public_port, internal_port, counters.clone()) {
                Ok((stop, handle)) => (real_child, Some(internal_port), Some(stop), Some(handle)),
                Err(e) => {
                    eprintln!(
                        "[ollama] proxy indisponível na porta {public_port} ({e}); expondo 'ollama serve' direto"
                    );
                    let mut real_child = real_child;
                    let _ = real_child.kill();
                    let _ = real_child.wait();
                    let direct_child = spawn_ollama_serve(public_port)?;
                    (direct_child, None, None, None)
                }
            }
        }
        None => {
            eprintln!("[ollama] nenhuma porta interna livre; expondo 'ollama serve' direto na porta pública");
            let direct_child = spawn_ollama_serve(public_port)?;
            (direct_child, None, None, None)
        }
    };

    let id = nanoid!(8);
    let info = OllamaInstanceInfo {
        id: id.clone(),
        port: public_port,
        model: model.clone(),
        pid: child.id(),
    };

    let Ok(mut map) = instances().lock() else {
        return Err("estado interno de instâncias indisponível".into());
    };
    map.insert(
        id,
        RunningInstance {
            child,
            port: public_port,
            model,
            internal_port,
            proxy_stop,
            proxy_thread,
            counters,
            started_at: Instant::now(),
        },
    );
    Ok(info)
}

fn stop_instance_inner(inst: &mut RunningInstance) -> Result<(), String> {
    if let Some(stop) = inst.proxy_stop.take() {
        stop.store(true, Ordering::SeqCst);
    }
    if let Some(handle) = inst.proxy_thread.take() {
        let _ = handle.join();
    }
    inst.child.kill().map_err(|e| format!("falha ao encerrar instância: {e}"))?;
    let _ = inst.child.wait();
    Ok(())
}

#[tauri::command]
pub fn ollama_stop_instance(id: String) -> Result<(), String> {
    let Ok(mut map) = instances().lock() else {
        return Err("estado interno de instâncias indisponível".into());
    };
    let Some(mut inst) = map.remove(&id) else {
        return Ok(());
    };
    stop_instance_inner(&mut inst)
}

#[tauri::command]
pub fn ollama_get_instance_stats(id: String) -> Result<OllamaInstanceStats, String> {
    let Ok(map) = instances().lock() else {
        return Err("estado interno de instâncias indisponível".into());
    };
    let inst = map
        .get(&id)
        .ok_or_else(|| format!("instância {id} não encontrada"))?;

    let pid = sysinfo::Pid::from_u32(inst.child.id());
    let (cpu_percent, ram_mb) = {
        let sys_lock = shared_system();
        let mut sys = sys_lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]));
        match sys.process(pid) {
            Some(process) => (process.cpu_usage(), process.memory() as f64 / 1024.0 / 1024.0),
            None => (0.0, 0.0),
        }
    };

    let proxied = inst.internal_port.is_some();
    let prompt_tokens_total = inst.counters.prompt_tokens_total.load(Ordering::Relaxed);
    let eval_tokens_total = inst.counters.eval_tokens_total.load(Ordering::Relaxed);
    let requests_total = inst.counters.requests_total.load(Ordering::Relaxed);
    let tokens_per_second = if proxied && eval_tokens_total > 0 {
        let elapsed_secs = inst.started_at.elapsed().as_secs_f32().max(1.0);
        Some(eval_tokens_total as f32 / elapsed_secs)
    } else {
        None
    };

    Ok(OllamaInstanceStats {
        id,
        cpu_percent,
        ram_mb,
        gpu_percent: gpu_utilization_percent(),
        proxied,
        prompt_tokens_total,
        eval_tokens_total,
        requests_total,
        tokens_per_second,
    })
}

/// Melhor esforço para achar a porta onde os modelos realmente estão: prefere
/// a porta padrão (cobre tanto um `ollama serve` de sistema já rodando quanto
/// o caso comum de uma única instância do Thor que conseguiu ficar nela) e só
/// cai para uma instância gerenciada pelo Thor se a padrão não responder e
/// houver exatamente uma rodando — com mais de uma, a ambiguidade não tem
/// solução sem escolher arbitrariamente, então mantemos a padrão.
fn resolve_ollama_base_port() -> u16 {
    let default_probe = blocking_client()
        .get(format!("http://127.0.0.1:{DEFAULT_PORT}/api/tags"))
        .timeout(Duration::from_millis(500))
        .send();
    if default_probe.is_ok() {
        return DEFAULT_PORT;
    }

    if let Ok(map) = instances().lock() {
        if map.len() == 1 {
            if let Some(inst) = map.values().next() {
                return inst.port;
            }
        }
    }
    DEFAULT_PORT
}

fn ollama_opencode_config_write_inner(repo: String, model: String) -> Result<(), String> {
    let root = repository_root(&repo)?;
    let path = root.join("opencode.json");
    let port = resolve_ollama_base_port();

    let mut config: serde_json::Map<String, Value> = if path.is_file() {
        let raw = std::fs::read_to_string(&path).map_err(|e| format!("read_failed:{e}"))?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(Value::Object(map)) => map,
            _ => return Ok(()),
        }
    } else {
        let mut map = serde_json::Map::new();
        map.insert(
            "$schema".to_string(),
            Value::String("https://opencode.ai/config.json".to_string()),
        );
        map
    };

    let provider = config
        .entry("provider".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Value::Object(provider_map) = provider {
        let ollama_entry = provider_map.entry("ollama".to_string()).or_insert_with(|| {
            serde_json::json!({
                "npm": "@ai-sdk/openai-compatible",
                "name": "Ollama (local)",
            })
        });
        if let Value::Object(ollama_map) = ollama_entry {
            ollama_map.insert(
                "options".to_string(),
                serde_json::json!({ "baseURL": format!("http://127.0.0.1:{port}/v1") }),
            );
            let models = ollama_map
                .entry("models".to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Value::Object(models_map) = models {
                models_map.insert(model.clone(), serde_json::json!({ "name": model }));
            }
        }
    }

    let body = serde_json::to_string_pretty(&Value::Object(config)).map_err(|e| e.to_string())?;
    std::fs::write(&path, body).map_err(|e| format!("write_failed:{e}"))
}

/// Garante que `opencode.json` (na raiz do repo) tenha um provider "ollama"
/// apontando pro daemon Ollama certo antes de abrir um terminal OpenCode com
/// `--model ollama/<model>` — sem isso o OpenCode rejeita o modelo com
/// "Provider not found: ollama" (ele não descobre daemons locais sozinho,
/// precisa do provider explícito no config). Nunca mexe em outras chaves do
/// arquivo (outros providers, blocos `mcp`, etc.).
#[tauri::command]
pub async fn ollama_opencode_config_write(repo: String, model: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || ollama_opencode_config_write_inner(repo, model))
        .await
        .map_err(|error| format!("ollama_opencode_config_write: falha na task bloqueante: {error}"))?
}

/// Chamado no shutdown do app para não deixar daemons `ollama serve` (nem
/// threads de proxy) órfãos.
pub fn kill_all_instances() {
    let Ok(mut map) = instances().lock() else {
        return;
    };
    for (_, mut inst) in map.drain() {
        let _ = stop_instance_inner(&mut inst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_git_repo(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("thor-{name}-{}", nanoid!(6)));
        std::fs::create_dir_all(&root).unwrap();
        std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        root
    }

    #[test]
    fn opencode_config_write_creates_and_merges_ollama_provider_without_clobbering() {
        let root = temp_git_repo("ollama-opencode-test");
        let root_str = root.to_string_lossy().into_owned();

        ollama_opencode_config_write_inner(root_str.clone(), "qwen2.5-coder:14b".to_string())
            .unwrap();
        let path = root.join("opencode.json");
        let parsed: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed["provider"]["ollama"]["npm"], "@ai-sdk/openai-compatible");
        assert!(parsed["provider"]["ollama"]["options"]["baseURL"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:"));
        assert!(parsed["provider"]["ollama"]["models"]["qwen2.5-coder:14b"].is_object());

        std::fs::write(
            &path,
            r#"{"model": "anthropic/claude-sonnet-4-5", "provider": {"openai": {"options": {"baseURL": "http://127.0.0.1:8787/w/opencode/openai/v1"}}}, "mcp": {"other": {"type": "local"}}}"#,
        )
        .unwrap();
        ollama_opencode_config_write_inner(root_str, "llama3.2:3b".to_string()).unwrap();
        let merged: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(merged["model"], "anthropic/claude-sonnet-4-5");
        assert_eq!(
            merged["provider"]["openai"]["options"]["baseURL"],
            "http://127.0.0.1:8787/w/opencode/openai/v1"
        );
        assert_eq!(merged["mcp"]["other"]["type"], "local");
        assert!(merged["provider"]["ollama"]["models"]["llama3.2:3b"].is_object());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn next_free_port_finds_an_open_port_starting_from_the_given_base() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let taken_port = listener.local_addr().unwrap().port();

        let found = next_free_port(taken_port).expect("should find a free port");
        assert_ne!(found, taken_port);
    }

    #[test]
    fn is_port_free_reports_false_for_a_bound_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!is_port_free(port));
    }

    #[test]
    fn internal_next_free_port_is_disjoint_from_the_public_range() {
        let port = internal_next_free_port().expect("should find a free internal port");
        assert!(port >= INTERNAL_PORT_BASE);
    }

    #[test]
    fn tee_counting_extracts_token_counts_from_the_final_ndjson_line() {
        let body = b"{\"response\":\"hi\",\"done\":false}\n{\"response\":\"\",\"done\":true,\"prompt_eval_count\":12,\"eval_count\":34}\n".to_vec();
        let counters = Arc::new(ProxyCounters::default());
        let mut tee = TeeCounting::new(std::io::Cursor::new(body.clone()), counters.clone());
        let mut sink = Vec::new();
        std::io::copy(&mut tee, &mut sink).unwrap();

        assert_eq!(sink, body, "bytes must pass through unchanged (streaming)");
        assert_eq!(counters.prompt_tokens_total.load(Ordering::Relaxed), 12);
        assert_eq!(counters.eval_tokens_total.load(Ordering::Relaxed), 34);
        assert_eq!(counters.requests_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tee_counting_handles_a_final_line_with_no_trailing_newline() {
        let body = b"{\"done\":true,\"prompt_eval_count\":5,\"eval_count\":7}".to_vec();
        let counters = Arc::new(ProxyCounters::default());
        let mut tee = TeeCounting::new(std::io::Cursor::new(body), counters.clone());
        let mut sink = Vec::new();
        std::io::copy(&mut tee, &mut sink).unwrap();

        assert_eq!(counters.eval_tokens_total.load(Ordering::Relaxed), 7);
        assert_eq!(counters.requests_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tee_counting_ignores_intermediate_streaming_lines() {
        let body = b"{\"done\":false}\n{\"done\":false}\n{\"done\":true,\"eval_count\":9}\n".to_vec();
        let counters = Arc::new(ProxyCounters::default());
        let mut tee = TeeCounting::new(std::io::Cursor::new(body), counters.clone());
        let mut sink = Vec::new();
        std::io::copy(&mut tee, &mut sink).unwrap();

        assert_eq!(counters.requests_total.load(Ordering::Relaxed), 1);
        assert_eq!(counters.eval_tokens_total.load(Ordering::Relaxed), 9);
    }
}

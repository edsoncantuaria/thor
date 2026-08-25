// Integração com Ollama: instalar, listar/baixar modelos, e subir/derrubar
// múltiplas instâncias do daemon `ollama serve` em portas separadas — cada
// instância roda o mesmo storage de modelos compartilhado (~/.ollama/models,
// não sobrescrevemos OLLAMA_MODELS), mas em processos e portas distintos, para
// permitir que sessões de agente diferentes conversem com dois daemons ao
// mesmo tempo em vez de disputar um único processo. O campo `model` de cada
// instância é só um rótulo de intenção (ex: "instância para llama3.2:3b") —
// o Ollama não restringe de fato quais modelos um daemon aceita servir.

use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

const DEFAULT_PORT: u16 = 11434;

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

struct RunningInstance {
    child: Child,
    port: u16,
    model: String,
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

#[tauri::command]
pub fn ollama_start_instance(model: String, port: Option<u16>) -> Result<OllamaInstanceInfo, String> {
    let port = match port {
        Some(p) if is_port_free(p) => p,
        Some(p) => return Err(format!("porta {p} já está em uso")),
        None => next_free_port(DEFAULT_PORT).ok_or("nenhuma porta livre encontrada")?,
    };

    let child = Command::new("ollama")
        .arg("serve")
        .env("OLLAMA_HOST", format!("127.0.0.1:{port}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("falha ao iniciar 'ollama serve' na porta {port}: {e}"))?;

    let id = nanoid!(8);
    let info = OllamaInstanceInfo {
        id: id.clone(),
        port,
        model: model.clone(),
        pid: child.id(),
    };

    let Ok(mut map) = instances().lock() else {
        return Err("estado interno de instâncias indisponível".into());
    };
    map.insert(id, RunningInstance { child, port, model });
    Ok(info)
}

#[tauri::command]
pub fn ollama_stop_instance(id: String) -> Result<(), String> {
    let Ok(mut map) = instances().lock() else {
        return Err("estado interno de instâncias indisponível".into());
    };
    let Some(mut inst) = map.remove(&id) else {
        return Ok(());
    };
    inst.child.kill().map_err(|e| format!("falha ao encerrar instância {id}: {e}"))?;
    let _ = inst.child.wait();
    Ok(())
}

/// Chamado no shutdown do app para não deixar daemons `ollama serve` órfãos.
pub fn kill_all_instances() {
    let Ok(mut map) = instances().lock() else {
        return;
    };
    for (_, mut inst) in map.drain() {
        let _ = inst.child.kill();
        let _ = inst.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

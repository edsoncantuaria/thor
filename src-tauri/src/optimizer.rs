// Instalação dos otimizadores de token para agentes de CLI. Detecção de "já
// instalado?" e versão são feitas pelos comandos genéricos já existentes em
// cli_resolver.rs (find_cli_launcher / agent_cli_version) — aqui só vive o
// que é específico de cada ferramenta: como instalar, e o init do RTK.
//
// Caveman e Headroom são "wrappers": prefixam o comando do agente
// (`caveman claude`, `headroom wrap claude`) — essa transformação vive só no
// frontend (src/lib/tauri/pty.ts, com seu próprio teste), porque é lá que o
// comando final é montado antes de chamar spawn_pty; não duplicamos a lógica
// aqui para não ter duas fontes de verdade que podem sair de sincronia. RTK é
// diferente: `rtk init -g` mexe uma vez na config global do Claude Code e não
// prefixa nada — pode ser usado junto com qualquer um dos dois wrappers.
// Caveman e Headroom não podem ser usados juntos, já que os dois interceptam
// o mesmo mecanismo de spawn.

use std::process::Command;

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("falha ao executar '{program} {}': {e}", args.join(" ")))?;
    if !status.success() {
        return Err(format!("'{program} {}' saiu com status {status}", args.join(" ")));
    }
    Ok(())
}

fn run_shell(script: &str) -> Result<(), String> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(script)
        .status()
        .map_err(|e| format!("falha ao executar instalador: {e}"))?;
    if !status.success() {
        return Err(format!("instalador saiu com status {status}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn optimizer_install_caveman() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        run("npm", &["install", "-g", "@caveman-ai/cli"])?;
        run("caveman", &["setup", "--install"])
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn optimizer_install_rtk() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        run_shell("curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh")
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn optimizer_configure_rtk() -> Result<(), String> {
    tokio::task::spawn_blocking(|| run("rtk", &["init", "-g"]))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn optimizer_install_headroom() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        if run("uv", &["tool", "install", "--python", "3.13", "headroom-ai[all]"]).is_ok() {
            return Ok(());
        }
        run("pip", &["install", "headroom-ai[all]"])
    })
    .await
    .map_err(|e| e.to_string())?
}

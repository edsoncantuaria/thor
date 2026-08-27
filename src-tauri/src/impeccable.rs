//! Project-scoped control for the Impeccable design-QA skill (impeccable.style).
//!
//! Impeccable installs itself per project (`.claude/skills/impeccable/`, `.impeccable/config.json`)
//! rather than as a Thor-wide integration, so every command here takes a `repo_path` and shells out
//! with that as `current_dir` — mirroring how `optimizer.rs` installs global CLIs, except scoped.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

use crate::cli_resolver;

fn run_in(cwd: &str, program: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("failed to run '{}': {error}", program.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "'{} {}' exited with {}: {stderr}",
            program.display(),
            args.join(" "),
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn resolve(command: &str) -> Result<PathBuf, String> {
    cli_resolver::find_windows_cli_launcher(command)
        .ok_or_else(|| format!("{command} not found on PATH — install Node.js first"))
}

fn hook_admin_script(repo_path: &str) -> PathBuf {
    Path::new(repo_path)
        .join(".claude")
        .join("skills")
        .join("impeccable")
        .join("scripts")
        .join("hook-admin.mjs")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpeccableStatus {
    pub installed: bool,
    pub hook_enabled: bool,
    pub ignore_rules: usize,
    pub ignore_files: usize,
    pub ignore_values: usize,
}

#[tauri::command]
pub fn impeccable_status(repo_path: String) -> ImpeccableStatus {
    let installed = hook_admin_script(&repo_path).is_file();

    let config: Option<Value> = std::fs::read_to_string(Path::new(&repo_path).join(".impeccable").join("config.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());

    let count = |key: &str| -> usize {
        config
            .as_ref()
            .and_then(|value| value.get("detector"))
            .and_then(|detector| detector.get(key))
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    };
    let hook_enabled = config
        .as_ref()
        .and_then(|value| value.get("hook"))
        .and_then(|hook| hook.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    ImpeccableStatus {
        installed,
        hook_enabled,
        ignore_rules: count("ignoreRules"),
        ignore_files: count("ignoreFiles"),
        ignore_values: count("ignoreValues"),
    }
}

/// Runs `impeccable install` with `repo_path` as cwd. With no TTY attached (as here), the CLI
/// falls back to its bracketed defaults at every prompt — detected harnesses only, project scope —
/// which is exactly the non-interactive behavior this command wants.
#[tauri::command]
pub async fn impeccable_install(repo_path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let npx = resolve("npx")?;
        run_in(&repo_path, &npx, &["--yes", "impeccable", "install"]).map(|_| ())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn impeccable_set_hook(repo_path: String, enabled: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let node = resolve("node")?;
        let script = hook_admin_script(&repo_path);
        if !script.is_file() {
            return Err("Impeccable is not installed in this project yet".to_string());
        }
        let action = if enabled { "on" } else { "off" };
        let script_str = script.to_string_lossy();
        run_in(&repo_path, &node, &[script_str.as_ref(), action]).map(|_| ())
    })
    .await
    .map_err(|error| error.to_string())?
}

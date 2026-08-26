//!
//! Equivalente ao "Install 'code' command in PATH" do VS Code: escreve um shim

//! `--open-path <dir>` (ver `cli_launch.rs`).
//!

//!

//!
//! | Plataforma      | Caminho                              | PATH                          |
//! |-----------------|--------------------------------------|-------------------------------|

//! | Windows         | `%LOCALAPPDATA%\Thor\bin\thor.cmd` | registrado em `HKCU\Environment` |
//!

use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(windows)]
const SHIM_FILE_NAME: &str = "thor.cmd";
#[cfg(not(windows))]
const SHIM_FILE_NAME: &str = "thor";

#[derive(Serialize, Default)]
pub struct CliShimStatus {
    pub supported: bool,
    pub installed: bool,

    pub stale: bool,

    pub path: Option<String>,
    pub bin_dir: Option<String>,

    pub on_path: bool,
}

fn shim_bin_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let base = dirs_next::data_local_dir()
            .ok_or_else(|| "não foi possível resolver LOCALAPPDATA".to_string())?;
        Ok(base.join("Thor").join("bin"))
    }
    #[cfg(not(windows))]
    {
        let home =
            dirs_next::home_dir().ok_or_else(|| "não foi possível resolver o HOME".to_string())?;
        Ok(home.join(".local").join("bin"))
    }
}

fn shim_path() -> Result<PathBuf, String> {
    Ok(shim_bin_dir()?.join(SHIM_FILE_NAME))
}

fn current_binary() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn app_bundle(binary: &Path) -> Option<PathBuf> {
    binary
        .ancestors()
        .find(|ancestor| ancestor.extension().and_then(|ext| ext.to_str()) == Some("app"))
        .map(|bundle| bundle.to_path_buf())
}

fn dir_on_path(dir: &Path) -> bool {
    if let Some(path_var) = std::env::var_os("PATH") {
        if std::env::split_paths(&path_var).any(|entry| entry == dir) {
            return true;
        }
    }

    #[cfg(windows)]
    {
        return windows_path::user_path_contains(dir);
    }

    #[cfg(target_os = "linux")]
    {
        return linux_path::user_path_contains(dir);
    }

    #[cfg(all(not(windows), not(target_os = "linux")))]
    false
}

#[cfg(not(windows))]
fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(not(windows))]
fn unix_shim_script(target_marker: &str, launch: &str) -> String {
    format!(
        r#"#!/bin/sh
# thor — abre um diretório no Thor a partir do terminal.
#
# Gerado automaticamente pelo Thor (Configurações ▸ Integrações ▸ Comando de
# terminal). Não edite à mão: reinstale por lá, principalmente depois de mover
# ou reinstalar o app.
#
# thor            → abre o diretório atual
# thor .          → idem
# thor ~/projeto  → abre o diretório informado
#
# THOR_TARGET_BIN: {target_marker}

set -e

target=${{1:-.}}

if [ ! -d "$target" ]; then
  echo "thor: diretório não encontrado: $target" >&2
  exit 1
fi

# Caminho absoluto: o app compara com o cwd salvo dos projetos.
target=$(cd "$target" && pwd)

{launch}
"#
    )
}

fn render_shim(binary: &Path) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle) = app_bundle(binary) {
            let quoted = sh_single_quote(&bundle.to_string_lossy());
            return Ok(unix_shim_script(
                &bundle.to_string_lossy(),
                &format!("exec open -na {quoted} --args --open-path \"$target\""),
            ));
        }
        let quoted = sh_single_quote(&binary.to_string_lossy());
        return Ok(unix_shim_script(
            &binary.to_string_lossy(),
            &format!(
                "nohup {quoted} --open-path \"$target\" >/dev/null 2>&1 &\n\
                 exit 0"
            ),
        ));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let quoted = sh_single_quote(&binary.to_string_lossy());
        return Ok(unix_shim_script(
            &binary.to_string_lossy(),
            &format!(
                "nohup {quoted} --open-path \"$target\" >/dev/null 2>&1 &\n\
                 exit 0"
            ),
        ));
    }

    #[cfg(windows)]
    {
        let binary = binary.to_string_lossy().to_string();
        return Ok(format!(
            r#"@echo off
rem thor - abre um diretorio no Thor a partir do terminal.
rem
rem Gerado automaticamente pelo Thor (Configuracoes > Integracoes > Comando de
rem terminal). Nao edite a mao: reinstale por la, principalmente depois de mover
rem ou reinstalar o app.
rem
rem THOR_TARGET_BIN: {binary}

setlocal
set "target=%~1"
if "%target%"=="" set "target=%CD%"

rem Caminho absoluto: o app compara com o cwd salvo dos projetos.
for %%I in ("%target%") do set "target=%%~fI"

if not exist "%target%\" (
  echo thor: diretorio nao encontrado: %target% 1>&2
  exit /b 1
)

start "" "{binary}" --open-path "%target%"
"#
        ));
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = binary;
        Err("plataforma sem suporte a comando de terminal".to_string())
    }
}

fn shim_targets_binary(contents: &str, binary: &Path) -> bool {
    let Some(line) = contents
        .lines()
        .find_map(|line| line.split_once("THOR_TARGET_BIN:"))
        .map(|(_, rest)| rest.trim())
    else {
        return false;
    };
    #[cfg(target_os = "macos")]
    if let Some(bundle) = app_bundle(binary) {
        return line == bundle.to_string_lossy();
    }
    line == binary.to_string_lossy()
}

fn build_status() -> Result<CliShimStatus, String> {
    let bin_dir = shim_bin_dir()?;
    let path = shim_path()?;
    let binary = current_binary()?;

    let contents = std::fs::read_to_string(&path).ok();
    let installed = contents.is_some();
    let stale = contents
        .map(|contents| !shim_targets_binary(&contents, &binary))
        .unwrap_or(false);

    Ok(CliShimStatus {
        supported: cfg!(any(unix, windows)),
        installed,
        stale,
        path: Some(path.to_string_lossy().to_string()),
        bin_dir: Some(bin_dir.to_string_lossy().to_string()),
        on_path: dir_on_path(&bin_dir),
    })
}

#[tauri::command]
pub fn cli_shim_status() -> Result<CliShimStatus, String> {
    build_status()
}

#[tauri::command]
pub fn cli_shim_install() -> Result<CliShimStatus, String> {
    let bin_dir = shim_bin_dir()?;
    let path = shim_path()?;
    let binary = current_binary()?;

    std::fs::create_dir_all(&bin_dir).map_err(|error| error.to_string())?;
    std::fs::write(&path, render_shim(&binary)?).map_err(|error| error.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }

    #[cfg(windows)]
    {
        windows_path::add_to_user_path(&bin_dir)?;
        windows_path::add_to_process_path(&bin_dir);
    }

    #[cfg(target_os = "linux")]
    {
        linux_path::add_to_user_path(&bin_dir)?;
        linux_path::add_to_process_path(&bin_dir);
    }

    build_status()
}

#[tauri::command]
pub fn cli_shim_uninstall() -> Result<CliShimStatus, String> {
    let path = shim_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }

    #[cfg(windows)]
    windows_path::remove_from_user_path(&shim_bin_dir()?)?;

    #[cfg(target_os = "linux")]
    linux_path::remove_from_user_path(&shim_bin_dir()?)?;

    build_status()
}

/// Linux user-PATH via systemd `environment.d` (picked up by new graphical
/// sessions). Mirrors the Windows HKCU Path registration.
#[cfg(target_os = "linux")]
mod linux_path {
    use std::path::{Path, PathBuf};

    const MARKER: &str = "# THOR_CLI_SHIM_PATH";
    const FILE_NAME: &str = "90-thor-bin.conf";

    fn config_path() -> Result<PathBuf, String> {
        let home =
            dirs_next::home_dir().ok_or_else(|| "could not resolve HOME".to_string())?;
        Ok(home.join(".config").join("environment.d").join(FILE_NAME))
    }

    pub(super) fn render_conf(bin_dir: &Path) -> String {
        let dir = bin_dir.to_string_lossy();
        format!(
            "{MARKER}\n\
             # Managed by Thor (Settings ▸ Integrations ▸ Terminal command).\n\
             # Do not edit by hand — reinstall or uninstall from there.\n\
             PATH=\"{dir}:$PATH\"\n"
        )
    }

    pub(super) fn is_ours(contents: &str) -> bool {
        contents.lines().any(|line| line.trim() == MARKER)
    }

    pub(super) fn conf_lists_dir(contents: &str, dir: &Path) -> bool {
        let needle = dir.to_string_lossy();
        contents.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("PATH=") && trimmed.contains(needle.as_ref())
        })
    }

    pub fn user_path_contains(dir: &Path) -> bool {
        let Ok(path) = config_path() else {
            return false;
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            return false;
        };
        is_ours(&contents) && conf_lists_dir(&contents, dir)
    }

    pub fn add_to_user_path(dir: &Path) -> Result<(), String> {
        if user_path_contains(dir) {
            return Ok(());
        }
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, render_conf(dir)).map_err(|error| error.to_string())
    }

    pub fn add_to_process_path(dir: &Path) {
        let Some(current) = std::env::var_os("PATH") else {
            std::env::set_var("PATH", dir);
            return;
        };
        if std::env::split_paths(&current).any(|entry| entry == dir) {
            return;
        }
        let mut paths = vec![dir.to_path_buf()];
        paths.extend(std::env::split_paths(&current));
        if let Ok(joined) = std::env::join_paths(paths) {
            std::env::set_var("PATH", joined);
        }
    }

    pub fn remove_from_user_path(_dir: &Path) -> Result<(), String> {
        let path = config_path()?;
        match std::fs::read_to_string(&path) {
            Ok(contents) if is_ours(&contents) => {
                std::fs::remove_file(&path).map_err(|error| error.to_string())
            }
            Ok(_) => Ok(()), // foreign file — leave it alone
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

///

/// `REG_EXPAND_SZ` (comum, with entries like `%USERPROFILE%\bin`)
#[cfg(windows)]
mod windows_path {
    use std::path::Path;

    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::types::FromRegValue;
    use winreg::{RegKey, RegValue};

    fn open_environment() -> Result<RegKey, String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .map_err(|error| error.to_string())
    }

    fn read_path(key: &RegKey) -> (String, RegType) {
        match key.get_raw_value("Path") {
            Ok(value) => {
                let vtype = value.vtype.clone();
                let text = String::from_reg_value(&value).unwrap_or_default();
                (text, vtype)
            }
            Err(_) => (String::new(), RegType::REG_EXPAND_SZ),
        }
    }

    fn write_path(key: &RegKey, value: &str, vtype: RegType) -> Result<(), String> {
        let mut bytes: Vec<u8> = value
            .encode_utf16()
            .flat_map(|unit| unit.to_ne_bytes())
            .collect();
        // Terminador UTF-16 — o registro guarda string larga com NUL final.
        bytes.extend_from_slice(&[0, 0]);
        key.set_raw_value("Path", &RegValue { bytes, vtype })
            .map_err(|error| error.to_string())
    }

    fn entries(path: &str) -> Vec<&str> {
        path.split(';')
            .filter(|entry| !entry.trim().is_empty())
            .collect()
    }

    pub fn user_path_contains(dir: &Path) -> bool {
        let Ok(key) = open_environment() else {
            return false;
        };
        let (current, _) = read_path(&key);
        let target = dir.to_string_lossy().to_string();
        entries(&current).iter().any(|entry| {
            entry
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(target.trim_end_matches('\\'))
        })
    }

    pub fn add_to_user_path(dir: &Path) -> Result<(), String> {
        let key = open_environment()?;
        let (current, vtype) = read_path(&key);
        let target = dir.to_string_lossy().to_string();

        if entries(&current).iter().any(|entry| {
            entry
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(target.trim_end_matches('\\'))
        }) {
            return Ok(());
        }

        let updated = if current.trim().is_empty() {
            target
        } else {
            format!("{};{}", current.trim_end_matches(';'), target)
        };
        write_path(&key, &updated, vtype)?;
        broadcast_environment_change();
        Ok(())
    }

    pub fn add_to_process_path(dir: &Path) {
        let Some(current) = std::env::var_os("PATH") else {
            std::env::set_var("PATH", dir);
            return;
        };
        if std::env::split_paths(&current).any(|entry| {
            entry
                .to_string_lossy()
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(dir.to_string_lossy().trim_end_matches('\\'))
        }) {
            return;
        }
        let updated = format!("{};{}", current.to_string_lossy(), dir.to_string_lossy());
        std::env::set_var("PATH", updated);
    }

    pub fn remove_from_user_path(dir: &Path) -> Result<(), String> {
        let key = open_environment()?;
        let (current, vtype) = read_path(&key);
        let target = dir.to_string_lossy().to_string();
        let target = target.trim_end_matches('\\');

        let all = entries(&current);
        let kept: Vec<&str> = all
            .iter()
            .copied()
            .filter(|entry| !entry.trim_end_matches('\\').eq_ignore_ascii_case(target))
            .collect();

        if kept.len() == all.len() {
            return Ok(());
        }
        write_path(&key, &kept.join(";"), vtype)?;
        broadcast_environment_change();
        Ok(())
    }

    fn broadcast_environment_change() {
        use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };

        let param: Vec<u16> = "Environment\0".encode_utf16().collect();
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST as HWND,
                WM_SETTINGCHANGE,
                0 as WPARAM,
                param.as_ptr() as LPARAM,
                SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_mentions_target_binary() {
        let binary = PathBuf::from("/opt/alethe/alethe");
        let script = render_shim(&binary).expect("script");
        assert!(shim_targets_binary(&script, &binary));
    }

    #[test]
    fn stale_shim_is_detected() {
        let old = PathBuf::from("/opt/alethe-antigo/alethe");
        let script = render_shim(&old).expect("script");
        assert!(!shim_targets_binary(
            &script,
            Path::new("/opt/alethe-novo/alethe")
        ));
    }

    #[test]
    fn shim_without_marker_counts_as_stale() {
        assert!(!shim_targets_binary(
            "#!/bin/sh\necho oi\n",
            Path::new("/opt/alethe/alethe")
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn shim_defaults_to_current_dir_and_is_a_posix_script() {
        let script = render_shim(Path::new("/opt/alethe/alethe")).expect("script");
        assert!(script.starts_with("#!/bin/sh"));

        assert!(script.contains("target=${1:-.}"));
        assert!(script.contains("--open-path"));
    }

    #[cfg(not(windows))]
    #[test]
    fn generated_shim_is_valid_shell_syntax() {
        use std::io::Write;
        use std::process::Command;

        for binary in [
            "/opt/alethe/alethe",
            "/Applications/Thor.app/Contents/MacOS/Thor",
        ] {
            let script = render_shim(Path::new(binary)).expect("script");
            let file = std::env::temp_dir().join(format!(
                "alethe-shim-syntax-{}.sh",
                binary.replace(['/', '.'], "_")
            ));
            std::fs::File::create(&file)
                .and_then(|mut handle| handle.write_all(script.as_bytes()))
                .expect("escrever shim");

            let output = Command::new("sh")
                .arg("-n")
                .arg(&file)
                .output()
                .expect("rodar sh -n");
            let _ = std::fs::remove_file(&file);

            assert!(
                output.status.success(),
                "shim inválido para {binary}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn single_quotes_in_path_are_escaped() {
        let script = render_shim(Path::new("/opt/alethe's app/alethe")).expect("script");

        assert!(script.contains(r"'\''"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn environment_d_conf_has_marker_and_path() {
        let dir = Path::new("/home/user/.local/bin");
        let conf = linux_path::render_conf(dir);
        assert!(linux_path::is_ours(&conf));
        assert!(linux_path::conf_lists_dir(&conf, dir));
        assert!(conf.contains("PATH=\"/home/user/.local/bin:$PATH\""));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn environment_d_uninstall_only_removes_ours() {
        assert!(linux_path::is_ours("# THOR_CLI_SHIM_PATH\nPATH=\"/x:$PATH\"\n"));
        assert!(!linux_path::is_ours("PATH=\"/home/user/.local/bin:$PATH\"\n"));
    }
}

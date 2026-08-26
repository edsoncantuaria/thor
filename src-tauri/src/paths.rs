use std::ffi::OsString;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const PROFILES_DIR_NAME: &str = "profiles";

pub(crate) fn env_os_prefer_thor(thor_key: &str, alethe_key: &str) -> Option<OsString> {
    std::env::var_os(thor_key)
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os(alethe_key).filter(|value| !value.is_empty()))
}

pub(crate) fn env_var_prefer_thor(thor_key: &str, alethe_key: &str) -> Option<String> {
    env_os_prefer_thor(thor_key, alethe_key).map(|value| value.to_string_lossy().into_owned())
}

pub(crate) fn root_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(override_dir) = env_var_prefer_thor("THOR_APP_DATA_DIR", "ALETHE_APP_DATA_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

///

pub fn profile_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let root = root_data_dir(app)?;
    let index = crate::profiles::ensure_profiles_index(app)?;
    Ok(root.join(PROFILES_DIR_NAME).join(&index.active_profile_id))
}

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    profile_data_dir(app)
}

pub fn scrollback_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(profile_data_dir(app)?.join("scrollback"))
}

pub fn scrollback_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(scrollback_dir(app)?.join(format!("{id}.bin")))
}

pub fn projects_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(profile_data_dir(app)?.join("projects.json"))
}

pub fn activity_stats_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(profile_data_dir(app)?.join("activity-stats.json"))
}

pub fn spawn_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(profile_data_dir(app)?.join("spawn.log"))
}

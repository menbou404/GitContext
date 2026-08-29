use std::{fs, path::PathBuf};

use tauri::{AppHandle, Manager};

use crate::models::AppData;

pub fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join("state.json"))
        .map_err(|error| format!("Could not resolve GitContext's settings directory: {error}"))
}

pub fn load(app: &AppHandle) -> Result<AppData, String> {
    let path = state_path(app)?;
    let backup = path.with_extension("json.backup");
    if !path.exists() && backup.exists() {
        fs::rename(&backup, &path)
            .map_err(|error| format!("Could not recover GitContext settings: {error}"))?;
    }
    if !path.exists() {
        let initial = AppData::default();
        save(app, &initial)?;
        return Ok(initial);
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("GitContext settings are invalid JSON: {error}"))
}

pub fn save(app: &AppHandle, data: &AppData) -> Result<(), String> {
    let path = state_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "The settings path has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(data)
        .map_err(|error| format!("Could not serialize GitContext settings: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.backup");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not stage {}: {error}", temporary.display()))?;

    if backup.exists() {
        fs::remove_file(&backup)
            .map_err(|error| format!("Could not clear an old settings backup: {error}"))?;
    }
    if path.exists() {
        fs::rename(&path, &backup)
            .map_err(|error| format!("Could not back up existing settings: {error}"))?;
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        if backup.exists() {
            let _ = fs::rename(&backup, &path);
        }
        return Err(format!("Could not activate the new settings file: {error}"));
    }
    if backup.exists() {
        fs::remove_file(&backup)
            .map_err(|error| format!("Settings were saved, but their temporary backup could not be removed: {error}"))?;
    }
    Ok(())
}

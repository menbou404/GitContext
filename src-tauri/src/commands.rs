use std::{process::Command, sync::Mutex};

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::{
    git_ops,
    models::{
        normalize_profile, validate_profile, AppData, ApplyPreview, BootstrapResult,
        EnvironmentStatus, Profile, RepositoryRecord, ToolStatus,
    },
    storage,
};

pub struct AppGate(pub Mutex<()>);

#[tauri::command]
pub fn bootstrap(app: AppHandle, gate: State<'_, AppGate>) -> Result<BootstrapResult, String> {
    let _guard = gate.0.lock().map_err(|_| "GitContext's state lock is unavailable.")?;
    let data = storage::load(&app)?;
    let storage_path = storage::state_path(&app)?.to_string_lossy().into_owned();
    Ok(BootstrapResult {
        data,
        environment: environment_status(),
        storage_path: Some(storage_path),
        demo_mode: false,
    })
}

#[tauri::command]
pub fn save_profile(
    app: AppHandle,
    gate: State<'_, AppGate>,
    profile: Profile,
) -> Result<AppData, String> {
    let _guard = gate.0.lock().map_err(|_| "GitContext's state lock is unavailable.")?;
    let mut profile = normalize_profile(profile);
    if profile.id.trim().is_empty() {
        profile.id = uuid::Uuid::new_v4().to_string();
    }
    validate_profile(&profile)?;

    let mut data = storage::load(&app)?;
    if let Some(existing) = data.profiles.iter_mut().find(|item| item.id == profile.id) {
        *existing = profile;
    } else {
        data.profiles.push(profile);
    }
    storage::save(&app, &data)?;
    Ok(data)
}

#[tauri::command]
pub fn add_repository(
    app: AppHandle,
    gate: State<'_, AppGate>,
    path: String,
) -> Result<RepositoryRecord, String> {
    let _guard = gate.0.lock().map_err(|_| "GitContext's state lock is unavailable.")?;
    let candidate = git_ops::inspect_repository(&path)?;
    let mut data = storage::load(&app)?;
    if let Some(existing) = data.repositories.iter().find(|item| item.path == candidate.path) {
        return Ok(existing.clone());
    }
    data.repositories.push(candidate.clone());
    storage::save(&app, &data)?;
    Ok(candidate)
}

#[tauri::command]
pub fn remove_repository(
    app: AppHandle,
    gate: State<'_, AppGate>,
    id: String,
) -> Result<AppData, String> {
    let _guard = gate.0.lock().map_err(|_| "GitContext's state lock is unavailable.")?;
    let mut data = storage::load(&app)?;
    data.repositories.retain(|repository| repository.id != id);
    storage::save(&app, &data)?;
    Ok(data)
}

#[tauri::command]
pub fn preview_assignment(
    app: AppHandle,
    gate: State<'_, AppGate>,
    repository_id: String,
    profile_id: String,
) -> Result<ApplyPreview, String> {
    let _guard = gate.0.lock().map_err(|_| "GitContext's state lock is unavailable.")?;
    let data = storage::load(&app)?;
    let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
    validate_profile(profile)?;
    let gh_available = command_available("gh");
    let mut preview = git_ops::build_preview(repository, profile, gh_available)?;
    if gh_available {
        if let Some(directory) = profile.gh_config_dir.as_deref().filter(|path| std::path::Path::new(path).is_dir()) {
            let status = Command::new("gh")
                .current_dir(&repository.path)
                .env("GH_CONFIG_DIR", directory)
                .args(["auth", "status", "--active", "--hostname", "github.com"])
                .output();
            match status {
                Ok(output) if output.status.success() => {}
                Ok(_) => preview.warnings.push(
                    "The selected gh config directory has no active github.com authentication.".into(),
                ),
                Err(error) => preview.warnings.push(format!("Could not inspect the gh profile: {error}")),
            }
        }
    }
    Ok(preview)
}

#[tauri::command]
pub fn apply_profile(
    app: AppHandle,
    gate: State<'_, AppGate>,
    repository_id: String,
    profile_id: String,
) -> Result<AppData, String> {
    let _guard = gate.0.lock().map_err(|_| "GitContext's state lock is unavailable.")?;
    let mut data = storage::load(&app)?;
    let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
    validate_profile(profile)?;
    git_ops::apply_profile(repository, profile)?;

    let target = data
        .repositories
        .iter_mut()
        .find(|item| item.id == repository_id)
        .ok_or_else(|| "Repository was not found.".to_string())?;
    target.profile_id = Some(profile_id);
    target.last_applied_at = Some(Utc::now().to_rfc3339());
    storage::save(&app, &data)?;
    Ok(data)
}

fn find_assignment<'a>(
    data: &'a AppData,
    repository_id: &str,
    profile_id: &str,
) -> Result<(&'a RepositoryRecord, &'a Profile), String> {
    let repository = data
        .repositories
        .iter()
        .find(|item| item.id == repository_id)
        .ok_or_else(|| "Repository was not found.".to_string())?;
    let profile = data
        .profiles
        .iter()
        .find(|item| item.id == profile_id)
        .ok_or_else(|| "Profile was not found.".to_string())?;
    Ok((repository, profile))
}

fn environment_status() -> EnvironmentStatus {
    let git = probe_tool("git", &["--version"]);
    let gh = probe_tool("gh", &["--version"]);
    let ssh = probe_tool("ssh", &["-V"]);
    let ssh_directory = git_ops::home_directory()
        .map(|home| home.join(".ssh"))
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned());
    EnvironmentStatus {
        git,
        gh,
        ssh,
        ssh_directory,
    }
}

fn probe_tool(program: &str, arguments: &[&str]) -> ToolStatus {
    match Command::new(program).args(arguments).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let first_line = if stdout.is_empty() { stderr } else { stdout }
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            ToolStatus {
                available: true,
                version: (!first_line.is_empty()).then_some(first_line),
                detail: None,
            }
        }
        Ok(output) => ToolStatus {
            available: false,
            version: None,
            detail: Some(format!("{program} exited with {}", output.status)),
        },
        Err(error) => ToolStatus {
            available: false,
            version: None,
            detail: Some(format!("{program} was not found: {error}")),
        },
    }
}

fn command_available(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok_and(|output| output.status.success())
}

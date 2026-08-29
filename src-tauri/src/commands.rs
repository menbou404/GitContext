use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::{
    git_ops,
    models::{
        normalize_profile, validate_profile, validate_profile_id, AppData, ApplyPreview,
        BootstrapResult, EnvironmentStatus, GhProfileStatus, Profile, RepositoryRecord, ToolStatus,
    },
    storage,
};

pub struct AppGate(pub Mutex<()>);

#[tauri::command]
pub fn bootstrap(app: AppHandle, gate: State<'_, AppGate>) -> Result<BootstrapResult, String> {
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "GitContext's state lock is unavailable.")?;
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
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "GitContext's state lock is unavailable.")?;
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
pub fn inspect_github_profile(
    app: AppHandle,
    profile_id: String,
    gh_config_dir: Option<String>,
) -> Result<GhProfileStatus, String> {
    let directory = resolve_gh_config_dir(&app, &profile_id, gh_config_dir.as_deref())?;
    Ok(inspect_gh_directory(&directory))
}

#[tauri::command]
pub async fn connect_github_profile(
    app: AppHandle,
    profile_id: String,
    gh_config_dir: Option<String>,
) -> Result<GhProfileStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !command_available("gh") {
            return Err("GitHub CLI is not installed.".into());
        }

        let directory = resolve_gh_config_dir(&app, &profile_id, gh_config_dir.as_deref())?;
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Could not create the gh profile directory: {error}"))?;

        let status = gh_command(&directory)
            .args([
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--git-protocol",
                "ssh",
                "--web",
                "--clipboard",
                "--skip-ssh-key",
            ])
            .status()
            .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
        if !status.success() {
            return Err("GitHub CLI login did not complete.".into());
        }

        let result = inspect_gh_directory(&directory);
        if result.authenticated {
            Ok(result)
        } else {
            Err(result
                .detail
                .unwrap_or_else(|| "GitHub authentication could not be verified.".into()))
        }
    })
    .await
    .map_err(|error| format!("GitHub connection task failed: {error}"))?
}

#[tauri::command]
pub fn add_repository(
    app: AppHandle,
    gate: State<'_, AppGate>,
    path: String,
) -> Result<RepositoryRecord, String> {
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "GitContext's state lock is unavailable.")?;
    let candidate = git_ops::inspect_repository(&path)?;
    let mut data = storage::load(&app)?;
    if let Some(existing) = data
        .repositories
        .iter()
        .find(|item| item.path == candidate.path)
    {
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
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "GitContext's state lock is unavailable.")?;
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
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "GitContext's state lock is unavailable.")?;
    let data = storage::load(&app)?;
    let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
    validate_profile(profile)?;
    let gh_available = command_available("gh");
    let mut preview = git_ops::build_preview(repository, profile, gh_available)?;
    if gh_available {
        if let Some(directory) = profile
            .gh_config_dir
            .as_deref()
            .filter(|path| std::path::Path::new(path).is_dir())
        {
            let status = inspect_gh_directory(Path::new(directory));
            if !status.authenticated {
                preview.warnings.push(status.detail.unwrap_or_else(|| {
                    "The selected gh config directory has no active github.com authentication."
                        .into()
                }));
            } else if let (Some(expected), Some(actual)) = (
                profile.github_username.as_deref(),
                status.username.as_deref(),
            ) {
                if !expected.eq_ignore_ascii_case(actual) {
                    preview.warnings.push(format!(
                        "The Profile expects @{expected}, but GitHub CLI is authenticated as @{actual}."
                    ));
                }
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
    let _guard = gate
        .0
        .lock()
        .map_err(|_| "GitContext's state lock is unavailable.")?;
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
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn resolve_gh_config_dir(
    app: &AppHandle,
    profile_id: &str,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    validate_profile_id(profile_id)?;
    let directory = match requested.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => storage::state_path(app)?
            .parent()
            .ok_or_else(|| "The settings path has no parent directory.".to_string())?
            .join("gh")
            .join(profile_id),
    };
    if !directory.is_absolute()
        || directory
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("The gh config directory must be an absolute path without '..'.".into());
    }
    let home = git_ops::home_directory()
        .ok_or_else(|| "Could not locate the user home directory.".to_string())?;
    if directory == home || !directory.starts_with(&home) {
        return Err("The gh config directory must be inside the user home directory.".into());
    }
    Ok(directory)
}

fn gh_command(directory: &Path) -> Command {
    let mut command = Command::new("gh");
    command
        .env("GH_CONFIG_DIR", directory)
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_ENTERPRISE_TOKEN")
        .env_remove("GITHUB_ENTERPRISE_TOKEN");
    command
}

fn inspect_gh_directory(directory: &Path) -> GhProfileStatus {
    let config_dir = Some(directory.to_string_lossy().into_owned());
    if !command_available("gh") {
        return GhProfileStatus {
            available: false,
            authenticated: false,
            username: None,
            detail: Some("GitHub CLI is not installed.".into()),
            config_dir,
        };
    }
    if !directory.is_dir() {
        return GhProfileStatus {
            available: true,
            authenticated: false,
            username: None,
            detail: Some("This Profile has not been connected to GitHub yet.".into()),
            config_dir,
        };
    }

    match gh_command(directory)
        .args(["api", "user", "--hostname", "github.com", "--jq", ".login"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
            GhProfileStatus {
                available: true,
                authenticated: !username.is_empty(),
                username: (!username.is_empty()).then_some(username),
                detail: None,
                config_dir,
            }
        }
        Ok(_) => GhProfileStatus {
            available: true,
            authenticated: false,
            username: None,
            detail: Some("No authenticated GitHub account was found in this gh Profile.".into()),
            config_dir,
        },
        Err(error) => GhProfileStatus {
            available: true,
            authenticated: false,
            username: None,
            detail: Some(format!("Could not inspect the GitHub account: {error}")),
            config_dir,
        },
    }
}

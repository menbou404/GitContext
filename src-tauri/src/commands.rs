use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    git_ops,
    models::{
        normalize_profile, validate_profile, validate_profile_id, AppData, ApplyPreview,
        BootstrapResult, EnvironmentStatus, GhProfileStatus, Profile, PublishResult,
        RepositoryRecord, ToolStatus,
    },
    storage,
};

pub struct AppGate(pub Mutex<()>);

const GITHUB_AUTH_PROMPT_EVENT: &str = "github-auth-prompt";
const GITHUB_DEVICE_URL: &str = "https://github.com/login/device";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubAuthPrompt {
    profile_id: String,
    code: String,
    verification_url: &'static str,
}

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

        let mut command = gh_command(&directory);
        command
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;

        let prompt_sent = Arc::new(AtomicBool::new(false));
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(spawn_github_auth_reader(
                stdout,
                app.clone(),
                profile_id.clone(),
                prompt_sent.clone(),
            ));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(spawn_github_auth_reader(
                stderr,
                app.clone(),
                profile_id,
                prompt_sent,
            ));
        }

        let status = child
            .wait()
            .map_err(|error| format!("Could not wait for GitHub CLI: {error}"))?;
        for reader in readers {
            let _ = reader.join();
        }
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
pub fn open_github_auth_page() -> Result<(), String> {
    open_github_device_page()
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

#[tauri::command]
pub async fn publish_repository(
    app: AppHandle,
    gate: State<'_, AppGate>,
    repository_id: String,
    profile_id: String,
    name: String,
    visibility: String,
    description: Option<String>,
) -> Result<PublishResult, String> {
    let name = validate_github_repository_name(&name)?;
    let description = validate_github_description(description)?;
    let visibility_flag = match visibility.as_str() {
        "private" => "--private",
        "public" => "--public",
        _ => return Err("Repository visibility must be private or public.".into()),
    };

    let (repository, profile) = {
        let _guard = gate
            .0
            .lock()
            .map_err(|_| "GitContext's state lock is unavailable.")?;
        let data = storage::load(&app)?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        if repository.profile_id.as_deref() != Some(profile_id.as_str())
            || repository.last_applied_at.is_none()
        {
            return Err("Apply this Profile to the repository before publishing.".into());
        }
        validate_profile(profile)?;
        (repository.clone(), profile.clone())
    };

    git_ops::validate_publish_source(&repository)?;
    let directory = resolve_gh_config_dir(&app, &profile.id, profile.gh_config_dir.as_deref())?;
    let gh_status = inspect_gh_directory(&directory);
    let owner = gh_status
        .username
        .filter(|_| gh_status.authenticated)
        .ok_or_else(|| {
            gh_status
                .detail
                .unwrap_or_else(|| "Connect this Profile to GitHub before publishing.".into())
        })?;
    if let Some(expected) = profile.github_username.as_deref() {
        if !expected.eq_ignore_ascii_case(&owner) {
            return Err(format!(
                "The Profile expects @{expected}, but GitHub CLI is authenticated as @{owner}."
            ));
        }
    }

    let full_name = format!("{owner}/{name}");
    let source = repository.path.clone();
    let command_full_name = full_name.clone();
    let command_directory = directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut command = gh_command(&command_directory);
        command.current_dir(&source).args([
            "repo",
            "create",
            command_full_name.as_str(),
            visibility_flag,
            "--source",
            source.as_str(),
            "--remote",
            "origin",
            "--push",
        ]);
        if let Some(description) = description {
            command.args(["--description", description.as_str()]);
        }
        let output = command
            .output()
            .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!("GitHub CLI exited with {}.", output.status)
        } else {
            detail
        })
    })
    .await
    .map_err(|error| format!("GitHub publish task failed: {error}"))??;

    let remote_url = git_ops::origin_url(&repository.path)?;
    let data = {
        let _guard = gate
            .0
            .lock()
            .map_err(|_| "GitContext's state lock is unavailable.")?;
        let mut data = storage::load(&app)?;
        let target = data
            .repositories
            .iter_mut()
            .find(|item| item.id == repository_id)
            .ok_or_else(|| "Repository was not found.".to_string())?;
        target.remote_url = Some(remote_url);
        storage::save(&app, &data)?;
        data
    };
    Ok(PublishResult {
        data,
        repository_url: format!("https://github.com/{full_name}"),
    })
}

fn validate_github_repository_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 100 || matches!(value, "." | "..") {
        return Err("GitHub repository name must contain 1 to 100 characters.".into());
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err("GitHub repository name contains unsupported characters.".into());
    }
    Ok(value.to_string())
}

fn validate_github_description(value: Option<String>) -> Result<Option<String>, String> {
    let value = value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty());
    if let Some(description) = value.as_deref() {
        if description.len() > 350 || description.chars().any(char::is_control) {
            return Err(
                "GitHub repository description is too long or contains control characters.".into(),
            );
        }
    }
    Ok(value)
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

fn spawn_github_auth_reader<R: Read + Send + 'static>(
    stream: R,
    app: AppHandle,
    profile_id: String,
    prompt_sent: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let Some(code) = extract_github_device_code(&line) else {
                continue;
            };
            if prompt_sent.swap(true, Ordering::SeqCst) {
                continue;
            }

            let prompt = GithubAuthPrompt {
                profile_id,
                code,
                verification_url: GITHUB_DEVICE_URL,
            };
            let _ = app.emit(GITHUB_AUTH_PROMPT_EVENT, prompt);
            let _ = open_github_device_page();
            break;
        }
    })
}

fn extract_github_device_code(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|part| {
        let candidate = part
            .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '-');
        let bytes = candidate.as_bytes();
        (bytes.len() == 9
            && bytes[4] == b'-'
            && bytes.iter().enumerate().all(|(index, byte)| {
                index == 4 || byte.is_ascii_uppercase() || byte.is_ascii_digit()
            }))
        .then(|| candidate.to_string())
    })
}

#[cfg(target_os = "windows")]
fn open_github_device_page() -> Result<(), String> {
    Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", GITHUB_DEVICE_URL])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the GitHub authentication page: {error}"))
}

#[cfg(target_os = "macos")]
fn open_github_device_page() -> Result<(), String> {
    Command::new("open")
        .arg(GITHUB_DEVICE_URL)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the GitHub authentication page: {error}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_github_device_page() -> Result<(), String> {
    Command::new("xdg-open")
        .arg(GITHUB_DEVICE_URL)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the GitHub authentication page: {error}"))
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

#[cfg(test)]
mod tests {
    use super::{extract_github_device_code, validate_github_repository_name};

    #[test]
    fn extracts_github_device_code_without_logging_the_line() {
        assert_eq!(
            extract_github_device_code("! One-time code (B13B-F49A) copied to clipboard"),
            Some("B13B-F49A".into())
        );
        assert_eq!(extract_github_device_code("authentication failed"), None);
    }

    #[test]
    fn validates_github_repository_names() {
        assert_eq!(
            validate_github_repository_name(" GitManager ").unwrap(),
            "GitManager"
        );
        assert!(validate_github_repository_name("owner/repository").is_err());
        assert!(validate_github_repository_name("unsafe name").is_err());
    }
}

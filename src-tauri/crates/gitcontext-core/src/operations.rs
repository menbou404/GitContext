use crate::{
    environment::*,
    git_ops,
    github::*,
    models::{
        normalize_profile, validate_profile, AiNotifications, AppData, AppSettings, ApplyPreview,
        AutoApprove, AutoApproveSource, BootstrapResult, BranchResult, CloneResult, CommitPreview,
        CommitResult, GhProfileStatus, GithubRepository, MergePullRequestResult,
        NewRepositoryDefaults, Profile, ProfileAutoApprove, PublishResult, PullRequestManagement,
        PullRequestPreview, PullRequestResult, PushPreview, PushResult, RepositoryRecord,
        SyncPreview,
    },
    storage::{development_data, StateStore},
};
use chrono::Utc;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{atomic::AtomicBool, Arc},
};

pub fn bootstrap(store: &StateStore) -> Result<BootstrapResult, String> {
    let _guard = store.lock()?;
    let data = store.load()?;
    let storage_path = store.state_path().to_string_lossy().into_owned();
    Ok(BootstrapResult {
        data,
        environment: environment_status(),
        storage_path: Some(storage_path),
        demo_mode: false,
        development_data: development_data(),
    })
}

pub fn set_locale(store: &StateStore, locale: String) -> Result<AppSettings, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    if matches!(locale.as_str(), "ja" | "en")
        && data.settings.locale.as_deref() != Some(locale.as_str())
    {
        data.settings.locale = Some(locale);
        store.save(&data)?;
    }
    Ok(data.settings)
}

pub fn set_close_to_tray(store: &StateStore, close_to_tray: bool) -> Result<AppSettings, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    if data.settings.close_to_tray != close_to_tray {
        data.settings.close_to_tray = close_to_tray;
        store.save(&data)?;
    }
    Ok(data.settings)
}

pub fn set_gui_confirmation(store: &StateStore, enabled: bool) -> Result<AppSettings, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    data.settings.gui_confirmation = enabled;
    store.save(&data)?;
    Ok(data.settings)
}

pub fn set_ai_notifications(
    store: &StateStore,
    value: AiNotifications,
) -> Result<AppSettings, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    if data.settings.ai_notifications != value {
        data.settings.ai_notifications = value;
        store.save(&data)?;
    }
    Ok(data.settings)
}

pub fn set_status_notifications(store: &StateStore, enabled: bool) -> Result<AppSettings, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    if data.settings.status_notifications != enabled {
        data.settings.status_notifications = enabled;
        store.save(&data)?;
    }
    Ok(data.settings)
}

pub fn dismiss_ai_integration_notice(store: &StateStore) -> Result<AppSettings, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    if !data.settings.ai_integration_notice_dismissed {
        data.settings.ai_integration_notice_dismissed = true;
        store.save(&data)?;
    }
    Ok(data.settings)
}

pub fn save_profile(store: &StateStore, profile: Profile) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut profile = normalize_profile(profile);
    if profile.id.trim().is_empty() {
        profile.id = uuid::Uuid::new_v4().to_string();
    }
    validate_profile(&profile)?;

    let mut data = store.load()?;
    if let Some(existing) = data.profiles.iter_mut().find(|item| item.id == profile.id) {
        profile.auto_approve = existing.auto_approve.clone();
        *existing = profile;
    } else {
        data.profiles.push(profile);
    }
    store.save(&data)?;
    Ok(data)
}

pub fn inspect_github_profile(
    store: &StateStore,
    profile_id: String,
    gh_config_dir: Option<String>,
) -> Result<GhProfileStatus, String> {
    let directory = resolve_gh_config_dir(store, &profile_id, gh_config_dir.as_deref())?;
    Ok(inspect_gh_directory(&directory))
}

pub fn connect_github_profile(
    store: &StateStore,
    on_prompt: std::sync::Arc<dyn Fn(GithubAuthPrompt) + Send + Sync>,
    profile_id: String,
    gh_config_dir: Option<String>,
) -> Result<GhProfileStatus, String> {
    if !command_available("gh") {
        return Err("GitHub CLI is not installed.".into());
    }

    let directory = resolve_gh_config_dir(store, &profile_id, gh_config_dir.as_deref())?;
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
            on_prompt.clone(),
            profile_id.clone(),
            prompt_sent.clone(),
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_github_auth_reader(
            stderr,
            on_prompt.clone(),
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
}

pub fn open_github_auth_page() -> Result<(), String> {
    open_github_device_page()
}

pub fn add_repository(store: &StateStore, path: String) -> Result<RepositoryRecord, String> {
    let _guard = store.lock()?;
    let candidate = git_ops::inspect_repository(&path)?;
    let mut data = store.load()?;
    if let Some(existing) = data
        .repositories
        .iter()
        .find(|item| item.path == candidate.path)
    {
        return Ok(existing.clone());
    }
    data.repositories.push(candidate.clone());
    store.save(&data)?;
    Ok(candidate)
}

pub fn set_repository_auto_approve(
    store: &StateStore,
    repository_id: String,
    auto_approve: AutoApprove,
) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let repository = data
        .repositories
        .iter_mut()
        .find(|item| item.id == repository_id)
        .ok_or_else(|| "Repository was not found.".to_string())?;
    repository.auto_approve = auto_approve;
    repository.auto_approve_source = None;
    store.save(&data)?;
    Ok(data)
}

pub fn set_profile_auto_approve(
    store: &StateStore,
    profile_id: String,
    auto_approve: ProfileAutoApprove,
) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let profile = data
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id)
        .ok_or_else(|| "Profile was not found.".to_string())?;
    profile.auto_approve.clone_repository = auto_approve.clone_repository;
    store.save(&data)?;
    Ok(data)
}

pub fn set_new_repository_defaults(
    store: &StateStore,
    profile_id: String,
    defaults: NewRepositoryDefaults,
) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let profile = data
        .profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or("Profile was not found.")?;
    profile.auto_approve.new_repository = defaults;
    store.save(&data)?;
    Ok(data)
}

fn path_parts(path: &str) -> Vec<String> {
    let path = path.replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let windows = path.as_bytes().get(1) == Some(&b':') || path.starts_with("//");
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(|part| {
            if cfg!(windows) || windows {
                part.to_lowercase()
            } else {
                part.to_string()
            }
        })
        .collect()
}

fn path_within(path: &str, folder: &str) -> bool {
    let child = path_parts(path);
    let parent = path_parts(folder);
    child.len() >= parent.len() && child.starts_with(&parent)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderRule {
    None,
    Profile(String),
    Ambiguous,
}

pub fn folder_rule(path: &str, profiles: &[Profile]) -> FolderRule {
    let mut best = 0;
    let mut matches = Vec::new();
    for profile in profiles {
        for folder in &profile.auto_approve.new_repository_folders {
            let depth = path_parts(folder).len();
            if depth > 0 && path_within(path, folder) {
                if depth > best {
                    best = depth;
                    matches.clear();
                }
                if depth == best && !matches.contains(&profile.id) {
                    matches.push(profile.id.clone());
                }
            }
        }
    }
    match matches.len() {
        0 => FolderRule::None,
        1 => FolderRule::Profile(matches.remove(0)),
        _ => FolderRule::Ambiguous,
    }
}

fn origin_owner(origin: &str) -> Option<&str> {
    let path = origin
        .strip_prefix("git@github.com:")?
        .strip_suffix(".git")?;
    let (owner, repo) = path.split_once('/')?;
    (!owner.is_empty() && !repo.is_empty() && !repo.contains('/')).then_some(owner)
}

pub fn origin_profile_candidates(
    repository: &RepositoryRecord,
    profiles: &[Profile],
) -> Vec<String> {
    repository
        .remote_url
        .as_deref()
        .and_then(origin_owner)
        .map(|owner| {
            profiles
                .iter()
                .filter(|p| {
                    p.github_username
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(owner))
                })
                .map(|p| p.id.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Initial selection for a human assignment confirmation.
pub fn assignment_initial_profile(
    repository: &RepositoryRecord,
    profiles: &[Profile],
    requested: Option<&str>,
) -> Option<String> {
    requested
        .filter(|id| profiles.iter().any(|profile| profile.id == *id))
        .map(str::to_owned)
        .or_else(|| {
            origin_profile_candidates(repository, profiles)
                .into_iter()
                .next()
        })
        .or_else(|| match folder_rule(&repository.path, profiles) {
            FolderRule::Profile(id) => Some(id),
            _ => None,
        })
}

pub fn assignment_default_for_profile(repository: &RepositoryRecord, profile: &Profile) -> bool {
    profile
        .auto_approve
        .new_repository_folders
        .iter()
        .any(|folder| path_within(&repository.path, folder))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AssignmentDecision {
    Automatic,
    NeedsConfirmation,
    Existing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentChoice {
    pub profile_id: Option<String>,
    pub decision: AssignmentDecision,
    pub inherit_defaults: bool,
    pub reason: String,
}

pub fn assignment_choice(
    repository: &RepositoryRecord,
    profiles: &[Profile],
    requested_profile_id: Option<&str>,
) -> AssignmentChoice {
    if repository.profile_id.is_some() || repository.last_applied_at.is_some() {
        return AssignmentChoice { profile_id: requested_profile_id.map(str::to_owned), decision: AssignmentDecision::Existing,
            inherit_defaults: false, reason: "Existing repository assignment uses the selected Profile without inheriting defaults.".into() };
    }
    let rule = folder_rule(&repository.path, profiles);
    let candidates = origin_profile_candidates(repository, profiles);
    let conflict = matches!(&rule, FolderRule::Profile(id) if !candidates.is_empty() && !candidates.contains(id));
    if let FolderRule::Profile(id) = &rule {
        if !conflict && requested_profile_id.is_none_or(|requested| requested == id) {
            return AssignmentChoice { profile_id: Some(id.clone()), decision: AssignmentDecision::Automatic,
                inherit_defaults: true, reason: "A single folder rule matches the new repository and agrees with the requested Profile and origin.".into() };
        }
    }
    let reason = if conflict {
        "The folder rule conflicts with the origin owner."
    } else if matches!(rule, FolderRule::Ambiguous) {
        "Multiple Profiles have equally specific folder rules."
    } else if matches!(rule, FolderRule::None) {
        "No folder rule matches this repository."
    } else {
        "The requested Profile differs from the folder rule."
    };
    AssignmentChoice {
        profile_id: requested_profile_id.map(str::to_owned),
        decision: AssignmentDecision::NeedsConfirmation,
        inherit_defaults: false,
        reason: reason.into(),
    }
}

pub fn add_new_repository_folder(
    store: &StateStore,
    profile_id: String,
    folder: String,
) -> Result<AppData, String> {
    let path = PathBuf::from(&folder);
    if !path.is_absolute() || path_parts(&folder).iter().any(|part| part == "..") {
        return Err("Choose an absolute folder under your home directory.".into());
    }
    let path = fs::canonicalize(&path)
        .map_err(|_| "Choose an existing folder under your home directory.")?;
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("Home directory is unavailable.")?;
    let home = fs::canonicalize(home).map_err(|_| "Home directory is unavailable.")?;
    let folder = path.to_string_lossy().into_owned();
    let home = home.to_string_lossy();
    let depth = path_parts(&folder).len();
    if !path_within(&folder, &home) || depth <= path_parts(&home).len() {
        return Err(
            "Choose a folder below your home directory, not the home or drive root.".into(),
        );
    }
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let profile = data
        .profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or("Profile was not found.")?;
    if !profile
        .auto_approve
        .new_repository_folders
        .iter()
        .any(|item| path_parts(item) == path_parts(&folder))
    {
        profile.auto_approve.new_repository_folders.push(folder);
        store.save(&data)?;
    }
    Ok(data)
}

pub fn remove_new_repository_folder(
    store: &StateStore,
    profile_id: String,
    folder: String,
) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let profile = data
        .profiles
        .iter_mut()
        .find(|p| p.id == profile_id)
        .ok_or("Profile was not found.")?;
    profile
        .auto_approve
        .new_repository_folders
        .retain(|item| path_parts(item) != path_parts(&folder));
    store.save(&data)?;
    Ok(data)
}

pub fn repository_default_branch(
    store: &StateStore,
    repository_id: &str,
    profile_id: &str,
) -> Result<String, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, repository_id, profile_id)?;
        ensure_applied_assignment(repository, profile, profile_id, "pushing")?;
        (repository.clone(), profile.clone())
    };
    let directory = connected_gh_directory(store, &profile)?;
    let name = github_repository_name(&repository)?;
    github_default_branch(&directory, &repository.path, &name)
}

pub fn list_github_repositories(
    store: &StateStore,
    profile_id: String,
) -> Result<Vec<GithubRepository>, String> {
    let profile = {
        let _guard = store.lock()?;
        let data = store.load()?;
        data.profiles
            .iter()
            .find(|item| item.id == profile_id)
            .cloned()
            .ok_or_else(|| "Profile was not found.".to_string())?
    };
    validate_profile(&profile)?;
    let directory = resolve_gh_config_dir(store, &profile.id, profile.gh_config_dir.as_deref())?;
    let status = inspect_gh_directory(&directory);
    let actual = status
        .username
        .filter(|_| status.authenticated)
        .ok_or_else(|| {
            status.detail.unwrap_or_else(|| {
                "Connect this Profile to GitHub before listing repositories.".into()
            })
        })?;
    if let Some(expected) = profile.github_username.as_deref() {
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(format!(
                "The Profile expects @{expected}, but GitHub CLI is authenticated as @{actual}."
            ));
        }
    }

    {
        let output = gh_command(&directory)
            .args([
                "api",
                "user/repos?affiliation=owner,collaborator,organization_member&per_page=100&sort=updated&direction=desc",
            ])
            .output()
            .map_err(|error| format!("Could not list GitHub repositories: {error}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if detail.is_empty() {
                format!("GitHub CLI exited with {}.", output.status)
            } else {
                detail
            });
        }
        let repositories: Vec<GithubApiRepository> = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("GitHub returned an invalid repository list: {error}"))?;
        Ok(repositories
            .into_iter()
            .map(|repository| GithubRepository {
                name: repository.name,
                name_with_owner: repository.full_name,
                description: repository.description,
                is_private: repository.private,
                ssh_url: repository.ssh_url,
                updated_at: repository.updated_at,
            })
            .collect())
    }
}

pub fn clone_repository(
    store: &StateStore,
    profile_id: String,
    repository_url: String,
    destination_parent: String,
) -> Result<CloneResult, String> {
    let (repository_url, repository_name) = validate_github_ssh_url(&repository_url)?;
    let profile = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let profile = data
            .profiles
            .iter()
            .find(|item| item.id == profile_id)
            .cloned()
            .ok_or_else(|| "Profile was not found.".to_string())?;
        validate_profile(&profile)?;
        profile
    };

    let cloned_profile = profile.clone();
    let mut repository = {
        git_ops::clone_repository(
            &repository_url,
            &repository_name,
            &destination_parent,
            &cloned_profile,
        )
    }?;

    repository.profile_id = Some(profile.id.clone());
    repository.last_applied_at = Some(Utc::now().to_rfc3339());
    let data = {
        let _guard = store.lock()?;
        let mut data = store.load()?;
        if data
            .repositories
            .iter()
            .any(|item| item.path == repository.path)
        {
            return Err("The cloned repository is already registered in GitContext.".into());
        }
        data.repositories.push(repository.clone());
        store.save(&data)?;
        data
    };
    Ok(CloneResult { data, repository })
}

pub fn preview_clone_repository(
    store: &StateStore,
    profile_id: String,
    ssh_url: String,
    destination_parent: String,
) -> Result<(String, String, String), String> {
    let (url, repository_name) = validate_github_ssh_url(&ssh_url)?;
    let data = {
        let _guard = store.lock()?;
        store.load()?
    };
    let profile = data
        .profiles
        .iter()
        .find(|item| item.id == profile_id)
        .ok_or_else(|| "Profile was not found.".to_string())?;
    validate_profile(profile)?;
    git_ops::validate_clone_profile(profile)?;
    let parent = fs::canonicalize(&destination_parent)
        .map_err(|error| format!("Clone destination is not accessible: {error}"))?;
    if !parent.is_dir() {
        return Err("Clone destination must be an existing directory.".into());
    }
    let home = git_ops::home_directory()
        .ok_or_else(|| "User home directory is unavailable.".to_string())?;
    let home = fs::canonicalize(home)
        .map_err(|error| format!("User home directory is unavailable: {error}"))?;
    if !parent.starts_with(&home) {
        return Err("Clone destination must be inside the user home directory.".into());
    }
    if parent.join(&repository_name).exists() {
        return Err("The clone destination already exists.".into());
    }
    Ok((url, parent.to_string_lossy().into_owned(), repository_name))
}

pub fn remove_repository(store: &StateStore, id: String) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    data.repositories.retain(|repository| repository.id != id);
    store.save(&data)?;
    Ok(data)
}

pub fn preview_assignment(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<ApplyPreview, String> {
    let _guard = store.lock()?;
    let data = store.load()?;
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

pub fn apply_profile(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
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
    store.save(&data)?;
    Ok(data)
}

pub fn apply_profile_with_defaults(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<AppData, String> {
    apply_profile_with_defaults_inner(store, repository_id, profile_id, false)
}

/// Use only after a human has confirmed the assignment and its defaults.
pub fn apply_profile_with_confirmed_defaults(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<AppData, String> {
    apply_profile_with_defaults_inner(store, repository_id, profile_id, true)
}

fn apply_profile_with_defaults_inner(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
    confirmed: bool,
) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
    let mut current_repository = repository.clone();
    current_repository.remote_url = git_ops::origin_url(&repository.path).ok();
    let choice = assignment_choice(&current_repository, &data.profiles, Some(&profile_id));
    if !confirmed && choice.decision != AssignmentDecision::Automatic {
        return Err(format!(
            "{} Ask the user which Profile to use, or assign it in GitContext.",
            choice.reason
        ));
    }
    validate_profile(profile)?;
    git_ops::apply_profile(repository, profile)?;
    let defaults = AutoApprove::from(&profile.auto_approve.new_repository);
    let applied_at = Utc::now().to_rfc3339();
    let target = data
        .repositories
        .iter_mut()
        .find(|item| item.id == repository_id)
        .ok_or("Repository was not found.")?;
    target.profile_id = Some(profile_id.clone());
    target.last_applied_at = Some(applied_at.clone());
    target.auto_approve = defaults;
    target.auto_approve_source = Some(AutoApproveSource {
        profile_id,
        applied_at,
    });
    store.save(&data)?;
    Ok(data)
}

/// GUI entry point. The MCP entry point keeps its existing caller-owned audit record.
pub fn apply_profile_from_gui(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<AppData, String> {
    let result = apply_profile(store, repository_id.clone(), profile_id.clone());
    let record = crate::audit::Audit::new(
        "apply_profile",
        Some(&repository_id),
        Some(&profile_id),
        if result.is_ok() { "success" } else { "failed" },
        if result.is_ok() {
            "Profile applied"
        } else {
            "Operation failed"
        },
        None,
        None,
    )
    .with_actor("gui");
    if let Err(error) = crate::audit::append(store, &record) {
        eprintln!("Audit log could not be written: {error}");
    }
    result
}

pub fn preview_push(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<PushPreview, String> {
    let _guard = store.lock()?;
    let data = store.load()?;
    let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
    if repository.profile_id.as_deref() != Some(profile_id.as_str())
        || repository.last_applied_at.is_none()
    {
        return Err("Apply this Profile to the repository before pushing.".into());
    }
    validate_profile(profile)?;
    git_ops::build_push_preview(repository, profile)
}

pub fn push_repository(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<PushResult, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        if repository.profile_id.as_deref() != Some(profile_id.as_str())
            || repository.last_applied_at.is_none()
        {
            return Err("Apply this Profile to the repository before pushing.".into());
        }
        validate_profile(profile)?;
        (repository.clone(), profile.clone())
    };

    {
        git_ops::push_current_branch(&repository, &profile)
    }
}

pub fn preview_repository_sync(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<SyncPreview, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "syncing with GitHub")?;
        (repository.clone(), profile.clone())
    };

    {
        git_ops::refresh_sync_preview(&repository, &profile)
    }
}

pub fn pull_repository(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<SyncPreview, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "syncing with GitHub")?;
        (repository.clone(), profile.clone())
    };

    {
        git_ops::pull_current_branch(&repository, &profile)
    }
}

pub fn preview_commit(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<CommitPreview, String> {
    let _guard = store.lock()?;
    let data = store.load()?;
    let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
    if repository.profile_id.as_deref() != Some(profile_id.as_str())
        || repository.last_applied_at.is_none()
    {
        return Err("Apply this Profile to the repository before committing.".into());
    }
    validate_profile(profile)?;
    git_ops::build_commit_preview(repository, profile)
}

pub fn commit_repository(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
    message: String,
) -> Result<CommitResult, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        if repository.profile_id.as_deref() != Some(profile_id.as_str())
            || repository.last_applied_at.is_none()
        {
            return Err("Apply this Profile to the repository before committing.".into());
        }
        validate_profile(profile)?;
        (repository.clone(), profile.clone())
    };

    {
        git_ops::commit_all_changes(&repository, &profile, &message)
    }
}

pub fn commit_previewed_changes(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
    message: String,
    changes: &[git_ops::ExactChange],
) -> Result<CommitResult, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "committing")?;
        (repository.clone(), profile.clone())
    };
    git_ops::commit_exact_changes(&repository, &profile, &message, changes)
}

pub fn preview_pull_request(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<PullRequestPreview, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "creating a pull request")?;
        (repository.clone(), profile.clone())
    };
    let directory = connected_gh_directory(store, &profile)?;
    let repository_name_with_owner = github_repository_name(&repository)?;

    {
        let base_branch =
            github_default_branch(&directory, &repository.path, &repository_name_with_owner)?;
        let source = git_ops::inspect_pull_request_source(&repository, &profile, &base_branch)?;
        let current_branch = source.current_branch;
        let requires_new_branch = current_branch == base_branch;
        let existing_pull_request = if requires_new_branch {
            None
        } else {
            find_existing_pull_request(
                &directory,
                &repository.path,
                &repository_name_with_owner,
                &current_branch,
            )?
        };
        Ok(PullRequestPreview {
            repository,
            profile,
            current_branch,
            base_branch,
            remote_url: source.remote_url,
            repository_name_with_owner,
            changes: source.changes,
            commits_ahead: source.commits_ahead,
            branch_pushed: source.branch_pushed,
            requires_new_branch,
            existing_pull_request,
        })
    }
}

pub fn create_branch(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
    branch_name: String,
) -> Result<BranchResult, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "creating a branch")?;
        (repository.clone(), profile.clone())
    };
    let branch = { git_ops::create_working_branch(&repository, &profile, &branch_name) }?;
    let _guard = store.lock()?;
    let mut data = store.load()?;
    let target = data
        .repositories
        .iter_mut()
        .find(|item| item.id == repository_id)
        .ok_or_else(|| "Repository was not found.".to_string())?;
    target.branch = Some(branch.clone());
    target.remote_url = git_ops::origin_url(&target.path).ok();
    store.save(&data)?;
    Ok(BranchResult { data, branch })
}

pub fn create_pull_request(
    store: &StateStore,
    input: CreatePullRequestInput,
) -> Result<PullRequestResult, String> {
    let title = validate_pull_request_title(&input.title)?;
    let body = validate_pull_request_body(&input.body)?;
    let repository_id = input.repository_id;
    let profile_id = input.profile_id;
    let base_branch = input.base_branch;
    let draft = input.draft;
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "creating a pull request")?;
        (repository.clone(), profile.clone())
    };
    let directory = connected_gh_directory(store, &profile)?;
    let repository_name_with_owner = github_repository_name(&repository)?;

    {
        let source = git_ops::inspect_pull_request_source(&repository, &profile, &base_branch)?;
        let branch = source.current_branch;
        if branch == base_branch {
            return Err(
                "Create or switch to a working branch before creating a pull request.".into(),
            );
        }
        if !source.changes.is_empty() {
            return Err("Commit all working tree changes before creating a pull request.".into());
        }
        if source.commits_ahead == 0 {
            return Err("The working branch has no commits to propose to the base branch.".into());
        }
        if !source.branch_pushed {
            return Err("Push the current working branch before creating a pull request.".into());
        }
        if let Some(existing) = find_existing_pull_request(
            &directory,
            &repository.path,
            &repository_name_with_owner,
            &branch,
        )? {
            return Ok(PullRequestResult {
                number: existing.number,
                url: existing.url,
                title: existing.title,
                branch,
                base_branch,
                existing: true,
            });
        }

        let mut command = gh_command(&directory);
        command.current_dir(&repository.path).args([
            "pr",
            "create",
            "--repo",
            repository_name_with_owner.as_str(),
            "--base",
            base_branch.as_str(),
            "--head",
            branch.as_str(),
            "--title",
            title.as_str(),
            "--body",
            body.as_str(),
        ]);
        if draft {
            command.arg("--draft");
        }
        let output = command
            .output()
            .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
        github_output_text(&output)?;
        let created = find_existing_pull_request(
            &directory,
            &repository.path,
            &repository_name_with_owner,
            &branch,
        )?
        .ok_or_else(|| {
            "GitHub created the pull request, but its details could not be read.".to_string()
        })?;
        Ok(PullRequestResult {
            number: created.number,
            url: created.url,
            title: created.title,
            branch,
            base_branch,
            existing: false,
        })
    }
}

pub fn list_pull_requests(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
) -> Result<PullRequestManagement, String> {
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "managing pull requests")?;
        (repository.clone(), profile.clone())
    };
    let directory = connected_gh_directory(store, &profile)?;
    let repository_name_with_owner = github_repository_name(&repository)?;
    let result_repository = repository.clone();
    let result_profile = profile.clone();
    let result_name = repository_name_with_owner.clone();

    {
        let mut command = gh_command(&directory);
        let output = command
            .current_dir(&repository.path)
            .args([
                "pr",
                "list",
                "--repo",
                repository_name_with_owner.as_str(),
                "--state",
                "open",
                "--limit",
                "30",
                "--json",
                pull_request_json_fields(),
            ])
            .output()
            .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
        let text = github_output_text(&output)?;
        let items: Vec<GithubPullRequest> = serde_json::from_str(&text)
            .map_err(|error| format!("GitHub CLI returned invalid pull request data: {error}"))?;
        Ok(PullRequestManagement {
            repository: result_repository,
            profile: result_profile,
            repository_name_with_owner: result_name,
            pull_requests: items.into_iter().map(normalize_pull_request).collect(),
        })
    }
}

pub fn preview_merge_pull_request(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
    number: u64,
) -> Result<(crate::models::ManagedPullRequest, Result<(), String>), String> {
    if number == 0 {
        return Err("Pull request number must be positive.".into());
    }
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "merging a pull request")?;
        (repository.clone(), profile.clone())
    };
    let directory = connected_gh_directory(store, &profile)?;
    let name = github_repository_name(&repository)?;
    let item = read_pull_request(&directory, &repository.path, &name, number)?;
    let validation = validate_pull_request_for_merge(&item, &item.head_ref_oid);
    Ok((normalize_pull_request(item), validation))
}

pub fn preview_publish_repository(
    store: &StateStore,
    repository_id: String,
    profile_id: String,
    name: String,
    visibility: String,
    description: Option<String>,
) -> Result<(String, Option<String>, String), String> {
    let name = validate_github_repository_name(&name)?;
    let description = validate_github_description(description)?;
    if !matches!(visibility.as_str(), "private" | "public") {
        return Err("Repository visibility must be private or public.".into());
    }
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) = find_assignment(&data, &repository_id, &profile_id)?;
        ensure_applied_assignment(repository, profile, &profile_id, "publishing")?;
        (repository.clone(), profile.clone())
    };
    git_ops::validate_publish_source(&repository)?;
    let directory = connected_gh_directory(store, &profile)?;
    let status = inspect_gh_directory(&directory);
    let username = status
        .username
        .filter(|_| status.authenticated)
        .ok_or_else(|| "Connect this Profile to GitHub before publishing.".to_string())?;
    Ok((name, description, username))
}

pub fn merge_pull_request(
    store: &StateStore,
    input: MergePullRequestInput,
) -> Result<MergePullRequestResult, String> {
    if input.number == 0 {
        return Err("Pull request number must be positive.".into());
    }
    if input.expected_head_oid.is_empty()
        || input.expected_head_oid.len() > 64
        || !input
            .expected_head_oid
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("The expected pull request commit is invalid. Refresh and try again.".into());
    }
    let strategy_flag = match input.strategy.as_str() {
        "squash" => "--squash",
        "merge" => "--merge",
        "rebase" => "--rebase",
        _ => return Err("Merge strategy must be squash, merge, or rebase.".into()),
    };
    let (repository, profile) = {
        let _guard = store.lock()?;
        let data = store.load()?;
        let (repository, profile) =
            find_assignment(&data, &input.repository_id, &input.profile_id)?;
        ensure_applied_assignment(
            repository,
            profile,
            &input.profile_id,
            "merging a pull request",
        )?;
        (repository.clone(), profile.clone())
    };
    let directory = connected_gh_directory(store, &profile)?;
    let repository_name_with_owner = github_repository_name(&repository)?;
    let number = input.number;
    let expected_head_oid = input.expected_head_oid;
    let strategy = input.strategy;

    {
        let pull_request = read_pull_request(
            &directory,
            &repository.path,
            &repository_name_with_owner,
            number,
        )?;
        validate_pull_request_for_merge(&pull_request, &expected_head_oid)?;

        let number_text = number.to_string();
        let mut command = gh_command(&directory);
        let output = command
            .current_dir(&repository.path)
            .args([
                "pr",
                "merge",
                number_text.as_str(),
                "--repo",
                repository_name_with_owner.as_str(),
                strategy_flag,
                "--match-head-commit",
                expected_head_oid.as_str(),
            ])
            .output()
            .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
        github_output_text(&output)?;

        let merged = read_pull_request(
            &directory,
            &repository.path,
            &repository_name_with_owner,
            number,
        )?;
        if merged.state != "MERGED" {
            return Err(
                "GitHub accepted the merge command, but the pull request is not merged.".into(),
            );
        }
        Ok(MergePullRequestResult {
            number: merged.number,
            url: merged.url,
            title: merged.title,
            strategy,
            merged_at: merged.merged_at,
        })
    }
}

pub fn publish_repository(
    store: &StateStore,
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
        let _guard = store.lock()?;
        let data = store.load()?;
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
    let directory = resolve_gh_config_dir(store, &profile.id, profile.gh_config_dir.as_deref())?;
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
    (|| {
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
    })()?;

    let remote_url = git_ops::origin_url(&repository.path)?;
    let data = {
        let _guard = store.lock()?;
        let mut data = store.load()?;
        let target = data
            .repositories
            .iter_mut()
            .find(|item| item.id == repository_id)
            .ok_or_else(|| "Repository was not found.".to_string())?;
        target.remote_url = Some(remote_url);
        store.save(&data)?;
        data
    };
    Ok(PublishResult {
        data,
        repository_url: format!("https://github.com/{full_name}"),
    })
}

fn ensure_applied_assignment(
    repository: &RepositoryRecord,
    profile: &Profile,
    profile_id: &str,
    action: &str,
) -> Result<(), String> {
    if repository.profile_id.as_deref() != Some(profile_id) || repository.last_applied_at.is_none()
    {
        return Err(format!(
            "Apply this Profile to the repository before {action}."
        ));
    }
    validate_profile(profile)
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

#[cfg(test)]
mod new_repository_tests {
    use super::*;

    fn profile(id: &str, folders: &[&str], owner: Option<&str>) -> Profile {
        Profile {
            id: id.into(),
            label: id.into(),
            accent: "#112233".into(),
            git_name: "Example".into(),
            git_email: "example@example.com".into(),
            github_username: owner.map(str::to_owned),
            ssh_key_path: None,
            gh_config_dir: None,
            auto_approve: ProfileAutoApprove {
                new_repository_folders: folders.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
        }
    }

    fn repository(path: &str) -> RepositoryRecord {
        RepositoryRecord {
            id: "r".into(),
            name: "r".into(),
            path: path.into(),
            remote_url: None,
            branch: None,
            profile_id: None,
            last_applied_at: None,
            auto_approve: AutoApprove::default(),
            auto_approve_source: None,
        }
    }

    #[test]
    fn folder_rule_uses_components_depth_and_windows_normalization() {
        let profiles = vec![
            profile("a", &[r"C:\Work\"], None),
            profile("b", &[r"\\?\C:\Work\Deep"], None),
        ];
        assert_eq!(
            folder_rule(r"c:\WORK\deep\repo", &profiles),
            FolderRule::Profile("b".into())
        );
        assert_eq!(folder_rule(r"C:\Work2\repo", &profiles), FolderRule::None);
        let tied = vec![
            profile("a", &[r"C:\Work"], None),
            profile("b", &[r"\\?\C:\work\"], None),
        ];
        assert_eq!(folder_rule(r"C:\Work\repo", &tied), FolderRule::Ambiguous);
    }

    #[test]
    fn assignment_confirmation_initial_values_follow_requested_origin_then_rule() {
        let profiles = vec![
            profile("rule", &[r"C:\Work"], None),
            profile("origin", &[r"C:\Other"], Some("sample-owner")),
            profile("chosen", &[r"C:\Work"], None),
        ];
        let mut repo = repository(r"C:\Work\repo");
        assert_eq!(assignment_initial_profile(&repo, &profiles, None), None); // tied rule
        repo.remote_url = Some("git@github.com:sample-owner/repo.git".into());
        assert_eq!(
            assignment_initial_profile(&repo, &profiles, None).as_deref(),
            Some("origin")
        );
        assert_eq!(
            assignment_initial_profile(&repo, &profiles, Some("chosen")).as_deref(),
            Some("chosen")
        );
        assert!(assignment_default_for_profile(&repo, &profiles[2]));
        assert!(!assignment_default_for_profile(&repo, &profiles[1]));
        assert_eq!(
            assignment_initial_profile(&repo, &profiles, Some("missing")).as_deref(),
            Some("origin")
        );
        repo.remote_url = None;
        assert_eq!(
            assignment_initial_profile(&repo, &profiles[..1], None).as_deref(),
            Some("rule")
        );
    }

    #[test]
    fn assignment_requires_new_matching_unambiguous_rule() {
        let profiles = vec![
            profile("a", &[r"C:\Work"], Some("alice")),
            profile("b", &[r"C:\Other"], Some("bob")),
        ];
        let mut repo = repository(r"C:\Work\repo");
        assert_eq!(
            assignment_choice(&repo, &profiles, None).decision,
            AssignmentDecision::Automatic
        );
        assert!(assignment_choice(&repo, &profiles, None).inherit_defaults);
        assert_eq!(
            assignment_choice(&repo, &profiles, Some("b")).decision,
            AssignmentDecision::NeedsConfirmation
        );
        repo.remote_url = Some("git@github.com:bob/repo.git".into());
        assert_eq!(
            assignment_choice(&repo, &profiles, Some("a")).decision,
            AssignmentDecision::NeedsConfirmation
        );
        repo.remote_url = None;
        repo.profile_id = Some("a".into());
        assert_eq!(
            assignment_choice(&repo, &profiles, Some("a")).decision,
            AssignmentDecision::Existing
        );
        assert!(!assignment_choice(&repo, &profiles, Some("a")).inherit_defaults);
        repo.profile_id = None;
        repo.last_applied_at = Some("earlier".into());
        assert_eq!(
            assignment_choice(&repo, &profiles, Some("a")).decision,
            AssignmentDecision::Existing
        );
        repo.last_applied_at = None;
        repo.path = r"C:\Outside\repo".into();
        assert_eq!(
            assignment_choice(&repo, &profiles, Some("a")).decision,
            AssignmentDecision::NeedsConfirmation
        );
    }

    #[test]
    fn rejects_folders_outside_home_and_home_itself() {
        let store = StateStore::new(
            std::env::temp_dir().join(format!("gitcontext-folder-test-{}", uuid::Uuid::new_v4())),
        );
        assert!(add_new_repository_folder(&store, "missing".into(), "relative".into()).is_err());
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(add_new_repository_folder(&store, "missing".into(), home).is_err());
        let root = if cfg!(windows) { r"C:\" } else { "/" };
        assert!(add_new_repository_folder(&store, "missing".into(), root.into()).is_err());
        let outside = if cfg!(windows) { r"C:\Windows" } else { "/etc" };
        assert!(add_new_repository_folder(&store, "missing".into(), outside.into()).is_err());
    }
}

#[cfg(test)]
mod auto_approve_tests {
    use super::*;
    use std::{path::Path, process::Command};

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn inherited_defaults_are_traced_and_gui_change_clears_source() {
        let root =
            std::env::temp_dir().join(format!("gitcontext-inherit-{}", uuid::Uuid::new_v4()));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        let store = StateStore::new(root.join("data"));
        let record = add_repository(&store, repo.to_string_lossy().into_owned()).unwrap();
        let profile = Profile {
            id: "sample".into(),
            label: "Sample".into(),
            accent: "#112233".into(),
            git_name: "Example Person".into(),
            git_email: "sample@example.com".into(),
            github_username: None,
            ssh_key_path: None,
            gh_config_dir: None,
            auto_approve: ProfileAutoApprove::default(),
        };
        save_profile(&store, profile).unwrap();
        let mut defaults = NewRepositoryDefaults {
            push_work_branch: true,
            ..Default::default()
        };
        defaults.publish_visibility = crate::models::PublishVisibility::Any;
        set_new_repository_defaults(&store, "sample".into(), defaults).unwrap();
        // Test state can store a rule outside the real user's home; the GUI setter validates additions.
        let mut state = store.load().unwrap();
        state.profiles[0].auto_approve.new_repository_folders.push(
            fs::canonicalize(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
        store.save(&state).unwrap();
        let mut edited = state.profiles[0].clone();
        edited.label = "Edited".into();
        edited.auto_approve = ProfileAutoApprove::default();
        let saved = save_profile(&store, edited).unwrap();
        assert_eq!(
            saved.profiles[0].auto_approve.new_repository_folders.len(),
            1
        );
        assert!(
            saved.profiles[0]
                .auto_approve
                .new_repository
                .push_work_branch
        );
        let applied =
            apply_profile_with_defaults(&store, record.id.clone(), "sample".into()).unwrap();
        assert!(applied.repositories[0].auto_approve.push_work_branch);
        assert_eq!(
            applied.repositories[0].auto_approve.publish_visibility,
            crate::models::PublishVisibility::Any
        );
        assert_eq!(
            applied.repositories[0]
                .auto_approve_source
                .as_ref()
                .unwrap()
                .profile_id,
            "sample"
        );
        let changed =
            set_repository_auto_approve(&store, record.id, AutoApprove::default()).unwrap();
        assert!(changed.repositories[0].auto_approve_source.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gui_apply_records_success_and_failure_without_sensitive_values() {
        let root =
            std::env::temp_dir().join(format!("gitcontext-gui-audit-{}", uuid::Uuid::new_v4()));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        let store = StateStore::new(root.join("data"));
        let record = add_repository(&store, repo.to_string_lossy().into_owned()).unwrap();
        save_profile(
            &store,
            Profile {
                id: "sample".into(),
                label: "Sample".into(),
                accent: "#112233".into(),
                git_name: "Example Person".into(),
                git_email: "sample@example.com".into(),
                github_username: None,
                ssh_key_path: None,
                gh_config_dir: None,
                auto_approve: ProfileAutoApprove::default(),
            },
        )
        .unwrap();
        apply_profile_from_gui(&store, record.id.clone(), "sample".into()).unwrap();
        assert!(apply_profile_from_gui(&store, record.id, "missing".into()).is_err());
        let entries = crate::audit::read_recent(&store).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.outcome == "failed"));
        assert!(entries.iter().any(|entry| entry.outcome == "success"));
        assert!(entries
            .iter()
            .all(|entry| entry.actor.as_deref() == Some("gui")
                && entry.client.is_none()
                && entry.confirmation.is_none()));
        let contents = fs::read_to_string(store.config_dir().join("mcp-audit.jsonl")).unwrap();
        assert!(!contents.contains("sample@example.com"));
        assert!(!contents.contains("Example Person"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn auto_approval_survives_repository_updates() {
        let root = std::env::temp_dir().join(format!("gitcontext-auto-{}", uuid::Uuid::new_v4()));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        let store = StateStore::new(root.join("data"));
        let record = add_repository(&store, repo.to_string_lossy().into_owned()).unwrap();
        assert!(!record.auto_approve.push_work_branch);
        let mut old_state = serde_json::to_value(store.load().unwrap()).unwrap();
        old_state["repositories"][0]
            .as_object_mut()
            .unwrap()
            .remove("autoApprove");
        fs::write(store.state_path(), old_state.to_string()).unwrap();
        assert!(
            !store.load().unwrap().repositories[0]
                .auto_approve
                .push_work_branch
        );
        let changed = set_repository_auto_approve(
            &store,
            record.id.clone(),
            AutoApprove {
                push_work_branch: true,
                create_pull_request: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(changed.repositories[0].auto_approve.create_pull_request);
        let again = add_repository(&store, repo.to_string_lossy().into_owned()).unwrap();
        assert_eq!(again.id, record.id);
        assert!(again.auto_approve.push_work_branch);
        let profile = Profile {
            id: "sample".into(),
            label: "Sample".into(),
            accent: "#112233".into(),
            git_name: "Sample Person".into(),
            git_email: "sample@example.com".into(),
            github_username: None,
            ssh_key_path: None,
            gh_config_dir: None,
            auto_approve: ProfileAutoApprove::default(),
        };
        save_profile(&store, profile).unwrap();
        let approved = set_profile_auto_approve(
            &store,
            "sample".into(),
            ProfileAutoApprove {
                clone_repository: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(approved.profiles[0].auto_approve.clone_repository);
        let mut edited = approved.profiles[0].clone();
        edited.label = "Edited".into();
        edited.auto_approve = ProfileAutoApprove::default();
        let saved = save_profile(&store, edited).unwrap();
        assert!(saved.profiles[0].auto_approve.clone_repository);
        assert!(
            set_profile_auto_approve(&store, "missing".into(), ProfileAutoApprove::default())
                .is_err()
        );
        let applied = apply_profile(&store, record.id.clone(), "sample".into()).unwrap();
        assert!(applied.repositories[0].auto_approve.push_work_branch);
        fs::write(repo.join("readme.txt"), "fixture").unwrap();
        git(&repo, &["add", "readme.txt"]);
        git(&repo, &["commit", "-m", "Initial fixture"]);
        let branch =
            create_branch(&store, record.id.clone(), "sample".into(), "work".into()).unwrap();
        assert_eq!(branch.branch, "work");
        assert!(branch.data.repositories[0].auto_approve.create_pull_request);
        let reloaded = store.load().unwrap();
        assert!(reloaded.repositories[0].auto_approve.push_work_branch);
        assert!(
            set_repository_auto_approve(&store, "missing".into(), AutoApprove::default()).is_err()
        );
        assert!(
            store.load().unwrap().repositories[0]
                .auto_approve
                .push_work_branch
        );
        fs::remove_dir_all(root).unwrap();
    }
}

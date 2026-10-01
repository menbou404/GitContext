use crate::{
    environment::*,
    git_ops,
    github::*,
    models::{
        normalize_profile, validate_profile, AppData, ApplyPreview, BootstrapResult, BranchResult,
        CloneResult, CommitPreview, CommitResult, GhProfileStatus, GithubRepository,
        MergePullRequestResult, Profile, PublishResult, PullRequestManagement, PullRequestPreview,
        PullRequestResult, PushPreview, PushResult, RepositoryRecord, SyncPreview,
    },
    storage::{development_data, StateStore},
};
use chrono::Utc;
use std::{
    fs,
    path::Path,
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

pub fn save_profile(store: &StateStore, profile: Profile) -> Result<AppData, String> {
    let _guard = store.lock()?;
    let mut profile = normalize_profile(profile);
    if profile.id.trim().is_empty() {
        profile.id = uuid::Uuid::new_v4().to_string();
    }
    validate_profile(&profile)?;

    let mut data = store.load()?;
    if let Some(existing) = data.profiles.iter_mut().find(|item| item.id == profile.id) {
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

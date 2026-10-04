use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::models::{
    ApplyPreview, CommitPreview, CommitResult, ConfigChange, Profile, PushPreview, PushResult,
    RepositoryRecord, SyncPreview, WorkingTreeChange,
};

pub struct PullRequestSourceState {
    pub current_branch: String,
    pub remote_url: String,
    pub changes: Vec<WorkingTreeChange>,
    pub commits_ahead: u64,
    pub branch_pushed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactChange {
    pub status: String,
    pub path: String,
    pub original_path: Option<String>,
}

pub fn head_commit(repository_path: &str) -> Result<String, String> {
    let root = repository_root(repository_path)?;
    Ok(git_optional(&root, &["rev-parse", "--verify", "HEAD"]).unwrap_or_default())
}

pub fn reference_commit(repository_path: &str, reference: &str) -> Result<String, String> {
    let root = repository_root(repository_path)?;
    Ok(git_optional(&root, &["rev-parse", "--verify", reference]).unwrap_or_default())
}

pub fn branch_name(repository_path: &str) -> Result<String, String> {
    current_branch(&repository_root(repository_path)?, "continuing")
}

pub fn tracking_branch(repository_path: &str) -> Result<Option<String>, String> {
    let root = repository_root(repository_path)?;
    Ok(git_optional(
        &root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    ))
}

pub fn exact_changes(repository_path: &str) -> Result<Vec<ExactChange>, String> {
    let root = repository_root(repository_path)?;
    let output = run_git(
        &root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    output_text(&output)?;
    parse_exact_changes(&output.stdout)
}

fn parse_exact_changes(bytes: &[u8]) -> Result<Vec<ExactChange>, String> {
    let mut fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut changes = Vec::new();
    while let Some(field) = fields.next() {
        if field.len() < 4 || field[2] != b' ' {
            return Err("Git returned invalid status data.".into());
        }
        let status = String::from_utf8_lossy(&field[..2]).trim().to_string();
        let path = String::from_utf8(field[3..].to_vec())
            .map_err(|_| "A changed file name is not UTF-8.".to_string())?;
        let original_path = if field[..2].contains(&b'R') || field[..2].contains(&b'C') {
            Some(
                String::from_utf8(
                    fields
                        .next()
                        .ok_or("Git returned an incomplete rename.")?
                        .to_vec(),
                )
                .map_err(|_| "A renamed file name is not UTF-8.".to_string())?,
            )
        } else {
            None
        };
        changes.push(ExactChange {
            status,
            path,
            original_path,
        });
    }
    Ok(changes)
}

fn change_paths(changes: &[ExactChange]) -> BTreeSet<String> {
    changes
        .iter()
        .flat_map(|change| std::iter::once(change.path.clone()).chain(change.original_path.clone()))
        .collect()
}

fn cached_paths(root: &Path) -> Result<BTreeSet<String>, String> {
    let output = run_git(
        root,
        &["diff", "--cached", "--no-renames", "--name-only", "-z"],
    )?;
    output_text(&output)?;
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| {
            String::from_utf8(field.to_vec())
                .map_err(|_| "A staged file name is not UTF-8.".to_string())
        })
        .collect()
}

pub fn commit_exact_changes(
    repository: &RepositoryRecord,
    profile: &Profile,
    message: &str,
    changes: &[ExactChange],
) -> Result<CommitResult, String> {
    let message = validate_commit_message(message)?;
    let root = repository_root(&repository.path)?;
    validate_applied_identity(&root, profile)?;
    let branch = current_branch(&root, "committing")?;
    if changes.is_empty() {
        return Err("There are no previewed changes to commit.".into());
    }
    let paths = change_paths(changes);
    if !cached_paths(&root)?.is_subset(&paths) {
        return Err("Staged files differ from the preview. No commit was made.".into());
    }
    let index_name = output_text(&run_git(&root, &["rev-parse", "--git-path", "index"])?)?;
    let index_path = root.join(index_name);
    let original_index = fs::read(&index_path).ok();
    let add_paths: Vec<_> = paths
        .iter()
        .filter(|path| {
            root.join(path).exists()
                || run_git(
                    &root,
                    &[
                        "ls-files",
                        "--error-unmatch",
                        "--",
                        &format!(":(literal){path}"),
                    ],
                )
                .is_ok_and(|output| output.status.success())
        })
        .map(|path| format!(":(literal){path}"))
        .collect();
    let staged = if add_paths.is_empty() {
        cached_paths(&root)
    } else {
        let output = Command::new("git")
            .current_dir(&root)
            .args(["add", "-A", "--"])
            .args(add_paths)
            .output()
            .map_err(|error| format!("Could not start git add: {error}"))?;
        output_text(&output).and_then(|_| cached_paths(&root))
    };
    if !matches!(staged.as_ref(), Ok(staged) if staged == &paths) {
        match original_index {
            Some(bytes) => fs::write(&index_path, bytes)
                .map_err(|_| "Could not restore the Git index after staging failed.".to_string())?,
            None if index_path.exists() => fs::remove_file(&index_path)
                .map_err(|_| "Could not restore the Git index after staging failed.".to_string())?,
            None => {}
        }
        return Err("Staged files differ from the preview. No commit was made.".into());
    }
    output_text(&run_git(&root, &["commit", "--message", &message])?)?;
    let commit_id = output_text(&run_git(&root, &["rev-parse", "--short", "HEAD"])?)?;
    Ok(CommitResult {
        branch,
        commit_id,
        message,
    })
}

pub fn inspect_repository(input: &str) -> Result<RepositoryRecord, String> {
    let root = repository_root(input)?;
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repository")
        .to_string();
    Ok(RepositoryRecord {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        path: root.to_string_lossy().into_owned(),
        remote_url: git_optional(&root, &["config", "--get", "remote.origin.url"]),
        branch: git_optional(&root, &["branch", "--show-current"]),
        profile_id: None,
        last_applied_at: None,
        auto_approve: Default::default(),
    })
}

pub fn clone_repository(
    repository_url: &str,
    repository_name: &str,
    destination_parent: &str,
    profile: &Profile,
) -> Result<RepositoryRecord, String> {
    let parent = fs::canonicalize(expand_home(destination_parent))
        .map_err(|error| format!("Clone destination is not accessible: {error}"))?;
    if !parent.is_dir() {
        return Err("Clone destination must be an existing directory.".into());
    }

    let destination = parent.join(repository_name);
    if destination.exists() {
        return Err(format!(
            "The clone destination already exists: {}",
            destination.display()
        ));
    }

    let ssh_command = desired_config(profile)?
        .into_iter()
        .find_map(|(key, value)| (key == "core.sshCommand").then_some(value).flatten())
        .ok_or_else(|| "Choose an SSH private key for this Profile before cloning.".to_string())?;
    let destination_text = destination.to_string_lossy().into_owned();
    let output = Command::new("git")
        .current_dir(&parent)
        .arg("-c")
        .arg(format!("core.sshCommand={ssh_command}"))
        .args(["clone", "--", repository_url, destination_text.as_str()])
        .output()
        .map_err(|error| format!("Could not start Git clone: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("Git clone exited with {}.", output.status)
        } else {
            detail
        });
    }

    let repository = inspect_repository(&destination_text)?;
    apply_profile(&repository, profile)?;
    Ok(repository)
}

pub fn validate_clone_profile(profile: &Profile) -> Result<(), String> {
    desired_config(profile)?
        .into_iter()
        .find_map(|(key, value)| (key == "core.sshCommand").then_some(value).flatten())
        .ok_or_else(|| "Choose an SSH private key for this Profile before cloning.".to_string())?;
    Ok(())
}

pub fn build_preview(
    repository: &RepositoryRecord,
    profile: &Profile,
    gh_available: bool,
) -> Result<ApplyPreview, String> {
    let root = repository_root(&repository.path)?;
    let changes = config_changes(&root, profile)?;

    let mut warnings = Vec::new();
    if profile.gh_config_dir.is_some() && !gh_available {
        warnings
            .push("GitHub CLI is not installed, so gh integration will remain inactive.".into());
    }
    if let Some(directory) = &profile.gh_config_dir {
        if !Path::new(directory).is_dir() {
            warnings.push("The selected gh config directory does not currently exist.".into());
        }
    }

    Ok(ApplyPreview {
        repository: repository.clone(),
        profile: profile.clone(),
        changes,
        warnings,
    })
}

pub fn apply_profile(repository: &RepositoryRecord, profile: &Profile) -> Result<(), String> {
    let root = repository_root(&repository.path)?;
    let changes = config_changes(&root, profile)?;
    let mut changed = Vec::new();

    for change in &changes {
        let result = match &change.next_value {
            Some(value) => write_local_config(&root, &change.key, value),
            None => unset_local_config(&root, &change.key),
        };
        if let Err(error) = result {
            rollback_local_config(&root, &changed);
            return Err(format!(
                "No changes were kept because Git rejected {}: {error}",
                change.key
            ));
        }
        changed.push(change.clone());
    }
    Ok(())
}

pub fn build_push_preview(
    repository: &RepositoryRecord,
    profile: &Profile,
) -> Result<PushPreview, String> {
    let root = repository_root(&repository.path)?;
    output_text(&run_git(&root, &["rev-parse", "--verify", "HEAD"])?)?;
    let branch = current_branch(&root, "pushing to GitHub")?;
    let remote_url = validate_push_settings(&root, profile)?;

    let upstream = git_optional(
        &root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    );
    let status = output_text(&run_git(&root, &["status", "--porcelain"])?)?;
    Ok(PushPreview {
        repository: repository.clone(),
        profile: profile.clone(),
        branch,
        remote_url,
        upstream,
        has_uncommitted_changes: !status.is_empty(),
    })
}

pub fn build_commit_preview(
    repository: &RepositoryRecord,
    profile: &Profile,
) -> Result<CommitPreview, String> {
    let root = repository_root(&repository.path)?;
    let branch = current_branch(&root, "committing")?;
    validate_applied_identity(&root, profile)?;
    let changes = working_tree_changes(&root)?;
    let (push_remote_url, push_unavailable_reason) = match validate_push_settings(&root, profile) {
        Ok(remote_url) => (Some(remote_url), None),
        Err(error) => (None, Some(error)),
    };

    Ok(CommitPreview {
        repository: repository.clone(),
        profile: profile.clone(),
        branch,
        changes,
        push_remote_url,
        push_unavailable_reason,
    })
}

pub fn inspect_pull_request_source(
    repository: &RepositoryRecord,
    profile: &Profile,
    base_branch: &str,
) -> Result<PullRequestSourceState, String> {
    let root = repository_root(&repository.path)?;
    validate_branch_name(&root, base_branch)?;
    let current = current_branch(&root, "creating a pull request")?;
    let remote_url = validate_push_settings(&root, profile)?;
    let changes = working_tree_changes(&root)?;
    let base_ref = format!("refs/remotes/origin/{base_branch}");
    let fetch_ref = format!("refs/heads/{base_branch}:{base_ref}");
    // The stdio fixture supplies a local tracking ref; debug tests must not contact GitHub.
    if !(cfg!(debug_assertions)
        && std::env::var_os("GITCONTEXT_TEST_SKIP_PULL_REQUEST_FETCH").is_some())
    {
        output_text(&run_git(
            &root,
            &["fetch", "--no-tags", "origin", &fetch_ref],
        )?)
        .map_err(|error| format!("Could not refresh origin/{base_branch}: {error}"))?;
    }
    let range = format!("origin/{base_branch}..HEAD");
    let commits_ahead = output_text(&run_git(&root, &["rev-list", "--count", &range])?)?
        .parse::<u64>()
        .map_err(|_| "Git returned an invalid commit count.".to_string())?;
    let remote_branch = format!("refs/remotes/origin/{current}");
    let local_head = git_optional(&root, &["rev-parse", "HEAD"]);
    let remote_head = git_optional(&root, &["rev-parse", &remote_branch]);
    let branch_pushed = local_head.is_some() && local_head == remote_head;
    Ok(PullRequestSourceState {
        current_branch: current,
        remote_url,
        changes,
        commits_ahead,
        branch_pushed,
    })
}

pub fn create_working_branch(
    repository: &RepositoryRecord,
    profile: &Profile,
    branch: &str,
) -> Result<String, String> {
    let root = repository_root(&repository.path)?;
    validate_applied_identity(&root, profile)?;
    let branch = validate_branch_name(&root, branch)?;
    let current = current_branch(&root, "creating a branch")?;
    if current == branch {
        return Err("The requested branch is already checked out.".into());
    }
    for reference in [
        format!("refs/heads/{branch}"),
        format!("refs/remotes/origin/{branch}"),
    ] {
        let output = run_git(&root, &["show-ref", "--verify", "--quiet", &reference])?;
        if output.status.success() {
            return Err("A local or known remote branch with this name already exists.".into());
        }
    }
    if git_optional(&root, &["config", "--get", "remote.origin.url"]).is_some() {
        let remote_reference = format!("refs/heads/{branch}");
        let output = run_git(
            &root,
            &[
                "ls-remote",
                "--exit-code",
                "--heads",
                "origin",
                &remote_reference,
            ],
        )?;
        if output.status.success() {
            return Err("A remote branch with this name already exists on GitHub.".into());
        }
        if output.status.code() != Some(2) {
            output_text(&output)?;
        }
    }
    output_text(&run_git(&root, &["switch", "--create", &branch])?)?;
    Ok(branch)
}

pub fn commit_all_changes(
    repository: &RepositoryRecord,
    profile: &Profile,
    message: &str,
) -> Result<CommitResult, String> {
    let message = validate_commit_message(message)?;
    let preview = build_commit_preview(repository, profile)?;
    if preview.changes.is_empty() {
        return Err("There are no changes to commit.".into());
    }
    let root = repository_root(&repository.path)?;
    output_text(&run_git(&root, &["add", "--all"])?)?;
    let output = run_git(&root, &["commit", "--message", &message])?;
    output_text(&output).map_err(|error| {
        format!("Commit failed. Files may remain staged in the repository. {error}")
    })?;
    let commit_id = output_text(&run_git(&root, &["rev-parse", "--short", "HEAD"])?)?;

    Ok(CommitResult {
        branch: preview.branch,
        commit_id,
        message,
    })
}

pub fn push_current_branch(
    repository: &RepositoryRecord,
    profile: &Profile,
) -> Result<PushResult, String> {
    let preview = build_push_preview(repository, profile)?;
    let root = repository_root(&repository.path)?;
    let refspec = format!(
        "refs/heads/{branch}:refs/heads/{branch}",
        branch = preview.branch
    );
    let output = Command::new("git")
        .current_dir(&root)
        .args(["push", "--set-upstream", "--", "origin", refspec.as_str()])
        .output()
        .map_err(|error| format!("Could not start Git push: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("Git push exited with {}.", output.status)
        } else {
            detail
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = [stdout, stderr].into_iter().find(|value| !value.is_empty());
    Ok(PushResult {
        branch: preview.branch,
        remote_url: preview.remote_url,
        detail,
    })
}

pub fn refresh_sync_preview(
    repository: &RepositoryRecord,
    profile: &Profile,
) -> Result<SyncPreview, String> {
    let root = repository_root(&repository.path)?;
    output_text(&run_git(&root, &["rev-parse", "--verify", "HEAD"])?)?;
    let branch = current_branch(&root, "syncing with GitHub")?;
    let remote_url = validate_push_settings(&root, profile)
        .map_err(|error| error.replace("pushing to GitHub", "syncing with GitHub"))?;

    output_text(&run_git(
        &root,
        &["fetch", "--prune", "--no-tags", "origin"],
    )?)
    .map_err(|error| format!("Could not fetch from origin: {error}"))?;

    build_sync_preview(repository, profile, &root, branch, remote_url)
}

pub fn pull_current_branch(
    repository: &RepositoryRecord,
    profile: &Profile,
) -> Result<SyncPreview, String> {
    let preview = refresh_sync_preview(repository, profile)?;
    if !preview.changes.is_empty() {
        return Err("Commit or discard local changes before pulling from GitHub.".into());
    }
    let remote_branch = preview
        .remote_branch
        .as_deref()
        .ok_or_else(|| "The current branch does not exist on origin yet.".to_string())?;
    if preview.ahead > 0 && preview.behind > 0 {
        return Err(
            "The local and remote branches have diverged. Resolve them manually before pulling."
                .into(),
        );
    }
    if preview.behind == 0 {
        return Ok(preview);
    }

    let root = repository_root(&repository.path)?;
    if current_branch(&root, "syncing with GitHub")? != preview.branch
        || !working_tree_changes(&root)?.is_empty()
    {
        return Err(
            "The branch or working tree changed while preparing the pull. Refresh and try again."
                .into(),
        );
    }
    output_text(&run_git(&root, &["merge", "--ff-only", remote_branch])?)
        .map_err(|error| format!("Fast-forward pull failed: {error}"))?;
    let branch = current_branch(&root, "syncing with GitHub")?;
    let remote_url = validate_push_settings(&root, profile)?;
    build_sync_preview(repository, profile, &root, branch, remote_url)
}

fn build_sync_preview(
    repository: &RepositoryRecord,
    profile: &Profile,
    root: &Path,
    branch: String,
    remote_url: String,
) -> Result<SyncPreview, String> {
    let upstream = git_optional(
        root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    );
    let origin_branch = format!("origin/{branch}");
    let remote_reference = format!("refs/remotes/{origin_branch}");
    let remote_branch = run_git(
        root,
        &["show-ref", "--verify", "--quiet", &remote_reference],
    )?
    .status
    .success()
    .then_some(origin_branch);
    let (ahead, behind) = match remote_branch.as_deref() {
        Some(reference) => {
            let range = format!("HEAD...{reference}");
            let counts = output_text(&run_git(
                root,
                &["rev-list", "--left-right", "--count", &range],
            )?)?;
            parse_ahead_behind(&counts)?
        }
        None => (0, 0),
    };

    Ok(SyncPreview {
        repository: repository.clone(),
        profile: profile.clone(),
        branch,
        remote_url,
        upstream,
        remote_branch,
        changes: working_tree_changes(root)?,
        ahead,
        behind,
        fetched_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn parse_ahead_behind(value: &str) -> Result<(u64, u64), String> {
    let mut counts = value.split_whitespace();
    let ahead = counts
        .next()
        .and_then(|count| count.parse::<u64>().ok())
        .ok_or_else(|| "Git returned an invalid ahead count.".to_string())?;
    let behind = counts
        .next()
        .and_then(|count| count.parse::<u64>().ok())
        .ok_or_else(|| "Git returned an invalid behind count.".to_string())?;
    if counts.next().is_some() {
        return Err("Git returned invalid synchronization counts.".into());
    }
    Ok((ahead, behind))
}

pub fn validate_publish_source(repository: &RepositoryRecord) -> Result<(), String> {
    let root = repository_root(&repository.path)?;
    if git_optional(&root, &["config", "--get", "remote.origin.url"]).is_some() {
        return Err("This repository already has an origin remote.".into());
    }
    output_text(&run_git(&root, &["rev-parse", "--verify", "HEAD"])?)?;
    let status = output_text(&run_git(&root, &["status", "--porcelain"])?)?;
    if !status.is_empty() {
        return Err("Commit or discard all local changes before publishing to GitHub.".into());
    }
    let branch = output_text(&run_git(&root, &["branch", "--show-current"])?)?;
    if branch.is_empty() {
        return Err("Check out a branch before publishing to GitHub.".into());
    }
    Ok(())
}

pub fn origin_url(repository_path: &str) -> Result<String, String> {
    let root = repository_root(repository_path)?;
    git_optional(&root, &["config", "--get", "remote.origin.url"])
        .ok_or_else(|| "GitHub CLI did not configure the origin remote.".into())
}

/// Read current local identity without returning any configuration values.
pub fn inspect_identity_status(
    repository: &RepositoryRecord,
    profile: Option<&Profile>,
) -> Result<(String, Vec<WorkingTreeChange>, Vec<ConfigChange>), String> {
    let root = repository_root(&repository.path)?;
    let branch = current_branch(&root, "inspecting the repository")?;
    let changes = working_tree_changes(&root)?;
    // Reuse the apply plan so status agrees with what applying would change.
    let mismatches = match profile {
        Some(profile) => config_changes(&root, profile)?
            .into_iter()
            .filter(|change| change.current_value != change.next_value)
            .collect(),
        None => Vec::new(),
    };
    Ok((branch, changes, mismatches))
}

fn validate_github_ssh_remote(value: &str) -> Result<(), String> {
    parse_github_ssh_remote(value).map(|_| ())
}

fn parse_github_ssh_remote(value: &str) -> Result<(String, String), String> {
    let path = value
        .strip_prefix("git@github.com:")
        .and_then(|value| value.strip_suffix(".git"))
        .ok_or_else(|| {
            "Push requires an SSH origin in the form git@github.com:owner/repository.git."
                .to_string()
        })?;
    let mut parts = path.split('/');
    let owner = parts.next().unwrap_or_default();
    let repository = parts.next().unwrap_or_default();
    if owner.is_empty() || repository.is_empty() || parts.next().is_some() {
        return Err(
            "Push requires an SSH origin in the form git@github.com:owner/repository.git.".into(),
        );
    }
    Ok((owner.to_string(), repository.to_string()))
}

fn current_branch(root: &Path, action: &str) -> Result<String, String> {
    git_optional(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .filter(|branch| !branch.is_empty())
        .ok_or_else(|| format!("Check out a branch before {action}."))
}

fn validate_applied_identity(root: &Path, profile: &Profile) -> Result<(), String> {
    let matches_profile =
        read_local_config(root, "gitcontext.profileId").as_deref() == Some(profile.id.as_str());
    let matches_name =
        read_local_config(root, "user.name").as_deref() == Some(profile.git_name.as_str());
    let matches_email =
        read_local_config(root, "user.email").as_deref() == Some(profile.git_email.as_str());
    if !matches_profile || !matches_name || !matches_email {
        return Err("Reapply this Profile before committing.".into());
    }
    Ok(())
}

fn validate_push_settings(root: &Path, profile: &Profile) -> Result<String, String> {
    validate_applied_identity(root, profile)
        .map_err(|_| "Reapply this Profile before pushing to GitHub.".to_string())?;
    let remote_url = git_optional(root, &["config", "--get", "remote.origin.url"])
        .ok_or_else(|| "This repository does not have an origin remote.".to_string())?;
    validate_github_ssh_remote(&remote_url)?;
    let expected_ssh = desired_config(profile)?
        .into_iter()
        .find_map(|(key, value)| (key == "core.sshCommand").then_some(value).flatten())
        .ok_or_else(|| "Choose an SSH private key for this Profile before pushing.".to_string())?;
    if read_local_config(root, "core.sshCommand").as_deref() != Some(expected_ssh.as_str()) {
        return Err("Reapply this Profile before pushing to GitHub.".into());
    }
    Ok(remote_url)
}

fn validate_commit_message(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 || value.chars().any(char::is_control) {
        return Err("Commit message must contain 1 to 200 characters on one line.".into());
    }
    Ok(value.to_string())
}

fn validate_branch_name(root: &Path, value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 {
        return Err("Branch name must contain 1 to 200 characters.".into());
    }
    let output = run_git(root, &["check-ref-format", "--branch", value])?;
    if !output.status.success() {
        return Err("Branch name is not valid for Git.".into());
    }
    Ok(value.to_string())
}

fn working_tree_changes(root: &Path) -> Result<Vec<WorkingTreeChange>, String> {
    let status = output_text(&run_git(
        root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?)?;
    Ok(parse_working_tree_changes(&status))
}

fn parse_working_tree_changes(status: &str) -> Vec<WorkingTreeChange> {
    status
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            Some(WorkingTreeChange {
                status: line[..2].trim().to_string(),
                path: line[3..].trim().to_string(),
            })
        })
        .collect()
}

fn repository_root(input: &str) -> Result<PathBuf, String> {
    let requested = fs::canonicalize(input)
        .map_err(|error| format!("Repository path is not accessible: {error}"))?;
    if !requested.is_dir() {
        return Err("The selected repository path is not a directory.".into());
    }
    let output = run_git(&requested, &["rev-parse", "--show-toplevel"])?;
    let discovered_text = output_text(&output)?;
    let discovered = fs::canonicalize(discovered_text.trim())
        .map_err(|error| format!("Git returned an inaccessible repository root: {error}"))?;
    if discovered != requested {
        return Err(format!(
            "Select the repository root itself: {}",
            discovered.display()
        ));
    }
    Ok(discovered)
}

fn desired_config(profile: &Profile) -> Result<Vec<(String, Option<String>)>, String> {
    let mut values = vec![
        ("user.name".into(), Some(profile.git_name.clone())),
        ("user.email".into(), Some(profile.git_email.clone())),
        ("gitcontext.profileId".into(), Some(profile.id.clone())),
        ("gitcontext.profileName".into(), Some(profile.label.clone())),
        (
            "gitcontext.githubUser".into(),
            profile.github_username.clone(),
        ),
        (
            "gitcontext.ghConfigDir".into(),
            profile.gh_config_dir.clone(),
        ),
    ];
    if let Some(key_path) = &profile.ssh_key_path {
        let key = validate_ssh_private_key(key_path)?;
        let portable = key.to_string_lossy().replace('\\', "/");
        values.push((
            "core.sshCommand".into(),
            Some(format!("ssh -i \"{portable}\" -o IdentitiesOnly=yes")),
        ));
    } else {
        values.push(("core.sshCommand".into(), None));
    }
    Ok(values)
}

fn is_gitcontext_ssh_command(value: &str) -> bool {
    value
        .strip_prefix("ssh -i \"")
        .and_then(|path| path.strip_suffix("\" -o IdentitiesOnly=yes"))
        .is_some_and(|path| !path.is_empty() && !path.contains(['"', '\n', '\r']))
}

fn config_changes(root: &Path, profile: &Profile) -> Result<Vec<ConfigChange>, String> {
    Ok(desired_config(profile)?
        .into_iter()
        .filter_map(|(key, next_value)| {
            let current_value = read_local_config(root, &key);
            if next_value.is_none()
                && (current_value.is_none()
                    || (key == "core.sshCommand"
                        && !current_value
                            .as_deref()
                            .is_some_and(is_gitcontext_ssh_command)))
            {
                return None;
            }
            Some(ConfigChange {
                key,
                current_value,
                next_value,
            })
        })
        .collect())
}

fn validate_ssh_private_key(input: &str) -> Result<PathBuf, String> {
    if input.contains('"') {
        return Err("SSH key paths containing a quote are not supported.".into());
    }
    if input.to_ascii_lowercase().ends_with(".pub") {
        return Err("Choose the private SSH key, not its .pub file.".into());
    }
    let key = fs::canonicalize(expand_home(input))
        .map_err(|error| format!("SSH key is not accessible: {error}"))?;
    if !key.is_file() {
        return Err("The selected SSH key is not a regular file.".into());
    }
    let home =
        home_directory().ok_or_else(|| "Could not locate the user home directory.".to_string())?;
    let ssh_root = fs::canonicalize(home.join(".ssh"))
        .map_err(|error| format!("The ~/.ssh directory is not accessible: {error}"))?;
    if !key.starts_with(&ssh_root) {
        return Err("For the MVP, SSH private keys must stay inside ~/.ssh.".into());
    }
    Ok(key)
}

fn expand_home(input: &str) -> PathBuf {
    if input == "~" {
        return home_directory().unwrap_or_else(|| PathBuf::from(input));
    }
    if let Some(remainder) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\"))
    {
        if let Some(home) = home_directory() {
            return home.join(remainder);
        }
    }
    PathBuf::from(input)
}

pub fn home_directory() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
}

fn read_local_config(root: &Path, key: &str) -> Option<String> {
    run_git(root, &["config", "--local", "--get", key])
        .ok()
        .and_then(|output| output_text(&output).ok())
}

fn write_local_config(root: &Path, key: &str, value: &str) -> Result<(), String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["config", "--local", "--replace-all", key, value])
        .output()
        .map_err(|error| format!("Could not start git: {error}"))?;
    output_text(&output).map(|_| ())
}

fn restore_local_config(root: &Path, key: &str, previous: Option<String>) {
    if let Some(value) = previous {
        let _ = write_local_config(root, key, &value);
    } else {
        let _ = unset_local_config(root, key);
    }
}

fn unset_local_config(root: &Path, key: &str) -> Result<(), String> {
    let output = run_git(root, &["config", "--local", "--unset-all", key])?;
    output_text(&output).map(|_| ())
}

fn rollback_local_config(root: &Path, changed: &[ConfigChange]) {
    for change in changed.iter().rev() {
        restore_local_config(root, &change.key, change.current_value.clone());
    }
}

fn git_optional(root: &Path, args: &[&str]) -> Option<String> {
    run_git(root, args)
        .ok()
        .and_then(|output| output_text(&output).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn run_git(root: &Path, args: &[&str]) -> Result<Output, String> {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("Could not start git: {error}"))
}

fn output_text(output: &Output) -> Result<String, String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if message.is_empty() {
        format!("Command exited with status {}", output.status)
    } else {
        message
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        apply_profile, build_commit_preview, build_preview, commit_all_changes,
        commit_exact_changes, create_working_branch, exact_changes, is_gitcontext_ssh_command,
        parse_ahead_behind, parse_working_tree_changes, read_local_config, rollback_local_config,
        run_git, unset_local_config, validate_commit_message, validate_github_ssh_remote,
        write_local_config,
    };
    use crate::models::{ConfigChange, Profile, RepositoryRecord};

    fn apply_test_repository() -> (PathBuf, RepositoryRecord, Profile) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("git-context-apply-test-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        assert!(run_git(&root, &["init"]).unwrap().status.success());
        let repository = RepositoryRecord {
            id: "test-repository".into(),
            name: "test".into(),
            path: root.to_string_lossy().into_owned(),
            remote_url: None,
            branch: None,
            profile_id: None,
            last_applied_at: None,
            auto_approve: Default::default(),
        };
        let profile = Profile {
            id: "test-profile".into(),
            label: "Test".into(),
            accent: "#123456".into(),
            git_name: "Test User".into(),
            git_email: "test@example.com".into(),
            github_username: None,
            ssh_key_path: None,
            gh_config_dir: None,
            auto_approve: Default::default(),
        };
        (root, repository, profile)
    }

    fn committed_test_repository() -> (PathBuf, RepositoryRecord, Profile) {
        let (root, repository, profile) = apply_test_repository();
        apply_profile(&repository, &profile).unwrap();
        fs::write(root.join("old name.txt"), "initial").unwrap();
        assert!(run_git(&root, &["add", "--all"]).unwrap().status.success());
        assert!(run_git(&root, &["commit", "-m", "Initial"])
            .unwrap()
            .status
            .success());
        (root, repository, profile)
    }

    #[test]
    fn fingerprint_git_references_are_read_without_tauri() {
        let (root, repository, profile) = apply_test_repository();
        assert!(super::head_commit(&repository.path).unwrap().is_empty());
        assert!(
            super::reference_commit(&repository.path, "refs/remotes/origin/main")
                .unwrap()
                .is_empty()
        );
        assert!(super::tracking_branch(&repository.path).unwrap().is_none());
        fs::write(root.join("first.txt"), "first").unwrap();
        assert!(run_git(&root, &["add", "--all"]).unwrap().status.success());
        apply_profile(&repository, &profile).unwrap();
        assert!(run_git(&root, &["commit", "-m", "Initial"])
            .unwrap()
            .status
            .success());
        assert!(!super::head_commit(&repository.path).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scoped_commit_excludes_files_created_after_preview() {
        let (root, repository, profile) = committed_test_repository();
        fs::write(root.join("first file.txt"), "previewed").unwrap();
        let changes = exact_changes(&repository.path).unwrap();
        fs::write(root.join("later.txt"), "later").unwrap();
        commit_exact_changes(&repository, &profile, "Scoped", &changes).unwrap();
        let names = super::output_text(
            &run_git(&root, &["show", "--pretty=format:", "--name-only", "HEAD"]).unwrap(),
        )
        .unwrap();
        assert!(names.contains("first file.txt"));
        assert!(!names.contains("later.txt"));
        assert!(root.join("later.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scoped_commit_handles_both_sides_of_rename() {
        let (root, repository, profile) = committed_test_repository();
        assert!(run_git(&root, &["mv", "old name.txt", "new name.txt"])
            .unwrap()
            .status
            .success());
        let changes = exact_changes(&repository.path).unwrap();
        assert_eq!(changes[0].path, "new name.txt");
        assert_eq!(changes[0].original_path.as_deref(), Some("old name.txt"));
        commit_exact_changes(&repository, &profile, "Rename", &changes).unwrap();
        let names = super::output_text(
            &run_git(
                &root,
                &["show", "--pretty=format:", "--name-status", "HEAD"],
            )
            .unwrap(),
        )
        .unwrap();
        assert!(names.contains("old name.txt"));
        assert!(names.contains("new name.txt"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scoped_commit_rejects_unrelated_staged_file() {
        let (root, repository, profile) = committed_test_repository();
        fs::write(root.join("previewed.txt"), "previewed").unwrap();
        let changes = exact_changes(&repository.path).unwrap();
        fs::write(root.join("unrelated.txt"), "unrelated").unwrap();
        assert!(run_git(&root, &["add", "--", "unrelated.txt"])
            .unwrap()
            .status
            .success());
        let before = super::head_commit(&repository.path).unwrap();
        assert!(commit_exact_changes(&repository, &profile, "Should fail", &changes).is_err());
        assert_eq!(super::head_commit(&repository.path).unwrap(), before);
        assert_eq!(
            super::cached_paths(&root).unwrap(),
            ["unrelated.txt".to_string()].into()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn apply_removes_stale_optional_identity_settings() {
        let (root, repository, profile) = apply_test_repository();
        write_local_config(&root, "gitcontext.githubUser", "old-user").unwrap();
        write_local_config(&root, "gitcontext.ghConfigDir", "old-gh-directory").unwrap();

        let preview = build_preview(&repository, &profile, false).unwrap();
        for key in ["gitcontext.githubUser", "gitcontext.ghConfigDir"] {
            let change = preview
                .changes
                .iter()
                .find(|change| change.key == key)
                .unwrap();
            assert!(change.current_value.is_some());
            assert_eq!(change.next_value, None);
        }
        let json = serde_json::to_value(&preview).unwrap();
        assert!(json["changes"].as_array().unwrap().iter().any(|change| {
            change["key"] == "gitcontext.ghConfigDir" && change["nextValue"].is_null()
        }));

        apply_profile(&repository, &profile).unwrap();
        assert_eq!(read_local_config(&root, "gitcontext.githubUser"), None);
        assert_eq!(read_local_config(&root, "gitcontext.ghConfigDir"), None);
        assert_eq!(
            read_local_config(&root, "user.name").as_deref(),
            Some("Test User")
        );
        assert_eq!(
            read_local_config(&root, "user.email").as_deref(),
            Some("test@example.com")
        );
        assert_eq!(
            read_local_config(&root, "gitcontext.profileId").as_deref(),
            Some("test-profile")
        );
        assert_eq!(
            read_local_config(&root, "gitcontext.profileName").as_deref(),
            Some("Test")
        );
        let repeated = build_preview(&repository, &profile, false).unwrap();
        assert!(!repeated
            .changes
            .iter()
            .any(|change| change.next_value.is_none()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn apply_removes_only_gitcontext_ssh_command() {
        let (root, repository, profile) = apply_test_repository();
        let managed = "ssh -i \"/temporary/key\" -o IdentitiesOnly=yes";
        write_local_config(&root, "core.sshCommand", managed).unwrap();
        let preview = build_preview(&repository, &profile, true).unwrap();
        let change = preview
            .changes
            .iter()
            .find(|change| change.key == "core.sshCommand")
            .unwrap();
        assert_eq!(change.current_value.as_deref(), Some(managed));
        assert_eq!(change.next_value, None);
        apply_profile(&repository, &profile).unwrap();
        assert_eq!(read_local_config(&root, "core.sshCommand"), None);

        write_local_config(&root, "core.sshCommand", "ssh -F /dev/null").unwrap();
        let preview = build_preview(&repository, &profile, true).unwrap();
        assert!(!preview
            .changes
            .iter()
            .any(|change| change.key == "core.sshCommand"));
        apply_profile(&repository, &profile).unwrap();
        assert_eq!(
            read_local_config(&root, "core.sshCommand").as_deref(),
            Some("ssh -F /dev/null")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn identifies_gitcontext_ssh_command_format() {
        assert!(is_gitcontext_ssh_command(
            "ssh -i \"C:/keys/test key\" -o IdentitiesOnly=yes"
        ));
        for value in [
            "ssh -F /dev/null",
            "ssh -i /temporary/key -o IdentitiesOnly=yes",
            "ssh -i \"\" -o IdentitiesOnly=yes",
            "ssh -i \"/temporary/key\" -o StrictHostKeyChecking=no",
            "ssh -i \"/temporary/\"key\" -o IdentitiesOnly=yes",
        ] {
            assert!(!is_gitcontext_ssh_command(value));
        }
    }

    #[test]
    fn rollback_restores_deleted_and_newly_set_keys() {
        let (root, _, _) = apply_test_repository();
        let old_value = "old-gh-directory";
        write_local_config(&root, "gitcontext.ghConfigDir", old_value).unwrap();
        let changed = [
            ConfigChange {
                key: "gitcontext.ghConfigDir".into(),
                current_value: Some(old_value.into()),
                next_value: None,
            },
            ConfigChange {
                key: "gitcontext.githubUser".into(),
                current_value: None,
                next_value: Some("new-user".into()),
            },
        ];
        unset_local_config(&root, &changed[0].key).unwrap();
        write_local_config(&root, &changed[1].key, "new-user").unwrap();
        rollback_local_config(&root, &changed);
        assert_eq!(
            read_local_config(&root, &changed[0].key).as_deref(),
            Some(old_value)
        );
        assert_eq!(read_local_config(&root, &changed[1].key), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn push_accepts_only_standard_github_ssh_remotes() {
        assert!(validate_github_ssh_remote("git@github.com:owner/repository.git").is_ok());
        assert!(validate_github_ssh_remote("https://github.com/owner/repository.git").is_err());
        assert!(validate_github_ssh_remote("git@github.com:owner/group/repository.git").is_err());
    }

    #[test]
    fn parses_ahead_and_behind_counts() {
        assert_eq!(parse_ahead_behind("3\t2").unwrap(), (3, 2));
        assert!(parse_ahead_behind("3").is_err());
        assert!(parse_ahead_behind("ahead behind").is_err());
    }

    #[test]
    fn commit_message_is_single_line_and_bounded() {
        assert_eq!(
            validate_commit_message("  Update README  ").unwrap(),
            "Update README"
        );
        assert!(validate_commit_message("").is_err());
        assert!(validate_commit_message("first\nsecond").is_err());
        assert!(validate_commit_message(&"a".repeat(201)).is_err());
    }

    #[test]
    fn parses_working_tree_status_for_preview() {
        let changes = parse_working_tree_changes(" M src/App.tsx\n?? notes.txt\n");
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].status, "M");
        assert_eq!(changes[0].path, "src/App.tsx");
        assert_eq!(changes[1].status, "??");
    }

    #[test]
    fn commits_all_changes_on_the_current_branch() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("git-context-commit-test-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        run_git(&root, &["init", "--initial-branch=main"]).unwrap();
        run_git(&root, &["config", "--local", "user.name", "Test User"]).unwrap();
        run_git(
            &root,
            &["config", "--local", "user.email", "test@example.com"],
        )
        .unwrap();
        run_git(
            &root,
            &["config", "--local", "gitcontext.profileId", "test-profile"],
        )
        .unwrap();
        fs::write(root.join("README.md"), "test\n").unwrap();

        let repository = RepositoryRecord {
            id: "test-repository".into(),
            name: "test".into(),
            path: root.to_string_lossy().into_owned(),
            remote_url: None,
            branch: Some("main".into()),
            profile_id: Some("test-profile".into()),
            last_applied_at: Some("now".into()),
            auto_approve: Default::default(),
        };
        let profile = Profile {
            id: "test-profile".into(),
            label: "Test".into(),
            accent: "#123456".into(),
            git_name: "Test User".into(),
            git_email: "test@example.com".into(),
            github_username: None,
            ssh_key_path: None,
            gh_config_dir: None,
            auto_approve: Default::default(),
        };

        let preview = build_commit_preview(&repository, &profile).unwrap();
        assert_eq!(preview.branch, "main");
        assert_eq!(preview.changes.len(), 1);
        let result = commit_all_changes(&repository, &profile, "Initial commit").unwrap();
        assert_eq!(result.branch, "main");
        assert!(!result.commit_id.is_empty());
        assert!(build_commit_preview(&repository, &profile)
            .unwrap()
            .changes
            .is_empty());
        assert_eq!(
            create_working_branch(&repository, &profile, "feature/test").unwrap(),
            "feature/test"
        );
        assert!(create_working_branch(&repository, &profile, "bad branch").is_err());

        fs::remove_dir_all(root).unwrap();
    }
}

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::models::{
    ApplyPreview, CommitPreview, CommitResult, ConfigChange, Profile, PushPreview, PushResult,
    RepositoryRecord, WorkingTreeChange,
};

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
        .find_map(|(key, value)| (key == "core.sshCommand").then_some(value))
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

pub fn build_preview(
    repository: &RepositoryRecord,
    profile: &Profile,
    gh_available: bool,
) -> Result<ApplyPreview, String> {
    let root = repository_root(&repository.path)?;
    let desired = desired_config(profile)?;
    let changes = desired
        .iter()
        .map(|(key, value)| ConfigChange {
            key: key.clone(),
            current_value: read_local_config(&root, key),
            next_value: value.clone(),
        })
        .collect();

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
    let desired = desired_config(profile)?;
    let previous: BTreeMap<String, Option<String>> = desired
        .iter()
        .map(|(key, _)| (key.clone(), read_local_config(&root, key)))
        .collect();
    let mut changed_keys: Vec<String> = Vec::new();

    for (key, value) in &desired {
        if let Err(error) = write_local_config(&root, key, value) {
            for changed in changed_keys.iter().rev() {
                restore_local_config(&root, changed, previous.get(changed).cloned().flatten());
            }
            return Err(format!(
                "No changes were kept because Git rejected {key}: {error}"
            ));
        }
        changed_keys.push(key.clone());
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
    let status = output_text(&run_git(
        &root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?)?;
    let changes = parse_working_tree_changes(&status);
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

fn validate_github_ssh_remote(value: &str) -> Result<(), String> {
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
    Ok(())
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
        .find_map(|(key, value)| (key == "core.sshCommand").then_some(value))
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

fn desired_config(profile: &Profile) -> Result<Vec<(String, String)>, String> {
    let mut values = vec![
        ("user.name".into(), profile.git_name.clone()),
        ("user.email".into(), profile.git_email.clone()),
        ("gitcontext.profileId".into(), profile.id.clone()),
        ("gitcontext.profileName".into(), profile.label.clone()),
    ];
    if let Some(username) = &profile.github_username {
        values.push(("gitcontext.githubUser".into(), username.clone()));
    }
    if let Some(directory) = &profile.gh_config_dir {
        values.push(("gitcontext.ghConfigDir".into(), directory.clone()));
    }
    if let Some(key_path) = &profile.ssh_key_path {
        let key = validate_ssh_private_key(key_path)?;
        let portable = key.to_string_lossy().replace('\\', "/");
        values.push((
            "core.sshCommand".into(),
            format!("ssh -i \"{portable}\" -o IdentitiesOnly=yes"),
        ));
    }
    Ok(values)
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
    git_optional(root, &["config", "--local", "--get", key])
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
        let _ = Command::new("git")
            .current_dir(root)
            .args(["config", "--local", "--unset-all", key])
            .output();
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
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        build_commit_preview, commit_all_changes, parse_working_tree_changes, run_git,
        validate_commit_message, validate_github_ssh_remote,
    };
    use crate::models::{Profile, RepositoryRecord};

    #[test]
    fn push_accepts_only_standard_github_ssh_remotes() {
        assert!(validate_github_ssh_remote("git@github.com:owner/repository.git").is_ok());
        assert!(validate_github_ssh_remote("https://github.com/owner/repository.git").is_err());
        assert!(validate_github_ssh_remote("git@github.com:owner/group/repository.git").is_err());
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

        fs::remove_dir_all(root).unwrap();
    }
}

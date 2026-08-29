use std::{
    collections::BTreeMap,
    env,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::models::{ApplyPreview, ConfigChange, Profile, RepositoryRecord};

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

pub fn build_preview(repository: &RepositoryRecord, profile: &Profile, gh_available: bool) -> Result<ApplyPreview, String> {
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
        warnings.push("GitHub CLI is not installed, so gh integration will remain inactive.".into());
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
            return Err(format!("No changes were kept because Git rejected {key}: {error}"));
        }
        changed_keys.push(key.clone());
    }
    Ok(())
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
    let home = home_directory().ok_or_else(|| "Could not locate the user home directory.".to_string())?;
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
    if let Some(remainder) = input.strip_prefix("~/").or_else(|| input.strip_prefix("~\\")) {
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

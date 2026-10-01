use crate::{
    environment::command_available,
    git_ops,
    models::{
        validate_profile_id, GhProfileStatus, ManagedPullRequest, Profile, PullRequestCheck,
        PullRequestSummary, RepositoryRecord,
    },
    storage::StateStore,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

const GITHUB_DEVICE_URL: &str = "https://github.com/login/device";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubAuthPrompt {
    pub profile_id: String,
    pub code: String,
    pub verification_url: &'static str,
}

#[derive(Deserialize)]
pub(crate) struct GithubApiRepository {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub ssh_url: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePullRequestInput {
    pub repository_id: String,
    pub profile_id: String,
    pub base_branch: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePullRequestInput {
    pub repository_id: String,
    pub profile_id: String,
    pub number: u64,
    pub strategy: String,
    pub expected_head_oid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub head_ref_oid: String,
    pub mergeable: String,
    pub merge_state_status: String,
    #[serde(default)]
    pub review_decision: String,
    #[serde(default)]
    pub author: Option<serde_json::Value>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub merged_at: Option<String>,
    #[serde(default)]
    pub status_check_rollup: Vec<serde_json::Value>,
}

pub(crate) fn validate_github_repository_name(value: &str) -> Result<String, String> {
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

pub(crate) fn validate_github_ssh_url(value: &str) -> Result<(String, String), String> {
    let value = value.trim();
    let path = value
        .strip_prefix("git@github.com:")
        .and_then(|value| value.strip_suffix(".git"))
        .ok_or_else(|| {
            "Use a GitHub SSH URL in the form git@github.com:owner/repository.git.".to_string()
        })?;
    let mut parts = path.split('/');
    let owner = parts.next().unwrap_or_default();
    let repository = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || owner.is_empty()
        || owner.len() > 100
        || owner.starts_with('-')
        || owner.ends_with('-')
        || !owner
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("The GitHub owner in the SSH URL is invalid.".into());
    }
    let repository = validate_github_repository_name(repository)?;
    Ok((
        format!("git@github.com:{owner}/{repository}.git"),
        repository,
    ))
}

pub(crate) fn validate_github_description(value: Option<String>) -> Result<Option<String>, String> {
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

pub(crate) fn validate_pull_request_title(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 256 || value.chars().any(char::is_control) {
        return Err("Pull request title must contain 1 to 256 characters on one line.".into());
    }
    Ok(value.to_string())
}

pub(crate) fn validate_pull_request_body(value: &str) -> Result<String, String> {
    if value.chars().count() > 65_536
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\r' | '\n' | '\t'))
    {
        return Err(
            "Pull request body is too long or contains unsupported control characters.".into(),
        );
    }
    Ok(value.trim().to_string())
}

pub(crate) fn connected_gh_directory(
    store: &StateStore,
    profile: &Profile,
) -> Result<PathBuf, String> {
    let directory = resolve_gh_config_dir(store, &profile.id, profile.gh_config_dir.as_deref())?;
    let status = inspect_gh_directory(&directory);
    let username = status
        .username
        .filter(|_| status.authenticated)
        .ok_or_else(|| {
            status.detail.unwrap_or_else(|| {
                "Connect this Profile to GitHub before creating a pull request.".into()
            })
        })?;
    if let Some(expected) = profile.github_username.as_deref() {
        if !expected.eq_ignore_ascii_case(&username) {
            return Err(format!(
                "The Profile expects @{expected}, but GitHub CLI is authenticated as @{username}."
            ));
        }
    }
    Ok(directory)
}

pub(crate) fn github_repository_name(repository: &RepositoryRecord) -> Result<String, String> {
    let remote_url = git_ops::origin_url(&repository.path)?;
    let (normalized, _) = validate_github_ssh_url(&remote_url)?;
    normalized
        .strip_prefix("git@github.com:")
        .and_then(|value| value.strip_suffix(".git"))
        .map(str::to_string)
        .ok_or_else(|| "The origin remote is not a supported GitHub SSH URL.".into())
}

pub(crate) fn github_default_branch(
    directory: &Path,
    repository_path: &str,
    repository_name_with_owner: &str,
) -> Result<String, String> {
    let mut command = gh_command(directory);
    let output = command
        .current_dir(repository_path)
        .args([
            "repo",
            "view",
            repository_name_with_owner,
            "--json",
            "defaultBranchRef",
            "--jq",
            ".defaultBranchRef.name",
        ])
        .output()
        .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
    let branch = github_output_text(&output)?;
    if branch.is_empty() {
        return Err("GitHub did not return a default branch for this repository.".into());
    }
    Ok(branch)
}

pub(crate) fn pull_request_json_fields() -> &'static str {
    "number,url,title,state,isDraft,baseRefName,headRefName,headRefOid,mergeable,mergeStateStatus,reviewDecision,author,updatedAt,mergedAt,statusCheckRollup"
}

pub(crate) fn read_pull_request(
    directory: &Path,
    repository_path: &str,
    repository_name_with_owner: &str,
    number: u64,
) -> Result<GithubPullRequest, String> {
    let number = number.to_string();
    let mut command = gh_command(directory);
    let output = command
        .current_dir(repository_path)
        .args([
            "pr",
            "view",
            number.as_str(),
            "--repo",
            repository_name_with_owner,
            "--json",
            pull_request_json_fields(),
        ])
        .output()
        .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
    let text = github_output_text(&output)?;
    serde_json::from_str(&text)
        .map_err(|error| format!("GitHub CLI returned invalid pull request data: {error}"))
}

pub(crate) fn normalize_pull_request(item: GithubPullRequest) -> ManagedPullRequest {
    let author = item
        .author
        .as_ref()
        .and_then(|value| value.get("login"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    ManagedPullRequest {
        number: item.number,
        url: item.url,
        title: item.title,
        state: item.state,
        is_draft: item.is_draft,
        base_branch: item.base_ref_name,
        head_branch: item.head_ref_name,
        head_oid: item.head_ref_oid,
        mergeable: item.mergeable,
        merge_state_status: item.merge_state_status,
        review_decision: item.review_decision,
        author,
        updated_at: item.updated_at,
        checks: item
            .status_check_rollup
            .iter()
            .map(normalize_pull_request_check)
            .collect(),
    }
}

pub(crate) fn normalize_pull_request_check(value: &serde_json::Value) -> PullRequestCheck {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let name = text("name")
        .or_else(|| text("context"))
        .unwrap_or_else(|| "GitHub check".into());
    let state = text("conclusion")
        .or_else(|| text("state"))
        .or_else(|| text("status"))
        .unwrap_or_else(|| "PENDING".into())
        .to_ascii_uppercase();
    let bucket = check_bucket(&state).to_string();
    PullRequestCheck {
        name,
        state,
        bucket,
        link: text("detailsUrl").or_else(|| text("targetUrl")),
        workflow: text("workflowName"),
    }
}

pub(crate) fn check_bucket(state: &str) -> &'static str {
    match state {
        "SUCCESS" | "NEUTRAL" => "pass",
        "SKIPPED" => "skipping",
        "CANCELLED" => "cancel",
        "FAILURE" | "ERROR" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE" | "STALE" => {
            "fail"
        }
        _ => "pending",
    }
}

pub(crate) fn validate_pull_request_for_merge(
    pull_request: &GithubPullRequest,
    expected_head_oid: &str,
) -> Result<(), String> {
    if pull_request.state != "OPEN" {
        return Err("Only an open pull request can be merged.".into());
    }
    if pull_request.is_draft {
        return Err("Mark the pull request ready for review before merging it.".into());
    }
    if pull_request.head_ref_oid != expected_head_oid {
        return Err("The pull request received new commits. Refresh it before merging.".into());
    }
    if pull_request.mergeable != "MERGEABLE" || pull_request.merge_state_status != "CLEAN" {
        return Err("GitHub reports that this pull request is not currently safe to merge.".into());
    }
    if pull_request.review_decision == "CHANGES_REQUESTED" {
        return Err("Resolve requested review changes before merging this pull request.".into());
    }
    let checks: Vec<PullRequestCheck> = pull_request
        .status_check_rollup
        .iter()
        .map(normalize_pull_request_check)
        .collect();
    if checks
        .iter()
        .any(|check| !matches!(check.bucket.as_str(), "pass" | "skipping"))
    {
        return Err("Wait for all CI checks to pass before merging this pull request.".into());
    }
    Ok(())
}

pub(crate) fn find_existing_pull_request(
    directory: &Path,
    repository_path: &str,
    repository_name_with_owner: &str,
    branch: &str,
) -> Result<Option<PullRequestSummary>, String> {
    let mut command = gh_command(directory);
    let output = command
        .current_dir(repository_path)
        .args([
            "pr",
            "list",
            "--repo",
            repository_name_with_owner,
            "--head",
            branch,
            "--state",
            "open",
            "--limit",
            "1",
            "--json",
            "number,url,title",
        ])
        .output()
        .map_err(|error| format!("Could not start GitHub CLI: {error}"))?;
    let text = github_output_text(&output)?;
    let mut pull_requests: Vec<PullRequestSummary> = serde_json::from_str(&text)
        .map_err(|error| format!("GitHub CLI returned invalid pull request data: {error}"))?;
    Ok(pull_requests.pop())
}

pub(crate) fn github_output_text(output: &std::process::Output) -> Result<String, String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("GitHub CLI exited with {}.", output.status)
    } else {
        detail
    })
}

pub(crate) fn resolve_gh_config_dir(
    store: &StateStore,
    profile_id: &str,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    validate_profile_id(profile_id)?;
    let directory = match requested.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => store
            .state_path()
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

pub(crate) fn gh_command(directory: &Path) -> Command {
    let mut command = Command::new("gh");
    command
        .env("GH_CONFIG_DIR", directory)
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_ENTERPRISE_TOKEN")
        .env_remove("GITHUB_ENTERPRISE_TOKEN");
    command
}

pub(crate) fn spawn_github_auth_reader<R: Read + Send + 'static>(
    stream: R,
    on_prompt: Arc<dyn Fn(GithubAuthPrompt) + Send + Sync>,
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
            on_prompt(prompt);
            let _ = open_github_device_page();
            break;
        }
    })
}

pub(crate) fn extract_github_device_code(line: &str) -> Option<String> {
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
pub(crate) fn open_github_device_page() -> Result<(), String> {
    Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", GITHUB_DEVICE_URL])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the GitHub authentication page: {error}"))
}

#[cfg(target_os = "macos")]
pub(crate) fn open_github_device_page() -> Result<(), String> {
    Command::new("open")
        .arg(GITHUB_DEVICE_URL)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the GitHub authentication page: {error}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn open_github_device_page() -> Result<(), String> {
    Command::new("xdg-open")
        .arg(GITHUB_DEVICE_URL)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the GitHub authentication page: {error}"))
}

pub(crate) fn inspect_gh_directory(directory: &Path) -> GhProfileStatus {
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
    use super::{
        check_bucket, extract_github_device_code, validate_github_repository_name,
        validate_github_ssh_url, validate_pull_request_body, validate_pull_request_for_merge,
        validate_pull_request_title, GithubPullRequest,
    };

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

    #[test]
    fn validates_and_normalizes_github_ssh_urls() {
        assert_eq!(
            validate_github_ssh_url(" git@github.com:student-user/course-project.git ").unwrap(),
            (
                "git@github.com:student-user/course-project.git".into(),
                "course-project".into()
            )
        );
        assert!(
            validate_github_ssh_url("git\\@github.com:student-user/course-project.git").is_err()
        );
        assert!(validate_github_ssh_url("https://github.com/owner/repo.git").is_err());
        assert!(validate_github_ssh_url("git@github.com:owner/group/repo.git").is_err());
    }

    #[test]
    fn validates_pull_request_text() {
        assert_eq!(
            validate_pull_request_title(" Add PR workflow ").unwrap(),
            "Add PR workflow"
        );
        assert!(validate_pull_request_title("first\nsecond").is_err());
        assert_eq!(
            validate_pull_request_body(" Summary\n\n- Tested\n").unwrap(),
            "Summary\n\n- Tested"
        );
        assert!(validate_pull_request_body("invalid\0body").is_err());
    }

    #[test]
    fn categorizes_github_check_states() {
        assert_eq!(check_bucket("SUCCESS"), "pass");
        assert_eq!(check_bucket("SKIPPED"), "skipping");
        assert_eq!(check_bucket("FAILURE"), "fail");
        assert_eq!(check_bucket("CANCELLED"), "cancel");
        assert_eq!(check_bucket("IN_PROGRESS"), "pending");
    }

    #[test]
    fn only_allows_a_fresh_clean_pull_request_with_finished_checks() {
        let ready = serde_json::json!({
            "number": 8,
            "url": "https://github.com/example/repo/pull/8",
            "title": "Ready",
            "state": "OPEN",
            "isDraft": false,
            "baseRefName": "main",
            "headRefName": "feature/ready",
            "headRefOid": "0123456789abcdef0123456789abcdef01234567",
            "mergeable": "MERGEABLE",
            "mergeStateStatus": "CLEAN",
            "reviewDecision": "APPROVED",
            "author": { "login": "example" },
            "updatedAt": "2026-09-27T00:00:00Z",
            "mergedAt": null,
            "statusCheckRollup": [{ "name": "CI", "status": "COMPLETED", "conclusion": "SUCCESS" }]
        });
        let mut pull_request: GithubPullRequest = serde_json::from_value(ready).unwrap();
        let head = pull_request.head_ref_oid.clone();
        assert!(validate_pull_request_for_merge(&pull_request, &head).is_ok());
        pull_request.status_check_rollup = vec![serde_json::json!({
            "name": "CI", "status": "IN_PROGRESS", "conclusion": ""
        })];
        assert!(validate_pull_request_for_merge(&pull_request, &head).is_err());
        assert!(validate_pull_request_for_merge(&pull_request, "ffffffff").is_err());
    }
}

use gitcontext_core::models::{
    AppData, ApplyPreview, AutoApprove, ConfigChange, ManagedPullRequest, Profile,
    PullRequestSummary, RepositoryRecord, WorkingTreeChange,
};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubDto {
    pub authenticated: bool,
    pub username: Option<String>,
    pub matches_profile: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyDto {
    configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto<'a> {
    id: &'a str,
    label: &'a str,
    git_name: &'a str,
    git_email: &'a str,
    github_username: &'a Option<String>,
    ssh_key: SshKeyDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<GithubDto>,
}

impl<'a> ProfileDto<'a> {
    pub fn new(profile: &'a Profile, github: Option<GithubDto>) -> Self {
        Self {
            id: &profile.id,
            label: &profile.label,
            git_name: &profile.git_name,
            git_email: &profile.git_email,
            github_username: &profile.github_username,
            ssh_key: SshKeyDto {
                configured: profile.ssh_key_path.is_some(),
                file_name: profile.ssh_key_path.as_deref().and_then(file_name),
            },
            github,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDto<'a> {
    id: &'a str,
    name: &'a str,
    path: &'a str,
    remote_url: &'a Option<String>,
    branch: &'a Option<String>,
    profile_id: &'a Option<String>,
    profile_label: Option<&'a str>,
    applied: bool,
    last_applied_at: &'a Option<String>,
    auto_approve: &'a AutoApprove,
}

impl<'a> RepositoryDto<'a> {
    pub fn new(repository: &'a RepositoryRecord, data: &'a AppData) -> Self {
        Self {
            id: &repository.id,
            name: &repository.name,
            path: &repository.path,
            remote_url: &repository.remote_url,
            branch: &repository.branch,
            profile_id: &repository.profile_id,
            profile_label: repository.profile_id.as_ref().and_then(|id| {
                data.profiles
                    .iter()
                    .find(|profile| &profile.id == id)
                    .map(|profile| profile.label.as_str())
            }),
            applied: repository.last_applied_at.is_some(),
            last_applied_at: &repository.last_applied_at,
            auto_approve: &repository.auto_approve,
        }
    }
}

fn file_name(path: &str) -> Option<String> {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn masked_value(key: &str, value: &Option<String>) -> Option<String> {
    let value = value.as_deref()?;
    match key {
        "gitcontext.ghConfigDir" => Some("(gh config directory)".into()),
        "core.sshCommand" => {
            let key_path = value
                .strip_prefix("ssh -i \"")
                .and_then(|part| part.strip_suffix("\" -o IdentitiesOnly=yes"));
            Some(match key_path.and_then(file_name) {
                Some(name) if !name.contains('"') => {
                    format!("ssh -i \"{name}\" -o IdentitiesOnly=yes")
                }
                _ => "(custom ssh command)".into(),
            })
        }
        _ => Some(value.to_string()),
    }
}

pub fn masked_change(change: &ConfigChange) -> Value {
    json!({
        "key": change.key,
        "currentValue": masked_value(&change.key, &change.current_value),
        "nextValue": masked_value(&change.key, &change.next_value),
    })
}

pub fn changes(items: &[WorkingTreeChange]) -> Vec<Value> {
    items
        .iter()
        .map(|item| json!({ "path": item.path, "status": item.status }))
        .collect()
}

pub fn pull_request_summary(item: &PullRequestSummary) -> Value {
    json!({ "number": item.number, "url": item.url, "title": item.title })
}

pub fn pull_requests(items: &[ManagedPullRequest]) -> Vec<Value> {
    items.iter().map(|item| json!({
        "number": item.number, "url": item.url, "title": item.title, "state": item.state,
        "isDraft": item.is_draft, "baseBranch": item.base_branch, "headBranch": item.head_branch,
        "headOid": item.head_oid, "mergeable": item.mergeable, "mergeStateStatus": item.merge_state_status,
        "reviewDecision": item.review_decision, "author": item.author, "updatedAt": item.updated_at,
        "checks": item.checks.iter().map(|check| json!({ "name": check.name, "state": check.state,
            "bucket": check.bucket, "link": check.link, "workflow": check.workflow })).collect::<Vec<_>>()
    })).collect()
}

pub fn assignment(preview: &ApplyPreview) -> Value {
    json!({
        "repositoryId": preview.repository.id,
        "profile": ProfileDto::new(&preview.profile, None),
        "changes": preview.changes.iter().map(masked_change).collect::<Vec<_>>(),
        "warnings": preview.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_and_previews_hide_secret_paths() {
        let profile = Profile {
            id: "fictional".into(),
            label: "Fictional".into(),
            accent: "#112233".into(),
            git_name: "Sample Person".into(),
            git_email: "sample@example.com".into(),
            github_username: Some("fictional".into()),
            ssh_key_path: Some("C:\\Fictional\\.ssh\\id_example".into()),
            gh_config_dir: Some("C:\\Fictional\\gh-private".into()),
        };
        let profile_json = serde_json::to_string(&ProfileDto::new(&profile, None)).unwrap();
        assert!(profile_json.contains("id_example"));
        assert!(!profile_json.contains("C:\\\\Fictional"));
        let preview = ApplyPreview {
            repository: RepositoryRecord {
                id: "repo".into(),
                name: "repo".into(),
                path: "repo".into(),
                remote_url: None,
                branch: None,
                profile_id: None,
                last_applied_at: None,
                auto_approve: AutoApprove::default(),
            },
            profile,
            changes: vec![
                ConfigChange {
                    key: "gitcontext.ghConfigDir".into(),
                    current_value: Some("C:\\Fictional\\old-gh".into()),
                    next_value: Some("C:\\Fictional\\gh-private".into()),
                },
                ConfigChange {
                    key: "core.sshCommand".into(),
                    current_value: Some(
                        "ssh -i \"C:/Fictional/.ssh/old_key\" -o IdentitiesOnly=yes".into(),
                    ),
                    next_value: Some(
                        "ssh -i \"C:/Fictional/.ssh/id_example\" -o IdentitiesOnly=yes".into(),
                    ),
                },
            ],
            warnings: vec![],
        };
        let preview_json = assignment(&preview).to_string();
        assert!(!preview_json.contains("Fictional\\\\"));
        assert!(!preview_json.contains("C:/Fictional"));
        assert!(preview_json.contains("(gh config directory)"));
        assert!(preview_json.contains("old_key"));
    }

    #[test]
    fn null_and_custom_values_are_distinct() {
        assert_eq!(masked_value("gitcontext.ghConfigDir", &None), None);
        assert_eq!(
            masked_value("core.sshCommand", &Some("ssh -F private.conf".into())),
            Some("(custom ssh command)".into())
        );
    }
}

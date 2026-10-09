use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub accent: String,
    pub git_name: String,
    pub git_email: String,
    #[serde(default)]
    pub github_username: Option<String>,
    #[serde(default)]
    pub ssh_key_path: Option<String>,
    #[serde(default)]
    pub gh_config_dir: Option<String>,
    #[serde(default)]
    pub auto_approve: ProfileAutoApprove,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAutoApprove {
    #[serde(default)]
    pub clone_repository: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApprove {
    #[serde(default)]
    pub push_work_branch: bool,
    #[serde(default)]
    pub push_default_branch: bool,
    #[serde(default)]
    pub create_pull_request: bool,
    #[serde(default)]
    pub merge_pull_request: bool,
    #[serde(default)]
    pub publish_repository: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub last_applied_at: Option<String>,
    #[serde(default)]
    pub auto_approve: AutoApprove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub version: u32,
    pub profiles: Vec<Profile>,
    pub repositories: Vec<RepositoryRecord>,
    #[serde(default)]
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default, deserialize_with = "deserialize_locale")]
    pub locale: Option<String>,
    #[serde(default)]
    pub ai_integration_notice_dismissed: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default = "default_true")]
    pub gui_confirmation: bool,
    #[serde(default, deserialize_with = "deserialize_ai_notifications")]
    pub ai_notifications: AiNotifications,
    #[serde(default = "default_true")]
    pub status_notifications: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiNotifications {
    Off,
    #[default]
    Github,
    All,
}

fn deserialize_ai_notifications<'de, D>(deserializer: D) -> Result<AiNotifications, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value.as_str() {
        Some("off") => AiNotifications::Off,
        Some("all") => AiNotifications::All,
        _ => AiNotifications::Github,
    })
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: None,
            ai_integration_notice_dismissed: false,
            close_to_tray: true,
            gui_confirmation: true,
            ai_notifications: AiNotifications::Github,
            status_notifications: true,
        }
    }
}

#[cfg(test)]
mod close_to_tray_tests {
    use super::*;

    #[test]
    fn old_state_defaults_to_tray_and_saves_choice() {
        let old = r#"{"version":2,"profiles":[],"repositories":[],"settings":{"locale":"ja"}}"#;
        let mut data: AppData = serde_json::from_str(old).unwrap();
        assert!(data.settings.close_to_tray);
        assert!(data.settings.gui_confirmation);
        assert_eq!(data.settings.ai_notifications, AiNotifications::Github);
        assert!(data.settings.status_notifications);
        data.settings.close_to_tray = false;
        let saved = serde_json::to_string(&data).unwrap();
        assert!(saved.contains("\"closeToTray\":false"));
        let loaded: AppData = serde_json::from_str(&saved).unwrap();
        assert!(!loaded.settings.close_to_tray);
    }

    #[test]
    fn invalid_ai_notification_value_defaults_to_github() {
        for value in ["null", "1", "\"unknown\"", "[]"] {
            let json = format!(
                r#"{{"version":2,"profiles":[],"repositories":[],"settings":{{"aiNotifications":{value}}}}}"#
            );
            let data: AppData = serde_json::from_str(&json).unwrap();
            assert_eq!(data.settings.ai_notifications, AiNotifications::Github);
        }
    }

    #[test]
    fn rejects_invalid_close_to_tray() {
        for value in ["null", "1", "\"false\"", "[]"] {
            let json = format!(
                r#"{{"version":2,"profiles":[],"repositories":[],"settings":{{"closeToTray":{value}}}}}"#
            );
            assert!(serde_json::from_str::<AppData>(&json).is_err(), "{value}");
        }
    }
}

fn deserialize_locale<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_str()
        .filter(|locale| matches!(*locale, "ja" | "en"))
        .map(str::to_owned))
}

#[cfg(test)]
mod auto_approve_tests {
    use super::*;

    #[test]
    fn old_state_defaults_auto_approval_and_partial_settings() {
        let old =
            r#"{"version":2,"profiles":[],"repositories":[{"id":"r","name":"r","path":"r"}]}"#;
        let data: AppData = serde_json::from_str(old).unwrap();
        assert!(!data.repositories[0].auto_approve.push_work_branch);
        assert!(!data.repositories[0].auto_approve.create_pull_request);
        assert!(!data.repositories[0].auto_approve.push_default_branch);
        assert!(!data.repositories[0].auto_approve.merge_pull_request);
        assert!(!data.repositories[0].auto_approve.publish_repository);
        let partial = r#"{"version":2,"profiles":[],"repositories":[{"id":"r","name":"r","path":"r","autoApprove":{"pushWorkBranch":true}}]}"#;
        let data: AppData = serde_json::from_str(partial).unwrap();
        assert!(data.repositories[0].auto_approve.push_work_branch);
        assert!(!data.repositories[0].auto_approve.create_pull_request);
        assert!(!data.repositories[0].auto_approve.push_default_branch);
        assert!(!data.repositories[0].auto_approve.merge_pull_request);
        assert!(!data.repositories[0].auto_approve.publish_repository);
        let old_profile = r##"{"version":2,"profiles":[{"id":"p","label":"P","accent":"#112233","gitName":"Test","gitEmail":"test@example.com"}],"repositories":[]}"##;
        let profiles: AppData = serde_json::from_str(old_profile).unwrap();
        assert!(!profiles.profiles[0].auto_approve.clone_repository);
        let saved = serde_json::to_value(data).unwrap();
        assert_eq!(
            saved["repositories"][0]["autoApprove"]["pushWorkBranch"],
            true
        );
        assert_eq!(
            saved["repositories"][0]["autoApprove"]["createPullRequest"],
            false
        );
    }
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: 2,
            profiles: Vec::new(),
            repositories: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub available: bool,
    pub version: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatus {
    pub git: ToolStatus,
    pub gh: ToolStatus,
    pub ssh: ToolStatus,
    pub ssh_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResult {
    pub data: AppData,
    pub environment: EnvironmentStatus,
    pub storage_path: Option<String>,
    pub demo_mode: bool,
    pub development_data: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhProfileStatus {
    pub available: bool,
    pub authenticated: bool,
    pub username: Option<String>,
    pub detail: Option<String>,
    pub config_dir: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryState {
    Ready,
    Reapply,
    Unassigned,
    Attention,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryStatus {
    pub repository_id: String,
    pub state: RepositoryState,
    pub branch: Option<String>,
    pub uncommitted_changes: Option<usize>,
    pub ahead: Option<u64>,
    pub identity_in_sync: bool,
    pub mismatched_keys: Vec<String>,
    pub github: Option<GhProfileStatus>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChange {
    pub key: String,
    pub current_value: Option<String>,
    pub next_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPreview {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub changes: Vec<ConfigChange>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub data: AppData,
    pub repository_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepository {
    pub name: String,
    pub name_with_owner: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub ssh_url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneResult {
    pub data: AppData,
    pub repository: RepositoryRecord,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPreview {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub branch: String,
    pub remote_url: String,
    pub upstream: Option<String>,
    pub has_uncommitted_changes: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub branch: String,
    pub remote_url: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub branch: String,
    pub remote_url: String,
    pub upstream: Option<String>,
    pub remote_branch: Option<String>,
    pub changes: Vec<WorkingTreeChange>,
    pub ahead: u64,
    pub behind: u64,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingTreeChange {
    pub status: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitPreview {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub branch: String,
    pub changes: Vec<WorkingTreeChange>,
    pub push_remote_url: Option<String>,
    pub push_unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub branch: String,
    pub commit_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchResult {
    pub data: AppData,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSummary {
    pub number: u64,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestPreview {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub current_branch: String,
    pub base_branch: String,
    pub remote_url: String,
    pub repository_name_with_owner: String,
    pub changes: Vec<WorkingTreeChange>,
    pub commits_ahead: u64,
    pub branch_pushed: bool,
    pub requires_new_branch: bool,
    pub existing_pull_request: Option<PullRequestSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestResult {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub branch: String,
    pub base_branch: String,
    pub existing: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCheck {
    pub name: String,
    pub state: String,
    pub bucket: String,
    pub link: Option<String>,
    pub workflow: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedPullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub base_branch: String,
    pub head_branch: String,
    pub head_oid: String,
    pub mergeable: String,
    pub merge_state_status: String,
    pub review_decision: String,
    pub author: Option<String>,
    pub updated_at: String,
    pub checks: Vec<PullRequestCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestManagement {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub repository_name_with_owner: String,
    pub pull_requests: Vec<ManagedPullRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePullRequestResult {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub strategy: String,
    pub merged_at: Option<String>,
}

pub fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

pub fn normalize_profile(mut profile: Profile) -> Profile {
    profile.label = profile.label.trim().to_string();
    profile.git_name = profile.git_name.trim().to_string();
    profile.git_email = profile.git_email.trim().to_string();
    profile.github_username = clean_optional(profile.github_username);
    profile.ssh_key_path = clean_optional(profile.ssh_key_path);
    profile.gh_config_dir = clean_optional(profile.gh_config_dir);
    profile
}

pub fn validate_profile(profile: &Profile) -> Result<(), String> {
    validate_profile_id(&profile.id)?;
    validate_text("Profile name", &profile.label, true, 80)?;
    validate_text("Git author name", &profile.git_name, true, 160)?;
    validate_text("Git author email", &profile.git_email, true, 254)?;
    if !profile.git_email.contains('@') {
        return Err("Git author email must look like an email address.".into());
    }
    if !is_hex_color(&profile.accent) {
        return Err("Profile color must use the #RRGGBB format.".into());
    }
    for (label, value) in [
        ("GitHub username", profile.github_username.as_deref()),
        ("SSH key path", profile.ssh_key_path.as_deref()),
        ("gh config directory", profile.gh_config_dir.as_deref()),
    ] {
        if let Some(value) = value {
            validate_text(label, value, false, 1024)?;
        }
    }
    Ok(())
}

pub fn validate_profile_id(id: &str) -> Result<(), String> {
    validate_text("Profile ID", id, true, 128)?;
    if !id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("Profile ID contains unsupported characters.".into());
    }
    Ok(())
}

pub fn migrate_app_data(data: &mut AppData) -> bool {
    if data.version >= 2 {
        return false;
    }

    let assigned_profile_ids = data
        .repositories
        .iter()
        .filter_map(|repository| repository.profile_id.clone())
        .collect::<Vec<_>>();
    data.profiles.retain(|profile| {
        let is_seed = matches!(profile.id.as_str(), "personal" | "school")
            && matches!(profile.label.as_str(), "Personal" | "School")
            && profile.git_name.trim().is_empty()
            && profile.git_email.trim().is_empty()
            && profile
                .github_username
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && profile
                .ssh_key_path
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && profile
                .gh_config_dir
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty();
        let is_assigned = assigned_profile_ids.iter().any(|id| id == &profile.id);
        !is_seed || is_assigned
    });
    data.version = 2;
    true
}

fn validate_text(label: &str, value: &str, required: bool, max_len: usize) -> Result<(), String> {
    if required && value.trim().is_empty() {
        return Err(format!("{label} is required."));
    }
    if value.len() > max_len {
        return Err(format!("{label} is too long."));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label} cannot contain control characters."));
    }
    Ok(())
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> Profile {
        Profile {
            id: "personal".into(),
            label: " Personal ".into(),
            accent: "#d8a33f".into(),
            git_name: "Your Name".into(),
            git_email: "you@example.com".into(),
            github_username: Some("  user  ".into()),
            ssh_key_path: Some(String::new()),
            gh_config_dir: None,
            auto_approve: ProfileAutoApprove::default(),
        }
    }

    #[test]
    fn normalization_trims_values_and_removes_empty_options() {
        let profile = normalize_profile(valid_profile());
        assert_eq!(profile.label, "Personal");
        assert_eq!(profile.github_username.as_deref(), Some("user"));
        assert_eq!(profile.ssh_key_path, None);
    }

    #[test]
    fn profile_rejects_control_characters() {
        let mut profile = valid_profile();
        profile.git_name = "Unsafe\nName".into();
        assert!(validate_profile(&profile).is_err());
    }

    #[test]
    fn migration_removes_only_unused_seed_profiles() {
        let mut data = AppData {
            version: 1,
            profiles: vec![
                Profile {
                    id: "personal".into(),
                    label: "Personal".into(),
                    accent: "#d8a33f".into(),
                    git_name: String::new(),
                    git_email: String::new(),
                    github_username: None,
                    ssh_key_path: None,
                    gh_config_dir: None,
                    auto_approve: ProfileAutoApprove::default(),
                },
                valid_profile(),
            ],
            repositories: Vec::new(),
            settings: AppSettings::default(),
        };

        assert!(migrate_app_data(&mut data));
        assert_eq!(data.version, 2);
        assert_eq!(data.profiles.len(), 1);
        assert_eq!(data.profiles[0].git_name, "Your Name");
    }
}

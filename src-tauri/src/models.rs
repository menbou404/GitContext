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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub version: u32,
    pub profiles: Vec<Profile>,
    pub repositories: Vec<RepositoryRecord>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: 2,
            profiles: Vec::new(),
            repositories: Vec::new(),
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChange {
    pub key: String,
    pub current_value: Option<String>,
    pub next_value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPreview {
    pub repository: RepositoryRecord,
    pub profile: Profile,
    pub changes: Vec<ConfigChange>,
    pub warnings: Vec<String>,
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
                },
                valid_profile(),
            ],
            repositories: Vec::new(),
        };

        assert!(migrate_app_data(&mut data));
        assert_eq!(data.version, 2);
        assert_eq!(data.profiles.len(), 1);
        assert_eq!(data.profiles[0].git_name, "Your Name");
    }
}

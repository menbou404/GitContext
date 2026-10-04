use gitcontext_core::{
    ai_clients::{
        self, Action as AiAction, Client as AiClient, Inventory as AiInventory, Plan as AiPlan,
        Tier as AiTier,
    },
    audit::{self, AuditEntry},
    github::{CreatePullRequestInput, GithubAuthPrompt, MergePullRequestInput},
    models::*,
    operations, repository_status,
    storage::{BackupEntry, StateStore},
};
use tauri::{AppHandle, Emitter, State};
const GITHUB_AUTH_PROMPT_EVENT: &str = "github-auth-prompt";

#[tauri::command]
pub fn list_ai_clients() -> Result<AiInventory, String> {
    ai_clients::list(&ai_clients::default_paths()?)
}

#[tauri::command]
pub fn plan_ai_client(
    client: AiClient,
    action: AiAction,
    tier: AiTier,
    trust: bool,
) -> Result<AiPlan, String> {
    ai_clients::plan(&ai_clients::default_paths()?, client, action, tier, trust)
}

#[tauri::command]
pub async fn apply_ai_client(
    store: State<'_, StateStore>,
    plan: AiPlan,
) -> Result<Option<std::path::PathBuf>, String> {
    let state_dir = store.config_dir().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        ai_clients::apply(&ai_clients::default_paths()?, &state_dir, &plan)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn verify_ai_client(client: AiClient) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ai_clients::verify(&ai_clients::default_paths()?, client)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn bootstrap(store: State<'_, StateStore>) -> Result<BootstrapResult, String> {
    operations::bootstrap(&store)
}

#[tauri::command]
pub fn set_locale(store: State<'_, StateStore>, locale: String) -> Result<AppSettings, String> {
    operations::set_locale(&store, locale)
}

#[tauri::command]
pub fn refresh_environment() -> EnvironmentStatus {
    gitcontext_core::environment::environment_status()
}

#[tauri::command]
pub fn list_backups(store: State<'_, StateStore>) -> Result<Vec<BackupEntry>, String> {
    store.list_backups()
}

#[tauri::command]
pub fn restore_backup(store: State<'_, StateStore>, file_name: String) -> Result<AppData, String> {
    store.restore_backup(&file_name)
}

#[tauri::command]
pub fn open_data_folder(store: State<'_, StateStore>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(store.config_dir())
            .spawn()
            .map_err(|error| format!("Could not open the data folder: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = store;
        Err("Opening the data folder is not supported on this platform.".into())
    }
}

#[tauri::command]
pub async fn inspect_repository_statuses(
    store: State<'_, StateStore>,
) -> Result<Vec<RepositoryStatus>, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let data = {
            let _guard = store.lock()?;
            store.load()?
        };
        Ok(repository_status::inspect_repository_statuses(
            &store, &data,
        ))
    })
    .await
    .map_err(|error| format!("Repository status task failed: {error}"))?
}

#[tauri::command]
pub fn save_profile(store: State<'_, StateStore>, profile: Profile) -> Result<AppData, String> {
    operations::save_profile(&store, profile)
}

#[tauri::command]
pub fn inspect_github_profile(
    store: State<'_, StateStore>,
    profile_id: String,
    gh_config_dir: Option<String>,
) -> Result<GhProfileStatus, String> {
    operations::inspect_github_profile(&store, profile_id, gh_config_dir)
}

#[tauri::command]
pub async fn connect_github_profile(
    app: AppHandle,
    store: State<'_, StateStore>,
    profile_id: String,
    gh_config_dir: Option<String>,
) -> Result<GhProfileStatus, String> {
    let store = (*store).clone();
    let on_prompt = std::sync::Arc::new(move |prompt: GithubAuthPrompt| {
        let _ = app.emit(GITHUB_AUTH_PROMPT_EVENT, prompt);
    });
    tauri::async_runtime::spawn_blocking(move || {
        operations::connect_github_profile(&store, on_prompt, profile_id, gh_config_dir)
    })
    .await
    .map_err(|error| format!("GitHub connection task failed: {error}"))?
}

#[tauri::command]
pub fn open_github_auth_page() -> Result<(), String> {
    operations::open_github_auth_page()
}

#[tauri::command]
pub fn add_repository(
    store: State<'_, StateStore>,
    path: String,
) -> Result<RepositoryRecord, String> {
    operations::add_repository(&store, path)
}

#[tauri::command]
pub fn set_repository_auto_approve(
    store: State<'_, StateStore>,
    repository_id: String,
    auto_approve: AutoApprove,
) -> Result<AppData, String> {
    operations::set_repository_auto_approve(&store, repository_id, auto_approve)
}

#[tauri::command]
pub fn set_profile_auto_approve(
    store: State<'_, StateStore>,
    profile_id: String,
    auto_approve: ProfileAutoApprove,
) -> Result<AppData, String> {
    operations::set_profile_auto_approve(&store, profile_id, auto_approve)
}

#[tauri::command]
pub async fn list_github_repositories(
    store: State<'_, StateStore>,
    profile_id: String,
) -> Result<Vec<GithubRepository>, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::list_github_repositories(&store, profile_id)
    })
    .await
    .map_err(|error| format!("GitHub repository listing task failed: {error}"))?
}

#[tauri::command]
pub async fn clone_repository(
    store: State<'_, StateStore>,
    profile_id: String,
    repository_url: String,
    destination_parent: String,
) -> Result<CloneResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::clone_repository(&store, profile_id, repository_url, destination_parent)
    })
    .await
    .map_err(|error| format!("Git clone task failed: {error}"))?
}

#[tauri::command]
pub fn remove_repository(store: State<'_, StateStore>, id: String) -> Result<AppData, String> {
    operations::remove_repository(&store, id)
}

#[tauri::command]
pub fn preview_assignment(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<ApplyPreview, String> {
    operations::preview_assignment(&store, repository_id, profile_id)
}

#[tauri::command]
pub fn apply_profile(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<AppData, String> {
    operations::apply_profile_from_gui(&store, repository_id, profile_id)
}

#[tauri::command]
pub fn list_history(store: State<'_, StateStore>) -> Result<Vec<AuditEntry>, String> {
    audit::read_recent(&store)
}

#[tauri::command]
pub fn preview_push(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<PushPreview, String> {
    operations::preview_push(&store, repository_id, profile_id)
}

#[tauri::command]
pub async fn push_repository(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<PushResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::push_repository(&store, repository_id, profile_id)
    })
    .await
    .map_err(|error| format!("Git push task failed: {error}"))?
}

#[tauri::command]
pub async fn preview_repository_sync(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<SyncPreview, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::preview_repository_sync(&store, repository_id, profile_id)
    })
    .await
    .map_err(|error| format!("Git fetch task failed: {error}"))?
}

#[tauri::command]
pub async fn pull_repository(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<SyncPreview, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::pull_repository(&store, repository_id, profile_id)
    })
    .await
    .map_err(|error| format!("Git pull task failed: {error}"))?
}

#[tauri::command]
pub fn preview_commit(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<CommitPreview, String> {
    operations::preview_commit(&store, repository_id, profile_id)
}

#[tauri::command]
pub async fn commit_repository(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
    message: String,
) -> Result<CommitResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::commit_repository(&store, repository_id, profile_id, message)
    })
    .await
    .map_err(|error| format!("Git commit task failed: {error}"))?
}

#[tauri::command]
pub async fn preview_pull_request(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<PullRequestPreview, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::preview_pull_request(&store, repository_id, profile_id)
    })
    .await
    .map_err(|error| format!("Pull request preview task failed: {error}"))?
}

#[tauri::command]
pub async fn create_branch(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
    branch_name: String,
) -> Result<BranchResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::create_branch(&store, repository_id, profile_id, branch_name)
    })
    .await
    .map_err(|error| format!("Branch creation task failed: {error}"))?
}

#[tauri::command]
pub async fn create_pull_request(
    store: State<'_, StateStore>,
    input: CreatePullRequestInput,
) -> Result<PullRequestResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || operations::create_pull_request(&store, input))
        .await
        .map_err(|error| format!("Pull request creation task failed: {error}"))?
}

#[tauri::command]
pub async fn list_pull_requests(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
) -> Result<PullRequestManagement, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::list_pull_requests(&store, repository_id, profile_id)
    })
    .await
    .map_err(|error| format!("Pull request listing task failed: {error}"))?
}

#[tauri::command]
pub async fn merge_pull_request(
    store: State<'_, StateStore>,
    input: MergePullRequestInput,
) -> Result<MergePullRequestResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || operations::merge_pull_request(&store, input))
        .await
        .map_err(|error| format!("Pull request merge task failed: {error}"))?
}

#[tauri::command]
pub async fn publish_repository(
    store: State<'_, StateStore>,
    repository_id: String,
    profile_id: String,
    name: String,
    visibility: String,
    description: Option<String>,
) -> Result<PublishResult, String> {
    let store = (*store).clone();
    tauri::async_runtime::spawn_blocking(move || {
        operations::publish_repository(
            &store,
            repository_id,
            profile_id,
            name,
            visibility,
            description,
        )
    })
    .await
    .map_err(|error| format!("GitHub publish task failed: {error}"))?
}

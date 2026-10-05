mod commands;

use gitcontext_core::storage::open_default_store;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(open_default_store()?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_ai_clients,
            commands::plan_ai_client,
            commands::apply_ai_client,
            commands::verify_ai_client,
            commands::bootstrap,
            commands::set_locale,
            commands::refresh_environment,
            commands::list_backups,
            commands::restore_backup,
            commands::open_data_folder,
            commands::inspect_repository_statuses,
            commands::save_profile,
            commands::inspect_github_profile,
            commands::connect_github_profile,
            commands::open_github_auth_page,
            commands::add_repository,
            commands::set_repository_auto_approve,
            commands::set_profile_auto_approve,
            commands::remove_repository,
            commands::preview_assignment,
            commands::apply_profile,
            commands::list_history,
            commands::publish_repository,
            commands::list_github_repositories,
            commands::clone_repository,
            commands::preview_push,
            commands::push_repository,
            commands::preview_repository_sync,
            commands::pull_repository,
            commands::preview_commit,
            commands::commit_repository,
            commands::preview_pull_request,
            commands::create_branch,
            commands::create_pull_request,
            commands::list_pull_requests,
            commands::merge_pull_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitContext");
}

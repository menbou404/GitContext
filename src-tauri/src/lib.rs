mod commands;

use gitcontext_core::storage::StateStore;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir().map_err(|error| {
                format!("Could not resolve GitContext's settings directory: {error}")
            })?;
            app.manage(StateStore::new(config_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_profile,
            commands::inspect_github_profile,
            commands::connect_github_profile,
            commands::open_github_auth_page,
            commands::add_repository,
            commands::remove_repository,
            commands::preview_assignment,
            commands::apply_profile,
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

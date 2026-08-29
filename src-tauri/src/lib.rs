mod commands;
mod git_ops;
mod models;
mod storage;

use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppGate(Mutex::new(())))
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitContext");
}

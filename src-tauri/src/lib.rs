mod approval;
mod background;
mod commands;

use gitcontext_core::storage::open_default_store;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

const TRAY_ID: &str = "gitcontext-tray";
const OPEN_ID: &str = "open";
const ATTENTION_ID: &str = "attention";
const QUIT_ID: &str = "quit";

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn tray_menu(app: &tauri::AppHandle, locale: Option<&str>) -> tauri::Result<Menu<tauri::Wry>> {
    let (open_label, quit_label) = if locale == Some("ja") {
        ("GitContextを開く", "終了")
    } else {
        ("Open GitContext", "Quit")
    };
    let open = MenuItem::with_id(app, OPEN_ID, open_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, quit_label, true, None::<&str>)?;
    let count = app
        .try_state::<background::AttentionState>()
        .and_then(|state| state.0.lock().ok().and_then(|value| *value))
        .unwrap_or(0);
    if count > 0 {
        let label = if locale == Some("ja") {
            format!("要対応のリポジトリ（{count}）")
        } else {
            format!("Repositories needing attention ({count})")
        };
        let attention = MenuItem::with_id(app, ATTENTION_ID, label, true, None::<&str>)?;
        Menu::with_items(app, &[&open, &attention, &quit])
    } else {
        Menu::with_items(app, &[&open, &quit])
    }
}

fn tray_tooltip(locale: Option<&str>, count: usize) -> String {
    let base = if cfg!(debug_assertions) {
        if locale == Some("ja") {
            "GitContext（開発版）"
        } else {
            "GitContext (development)"
        }
    } else {
        "GitContext"
    };
    if count == 0 {
        return base.into();
    }
    if locale == Some("ja") {
        format!("{base} — 要対応 {count}件")
    } else {
        format!("{base} — {count} need attention")
    }
}

pub(crate) fn update_tray_menu(app: &tauri::AppHandle, locale: Option<&str>) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let count = app
            .state::<background::AttentionState>()
            .0
            .lock()
            .map_err(|error| error.to_string())?
            .unwrap_or(0);
        tray.set_menu(Some(
            tray_menu(app, locale).map_err(|error| error.to_string())?,
        ))
        .map_err(|error| error.to_string())?;
        tray.set_tooltip(Some(tray_tooltip(locale, count)))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let store = open_default_store()?;
            let settings = {
                let _guard = store.lock()?;
                store.load()?.settings
            };
            app.manage(AtomicBool::new(settings.close_to_tray));
            app.manage(background::AttentionState::default());
            app.manage(approval::ApprovalState::new(settings.gui_confirmation));
            if settings.gui_confirmation {
                let _ = approval::start(app.handle());
            }
            let locale = settings.locale.as_deref();
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or("App icon is missing")?;
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(icon)
                .tooltip(tray_tooltip(locale, 0))
                .menu(&tray_menu(app.handle(), locale)?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    OPEN_ID => show_main_window(app),
                    ATTENTION_ID => {
                        show_main_window(app);
                        if let Err(error) = app.emit("open-attention-repositories", ()) {
                            eprintln!("Could not open attention filter: {error}");
                        }
                    }
                    QUIT_ID => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            app.manage(store);
            if !std::env::args_os().any(|arg| arg == "--hidden") {
                show_main_window(app.handle());
            }
            background::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if window.state::<AtomicBool>().load(Ordering::Relaxed) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
            if window.label() == "approval" {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    window.state::<approval::ApprovalState>().decline();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_ai_clients,
            commands::plan_ai_client,
            commands::apply_ai_client,
            commands::verify_ai_client,
            commands::bootstrap,
            commands::set_locale,
            commands::set_close_to_tray,
            commands::set_gui_confirmation,
            commands::set_ai_notifications,
            commands::set_status_notifications,
            commands::report_repository_statuses,
            commands::gui_confirmation_status,
            commands::current_approval,
            commands::approval_locale,
            commands::answer_approval,
            commands::is_autostart_enabled,
            commands::set_autostart_enabled,
            commands::dismiss_ai_integration_notice,
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

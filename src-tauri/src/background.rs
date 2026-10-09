use gitcontext_core::{
    audit::{AuditEntry, AuditTail},
    background::{needs_attention_count, newly_needs_attention, should_notify_ai},
    models::{AppData, RepositoryStatus},
    repository_status,
    storage::StateStore,
};
use std::{sync::Mutex, thread, time::Duration};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

#[derive(Default)]
pub struct AttentionState(pub Mutex<Option<usize>>);

fn load_data(store: &StateStore) -> Result<AppData, String> {
    let _guard = store.lock()?;
    store.load()
}

pub fn report_statuses(app: &AppHandle, statuses: &[RepositoryStatus]) {
    report_attention_count(app, needs_attention_count(statuses));
}

pub fn report_attention_count(app: &AppHandle, count: usize) {
    let state = app.state::<AttentionState>();
    let mut previous = match state.0.lock() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Attention state lock failed: {error}");
            return;
        }
    };
    let notify = newly_needs_attention(*previous, count);
    *previous = Some(count);
    drop(previous);

    let store = app.state::<StateStore>();
    let settings = match load_data(&store) {
        Ok(data) => data.settings,
        Err(error) => {
            eprintln!("Could not load notification settings: {error}");
            return;
        }
    };
    if let Err(error) = crate::update_tray_menu(app, settings.locale.as_deref()) {
        eprintln!("Could not update tray: {error}");
    }
    if notify && settings.status_notifications && cfg!(target_os = "windows") {
        let body = if settings.locale.as_deref() == Some("ja") {
            format!("要対応のリポジトリが{count}件あります")
        } else {
            format!("{count} repositories need attention")
        };
        if let Err(error) = app
            .notification()
            .builder()
            .title("GitContext")
            .body(body)
            .show()
        {
            eprintln!("Could not send status notification: {error}");
        }
    }
}

fn tool_name(tool: &str, ja: bool) -> &str {
    match (tool, ja) {
        ("push", _) => "push",
        ("create_pull_request", true) => "プルリクエスト作成",
        ("create_pull_request", false) => "create a pull request",
        ("merge_pull_request", _) => "merge",
        ("clone_repository", _) => "clone",
        ("publish_repository", true) => "リポジトリ公開",
        ("publish_repository", false) => "publish a repository",
        ("commit", _) => "commit",
        ("create_branch", true) => "ブランチ作成",
        ("create_branch", false) => "create a branch",
        ("pull", _) => "pull",
        ("apply_profile", true) => "プロファイル適用",
        ("apply_profile", false) => "apply a profile",
        _ => "operation",
    }
}

fn notification_body(entry: &AuditEntry, data: &AppData) -> String {
    let ja = data.settings.locale.as_deref() == Some("ja");
    let repository = entry
        .repository_id
        .as_deref()
        .and_then(|id| data.repositories.iter().find(|item| item.id == id))
        .map(|item| item.name.as_str())
        .unwrap_or(if ja {
            "削除済みのリポジトリ"
        } else {
            "Deleted repository"
        });
    let tool = tool_name(&entry.tool, ja);
    let result = match (entry.outcome.as_str(), ja) {
        ("success", true) => "しました",
        ("rejected", true) => "を拒否しました",
        (_, true) => "が失敗しました",
        ("success", false) => "completed",
        ("rejected", false) => "was rejected",
        (_, false) => "failed",
    };
    if ja {
        if entry.outcome == "success" {
            format!("AIがGitContextで{tool}しました（{repository}）")
        } else {
            format!("AIの{tool}{result}（{repository}）")
        }
    } else {
        format!("AI {tool} {result} in GitContext ({repository})")
    }
}

pub fn start(app: &AppHandle) {
    let app_for_audit = app.clone();
    thread::spawn(move || {
        let store = app_for_audit.state::<StateStore>().inner().clone();
        let mut tail = match AuditTail::at_end(store.config_dir()) {
            Ok(tail) => tail,
            Err(error) => {
                eprintln!("Could not start audit watcher: {error}");
                return;
            }
        };
        loop {
            thread::sleep(Duration::from_secs(3));
            let entries = match store
                .lock()
                .and_then(|_guard| tail.read_new(store.config_dir()))
            {
                Ok(entries) => entries,
                Err(error) => {
                    eprintln!("Could not read audit updates: {error}");
                    continue;
                }
            };
            if entries.is_empty() {
                continue;
            }
            let data = match load_data(&store) {
                Ok(data) => data,
                Err(error) => {
                    eprintln!("Could not load AI notification settings: {error}");
                    continue;
                }
            };
            for entry in entries
                .iter()
                .filter(|entry| should_notify_ai(data.settings.ai_notifications, entry))
            {
                if let Err(error) = app_for_audit
                    .notification()
                    .builder()
                    .title("GitContext")
                    .body(notification_body(entry, &data))
                    .show()
                {
                    eprintln!("Could not send AI notification: {error}");
                }
            }
        }
    });

    let app_for_status = app.clone();
    thread::spawn(move || loop {
        let visible = app_for_status
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(true);
        if !visible {
            let store = app_for_status.state::<StateStore>().inner().clone();
            match load_data(&store) {
                Ok(data) => {
                    let statuses = repository_status::inspect_repository_statuses(&store, &data);
                    report_statuses(&app_for_status, &statuses);
                }
                Err(error) => eprintln!("Could not inspect background status: {error}"),
            }
        }
        thread::sleep(Duration::from_secs(15 * 60));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_uses_saved_language_and_does_not_include_summary() {
        let mut data = AppData::default();
        data.settings.locale = Some("ja".into());
        let entry = AuditEntry {
            at: "now".into(),
            tool: "push".into(),
            repository_id: Some("removed".into()),
            profile_id: None,
            outcome: "rejected".into(),
            summary: "secret@example.com".into(),
            client: None,
            confirmation: None,
            actor: None,
        };
        let body = notification_body(&entry, &data);
        assert!(body.contains("削除済みのリポジトリ"));
        assert!(body.contains("拒否"));
        assert!(!body.contains("secret@example.com"));
        data.settings.locale = Some("en".into());
        assert!(notification_body(&entry, &data).contains("Deleted repository"));
    }
}

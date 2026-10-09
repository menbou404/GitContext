use crate::{
    audit::AuditEntry,
    models::{AiNotifications, RepositoryState, RepositoryStatus},
};

pub fn should_notify_ai(setting: AiNotifications, entry: &AuditEntry) -> bool {
    if setting == AiNotifications::Off || entry.actor.as_deref() == Some("gui") {
        return false;
    }
    setting == AiNotifications::All
        || matches!(
            entry.tool.as_str(),
            "push"
                | "create_pull_request"
                | "merge_pull_request"
                | "clone_repository"
                | "publish_repository"
        )
}

pub fn needs_attention_count(statuses: &[RepositoryStatus]) -> usize {
    statuses
        .iter()
        .filter(|item| {
            matches!(
                item.state,
                RepositoryState::Reapply | RepositoryState::Attention
            )
        })
        .count()
}

pub fn newly_needs_attention(previous: Option<usize>, next: usize) -> bool {
    previous == Some(0) && next > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tool: &str, actor: Option<&str>) -> AuditEntry {
        AuditEntry {
            at: "now".into(),
            tool: tool.into(),
            repository_id: None,
            profile_id: None,
            outcome: "success".into(),
            summary: String::new(),
            client: None,
            confirmation: None,
            actor: actor.map(str::to_owned),
        }
    }

    #[test]
    fn ai_notification_modes_and_gui_exclusion() {
        assert!(!should_notify_ai(
            AiNotifications::Off,
            &entry("push", None)
        ));
        assert!(should_notify_ai(
            AiNotifications::Github,
            &entry("push", None)
        ));
        assert!(!should_notify_ai(
            AiNotifications::Github,
            &entry("commit", None)
        ));
        assert!(should_notify_ai(
            AiNotifications::All,
            &entry("commit", None)
        ));
        assert!(!should_notify_ai(
            AiNotifications::All,
            &entry("push", Some("gui"))
        ));
    }

    #[test]
    fn attention_count_and_transition() {
        let status = |state| RepositoryStatus {
            repository_id: String::new(),
            state,
            branch: None,
            uncommitted_changes: None,
            ahead: None,
            identity_in_sync: false,
            mismatched_keys: Vec::new(),
            github: None,
            error: None,
        };
        assert_eq!(
            needs_attention_count(&[
                status(RepositoryState::Ready),
                status(RepositoryState::Reapply),
                status(RepositoryState::Attention),
                status(RepositoryState::Unassigned)
            ]),
            2
        );
        assert!(!newly_needs_attention(None, 1));
        assert!(newly_needs_attention(Some(0), 1));
        assert!(!newly_needs_attention(Some(1), 2));
        assert!(!newly_needs_attention(Some(1), 0));
    }
}

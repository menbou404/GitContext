use chrono::{DateTime, Duration, Utc};
use gitcontext_core::{git_ops::ExactChange, models::ConfigChange};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration as StdDuration, Instant},
};
use uuid::Uuid;

pub const CHANGED: &str = "The repository changed after the preview. Run the preview again.";
const INVALID: &str = "Preview ID is invalid, expired, or already used. Run the preview again.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Apply,
    Commit,
    Pull,
    CreateBranch,
    Push,
    CreatePullRequest,
    Merge,
    Clone,
    Publish,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fingerprint {
    Apply {
        head: String,
        changes: Vec<(String, Option<String>, Option<String>)>,
    },
    Commit {
        branch: String,
        head: String,
        changes: Vec<ExactChange>,
    },
    Pull {
        branch: String,
        head: String,
        upstream: Option<String>,
        remote_head: String,
        clean: bool,
    },
    CreateBranch {
        branch: String,
        head: String,
        base_branch: String,
    },
    Push {
        branch: String,
        head: String,
        origin: String,
        upstream: Option<String>,
        dirty: bool,
    },
    CreatePullRequest {
        branch: String,
        head: String,
        base: String,
        pushed: bool,
        ahead: u64,
        clean: bool,
        existing: Option<u64>,
    },
    Merge {
        number: u64,
        head_oid: String,
        merge_state: String,
    },
    Clone {
        url: String,
        parent: String,
        absent: bool,
    },
    Publish {
        branch: String,
        head: String,
        name: String,
        description: Option<String>,
        visibility: String,
        username: String,
    },
}

impl Fingerprint {
    pub fn assignment(head: String, changes: &[ConfigChange]) -> Self {
        let mut changes: Vec<_> = changes
            .iter()
            .map(|c| (c.key.clone(), c.current_value.clone(), c.next_value.clone()))
            .collect();
        changes.sort();
        Self::Apply { head, changes }
    }

    pub fn commit(branch: String, head: String, mut changes: Vec<ExactChange>) -> Self {
        changes.sort_by(|a, b| {
            (&a.path, &a.original_path, &a.status).cmp(&(&b.path, &b.original_path, &b.status))
        });
        Self::Commit {
            branch,
            head,
            changes,
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    pub operation: Operation,
    pub repository_id: String,
    pub profile_id: String,
    pub fingerprint: Fingerprint,
    pub created_at: DateTime<Utc>,
    pub used: bool,
}

#[derive(Default)]
pub struct Previews(Mutex<HashMap<String, Entry>>);

pub struct PendingConfirmation {
    pub entry: Entry,
    pub preview_id: String,
    pub tool: &'static str,
    pub prompt: String,
    created_at: Instant,
}

#[derive(Default)]
pub struct PendingConfirmations(Mutex<HashMap<String, PendingConfirmation>>);

impl PendingConfirmations {
    pub fn issue(
        &self,
        entry: Entry,
        preview_id: String,
        tool: &'static str,
        prompt: String,
        timeout: StdDuration,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Instant::now();
        let mut entries = self.0.lock().unwrap();
        entries.retain(|_, pending| now.duration_since(pending.created_at) < timeout);
        entries.insert(
            id.clone(),
            PendingConfirmation {
                entry,
                preview_id,
                tool,
                prompt,
                created_at: now,
            },
        );
        id
    }

    pub fn take(
        &self,
        id: &str,
        tool: &str,
        timeout: StdDuration,
    ) -> Result<PendingConfirmation, String> {
        // Remove before checking anything: even a malformed retry spends this approval.
        let pending = self
            .0
            .lock()
            .unwrap()
            .remove(id)
            .ok_or("Confirmation is invalid, expired, or already used.")?;
        if pending.created_at.elapsed() >= timeout {
            return Err("Confirmation timed out.".into());
        }
        if pending.tool != tool {
            return Err("Confirmation is for a different tool.".into());
        }
        Ok(pending)
    }
}

impl Previews {
    pub fn issue(
        &self,
        operation: Operation,
        repository_id: String,
        profile_id: String,
        fingerprint: Fingerprint,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let mut entries = self.0.lock().unwrap();
        // Drop used and expired IDs so a long-running server does not grow without bound.
        entries.retain(|_, entry| {
            !entry.used && now.signed_duration_since(entry.created_at) < Duration::minutes(10)
        });
        entries.insert(
            id.clone(),
            Entry {
                operation,
                repository_id,
                profile_id,
                fingerprint,
                created_at: now,
                used: false,
            },
        );
        id
    }

    pub fn consume(
        &self,
        id: &str,
        operation: Operation,
        now: DateTime<Utc>,
    ) -> Result<Entry, String> {
        let mut entries = self.0.lock().unwrap();
        let entry = entries.get_mut(id).ok_or(INVALID)?;
        if entry.used {
            return Err(INVALID.into());
        }
        entry.used = true;
        if now.signed_duration_since(entry.created_at) >= Duration::minutes(10)
            || now < entry.created_at
            || entry.operation != operation
        {
            return Err(INVALID.into());
        }
        Ok(entry.clone())
    }

    #[cfg(test)]
    pub fn consume_for_repository(
        &self,
        id: &str,
        operation: Operation,
        repository_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Entry, String> {
        let entry = self.consume(id, operation, now)?;
        if entry.repository_id != repository_id {
            return Err(INVALID.into());
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issued() -> (Previews, String) {
        let previews = Previews::default();
        let id = previews.issue(
            Operation::Commit,
            "repo".into(),
            "profile".into(),
            Fingerprint::commit("main".into(), "head".into(), vec![]),
        );
        (previews, id)
    }

    #[test]
    fn rejects_unknown_used_expired_wrong_operation_and_repository() {
        let (previews, id) = issued();
        let now = Utc::now();
        assert!(previews.consume("unknown", Operation::Commit, now).is_err());
        assert!(previews
            .consume_for_repository(&id, Operation::Commit, "other", now)
            .is_err());
        assert!(previews.consume(&id, Operation::Commit, now).is_err());
        let (previews, id) = issued();
        assert!(previews.consume(&id, Operation::Apply, now).is_err());
        assert!(previews.consume(&id, Operation::Commit, now).is_err());
        let (previews, id) = issued();
        assert!(previews
            .consume(&id, Operation::Commit, now + Duration::minutes(11))
            .is_err());
        assert!(previews.consume(&id, Operation::Commit, now).is_err());
        let (previews, id) = issued();
        assert!(previews.consume(&id, Operation::Commit, Utc::now()).is_ok());
        assert!(previews.consume(&id, Operation::Commit, now).is_err());
    }

    #[test]
    fn issuing_drops_used_and_expired_ids() {
        let (previews, used) = issued();
        previews
            .consume(&used, Operation::Commit, Utc::now())
            .unwrap();
        let expired = previews.issue(
            Operation::Commit,
            "repo".into(),
            "profile".into(),
            Fingerprint::commit("main".into(), "head".into(), vec![]),
        );
        previews
            .0
            .lock()
            .unwrap()
            .get_mut(&expired)
            .unwrap()
            .created_at = Utc::now() - Duration::minutes(11);
        let fresh = previews.issue(
            Operation::Commit,
            "repo".into(),
            "profile".into(),
            Fingerprint::commit("main".into(), "head".into(), vec![]),
        );
        let entries = previews.0.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries.contains_key(&fresh));
    }

    #[test]
    fn file_order_does_not_change_fingerprint() {
        let a = ExactChange {
            path: "a".into(),
            original_path: None,
            status: "M".into(),
        };
        let b = ExactChange {
            path: "b".into(),
            original_path: None,
            status: "??".into(),
        };
        assert_eq!(
            Fingerprint::commit("main".into(), "head".into(), vec![a.clone(), b.clone()]),
            Fingerprint::commit("main".into(), "head".into(), vec![b, a])
        );
    }

    #[test]
    fn pending_confirmations_expire_and_are_single_use() {
        let (previews, preview_id) = issued();
        let entry = previews
            .consume(&preview_id, Operation::Commit, Utc::now())
            .unwrap();
        let pending = PendingConfirmations::default();
        let timeout = StdDuration::from_secs(120);
        let expired = pending.issue(
            entry.clone(),
            preview_id.clone(),
            "push",
            "Confirm".into(),
            timeout,
        );
        pending
            .0
            .lock()
            .unwrap()
            .get_mut(&expired)
            .unwrap()
            .created_at -= timeout;
        let fresh = pending.issue(entry, preview_id, "push", "Confirm".into(), timeout);
        assert_eq!(pending.0.lock().unwrap().len(), 1);
        assert!(pending.take(&expired, "push", timeout).is_err());
        assert!(pending.take(&fresh, "push", timeout).is_ok());
        assert!(pending.take(&fresh, "push", timeout).is_err());
    }
}

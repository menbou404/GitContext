use crate::storage::StateStore;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
};

const MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit<'a> {
    pub at: String,
    pub tool: &'a str,
    pub repository_id: Option<&'a str>,
    pub profile_id: Option<&'a str>,
    pub outcome: &'a str,
    pub summary: &'a str,
    pub client: Option<&'a str>,
    pub confirmation: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<&'a str>,
}

impl<'a> Audit<'a> {
    pub fn new(
        tool: &'a str,
        repository_id: Option<&'a str>,
        profile_id: Option<&'a str>,
        outcome: &'a str,
        summary: &'a str,
        client: Option<&'a str>,
        confirmation: Option<&'a str>,
    ) -> Self {
        Self {
            at: Utc::now().to_rfc3339(),
            tool,
            repository_id,
            profile_id,
            outcome,
            summary,
            client,
            confirmation,
            actor: None,
        }
    }

    pub fn with_actor(mut self, actor: &'a str) -> Self {
        self.actor = Some(actor);
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub at: String,
    pub tool: String,
    pub repository_id: Option<String>,
    pub profile_id: Option<String>,
    pub outcome: String,
    pub summary: String,
    pub client: Option<String>,
    pub confirmation: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
}

pub fn append(store: &StateStore, record: &Audit<'_>) -> Result<(), String> {
    let _guard = store.lock()?;
    let current = store.config_dir().join("mcp-audit.jsonl");
    let rotated = store.config_dir().join("mcp-audit.1.jsonl");
    let mut line =
        serde_json::to_vec(record).map_err(|_| "Could not encode audit record.".to_string())?;
    line.push(b'\n');
    if current
        .metadata()
        .is_ok_and(|meta| meta.len() + line.len() as u64 > MAX_BYTES)
    {
        if rotated.exists() {
            fs::remove_file(&rotated).map_err(|_| "Could not rotate audit log.".to_string())?;
        }
        fs::rename(&current, &rotated).map_err(|_| "Could not rotate audit log.".to_string())?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(current)
        .and_then(|mut file| file.write_all(&line))
        .map_err(|_| "Could not write audit log.".to_string())
}

pub const HISTORY_LIMIT: usize = 500;

pub fn read_recent(store: &StateStore) -> Result<Vec<AuditEntry>, String> {
    let _guard = store.lock()?;
    let mut entries = Vec::new();
    for name in ["mcp-audit.1.jsonl", "mcp-audit.jsonl"] {
        let path = store.config_dir().join(name);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("Could not read audit log.".into()),
        };
        entries.extend(
            contents
                .split(|byte| *byte == b'\n')
                .filter_map(|line| serde_json::from_slice::<AuditEntry>(line).ok()),
        );
    }
    entries.reverse();
    entries.sort_by(|left, right| right.at.cmp(&left.at));
    entries.truncate(HISTORY_LIMIT);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_after_five_megabytes() {
        let root = std::env::temp_dir().join(format!("gitcontext-audit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let store = StateStore::new(root.clone());
        fs::write(root.join("mcp-audit.jsonl"), vec![b'x'; MAX_BYTES as usize]).unwrap();
        fs::write(root.join("mcp-audit.1.jsonl"), "old").unwrap();
        append(
            &store,
            &Audit::new(
                "commit",
                Some("repo"),
                Some("profile"),
                "success",
                "commit abc",
                None,
                None,
            ),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(root.join("mcp-audit.1.jsonl")).unwrap().len(),
            MAX_BYTES
        );
        let line = fs::read_to_string(root.join("mcp-audit.jsonl")).unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains("commit abc"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_two_files_newest_first_skipping_invalid_lines_and_capping_results() {
        let root =
            std::env::temp_dir().join(format!("gitcontext-history-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let store = StateStore::new(root.clone());
        let mut older = Audit::new(
            "commit",
            Some("repo"),
            None,
            "success",
            "commit abc",
            Some("Client"),
            None,
        );
        older.at = "2026-10-03T00:00:00Z".into();
        append(&store, &older).unwrap();
        fs::rename(root.join("mcp-audit.jsonl"), root.join("mcp-audit.1.jsonl")).unwrap();
        fs::write(root.join("mcp-audit.jsonl"), b"broken line\n\xff\n").unwrap();
        for index in 0..HISTORY_LIMIT + 2 {
            let mut entry = Audit::new(
                "apply_profile",
                Some("repo"),
                Some("profile"),
                "success",
                "Profile applied",
                None,
                None,
            )
            .with_actor("gui");
            entry.at = format!("2026-10-04T00:{:02}:{:02}Z", index / 60, index % 60);
            append(&store, &entry).unwrap();
            if index == 0 {
                let combined = read_recent(&store).unwrap();
                assert_eq!(combined.len(), 2);
                assert_eq!(combined[0].tool, "apply_profile");
                assert_eq!(combined[1].tool, "commit");
            }
        }
        let entries = read_recent(&store).unwrap();
        assert_eq!(entries.len(), HISTORY_LIMIT);
        assert_eq!(entries[0].actor.as_deref(), Some("gui"));
        assert!(entries.windows(2).all(|pair| pair[0].at >= pair[1].at));
        assert!(!entries.iter().any(|entry| entry.tool == "commit"));
        fs::remove_dir_all(root).unwrap();
    }
}

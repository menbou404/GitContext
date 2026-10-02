use chrono::Utc;
use gitcontext_core::storage::StateStore;
use serde::Serialize;
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
}

impl<'a> Audit<'a> {
    pub fn new(
        tool: &'a str,
        repository_id: Option<&'a str>,
        profile_id: Option<&'a str>,
        outcome: &'a str,
        summary: &'a str,
        client: Option<&'a str>,
    ) -> Self {
        Self {
            at: Utc::now().to_rfc3339(),
            tool,
            repository_id,
            profile_id,
            outcome,
            summary,
            client,
        }
    }
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
}

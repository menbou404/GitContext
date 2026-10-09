use crate::storage::StateStore;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::SystemTime,
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

/// The offsets refer to bytes already consumed, including an unfinished last line.
#[derive(Debug, Default)]
pub struct AuditTail {
    pub current_offset: u64,
    pending: Vec<u8>,
    modified: Option<SystemTime>,
    rotated_stamp: (u64, Option<SystemTime>),
}

impl AuditTail {
    pub fn at_end(dir: &Path) -> Result<Self, String> {
        let (current_offset, modified) = file_stamp(&dir.join("mcp-audit.jsonl"))?;
        Ok(Self {
            current_offset,
            pending: Vec::new(),
            modified,
            rotated_stamp: file_stamp(&dir.join("mcp-audit.1.jsonl"))?,
        })
    }

    pub fn read_new(&mut self, dir: &Path) -> Result<Vec<AuditEntry>, String> {
        let current = dir.join("mcp-audit.jsonl");
        let rotated = dir.join("mcp-audit.1.jsonl");
        let (current_len, modified) = file_stamp(&current)?;
        let rotated_stamp = file_stamp(&rotated)?;
        let rotated_len = if rotated_stamp != self.rotated_stamp {
            rotated_stamp.0
        } else {
            0
        };
        let replaced_at_same_size = current_len == self.current_offset
            && current_len > 0
            && modified != self.modified
            && rotated_len > self.current_offset;
        let plan = if replaced_at_same_size {
            plan_tail_read(self.current_offset, 0, rotated_len)
        } else {
            plan_tail_read(self.current_offset, current_len, rotated_len)
        };
        let mut entries = Vec::new();
        if (current_len < self.current_offset || replaced_at_same_size)
            && plan.rotated_from.is_none()
        {
            self.pending.clear();
        }
        if let Some(offset) = plan.rotated_from {
            let bytes = read_from(&rotated, offset)?;
            entries.extend(consume_complete_lines(&mut self.pending, &bytes));
            // A rotated file cannot receive more bytes. Drop any incomplete last line.
            self.pending.clear();
        }
        let bytes = read_from(&current, plan.current_from)?;
        entries.extend(consume_complete_lines(&mut self.pending, &bytes));
        self.current_offset = plan.current_from + bytes.len() as u64;
        self.modified = modified;
        self.rotated_stamp = rotated_stamp;
        Ok(entries)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TailReadPlan {
    pub rotated_from: Option<u64>,
    pub current_from: u64,
}

/// Decides which byte ranges to read. Rotation moves the former current file to `.1`.
pub fn plan_tail_read(current_offset: u64, current_len: u64, rotated_len: u64) -> TailReadPlan {
    if current_len < current_offset {
        TailReadPlan {
            rotated_from: (rotated_len > current_offset).then_some(current_offset),
            current_from: 0,
        }
    } else {
        TailReadPlan {
            rotated_from: None,
            current_from: current_offset,
        }
    }
}

/// Parses only newline-terminated records and retains a partial record for the next poll.
pub fn consume_complete_lines(pending: &mut Vec<u8>, chunk: &[u8]) -> Vec<AuditEntry> {
    pending.extend_from_slice(chunk);
    let mut entries = Vec::new();
    let mut consumed = 0;
    for (index, byte) in pending.iter().enumerate() {
        if *byte == b'\n' {
            if let Ok(entry) = serde_json::from_slice::<AuditEntry>(&pending[consumed..index]) {
                entries.push(entry);
            }
            consumed = index + 1;
        }
    }
    pending.drain(..consumed);
    entries
}

fn file_stamp(path: &Path) -> Result<(u64, Option<SystemTime>), String> {
    match fs::metadata(path) {
        Ok(meta) => Ok((meta.len(), meta.modified().ok())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((0, None)),
        Err(error) => Err(format!("Could not inspect audit log: {error}")),
    }
}

fn read_from(path: &Path, offset: u64) -> Result<Vec<u8>, String> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Could not read audit log: {error}")),
    };
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("Could not seek audit log: {error}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read audit log: {error}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_parses_append_partial_and_invalid_lines() {
        let entry = br#"{"at":"now","tool":"push","repositoryId":"r","profileId":null,"outcome":"success","summary":"ok","client":null,"confirmation":null}"#;
        let mut pending = Vec::new();
        assert!(consume_complete_lines(&mut pending, &entry[..20]).is_empty());
        let mut rest = entry[20..].to_vec();
        rest.extend_from_slice(b"\nbad json\n");
        let entries = consume_complete_lines(&mut pending, &rest);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool, "push");
        assert!(pending.is_empty());
    }

    #[test]
    fn tail_rotation_reads_old_unread_bytes_before_new_file() {
        assert_eq!(plan_tail_read(100, 120, 0).current_from, 100);
        assert_eq!(
            plan_tail_read(100, 12, 140),
            TailReadPlan {
                rotated_from: Some(100),
                current_from: 0
            }
        );
        assert_eq!(
            plan_tail_read(100, 12, 0),
            TailReadPlan {
                rotated_from: None,
                current_from: 0
            }
        );
    }

    #[test]
    fn tail_starts_at_end_and_reads_rotated_remainder_then_new_file() {
        let root = std::env::temp_dir().join(format!("gitcontext-tail-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("mcp-audit.jsonl");
        let line = b"{\"at\":\"now\",\"tool\":\"push\",\"repositoryId\":null,\"profileId\":null,\"outcome\":\"success\",\"summary\":\"ok\",\"client\":null,\"confirmation\":null}\n";
        fs::write(&path, line).unwrap();
        let mut tail = AuditTail::at_end(&root).unwrap();
        assert!(tail.read_new(&root).unwrap().is_empty());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(line)
            .unwrap();
        assert_eq!(tail.read_new(&root).unwrap().len(), 1);
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(line)
            .unwrap();
        fs::rename(&path, root.join("mcp-audit.1.jsonl")).unwrap();
        fs::write(&path, line).unwrap();
        let entries = tail.read_new(&root).unwrap();
        assert_eq!(entries.len(), 2);
        fs::remove_dir_all(root).unwrap();
    }

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

use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use crate::models::AppData;

#[derive(Clone)]
pub struct StateStore {
    config_dir: PathBuf,
}

pub struct StateLock {
    file: File,
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl StateStore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn state_path(&self) -> PathBuf {
        self.config_dir.join("state.json")
    }

    pub fn lock(&self) -> Result<StateLock, String> {
        self.lock_with_timeout(Duration::from_secs(5))
    }

    fn lock_with_timeout(&self, timeout: Duration) -> Result<StateLock, String> {
        fs::create_dir_all(&self.config_dir)
            .map_err(|error| format!("Could not create {}: {error}", self.config_dir.display()))?;
        let path = self.config_dir.join("state.lock");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
        let started = Instant::now();
        loop {
            if file.try_lock().is_ok() {
                return Ok(StateLock { file });
            }
            if started.elapsed() >= timeout {
                return Err(
                    "Another GitContext operation is in progress. Try again in a moment.".into(),
                );
            }
            thread::sleep(Duration::from_millis(50).min(timeout.saturating_sub(started.elapsed())));
        }
    }

    pub fn load(&self) -> Result<AppData, String> {
        let path = self.state_path();
        let backup = path.with_extension("json.backup");
        if !path.exists() && backup.exists() {
            fs::rename(&backup, &path)
                .map_err(|error| format!("Could not recover GitContext settings: {error}"))?;
        }
        if !path.exists() {
            let initial = AppData::default();
            self.save(&initial)?;
            return Ok(initial);
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        let mut data: AppData = serde_json::from_slice(&bytes)
            .map_err(|error| format!("GitContext settings are invalid JSON: {error}"))?;
        if crate::models::migrate_app_data(&mut data) {
            self.save(&data)?;
        }
        Ok(data)
    }

    pub fn save(&self, data: &AppData) -> Result<(), String> {
        let path = self.state_path();
        let parent = path
            .parent()
            .ok_or_else(|| "The settings path has no parent directory.".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        let bytes = serde_json::to_vec_pretty(data)
            .map_err(|error| format!("Could not serialize GitContext settings: {error}"))?;
        let temporary = path.with_extension("json.tmp");
        let backup = path.with_extension("json.backup");
        fs::write(&temporary, bytes)
            .map_err(|error| format!("Could not stage {}: {error}", temporary.display()))?;

        if backup.exists() {
            fs::remove_file(&backup)
                .map_err(|error| format!("Could not clear an old settings backup: {error}"))?;
        }
        if path.exists() {
            fs::rename(&path, &backup)
                .map_err(|error| format!("Could not back up existing settings: {error}"))?;
        }
        if let Err(error) = fs::rename(&temporary, &path) {
            if backup.exists() {
                let _ = fs::rename(&backup, &path);
            }
            return Err(format!("Could not activate the new settings file: {error}"));
        }
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| {
                format!(
                    "Settings were saved, but their temporary backup could not be removed: {error}"
                )
            })?;
        }
        Ok(())
    }
}

pub fn default_config_dir() -> Result<PathBuf, String> {
    const IDENTIFIER: &str = "app.gitcontext.desktop";
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config"))
        });
    base.map(|base| base.join(IDENTIFIER))
        .ok_or_else(|| "Could not locate GitContext's settings directory.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn temporary_store() -> StateStore {
        StateStore::new(
            std::env::temp_dir().join(format!("gitcontext-core-{}", uuid::Uuid::new_v4())),
        )
    }

    #[test]
    fn load_save_round_trip() {
        let store = temporary_store();
        let mut data = store.load().unwrap();
        assert!(store.state_path().exists());
        assert!(data.profiles.is_empty());
        data.version = 3;
        store.save(&data).unwrap();
        assert_eq!(store.load().unwrap().version, 3);
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    fn lock_waits_and_times_out() {
        let store = temporary_store();
        let guard = store.lock().unwrap();
        let other = store.clone();
        let waiting = thread::spawn(move || {
            other
                .lock_with_timeout(Duration::from_millis(150))
                .err()
                .unwrap()
        });
        assert_eq!(
            waiting.join().unwrap(),
            "Another GitContext operation is in progress. Try again in a moment."
        );
        let other = store.clone();
        let barrier = Arc::new(Barrier::new(2));
        let ready = barrier.clone();
        let waiting = thread::spawn(move || {
            ready.wait();
            other.lock().unwrap();
        });
        barrier.wait();
        thread::sleep(Duration::from_millis(100));
        assert!(!waiting.is_finished());
        drop(guard);
        waiting.join().unwrap();
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    fn concurrent_updates_are_preserved() {
        let store = temporary_store();
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let store = store.clone();
                thread::spawn(move || {
                    for _ in 0..20 {
                        let _guard = store.lock().unwrap();
                        let mut data = store.load().unwrap();
                        data.version += 1;
                        store.save(&data).unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(store.load().unwrap().version, 42);
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_config_dir_uses_appdata() {
        let expected =
            PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("app.gitcontext.desktop");
        assert_eq!(default_config_dir().unwrap(), expected);
    }
}

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use crate::models::AppData;
use chrono::Utc;

const RELEASE_IDENTIFIER: &str = "app.gitcontext.desktop";
const DEVELOPMENT_IDENTIFIER: &str = "app.gitcontext.dev";
const BACKUP_LIMIT: usize = 20;

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
        #[cfg(test)]
        for release_dir in [platform_config_dir(), legacy_config_dir()]
            .into_iter()
            .flatten()
        {
            assert!(
                config_dir != release_dir
                    && config_dir != release_dir.with_file_name(DEVELOPMENT_IDENTIFIER)
                    && config_dir != release_dir.with_file_name(RELEASE_IDENTIFIER)
                    && config_dir != release_dir.with_file_name(".gitcontext-dev"),
                "tests must not use a real GitContext settings directory"
            );
        }
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
        // save() backs up the pre-migration file before replacing it.
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
        if path.exists()
            && fs::read(&path)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?
                == bytes
        {
            return Ok(());
        }
        self.backup_existing_state()?;
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

    fn backup_existing_state(&self) -> Result<(), String> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(());
        }
        let existing = fs::read(&path)
            .map_err(|error| format!("Could not read {} for backup: {error}", path.display()))?;
        let backup_dir = self.config_dir.join("backups");
        fs::create_dir_all(&backup_dir)
            .map_err(|error| format!("Could not create {}: {error}", backup_dir.display()))?;
        let mut backups = state_backups(&backup_dir)?;
        backups.sort();
        if let Some(latest) = backups.last() {
            let previous = fs::read(latest)
                .map_err(|error| format!("Could not read {}: {error}", latest.display()))?;
            if previous == existing {
                return Ok(());
            }
        }

        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let mut sequence = 0;
        let backup_path = loop {
            let name = if sequence == 0 {
                format!("state-{timestamp}.json")
            } else {
                format!("state-{timestamp}~{sequence:04}.json")
            };
            let candidate = backup_dir.join(name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(&existing).and_then(|_| file.sync_all()) {
                        drop(file);
                        let _ = fs::remove_file(&candidate);
                        return Err(format!("Could not write {}: {error}", candidate.display()));
                    }
                    break candidate;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    sequence += 1;
                }
                Err(error) => {
                    return Err(format!("Could not create {}: {error}", candidate.display()));
                }
            }
        };
        backups.push(backup_path);
        backups.sort();
        for old in backups
            .iter()
            .take(backups.len().saturating_sub(BACKUP_LIMIT))
        {
            fs::remove_file(old)
                .map_err(|error| format!("Could not remove {}: {error}", old.display()))?;
        }
        Ok(())
    }
}

fn state_backups(backup_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut backups = Vec::new();
    for entry in fs::read_dir(backup_dir)
        .map_err(|error| format!("Could not list {}: {error}", backup_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("Could not list backup: {error}"))?;
        let is_file = entry
            .file_type()
            .map_err(|error| format!("Could not inspect backup: {error}"))?
            .is_file();
        let is_state_backup = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("state-") && name.ends_with(".json"));
        if is_file && is_state_backup {
            backups.push(entry.path());
        }
    }
    Ok(backups)
}

pub fn build_config_dir(release_config_dir: PathBuf) -> Result<PathBuf, String> {
    build_config_dir_with_override(
        release_config_dir,
        std::env::var_os("GITCONTEXT_DATA_DIR"),
        cfg!(debug_assertions),
    )
}

fn build_config_dir_with_override(
    release_config_dir: PathBuf,
    override_dir: Option<OsString>,
    debug: bool,
) -> Result<PathBuf, String> {
    if let Some(override_dir) = override_dir.filter(|value| !value.is_empty()) {
        let override_dir = PathBuf::from(override_dir);
        if !override_dir.is_absolute() {
            return Err("GITCONTEXT_DATA_DIR must be an absolute path.".into());
        }
        return Ok(override_dir);
    }
    if debug {
        return Ok(release_config_dir.with_file_name(if cfg!(windows) {
            ".gitcontext-dev"
        } else {
            DEVELOPMENT_IDENTIFIER
        }));
    }
    Ok(release_config_dir)
}

pub fn development_data() -> bool {
    cfg!(debug_assertions)
        || std::env::var_os("GITCONTEXT_DATA_DIR").is_some_and(|value| !value.is_empty())
}

pub fn default_config_dir() -> Result<PathBuf, String> {
    if let Some(override_dir) = std::env::var_os("GITCONTEXT_DATA_DIR").filter(|v| !v.is_empty()) {
        return build_config_dir_with_override(PathBuf::new(), Some(override_dir), false);
    }
    build_config_dir(platform_config_dir()?)
}

/// Resolve the same directory for the GUI and MCP, and migrate before either loads state.
pub fn open_default_store() -> Result<StateStore, String> {
    let override_dir = std::env::var_os("GITCONTEXT_DATA_DIR").filter(|value| !value.is_empty());
    let store = StateStore::new(default_config_dir()?);
    #[cfg(windows)]
    if override_dir.is_none() && !store.state_path().exists() {
        if let Ok(old_dir) = legacy_config_dir() {
            migrate_from_legacy(&store, &old_dir, false, || {})?;
        }
    }
    #[cfg(not(windows))]
    let _ = override_dir;
    Ok(store)
}

#[cfg(windows)]
fn migrate_from_legacy(
    store: &StateStore,
    old_dir: &Path,
    overridden: bool,
    after_lock: impl FnOnce(),
) -> Result<(), String> {
    if overridden
        || store.state_path().exists()
        || !legacy_state_is_regular(&old_dir.join("state.json"))?
    {
        return Ok(());
    }
    let _guard = store.lock()?;
    after_lock();
    if store.state_path().exists() {
        return Ok(());
    }
    copy_legacy_entries(old_dir, store.config_dir(), true)?;
    let original = fs::read(old_dir.join("state.json"))
        .map_err(|error| format!("Could not read legacy settings: {error}"))?;
    let mut data: AppData = serde_json::from_slice(&original)
        .map_err(|error| format!("Legacy GitContext settings are invalid JSON: {error}"))?;
    let changed = rewrite_gh_config_dirs(&mut data, old_dir, store.config_dir());
    let temporary = store.config_dir().join("state.json.tmp");
    fs::write(&temporary, &original)
        .map_err(|error| format!("Could not stage migrated settings: {error}"))?;
    fs::rename(&temporary, store.state_path())
        .map_err(|error| format!("Could not activate migrated settings: {error}"))?;
    if changed {
        if let Err(error) = store.save(&data) {
            let _ = fs::remove_file(store.state_path());
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn legacy_state_is_regular(path: &Path) -> Result<bool, String> {
    use std::os::windows::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && metadata.file_attributes() & 0x400 == 0),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Could not inspect legacy settings: {error}")),
    }
}

#[cfg(windows)]
fn copy_legacy_entries(source: &Path, destination: &Path, root: bool) -> Result<(), String> {
    for entry in
        fs::read_dir(source).map_err(|error| format!("Could not list legacy settings: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Could not list legacy entry: {error}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case("state.lock")
            || name.eq_ignore_ascii_case("state.json.backup")
            || name.to_ascii_lowercase().ends_with(".tmp")
            || (root && name.eq_ignore_ascii_case("state.json"))
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("Could not inspect legacy entry: {error}"))?;
        use std::os::windows::fs::MetadataExt;
        if metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0 {
            continue;
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| format!("Could not create migrated directory: {error}"))?;
            copy_legacy_entries(&entry.path(), &target, false)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|error| format!("Could not copy legacy settings: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn rewrite_gh_config_dirs(data: &mut AppData, old_dir: &Path, new_dir: &Path) -> bool {
    let old = old_dir.to_string_lossy().replace('/', "\\");
    let old_parts: Vec<_> = old.split('\\').filter(|part| !part.is_empty()).collect();
    let mut changed = false;
    for profile in &mut data.profiles {
        if let Some(path) = &mut profile.gh_config_dir {
            let normalized = path.replace('/', "\\");
            let parts: Vec<_> = normalized
                .split('\\')
                .filter(|part| !part.is_empty())
                .collect();
            if parts.len() >= old_parts.len()
                && !parts.iter().any(|part| *part == "." || *part == "..")
                && parts[..old_parts.len()]
                    .iter()
                    .zip(&old_parts)
                    .all(|(part, old)| part.to_lowercase() == old.to_lowercase())
            {
                let mut new_path = new_dir.to_path_buf();
                for part in &parts[old_parts.len()..] {
                    new_path.push(part);
                }
                *path = new_path.to_string_lossy().into_owned();
                changed = true;
            }
        }
    }
    changed
}

#[cfg(windows)]
fn legacy_config_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "Could not locate legacy GitContext settings directory.".to_string())?;
    Ok(legacy_config_dir_from_base(&base, cfg!(debug_assertions)))
}

#[cfg(windows)]
fn legacy_config_dir_from_base(base: &Path, debug: bool) -> PathBuf {
    base.join(if debug {
        DEVELOPMENT_IDENTIFIER
    } else {
        RELEASE_IDENTIFIER
    })
}

#[cfg(not(windows))]
#[cfg(test)]
fn legacy_config_dir() -> Result<PathBuf, String> {
    Err("No legacy directory on this platform.".into())
}

fn platform_config_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("USERPROFILE").map(PathBuf::from);
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
    base.map(|base| {
        #[cfg(windows)]
        {
            windows_config_dir_from_home(&base, false)
        }
        #[cfg(not(windows))]
        {
            base.join(RELEASE_IDENTIFIER)
        }
    })
    .ok_or_else(|| "Could not locate GitContext's settings directory.".into())
}

#[cfg(windows)]
fn windows_config_dir_from_home(home: &Path, debug: bool) -> PathBuf {
    home.join(if debug {
        ".gitcontext-dev"
    } else {
        ".gitcontext"
    })
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

    fn backup_paths(store: &StateStore) -> Vec<PathBuf> {
        let backup_dir = store.config_dir().join("backups");
        if !backup_dir.exists() {
            return Vec::new();
        }
        state_backups(&backup_dir).unwrap()
    }

    #[test]
    fn locale_survives_legacy_state_and_other_saves() {
        let store = temporary_store();
        fs::create_dir_all(store.config_dir()).unwrap();
        fs::write(
            store.state_path(),
            r#"{"version":2,"profiles":[],"repositories":[]}"#,
        )
        .unwrap();
        assert!(store.load().unwrap().settings.locale.is_none());

        let settings = crate::operations::set_locale(&store, "ja".into()).unwrap();
        assert_eq!(settings.locale.as_deref(), Some("ja"));
        assert_eq!(store.load().unwrap().settings.locale.as_deref(), Some("ja"));

        crate::operations::save_profile(
            &store,
            crate::models::Profile {
                id: "test-profile".into(),
                label: "Test".into(),
                accent: "#112233".into(),
                git_name: "Example".into(),
                git_email: "test@example.com".into(),
                github_username: None,
                ssh_key_path: None,
                gh_config_dir: None,
                auto_approve: crate::models::ProfileAutoApprove::default(),
            },
        )
        .unwrap();
        let data = store.load().unwrap();
        assert_eq!(data.settings.locale.as_deref(), Some("ja"));
        assert_eq!(data.profiles.len(), 1);
        assert_eq!(
            crate::operations::set_locale(&store, "fr".into())
                .unwrap()
                .locale
                .as_deref(),
            Some("ja")
        );
        assert_eq!(store.load().unwrap().settings.locale.as_deref(), Some("ja"));
    }

    #[test]
    fn invalid_saved_locales_are_ignored() {
        let store = temporary_store();
        fs::create_dir_all(store.config_dir()).unwrap();
        for value in [r#""fr""#, "null", "12", "true"] {
            fs::write(
                store.state_path(),
                format!(r#"{{"version":2,"profiles":[],"repositories":[],"settings":{{"locale":{value}}}}}"#),
            )
            .unwrap();
            assert!(store.load().unwrap().settings.locale.is_none());
        }
    }

    #[test]
    fn config_dir_selects_debug_sibling_and_rejects_relative_override() {
        let release = std::env::temp_dir()
            .join("gitcontext-config-test")
            .join(if cfg!(windows) {
                ".gitcontext"
            } else {
                RELEASE_IDENTIFIER
            });
        assert_eq!(
            build_config_dir_with_override(release.clone(), None, true).unwrap(),
            release.with_file_name(if cfg!(windows) {
                ".gitcontext-dev"
            } else {
                DEVELOPMENT_IDENTIFIER
            })
        );
        assert_eq!(
            build_config_dir_with_override(release.clone(), None, false).unwrap(),
            release
        );
        assert!(build_config_dir_with_override(
            release.clone(),
            Some(OsString::from("relative-data")),
            true
        )
        .is_err());
        assert_eq!(
            build_config_dir_with_override(
                release.clone(),
                Some(release.clone().into_os_string()),
                true
            )
            .unwrap(),
            release
        );
    }

    #[test]
    fn saves_keep_two_distinct_previous_versions() {
        let store = temporary_store();
        let mut data = AppData::default();
        store.save(&data).unwrap();
        let first = fs::read(store.state_path()).unwrap();
        data.version = 3;
        store.save(&data).unwrap();
        let second = fs::read(store.state_path()).unwrap();
        data.version = 4;
        store.save(&data).unwrap();
        let backups = backup_paths(&store);
        assert_eq!(backups.len(), 2);
        let contents: Vec<_> = backups.iter().map(|path| fs::read(path).unwrap()).collect();
        assert!(contents.contains(&first));
        assert!(contents.contains(&second));
        store.save(&data).unwrap();
        assert_eq!(backup_paths(&store).len(), 2);
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    fn backups_keep_twenty_versions_and_leave_other_files() {
        let store = temporary_store();
        let mut data = AppData::default();
        store.save(&data).unwrap();
        let backup_dir = store.config_dir().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();
        fs::write(backup_dir.join("note.txt"), "keep me").unwrap();
        for version in 3..=25 {
            data.version = version;
            store.save(&data).unwrap();
        }
        assert_eq!(backup_paths(&store).len(), BACKUP_LIMIT);
        assert_eq!(
            fs::read_to_string(backup_dir.join("note.txt")).unwrap(),
            "keep me"
        );
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    fn invalid_json_is_preserved_on_load() {
        let store = temporary_store();
        fs::create_dir_all(store.config_dir()).unwrap();
        let invalid = b"{invalid json";
        fs::write(store.state_path(), invalid).unwrap();
        assert!(store.load().is_err());
        assert_eq!(fs::read(store.state_path()).unwrap(), invalid);
        assert!(backup_paths(&store).is_empty());
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    fn migration_backs_up_original_bytes() {
        let store = temporary_store();
        fs::create_dir_all(store.config_dir()).unwrap();
        let original = br#"{"version":1,"profiles":[],"repositories":[]}"#;
        fs::write(store.state_path(), original).unwrap();
        assert_eq!(store.load().unwrap().version, 2);
        let backups = backup_paths(&store);
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&backups[0]).unwrap(), original);
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    fn failed_backup_prevents_save() {
        let store = temporary_store();
        let mut data = AppData::default();
        store.save(&data).unwrap();
        let original = fs::read(store.state_path()).unwrap();
        fs::write(store.config_dir().join("backups"), "not a directory").unwrap();
        data.version = 3;
        assert!(store.save(&data).is_err());
        assert_eq!(fs::read(store.state_path()).unwrap(), original);
        fs::remove_dir_all(store.config_dir()).unwrap();
    }

    #[test]
    #[should_panic(expected = "tests must not use a real GitContext settings directory")]
    fn real_config_dir_is_forbidden_in_tests() {
        StateStore::new(platform_config_dir().unwrap());
    }

    #[test]
    #[should_panic(expected = "tests must not use a real GitContext settings directory")]
    fn real_development_config_dir_is_forbidden_in_tests() {
        StateStore::new(
            platform_config_dir()
                .unwrap()
                .with_file_name(if cfg!(windows) {
                    ".gitcontext-dev"
                } else {
                    DEVELOPMENT_IDENTIFIER
                }),
        );
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
    fn windows_default_config_dir_uses_home() {
        let home = std::env::temp_dir().join("gitcontext-test-home");
        let roaming = std::env::temp_dir().join("gitcontext-test-roaming");
        assert_eq!(
            windows_config_dir_from_home(&home, false),
            home.join(".gitcontext")
        );
        assert_eq!(
            windows_config_dir_from_home(&home, true),
            home.join(".gitcontext-dev")
        );
        assert_eq!(
            legacy_config_dir_from_base(&roaming, false),
            roaming.join(RELEASE_IDENTIFIER)
        );
        assert_eq!(
            legacy_config_dir_from_base(&roaming, true),
            roaming.join(DEVELOPMENT_IDENTIFIER)
        );
    }

    #[cfg(windows)]
    fn migration_dirs() -> (PathBuf, PathBuf, StateStore) {
        let root =
            std::env::temp_dir().join(format!("gitcontext-migrate-{}", uuid::Uuid::new_v4()));
        let old = root.join("roaming").join(DEVELOPMENT_IDENTIFIER);
        let new = root.join("home").join(".gitcontext-dev");
        fs::create_dir_all(&old).unwrap();
        (root, old, StateStore::new(new))
    }

    #[cfg(windows)]
    #[test]
    fn migrates_files_and_rewrites_only_legacy_gh_paths() {
        let (root, old, store) = migration_dirs();
        let inside = old.join("gh").join("one");
        let outside = root.join("other").join("gh");
        let state = serde_json::json!({
            "version": 2,
            "profiles": [
                {"id":"one","label":"One","accent":"blue","gitName":"Test","gitEmail":"test@example.com","ghConfigDir":inside.to_string_lossy().to_uppercase()},
                {"id":"two","label":"Two","accent":"blue","gitName":"Test","gitEmail":"test@example.com","ghConfigDir":outside.to_string_lossy()},
                {"id":"three","label":"Three","accent":"blue","gitName":"Test","gitEmail":"test@example.com","ghConfigDir":format!("{}-other", old.display())}
            ], "repositories": []
        });
        let original = serde_json::to_vec(&state).unwrap();
        fs::write(old.join("state.json"), &original).unwrap();
        fs::create_dir_all(&inside).unwrap();
        fs::write(inside.join("config.yml"), "credential").unwrap();
        fs::create_dir_all(old.join("backups")).unwrap();
        fs::write(old.join("backups").join("state-old.json"), "backup").unwrap();
        fs::write(old.join("mcp-audit.jsonl"), "audit").unwrap();
        fs::write(old.join("mcp-audit.jsonl.1"), "rotated").unwrap();
        fs::write(old.join("state.lock"), "old lock").unwrap();
        fs::write(old.join("ignored.tmp"), "temporary").unwrap();
        fs::write(old.join("state.json.backup"), "old temporary backup").unwrap();
        migrate_from_legacy(&store, &old, false, || {}).unwrap();
        let data = store.load().unwrap();
        assert_eq!(
            data.profiles[0]
                .gh_config_dir
                .as_deref()
                .unwrap()
                .to_lowercase(),
            store
                .config_dir()
                .join("gh")
                .join("one")
                .to_string_lossy()
                .to_lowercase()
        );
        assert_eq!(
            data.profiles[1].gh_config_dir.as_deref(),
            Some(outside.to_str().unwrap())
        );
        assert_eq!(
            data.profiles[2].gh_config_dir.as_deref(),
            Some(format!("{}-other", old.display()).as_str())
        );
        assert_eq!(
            fs::read(store.config_dir().join("gh/one/config.yml")).unwrap(),
            b"credential"
        );
        assert_eq!(
            fs::read(store.config_dir().join("mcp-audit.jsonl.1")).unwrap(),
            b"rotated"
        );
        assert_eq!(
            fs::read(store.config_dir().join("backups/state-old.json")).unwrap(),
            b"backup"
        );
        assert!(!store.config_dir().join("ignored.tmp").exists());
        assert!(!store.config_dir().join("state.json.backup").exists());
        assert_eq!(
            fs::read(store.config_dir().join("state.lock")).unwrap(),
            b""
        );
        assert_eq!(fs::read(old.join("state.json")).unwrap(), original);
        assert_eq!(fs::read(old.join("state.lock")).unwrap(), b"old lock");
        assert_eq!(fs::read(old.join("ignored.tmp")).unwrap(), b"temporary");
        assert!(backup_paths(&store)
            .iter()
            .any(|path| fs::read(path).unwrap() == original));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn migration_skips_existing_state_override_and_post_lock_state() {
        let (root, old, store) = migration_dirs();
        fs::write(old.join("state.json"), b"legacy").unwrap();
        fs::create_dir_all(store.config_dir()).unwrap();
        fs::write(store.state_path(), b"current").unwrap();
        migrate_from_legacy(&store, &old, false, || panic!("must not lock")).unwrap();
        assert_eq!(fs::read(store.state_path()).unwrap(), b"current");
        fs::remove_file(store.state_path()).unwrap();
        migrate_from_legacy(&store, &old, true, || panic!("must not lock")).unwrap();
        assert!(!store.state_path().exists());
        migrate_from_legacy(&store, &old, false, || {
            fs::write(store.state_path(), b"created while waiting").unwrap();
        })
        .unwrap();
        assert_eq!(
            fs::read(store.state_path()).unwrap(),
            b"created while waiting"
        );
        assert_eq!(fs::read(old.join("state.json")).unwrap(), b"legacy");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn invalid_legacy_state_does_not_activate_new_state() {
        let (root, old, store) = migration_dirs();
        fs::write(old.join("state.json"), b"invalid json").unwrap();
        assert!(migrate_from_legacy(&store, &old, false, || {}).is_err());
        assert!(!store.state_path().exists());
        assert_eq!(fs::read(old.join("state.json")).unwrap(), b"invalid json");
        fs::remove_dir_all(root).unwrap();
    }
}

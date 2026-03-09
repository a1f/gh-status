use crate::error::AppError;
use crate::types::RepoRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Persisted app preferences and cached state.
/// Stored as JSON so the app survives restarts without re-fetching everything.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    /// Which repos to watch, keyed by account_id.
    pub watched_repos: HashMap<String, Vec<RepoRef>>,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
    /// Whether macOS notifications are enabled.
    pub notifications_enabled: bool,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            watched_repos: HashMap::new(),
            poll_interval_secs: 120,
            notifications_enabled: true,
        }
    }
}

/// Thread-safe preferences storage with JSON file persistence.
///
/// Uses `RwLock` with poison recovery so a panicked thread
/// does not permanently lock preferences.
pub struct PreferencesStore {
    prefs: RwLock<AppPreferences>,
    path: PathBuf,
}

impl PreferencesStore {
    /// Creates a store backed by the given file path.
    /// Loads existing preferences from disk, or uses defaults if the file
    /// is missing or corrupt.
    pub fn new(path: PathBuf) -> Self {
        let prefs = Self::load(&path);
        Self {
            prefs: RwLock::new(prefs),
            path,
        }
    }

    /// Returns a snapshot of current preferences.
    pub fn get(&self) -> AppPreferences {
        self.prefs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns watched repos for a specific account.
    pub fn get_watched_repos(&self, account_id: &str) -> Vec<RepoRef> {
        let prefs = self.prefs.read().unwrap_or_else(|e| e.into_inner());
        prefs
            .watched_repos
            .get(account_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Adds a repo to the watchlist for the given account.
    /// Silently skips if the repo is already watched (idempotent).
    pub fn add_watched_repo(
        &self,
        account_id: &str,
        repo: RepoRef,
    ) -> Result<(), AppError> {
        let mut prefs = self.prefs.write().unwrap_or_else(|e| e.into_inner());
        let repos = prefs
            .watched_repos
            .entry(account_id.to_string())
            .or_default();

        if !repos.contains(&repo) {
            repos.push(repo);
        }

        self.save_inner(&prefs)
    }

    /// Removes a repo from the watchlist for the given account.
    /// Succeeds silently if the repo was not watched (idempotent).
    pub fn remove_watched_repo(
        &self,
        account_id: &str,
        repo: &RepoRef,
    ) -> Result<(), AppError> {
        let mut prefs = self.prefs.write().unwrap_or_else(|e| e.into_inner());
        if let Some(repos) = prefs.watched_repos.get_mut(account_id) {
            repos.retain(|r| r != repo);
        }

        self.save_inner(&prefs)
    }

    /// Updates the polling interval.
    pub fn set_poll_interval(&self, secs: u64) -> Result<(), AppError> {
        let mut prefs = self.prefs.write().unwrap_or_else(|e| e.into_inner());
        prefs.poll_interval_secs = secs;
        self.save_inner(&prefs)
    }

    /// Writes preferences to disk. Called while the write lock is held,
    /// so we accept a reference to avoid re-locking.
    fn save_inner(&self, prefs: &AppPreferences) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Config(format!(
                    "failed to create preferences directory: {e}"
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(prefs).map_err(|e| {
            AppError::Config(format!("failed to serialize preferences: {e}"))
        })?;

        fs::write(&self.path, json).map_err(|e| {
            AppError::Config(format!("failed to write preferences file: {e}"))
        })
    }

    /// Loads preferences from a JSON file, returning defaults if the
    /// file is missing or contains invalid JSON.
    fn load(path: &Path) -> AppPreferences {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return AppPreferences::default(),
        };

        serde_json::from_str(&content).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn make_repo(owner: &str, name: &str) -> RepoRef {
        RepoRef {
            owner: owner.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn test_default_preferences() {
        let prefs = AppPreferences::default();
        assert_eq!(prefs.poll_interval_secs, 120);
        assert!(prefs.notifications_enabled);
        assert!(prefs.watched_repos.is_empty());
    }

    #[test]
    fn test_roundtrip_serialize() {
        let mut prefs = AppPreferences::default();
        prefs.poll_interval_secs = 60;
        prefs.notifications_enabled = false;
        prefs
            .watched_repos
            .insert("alice".to_string(), vec![make_repo("org", "repo")]);

        let json = serde_json::to_string(&prefs).unwrap();
        let restored: AppPreferences = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.poll_interval_secs, 60);
        assert!(!restored.notifications_enabled);
        assert_eq!(restored.watched_repos.len(), 1);
        assert_eq!(restored.watched_repos["alice"][0].owner, "org");
        assert_eq!(restored.watched_repos["alice"][0].name, "repo");
    }

    #[test]
    fn test_camelcase_keys() {
        let prefs = AppPreferences::default();
        let json = serde_json::to_string(&prefs).unwrap();

        assert!(json.contains("watchedRepos"), "expected camelCase key 'watchedRepos' in: {json}");
        assert!(json.contains("pollIntervalSecs"), "expected camelCase key 'pollIntervalSecs' in: {json}");
        assert!(json.contains("notificationsEnabled"), "expected camelCase key 'notificationsEnabled' in: {json}");
    }

    #[test]
    fn test_add_watched_repo() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(dir.path().join("prefs.json"));

        store
            .add_watched_repo("alice", make_repo("org", "repo"))
            .unwrap();

        let repos = store.get_watched_repos("alice");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0], make_repo("org", "repo"));
    }

    #[test]
    fn test_add_duplicate_repo() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(dir.path().join("prefs.json"));

        store
            .add_watched_repo("alice", make_repo("org", "repo"))
            .unwrap();
        store
            .add_watched_repo("alice", make_repo("org", "repo"))
            .unwrap();

        let repos = store.get_watched_repos("alice");
        assert_eq!(repos.len(), 1, "duplicate repo should not be added twice");
    }

    #[test]
    fn test_remove_watched_repo() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(dir.path().join("prefs.json"));

        store
            .add_watched_repo("alice", make_repo("org", "repo"))
            .unwrap();
        store
            .remove_watched_repo("alice", &make_repo("org", "repo"))
            .unwrap();

        let repos = store.get_watched_repos("alice");
        assert!(repos.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_repo() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(dir.path().join("prefs.json"));

        let result =
            store.remove_watched_repo("alice", &make_repo("org", "repo"));
        assert!(
            result.is_ok(),
            "removing non-existent repo should be idempotent"
        );
    }

    #[test]
    fn test_multiple_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(dir.path().join("prefs.json"));

        store
            .add_watched_repo("alice", make_repo("org-a", "repo-a"))
            .unwrap();
        store
            .add_watched_repo("bob", make_repo("org-b", "repo-b"))
            .unwrap();

        let alice_repos = store.get_watched_repos("alice");
        let bob_repos = store.get_watched_repos("bob");

        assert_eq!(alice_repos.len(), 1);
        assert_eq!(alice_repos[0], make_repo("org-a", "repo-a"));
        assert_eq!(bob_repos.len(), 1);
        assert_eq!(bob_repos[0], make_repo("org-b", "repo-b"));
    }

    #[test]
    fn test_persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");

        {
            let store = PreferencesStore::new(path.clone());
            store
                .add_watched_repo("alice", make_repo("org", "repo"))
                .unwrap();
            store.set_poll_interval(60).unwrap();
        }

        // New store from same file should have the persisted data.
        let store = PreferencesStore::new(path);
        let prefs = store.get();
        assert_eq!(prefs.poll_interval_secs, 60);

        let repos = store.get_watched_repos("alice");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0], make_repo("org", "repo"));
    }

    #[test]
    fn test_load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            PreferencesStore::new(dir.path().join("nonexistent.json"));

        let prefs = store.get();
        assert_eq!(prefs.poll_interval_secs, 120);
        assert!(prefs.notifications_enabled);
        assert!(prefs.watched_repos.is_empty());
    }

    #[test]
    fn test_load_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");

        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"not valid json {{{{").unwrap();

        let store = PreferencesStore::new(path);
        let prefs = store.get();
        assert_eq!(
            prefs.poll_interval_secs, 120,
            "corrupt file should fall back to defaults"
        );
    }
}

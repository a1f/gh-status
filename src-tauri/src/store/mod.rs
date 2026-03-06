use crate::types::RepoRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Persisted app preferences and cached state.
/// Stored as JSON via Tauri's store plugin so the app
/// survives restarts without re-fetching everything.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    /// Which repos to watch, keyed by account_id
    pub watched_repos: HashMap<String, Vec<RepoRef>>,
    /// Polling interval in seconds (default 120)
    pub poll_interval_secs: u64,
    /// Whether macOS notifications are enabled
    pub notifications_enabled: bool,
}

impl AppPreferences {
    pub fn default_interval() -> u64 {
        120
    }
}

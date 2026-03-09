mod accounts;
mod commands;
mod error;
mod github;
mod store;
mod sync;
mod types;

use std::path::PathBuf;
use std::sync::Arc;

use accounts::{AccountManager, KeychainStore};
use commands::{
    accounts as acc_cmd, pulls as pr_cmd, repos as repo_cmd,
    sync as sync_cmd,
};
use store::PreferencesStore;

/// Returns the path to the preferences JSON file under `~/.gh-status/`.
fn preferences_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".gh-status")
        .join("preferences.json")
}

pub fn run() {
    let manager = Arc::new(AccountManager::new(Box::new(KeychainStore::new())));
    let prefs_store = Arc::new(PreferencesStore::new(preferences_path()));

    tauri::Builder::default()
        .manage(manager)
        .manage(prefs_store)
        .invoke_handler(tauri::generate_handler![
            acc_cmd::list_accounts,
            acc_cmd::add_account,
            acc_cmd::remove_account,
            pr_cmd::list_pull_requests,
            repo_cmd::list_watched_repos,
            repo_cmd::add_watched_repo,
            repo_cmd::remove_watched_repo,
            sync_cmd::trigger_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

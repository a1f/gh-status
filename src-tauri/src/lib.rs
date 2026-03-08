mod accounts;
mod commands;
mod error;
mod github;
mod store;
mod sync;
mod types;

use std::sync::Arc;

use accounts::{AccountManager, KeychainStore};
use commands::{accounts as acc_cmd, pulls as pr_cmd, sync as sync_cmd};

pub fn run() {
    let manager = Arc::new(AccountManager::new(Box::new(KeychainStore::new())));

    tauri::Builder::default()
        .manage(manager)
        .invoke_handler(tauri::generate_handler![
            acc_cmd::list_accounts,
            acc_cmd::add_account,
            acc_cmd::remove_account,
            pr_cmd::list_pull_requests,
            sync_cmd::trigger_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod accounts;
mod commands;
mod error;
mod github;
mod store;
mod sync;
mod types;

use commands::{accounts as acc_cmd, pulls as pr_cmd, sync as sync_cmd};

pub fn run() {
    tauri::Builder::default()
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

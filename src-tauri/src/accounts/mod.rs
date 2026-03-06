pub mod keyring;

use crate::error::AppError;
use crate::types::Account;
use std::sync::RwLock;

/// Manages GitHub accounts and their lifecycle.
/// Account metadata (username, avatar, orgs) is held in memory;
/// tokens are stored/retrieved from macOS Keychain on demand.
pub struct AccountManager {
    accounts: RwLock<Vec<Account>>,
}

impl AccountManager {
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(Vec::new()),
        }
    }

    pub fn list_accounts(&self) -> Vec<Account> {
        self.accounts.read().unwrap().clone()
    }

    /// Validates the token against GitHub API, fetches profile info,
    /// stores token in Keychain, and adds account to the in-memory list.
    pub async fn add_account(&self, _id: &str, _token: &str) -> Result<Account, AppError> {
        // TODO: implement in M1.2 — validate token, fetch viewer, store in keychain
        Err(AppError::Auth("not yet implemented".into()))
    }

    pub async fn remove_account(&self, _id: &str) -> Result<(), AppError> {
        // TODO: implement in M1.2 — remove from keychain + memory
        Err(AppError::Auth("not yet implemented".into()))
    }
}

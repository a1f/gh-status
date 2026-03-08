use std::sync::Arc;

use tauri::State;

use crate::accounts::AccountManager;
use crate::error::AppError;
use crate::types::Account;

#[tauri::command]
pub async fn list_accounts(
    manager: State<'_, Arc<AccountManager>>,
) -> Result<Vec<Account>, AppError> {
    Ok(manager.list_accounts())
}

#[tauri::command]
pub async fn add_account(
    token: String,
    manager: State<'_, Arc<AccountManager>>,
) -> Result<Account, AppError> {
    manager.add_account(&token).await
}

#[tauri::command]
pub async fn remove_account(
    id: String,
    manager: State<'_, Arc<AccountManager>>,
) -> Result<(), AppError> {
    manager.remove_account(&id).await
}

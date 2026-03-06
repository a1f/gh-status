use crate::error::AppError;
use crate::types::Account;

/// Thin command handlers for account management.
/// All business logic lives in AccountManager — these just
/// bridge the Tauri IPC boundary.

#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, AppError> {
    // TODO: wire to AppState in M1.2
    Ok(Vec::new())
}

#[tauri::command]
pub async fn add_account(_id: String, _token: String) -> Result<Account, AppError> {
    Err(AppError::Auth("not yet implemented".into()))
}

#[tauri::command]
pub async fn remove_account(_id: String) -> Result<(), AppError> {
    Err(AppError::Auth("not yet implemented".into()))
}

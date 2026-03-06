use crate::error::AppError;

#[tauri::command]
pub async fn trigger_sync(_account_id: Option<String>) -> Result<(), AppError> {
    // TODO: wire to SyncEngine in M1.4
    Ok(())
}

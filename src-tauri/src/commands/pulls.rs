use crate::error::AppError;
use crate::types::PullRequest;

#[tauri::command]
pub async fn list_pull_requests(
    _account_id: Option<String>,
) -> Result<Vec<PullRequest>, AppError> {
    // TODO: wire to SyncEngine cache in M1.4
    Ok(Vec::new())
}

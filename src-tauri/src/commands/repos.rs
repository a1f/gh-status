use std::sync::Arc;

use tauri::State;

use crate::error::AppError;
use crate::store::PreferencesStore;
use crate::types::RepoRef;

#[tauri::command]
pub async fn list_watched_repos(
    account_id: String,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<Vec<RepoRef>, AppError> {
    Ok(store.get_watched_repos(&account_id))
}

#[tauri::command]
pub async fn add_watched_repo(
    account_id: String,
    owner: String,
    name: String,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<(), AppError> {
    store.add_watched_repo(&account_id, RepoRef { owner, name })
}

#[tauri::command]
pub async fn remove_watched_repo(
    account_id: String,
    owner: String,
    name: String,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<(), AppError> {
    store.remove_watched_repo(&account_id, &RepoRef { owner, name })
}

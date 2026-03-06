use crate::error::AppError;

const SERVICE_NAME: &str = "com.gh-status.github-pat";

/// Thin wrapper around the macOS Keychain for storing GitHub PATs.
/// Each account gets its own keyring entry keyed by account_id.
pub struct KeychainStore;

impl KeychainStore {
    pub fn store_token(_account_id: &str, _token: &str) -> Result<(), AppError> {
        // TODO: implement with keyring crate in M1.2
        Err(AppError::Keychain("not yet implemented".into()))
    }

    pub fn get_token(_account_id: &str) -> Result<String, AppError> {
        Err(AppError::Keychain("not yet implemented".into()))
    }

    pub fn delete_token(_account_id: &str) -> Result<(), AppError> {
        Err(AppError::Keychain("not yet implemented".into()))
    }

    /// Returns the service name used for all keychain entries,
    /// allowing tests to verify the correct namespace.
    pub fn service_name() -> &'static str {
        SERVICE_NAME
    }
}

use serde::Serialize;

/// Unified error type for all gh-status backend operations.
/// Serializable so Tauri can send structured errors to the frontend
/// instead of opaque strings.
#[derive(Debug, thiserror::Error, Serialize)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Blanket conversion so we can use `?` with any std::error::Error
/// inside command handlers without manual mapping everywhere.
impl From<Box<dyn std::error::Error>> for AppError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}

use serde::{Deserialize, Serialize};

/// Reference to a GitHub repository, used throughout the app
/// to identify which repo a PR belongs to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

impl std::fmt::Display for RepoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewStatus {
    Approved,
    ChangesRequested,
    Pending,
    ReviewRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CiStatus {
    Success,
    Failure,
    Pending,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Mergeable {
    Clean,
    Conflicting,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    pub color: String,
}

/// Core PR representation used across the entire app.
/// Fetched from GitHub GraphQL, cached in memory by the sync engine,
/// and sent to the frontend via Tauri commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub is_draft: bool,
    pub author: Author,
    pub repo: RepoRef,
    /// Which account fetched this PR — needed for multi-account support
    pub account_id: String,
    pub base_ref: String,
    pub head_ref: String,
    pub url: String,
    pub created_at: String,
    pub updated_at: String,
    pub mergeable: Mergeable,
    pub review_status: ReviewStatus,
    pub review_requests: Vec<String>,
    pub ci_status: CiStatus,
    pub comment_count: u32,
    pub labels: Vec<Label>,
    pub last_comment_at: Option<String>,
}

/// Stored account metadata — tokens are never held here,
/// they live exclusively in the macOS Keychain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub username: String,
    pub avatar_url: String,
    pub orgs: Vec<String>,
}

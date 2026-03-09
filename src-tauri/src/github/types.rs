//! GitHub GraphQL API response DTOs.
//!
//! These structs mirror the exact shape of the GraphQL response
//! and are deserialized via serde. They are then converted into
//! domain types (`crate::types::PullRequest`) by `graphql.rs`.
//! Kept separate from domain types because API shapes change
//! independently of our internal model.

use serde::Deserialize;

/// A single PR node from `pullRequests(first: 50, states: [OPEN]) { nodes { ... } }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlPullRequestNode {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: Option<GqlAuthor>,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub url: String,
    pub created_at: String,
    pub updated_at: String,
    pub mergeable: String,
    pub review_decision: Option<String>,
    pub review_requests: GqlReviewRequestConnection,
    pub commits: GqlCommitConnection,
    pub comments: GqlTotalCountConnection,
    pub labels: GqlLabelConnection,
    pub latest_comments: GqlLatestCommentsConnection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlAuthor {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlReviewRequestConnection {
    pub nodes: Vec<GqlReviewRequestNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlReviewRequestNode {
    pub requested_reviewer: Option<GqlRequestedReviewer>,
}

/// Can be a User (has login) or Team (has name but no login).
/// We only extract login when present.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlRequestedReviewer {
    pub login: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlStatusCheckRollup {
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlTotalCountConnection {
    pub total_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlLabelConnection {
    pub nodes: Vec<GqlLabelNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlLabelNode {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlCommitConnection {
    pub nodes: Vec<GqlCommitNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlCommitNode {
    pub commit: GqlCommit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlCommit {
    pub status_check_rollup: Option<GqlStatusCheckRollup>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlLatestCommentsConnection {
    pub nodes: Vec<GqlLatestComment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GqlLatestComment {
    pub created_at: String,
}

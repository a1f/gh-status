use crate::types::PullRequest;
use serde::Serialize;
use std::collections::HashMap;

/// What specifically changed about a PR between two sync cycles.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    NewComments(u32),
    ReviewStatusChanged,
    CiStatusChanged,
    TitleChanged,
    Merged,
    Closed,
    DraftStatusChanged,
    NewReviewRequest,
}

/// Represents a detected change from one sync cycle to the next.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PrChange {
    Added { pr: PullRequest },
    Removed { pr: PullRequest },
    Updated {
        before: Box<PullRequest>,
        after: Box<PullRequest>,
        changes: Vec<ChangeKind>,
    },
}

/// Compares previous PR state with fresh data to detect what changed.
/// Returns a list of changes the sync engine uses to emit targeted events
/// and trigger notifications only for meaningful updates.
pub fn diff_pr_states(
    _old: &HashMap<String, PullRequest>,
    _new: &[PullRequest],
) -> Vec<PrChange> {
    // TODO: implement in M1.4
    Vec::new()
}

#[cfg(test)]
mod tests {
    // TODO: add differ tests in M1.4
}

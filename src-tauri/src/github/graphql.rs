//! GraphQL query construction and response parsing.
//!
//! Builds batched queries using aliases so a single API call
//! can fetch PRs from up to 10 repos at once:
//!
//!   `query { repo_0: repository(...) { pullRequests { ... } } repo_1: ... }`
//!
//! Responses are parsed via `serde_json::Value` since alias keys
//! are dynamic (`repo_0`, `repo_1`, etc.) and can't be modeled
//! as fixed Rust struct fields. Individual PR nodes are then
//! deserialized into typed DTOs and converted to domain types.

use crate::error::AppError;
use crate::github::types::{
    GqlAuthor, GqlPullRequestNode, GqlReviewRequestConnection, GqlStatusCheckRollup,
};
use crate::types::{
    Author, CiStatus, Label, Mergeable, PrState, PullRequest, RepoRef,
    ReviewStatus,
};
use std::fmt::Write;

/// Maximum repos per GraphQL query. GitHub's complexity limit
/// makes batching more than 10 repos risky.
pub const MAX_REPOS_PER_QUERY: usize = 10;

/// GraphQL field selection used inside each aliased repository block.
const PR_FRAGMENT: &str = r#"pullRequests(first: 50, states: [OPEN]) {
      nodes {
        id
        number
        title
        state
        isDraft
        author { login avatarUrl }
        baseRefName
        headRefName
        url
        createdAt
        updatedAt
        mergeable
        reviewDecision
        reviewRequests(first: 20) {
          nodes { requestedReviewer { ... on User { login } ... on Team { name } } }
        }
        commits(last: 1) {
          nodes { commit { statusCheckRollup { state } } }
        }
        comments { totalCount }
        labels(first: 20) { nodes { name color } }
        latestComments: comments(last: 1) { nodes { createdAt } }
      }
    }"#;

/// Returns true if a GitHub owner or repo name contains only safe
/// characters (alphanumeric, hyphens, underscores, dots).
fn is_safe_graphql_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Constructs a single GraphQL query string with one aliased
/// `repository(...)` block per repo.
///
/// Caller must ensure `repos.len() <= MAX_REPOS_PER_QUERY`.
/// This function does not chunk -- chunking is the caller's job.
pub fn build_pr_query(repos: &[RepoRef]) -> String {
    if repos.is_empty() {
        return "{ __typename }".to_string();
    }

    let mut query = String::with_capacity(repos.len() * 600 + 20);
    query.push_str("query {\n");

    for (i, repo) in repos.iter().enumerate() {
        if !is_safe_graphql_name(&repo.owner) || !is_safe_graphql_name(&repo.name) {
            continue;
        }
        let _ = write!(
            query,
            "  repo_{i}: repository(owner: \"{owner}\", name: \"{name}\") {{\n\
             \x20\x20\x20\x20{PR_FRAGMENT}\n\
             \x20\x20}}\n",
            owner = repo.owner,
            name = repo.name,
        );
    }

    query.push('}');
    query
}

/// Splits a slice of repos into sub-slices of at most
/// `MAX_REPOS_PER_QUERY` items each.
pub fn chunk_repos(repos: &[RepoRef]) -> Vec<&[RepoRef]> {
    repos.chunks(MAX_REPOS_PER_QUERY).collect()
}

/// Walks dynamic alias keys in a GraphQL response, deserializes
/// PR nodes from each aliased repo block, and converts DTOs to
/// domain `PullRequest` values.
///
/// Returns PRs with `account_id` set to an empty string -- the
/// caller stamps the correct account_id after parsing.
pub fn parse_pr_response(
    body: &serde_json::Value,
    repos: &[RepoRef],
) -> Result<Vec<PullRequest>, AppError> {
    let data = match body.get("data") {
        Some(d) if !d.is_null() => d,
        _ => {
            return Err(extract_graphql_errors(body));
        }
    };

    let mut prs = Vec::new();

    for (i, repo) in repos.iter().enumerate() {
        let alias = format!("repo_{i}");

        let repo_data = match data.get(&alias) {
            Some(serde_json::Value::Null) | None => continue,
            Some(d) => d,
        };

        let nodes = repo_data
            .get("pullRequests")
            .and_then(|pr| pr.get("nodes"))
            .and_then(|n| n.as_array());

        let nodes = match nodes {
            Some(arr) => arr,
            None => continue,
        };

        for (node_idx, node) in nodes.iter().enumerate() {
            let pr = parse_pr_node(node.clone(), repo).map_err(|e| {
                AppError::GitHub(format!(
                    "failed to parse PR node {node_idx} in {repo}: {e}"
                ))
            })?;
            prs.push(pr);
        }
    }

    Ok(prs)
}

/// Extracts error messages from a GraphQL error response, or
/// returns a generic message if no errors array is present.
fn extract_graphql_errors(body: &serde_json::Value) -> AppError {
    if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
        let messages: Vec<&str> = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
            .collect();

        if !messages.is_empty() {
            return AppError::GitHub(messages.join("; "));
        }
    }

    AppError::GitHub("unexpected API response format".into())
}

/// Deserializes a single PR JSON node into a typed DTO, then
/// converts it to a domain `PullRequest`.
fn parse_pr_node(
    node: serde_json::Value,
    repo: &RepoRef,
) -> Result<PullRequest, AppError> {
    let dto: GqlPullRequestNode =
        serde_json::from_value(node).map_err(|e| AppError::GitHub(e.to_string()))?;

    Ok(convert_pr_node(dto, repo))
}

/// Pure mapping from DTO fields to domain `PullRequest` fields.
/// Handles all enum conversions and nested field extraction.
fn convert_pr_node(dto: GqlPullRequestNode, repo: &RepoRef) -> PullRequest {
    let ci_status = dto
        .commits
        .nodes
        .first()
        .and_then(|n| n.commit.status_check_rollup.as_ref())
        .cloned();

    let last_comment_at = dto
        .latest_comments
        .nodes
        .first()
        .map(|c| c.created_at.clone());

    PullRequest {
        id: dto.id,
        number: dto.number,
        title: dto.title,
        state: convert_pr_state(&dto.state),
        is_draft: dto.is_draft,
        author: convert_author(dto.author),
        repo: repo.clone(),
        account_id: String::new(),
        base_ref: dto.base_ref_name,
        head_ref: dto.head_ref_name,
        url: dto.url,
        created_at: dto.created_at,
        updated_at: dto.updated_at,
        mergeable: convert_mergeable(&dto.mergeable),
        review_status: convert_review_decision(dto.review_decision),
        review_requests: extract_reviewer_logins(dto.review_requests),
        ci_status: convert_ci_state(ci_status),
        comment_count: dto.comments.total_count,
        labels: dto
            .labels
            .nodes
            .into_iter()
            .map(|l| Label {
                name: l.name,
                color: l.color,
            })
            .collect(),
        last_comment_at,
    }
}

fn convert_pr_state(state: &str) -> PrState {
    match state {
        "OPEN" => PrState::Open,
        "CLOSED" => PrState::Closed,
        "MERGED" => PrState::Merged,
        _ => PrState::Open,
    }
}

fn convert_author(author: Option<GqlAuthor>) -> Author {
    match author {
        Some(a) => Author {
            login: a.login,
            avatar_url: a.avatar_url,
        },
        None => Author {
            login: "ghost".to_string(),
            avatar_url: String::new(),
        },
    }
}

fn convert_mergeable(mergeable: &str) -> Mergeable {
    match mergeable {
        "MERGEABLE" => Mergeable::Clean,
        "CONFLICTING" => Mergeable::Conflicting,
        _ => Mergeable::Unknown,
    }
}

fn convert_review_decision(decision: Option<String>) -> ReviewStatus {
    match decision.as_deref() {
        Some("APPROVED") => ReviewStatus::Approved,
        Some("CHANGES_REQUESTED") => ReviewStatus::ChangesRequested,
        Some("REVIEW_REQUIRED") => ReviewStatus::ReviewRequired,
        _ => ReviewStatus::Pending,
    }
}

fn convert_ci_state(rollup: Option<GqlStatusCheckRollup>) -> CiStatus {
    match rollup.as_ref().map(|r| r.state.as_str()) {
        None => CiStatus::None,
        Some("SUCCESS") => CiStatus::Success,
        Some("FAILURE" | "ERROR") => CiStatus::Failure,
        Some(_) => CiStatus::Pending,
    }
}

fn extract_reviewer_logins(
    connection: GqlReviewRequestConnection,
) -> Vec<String> {
    connection
        .nodes
        .into_iter()
        .filter_map(|n| n.requested_reviewer)
        .filter_map(|r| r.login)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- Helpers --

    fn make_repos(n: usize) -> Vec<RepoRef> {
        (0..n)
            .map(|i| RepoRef {
                owner: "test-org".to_string(),
                name: format!("repo-{i}"),
            })
            .collect()
    }

    fn make_pr_json(overrides: serde_json::Value) -> serde_json::Value {
        let mut base = json!({
            "id": "PR_abc123",
            "number": 42,
            "title": "Fix the thing",
            "state": "OPEN",
            "isDraft": false,
            "url": "https://github.com/test-org/repo-0/pull/42",
            "createdAt": "2026-01-10T08:00:00Z",
            "updatedAt": "2026-01-12T14:30:00Z",
            "baseRefName": "main",
            "headRefName": "fix/the-thing",
            "mergeable": "MERGEABLE",
            "reviewDecision": null,
            "author": {
                "login": "testuser",
                "avatarUrl": "https://avatars.test/testuser"
            },
            "labels": { "nodes": [] },
            "reviewRequests": { "nodes": [] },
            "comments": { "totalCount": 0 },
            "commits": {
                "nodes": [{
                    "commit": {
                        "statusCheckRollup": null
                    }
                }]
            },
            "latestComments": { "nodes": [] }
        });
        if let (Some(base_obj), Some(over_obj)) =
            (base.as_object_mut(), overrides.as_object())
        {
            for (k, v) in over_obj {
                base_obj.insert(k.clone(), v.clone());
            }
        }
        base
    }

    fn make_graphql_response(
        repo_prs: Vec<Vec<serde_json::Value>>,
    ) -> serde_json::Value {
        let mut data = serde_json::Map::new();
        for (i, prs) in repo_prs.into_iter().enumerate() {
            data.insert(
                format!("repo_{i}"),
                json!({ "pullRequests": { "nodes": prs } }),
            );
        }
        json!({ "data": data })
    }

    // -------------------------------------------------------
    // Query building tests
    // -------------------------------------------------------

    #[test]
    fn test_build_single_repo_query() {
        let repos = vec![RepoRef {
            owner: "octocat".into(),
            name: "hello-world".into(),
        }];
        let query = build_pr_query(&repos);

        assert!(
            query.contains(r#"repo_0: repository(owner: "octocat", name: "hello-world")"#),
            "expected repo_0 alias with correct owner/name, got:\n{query}"
        );
        assert!(
            !query.contains("repo_1"),
            "single-repo query should not contain repo_1"
        );
    }

    #[test]
    fn test_build_multi_repo_query() {
        let repos = vec![
            RepoRef { owner: "org-a".into(), name: "repo1".into() },
            RepoRef { owner: "org-a".into(), name: "repo2".into() },
            RepoRef { owner: "org-b".into(), name: "repo3".into() },
        ];
        let query = build_pr_query(&repos);

        assert!(query.contains("repo_0"), "missing alias repo_0");
        assert!(query.contains("repo_1"), "missing alias repo_1");
        assert!(query.contains("repo_2"), "missing alias repo_2");
        assert!(
            query.contains(r#"owner: "org-a", name: "repo1""#),
            "repo_0 should have org-a/repo1"
        );
        assert!(
            query.contains(r#"owner: "org-b", name: "repo3""#),
            "repo_2 should have org-b/repo3"
        );
    }

    #[test]
    fn test_build_chunks_at_10() {
        let repos = make_repos(15);
        let chunks = chunk_repos(&repos);

        assert_eq!(chunks.len(), 2, "15 repos should produce 2 chunks");
        assert_eq!(chunks[0].len(), 10);
        assert_eq!(chunks[1].len(), 5);

        let q1 = build_pr_query(chunks[0]);
        let q2 = build_pr_query(chunks[1]);

        assert!(q1.contains("repo_9"), "first chunk should have repo_9");
        assert!(
            !q1.contains("repo_10"),
            "first chunk should not have repo_10"
        );
        // Indices reset per chunk.
        assert!(
            q2.contains("repo_0"),
            "second chunk should restart at repo_0"
        );
        assert!(
            q2.contains("repo_4"),
            "second chunk should have repo_4"
        );
        assert!(
            !q2.contains("repo_5"),
            "second chunk should not have repo_5"
        );
    }

    #[test]
    fn test_build_empty_repos() {
        let query = build_pr_query(&[]);
        assert_eq!(query, "{ __typename }");
    }

    #[test]
    fn test_build_escapes_special_chars() {
        let repos = vec![
            RepoRef { owner: "my-org".into(), name: "repo.js".into() },
            RepoRef { owner: "x_y".into(), name: "a-b-c".into() },
        ];
        let query = build_pr_query(&repos);

        assert!(query.contains("repo.js"), "name with dot preserved");
        assert!(query.contains("my-org"), "owner with hyphen preserved");
        assert!(query.contains("x_y"), "owner with underscore preserved");
        assert!(query.contains("a-b-c"), "name with hyphens preserved");
        assert!(query.contains("repo_0"), "alias unaffected by special chars");
        assert!(query.contains("repo_1"), "alias unaffected by special chars");
    }

    #[test]
    fn test_build_query_has_required_fields() {
        let repos = make_repos(1);
        let query = build_pr_query(&repos);

        let required = [
            "number", "title", "isDraft", "state", "url",
            "createdAt", "updatedAt", "baseRefName", "headRefName",
            "mergeable", "author", "labels", "reviewRequests",
            "reviewDecision", "commits", "comments",
        ];
        for field in &required {
            assert!(
                query.contains(field),
                "query missing required field '{field}'"
            );
        }
    }

    #[test]
    fn test_build_exact_boundary_10() {
        let repos = make_repos(10);
        let chunks = chunk_repos(&repos);

        assert_eq!(chunks.len(), 1, "exactly 10 repos = 1 chunk");

        let query = build_pr_query(chunks[0]);
        assert!(query.contains("repo_0"));
        assert!(query.contains("repo_9"));
    }

    #[test]
    fn test_build_exact_boundary_11() {
        let repos = make_repos(11);
        let chunks = chunk_repos(&repos);

        assert_eq!(chunks.len(), 2, "11 repos = 2 chunks");
        assert_eq!(chunks[0].len(), 10);
        assert_eq!(chunks[1].len(), 1);
    }

    #[test]
    fn test_build_query_starts_with_query() {
        let repos = make_repos(1);
        let query = build_pr_query(&repos);

        assert!(
            query.starts_with("query {"),
            "query should start with 'query {{', got: {}",
            &query[..20.min(query.len())]
        );
        assert!(
            query.ends_with('}'),
            "query should end with '}}'"
        );
    }

    // -------------------------------------------------------
    // Response parsing tests
    // -------------------------------------------------------

    #[test]
    fn test_parse_single_repo_response() {
        let repos = make_repos(1);
        let pr1 = make_pr_json(json!({
            "number": 1,
            "title": "First PR",
            "state": "OPEN",
            "isDraft": false,
            "author": { "login": "octocat", "avatarUrl": "https://avatars.test/octocat" },
            "reviewDecision": "APPROVED",
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "SUCCESS" } } }] },
            "labels": { "nodes": [{ "name": "enhancement", "color": "a2eeef" }] },
            "reviewRequests": { "nodes": [{ "requestedReviewer": { "login": "alice" } }] },
            "comments": { "totalCount": 3 },
            "latestComments": { "nodes": [{ "createdAt": "2026-02-05T16:45:00Z" }] }
        }));
        let pr2 = make_pr_json(json!({ "number": 2, "title": "Second PR" }));
        let response = make_graphql_response(vec![vec![pr1, pr2]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result.len(), 2, "should parse 2 PRs");

        let first = &result[0];
        assert_eq!(first.number, 1);
        assert_eq!(first.title, "First PR");
        assert_eq!(first.state, PrState::Open);
        assert!(!first.is_draft);
        assert_eq!(first.author.login, "octocat");
        assert_eq!(first.author.avatar_url, "https://avatars.test/octocat");
        assert_eq!(first.repo.owner, "test-org");
        assert_eq!(first.repo.name, "repo-0");
        assert_eq!(first.review_status, ReviewStatus::Approved);
        assert_eq!(first.ci_status, CiStatus::Success);
        assert_eq!(first.labels.len(), 1);
        assert_eq!(first.labels[0].name, "enhancement");
        assert_eq!(first.review_requests, vec!["alice"]);
        assert_eq!(first.comment_count, 3);
        assert_eq!(
            first.last_comment_at.as_deref(),
            Some("2026-02-05T16:45:00Z")
        );
        // account_id is empty -- caller stamps it.
        assert!(first.account_id.is_empty());
    }

    #[test]
    fn test_parse_multi_repo_response() {
        let repos = make_repos(3);
        let response = make_graphql_response(vec![
            vec![make_pr_json(json!({})), make_pr_json(json!({}))],
            vec![make_pr_json(json!({}))],
            vec![],
        ]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result.len(), 3, "2 + 1 + 0 = 3 PRs");

        assert_eq!(result[0].repo.name, "repo-0");
        assert_eq!(result[1].repo.name, "repo-0");
        assert_eq!(result[2].repo.name, "repo-1");
    }

    #[test]
    fn test_parse_empty_pr_list() {
        let repos = make_repos(1);
        let response = make_graphql_response(vec![vec![]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_partial_success() {
        let repos = make_repos(2);
        let mut response = make_graphql_response(vec![
            vec![make_pr_json(json!({}))],
        ]);
        // Add errors array alongside valid data.
        response.as_object_mut().unwrap().insert(
            "errors".into(),
            json!([{ "message": "repo_1 not found" }]),
        );

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result.len(), 1, "should return the 1 valid PR");
    }

    #[test]
    fn test_parse_error_no_data_key() {
        let repos = make_repos(1);
        // "data" key is entirely absent -- triggers error extraction.
        let response = json!({
            "errors": [{ "message": "Bad credentials" }]
        });

        let result = parse_pr_response(&response, &repos);
        assert!(result.is_err(), "missing data key should fail");

        match result {
            Err(AppError::GitHub(msg)) => {
                assert!(
                    msg.contains("Bad credentials"),
                    "error should mention 'Bad credentials', got: {msg}"
                );
            }
            other => panic!("expected GitHub error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_null_data_returns_error() {
        let repos = make_repos(1);
        // "data" is null with errors -- should return error, not empty vec.
        let response = json!({
            "data": null,
            "errors": [{ "message": "some partial error" }]
        });

        let result = parse_pr_response(&response, &repos);
        assert!(result.is_err(), "null data with errors should fail");
        match result {
            Err(AppError::GitHub(msg)) => {
                assert!(msg.contains("some partial error"));
            }
            other => panic!("expected GitHub error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_no_data_no_errors() {
        let repos = make_repos(1);
        let response = json!({ "unexpected": "shape" });

        let result = parse_pr_response(&response, &repos);
        assert!(result.is_err());

        match result {
            Err(AppError::GitHub(msg)) => {
                assert!(
                    msg.contains("unexpected"),
                    "error should mention unexpected format, got: {msg}"
                );
            }
            other => panic!("expected GitHub error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_null_optional_fields() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "reviewDecision": null,
            "labels": { "nodes": [] },
            "reviewRequests": { "nodes": [] },
            "latestComments": { "nodes": [] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        let pr = &result[0];
        assert_eq!(pr.last_comment_at, None);
        assert!(pr.labels.is_empty());
        assert!(pr.review_requests.is_empty());
        assert_eq!(pr.review_status, ReviewStatus::Pending);
    }

    #[test]
    fn test_parse_all_pr_states() {
        let repos = make_repos(1);
        let prs = vec![
            make_pr_json(json!({ "state": "OPEN" })),
            make_pr_json(json!({ "state": "CLOSED" })),
            make_pr_json(json!({ "state": "MERGED" })),
        ];
        let response = make_graphql_response(vec![prs]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].state, PrState::Open);
        assert_eq!(result[1].state, PrState::Closed);
        assert_eq!(result[2].state, PrState::Merged);
    }

    #[test]
    fn test_parse_review_statuses() {
        let repos = make_repos(1);
        let prs = vec![
            make_pr_json(json!({ "reviewDecision": "APPROVED" })),
            make_pr_json(json!({ "reviewDecision": "CHANGES_REQUESTED" })),
            make_pr_json(json!({ "reviewDecision": "REVIEW_REQUIRED" })),
            make_pr_json(json!({ "reviewDecision": null })),
        ];
        let response = make_graphql_response(vec![prs]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].review_status, ReviewStatus::Approved);
        assert_eq!(result[1].review_status, ReviewStatus::ChangesRequested);
        assert_eq!(result[2].review_status, ReviewStatus::ReviewRequired);
        assert_eq!(result[3].review_status, ReviewStatus::Pending);
    }

    #[test]
    fn test_parse_ci_status_success() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "SUCCESS" } } }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].ci_status, CiStatus::Success);
    }

    #[test]
    fn test_parse_ci_status_failure() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "FAILURE" } } }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].ci_status, CiStatus::Failure);
    }

    #[test]
    fn test_parse_ci_status_pending() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "PENDING" } } }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].ci_status, CiStatus::Pending);
    }

    #[test]
    fn test_parse_ci_status_none() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": null } }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].ci_status, CiStatus::None);
    }

    #[test]
    fn test_parse_draft_pr() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({ "isDraft": true }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert!(result[0].is_draft);
    }

    #[test]
    fn test_parse_non_draft_pr() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({ "isDraft": false }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert!(!result[0].is_draft);
    }

    #[test]
    fn test_parse_labels() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "labels": { "nodes": [
                { "name": "bug", "color": "d73a4a" },
                { "name": "help wanted", "color": "0075ca" }
            ]}
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].labels.len(), 2);
        assert_eq!(result[0].labels[0].name, "bug");
        assert_eq!(result[0].labels[0].color, "d73a4a");
        assert_eq!(result[0].labels[1].name, "help wanted");
        assert_eq!(result[0].labels[1].color, "0075ca");
    }

    #[test]
    fn test_parse_review_requests() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "reviewRequests": { "nodes": [
                { "requestedReviewer": { "login": "alice" } },
                { "requestedReviewer": { "login": "bob" } }
            ]}
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].review_requests, vec!["alice", "bob"]);
    }

    #[test]
    fn test_parse_mergeable_variants() {
        let repos = make_repos(1);
        let prs = vec![
            make_pr_json(json!({ "mergeable": "MERGEABLE" })),
            make_pr_json(json!({ "mergeable": "CONFLICTING" })),
            make_pr_json(json!({ "mergeable": "UNKNOWN" })),
        ];
        let response = make_graphql_response(vec![prs]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].mergeable, Mergeable::Clean);
        assert_eq!(result[1].mergeable, Mergeable::Conflicting);
        assert_eq!(result[2].mergeable, Mergeable::Unknown);
    }

    #[test]
    fn test_parse_comment_count() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({ "comments": { "totalCount": 7 } }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].comment_count, 7);
    }

    #[test]
    fn test_parse_last_comment_at_present() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "latestComments": { "nodes": [{ "createdAt": "2026-01-15T10:30:00Z" }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(
            result[0].last_comment_at.as_deref(),
            Some("2026-01-15T10:30:00Z")
        );
    }

    #[test]
    fn test_parse_author_fields() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "author": {
                "login": "octocat",
                "avatarUrl": "https://avatars.githubusercontent.com/u/583231"
            }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].author.login, "octocat");
        assert_eq!(
            result[0].author.avatar_url,
            "https://avatars.githubusercontent.com/u/583231"
        );
    }

    #[test]
    fn test_parse_base_and_head_refs() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "baseRefName": "main",
            "headRefName": "feature/cool-thing"
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].base_ref, "main");
        assert_eq!(result[0].head_ref, "feature/cool-thing");
    }

    #[test]
    fn test_parse_ghost_author() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({ "author": null }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].author.login, "ghost");
        assert!(result[0].author.avatar_url.is_empty());
    }

    #[test]
    fn test_parse_ci_error_maps_to_failure() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "ERROR" } } }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].ci_status, CiStatus::Failure);
    }

    #[test]
    fn test_parse_ci_expected_maps_to_pending() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "EXPECTED" } } }] }
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result[0].ci_status, CiStatus::Pending);
    }

    #[test]
    fn test_parse_missing_repo_alias_skipped() {
        let repos = make_repos(3);
        // Only provide data for repo_0 and repo_2, skip repo_1.
        let response = json!({
            "data": {
                "repo_0": { "pullRequests": { "nodes": [make_pr_json(json!({}))] } },
                "repo_2": { "pullRequests": { "nodes": [make_pr_json(json!({}))] } }
            }
        });

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].repo.name, "repo-0");
        assert_eq!(result[1].repo.name, "repo-2");
    }

    #[test]
    fn test_parse_null_repo_alias_skipped() {
        let repos = make_repos(2);
        // repo_1 is explicitly null (token lacks access).
        let response = json!({
            "data": {
                "repo_0": { "pullRequests": { "nodes": [make_pr_json(json!({}))] } },
                "repo_1": null
            }
        });

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_review_request_team_without_login_filtered() {
        let repos = make_repos(1);
        let pr = make_pr_json(json!({
            "reviewRequests": { "nodes": [
                { "requestedReviewer": { "login": "alice" } },
                { "requestedReviewer": { "login": null } },
                { "requestedReviewer": null }
            ]}
        }));
        let response = make_graphql_response(vec![vec![pr]]);

        let result = parse_pr_response(&response, &repos).unwrap();
        assert_eq!(
            result[0].review_requests,
            vec!["alice"],
            "null-login and null-reviewer entries should be filtered out"
        );
    }

    #[test]
    fn test_build_skips_unsafe_repo_names() {
        let repos = vec![
            RepoRef {
                owner: "good-org".to_string(),
                name: "good-repo".to_string(),
            },
            RepoRef {
                owner: "evil\"org".to_string(),
                name: "repo".to_string(),
            },
            RepoRef {
                owner: "org".to_string(),
                name: "repo with spaces".to_string(),
            },
        ];
        let query = build_pr_query(&repos);
        assert!(query.contains("repo_0:"), "safe repo should be included");
        assert!(
            !query.contains("evil"),
            "repo with quotes should be skipped"
        );
        assert!(
            !query.contains("spaces"),
            "repo with spaces should be skipped"
        );
    }

    #[test]
    fn test_parse_null_data_without_errors() {
        let repos = make_repos(1);
        let response = json!({ "data": null });

        let result = parse_pr_response(&response, &repos);
        assert!(result.is_err(), "null data should fail even without errors");
    }
}

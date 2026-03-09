//! GitHub API client for fetching pull requests via GraphQL.
//!
//! Each `GitHubClient` is bound to a single account's PAT.
//! The sync engine creates one per account on demand.
//! Rate limit headers are tracked per-client so the sync engine
//! can back off when approaching limits.

use crate::error::AppError;
use crate::github::graphql;
use crate::types::{PullRequest, RepoRef};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

pub(crate) const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";
pub(crate) const USER_AGENT: &str = "gh-status";

const TIMEOUT_SECS: u64 = 30;
const CONNECT_TIMEOUT_SECS: u64 = 10;

/// Sentinel value meaning "no request has been made yet".
const RATE_LIMIT_UNKNOWN: u32 = u32::MAX;

pub struct GitHubClient {
    http: reqwest::Client,
    token: String,
    account_id: String,
    base_url: String,
    rate_limit_remaining: AtomicU32,
}

impl GitHubClient {
    pub fn new(token: String, account_id: String) -> Self {
        Self::build(token, account_id, GITHUB_GRAPHQL_URL.to_string())
    }

    #[cfg(test)]
    pub fn with_base_url(token: String, account_id: String, base_url: String) -> Self {
        Self::build(token, account_id, base_url)
    }

    fn build(token: String, account_id: String, base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            token,
            account_id,
            base_url,
            rate_limit_remaining: AtomicU32::new(RATE_LIMIT_UNKNOWN),
        }
    }

    /// Fetches all open PRs across the given repos using batched
    /// GraphQL queries. Each returned PR has `account_id` stamped.
    /// Fails fast on the first chunk error.
    pub async fn fetch_prs(
        &self,
        repos: &[RepoRef],
    ) -> Result<Vec<PullRequest>, AppError> {
        let chunks = graphql::chunk_repos(repos);
        let mut all_prs = Vec::new();

        for chunk in chunks {
            let query = graphql::build_pr_query(chunk);
            let body = self.execute_graphql(&query).await?;
            let mut prs = graphql::parse_pr_response(&body, chunk)?;
            stamp_account_id(&mut prs, &self.account_id);
            all_prs.append(&mut prs);
        }

        Ok(all_prs)
    }

    /// Returns the last observed rate limit remaining value, or
    /// `None` if no request has been made yet.
    pub fn rate_limit_remaining(&self) -> Option<u32> {
        let val = self.rate_limit_remaining.load(Ordering::Relaxed);
        if val == RATE_LIMIT_UNKNOWN {
            None
        } else {
            Some(val)
        }
    }

    /// Placeholder for Phase 5's detailed PR fetch with full
    /// comments/timeline.
    pub async fn fetch_pr_detail(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<PullRequest, AppError> {
        Err(AppError::Internal("not yet implemented".into()))
    }

    /// Sends a single GraphQL query to GitHub and returns the
    /// parsed JSON body. Updates rate limit tracking from response
    /// headers.
    async fn execute_graphql(
        &self,
        query: &str,
    ) -> Result<serde_json::Value, AppError> {
        let resp = self
            .http
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", USER_AGENT)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await?;

        if let Some(remaining) = extract_rate_limit(resp.headers()) {
            self.rate_limit_remaining
                .store(remaining, Ordering::Relaxed);
        }

        if let Some(err) = map_http_status(resp.status()) {
            return Err(err);
        }

        let body: serde_json::Value = resp.json().await?;
        Ok(body)
    }
}

fn stamp_account_id(prs: &mut [PullRequest], account_id: &str) {
    for pr in prs {
        pr.account_id = account_id.to_string();
    }
}

fn extract_rate_limit(
    headers: &reqwest::header::HeaderMap,
) -> Option<u32> {
    headers
        .get("x-ratelimit-remaining")?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

fn map_http_status(status: reqwest::StatusCode) -> Option<AppError> {
    if status.is_success() {
        return None;
    }

    Some(match status.as_u16() {
        401 => AppError::Auth("invalid or expired token".into()),
        403 => AppError::GitHub(
            "forbidden: rate limit exceeded or insufficient scopes".into(),
        ),
        _ => AppError::Network(format!(
            "GitHub API returned status {status}"
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rate_limit_valid_header() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "4999".parse().unwrap());
        assert_eq!(extract_rate_limit(&headers), Some(4999));
    }

    #[test]
    fn test_extract_rate_limit_missing_header() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(extract_rate_limit(&headers), None);
    }

    #[test]
    fn test_extract_rate_limit_non_numeric() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "not-a-number".parse().unwrap());
        assert_eq!(extract_rate_limit(&headers), None);
    }

    #[test]
    fn test_map_http_status_success() {
        assert!(map_http_status(reqwest::StatusCode::OK).is_none());
    }

    #[test]
    fn test_map_http_status_401() {
        match map_http_status(reqwest::StatusCode::UNAUTHORIZED) {
            Some(AppError::Auth(msg)) => {
                assert!(msg.contains("invalid or expired"));
            }
            other => panic!("expected Auth error, got: {other:?}"),
        }
    }

    #[test]
    fn test_map_http_status_403() {
        match map_http_status(reqwest::StatusCode::FORBIDDEN) {
            Some(AppError::GitHub(msg)) => {
                assert!(msg.contains("rate limit"));
            }
            other => panic!("expected GitHub error, got: {other:?}"),
        }
    }

    #[test]
    fn test_map_http_status_500() {
        match map_http_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR) {
            Some(AppError::Network(msg)) => {
                assert!(msg.contains("500"));
            }
            other => panic!("expected Network error, got: {other:?}"),
        }
    }

    #[test]
    fn test_map_http_status_redirect() {
        match map_http_status(reqwest::StatusCode::MOVED_PERMANENTLY) {
            Some(AppError::Network(msg)) => {
                assert!(msg.contains("301"));
            }
            other => panic!("expected Network error, got: {other:?}"),
        }
    }

    #[test]
    fn test_stamp_account_id_sets_all() {
        let repo = RepoRef {
            owner: "org".into(),
            name: "repo".into(),
        };
        let mut prs = vec![
            make_empty_pr(&repo),
            make_empty_pr(&repo),
        ];

        stamp_account_id(&mut prs, "acct-123");

        for pr in &prs {
            assert_eq!(pr.account_id, "acct-123");
        }
    }

    #[test]
    fn test_stamp_account_id_empty_slice() {
        let mut prs: Vec<PullRequest> = vec![];
        stamp_account_id(&mut prs, "acct-123");
        assert!(prs.is_empty());
    }

    #[test]
    fn test_rate_limit_remaining_before_any_request() {
        let client = GitHubClient::new(
            "token".into(),
            "acct".into(),
        );
        assert_eq!(client.rate_limit_remaining(), None);
    }

    fn make_empty_pr(repo: &RepoRef) -> PullRequest {
        use crate::types::*;
        PullRequest {
            id: String::new(),
            number: 0,
            title: String::new(),
            state: PrState::Open,
            is_draft: false,
            author: Author {
                login: String::new(),
                avatar_url: String::new(),
            },
            repo: repo.clone(),
            account_id: String::new(),
            base_ref: String::new(),
            head_ref: String::new(),
            url: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            mergeable: Mergeable::Unknown,
            review_status: ReviewStatus::Pending,
            review_requests: vec![],
            ci_status: CiStatus::None,
            comment_count: 0,
            labels: vec![],
            last_comment_at: None,
        }
    }

    // -- Wiremock-based async test helpers --

    use serde_json::json;
    use wiremock::matchers::{body_string_contains, header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
                    "commit": { "statusCheckRollup": null }
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

    /// Builds a GraphQL response JSON with aliased repo keys.
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

    fn make_client(server: &MockServer) -> GitHubClient {
        GitHubClient::with_base_url(
            "ghp_test_token".into(),
            "acct-1".into(),
            server.uri(),
        )
    }

    // -------------------------------------------------------
    // 1.1 fetch_prs -- Happy Path
    // -------------------------------------------------------

    #[tokio::test]
    async fn test_fetch_prs_success() {
        let server = MockServer::start().await;
        let response_body = make_graphql_response(vec![
            vec![make_pr_json(json!({ "number": 1, "title": "PR one" }))],
            vec![make_pr_json(json!({ "number": 2, "title": "PR two" }))],
        ]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&response_body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "4999",
                    ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(2);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].repo.name, "repo-0");
        assert_eq!(prs[1].repo.name, "repo-1");
        assert_eq!(prs[0].account_id, "acct-1");
        assert_eq!(prs[1].account_id, "acct-1");
    }

    #[tokio::test]
    async fn test_fetch_prs_multiple_chunks() {
        let server = MockServer::start().await;

        // 15 repos = 2 chunks (10 + 5). Differentiate mocks by
        // matching on a repo name unique to each chunk.
        let chunk1_body = make_graphql_response(
            (0..10)
                .map(|i| {
                    vec![make_pr_json(
                        json!({ "number": i + 1 }),
                    )]
                })
                .collect(),
        );
        let chunk2_body = make_graphql_response(
            (0..5)
                .map(|i| {
                    vec![make_pr_json(
                        json!({ "number": i + 11 }),
                    )]
                })
                .collect(),
        );

        // Chunk 1 contains repo-0 (repos 0-9).
        Mock::given(method("POST"))
            .and(body_string_contains("repo-0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&chunk1_body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "4999",
                    ),
            )
            .expect(1)
            .mount(&server)
            .await;

        // Chunk 2 contains repo-10 (repos 10-14).
        Mock::given(method("POST"))
            .and(body_string_contains("repo-10"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&chunk2_body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "4998",
                    ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(15);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(prs.len(), 15);
        for pr in &prs {
            assert_eq!(pr.account_id, "acct-1");
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_empty_repos() {
        let server = MockServer::start().await;
        // No mocks registered -- any request would panic.

        let client = make_client(&server);
        let prs = client.fetch_prs(&[]).await.unwrap();

        assert!(prs.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_prs_account_id_stamped() {
        let server = MockServer::start().await;
        let response_body = make_graphql_response(vec![
            vec![
                make_pr_json(json!({ "number": 1 })),
                make_pr_json(json!({ "number": 2 })),
            ],
            vec![make_pr_json(json!({ "number": 3 }))],
        ]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&response_body),
            )
            .mount(&server)
            .await;

        let client = GitHubClient::with_base_url(
            "token".into(),
            "user-42".into(),
            server.uri(),
        );
        let repos = make_repos(2);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(prs.len(), 3);
        for pr in &prs {
            assert_eq!(
                pr.account_id, "user-42",
                "every PR must have account_id stamped"
            );
            assert!(
                !pr.account_id.is_empty(),
                "account_id must not be empty"
            );
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_single_repo() {
        let server = MockServer::start().await;
        let response_body = make_graphql_response(vec![
            vec![make_pr_json(json!({ "number": 7 }))],
        ]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&response_body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(1);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 7);
    }

    #[tokio::test]
    async fn test_fetch_prs_exactly_10_repos() {
        let server = MockServer::start().await;
        let response_body = make_graphql_response(
            (0..10)
                .map(|i| {
                    vec![make_pr_json(json!({ "number": i + 1 }))]
                })
                .collect(),
        );

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&response_body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(10);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(prs.len(), 10);
    }

    #[tokio::test]
    async fn test_fetch_prs_exactly_11_repos() {
        let server = MockServer::start().await;
        let chunk1_body = make_graphql_response(
            (0..10)
                .map(|i| {
                    vec![make_pr_json(json!({ "number": i + 1 }))]
                })
                .collect(),
        );
        let chunk2_body = make_graphql_response(vec![
            vec![make_pr_json(json!({ "number": 11 }))],
        ]);

        // Chunk 1 has repo-0, chunk 2 has repo-10.
        Mock::given(method("POST"))
            .and(body_string_contains("repo-9"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&chunk1_body),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_string_contains("repo-10"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&chunk2_body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(11);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(prs.len(), 11);
    }

    #[tokio::test]
    async fn test_fetch_prs_repo_with_zero_prs() {
        let server = MockServer::start().await;
        let response_body =
            make_graphql_response(vec![vec![], vec![]]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&response_body),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(2);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert!(prs.is_empty());
    }

    // -------------------------------------------------------
    // 1.2 fetch_prs -- HTTP Error Mapping
    // -------------------------------------------------------

    #[tokio::test]
    async fn test_fetch_prs_http_401() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::Auth(msg)) => {
                assert!(
                    msg.contains("invalid or expired"),
                    "expected auth message, got: {msg}"
                );
            }
            other => panic!("expected Auth error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_http_403_rate_limit() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(403).append_header(
                    "x-ratelimit-remaining",
                    "0",
                ),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::GitHub(msg)) => {
                assert!(
                    msg.contains("rate limit")
                        || msg.contains("forbidden"),
                    "expected rate limit/forbidden message, got: {msg}"
                );
            }
            other => {
                panic!("expected GitHub error, got: {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_network_error() {
        // Point to an address where nothing is listening.
        let client = GitHubClient::with_base_url(
            "token".into(),
            "acct".into(),
            "http://127.0.0.1:1".into(),
        );

        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::Network(_)) => {}
            other => {
                panic!("expected Network error, got: {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_partial_chunk_failure() {
        let server = MockServer::start().await;

        // First chunk (repos 0-9) succeeds, second (repos 10-14)
        // returns 500.
        let chunk1_body = make_graphql_response(
            (0..10)
                .map(|_| {
                    vec![make_pr_json(json!({}))]
                })
                .collect(),
        );

        Mock::given(method("POST"))
            .and(body_string_contains("repo-0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&chunk1_body),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_string_contains("repo-10"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(15);
        let result = client.fetch_prs(&repos).await;

        assert!(
            result.is_err(),
            "should fail-fast on second chunk error"
        );
    }

    #[tokio::test]
    async fn test_fetch_prs_http_500() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::Network(msg)) => {
                assert!(
                    msg.contains("500"),
                    "expected status 500 in message, got: {msg}"
                );
            }
            other => {
                panic!("expected Network error, got: {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_http_502() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(502))
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::Network(msg)) => {
                assert!(
                    msg.contains("502"),
                    "expected status 502 in message, got: {msg}"
                );
            }
            other => {
                panic!("expected Network error, got: {other:?}")
            }
        }
    }

    // -------------------------------------------------------
    // 1.3 fetch_prs -- Response Parsing Errors
    // -------------------------------------------------------

    #[tokio::test]
    async fn test_fetch_prs_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("not json at all"),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        assert!(
            result.is_err(),
            "malformed JSON should produce an error"
        );
    }

    #[tokio::test]
    async fn test_fetch_prs_graphql_errors() {
        let server = MockServer::start().await;
        let body = json!({
            "errors": [{ "message": "API rate limit exceeded" }],
            "data": null
        });

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::GitHub(msg)) => {
                assert!(
                    msg.contains("rate limit"),
                    "expected rate limit in error, got: {msg}"
                );
            }
            other => {
                panic!("expected GitHub error, got: {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_empty_json_object() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({})),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let result = client.fetch_prs(&make_repos(1)).await;

        match result {
            Err(AppError::GitHub(msg)) => {
                assert!(
                    msg.contains("unexpected"),
                    "expected 'unexpected' in error, got: {msg}"
                );
            }
            other => {
                panic!("expected GitHub error, got: {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_prs_graphql_partial_errors() {
        let server = MockServer::start().await;
        // Valid data alongside errors -- parse_pr_response prefers
        // data when present.
        let mut body = make_graphql_response(vec![
            vec![make_pr_json(json!({ "number": 1 }))],
        ]);
        body.as_object_mut().unwrap().insert(
            "errors".into(),
            json!([{ "message": "partial error" }]),
        );

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let repos = make_repos(1);
        let prs = client.fetch_prs(&repos).await.unwrap();

        assert_eq!(
            prs.len(),
            1,
            "should return PRs when data is present alongside errors"
        );
    }

    // -------------------------------------------------------
    // 1.5 rate_limit_remaining
    // -------------------------------------------------------

    #[tokio::test]
    async fn test_rate_limit_from_header() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "4999",
                    ),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();

        assert_eq!(client.rate_limit_remaining(), Some(4999));
    }

    #[tokio::test]
    async fn test_rate_limit_no_header() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();

        assert_eq!(
            client.rate_limit_remaining(),
            None,
            "should stay None when header is absent"
        );
    }

    #[tokio::test]
    async fn test_rate_limit_updates_across_calls() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        // Register in order: first request gets 4999, second gets
        // 4998. Each mock is consumed after one match.
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "4999",
                    ),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "4998",
                    ),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;

        let client = make_client(&server);

        client.fetch_prs(&make_repos(1)).await.unwrap();
        assert_eq!(client.rate_limit_remaining(), Some(4999));

        client.fetch_prs(&make_repos(1)).await.unwrap();
        assert_eq!(client.rate_limit_remaining(), Some(4998));
    }

    #[tokio::test]
    async fn test_rate_limit_after_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(403).append_header(
                    "x-ratelimit-remaining",
                    "0",
                ),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        let _ = client.fetch_prs(&make_repos(1)).await;

        assert_eq!(
            client.rate_limit_remaining(),
            Some(0),
            "rate limit should be extracted even from error responses"
        );
    }

    #[tokio::test]
    async fn test_rate_limit_invalid_header() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body)
                    .append_header(
                        "x-ratelimit-remaining",
                        "abc",
                    ),
            )
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();

        assert_eq!(
            client.rate_limit_remaining(),
            None,
            "invalid header value should be treated as absent"
        );
    }

    // -------------------------------------------------------
    // 1.6 Request Construction
    // -------------------------------------------------------

    #[tokio::test]
    async fn test_client_sets_auth_header() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .and(header("Authorization", "Bearer ghp_test_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();
        // If the header doesn't match, wiremock will not match
        // the mock and the test will fail with a connection error
        // or the expect(1) assertion will fail on drop.
    }

    #[tokio::test]
    async fn test_client_sets_user_agent() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .and(header("User-Agent", "gh-status"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();
    }

    #[tokio::test]
    async fn test_client_sends_post_request() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();
    }

    #[tokio::test]
    async fn test_client_sends_json_body_with_query() {
        let server = MockServer::start().await;
        let body = make_graphql_response(vec![vec![]]);

        Mock::given(method("POST"))
            .and(body_string_contains("\"query\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&body),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server);
        client.fetch_prs(&make_repos(1)).await.unwrap();
    }

    // -------------------------------------------------------
    // 2. Integration Tests (ignored -- require real token)
    // -------------------------------------------------------

    #[tokio::test]
    #[ignore]
    async fn test_fetch_prs_real_github() {
        let token = std::env::var("GITHUB_TOKEN")
            .expect("GITHUB_TOKEN must be set for integration tests");
        let client =
            GitHubClient::new(token, "integration-test".into());

        let repos = vec![RepoRef {
            owner: "cli".into(),
            name: "cli".into(),
        }];
        let result = client.fetch_prs(&repos).await;

        assert!(
            result.is_ok(),
            "real GitHub call should succeed, got: {result:?}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_prs_invalid_token_real() {
        let client = GitHubClient::new(
            "ghp_this_is_not_valid".into(),
            "test".into(),
        );

        let repos = vec![RepoRef {
            owner: "cli".into(),
            name: "cli".into(),
        }];
        let result = client.fetch_prs(&repos).await;

        match result {
            Err(AppError::Auth(_)) => {}
            other => {
                panic!("expected Auth error, got: {other:?}")
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_rate_limit_real_github() {
        let token = std::env::var("GITHUB_TOKEN")
            .expect("GITHUB_TOKEN must be set for integration tests");
        let client =
            GitHubClient::new(token, "integration-test".into());

        let repos = vec![RepoRef {
            owner: "cli".into(),
            name: "cli".into(),
        }];
        client.fetch_prs(&repos).await.unwrap();

        let remaining = client.rate_limit_remaining();
        assert!(
            remaining.is_some(),
            "real GitHub should return rate limit header"
        );
        assert!(
            remaining.unwrap() > 0,
            "rate limit should be positive"
        );
    }
}

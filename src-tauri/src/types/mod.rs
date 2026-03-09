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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    pub color: String,
}

/// Core PR representation used across the entire app.
/// Fetched from GitHub GraphQL, cached in memory by the sync engine,
/// and sent to the frontend via Tauri commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub username: String,
    pub avatar_url: String,
    pub orgs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_pr() -> PullRequest {
        PullRequest {
            id: "PR_abc".into(),
            number: 42,
            title: "Fix bug".into(),
            state: PrState::Open,
            is_draft: true,
            author: Author {
                login: "octocat".into(),
                avatar_url: "https://avatars.test/octocat".into(),
            },
            repo: RepoRef {
                owner: "org".into(),
                name: "repo".into(),
            },
            account_id: "acct-1".into(),
            base_ref: "main".into(),
            head_ref: "fix/bug".into(),
            url: "https://github.com/org/repo/pull/42".into(),
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-02T00:00:00Z".into(),
            mergeable: Mergeable::Clean,
            review_status: ReviewStatus::Approved,
            review_requests: vec!["alice".into()],
            ci_status: CiStatus::Success,
            comment_count: 5,
            labels: vec![Label {
                name: "bug".into(),
                color: "d73a4a".into(),
            }],
            last_comment_at: Some("2024-01-02T12:00:00Z".into()),
        }
    }

    #[test]
    fn test_pullrequest_serialize_camelcase() {
        let pr = make_pr();
        let val = serde_json::to_value(&pr).unwrap();
        let obj = val.as_object().unwrap();

        let expected_keys = [
            "id", "number", "title", "state", "isDraft", "author",
            "repo", "accountId", "baseRef", "headRef", "url",
            "createdAt", "updatedAt", "mergeable", "reviewStatus",
            "reviewRequests", "ciStatus", "commentCount", "labels",
            "lastCommentAt",
        ];
        for key in &expected_keys {
            assert!(
                obj.contains_key(*key),
                "serialized JSON missing camelCase key '{key}'"
            );
        }

        // Verify snake_case keys are absent.
        let snake_keys = [
            "is_draft", "avatar_url", "base_ref", "head_ref",
            "account_id", "review_status", "ci_status",
            "comment_count", "last_comment_at", "review_requests",
        ];
        for key in &snake_keys {
            assert!(
                !obj.contains_key(*key),
                "serialized JSON should not contain snake_case key '{key}'"
            );
        }
    }

    #[test]
    fn test_pullrequest_deserialize_camelcase() {
        let json_val = json!({
            "id": "PR_xyz",
            "number": 99,
            "title": "Add feature",
            "state": "closed",
            "isDraft": false,
            "author": { "login": "bob", "avatarUrl": "https://a.test/bob" },
            "repo": { "owner": "myorg", "name": "myrepo" },
            "accountId": "acct-2",
            "baseRef": "develop",
            "headRef": "feat/thing",
            "url": "https://github.com/myorg/myrepo/pull/99",
            "createdAt": "2024-03-01T00:00:00Z",
            "updatedAt": "2024-03-02T00:00:00Z",
            "mergeable": "conflicting",
            "reviewStatus": "changesRequested",
            "reviewRequests": ["charlie"],
            "ciStatus": "failure",
            "commentCount": 3,
            "labels": [{ "name": "wip", "color": "ffffff" }],
            "lastCommentAt": "2024-03-02T10:00:00Z"
        });

        let pr: PullRequest = serde_json::from_value(json_val).unwrap();

        assert_eq!(pr.id, "PR_xyz");
        assert_eq!(pr.number, 99);
        assert_eq!(pr.state, PrState::Closed);
        assert!(!pr.is_draft);
        assert_eq!(pr.author.login, "bob");
        assert_eq!(pr.author.avatar_url, "https://a.test/bob");
        assert_eq!(pr.account_id, "acct-2");
        assert_eq!(pr.base_ref, "develop");
        assert_eq!(pr.head_ref, "feat/thing");
        assert_eq!(pr.mergeable, Mergeable::Conflicting);
        assert_eq!(pr.review_status, ReviewStatus::ChangesRequested);
        assert_eq!(pr.ci_status, CiStatus::Failure);
        assert_eq!(pr.review_requests, vec!["charlie"]);
        assert_eq!(pr.comment_count, 3);
        assert_eq!(pr.labels.len(), 1);
        assert_eq!(
            pr.last_comment_at.as_deref(),
            Some("2024-03-02T10:00:00Z")
        );
    }

    #[test]
    fn test_pullrequest_roundtrip() {
        let original = make_pr();
        let json_str = serde_json::to_string(&original).unwrap();
        let restored: PullRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_enum_serialize_values() {
        assert_eq!(serde_json::to_value(PrState::Open).unwrap(), "open");
        assert_eq!(serde_json::to_value(PrState::Closed).unwrap(), "closed");
        assert_eq!(serde_json::to_value(PrState::Merged).unwrap(), "merged");

        assert_eq!(
            serde_json::to_value(ReviewStatus::ChangesRequested).unwrap(),
            "changesRequested"
        );
        assert_eq!(
            serde_json::to_value(ReviewStatus::Approved).unwrap(),
            "approved"
        );
        assert_eq!(
            serde_json::to_value(ReviewStatus::Pending).unwrap(),
            "pending"
        );
        assert_eq!(
            serde_json::to_value(ReviewStatus::ReviewRequired).unwrap(),
            "reviewRequired"
        );

        assert_eq!(serde_json::to_value(CiStatus::None).unwrap(), "none");
        assert_eq!(
            serde_json::to_value(CiStatus::Success).unwrap(),
            "success"
        );
        assert_eq!(
            serde_json::to_value(CiStatus::Failure).unwrap(),
            "failure"
        );
        assert_eq!(
            serde_json::to_value(CiStatus::Pending).unwrap(),
            "pending"
        );

        assert_eq!(serde_json::to_value(Mergeable::Clean).unwrap(), "clean");
        assert_eq!(
            serde_json::to_value(Mergeable::Conflicting).unwrap(),
            "conflicting"
        );
        assert_eq!(
            serde_json::to_value(Mergeable::Unknown).unwrap(),
            "unknown"
        );
    }

    #[test]
    fn test_enum_deserialize_values() {
        assert_eq!(
            serde_json::from_str::<PrState>("\"open\"").unwrap(),
            PrState::Open
        );
        assert_eq!(
            serde_json::from_str::<PrState>("\"closed\"").unwrap(),
            PrState::Closed
        );
        assert_eq!(
            serde_json::from_str::<PrState>("\"merged\"").unwrap(),
            PrState::Merged
        );

        assert_eq!(
            serde_json::from_str::<ReviewStatus>("\"changesRequested\"").unwrap(),
            ReviewStatus::ChangesRequested
        );
        assert_eq!(
            serde_json::from_str::<CiStatus>("\"none\"").unwrap(),
            CiStatus::None
        );
        assert_eq!(
            serde_json::from_str::<Mergeable>("\"clean\"").unwrap(),
            Mergeable::Clean
        );
    }

    #[test]
    fn test_optional_field_null() {
        let json_val = json!({
            "id": "PR_1", "number": 1, "title": "T", "state": "open",
            "isDraft": false,
            "author": { "login": "u", "avatarUrl": "" },
            "repo": { "owner": "o", "name": "r" },
            "accountId": "", "baseRef": "main", "headRef": "b",
            "url": "", "createdAt": "", "updatedAt": "",
            "mergeable": "unknown", "reviewStatus": "pending",
            "reviewRequests": [], "ciStatus": "none",
            "commentCount": 0, "labels": [],
            "lastCommentAt": null
        });

        let pr: PullRequest = serde_json::from_value(json_val).unwrap();
        assert_eq!(pr.last_comment_at, None);
    }

    #[test]
    fn test_optional_field_present() {
        let json_val = json!({
            "id": "PR_1", "number": 1, "title": "T", "state": "open",
            "isDraft": false,
            "author": { "login": "u", "avatarUrl": "" },
            "repo": { "owner": "o", "name": "r" },
            "accountId": "", "baseRef": "main", "headRef": "b",
            "url": "", "createdAt": "", "updatedAt": "",
            "mergeable": "unknown", "reviewStatus": "pending",
            "reviewRequests": [], "ciStatus": "none",
            "commentCount": 0, "labels": [],
            "lastCommentAt": "2024-01-01T00:00:00Z"
        });

        let pr: PullRequest = serde_json::from_value(json_val).unwrap();
        assert_eq!(
            pr.last_comment_at.as_deref(),
            Some("2024-01-01T00:00:00Z")
        );
    }

    #[test]
    fn test_empty_collections() {
        let json_val = json!({
            "id": "PR_1", "number": 1, "title": "T", "state": "open",
            "isDraft": false,
            "author": { "login": "u", "avatarUrl": "" },
            "repo": { "owner": "o", "name": "r" },
            "accountId": "", "baseRef": "main", "headRef": "b",
            "url": "", "createdAt": "", "updatedAt": "",
            "mergeable": "unknown", "reviewStatus": "pending",
            "reviewRequests": [], "ciStatus": "none",
            "commentCount": 0, "labels": [],
            "lastCommentAt": null
        });

        let pr: PullRequest = serde_json::from_value(json_val).unwrap();
        assert!(pr.labels.is_empty());
        assert!(pr.review_requests.is_empty());
    }

    #[test]
    fn test_reporef_display() {
        let r = RepoRef {
            owner: "org".into(),
            name: "repo".into(),
        };
        assert_eq!(r.to_string(), "org/repo");
    }

    #[test]
    fn test_reporef_hash_eq() {
        let a = RepoRef { owner: "org".into(), name: "repo".into() };
        let b = RepoRef { owner: "org".into(), name: "repo".into() };
        assert_eq!(a, b);

        let mut map = HashMap::new();
        map.insert(a, 1);
        assert_eq!(map.get(&b), Some(&1));
    }

    #[test]
    fn test_account_roundtrip() {
        let account = Account {
            id: "acct-1".into(),
            username: "octocat".into(),
            avatar_url: "https://avatars.test/octocat".into(),
            orgs: vec!["org-a".into(), "org-b".into()],
        };

        let json_str = serde_json::to_string(&account).unwrap();
        let restored: Account = serde_json::from_str(&json_str).unwrap();
        assert_eq!(account, restored);
    }

    #[test]
    fn test_account_camelcase() {
        let account = Account {
            id: "acct-1".into(),
            username: "octocat".into(),
            avatar_url: "https://avatars.test/octocat".into(),
            orgs: vec!["org-a".into()],
        };

        let val = serde_json::to_value(&account).unwrap();
        let obj = val.as_object().unwrap();

        assert!(obj.contains_key("avatarUrl"), "missing camelCase 'avatarUrl'");
        assert!(!obj.contains_key("avatar_url"), "should not have snake_case 'avatar_url'");
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("username"));
        assert!(obj.contains_key("orgs"));
    }
}

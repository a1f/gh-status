pub mod keyring;
pub use keyring::{KeychainStore, TokenStore};

use crate::error::AppError;
use crate::github::client::{GITHUB_GRAPHQL_URL, USER_AGENT};
use crate::types::Account;
use std::future::Future;
use std::pin::Pin;
use std::sync::RwLock;

/// Validates a GitHub PAT by calling the API and returning account metadata.
pub trait TokenValidator: Send + Sync {
    fn validate_token<'a>(
        &'a self,
        token: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Account, AppError>> + Send + 'a>>;
}

struct GitHubTokenValidator;

impl TokenValidator for GitHubTokenValidator {
    fn validate_token<'a>(
        &'a self,
        token: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Account, AppError>> + Send + 'a>> {
        Box::pin(fetch_viewer(token))
    }
}

const VIEWER_QUERY: &str =
    "{ viewer { login avatarUrl organizations(first: 100) { nodes { login } } } }";

#[derive(serde::Deserialize)]
struct GraphQLResponse {
    data: Option<ViewerData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(serde::Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(serde::Deserialize)]
struct ViewerData {
    viewer: Viewer,
}

#[derive(serde::Deserialize)]
struct Viewer {
    login: String,
    #[serde(rename = "avatarUrl")]
    avatar_url: String,
    organizations: OrgConnection,
}

#[derive(serde::Deserialize)]
struct OrgConnection {
    nodes: Vec<OrgNode>,
}

#[derive(serde::Deserialize)]
struct OrgNode {
    login: String,
}

/// Calls the GitHub GraphQL API to fetch the authenticated user's profile.
async fn fetch_viewer(token: &str) -> Result<Account, AppError> {
    if token.trim().is_empty() {
        return Err(AppError::Auth("token must not be empty".into()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let resp = client
        .post(GITHUB_GRAPHQL_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", USER_AGENT)
        .json(&serde_json::json!({ "query": VIEWER_QUERY }))
        .send()
        .await?;

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::Auth("invalid or expired token".into()));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(AppError::Auth("token lacks required scopes".into()));
    }
    if !status.is_success() {
        return Err(AppError::GitHub(format!(
            "GitHub API returned status {status}"
        )));
    }

    let body: GraphQLResponse = resp.json().await?;

    // Prefer data over errors — GitHub can return partial results with both
    let data = match body.data {
        Some(d) => d,
        None => {
            if let Some(errors) = body.errors {
                let msg = errors
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(AppError::Auth(msg));
            }
            return Err(AppError::GitHub("unexpected API response format".into()));
        }
    };
    let viewer = data.viewer;

    Ok(Account {
        id: viewer.login.clone(),
        username: viewer.login,
        avatar_url: viewer.avatar_url,
        orgs: viewer
            .organizations
            .nodes
            .into_iter()
            .map(|n| n.login)
            .collect(),
    })
}

/// Manages GitHub accounts and their lifecycle.
/// Account metadata (username, avatar, orgs) is held in memory;
/// tokens are stored/retrieved from macOS Keychain on demand.
pub struct AccountManager {
    token_store: Box<dyn TokenStore>,
    validator: Box<dyn TokenValidator>,
    accounts: RwLock<Vec<Account>>,
}

impl AccountManager {
    pub fn new(token_store: Box<dyn TokenStore>) -> Self {
        Self {
            token_store,
            validator: Box::new(GitHubTokenValidator),
            accounts: RwLock::new(Vec::new()),
        }
    }

    #[cfg(test)]
    fn with_validator(
        token_store: Box<dyn TokenStore>,
        validator: Box<dyn TokenValidator>,
    ) -> Self {
        Self {
            token_store,
            validator,
            accounts: RwLock::new(Vec::new()),
        }
    }

    pub fn list_accounts(&self) -> Vec<Account> {
        self.accounts
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Validates the token against GitHub API, fetches profile info,
    /// stores token in Keychain, and adds account to the in-memory list.
    pub async fn add_account(&self, token: &str) -> Result<Account, AppError> {
        let account = self.validator.validate_token(token).await?;

        self.token_store.store_token(&account.id, token)?;

        let mut accounts = self.accounts.write().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = accounts.iter().position(|a| a.id == account.id) {
            accounts[pos] = account.clone();
        } else {
            accounts.push(account.clone());
        }

        Ok(account)
    }

    /// Removes an account's token from Keychain and metadata from memory.
    pub async fn remove_account(&self, id: &str) -> Result<(), AppError> {
        // Suppress "not found" errors -- the desired end state is achieved.
        match self.token_store.delete_token(id) {
            Ok(()) => {}
            Err(AppError::Keychain(msg)) if msg.contains("no matching entry") => {}
            Err(e) => return Err(e),
        }

        let mut accounts = self.accounts.write().unwrap_or_else(|e| e.into_inner());
        accounts.retain(|a| a.id != id);

        Ok(())
    }

    /// Retrieves a stored token for the given account from the token store.
    pub fn get_token(&self, account_id: &str) -> Result<String, AppError> {
        self.token_store.get_token(account_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock as StdRwLock;

    // -- Mock: TokenStore backed by an in-memory HashMap --

    struct MockTokenStore {
        tokens: StdRwLock<HashMap<String, String>>,
        fail_on_store: StdRwLock<Option<String>>,
    }

    impl MockTokenStore {
        fn new() -> Self {
            Self {
                tokens: StdRwLock::new(HashMap::new()),
                fail_on_store: StdRwLock::new(None),
            }
        }

        fn with_tokens(pairs: Vec<(&str, &str)>) -> Self {
            let map: HashMap<String, String> = pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Self {
                tokens: StdRwLock::new(map),
                fail_on_store: StdRwLock::new(None),
            }
        }

        fn failing_store(msg: &str) -> Self {
            Self {
                tokens: StdRwLock::new(HashMap::new()),
                fail_on_store: StdRwLock::new(Some(msg.to_string())),
            }
        }
    }

    impl TokenStore for MockTokenStore {
        fn store_token(
            &self,
            account_id: &str,
            token: &str,
        ) -> Result<(), AppError> {
            if let Some(msg) = self.fail_on_store.read().unwrap().as_ref() {
                return Err(AppError::Keychain(msg.clone()));
            }
            self.tokens
                .write()
                .unwrap()
                .insert(account_id.to_string(), token.to_string());
            Ok(())
        }

        fn get_token(&self, account_id: &str) -> Result<String, AppError> {
            self.tokens
                .read()
                .unwrap()
                .get(account_id)
                .cloned()
                .ok_or_else(|| {
                    AppError::Keychain(
                        "no matching entry found in secure storage".into(),
                    )
                })
        }

        fn delete_token(&self, account_id: &str) -> Result<(), AppError> {
            if self.tokens.write().unwrap().remove(account_id).is_some() {
                Ok(())
            } else {
                Err(AppError::Keychain(
                    "no matching entry found in secure storage".into(),
                ))
            }
        }
    }

    // -- Mock: TokenValidator returning canned responses --

    struct MockTokenValidator {
        account: Option<Account>,
        error_msg: Option<String>,
    }

    impl MockTokenValidator {
        fn success(account: Account) -> Self {
            Self {
                account: Some(account),
                error_msg: None,
            }
        }

        fn failure(msg: &str) -> Self {
            Self {
                account: None,
                error_msg: Some(msg.to_string()),
            }
        }
    }

    impl TokenValidator for MockTokenValidator {
        fn validate_token<'a>(
            &'a self,
            token: &'a str,
        ) -> Pin<
            Box<dyn Future<Output = Result<Account, AppError>> + Send + 'a>,
        > {
            // Mirror the real validator's empty-token check so tests
            // exercise the same contract as production code.
            if token.trim().is_empty() {
                return Box::pin(async {
                    Err(AppError::Auth(
                        "token must not be empty".into(),
                    ))
                });
            }
            let result = match &self.account {
                Some(a) => Ok(a.clone()),
                None => Err(AppError::Auth(
                    self.error_msg.clone().unwrap_or_default(),
                )),
            };
            Box::pin(async move { result })
        }
    }

    /// Returns sequential accounts on each call (pops from the back).
    struct SequenceValidator {
        accounts: StdRwLock<Vec<Account>>,
    }

    impl SequenceValidator {
        fn new(accounts: Vec<Account>) -> Self {
            Self {
                accounts: StdRwLock::new(accounts),
            }
        }
    }

    impl TokenValidator for SequenceValidator {
        fn validate_token<'a>(
            &'a self,
            _token: &'a str,
        ) -> Pin<
            Box<dyn Future<Output = Result<Account, AppError>> + Send + 'a>,
        > {
            let account = self
                .accounts
                .write()
                .unwrap()
                .pop()
                .expect("SequenceValidator: no more accounts");
            Box::pin(async move { Ok(account) })
        }
    }

    // -- Helpers --

    fn make_test_account(username: &str) -> Account {
        Account {
            id: username.to_string(),
            username: username.to_string(),
            avatar_url: format!("https://avatars.test/{username}"),
            orgs: vec![],
        }
    }

    fn make_manager(validator: MockTokenValidator) -> AccountManager {
        AccountManager::with_validator(
            Box::new(MockTokenStore::new()),
            Box::new(validator),
        )
    }

    /// Builds a manager that returns different accounts on successive
    /// `add_account` calls. Accounts are consumed in the order given
    /// (internally reversed since `SequenceValidator` pops from the back).
    fn make_multi_user_manager(
        usernames: &[&str],
    ) -> AccountManager {
        let mut accounts: Vec<Account> =
            usernames.iter().map(|u| make_test_account(u)).collect();
        accounts.reverse();
        AccountManager::with_validator(
            Box::new(MockTokenStore::new()),
            Box::new(SequenceValidator::new(accounts)),
        )
    }

    // -- Tests --

    #[test]
    fn test_list_accounts_initially_empty() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("unused"),
        ));
        assert!(mgr.list_accounts().is_empty());
    }

    #[tokio::test]
    async fn test_add_account_stores_token_and_adds_to_list() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("octocat"),
        ));

        let result = mgr.add_account("ghp_valid_token").await;
        assert!(result.is_ok());

        let accounts = mgr.list_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].username, "octocat");

        let token = mgr.get_token("octocat").unwrap();
        assert_eq!(token, "ghp_valid_token");
    }

    #[tokio::test]
    async fn test_add_account_returns_correct_account_metadata() {
        let account = Account {
            id: "octocat".to_string(),
            username: "octocat".to_string(),
            avatar_url: "https://avatars.githubusercontent.com/u/1"
                .to_string(),
            orgs: vec!["github".to_string(), "rust-lang".to_string()],
        };
        let mgr = make_manager(MockTokenValidator::success(account));

        let result = mgr.add_account("ghp_xxx").await.unwrap();
        assert_eq!(result.username, "octocat");
        assert_eq!(
            result.avatar_url,
            "https://avatars.githubusercontent.com/u/1"
        );
        assert_eq!(result.orgs, vec!["github", "rust-lang"]);
    }

    #[tokio::test]
    async fn test_add_account_rejects_empty_token() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("octocat"),
        ));

        let result = mgr.add_account("").await;
        match result {
            Err(AppError::Auth(msg)) => {
                assert!(
                    msg.contains("empty"),
                    "expected message about empty token, got: {msg}"
                );
            }
            other => panic!("expected Auth error, got {:?}", other),
        }

        assert!(mgr.list_accounts().is_empty());
    }

    #[tokio::test]
    async fn test_add_account_rejects_whitespace_token() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("octocat"),
        ));

        let result = mgr.add_account("   ").await;
        match result {
            Err(AppError::Auth(msg)) => {
                assert!(
                    msg.contains("empty"),
                    "expected message about empty token, got: {msg}"
                );
            }
            other => panic!("expected Auth error, got {:?}", other),
        }

        assert!(mgr.list_accounts().is_empty());
    }

    #[tokio::test]
    async fn test_add_account_validation_failure_no_side_effects() {
        let mgr =
            make_manager(MockTokenValidator::failure("bad credentials"));

        let result = mgr.add_account("ghp_invalid").await;
        match result {
            Err(AppError::Auth(msg)) => {
                assert!(
                    msg.contains("bad credentials"),
                    "expected 'bad credentials', got: {msg}"
                );
            }
            other => panic!("expected Auth error, got {:?}", other),
        }

        assert!(mgr.list_accounts().is_empty());
        assert!(mgr.get_token("anyone").is_err());
    }

    #[tokio::test]
    async fn test_add_account_updates_existing() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("octocat"),
        ));

        mgr.add_account("ghp_old_token").await.unwrap();
        mgr.add_account("ghp_new_token").await.unwrap();

        let accounts = mgr.list_accounts();
        assert_eq!(
            accounts.len(),
            1,
            "duplicate should replace, not append"
        );

        let token = mgr.get_token("octocat").unwrap();
        assert_eq!(token, "ghp_new_token");
    }

    #[tokio::test]
    async fn test_add_account_different_users() {
        let mgr = make_multi_user_manager(&["alice", "bob"]);

        mgr.add_account("ghp_alice").await.unwrap();
        mgr.add_account("ghp_bob").await.unwrap();

        let accounts = mgr.list_accounts();
        assert_eq!(accounts.len(), 2);

        let names: Vec<&str> =
            accounts.iter().map(|a| a.username.as_str()).collect();
        assert!(names.contains(&"alice"));
        assert!(names.contains(&"bob"));
    }

    #[tokio::test]
    async fn test_remove_account_deletes_and_removes() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("octocat"),
        ));

        mgr.add_account("ghp_token").await.unwrap();
        assert_eq!(mgr.list_accounts().len(), 1);

        mgr.remove_account("octocat").await.unwrap();

        assert!(mgr.list_accounts().is_empty());
        assert!(mgr.get_token("octocat").is_err());
    }

    #[tokio::test]
    async fn test_remove_account_nonexistent_succeeds() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("unused"),
        ));

        let result = mgr.remove_account("nobody").await;
        assert!(
            result.is_ok(),
            "removing non-existent account should be idempotent, \
             got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_remove_account_only_removes_target() {
        let mgr = make_multi_user_manager(&["alice", "bob"]);

        mgr.add_account("ghp_alice").await.unwrap();
        mgr.add_account("ghp_bob").await.unwrap();
        assert_eq!(mgr.list_accounts().len(), 2);

        mgr.remove_account("alice").await.unwrap();

        let accounts = mgr.list_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].username, "bob");

        assert!(mgr.get_token("bob").is_ok());
        assert!(mgr.get_token("alice").is_err());
    }

    #[test]
    fn test_get_token_delegates() {
        let store =
            MockTokenStore::with_tokens(vec![("user1", "ghp_abc")]);
        let mgr = AccountManager::with_validator(
            Box::new(store),
            Box::new(MockTokenValidator::success(
                make_test_account("unused"),
            )),
        );

        let token = mgr.get_token("user1").unwrap();
        assert_eq!(token, "ghp_abc");
    }

    #[test]
    fn test_get_token_missing_returns_error() {
        let mgr = make_manager(MockTokenValidator::success(
            make_test_account("unused"),
        ));

        match mgr.get_token("nobody") {
            Err(AppError::Keychain(msg)) => {
                assert!(
                    msg.contains("no matching entry"),
                    "expected 'no matching entry', got: {msg}"
                );
            }
            other => panic!("expected Keychain error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_add_account_token_store_failure_no_account_added() {
        let store = MockTokenStore::failing_store("disk full");
        let mgr = AccountManager::with_validator(
            Box::new(store),
            Box::new(MockTokenValidator::success(
                make_test_account("octocat"),
            )),
        );

        let result = mgr.add_account("ghp_valid").await;
        match result {
            Err(AppError::Keychain(msg)) => {
                assert!(
                    msg.contains("disk full"),
                    "expected 'disk full', got: {msg}"
                );
            }
            other => panic!("expected Keychain error, got {:?}", other),
        }

        assert!(
            mgr.list_accounts().is_empty(),
            "account should not be added when token storage fails"
        );
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let mgr = make_multi_user_manager(&["alice", "bob"]);

        // Add two accounts.
        let alice = mgr.add_account("ghp_alice_token").await.unwrap();
        assert_eq!(alice.username, "alice");

        let bob = mgr.add_account("ghp_bob_token").await.unwrap();
        assert_eq!(bob.username, "bob");

        // Both are listed.
        assert_eq!(mgr.list_accounts().len(), 2);

        // Tokens are independently retrievable.
        assert_eq!(
            mgr.get_token("alice").unwrap(),
            "ghp_alice_token"
        );
        assert_eq!(mgr.get_token("bob").unwrap(), "ghp_bob_token");

        // Remove alice.
        mgr.remove_account("alice").await.unwrap();
        assert_eq!(mgr.list_accounts().len(), 1);
        assert_eq!(mgr.list_accounts()[0].username, "bob");

        // Alice's token is gone; bob's is intact.
        assert!(mgr.get_token("alice").is_err());
        assert_eq!(mgr.get_token("bob").unwrap(), "ghp_bob_token");

        // Remove bob.
        mgr.remove_account("bob").await.unwrap();
        assert!(mgr.list_accounts().is_empty());
        assert!(mgr.get_token("bob").is_err());
    }
}

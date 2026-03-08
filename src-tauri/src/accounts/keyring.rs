use crate::error::AppError;

const SERVICE_NAME: &str = "com.gh-status.github-pat";

/// Thin wrapper around the macOS Keychain for storing GitHub PATs.
/// Each account gets its own keyring entry keyed by `account_id`.
pub struct KeychainStore {
    service_name: &'static str,
}

impl KeychainStore {
    /// Creates a new store using the default service name.
    pub fn new() -> Self {
        Self {
            service_name: SERVICE_NAME,
        }
    }

    /// Persists a GitHub PAT for the given account in the macOS Keychain.
    pub fn store_token(&self, account_id: &str, token: &str) -> Result<(), AppError> {
        if token.is_empty() {
            return Err(AppError::Keychain("token must not be empty".into()));
        }
        let entry = self.entry(account_id)?;
        entry.set_password(token).map_err(map_keyring_error)
    }

    /// Retrieves the stored PAT for the given account from the macOS Keychain.
    pub fn get_token(&self, account_id: &str) -> Result<String, AppError> {
        let entry = self.entry(account_id)?;
        entry.get_password().map_err(map_keyring_error)
    }

    /// Removes the stored PAT for the given account from the macOS Keychain.
    pub fn delete_token(&self, account_id: &str) -> Result<(), AppError> {
        let entry = self.entry(account_id)?;
        entry.delete_credential().map_err(map_keyring_error)
    }

    /// Returns the service name used for all keychain entries.
    pub fn service_name(&self) -> &str {
        self.service_name
    }

    fn entry(&self, account_id: &str) -> Result<keyring::Entry, AppError> {
        if account_id.is_empty() {
            return Err(AppError::Keychain("account_id must not be empty".into()));
        }
        keyring::Entry::new(self.service_name, account_id).map_err(map_keyring_error)
    }
}

fn map_keyring_error(err: keyring::Error) -> AppError {
    match err {
        keyring::Error::NoEntry => {
            AppError::Keychain("no matching entry found in secure storage".into())
        }
        keyring::Error::NoStorageAccess(e) => {
            AppError::Keychain(format!("keychain access denied: {e}"))
        }
        keyring::Error::PlatformFailure(e) => {
            AppError::Keychain(format!("keychain platform error: {e}"))
        }
        keyring::Error::BadEncoding(_) => {
            AppError::Keychain("stored credential has invalid encoding".into())
        }
        keyring::Error::TooLong(attr, limit) => {
            AppError::Keychain(format!("{attr} exceeds platform limit of {limit} bytes"))
        }
        keyring::Error::Invalid(attr, reason) => {
            AppError::Keychain(format!("invalid {attr}: {reason}"))
        }
        keyring::Error::Ambiguous(_) => {
            AppError::Keychain("multiple matching credentials found".into())
        }
        other => AppError::Keychain(format!("unexpected keychain error: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account_id(suffix: &str) -> String {
        format!("__test_keyring__{suffix}")
    }

    fn cleanup_token(store: &KeychainStore, account_id: &str) {
        let _ = store.delete_token(account_id);
    }

    struct CleanupGuard<'a> {
        store: &'a KeychainStore,
        account_id: String,
    }

    impl Drop for CleanupGuard<'_> {
        fn drop(&mut self) {
            cleanup_token(self.store, &self.account_id);
        }
    }

    // --- Unit tests (always run, no keychain mutations) ---

    #[test]
    fn test_service_name_is_correct() {
        let store = KeychainStore::new();
        assert_eq!(store.service_name(), "com.gh-status.github-pat");
    }

    #[test]
    fn test_empty_account_id_returns_error() {
        let store = KeychainStore::new();
        let result = store.store_token("", "ghp_xxx");
        match result {
            Err(AppError::Keychain(msg)) => {
                assert!(
                    msg.contains("empty"),
                    "expected message about empty account_id, got: {msg}"
                );
            }
            other => panic!("expected Keychain error, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_token_returns_error() {
        let store = KeychainStore::new();
        let result = store.store_token("some-account", "");
        match result {
            Err(AppError::Keychain(msg)) => {
                assert!(
                    msg.contains("empty"),
                    "expected message about empty token, got: {msg}"
                );
            }
            other => panic!("expected Keychain error, got {:?}", other),
        }
    }

    #[test]
    fn test_get_nonexistent_token_returns_keychain_error() {
        let store = KeychainStore::new();
        let result = store.get_token("nonexistent-abc123xyz");
        match result {
            Err(AppError::Keychain(_)) => {}
            other => panic!("expected Keychain error, got {:?}", other),
        }
    }

    #[test]
    fn test_delete_nonexistent_token_returns_keychain_error() {
        let store = KeychainStore::new();
        let result = store.delete_token("nonexistent-abc123xyz");
        match result {
            Err(AppError::Keychain(_)) => {}
            other => panic!("expected Keychain error, got {:?}", other),
        }
    }

    #[test]
    fn test_error_display_includes_context() {
        let err = AppError::Keychain("some context".into());
        let display = err.to_string();
        assert!(
            display.contains("Keychain error"),
            "expected 'Keychain error' in display, got: {display}"
        );
        assert!(
            display.contains("some context"),
            "expected 'some context' in display, got: {display}"
        );
    }

    // --- Integration tests (real keychain, ignored in CI) ---

    #[test]
    #[ignore]
    fn test_store_and_get_token_roundtrip() {
        let store = KeychainStore::new();
        let id = test_account_id("roundtrip");
        let _guard = CleanupGuard {
            store: &store,
            account_id: id.clone(),
        };
        cleanup_token(&store, &id);

        store.store_token(&id, "ghp_test123").unwrap();
        let retrieved = store.get_token(&id).unwrap();
        assert_eq!(retrieved, "ghp_test123");
    }

    #[test]
    #[ignore]
    fn test_store_overwrites_existing_token() {
        let store = KeychainStore::new();
        let id = test_account_id("overwrite");
        let _guard = CleanupGuard {
            store: &store,
            account_id: id.clone(),
        };
        cleanup_token(&store, &id);

        store.store_token(&id, "ghp_first").unwrap();
        store.store_token(&id, "ghp_second").unwrap();
        let retrieved = store.get_token(&id).unwrap();
        assert_eq!(retrieved, "ghp_second");
    }

    #[test]
    #[ignore]
    fn test_delete_token_removes_credential() {
        let store = KeychainStore::new();
        let id = test_account_id("delete");
        let _guard = CleanupGuard {
            store: &store,
            account_id: id.clone(),
        };
        cleanup_token(&store, &id);

        store.store_token(&id, "ghp_to_delete").unwrap();
        assert!(store.get_token(&id).is_ok());

        store.delete_token(&id).unwrap();

        match store.get_token(&id) {
            Err(AppError::Keychain(_)) => {}
            other => panic!("expected Keychain error after delete, got {:?}", other),
        }
    }

    #[test]
    #[ignore]
    fn test_delete_then_store_again() {
        let store = KeychainStore::new();
        let id = test_account_id("delete-restore");
        let _guard = CleanupGuard {
            store: &store,
            account_id: id.clone(),
        };
        cleanup_token(&store, &id);

        store.store_token(&id, "ghp_original").unwrap();
        store.delete_token(&id).unwrap();
        store.store_token(&id, "ghp_new_value").unwrap();

        let retrieved = store.get_token(&id).unwrap();
        assert_eq!(retrieved, "ghp_new_value");
    }

    #[test]
    #[ignore]
    fn test_multiple_accounts_isolated() {
        let store = KeychainStore::new();
        let id_a = test_account_id("iso-a");
        let id_b = test_account_id("iso-b");
        let _guard_a = CleanupGuard {
            store: &store,
            account_id: id_a.clone(),
        };
        let _guard_b = CleanupGuard {
            store: &store,
            account_id: id_b.clone(),
        };
        cleanup_token(&store, &id_a);
        cleanup_token(&store, &id_b);

        store.store_token(&id_a, "token_for_a").unwrap();
        store.store_token(&id_b, "token_for_b").unwrap();

        assert_eq!(store.get_token(&id_a).unwrap(), "token_for_a");
        assert_eq!(store.get_token(&id_b).unwrap(), "token_for_b");
    }

    #[test]
    #[ignore]
    fn test_special_characters_in_account_id() {
        let store = KeychainStore::new();
        let ids_and_tokens = [
            (test_account_id("user@org.com"), "ghp_at_sign"),
            (test_account_id("org/user"), "ghp_slash"),
            (test_account_id("name with spaces"), "ghp_spaces"),
            (test_account_id("hyphen-ated"), "ghp_hyphens"),
        ];

        let _guards: Vec<CleanupGuard> = ids_and_tokens
            .iter()
            .map(|(id, _)| {
                cleanup_token(&store, id);
                CleanupGuard {
                    store: &store,
                    account_id: id.clone(),
                }
            })
            .collect();

        for (id, token) in &ids_and_tokens {
            store.store_token(id, token).unwrap();
        }

        for (id, expected_token) in &ids_and_tokens {
            let retrieved = store.get_token(id).unwrap();
            assert_eq!(
                &retrieved, expected_token,
                "token mismatch for account_id '{id}'"
            );
        }
    }

    #[test]
    #[ignore]
    fn test_long_token_storage() {
        let store = KeychainStore::new();
        let id = test_account_id("long-token");
        let _guard = CleanupGuard {
            store: &store,
            account_id: id.clone(),
        };
        cleanup_token(&store, &id);

        let long_token: String = "ghp_"
            .chars()
            .chain(std::iter::repeat_n('x', 296))
            .collect();
        assert_eq!(long_token.len(), 300);

        store.store_token(&id, &long_token).unwrap();
        let retrieved = store.get_token(&id).unwrap();
        assert_eq!(retrieved, long_token);
    }

    #[test]
    #[ignore]
    fn test_unicode_in_token_value() {
        let store = KeychainStore::new();
        let id = test_account_id("unicode");
        let _guard = CleanupGuard {
            store: &store,
            account_id: id.clone(),
        };
        cleanup_token(&store, &id);

        let unicode_token = "ghp_t\u{00e9}st_\u{1f512}_\u{00fc}nic\u{00f6}de";

        store.store_token(&id, unicode_token).unwrap();
        let retrieved = store.get_token(&id).unwrap();
        assert_eq!(retrieved, unicode_token);
    }
}

//! GitHub API client wrapping octocrab.
//!
//! Each GitHubClient is bound to a single account's PAT.
//! The AccountManager creates one per account on demand.
//! Rate limit headers are tracked per-client so the sync engine
//! can back off when approaching limits.

// TODO: implement GitHubClient struct in M1.3

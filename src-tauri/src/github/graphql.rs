//! GraphQL query construction and response parsing.
//!
//! Builds batched queries using aliases so a single API call
//! can fetch PRs from up to 10 repos at once:
//!   query { repo_0: repository(...) { pullRequests { ... } } repo_1: ... }
//!
//! Responses are parsed via serde_json::Value since alias keys
//! are dynamic (repo_0, repo_1, etc.) and can't be modeled
//! as fixed Rust struct fields.

// TODO: implement query builder and parser in M1.3

//! Coordinates background polling of GitHub API for all accounts.
//!
//! Spawns one tokio task per account that runs on a configurable interval
//! (default 2 minutes). Each cycle: fetch PRs -> diff against cached state ->
//! emit Tauri events for changes -> update cache -> persist to disk.

pub mod differ;

// TODO: implement SyncEngine in M1.4

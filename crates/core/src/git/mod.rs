//! Git operations for GitSvnSync.

pub mod client;
pub mod github;

pub use client::GitClient;
pub use github::GitHubClient;

//! Git operations for GitSvnSync.

pub mod client;
pub mod github;
pub mod remote_url;

pub use client::GitClient;
pub use github::GitHubClient;
pub use remote_url::derive_git_remote_url;

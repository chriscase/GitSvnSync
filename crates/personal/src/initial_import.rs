//! Initial SVN→Git import for personal branch mode.
//!
//! Supports two modes:
//! - **Snapshot**: Export SVN HEAD as a single Git commit.
//! - **Full history**: Replay all SVN revisions as individual Git commits.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::github::GitHubClient;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::personal_config::PersonalConfig;
use gitsvnsync_core::svn::SvnClient;

use crate::commit_format::CommitFormatter;

/// Import mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// Export HEAD only — one Git commit with all current files.
    Snapshot,
    /// Replay every SVN revision as an individual Git commit.
    Full,
}

/// Handles the initial import of SVN history into Git.
pub struct InitialImport<'a> {
    pub svn_client: &'a SvnClient,
    pub git_client: &'a Arc<Mutex<GitClient>>,
    pub github_client: &'a GitHubClient,
    pub db: &'a Database,
    pub config: &'a PersonalConfig,
    pub formatter: &'a CommitFormatter,
}

impl<'a> InitialImport<'a> {
    /// Run the import.
    ///
    /// Returns the number of commits created.
    pub async fn import(&self, mode: ImportMode) -> Result<u64> {
        // Ensure the GitHub repo exists (auto-create if configured)
        self.ensure_github_repo().await?;

        match mode {
            ImportMode::Snapshot => self.import_snapshot().await,
            ImportMode::Full => self.import_full().await,
        }
    }

    /// Auto-create the GitHub repo if it doesn't exist and auto_create is enabled.
    async fn ensure_github_repo(&self) -> Result<()> {
        let repo = &self.config.github.repo;

        let exists = self
            .github_client
            .repo_exists(repo)
            .await
            .context("failed to check if GitHub repo exists")?;

        if exists {
            info!(repo, "GitHub repository already exists");
            return Ok(());
        }

        if !self.config.github.auto_create {
            anyhow::bail!(
                "GitHub repository '{}' does not exist and auto_create is disabled",
                repo
            );
        }

        // Extract repo name from "owner/name" format
        let name = repo
            .split('/')
            .nth(1)
            .context("invalid repo format, expected 'owner/repo'")?;

        info!(
            repo,
            private = self.config.github.private,
            "creating GitHub repository"
        );
        self.github_client
            .create_repo(
                name,
                self.config.github.private,
                &format!(
                    "SVN mirror managed by GitSvnSync (source: {})",
                    self.config.svn.url
                ),
            )
            .await
            .context("failed to create GitHub repository")?;

        info!(repo, "GitHub repository created successfully");
        Ok(())
    }

    /// Snapshot import: export HEAD, commit, push.
    async fn import_snapshot(&self) -> Result<u64> {
        info!("starting snapshot import");

        // Get SVN HEAD info
        let svn_info = self
            .svn_client
            .info()
            .await
            .context("failed to get SVN info")?;
        let head_rev = svn_info.latest_rev;
        info!(head_rev, "SVN HEAD revision");

        // Export HEAD to the Git working tree
        let git_client = self.git_client.lock().await;
        let repo_path = git_client.repo_path().to_path_buf();
        drop(git_client);

        self.svn_client
            .export("", head_rev, &repo_path)
            .await
            .context("failed to export SVN HEAD")?;

        // Commit
        let message = self.formatter.format_svn_to_git(
            &format!("Initial import from SVN (snapshot at r{})", head_rev),
            head_rev,
            &self.config.developer.svn_username,
            &chrono::Utc::now().to_rfc3339(),
        );

        let git_client = self.git_client.lock().await;
        let oid = git_client
            .commit(
                &message,
                &self.config.developer.name,
                &self.config.developer.email,
                &self.config.developer.name,
                &self.config.developer.email,
            )
            .context("failed to create initial commit")?;

        let sha = oid.to_string();
        info!(sha = %sha, rev = head_rev, "created snapshot commit");

        // Push
        let token = self.config.github.token.as_deref();
        git_client
            .push("origin", &self.config.github.default_branch, token)
            .context("failed to push to GitHub")?;
        drop(git_client);

        // Record in database
        self.db
            .insert_commit_map(
                head_rev,
                &sha,
                "svn_to_git",
                &self.config.developer.svn_username,
                &format!(
                    "{} <{}>",
                    self.config.developer.name, self.config.developer.email
                ),
            )
            .context("failed to record in commit_map")?;

        self.db
            .set_watermark("svn_rev", &head_rev.to_string())
            .context("failed to set SVN watermark")?;

        self.db
            .set_watermark("git_sha", &sha)
            .context("failed to set Git watermark")?;

        self.db
            .insert_audit_log(
                "import_snapshot",
                Some("svn_to_git"),
                Some(head_rev),
                Some(&sha),
                Some(&self.config.developer.svn_username),
                Some(&format!("Snapshot import from SVN r{}", head_rev)),
                true,
            )
            .ok();

        info!("snapshot import completed successfully");
        Ok(1)
    }

    /// Full history import: replay every SVN revision as a Git commit.
    async fn import_full(&self) -> Result<u64> {
        info!("starting full history import");

        // Get SVN HEAD info
        let svn_info = self
            .svn_client
            .info()
            .await
            .context("failed to get SVN info")?;
        let head_rev = svn_info.latest_rev;
        info!(head_rev, "SVN HEAD revision — will import all revisions");

        let mut count = 0u64;

        // Iterate through all revisions
        let log_entries = self
            .svn_client
            .log(1, head_rev)
            .await
            .context("failed to get SVN log")?;

        let git_client_guard = self.git_client.lock().await;
        let repo_path = git_client_guard.repo_path().to_path_buf();
        drop(git_client_guard);

        for entry in &log_entries {
            let rev = entry.revision;

            // Export this revision to the git working tree
            if let Err(e) = self.svn_client.export("", rev, &repo_path).await {
                warn!(rev, error = %e, "failed to export revision, skipping");
                continue;
            }

            let message =
                self.formatter
                    .format_svn_to_git(&entry.message, rev, &entry.author, &entry.date);

            let git_client = self.git_client.lock().await;
            match git_client.commit(
                &message,
                &self.config.developer.name,
                &self.config.developer.email,
                &self.config.developer.name,
                &self.config.developer.email,
            ) {
                Ok(oid) => {
                    let sha = oid.to_string();
                    debug!(rev, sha = %sha, "committed revision");

                    self.db
                        .insert_commit_map(
                            rev,
                            &sha,
                            "svn_to_git",
                            &entry.author,
                            &format!(
                                "{} <{}>",
                                self.config.developer.name, self.config.developer.email
                            ),
                        )
                        .ok();

                    count += 1;
                }
                Err(e) => {
                    // Empty commits (no file changes) are expected for property-only revisions
                    debug!(rev, error = %e, "commit failed (possibly empty revision)");
                }
            }
            drop(git_client);
        }

        // Push all at once
        if count > 0 {
            let git_client = self.git_client.lock().await;
            let token = self.config.github.token.as_deref();
            git_client
                .push("origin", &self.config.github.default_branch, token)
                .context("failed to push to GitHub")?;
            drop(git_client);
        }

        // Set watermarks
        if let Some(last) = log_entries.last() {
            self.db
                .set_watermark("svn_rev", &last.revision.to_string())
                .ok();
        }

        let git_client = self.git_client.lock().await;
        if let Ok(sha) = git_client.get_head_sha() {
            self.db.set_watermark("git_sha", &sha).ok();
        }
        drop(git_client);

        self.db
            .insert_audit_log(
                "import_full",
                Some("svn_to_git"),
                Some(head_rev),
                None,
                Some(&self.config.developer.svn_username),
                Some(&format!(
                    "Full history import: {} commits from {} revisions",
                    count,
                    log_entries.len()
                )),
                true,
            )
            .ok();

        info!(
            count,
            revisions = log_entries.len(),
            "full import completed"
        );
        Ok(count)
    }
}

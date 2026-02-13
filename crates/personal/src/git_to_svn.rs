//! Git-to-SVN sync engine for Personal Branch Mode.
//!
//! Replays merged pull request commits from a GitHub repository back into an
//! SVN working copy. Each PR's commits are applied in order and committed to
//! SVN with metadata trailers (Git SHA, PR number, branch) for traceability
//! and echo suppression.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{debug, error, info, instrument, warn};

use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::github::{GitHubClient, GitHubCommit, PullRequest};
use gitsvnsync_core::personal_config::PersonalConfig;
use gitsvnsync_core::svn::SvnClient;

use crate::commit_format::CommitFormatter;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of a single Git-to-SVN sync cycle.
#[derive(Debug, Clone, Default)]
pub struct GitToSvnResult {
    /// Total number of commits replayed to SVN.
    pub commits_synced: u64,
    /// Number of PRs fully processed.
    pub prs_synced: u64,
    /// Number of PRs skipped (already synced).
    pub prs_skipped: u64,
    /// Number of PRs that failed to sync.
    pub prs_failed: u64,
}

/// Syncs merged PR commits from Git back to SVN.
pub struct GitToSvnSync {
    svn: SvnClient,
    github: GitHubClient,
    db: Arc<Database>,
    formatter: CommitFormatter,
    svn_wc_path: PathBuf,
    git_repo_path: PathBuf,
    github_repo: String,
    default_branch: String,
    svn_author: String,
}

impl GitToSvnSync {
    /// Create a new `GitToSvnSync` from resolved configuration.
    ///
    /// `svn_wc_path` is the path to the local SVN working copy.
    /// `git_repo_path` is the path to the local Git repository clone.
    pub fn new(
        svn: SvnClient,
        github: GitHubClient,
        db: Arc<Database>,
        config: &PersonalConfig,
        svn_wc_path: PathBuf,
        git_repo_path: PathBuf,
    ) -> Self {
        let formatter = CommitFormatter::new(&config.commit_format);
        Self {
            svn,
            github,
            db,
            formatter,
            svn_wc_path,
            git_repo_path,
            github_repo: config.github.repo.clone(),
            default_branch: config.github.default_branch.clone(),
            svn_author: config.developer.svn_username.clone(),
        }
    }

    /// Run a full Git-to-SVN sync cycle.
    ///
    /// 1. Fetch recently merged PRs from GitHub.
    /// 2. Skip any PRs whose merge SHA is already recorded in `pr_sync_log`.
    /// 3. For each unsynced PR, replay its commits into the SVN working copy.
    /// 4. Record results in `pr_sync_log` and `commit_map`.
    ///
    /// Returns a summary of what was synced.
    #[instrument(skip(self), fields(repo = %self.github_repo))]
    pub async fn sync(&self) -> Result<GitToSvnResult> {
        info!("starting git-to-svn sync cycle");
        let mut result = GitToSvnResult::default();

        // Determine the "since" timestamp from the last completed PR sync.
        let since = self.db.get_last_pr_sync_time()
            .context("failed to query last PR sync time")?;
        let since_ref = since.as_deref();

        // Fetch recently merged PRs targeting the default branch.
        let merged_prs = self
            .github
            .get_merged_pull_requests(&self.github_repo, &self.default_branch, since_ref)
            .await
            .context("failed to fetch merged pull requests")?;

        info!(count = merged_prs.len(), "found merged pull requests");

        if merged_prs.is_empty() {
            debug!("no new merged PRs to sync");
            return Ok(result);
        }

        // Process each merged PR (oldest first for correct ordering).
        let mut prs_ordered: Vec<PullRequest> = merged_prs;
        prs_ordered.sort_by(|a, b| {
            let a_time = a.merged_at.as_deref().unwrap_or("");
            let b_time = b.merged_at.as_deref().unwrap_or("");
            a_time.cmp(b_time)
        });

        for pr in &prs_ordered {
            let merge_sha = match &pr.merge_commit_sha {
                Some(sha) => sha.clone(),
                None => {
                    warn!(pr_number = pr.number, "PR has no merge_commit_sha, skipping");
                    continue;
                }
            };

            // Check if this PR merge has already been processed.
            let already_synced = self.db.is_pr_synced(&merge_sha)
                .context("failed to check pr_sync_log")?;
            if already_synced {
                debug!(pr_number = pr.number, merge_sha = %merge_sha, "PR already synced, skipping");
                result.prs_skipped += 1;
                continue;
            }

            match self.sync_pr(pr, &merge_sha).await {
                Ok(commit_count) => {
                    result.commits_synced += commit_count;
                    result.prs_synced += 1;
                    info!(
                        pr_number = pr.number,
                        commits = commit_count,
                        "PR synced to SVN"
                    );
                }
                Err(e) => {
                    error!(pr_number = pr.number, error = %e, "failed to sync PR to SVN");
                    result.prs_failed += 1;
                }
            }
        }

        info!(
            commits = result.commits_synced,
            prs = result.prs_synced,
            skipped = result.prs_skipped,
            failed = result.prs_failed,
            "git-to-svn sync cycle complete"
        );

        Ok(result)
    }

    /// Sync a single merged PR's commits to SVN.
    ///
    /// Returns the number of commits successfully replayed.
    #[instrument(skip(self, pr), fields(pr_number = pr.number, merge_sha = %merge_sha))]
    async fn sync_pr(&self, pr: &PullRequest, merge_sha: &str) -> Result<u64> {
        let pr_branch = &pr.head.ref_name;

        // Fetch the commits belonging to this PR.
        let commits = self
            .github
            .get_pr_commits(&self.github_repo, pr.number)
            .await
            .context("failed to fetch PR commits")?;

        if commits.is_empty() {
            warn!(pr_number = pr.number, "PR has no commits, skipping");
            return Ok(0);
        }

        // Detect merge strategy for metadata.
        let merge_strategy = self.detect_merge_strategy(pr, &commits).await;

        // Record PR sync as pending.
        let sync_id = self.db.insert_pr_sync(
            pr.number as i64,
            &pr.title,
            pr_branch,
            merge_sha,
            &merge_strategy,
            commits.len() as i64,
        ).context("failed to insert pr_sync_log entry")?;

        // Filter out echo commits (ones we created during SVN-to-Git sync).
        let commits_to_replay: Vec<&GitHubCommit> = commits
            .iter()
            .filter(|c| !CommitFormatter::is_sync_marker(&c.commit.message))
            .collect();

        if commits_to_replay.is_empty() {
            info!(pr_number = pr.number, "all PR commits are echo commits, marking as synced");
            self.db.complete_pr_sync(sync_id, 0, 0)
                .context("failed to complete pr_sync_log entry")?;
            return Ok(0);
        }

        let mut synced_count: u64 = 0;
        let mut first_svn_rev: Option<i64> = None;
        let mut last_svn_rev: Option<i64> = None;

        for commit in &commits_to_replay {
            match self.replay_commit(commit, pr.number, pr_branch).await {
                Ok(svn_rev) => {
                    if first_svn_rev.is_none() {
                        first_svn_rev = Some(svn_rev);
                    }
                    last_svn_rev = Some(svn_rev);
                    synced_count += 1;

                    // Record the commit mapping.
                    let git_author = commit
                        .commit
                        .author
                        .name
                        .as_str();
                    if let Err(e) = self.db.insert_commit_map(
                        svn_rev,
                        &commit.sha,
                        "git_to_svn",
                        &self.svn_author,
                        git_author,
                    ) {
                        warn!(
                            svn_rev,
                            git_sha = %commit.sha,
                            error = %e,
                            "failed to record commit mapping (continuing)"
                        );
                    }

                    // Audit log entry.
                    if let Err(e) = self.db.insert_audit_log(
                        "git_to_svn_commit",
                        Some("git_to_svn"),
                        Some(svn_rev),
                        Some(&commit.sha),
                        Some(&self.svn_author),
                        Some(&format!(
                            "PR #{}: replayed commit {} as r{}",
                            pr.number, &commit.sha[..8.min(commit.sha.len())], svn_rev
                        )),
                    ) {
                        warn!(error = %e, "failed to insert audit log entry (continuing)");
                    }
                }
                Err(e) => {
                    error!(
                        git_sha = %commit.sha,
                        error = %e,
                        "failed to replay commit to SVN"
                    );

                    // Mark PR sync as failed.
                    let _ = self.db.fail_pr_sync(sync_id, &format!("{:#}", e));

                    // Audit failure.
                    let _ = self.db.insert_audit_log(
                        "git_to_svn_error",
                        Some("git_to_svn"),
                        None,
                        Some(&commit.sha),
                        Some(&self.svn_author),
                        Some(&format!(
                            "PR #{}: failed to replay commit {}: {}",
                            pr.number,
                            &commit.sha[..8.min(commit.sha.len())],
                            e
                        )),
                    );

                    return Err(e);
                }
            }
        }

        // Mark PR sync as completed.
        let svn_start = first_svn_rev.unwrap_or(0);
        let svn_end = last_svn_rev.unwrap_or(0);
        self.db
            .complete_pr_sync(sync_id, svn_start, svn_end)
            .context("failed to complete pr_sync_log entry")?;

        Ok(synced_count)
    }

    /// Replay a single Git commit into the SVN working copy.
    ///
    /// Steps:
    /// 1. `svn update` the working copy to HEAD.
    /// 2. Copy changed files from the Git repo into the SVN working copy.
    /// 3. Detect added/deleted files and run `svn add` / `svn rm`.
    /// 4. `svn commit` with formatted message including metadata trailers.
    ///
    /// Returns the new SVN revision number.
    #[instrument(skip(self, commit), fields(git_sha = %commit.sha))]
    async fn replay_commit(
        &self,
        commit: &GitHubCommit,
        pr_number: u64,
        pr_branch: &str,
    ) -> Result<i64> {
        // 1. Update SVN working copy to latest.
        self.svn
            .update(&self.svn_wc_path)
            .await
            .context("svn update failed")?;

        // 2. Copy files from Git repo to SVN working copy.
        self.apply_git_changes_to_svn(commit)
            .await
            .context("failed to apply git changes to SVN working copy")?;

        // 3. Detect status changes and stage them.
        let status_output = self
            .svn
            .status(&self.svn_wc_path)
            .await
            .context("svn status failed")?;

        let (added, deleted) = parse_svn_status(&status_output);

        if !added.is_empty() {
            let refs: Vec<&str> = added.iter().map(|s| s.as_str()).collect();
            self.svn
                .add(&self.svn_wc_path, &refs)
                .await
                .context("svn add failed")?;
            debug!(count = added.len(), "staged new files for svn add");
        }

        if !deleted.is_empty() {
            let refs: Vec<&str> = deleted.iter().map(|s| s.as_str()).collect();
            self.svn
                .rm(&self.svn_wc_path, &refs)
                .await
                .context("svn rm failed")?;
            debug!(count = deleted.len(), "staged deleted files for svn rm");
        }

        // If there are no changes, the commit is a no-op (e.g., merge-only commits).
        if status_output.trim().is_empty() && added.is_empty() && deleted.is_empty() {
            warn!(
                git_sha = %commit.sha,
                "no file changes detected, performing empty-diff commit for traceability"
            );
        }

        // 4. Format commit message with trailers and commit.
        let formatted_message = self.formatter.format_git_to_svn(
            &commit.commit.message,
            &commit.sha,
            pr_number,
            pr_branch,
        );

        let svn_rev = self
            .svn
            .commit(&self.svn_wc_path, &formatted_message, &self.svn_author)
            .await
            .context("svn commit failed")?;

        info!(
            svn_rev,
            git_sha = %commit.sha,
            "replayed git commit to SVN"
        );

        Ok(svn_rev)
    }

    /// Copy changed files from the local Git repository into the SVN working
    /// copy directory. This performs a simple file-level sync by walking the
    /// Git repo and mirroring file additions, modifications, and deletions.
    async fn apply_git_changes_to_svn(&self, commit: &GitHubCommit) -> Result<()> {
        // Use a simple recursive directory copy from git repo to SVN working copy,
        // skipping VCS metadata directories (.git, .svn).
        copy_tree(&self.git_repo_path, &self.svn_wc_path)
            .context("failed to copy git tree to SVN working copy")?;

        // Clean up files in SVN WC that no longer exist in Git repo.
        remove_stale_files(&self.git_repo_path, &self.svn_wc_path)
            .context("failed to clean stale files from SVN working copy")?;

        debug!(
            git_sha = %commit.sha,
            "applied git changes to SVN working copy"
        );

        Ok(())
    }

    /// Detect the merge strategy used for a PR by inspecting the merge commit.
    async fn detect_merge_strategy(
        &self,
        pr: &PullRequest,
        commits: &[GitHubCommit],
    ) -> String {
        let merge_sha = match &pr.merge_commit_sha {
            Some(sha) => sha,
            None => return "unknown".to_string(),
        };

        // Try to get the merge commit details to check parent count.
        match self.github.get_commit(&self.github_repo, merge_sha).await {
            Ok(detail) => {
                let parent_count = detail.parents.len();
                if parent_count >= 2 {
                    "merge".to_string()
                } else if commits.len() == 1 {
                    "squash".to_string()
                } else {
                    "rebase".to_string()
                }
            }
            Err(e) => {
                warn!(error = %e, "could not detect merge strategy, defaulting to unknown");
                "unknown".to_string()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// File-level helpers
// ---------------------------------------------------------------------------

/// Recursively copy files from `src` to `dst`, skipping `.git` and `.svn`
/// directories. Existing files are overwritten.
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    let entries = std::fs::read_dir(src)
        .with_context(|| format!("failed to read directory: {}", src.display()))?;

    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip VCS metadata directories.
        if name == ".git" || name == ".svn" {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&file_name);

        if src_path.is_dir() {
            if !dst_path.exists() {
                std::fs::create_dir_all(&dst_path)
                    .with_context(|| format!("failed to create directory: {}", dst_path.display()))?;
            }
            copy_tree(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .with_context(|| {
                    format!(
                        "failed to copy {} -> {}",
                        src_path.display(),
                        dst_path.display()
                    )
                })?;
        }
    }

    Ok(())
}

/// Remove files from `dst` that no longer exist in `src`, skipping `.git`
/// and `.svn` directories. Empty directories are also removed.
fn remove_stale_files(src: &Path, dst: &Path) -> Result<()> {
    let entries = match std::fs::read_dir(dst) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(e)
                .with_context(|| format!("failed to read directory: {}", dst.display()));
        }
    };

    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Never touch VCS metadata directories.
        if name == ".git" || name == ".svn" {
            continue;
        }

        let src_path = src.join(&file_name);
        let dst_path = entry.path();

        if dst_path.is_dir() {
            if !src_path.exists() {
                // Entire directory removed in git -- leave it for `svn rm` to handle.
                // We just mark it by ensuring the files don't exist, and `svn status`
                // will pick up the missing items.
                continue;
            }
            remove_stale_files(&src_path, &dst_path)?;
        } else if !src_path.exists() {
            // File exists in SVN WC but not in Git repo -- remove it so
            // `svn status` reports it as missing (which we convert to `svn rm`).
            std::fs::remove_file(&dst_path)
                .with_context(|| format!("failed to remove stale file: {}", dst_path.display()))?;
        }
    }

    Ok(())
}

/// Parse `svn status` output to identify unversioned (?) and missing (!) files.
///
/// Returns `(added, deleted)` where:
/// - `added` contains paths of unversioned files to `svn add`.
/// - `deleted` contains paths of missing files to `svn rm`.
fn parse_svn_status(output: &str) -> (Vec<String>, Vec<String>) {
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    for line in output.lines() {
        let line = line.trim_end();
        if line.len() < 2 {
            continue;
        }

        let status_char = line.chars().next().unwrap_or(' ');
        // The file path starts at column 8 in standard `svn status` output,
        // but we handle both formats by trimming leading whitespace after the
        // status character.
        let path = line[1..].trim_start();
        if path.is_empty() {
            continue;
        }

        match status_char {
            '?' => added.push(path.to_string()),
            '!' => deleted.push(path.to_string()),
            _ => {}
        }
    }

    (added, deleted)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_svn_status_added_and_deleted() {
        let output = "\
?       src/new_file.rs
M       src/modified.rs
!       src/removed.rs
?       docs/readme.md
A       src/already_added.rs
!       old/legacy.txt
";
        let (added, deleted) = parse_svn_status(output);
        assert_eq!(added, vec!["src/new_file.rs", "docs/readme.md"]);
        assert_eq!(deleted, vec!["src/removed.rs", "old/legacy.txt"]);
    }

    #[test]
    fn test_parse_svn_status_empty() {
        let (added, deleted) = parse_svn_status("");
        assert!(added.is_empty());
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_parse_svn_status_no_unversioned() {
        let output = "\
M       src/lib.rs
M       Cargo.toml
";
        let (added, deleted) = parse_svn_status(output);
        assert!(added.is_empty());
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_copy_tree_basic() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Create source structure.
        std::fs::write(src.path().join("a.txt"), "hello").unwrap();
        std::fs::create_dir(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub/b.txt"), "world").unwrap();
        std::fs::create_dir(src.path().join(".git")).unwrap();
        std::fs::write(src.path().join(".git/config"), "secret").unwrap();

        copy_tree(src.path(), dst.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.path().join("a.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(dst.path().join("sub/b.txt")).unwrap(),
            "world"
        );
        // .git should NOT be copied.
        assert!(!dst.path().join(".git").exists());
    }

    #[test]
    fn test_remove_stale_files() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        // Setup: dst has files that src does not.
        std::fs::write(dst.path().join("keep.txt"), "keep").unwrap();
        std::fs::write(dst.path().join("stale.txt"), "remove me").unwrap();
        std::fs::create_dir(dst.path().join(".svn")).unwrap();
        std::fs::write(dst.path().join(".svn/entries"), "svn data").unwrap();

        // src only has keep.txt.
        std::fs::write(src.path().join("keep.txt"), "keep").unwrap();

        remove_stale_files(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("keep.txt").exists());
        assert!(!dst.path().join("stale.txt").exists());
        // .svn must be preserved.
        assert!(dst.path().join(".svn/entries").exists());
    }

    #[test]
    fn test_git_to_svn_result_default() {
        let result = GitToSvnResult::default();
        assert_eq!(result.commits_synced, 0);
        assert_eq!(result.prs_synced, 0);
        assert_eq!(result.prs_skipped, 0);
        assert_eq!(result.prs_failed, 0);
    }
}

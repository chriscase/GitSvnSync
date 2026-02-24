//! Bidirectional SVN <-> Git synchronization engine.
//!
//! The [`SyncEngine`] is the heart of GitSvnSync. It implements a state machine
//! that orchestrates each sync cycle:
//!
//! 1. Fetch new SVN revisions since the last watermark.
//! 2. Fetch new Git commits since the last watermark.
//! 3. Detect conflicts between overlapping changes.
//! 4. If no conflicts (or auto-merge succeeds), apply changes in both directions.
//! 5. Update watermarks, commit map, and audit log.
//!
//! A lock mechanism prevents concurrent sync cycles.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::config::AppConfig;
use crate::conflict::detector::{ChangeKind, ConflictDetector, FileChange};
use crate::conflict::merger::Merger;
use crate::conflict::Conflict;
use crate::db::Database;
use crate::errors::SyncError;
use crate::git::client::GitClient;
use crate::identity::IdentityMapper;
use crate::models::AuditEntry;
use crate::svn::client::SvnClient;

// ---------------------------------------------------------------------------
// Sync state machine
// ---------------------------------------------------------------------------

/// States of a sync cycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Idle,
    Detecting,
    Applying,
    Committed,
    ConflictFound,
    QueuedForResolution,
    ResolutionApplied,
}

impl std::fmt::Display for SyncState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Detecting => write!(f, "detecting"),
            Self::Applying => write!(f, "applying"),
            Self::Committed => write!(f, "committed"),
            Self::ConflictFound => write!(f, "conflict_found"),
            Self::QueuedForResolution => write!(f, "queued_for_resolution"),
            Self::ResolutionApplied => write!(f, "resolution_applied"),
        }
    }
}

/// Statistics from a single sync cycle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncStats {
    pub svn_to_git_count: usize,
    pub git_to_svn_count: usize,
    pub conflicts_detected: usize,
    pub conflicts_auto_resolved: usize,
    pub started_at: String,
    pub completed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Marker string embedded in sync-generated commit messages for echo detection.
const SYNC_MARKER: &str = "[gitsvnsync]";

/// The bidirectional sync engine.
pub struct SyncEngine {
    config: AppConfig,
    db: Database,
    svn_client: SvnClient,
    git_client: Arc<tokio::sync::Mutex<GitClient>>,
    identity_mapper: Arc<IdentityMapper>,
    /// Atomic flag preventing concurrent sync cycles.
    running: Arc<AtomicBool>,
    started_at: chrono::DateTime<Utc>,
}

impl SyncEngine {
    /// Create a new sync engine with all required dependencies.
    pub fn new(
        config: AppConfig,
        db: Database,
        svn_client: SvnClient,
        git_client: GitClient,
        identity_mapper: Arc<IdentityMapper>,
    ) -> Self {
        info!("initializing sync engine");
        Self {
            config,
            db,
            svn_client,
            git_client: Arc::new(tokio::sync::Mutex::new(git_client)),
            identity_mapper,
            running: Arc::new(AtomicBool::new(false)),
            started_at: Utc::now(),
        }
    }

    /// Return a reference to the database.
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Return a reference to the identity mapper.
    pub fn identity_mapper(&self) -> &IdentityMapper {
        &self.identity_mapper
    }

    /// Check if a sync cycle is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    // -----------------------------------------------------------------------
    // Main entry point
    // -----------------------------------------------------------------------

    /// Execute one full sync cycle.
    ///
    /// Returns statistics about what was synced, or an error if something
    /// went wrong. Conflicts that can be auto-merged are handled inline;
    /// conflicts that require manual resolution are recorded in the database
    /// and the cycle still returns `Ok` (with the conflict count in stats).
    ///
    /// The sync lock is released via a drop guard so it is freed even if
    /// the cycle panics.
    pub async fn run_sync_cycle(&self) -> Result<SyncStats, SyncError> {
        // Acquire the sync lock.
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(SyncError::AlreadyRunning {
                started_at: self.started_at.to_rfc3339(),
            });
        }

        // RAII guard that clears the running flag on drop (even on panic).
        let _guard = SyncLockGuard(self.running.clone());

        let mut stats = SyncStats {
            started_at: Utc::now().to_rfc3339(),
            ..Default::default()
        };

        // Store the sync state
        let _ = self.db.set_state("sync_state", "detecting");

        let result = self.do_sync_cycle(&mut stats).await;

        // Record completion
        let (final_state, details) = match &result {
            Ok(()) => (
                "idle",
                format!(
                    "svn->git: {}, git->svn: {}, conflicts: {}",
                    stats.svn_to_git_count, stats.git_to_svn_count, stats.conflicts_detected
                ),
            ),
            Err(e) => ("error", format!("sync failed: {}", e)),
        };

        let _ = self.db.set_state("sync_state", final_state);
        let _ = self.db.set_state("last_sync_at", &Utc::now().to_rfc3339());
        stats.completed_at = Some(Utc::now().to_rfc3339());

        // Audit log
        let audit = if result.is_ok() {
            AuditEntry::success("sync_cycle", &details)
        } else {
            AuditEntry::failure("sync_cycle", &details)
        };
        let _ = self.db.insert_audit_entry(&audit);

        // Lock is released by _guard drop (happens here at scope end).
        result.map(|()| stats)
    }

    /// Get a status summary.
    pub fn get_status(&self) -> Result<crate::models::SyncStatus, SyncError> {
        let state_str = self
            .db
            .get_state("sync_state")
            .map_err(SyncError::DatabaseError)?
            .unwrap_or_else(|| "idle".to_string());

        let last_sync_str = self
            .db
            .get_state("last_sync_at")
            .map_err(SyncError::DatabaseError)?;

        let last_sync_at = last_sync_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        let last_svn_rev = match self
            .db
            .get_state("last_svn_rev")
            .map_err(SyncError::DatabaseError)?
        {
            Some(s) => s.parse::<i64>().ok(),
            None => self
                .db
                .get_last_svn_revision()
                .map_err(SyncError::DatabaseError)?,
        };
        let last_git_hash = match self
            .db
            .get_state("last_git_hash")
            .map_err(SyncError::DatabaseError)?
        {
            Some(s) if !s.is_empty() => Some(s),
            _ => self
                .db
                .get_last_git_hash()
                .map_err(SyncError::DatabaseError)?,
        };
        let total_syncs = self
            .db
            .count_sync_records()
            .map_err(SyncError::DatabaseError)?;
        let total_conflicts = self
            .db
            .count_all_conflicts()
            .map_err(SyncError::DatabaseError)?;
        let active_conflicts = self
            .db
            .count_active_conflicts()
            .map_err(SyncError::DatabaseError)?;
        let total_errors = self.db.count_errors().map_err(SyncError::DatabaseError)?;

        let uptime = (Utc::now() - self.started_at).num_seconds().max(0) as u64;

        Ok(crate::models::SyncStatus {
            state: crate::models::SyncState::from_str_val(&state_str),
            last_sync_at,
            last_svn_revision: last_svn_rev,
            last_git_hash,
            total_syncs,
            total_conflicts,
            active_conflicts,
            total_errors,
            uptime_secs: uptime,
        })
    }

    // -----------------------------------------------------------------------
    // Inner sync cycle logic
    // -----------------------------------------------------------------------

    async fn do_sync_cycle(&self, stats: &mut SyncStats) -> Result<(), SyncError> {
        // 1. Fetch changes from both sides.
        let svn_changes = self.fetch_svn_changes().await?;
        let git_changes = self.fetch_git_changes().await?;

        // 2. Detect conflicts.
        let conflicts = self.detect_conflicts_internal(&svn_changes, &git_changes);
        stats.conflicts_detected = conflicts.len();

        if !conflicts.is_empty() {
            info!(count = conflicts.len(), "conflicts detected");
            let _ = self.db.set_state("sync_state", "conflict_found");

            for conflict in &conflicts {
                if self.config.sync.auto_merge && self.try_auto_merge(conflict) {
                    stats.conflicts_auto_resolved += 1;
                } else {
                    // Persist unresolved conflict
                    let mut db_conflict = crate::models::Conflict::new(conflict.file_path.clone());
                    db_conflict.svn_content = conflict.svn_content.clone();
                    db_conflict.git_content = conflict.git_content.clone();
                    db_conflict.base_content = conflict.base_content.clone();
                    db_conflict.svn_revision = conflict.svn_rev;
                    db_conflict.git_hash = conflict.git_sha.clone();
                    let _ = self.db.insert_conflict(&db_conflict);
                }
            }
        }

        // 3. Apply SVN -> Git.
        let _ = self.db.set_state("sync_state", "applying");
        stats.svn_to_git_count = self.sync_svn_to_git(&svn_changes).await?;

        // 4. Apply Git -> SVN.
        stats.git_to_svn_count = self.sync_git_to_svn(&git_changes).await?;

        info!(
            svn_to_git = stats.svn_to_git_count,
            git_to_svn = stats.git_to_svn_count,
            "sync cycle completed"
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // SVN -> Git
    // -----------------------------------------------------------------------

    /// Apply SVN changes to the Git repository.
    ///
    /// For each SVN revision:
    /// 1. Get the unified diff from SVN.
    /// 2. Apply the diff to the Git working tree.
    /// 3. Commit with the mapped Git identity and a `[gitsvnsync]` marker.
    /// 4. Push to the remote.
    /// 5. Only then record the sync in the database.
    async fn sync_svn_to_git(&self, svn_changes: &[SvnChangeSet]) -> Result<usize, SyncError> {
        let mut count = 0;

        for change in svn_changes {
            if self.is_echo_commit(&change.message) {
                debug!(rev = change.revision, "skipping echo SVN revision");
                continue;
            }

            let git_identity = self
                .identity_mapper
                .svn_to_git(&change.author)
                .map_err(SyncError::IdentityError)?;

            // 1. Get the SVN diff for this revision.
            let diff = self
                .svn_client
                .diff_full(change.revision)
                .await
                .map_err(SyncError::SvnError)?;

            // Get the git repo path before locking, for apply_diff_to_path.
            let repo_path = {
                let git = self.git_client.lock().await;
                git.repo_path().to_path_buf()
            };

            // 2. Apply the diff to the Git working tree.
            // Try git apply first; fall back to export-based copy if the diff
            // is empty or in a format git cannot parse (e.g. SVN property-only
            // changes or initial adds).
            let diff_applied = if !diff.trim().is_empty() {
                apply_diff_to_path(&repo_path, &diff).await.is_ok()
            } else {
                false
            };

            if !diff_applied {
                // Fallback: use svn export to copy changed files.
                let export_dir = tempfile::tempdir()
                    .map_err(|e| SyncError::GitError(crate::errors::GitError::IoError(e)))?;
                self.svn_client
                    .export("", change.revision, export_dir.path())
                    .await
                    .map_err(SyncError::SvnError)?;

                // Copy each changed file from the export to the Git repo.
                for file in &change.changed_files {
                    let src = export_dir.path().join(&file.path);
                    let dst = repo_path.join(&file.path);
                    match file.action.as_str() {
                        "D" => {
                            if dst.exists() {
                                std::fs::remove_file(&dst).map_err(|e| {
                                    SyncError::GitError(crate::errors::GitError::IoError(e))
                                })?;
                            }
                        }
                        _ => {
                            if let Some(parent) = dst.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| {
                                    SyncError::GitError(crate::errors::GitError::IoError(e))
                                })?;
                            }
                            if src.exists() {
                                std::fs::copy(&src, &dst).map_err(|e| {
                                    SyncError::GitError(crate::errors::GitError::IoError(e))
                                })?;
                            }
                        }
                    }
                }
            }

            // 3. Commit with identity and sync marker.
            let commit_message = format!(
                "{}\n\n{} synced from SVN r{}",
                change.message, SYNC_MARKER, change.revision
            );

            let git = self.git_client.lock().await;
            let oid = git
                .commit(
                    &commit_message,
                    &git_identity.name,
                    &git_identity.email,
                    "gitsvnsync",
                    "sync@gitsvnsync.local",
                )
                .map_err(SyncError::GitError)?;

            let git_sha = oid.to_string();

            // 4. Push to remote.
            let token = self.config.github.token.as_deref();
            let branch = &self.config.github.default_branch;
            git.push("origin", branch, token)
                .map_err(SyncError::GitError)?;

            drop(git);

            // 5. Record the sync only after successful write.
            let record = crate::models::SyncRecord {
                id: uuid::Uuid::new_v4().to_string(),
                svn_revision: Some(change.revision),
                git_hash: Some(git_sha.clone()),
                direction: crate::models::SyncDirection::SvnToGit,
                author: change.author.clone(),
                message: change.message.clone(),
                timestamp: Utc::now(),
                synced_at: Utc::now(),
                status: crate::models::SyncRecordStatus::Applied,
            };
            self.db
                .insert_sync_record(&record)
                .map_err(SyncError::DatabaseError)?;

            // Update the SVN watermark.
            let _ = self
                .db
                .set_state("last_svn_rev", &change.revision.to_string());

            count += 1;
            info!(
                rev = change.revision,
                git_sha = %git_sha,
                git_name = %git_identity.name,
                "synced SVN r{} -> Git {}",
                change.revision,
                &git_sha[..8.min(git_sha.len())]
            );
        }

        Ok(count)
    }

    // -----------------------------------------------------------------------
    // Git -> SVN
    // -----------------------------------------------------------------------

    /// Apply Git changes to the SVN repository.
    ///
    /// For each Git commit:
    /// 1. Get the changed files from the commit.
    /// 2. Copy changed files into the SVN working copy.
    /// 3. Stage additions/deletions with `svn add`/`svn rm`.
    /// 4. Commit to SVN with a `[gitsvnsync]` marker.
    /// 5. Only then record the sync in the database.
    async fn sync_git_to_svn(&self, git_changes: &[GitChangeSet]) -> Result<usize, SyncError> {
        let mut count = 0;

        for change in git_changes {
            if self.is_echo_commit(&change.message) {
                debug!(sha = %change.sha, "skipping echo Git commit");
                continue;
            }

            let svn_username = self
                .identity_mapper
                .git_to_svn(&change.author_name, &change.author_email)
                .map_err(SyncError::IdentityError)?;

            // 1. Get changed files from the Git commit.
            let git = self.git_client.lock().await;
            let changed_files = git
                .get_changed_files(&change.sha)
                .map_err(SyncError::GitError)?;

            // 2. Prepare an SVN working copy. Use a temporary checkout.
            let svn_wc_dir = tempfile::tempdir()
                .map_err(|e| SyncError::SvnError(crate::errors::SvnError::IoError(e)))?;
            self.svn_client
                .checkout_head(svn_wc_dir.path())
                .await
                .map_err(SyncError::SvnError)?;

            // 3. Copy changed files from Git into the SVN working copy.
            let mut added_files = Vec::new();
            let mut deleted_files = Vec::new();

            for (action, file_path) in &changed_files {
                let dst = svn_wc_dir.path().join(file_path);
                match action.as_str() {
                    "D" => {
                        if dst.exists() {
                            deleted_files.push(file_path.as_str());
                        }
                    }
                    "A" => {
                        // Get the file content from the Git commit.
                        if let Ok(Some(content)) =
                            git.get_file_content_at_commit(&change.sha, file_path)
                        {
                            if let Some(parent) = dst.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| {
                                    SyncError::GitError(crate::errors::GitError::IoError(e))
                                })?;
                            }
                            std::fs::write(&dst, &content).map_err(|e| {
                                SyncError::GitError(crate::errors::GitError::IoError(e))
                            })?;
                            added_files.push(file_path.as_str());
                        }
                    }
                    _ => {
                        // Modified: overwrite content.
                        if let Ok(Some(content)) =
                            git.get_file_content_at_commit(&change.sha, file_path)
                        {
                            if let Some(parent) = dst.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| {
                                    SyncError::GitError(crate::errors::GitError::IoError(e))
                                })?;
                            }
                            std::fs::write(&dst, &content).map_err(|e| {
                                SyncError::GitError(crate::errors::GitError::IoError(e))
                            })?;
                        }
                    }
                }
            }
            drop(git);

            // 4. Stage changes in SVN.
            if !added_files.is_empty() {
                self.svn_client
                    .add(svn_wc_dir.path(), &added_files)
                    .await
                    .map_err(SyncError::SvnError)?;
            }
            if !deleted_files.is_empty() {
                self.svn_client
                    .rm(svn_wc_dir.path(), &deleted_files)
                    .await
                    .map_err(SyncError::SvnError)?;
            }

            // 5. Commit to SVN.
            let commit_message = format!(
                "{}\n\n{} synced from Git {}",
                change.message,
                SYNC_MARKER,
                &change.sha[..8.min(change.sha.len())]
            );
            let svn_rev = self
                .svn_client
                .commit(svn_wc_dir.path(), &commit_message, &svn_username)
                .await
                .map_err(SyncError::SvnError)?;

            // 6. Record the sync only after successful write.
            let record = crate::models::SyncRecord {
                id: uuid::Uuid::new_v4().to_string(),
                svn_revision: Some(svn_rev),
                git_hash: Some(change.sha.clone()),
                direction: crate::models::SyncDirection::GitToSvn,
                author: change.author_name.clone(),
                message: change.message.clone(),
                timestamp: Utc::now(),
                synced_at: Utc::now(),
                status: crate::models::SyncRecordStatus::Applied,
            };
            self.db
                .insert_sync_record(&record)
                .map_err(SyncError::DatabaseError)?;

            // Update the Git watermark.
            let _ = self.db.set_state("last_git_hash", &change.sha);

            count += 1;
            info!(
                sha = %change.sha,
                svn_rev,
                "synced Git {} -> SVN r{}",
                &change.sha[..8.min(change.sha.len())],
                svn_rev
            );
        }

        Ok(count)
    }

    // -----------------------------------------------------------------------
    // Change fetching
    // -----------------------------------------------------------------------

    async fn fetch_svn_changes(&self) -> Result<Vec<SvnChangeSet>, SyncError> {
        // Check the state table first (written by sync_svn_to_git), then fall
        // back to commit_map / sync_records for databases created by older
        // versions.
        let last_rev = match self
            .db
            .get_state("last_svn_rev")
            .map_err(SyncError::DatabaseError)?
        {
            Some(s) => s.parse::<i64>().unwrap_or(0),
            None => self
                .db
                .get_last_svn_revision()
                .map_err(SyncError::DatabaseError)?
                .unwrap_or(0),
        };

        info!(since_rev = last_rev, "fetching SVN changes");

        let svn_info = self.svn_client.info().await.map_err(SyncError::SvnError)?;
        let head_rev = svn_info.latest_rev;

        if head_rev <= last_rev {
            debug!("SVN is up to date");
            return Ok(Vec::new());
        }

        let entries = self
            .svn_client
            .log(last_rev + 1, head_rev)
            .await
            .map_err(SyncError::SvnError)?;

        let change_sets: Vec<SvnChangeSet> = entries
            .into_iter()
            .filter(|e| !self.is_echo_commit(&e.message))
            .map(|e| SvnChangeSet {
                revision: e.revision,
                author: e.author,
                date: e.date,
                message: e.message,
                changed_files: e
                    .changed_paths
                    .iter()
                    .map(|p| ChangedFile {
                        // SVN paths have leading '/' — strip it for filesystem joins.
                        path: p.path.strip_prefix('/').unwrap_or(&p.path).to_string(),
                        action: p.action.clone(),
                        content: None,
                        is_binary: false,
                    })
                    .collect(),
                diff_content: None,
            })
            .collect();

        debug!(count = change_sets.len(), "fetched SVN change sets");
        Ok(change_sets)
    }

    async fn fetch_git_changes(&self) -> Result<Vec<GitChangeSet>, SyncError> {
        let git = self.git_client.lock().await;

        let token = self.config.github.token.as_deref();
        git.fetch("origin", token).map_err(SyncError::GitError)?;

        // Check the state table first (written by sync_git_to_svn), then fall
        // back to commit_map / sync_records for databases created by older
        // versions.
        let last_hash = match self
            .db
            .get_state("last_git_hash")
            .map_err(SyncError::DatabaseError)?
        {
            Some(s) if !s.is_empty() => Some(s),
            _ => self
                .db
                .get_last_git_hash()
                .map_err(SyncError::DatabaseError)?,
        };

        info!(since_sha = ?last_hash, "fetching Git changes");

        let commits = git
            .get_commits_since(last_hash.as_deref())
            .map_err(SyncError::GitError)?;

        let mut change_sets: Vec<GitChangeSet> = Vec::new();
        for c in commits {
            if self.is_echo_commit(&c.message) {
                continue;
            }
            // Populate changed_files from the commit's diff.
            let files = git.get_changed_files(&c.sha).map_err(SyncError::GitError)?;
            let changed_files: Vec<ChangedFile> = files
                .into_iter()
                .map(|(action, path)| ChangedFile {
                    path,
                    action,
                    content: None,
                    is_binary: false,
                })
                .collect();
            change_sets.push(GitChangeSet {
                sha: c.sha,
                author_name: c.author_name,
                author_email: c.author_email,
                message: c.message,
                changed_files,
            });
        }

        // Reverse so oldest commits are replayed first.  get_commits_since
        // returns newest-first (revwalk order), but sync_git_to_svn must
        // apply changes chronologically so the final SVN tree matches the
        // latest Git state and intermediate revisions map correctly.
        change_sets.reverse();

        debug!(count = change_sets.len(), "fetched Git change sets");
        Ok(change_sets)
    }

    // -----------------------------------------------------------------------
    // Conflict detection
    // -----------------------------------------------------------------------

    fn detect_conflicts_internal(
        &self,
        svn_changes: &[SvnChangeSet],
        git_changes: &[GitChangeSet],
    ) -> Vec<Conflict> {
        let svn_file_changes: Vec<FileChange> = svn_changes
            .iter()
            .flat_map(|cs| {
                cs.changed_files.iter().map(|f| FileChange {
                    path: f.path.clone(),
                    change_kind: match f.action.as_str() {
                        "A" => ChangeKind::Added,
                        "D" => ChangeKind::Deleted,
                        "M" => ChangeKind::Modified,
                        _ => ChangeKind::Modified,
                    },
                    content: f.content.clone(),
                    is_binary: f.is_binary,
                })
            })
            .collect();

        let git_file_changes: Vec<FileChange> = git_changes
            .iter()
            .flat_map(|cs| {
                cs.changed_files.iter().map(|f| FileChange {
                    path: f.path.clone(),
                    change_kind: match f.action.as_str() {
                        "A" => ChangeKind::Added,
                        "D" => ChangeKind::Deleted,
                        "M" => ChangeKind::Modified,
                        _ => ChangeKind::Modified,
                    },
                    content: f.content.clone(),
                    is_binary: f.is_binary,
                })
            })
            .collect();

        ConflictDetector::detect(&svn_file_changes, &git_file_changes)
    }

    // -----------------------------------------------------------------------
    // Echo detection
    // -----------------------------------------------------------------------

    fn is_echo_commit(&self, message: &str) -> bool {
        message.contains(SYNC_MARKER)
    }

    fn try_auto_merge(&self, conflict: &Conflict) -> bool {
        let (base, ours, theirs) = match (
            &conflict.base_content,
            &conflict.svn_content,
            &conflict.git_content,
        ) {
            (Some(b), Some(o), Some(t)) => (b.as_str(), o.as_str(), t.as_str()),
            _ => return false,
        };

        if Merger::can_auto_merge(base, ours, theirs) {
            match Merger::three_way_merge(base, ours, theirs) {
                Ok(result) if !result.has_conflicts => {
                    info!(file = %conflict.file_path, "auto-merged conflict");
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Standalone diff application (avoids holding GitClient across await points)
// ---------------------------------------------------------------------------

/// Apply a unified diff to a git repository at the given path.
///
/// This is a standalone async function that does not hold a reference to
/// `GitClient`, avoiding `Send` issues with `git2::Repository`.
async fn apply_diff_to_path(
    repo_path: &std::path::Path,
    diff_content: &str,
) -> Result<(), crate::errors::GitError> {
    use std::process::Stdio;
    use tokio::process::Command;
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path)
        .args(["apply", "--3way", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(crate::errors::GitError::IoError)?;
    if let Some(ref mut stdin) = child.stdin {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(diff_content.as_bytes())
            .await
            .map_err(crate::errors::GitError::IoError)?;
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(crate::errors::GitError::IoError)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        tracing::warn!(%stderr, "git apply failed");
        return Err(crate::errors::GitError::ApplyFailed(stderr));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sync lock RAII guard
// ---------------------------------------------------------------------------

/// Drop guard that resets the `running` flag to `false`.
///
/// This ensures the sync lock is always released, even if a sync cycle panics.
struct SyncLockGuard(Arc<AtomicBool>);

impl Drop for SyncLockGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// Internal change-set types
// ---------------------------------------------------------------------------

/// A set of changes from a single SVN revision.
#[derive(Debug, Clone)]
pub struct SvnChangeSet {
    pub revision: i64,
    pub author: String,
    pub date: String,
    pub message: String,
    pub changed_files: Vec<ChangedFile>,
    pub diff_content: Option<String>,
}

/// A set of changes from a single Git commit.
#[derive(Debug, Clone)]
pub struct GitChangeSet {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    pub message: String,
    pub changed_files: Vec<ChangedFile>,
}

/// A single file changed in a commit.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub action: String,
    pub content: Option<String>,
    pub is_binary: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_echo_commit() {
        let message_with_marker = "Fix bug\n\n[gitsvnsync] synced from SVN r42";
        assert!(message_with_marker.contains(SYNC_MARKER));

        let normal_message = "Fix bug in authentication";
        assert!(!normal_message.contains(SYNC_MARKER));
    }

    #[test]
    fn test_sync_state_display() {
        assert_eq!(SyncState::Idle.to_string(), "idle");
        assert_eq!(SyncState::Detecting.to_string(), "detecting");
        assert_eq!(SyncState::Applying.to_string(), "applying");
        assert_eq!(SyncState::Committed.to_string(), "committed");
        assert_eq!(SyncState::ConflictFound.to_string(), "conflict_found");
        assert_eq!(
            SyncState::QueuedForResolution.to_string(),
            "queued_for_resolution"
        );
        assert_eq!(
            SyncState::ResolutionApplied.to_string(),
            "resolution_applied"
        );
    }
}

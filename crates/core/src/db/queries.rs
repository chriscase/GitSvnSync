//! Typed query helpers for every table in the GitSvnSync database.

use chrono::Utc;
use rusqlite::params;
use tracing::debug;
use uuid::Uuid;

use super::Database;
use crate::errors::DatabaseError;
use crate::models;

// ---------------------------------------------------------------------------
// Domain structs returned by queries
// ---------------------------------------------------------------------------

/// A row from the `commit_map` table.
#[derive(Debug, Clone)]
pub struct CommitMapEntry {
    pub id: i64,
    pub svn_rev: i64,
    pub git_sha: String,
    pub direction: String,
    pub synced_at: String,
    pub svn_author: String,
    pub git_author: String,
}

/// A row from the `sync_state` table.
#[derive(Debug, Clone)]
pub struct SyncStateEntry {
    pub id: i64,
    pub state: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub details: Option<String>,
}

/// A row from the `conflicts` table.
#[derive(Debug, Clone)]
pub struct ConflictEntry {
    pub id: String,
    pub file_path: String,
    pub conflict_type: String,
    pub svn_content: Option<String>,
    pub git_content: Option<String>,
    pub base_content: Option<String>,
    pub svn_rev: Option<i64>,
    pub git_sha: Option<String>,
    pub status: String,
    pub resolution: Option<String>,
    pub resolved_by: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// A row from the `watermarks` table.
#[derive(Debug, Clone)]
pub struct WatermarkEntry {
    pub source: String,
    pub value: String,
    pub updated_at: String,
}

/// A row from the `audit_log` table.
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub id: i64,
    pub action: String,
    pub direction: Option<String>,
    pub svn_rev: Option<i64>,
    pub git_sha: Option<String>,
    pub author: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Query implementations
// ---------------------------------------------------------------------------

impl Database {
    // -- commit_map ---------------------------------------------------------

    /// Insert a new commit-map entry linking an SVN revision to a Git SHA.
    pub fn insert_commit_map(
        &self,
        svn_rev: i64,
        git_sha: &str,
        direction: &str,
        svn_author: &str,
        git_author: &str,
    ) -> Result<i64, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO commit_map (svn_rev, git_sha, direction, synced_at, svn_author, git_author)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![svn_rev, git_sha, direction, now, svn_author, git_author],
        )?;
        let id = self.conn().last_insert_rowid();
        debug!(id, svn_rev, git_sha, direction, "inserted commit_map entry");
        Ok(id)
    }

    /// Look up a Git SHA by SVN revision.
    pub fn get_git_sha_for_svn_rev(&self, svn_rev: i64) -> Result<Option<String>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT git_sha FROM commit_map WHERE svn_rev = ?1 LIMIT 1")?;
        let mut rows = stmt.query_map(params![svn_rev], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(sha)) => Ok(Some(sha)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Look up an SVN revision by Git SHA.
    pub fn get_svn_rev_for_git_sha(&self, git_sha: &str) -> Result<Option<i64>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT svn_rev FROM commit_map WHERE git_sha = ?1 LIMIT 1")?;
        let mut rows = stmt.query_map(params![git_sha], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(rev)) => Ok(Some(rev)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Return the most recent N commit-map entries ordered by synced_at desc.
    pub fn list_commit_map(&self, limit: u32) -> Result<Vec<CommitMapEntry>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, svn_rev, git_sha, direction, synced_at, svn_author, git_author
             FROM commit_map ORDER BY id DESC LIMIT ?1",
        )?;
        let entries = stmt
            .query_map(params![limit], |row| {
                Ok(CommitMapEntry {
                    id: row.get(0)?,
                    svn_rev: row.get(1)?,
                    git_sha: row.get(2)?,
                    direction: row.get(3)?,
                    synced_at: row.get(4)?,
                    svn_author: row.get(5)?,
                    git_author: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Check whether a given SVN revision has already been synced.
    pub fn is_svn_rev_synced(&self, svn_rev: i64) -> Result<bool, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM commit_map WHERE svn_rev = ?1",
            params![svn_rev],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check whether a given Git SHA has already been synced.
    pub fn is_git_sha_synced(&self, git_sha: &str) -> Result<bool, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM commit_map WHERE git_sha = ?1",
            params![git_sha],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // -- sync_state ---------------------------------------------------------

    /// Record the start of a new sync cycle.
    pub fn start_sync_state(&self, state: &str, details: Option<&str>) -> Result<i64, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO sync_state (state, started_at, details) VALUES (?1, ?2, ?3)",
            params![state, now, details],
        )?;
        let id = self.conn().last_insert_rowid();
        debug!(id, state, "started sync_state");
        Ok(id)
    }

    /// Mark a sync-state entry as completed.
    pub fn complete_sync_state(
        &self,
        id: i64,
        state: &str,
        details: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let changed = self.conn().execute(
            "UPDATE sync_state SET state = ?1, completed_at = ?2, details = ?3 WHERE id = ?4",
            params![state, now, details, id],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound {
                entity: "sync_state".into(),
                id: id.to_string(),
            });
        }
        debug!(id, state, "completed sync_state");
        Ok(())
    }

    /// Get the latest sync-state entry.
    pub fn get_latest_sync_state(&self) -> Result<Option<SyncStateEntry>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, state, started_at, completed_at, details
             FROM sync_state ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], |row| {
            Ok(SyncStateEntry {
                id: row.get(0)?,
                state: row.get(1)?,
                started_at: row.get(2)?,
                completed_at: row.get(3)?,
                details: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(Ok(entry)) => Ok(Some(entry)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    // -- conflicts ----------------------------------------------------------

    /// Insert a new conflict record.
    pub fn insert_conflict_entry(
        &self,
        file_path: &str,
        conflict_type: &str,
        svn_content: Option<&str>,
        git_content: Option<&str>,
        base_content: Option<&str>,
        svn_rev: Option<i64>,
        git_sha: Option<&str>,
    ) -> Result<String, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO conflicts (id, file_path, conflict_type, svn_content, git_content,
             base_content, svn_rev, git_sha, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'detected', ?9)",
            params![id, file_path, conflict_type, svn_content, git_content, base_content, svn_rev, git_sha, now],
        )?;
        debug!(id = %id, file_path, conflict_type, "inserted conflict");
        Ok(id)
    }

    /// Insert a conflict from a model struct.
    pub fn insert_conflict(&self, conflict: &models::Conflict) -> Result<String, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO conflicts (id, file_path, conflict_type, svn_content, git_content,
             base_content, svn_rev, git_sha, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                conflict.id,
                conflict.file_path,
                conflict.conflict_type,
                conflict.svn_content,
                conflict.git_content,
                conflict.base_content,
                conflict.svn_revision,
                conflict.git_hash,
                conflict.status,
                now
            ],
        )?;
        debug!(id = %conflict.id, file_path = %conflict.file_path, "inserted conflict");
        Ok(conflict.id.clone())
    }

    /// Get a conflict by ID.
    pub fn get_conflict(&self, id: &str) -> Result<ConflictEntry, DatabaseError> {
        self.conn()
            .query_row(
                "SELECT id, file_path, conflict_type, svn_content, git_content, base_content,
                 svn_rev, git_sha, status, resolution, resolved_by, created_at, resolved_at
                 FROM conflicts WHERE id = ?1",
                params![id],
                |row| {
                    Ok(ConflictEntry {
                        id: row.get(0)?,
                        file_path: row.get(1)?,
                        conflict_type: row.get(2)?,
                        svn_content: row.get(3)?,
                        git_content: row.get(4)?,
                        base_content: row.get(5)?,
                        svn_rev: row.get(6)?,
                        git_sha: row.get(7)?,
                        status: row.get(8)?,
                        resolution: row.get(9)?,
                        resolved_by: row.get(10)?,
                        created_at: row.get(11)?,
                        resolved_at: row.get(12)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DatabaseError::NotFound {
                    entity: "conflict".into(),
                    id: id.to_string(),
                },
                other => other.into(),
            })
    }

    /// List conflicts filtered by status, ordered by creation date descending.
    pub fn list_conflicts(
        &self,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ConflictEntry>, DatabaseError> {
        let (sql, bound_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match status {
            Some(s) => (
                "SELECT id, file_path, conflict_type, svn_content, git_content, base_content,
                 svn_rev, git_sha, status, resolution, resolved_by, created_at, resolved_at
                 FROM conflicts WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2"
                    .to_string(),
                vec![Box::new(s.to_string()), Box::new(limit)],
            ),
            None => (
                "SELECT id, file_path, conflict_type, svn_content, git_content, base_content,
                 svn_rev, git_sha, status, resolution, resolved_by, created_at, resolved_at
                 FROM conflicts ORDER BY created_at DESC LIMIT ?1"
                    .to_string(),
                vec![Box::new(limit)],
            ),
        };

        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            bound_params.iter().map(|p| p.as_ref()).collect();
        let entries = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(ConflictEntry {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    conflict_type: row.get(2)?,
                    svn_content: row.get(3)?,
                    git_content: row.get(4)?,
                    base_content: row.get(5)?,
                    svn_rev: row.get(6)?,
                    git_sha: row.get(7)?,
                    status: row.get(8)?,
                    resolution: row.get(9)?,
                    resolved_by: row.get(10)?,
                    created_at: row.get(11)?,
                    resolved_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Update the status and resolution of a conflict.
    pub fn resolve_conflict(
        &self,
        id: &str,
        status: &str,
        resolution: &str,
        resolved_by: &str,
    ) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let changed = self.conn().execute(
            "UPDATE conflicts SET status = ?1, resolution = ?2, resolved_by = ?3, resolved_at = ?4
             WHERE id = ?5",
            params![status, resolution, resolved_by, now, id],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound {
                entity: "conflict".into(),
                id: id.to_string(),
            });
        }
        debug!(id, status, resolution, "resolved conflict");
        Ok(())
    }

    /// Count conflicts by status.
    pub fn count_conflicts_by_status(&self, status: &str) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM conflicts WHERE status = ?1",
            params![status],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Count total conflicts.
    pub fn count_all_conflicts(&self) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM conflicts",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Count active (non-resolved, non-deferred) conflicts.
    pub fn count_active_conflicts(&self) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM conflicts WHERE status NOT IN ('resolved', 'deferred')",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // -- watermarks ---------------------------------------------------------

    /// Get the watermark value for a given source.
    pub fn get_watermark(&self, source: &str) -> Result<Option<String>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT value FROM watermarks WHERE source = ?1")?;
        let mut rows = stmt.query_map(params![source], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Upsert a watermark value.
    pub fn set_watermark(&self, source: &str, value: &str) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO watermarks (source, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(source) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![source, value, now],
        )?;
        debug!(source, value, "set watermark");
        Ok(())
    }

    /// List all watermarks.
    pub fn list_watermarks(&self) -> Result<Vec<WatermarkEntry>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT source, value, updated_at FROM watermarks ORDER BY source")?;
        let entries = stmt
            .query_map([], |row| {
                Ok(WatermarkEntry {
                    source: row.get(0)?,
                    value: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    // -- audit_log ----------------------------------------------------------

    /// Insert an audit-log entry.
    pub fn insert_audit_log(
        &self,
        action: &str,
        direction: Option<&str>,
        svn_rev: Option<i64>,
        git_sha: Option<&str>,
        author: Option<&str>,
        details: Option<&str>,
    ) -> Result<i64, DatabaseError> {
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO audit_log (action, direction, svn_rev, git_sha, author, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![action, direction, svn_rev, git_sha, author, details, now],
        )?;
        let id = self.conn().last_insert_rowid();
        debug!(id, action, "inserted audit_log entry");
        Ok(id)
    }

    /// Insert an audit entry from a model struct.
    pub fn insert_audit_entry(&self, entry: &models::AuditEntry) -> Result<i64, DatabaseError> {
        self.insert_audit_log(
            &entry.action,
            None,
            None,
            None,
            None,
            Some(&entry.details),
        )
    }

    /// List recent audit-log entries.
    pub fn list_audit_log(&self, limit: u32) -> Result<Vec<AuditLogEntry>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, action, direction, svn_rev, git_sha, author, details, created_at
             FROM audit_log ORDER BY id DESC LIMIT ?1",
        )?;
        let entries = stmt
            .query_map(params![limit], |row| {
                Ok(AuditLogEntry {
                    id: row.get(0)?,
                    action: row.get(1)?,
                    direction: row.get(2)?,
                    svn_rev: row.get(3)?,
                    git_sha: row.get(4)?,
                    author: row.get(5)?,
                    details: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Count total audit-log entries.
    pub fn count_audit_log(&self) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM audit_log",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// List audit-log entries filtered by action.
    pub fn list_audit_log_by_action(
        &self,
        action: &str,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, action, direction, svn_rev, git_sha, author, details, created_at
             FROM audit_log WHERE action = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let entries = stmt
            .query_map(params![action, limit], |row| {
                Ok(AuditLogEntry {
                    id: row.get(0)?,
                    action: row.get(1)?,
                    direction: row.get(2)?,
                    svn_rev: row.get(3)?,
                    git_sha: row.get(4)?,
                    author: row.get(5)?,
                    details: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Count audit entries that represent errors.
    pub fn count_errors(&self) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action LIKE '%error%' OR action LIKE '%fail%'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // -- kv_state -----------------------------------------------------------

    /// Get a key-value state entry.
    pub fn get_state(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT value FROM kv_state WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Set a key-value state entry (upsert).
    pub fn set_state(&self, key: &str, value: &str) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO kv_state (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now],
        )?;
        debug!(key, value, "set kv_state");
        Ok(())
    }

    // -- sync_records -------------------------------------------------------

    /// Insert a sync record.
    pub fn insert_sync_record(&self, record: &models::SyncRecord) -> Result<(), DatabaseError> {
        self.conn().execute(
            "INSERT INTO sync_records (id, svn_rev, git_sha, direction, author, message, timestamp, synced_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.svn_revision,
                record.git_hash,
                record.direction.to_string(),
                record.author,
                record.message,
                record.timestamp.to_rfc3339(),
                record.synced_at.to_rfc3339(),
                record.status.to_string(),
            ],
        )?;
        debug!(id = %record.id, "inserted sync_record");
        Ok(())
    }

    /// Count total sync records.
    pub fn count_sync_records(&self) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sync_records",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get the last SVN revision from the commit map or sync records.
    pub fn get_last_svn_revision(&self) -> Result<Option<i64>, DatabaseError> {
        let result: Result<Option<i64>, _> = self.conn().query_row(
            "SELECT MAX(svn_rev) FROM commit_map",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(Some(rev)) => Ok(Some(rev)),
            Ok(None) => {
                // Try sync_records as fallback
                let result2: Result<Option<i64>, _> = self.conn().query_row(
                    "SELECT MAX(svn_rev) FROM sync_records WHERE svn_rev IS NOT NULL",
                    [],
                    |row| row.get(0),
                );
                Ok(result2.unwrap_or(None))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Get the last Git hash from the commit map or sync records.
    pub fn get_last_git_hash(&self) -> Result<Option<String>, DatabaseError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT git_sha FROM commit_map ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(sha)) => Ok(Some(sha)),
            Some(Err(e)) => Err(e.into()),
            None => {
                // Need to drop stmt and rows before getting a new stmt
                drop(rows);
                drop(stmt);
                // Try sync_records as fallback
                let mut stmt2 = conn.prepare(
                    "SELECT git_sha FROM sync_records WHERE git_sha IS NOT NULL ORDER BY synced_at DESC LIMIT 1",
                )?;
                let mut rows2 = stmt2.query_map([], |row| row.get::<_, String>(0))?;
                match rows2.next() {
                    Some(Ok(sha)) => Ok(Some(sha)),
                    Some(Err(e)) => Err(e.into()),
                    None => Ok(None),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Database {
        let db = Database::in_memory().unwrap();
        db.initialize().unwrap();
        db
    }

    #[test]
    fn test_commit_map_crud() {
        let db = setup_db();
        let id = db
            .insert_commit_map(100, "abc123", "svn_to_git", "alice", "Alice <alice@ex.com>")
            .unwrap();
        assert!(id > 0);
        assert_eq!(db.get_git_sha_for_svn_rev(100).unwrap().as_deref(), Some("abc123"));
        assert_eq!(db.get_svn_rev_for_git_sha("abc123").unwrap(), Some(100));
        assert!(db.is_svn_rev_synced(100).unwrap());
        assert!(!db.is_svn_rev_synced(999).unwrap());
    }

    #[test]
    fn test_sync_state() {
        let db = setup_db();
        let id = db.start_sync_state("running", Some("cycle 1")).unwrap();
        db.complete_sync_state(id, "completed", Some("ok")).unwrap();
        let latest = db.get_latest_sync_state().unwrap().unwrap();
        assert_eq!(latest.state, "completed");
    }

    #[test]
    fn test_conflict_crud() {
        let db = setup_db();
        let id = db
            .insert_conflict_entry("src/main.rs", "content", Some("svn"), Some("git"), Some("base"), Some(42), Some("abc"))
            .unwrap();
        let conflict = db.get_conflict(&id).unwrap();
        assert_eq!(conflict.file_path, "src/main.rs");
        assert_eq!(conflict.status, "detected");

        db.resolve_conflict(&id, "resolved", "accept_svn", "admin").unwrap();
        let resolved = db.get_conflict(&id).unwrap();
        assert_eq!(resolved.status, "resolved");
    }

    #[test]
    fn test_watermark_crud() {
        let db = setup_db();
        assert!(db.get_watermark("svn").unwrap().is_none());
        db.set_watermark("svn", "100").unwrap();
        assert_eq!(db.get_watermark("svn").unwrap().as_deref(), Some("100"));
        db.set_watermark("svn", "200").unwrap();
        assert_eq!(db.get_watermark("svn").unwrap().as_deref(), Some("200"));
    }

    #[test]
    fn test_audit_log() {
        let db = setup_db();
        db.insert_audit_log("sync", Some("svn_to_git"), Some(42), Some("abc"), Some("alice"), Some("test"))
            .unwrap();
        let entries = db.list_audit_log(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(db.count_audit_log().unwrap(), 1);
    }

    #[test]
    fn test_kv_state() {
        let db = setup_db();
        assert!(db.get_state("foo").unwrap().is_none());
        db.set_state("foo", "bar").unwrap();
        assert_eq!(db.get_state("foo").unwrap().as_deref(), Some("bar"));
        db.set_state("foo", "baz").unwrap();
        assert_eq!(db.get_state("foo").unwrap().as_deref(), Some("baz"));
    }
}

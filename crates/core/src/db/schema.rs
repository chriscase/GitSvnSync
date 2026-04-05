//! Database schema definitions and migration runner.
//!
//! Migrations are simple SQL strings applied in order. The `schema_version`
//! user-version pragma tracks which migrations have already been applied.

use rusqlite::Connection;
use tracing::{debug, info};

use crate::errors::DatabaseError;

/// All migrations, in order. Each entry is `(version, description, sql)`.
/// Versions start at 1. The current schema version is stored in the SQLite
/// `user_version` pragma.
static MIGRATIONS: &[(u32, &str, &str)] = &[
    (
        1,
        "initial schema",
        r#"
        CREATE TABLE IF NOT EXISTS commit_map (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            svn_rev     INTEGER NOT NULL,
            git_sha     TEXT    NOT NULL,
            direction   TEXT    NOT NULL CHECK (direction IN ('svn_to_git', 'git_to_svn')),
            synced_at   TEXT    NOT NULL,
            svn_author  TEXT    NOT NULL DEFAULT '',
            git_author  TEXT    NOT NULL DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_commit_map_svn_rev ON commit_map (svn_rev);
        CREATE INDEX IF NOT EXISTS idx_commit_map_git_sha ON commit_map (git_sha);

        CREATE TABLE IF NOT EXISTS sync_state (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            state        TEXT    NOT NULL,
            started_at   TEXT    NOT NULL,
            completed_at TEXT,
            details      TEXT
        );

        CREATE TABLE IF NOT EXISTS conflicts (
            id              TEXT PRIMARY KEY,
            file_path       TEXT NOT NULL,
            conflict_type   TEXT NOT NULL,
            svn_content     TEXT,
            git_content     TEXT,
            base_content    TEXT,
            svn_rev         INTEGER,
            git_sha         TEXT,
            status          TEXT NOT NULL DEFAULT 'detected',
            resolution      TEXT,
            resolved_by     TEXT,
            created_at      TEXT NOT NULL,
            resolved_at     TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_conflicts_status ON conflicts (status);

        CREATE TABLE IF NOT EXISTS watermarks (
            source      TEXT PRIMARY KEY,
            value       TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            action      TEXT NOT NULL,
            direction   TEXT,
            svn_rev     INTEGER,
            git_sha     TEXT,
            author      TEXT,
            details     TEXT,
            created_at  TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log (created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log (action);

        CREATE TABLE IF NOT EXISTS sync_records (
            id          TEXT PRIMARY KEY,
            svn_rev     INTEGER,
            git_sha     TEXT,
            direction   TEXT NOT NULL,
            author      TEXT NOT NULL DEFAULT '',
            message     TEXT NOT NULL DEFAULT '',
            timestamp   TEXT NOT NULL,
            synced_at   TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'pending'
        );

        CREATE INDEX IF NOT EXISTS idx_sync_records_direction ON sync_records (direction);
        CREATE INDEX IF NOT EXISTS idx_sync_records_synced_at ON sync_records (synced_at);

        CREATE TABLE IF NOT EXISTS kv_state (
            key         TEXT PRIMARY KEY,
            value       TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        "#,
    ),
    (
        2,
        "personal branch PR sync log",
        r#"
        CREATE TABLE IF NOT EXISTS pr_sync_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            pr_number       INTEGER NOT NULL,
            pr_title        TEXT NOT NULL DEFAULT '',
            pr_branch       TEXT NOT NULL DEFAULT '',
            merge_sha       TEXT NOT NULL,
            merge_strategy  TEXT NOT NULL DEFAULT 'unknown',
            svn_rev_start   INTEGER,
            svn_rev_end     INTEGER,
            commit_count    INTEGER NOT NULL DEFAULT 0,
            status          TEXT NOT NULL DEFAULT 'pending',
            error_message   TEXT,
            detected_at     TEXT NOT NULL,
            completed_at    TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_pr_sync_log_merge_sha ON pr_sync_log (merge_sha);
        CREATE INDEX IF NOT EXISTS idx_pr_sync_log_status ON pr_sync_log (status);
        "#,
    ),
    (
        3,
        "add success column to audit_log",
        r#"
        ALTER TABLE audit_log ADD COLUMN success INTEGER NOT NULL DEFAULT 1;
        CREATE INDEX IF NOT EXISTS idx_audit_log_success ON audit_log (success);
        "#,
    ),
];

/// Run all pending migrations against `conn`.
pub fn run_migrations(conn: &Connection) -> Result<(), DatabaseError> {
    let current_version = get_schema_version(conn)?;
    info!(
        current_version,
        target_version = MIGRATIONS.last().map(|m| m.0).unwrap_or(0),
        "checking database migrations"
    );

    for &(version, description, sql) in MIGRATIONS {
        if version > current_version {
            info!(version, description, "applying migration");
            conn.execute_batch(sql)
                .map_err(|e| DatabaseError::MigrationFailed {
                    version,
                    detail: e.to_string(),
                })?;
            set_schema_version(conn, version)?;
            debug!(version, "migration applied successfully");
        }
    }

    Ok(())
}

/// Read the current schema version from the SQLite `user_version` pragma.
fn get_schema_version(conn: &Connection) -> Result<u32, DatabaseError> {
    let version: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(version)
}

/// Set the schema version via the SQLite `user_version` pragma.
fn set_schema_version(conn: &Connection, version: u32) -> Result<(), DatabaseError> {
    conn.pragma_update(None, "user_version", version)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_run_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), 3);
    }

    #[test]
    fn test_tables_created() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };

        assert!(tables.contains(&"commit_map".to_string()));
        assert!(tables.contains(&"sync_state".to_string()));
        assert!(tables.contains(&"conflicts".to_string()));
        assert!(tables.contains(&"watermarks".to_string()));
        assert!(tables.contains(&"audit_log".to_string()));
        assert!(tables.contains(&"sync_records".to_string()));
        assert!(tables.contains(&"kv_state".to_string()));
        assert!(tables.contains(&"pr_sync_log".to_string()));
    }
}

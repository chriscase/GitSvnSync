//! Sync history and commit-map API endpoints.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::auth::validate_session;
use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<u32>,
    pub repo_id: Option<String>,
}

#[derive(Serialize)]
pub struct CommitMapEntryView {
    pub id: i64,
    pub svn_rev: i64,
    pub git_sha: String,
    pub direction: String,
    pub synced_at: String,
    pub svn_author: String,
    pub git_author: String,
}

#[derive(Serialize)]
pub struct CommitMapResponse {
    pub entries: Vec<CommitMapEntryView>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct SyncRecordView {
    pub id: String,
    pub svn_rev: Option<i64>,
    pub git_sha: Option<String>,
    pub direction: String,
    pub author: String,
    pub message: String,
    pub timestamp: String,
    pub synced_at: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct SyncRecordResponse {
    pub entries: Vec<SyncRecordView>,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/commit-map", get(list_commit_map))
        .route("/api/sync-records", get(list_sync_records))
}

async fn list_commit_map(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<CommitMapResponse>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let limit = query.limit.unwrap_or(100).min(500);

    let db = &state.db;

    // Support optional repo_id filtering
    let conn = db.conn();
    let (sql, views) = if let Some(ref rid) = query.repo_id {
        let mut stmt = conn.prepare(
            "SELECT id, svn_rev, git_sha, direction, synced_at, svn_author, git_author
             FROM commit_map WHERE repo_id = ?1 ORDER BY id DESC LIMIT ?2",
        ).map_err(|e| AppError::Internal(format!("prepare: {}", e)))?;
        let rows: Vec<CommitMapEntryView> = stmt.query_map(rusqlite::params![rid, limit], |row| {
            Ok(CommitMapEntryView {
                id: row.get(0)?, svn_rev: row.get(1)?, git_sha: row.get(2)?,
                direction: row.get(3)?, synced_at: row.get(4)?,
                svn_author: row.get(5)?, git_author: row.get(6)?,
            })
        }).map_err(|e| AppError::Internal(format!("query: {}", e)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(format!("row: {}", e)))?;
        ("filtered".to_string(), rows)
    } else {
        let entries = db.list_commit_map(limit)
            .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;
        let v: Vec<CommitMapEntryView> = entries.into_iter().map(|e| CommitMapEntryView {
            id: e.id, svn_rev: e.svn_rev, git_sha: e.git_sha,
            direction: e.direction, synced_at: e.synced_at,
            svn_author: e.svn_author, git_author: e.git_author,
        }).collect();
        ("all".to_string(), v)
    };
    let _ = sql;

    let total = views.len();
    Ok(Json(CommitMapResponse {
        entries: views,
        total,
    }))
}

async fn list_sync_records(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<SyncRecordResponse>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let limit = query.limit.unwrap_or(100).min(500);

    let db = &state.db;

    let conn = db.conn();
    let (sql, params_list): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(ref rid) = query.repo_id {
        (
            "SELECT id, svn_rev, git_sha, direction, author, message, timestamp, synced_at, status
             FROM sync_records WHERE repo_id = ?1 ORDER BY synced_at DESC LIMIT ?2".to_string(),
            vec![Box::new(rid.clone()), Box::new(limit)],
        )
    } else {
        (
            "SELECT id, svn_rev, git_sha, direction, author, message, timestamp, synced_at, status
             FROM sync_records ORDER BY synced_at DESC LIMIT ?1".to_string(),
            vec![Box::new(limit)],
        )
    };
    let mut stmt = conn.prepare(&sql)
        .map_err(|e| AppError::Internal(format!("prepare error: {}", e)))?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_list.iter().map(|b| b.as_ref()).collect();

    let entries: Vec<SyncRecordView> = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(SyncRecordView {
                id: row.get(0)?,
                svn_rev: row.get(1)?,
                git_sha: row.get(2)?,
                direction: row.get(3)?,
                author: row.get(4)?,
                message: row.get(5)?,
                timestamp: row.get(6)?,
                synced_at: row.get(7)?,
                status: row.get(8)?,
            })
        })
        .map_err(|e| AppError::Internal(format!("query error: {}", e)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(format!("row error: {}", e)))?;

    let total = entries.len();
    Ok(Json(SyncRecordResponse {
        entries,
        total,
    }))
}

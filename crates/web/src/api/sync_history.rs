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

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;
    let entries = db
        .list_commit_map(limit)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    let total = entries.len();
    let views: Vec<CommitMapEntryView> = entries
        .into_iter()
        .map(|e| CommitMapEntryView {
            id: e.id,
            svn_rev: e.svn_rev,
            git_sha: e.git_sha,
            direction: e.direction,
            synced_at: e.synced_at,
            svn_author: e.svn_author,
            git_author: e.git_author,
        })
        .collect();

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

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, svn_rev, git_sha, direction, author, message, timestamp, synced_at, status
             FROM sync_records ORDER BY synced_at DESC LIMIT ?1",
        )
        .map_err(|e| AppError::Internal(format!("prepare error: {}", e)))?;

    let entries: Vec<SyncRecordView> = stmt
        .query_map(rusqlite::params![limit], |row| {
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

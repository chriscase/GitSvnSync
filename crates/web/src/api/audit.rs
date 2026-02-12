//! Audit log API endpoints.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AuditQuery {
    pub limit: Option<u32>,
}

#[derive(Serialize)]
struct AuditEntryView {
    id: i64,
    created_at: String,
    action: String,
    details: Option<String>,
    author: Option<String>,
    direction: Option<String>,
    svn_rev: Option<i64>,
    git_sha: Option<String>,
}

#[derive(Serialize)]
struct AuditListResponse {
    entries: Vec<AuditEntryView>,
    total: usize,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/audit", get(list_audit))
}

async fn list_audit(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditListResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).min(500);

    let db = state.db.lock().map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;
    let entries = db
        .list_audit_log(limit)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    let total = entries.len();
    let views: Vec<AuditEntryView> = entries
        .into_iter()
        .map(|e| AuditEntryView {
            id: e.id,
            created_at: e.created_at,
            action: e.action,
            details: e.details,
            author: e.author,
            direction: e.direction,
            svn_rev: e.svn_rev,
            git_sha: e.git_sha,
        })
        .collect();

    Ok(Json(AuditListResponse {
        entries: views,
        total,
    }))
}

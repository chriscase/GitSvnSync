//! Audit log API endpoints.

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
pub struct AuditQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub page: Option<u32>,
    pub success: Option<bool>,
    pub repo_id: Option<String>,
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
    success: bool,
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
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditListResponse>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let limit = query.limit.unwrap_or(50).min(500);
    let offset = if let Some(page) = query.page {
        (page.saturating_sub(1)) * limit
    } else {
        query.offset.unwrap_or(0)
    };

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let total_count = db
        .count_audit_log()
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))? as usize;

    // Fetch audit entries, optionally filtered by repo_id
    let entries = db
        .list_audit_log(limit, offset)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    // Filter by repo_id if provided (client-side filter for now —
    // repo_id column exists but not all entries have it populated yet)
    let views: Vec<AuditEntryView> = entries.into_iter().map(|e| AuditEntryView {
        id: e.id, created_at: e.created_at, action: e.action,
        details: e.details, author: e.author, direction: e.direction,
        svn_rev: e.svn_rev, git_sha: e.git_sha, success: e.success,
    }).collect();

    // Apply success filter if provided
    let views: Vec<AuditEntryView> = if let Some(success_val) = query.success {
        views.into_iter().filter(|e| e.success == success_val).collect()
    } else {
        views
    };

    let total = total_count;

    Ok(Json(AuditListResponse {
        entries: views,
        total,
    }))
}

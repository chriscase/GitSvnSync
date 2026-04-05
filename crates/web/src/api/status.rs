//! Status and health check endpoints.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Health check response.
#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    version: String,
}

/// Status response wrapping the core SyncStatus.
#[derive(Serialize)]
struct StatusResponse {
    state: String,
    last_sync_at: Option<String>,
    last_svn_revision: Option<i64>,
    last_git_hash: Option<String>,
    total_syncs: i64,
    total_conflicts: i64,
    active_conflicts: i64,
    total_errors: i64,
    uptime_secs: u64,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/status/health", get(health_check))
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Deserialize)]
struct StatusQuery {
    repo_id: Option<String>,
}

async fn get_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<StatusQuery>,
) -> Result<Json<StatusResponse>, AppError> {
    crate::api::auth::validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let status = state
        .sync_engine
        .get_status()
        .map_err(|e| AppError::Internal(format!("failed to get sync status: {}", e)))?;

    // When a repo_id is specified, override the global watermarks with
    // per-repo values stored in kv_state by the import/sync pipeline.
    let (svn_rev, git_hash) = if let Some(ref rid) = query.repo_id {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::Internal("db lock poisoned".into()))?;
        let repo_svn = db
            .get_state(&format!("last_svn_rev_{}", rid))
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok());
        let repo_git = db
            .get_state(&format!("last_git_sha_{}", rid))
            .ok()
            .flatten();
        (
            repo_svn.or(status.last_svn_revision),
            repo_git.or(status.last_git_hash),
        )
    } else {
        (status.last_svn_revision, status.last_git_hash)
    };

    Ok(Json(StatusResponse {
        state: status.state.to_string(),
        last_sync_at: status.last_sync_at.map(|t| t.to_rfc3339()),
        last_svn_revision: svn_rev,
        last_git_hash: git_hash,
        total_syncs: status.total_syncs,
        total_conflicts: status.total_conflicts,
        active_conflicts: status.active_conflicts,
        total_errors: status.total_errors,
        uptime_secs: status.uptime_secs,
    }))
}

// ---------------------------------------------------------------------------
// Shared error type for API handlers
// ---------------------------------------------------------------------------

/// Simple API error type that converts to an Axum response.
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Unauthorized(String),
    Internal(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (axum::http::StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized(msg) => (axum::http::StatusCode::UNAUTHORIZED, msg),
            AppError::Internal(msg) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = serde_json::json!({ "error": message });
        (status, Json(body)).into_response()
    }
}

//! Status and health check endpoints.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

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

async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatusResponse>, AppError> {
    let status = state.sync_engine.get_status().map_err(|e| {
        AppError::Internal(format!("failed to get sync status: {}", e))
    })?;

    Ok(Json(StatusResponse {
        state: status.state.to_string(),
        last_sync_at: status.last_sync_at.map(|t| t.to_rfc3339()),
        last_svn_revision: status.last_svn_revision,
        last_git_hash: status.last_git_hash,
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

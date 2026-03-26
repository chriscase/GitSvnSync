//! Configuration API endpoints.

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::auth::validate_session;
use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ConfigResponse {
    daemon: DaemonConfigView,
    svn: SvnConfigView,
    github: GitHubConfigView,
    web: WebConfigView,
    sync: SyncConfigView,
}

#[derive(Serialize)]
struct DaemonConfigView {
    poll_interval_secs: u64,
    log_level: String,
    data_dir: String,
}

#[derive(Serialize)]
struct SvnConfigView {
    url: String,
    username: String,
    password: String, // redacted
    trunk_path: String,
}

#[derive(Serialize)]
struct GitHubConfigView {
    api_url: String,
    repo: String,
    token: String, // redacted
    default_branch: String,
}

#[derive(Serialize)]
struct WebConfigView {
    listen: String,
    auth_mode: String,
}

#[derive(Serialize)]
struct SyncConfigView {
    mode: String,
    auto_merge: bool,
    sync_tags: bool,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Identity mapping types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
struct AuthorMapping {
    svn_username: String,
    name: String,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<String>,
}

#[derive(Deserialize)]
struct UpdateMappingsRequest {
    mappings: Vec<AuthorMapping>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/config", get(get_config))
        .route(
            "/api/config/identity",
            get(get_identity_mappings).put(update_identity_mappings),
        )
}

async fn get_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ConfigResponse>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let cfg = &state.config;

    Ok(Json(ConfigResponse {
        daemon: DaemonConfigView {
            poll_interval_secs: cfg.daemon.poll_interval_secs,
            log_level: cfg.daemon.log_level.clone(),
            data_dir: cfg.daemon.data_dir.display().to_string(),
        },
        svn: SvnConfigView {
            url: cfg.svn.url.clone(),
            username: cfg.svn.username.clone(),
            password: "***REDACTED***".into(),
            trunk_path: cfg.svn.trunk_path.clone(),
        },
        github: GitHubConfigView {
            api_url: cfg.github.api_url.clone(),
            repo: cfg.github.repo.clone(),
            token: "***REDACTED***".into(),
            default_branch: cfg.github.default_branch.clone(),
        },
        web: WebConfigView {
            listen: cfg.web.listen.clone(),
            auth_mode: format!("{:?}", cfg.web.auth_mode),
        },
        sync: SyncConfigView {
            mode: format!("{:?}", cfg.sync.mode),
            auto_merge: cfg.sync.auto_merge,
            sync_tags: cfg.sync.sync_tags,
        },
    }))
}

async fn get_identity_mappings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuthorMapping>>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let value: Option<String> = db
        .get_state("identity_mappings")
        .map_err(|e| AppError::Internal(format!("db error: {}", e)))?;

    match value {
        Some(json_str) => {
            let mappings: Vec<AuthorMapping> = serde_json::from_str(&json_str)
                .map_err(|e| AppError::Internal(format!("parse identity mappings: {}", e)))?;
            Ok(Json(mappings))
        }
        None => Ok(Json(vec![])),
    }
}

async fn update_identity_mappings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<UpdateMappingsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let json_str = serde_json::to_string(&body.mappings)
        .map_err(|e| AppError::Internal(format!("serialize mappings: {}", e)))?;

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    db.set_state("identity_mappings", &json_str)
        .map_err(|e| AppError::Internal(format!("db error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "count": body.mappings.len(),
    })))
}

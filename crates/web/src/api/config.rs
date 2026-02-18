//! Configuration API endpoints.

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

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

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/config", get(get_config))
}

async fn get_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ConfigResponse>, AppError> {
    validate_session(&state, headers.get("authorization").and_then(|v| v.to_str().ok())).await?;

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

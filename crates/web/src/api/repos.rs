//! Repository management API endpoints (multi-repo support).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::auth::{validate_session, validate_session_with_role};
use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateRepoRequest {
    name: String,
    svn_url: String,
    #[serde(default)]
    svn_branch: String,
    #[serde(default)]
    svn_username: String,
    #[serde(default = "default_github")]
    git_provider: String,
    #[serde(default)]
    git_api_url: String,
    #[serde(default)]
    git_repo: String,
    #[serde(default = "default_main")]
    git_branch: String,
    #[serde(default = "default_direct")]
    sync_mode: String,
    #[serde(default = "default_60")]
    poll_interval_secs: i64,
    #[serde(default)]
    lfs_threshold_mb: i64,
    #[serde(default = "default_true")]
    auto_merge: bool,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_github() -> String {
    "github".to_string()
}

fn default_main() -> String {
    "main".to_string()
}

fn default_direct() -> String {
    "direct".to_string()
}

fn default_60() -> i64 {
    60
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct UpdateRepoRequest {
    name: Option<String>,
    svn_url: Option<String>,
    svn_branch: Option<String>,
    svn_username: Option<String>,
    git_provider: Option<String>,
    git_api_url: Option<String>,
    git_repo: Option<String>,
    git_branch: Option<String>,
    sync_mode: Option<String>,
    poll_interval_secs: Option<i64>,
    lfs_threshold_mb: Option<i64>,
    auto_merge: Option<bool>,
    enabled: Option<bool>,
}

#[derive(Serialize)]
struct RepoSummary {
    id: String,
    name: String,
    svn_url: String,
    svn_branch: String,
    git_provider: String,
    git_repo: String,
    git_branch: String,
    sync_mode: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
    /// Current sync status label, if available.
    status: String,
}

#[derive(Serialize)]
struct RepoDetail {
    id: String,
    name: String,
    svn_url: String,
    svn_branch: String,
    svn_username: String,
    git_provider: String,
    git_api_url: String,
    git_repo: String,
    git_branch: String,
    sync_mode: String,
    poll_interval_secs: i64,
    lfs_threshold_mb: i64,
    auto_merge: bool,
    enabled: bool,
    created_by: Option<String>,
    created_at: String,
    updated_at: String,
    /// Current sync status label, if available.
    status: String,
}

impl From<gitsvnsync_core::models::Repository> for RepoDetail {
    fn from(r: gitsvnsync_core::models::Repository) -> Self {
        Self {
            id: r.id,
            name: r.name,
            svn_url: r.svn_url,
            svn_branch: r.svn_branch,
            svn_username: r.svn_username,
            git_provider: r.git_provider,
            git_api_url: r.git_api_url,
            git_repo: r.git_repo,
            git_branch: r.git_branch,
            sync_mode: r.sync_mode,
            poll_interval_secs: r.poll_interval_secs,
            lfs_threshold_mb: r.lfs_threshold_mb,
            auto_merge: r.auto_merge,
            enabled: r.enabled,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
            status: "unknown".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Credential request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SaveCredentialsRequest {
    svn_password: Option<String>,
    git_token: Option<String>,
}

#[derive(Serialize)]
struct CredentialStatus {
    svn_password_set: bool,
    git_token_set: bool,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/repos", get(list_repos))
        .route("/api/repos", post(create_repo))
        .route("/api/repos/:id", get(get_repo))
        .route("/api/repos/:id", put(update_repo))
        .route("/api/repos/:id", delete(delete_repo))
        .route("/api/repos/:id/sync", post(trigger_sync))
        .route("/api/repos/:id/credentials", get(get_credentials))
        .route("/api/repos/:id/credentials", post(save_credentials))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_repos(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<RepoSummary>>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let db = &state.db;

    let repos = db
        .list_repositories()
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    let summaries: Vec<RepoSummary> = repos
        .into_iter()
        .map(|r| RepoSummary {
            id: r.id,
            name: r.name,
            svn_url: r.svn_url,
            svn_branch: r.svn_branch,
            git_provider: r.git_provider,
            git_repo: r.git_repo,
            git_branch: r.git_branch,
            sync_mode: r.sync_mode,
            enabled: r.enabled,
            created_at: r.created_at,
            updated_at: r.updated_at,
            status: "unknown".to_string(),
        })
        .collect();

    Ok(Json(summaries))
}

async fn create_repo(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateRepoRequest>,
) -> Result<Json<RepoDetail>, AppError> {
    let (user_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    if body.name.is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    if body.svn_url.is_empty() {
        return Err(AppError::BadRequest("svn_url is required".into()));
    }

    let now = Utc::now().to_rfc3339();
    let repo = gitsvnsync_core::models::Repository {
        id: Uuid::new_v4().to_string(),
        name: body.name,
        svn_url: body.svn_url,
        svn_branch: body.svn_branch,
        svn_username: body.svn_username,
        git_provider: body.git_provider,
        git_api_url: body.git_api_url,
        git_repo: body.git_repo,
        git_branch: body.git_branch,
        sync_mode: body.sync_mode,
        poll_interval_secs: body.poll_interval_secs,
        lfs_threshold_mb: body.lfs_threshold_mb,
        auto_merge: body.auto_merge,
        enabled: body.enabled,
        created_by: Some(user_id),
        created_at: now.clone(),
        updated_at: now,
    };

    let db = &state.db;

    db.insert_repository(&repo)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(RepoDetail::from(repo)))
}

async fn get_repo(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<RepoDetail>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let db = &state.db;

    let repo = db
        .get_repository(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("repository not found".into()))?;

    Ok(Json(RepoDetail::from(repo)))
}

async fn update_repo(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateRepoRequest>,
) -> Result<Json<RepoDetail>, AppError> {
    let (_user_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    let db = &state.db;

    let existing = db
        .get_repository(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("repository not found".into()))?;

    let now = Utc::now().to_rfc3339();
    let updated = gitsvnsync_core::models::Repository {
        id: id.clone(),
        name: body.name.unwrap_or(existing.name),
        svn_url: body.svn_url.unwrap_or(existing.svn_url),
        svn_branch: body.svn_branch.unwrap_or(existing.svn_branch),
        svn_username: body.svn_username.unwrap_or(existing.svn_username),
        git_provider: body.git_provider.unwrap_or(existing.git_provider),
        git_api_url: body.git_api_url.unwrap_or(existing.git_api_url),
        git_repo: body.git_repo.unwrap_or(existing.git_repo),
        git_branch: body.git_branch.unwrap_or(existing.git_branch),
        sync_mode: body.sync_mode.unwrap_or(existing.sync_mode),
        poll_interval_secs: body.poll_interval_secs.unwrap_or(existing.poll_interval_secs),
        lfs_threshold_mb: body.lfs_threshold_mb.unwrap_or(existing.lfs_threshold_mb),
        auto_merge: body.auto_merge.unwrap_or(existing.auto_merge),
        enabled: body.enabled.unwrap_or(existing.enabled),
        created_by: existing.created_by,
        created_at: existing.created_at,
        updated_at: now,
    };

    db.update_repository(&updated)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(RepoDetail::from(updated)))
}

async fn delete_repo(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (_user_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    let db = &state.db;

    // Soft delete: disable the repository rather than removing it.
    let existing = db
        .get_repository(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("repository not found".into()))?;

    let disabled = gitsvnsync_core::models::Repository {
        enabled: false,
        updated_at: Utc::now().to_rfc3339(),
        ..existing
    };

    db.update_repository(&disabled)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "repository disabled",
    })))
}

async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    // Verify the repository exists.
    {
        let db = &state.db;
        let _repo = db
            .get_repository(&id)
            .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
            .ok_or_else(|| AppError::NotFound("repository not found".into()))?;
    }

    // For now, just log that sync was requested. The actual per-repo sync
    // engine will be wired in a later phase.
    tracing::info!(repo_id = %id, "manual sync triggered for repository");

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "Sync triggered",
    })))
}

async fn get_credentials(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CredentialStatus>, AppError> {
    validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let db = &state.db;

    // Verify repo exists
    let _repo = db
        .get_repository(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("repository not found".into()))?;

    let svn_key = format!("secret_svn_password_{}", id);
    let git_key = format!("secret_git_token_{}", id);

    let svn_set = db
        .get_state(&svn_key)
        .unwrap_or(None)
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let git_set = db
        .get_state(&git_key)
        .unwrap_or(None)
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    // Fall back to global keys for repos that were migrated from single-repo config
    let svn_set = svn_set
        || db
            .get_state("secret_svn_password")
            .unwrap_or(None)
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    let git_set = git_set
        || db
            .get_state("secret_git_token")
            .unwrap_or(None)
            .map(|v| !v.is_empty())
            .unwrap_or(false);

    Ok(Json(CredentialStatus {
        svn_password_set: svn_set,
        git_token_set: git_set,
    }))
}

async fn save_credentials(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveCredentialsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (_user_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    let db = &state.db;

    // Verify repo exists
    let _repo = db
        .get_repository(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("repository not found".into()))?;

    let now = Utc::now().to_rfc3339();

    if let Some(ref password) = body.svn_password {
        if !password.is_empty() {
            let key = format!("secret_svn_password_{}", id);
            let _ = db.conn().execute(
                "INSERT OR REPLACE INTO kv_state (key, value, updated_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, password, now],
            );
            // Also update global key for backward compat with current sync engine
            let _ = db.conn().execute(
                "INSERT OR REPLACE INTO kv_state (key, value, updated_at) VALUES ('secret_svn_password', ?1, ?2)",
                rusqlite::params![password, now],
            );
            tracing::info!(repo_id = %id, "SVN password stored for repository");
        }
    }

    if let Some(ref token) = body.git_token {
        if !token.is_empty() {
            let key = format!("secret_git_token_{}", id);
            let _ = db.conn().execute(
                "INSERT OR REPLACE INTO kv_state (key, value, updated_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, token, now],
            );
            // Also update global key for backward compat
            let _ = db.conn().execute(
                "INSERT OR REPLACE INTO kv_state (key, value, updated_at) VALUES ('secret_git_token', ?1, ?2)",
                rusqlite::params![token, now],
            );
            tracing::info!(repo_id = %id, "Git token stored for repository");
        }
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

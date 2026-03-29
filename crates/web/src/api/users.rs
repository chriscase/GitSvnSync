//! User management and credential storage endpoints.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::auth::validate_session_with_role;
use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub password: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "user".to_string()
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub password: Option<String>,
}

#[derive(Serialize)]
struct UserResponse {
    id: String,
    username: String,
    display_name: String,
    email: String,
    role: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

impl From<gitsvnsync_core::models::User> for UserResponse {
    fn from(u: gitsvnsync_core::models::User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            display_name: u.display_name,
            email: u.email,
            role: u.role,
            enabled: u.enabled,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateCredentialRequest {
    pub service: String,
    pub server_url: String,
    pub username: String,
    pub value: String,
}

#[derive(Serialize)]
struct CredentialSummary {
    id: String,
    service: String,
    server_url: String,
    username: String,
    created_at: String,
    updated_at: String,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users", post(create_user))
        .route("/api/users/{id}", get(get_user))
        .route("/api/users/{id}", put(update_user))
        .route("/api/users/{id}", delete(disable_user))
        .route("/api/users/{id}/credentials", get(list_credentials))
        .route("/api/users/{id}/credentials", post(create_credential))
        .route(
            "/api/users/{id}/credentials/{cred_id}",
            delete(delete_credential),
        )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let (_user_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let users = db
        .list_users()
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(users.into_iter().map(UserResponse::from).collect()))
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let (_user_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    if body.username.is_empty() {
        return Err(AppError::BadRequest("username is required".into()));
    }
    if body.password.is_empty() {
        return Err(AppError::BadRequest("password is required".into()));
    }
    if !matches!(body.role.as_str(), "admin" | "user") {
        return Err(AppError::BadRequest(
            "role must be 'admin' or 'user'".into(),
        ));
    }

    let password_hash = gitsvnsync_core::crypto::hash_password(&body.password)
        .map_err(|e| AppError::Internal(format!("password hashing error: {}", e)))?;

    let now = Utc::now().to_rfc3339();
    let user = gitsvnsync_core::models::User {
        id: Uuid::new_v4().to_string(),
        username: body.username,
        display_name: body.display_name,
        email: body.email,
        password_hash,
        role: body.role,
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    };

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    db.insert_user(&user)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(UserResponse::from(user)))
}

async fn get_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let (caller_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    // Admin can view anyone; users can only view themselves
    if role != "admin" && caller_id != id {
        return Err(AppError::Unauthorized("access denied".into()));
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let user = db
        .get_user(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    Ok(Json(UserResponse::from(user)))
}

async fn update_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let (caller_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    // Admin can update anyone; users can only update themselves
    if role != "admin" && caller_id != id {
        return Err(AppError::Unauthorized("access denied".into()));
    }

    // Non-admins cannot change role or enabled status
    if role != "admin" && (body.role.is_some() || body.enabled.is_some()) {
        return Err(AppError::Unauthorized(
            "only admins can change role or enabled status".into(),
        ));
    }

    if let Some(ref r) = body.role {
        if !matches!(r.as_str(), "admin" | "user") {
            return Err(AppError::BadRequest(
                "role must be 'admin' or 'user'".into(),
            ));
        }
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let existing = db
        .get_user(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    // Update password if provided
    if let Some(ref new_password) = body.password {
        if new_password.is_empty() {
            return Err(AppError::BadRequest("password cannot be empty".into()));
        }
        let hash = gitsvnsync_core::crypto::hash_password(new_password)
            .map_err(|e| AppError::Internal(format!("password hashing error: {}", e)))?;
        db.update_user_password(&id, &hash)
            .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;
    }

    // Update other fields
    let display_name = body.display_name.as_deref().unwrap_or(&existing.display_name);
    let email = body.email.as_deref().unwrap_or(&existing.email);
    let user_role = body.role.as_deref().unwrap_or(&existing.role);
    let enabled = body.enabled.unwrap_or(existing.enabled);

    db.update_user(&id, display_name, email, user_role, enabled)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    // Re-fetch the updated user
    let updated = db
        .get_user(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::Internal("user disappeared after update".into()))?;

    Ok(Json(UserResponse::from(updated)))
}

async fn disable_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (_caller_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" {
        return Err(AppError::Unauthorized("admin access required".into()));
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    db.disable_user(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "user disabled",
    })))
}

// ---------------------------------------------------------------------------
// Credential handlers
// ---------------------------------------------------------------------------

async fn list_credentials(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<CredentialSummary>>, AppError> {
    let (caller_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" && caller_id != id {
        return Err(AppError::Unauthorized("access denied".into()));
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let creds = db
        .list_user_credentials(&id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    let summaries: Vec<CredentialSummary> = creds
        .into_iter()
        .map(|c| CredentialSummary {
            id: c.id,
            service: c.service,
            server_url: c.server_url,
            username: c.username,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();

    Ok(Json(summaries))
}

async fn create_credential(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<CreateCredentialRequest>,
) -> Result<Json<CredentialSummary>, AppError> {
    let (caller_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" && caller_id != id {
        return Err(AppError::Unauthorized("access denied".into()));
    }

    if body.service.is_empty() || body.server_url.is_empty() || body.username.is_empty() {
        return Err(AppError::BadRequest(
            "service, server_url, and username are required".into(),
        ));
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    // Get encryption key
    let enc_key = gitsvnsync_core::crypto::get_or_create_encryption_key(&db)
        .map_err(|e| AppError::Internal(format!("encryption key error: {}", e)))?;

    // Encrypt the credential value
    let (encrypted_value, nonce) =
        gitsvnsync_core::crypto::encrypt_credential(&body.value, &enc_key)
            .map_err(|e| AppError::Internal(format!("encryption error: {}", e)))?;

    let now = Utc::now().to_rfc3339();
    let cred = gitsvnsync_core::models::UserCredential {
        id: Uuid::new_v4().to_string(),
        user_id: id,
        service: body.service.clone(),
        server_url: body.server_url.clone(),
        username: body.username.clone(),
        encrypted_value,
        nonce,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    db.insert_user_credential(&cred)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(CredentialSummary {
        id: cred.id,
        service: body.service,
        server_url: body.server_url,
        username: body.username,
        created_at: now.clone(),
        updated_at: now,
    }))
}

async fn delete_credential(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((id, cred_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (caller_id, role) = validate_session_with_role(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    if role != "admin" && caller_id != id {
        return Err(AppError::Unauthorized("access denied".into()));
    }

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    // Verify the credential belongs to the user
    let cred = db
        .get_user_credential(&cred_id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound("credential not found".into()))?;

    if cred.user_id != id {
        return Err(AppError::NotFound("credential not found".into()));
    }

    db.delete_user_credential(&cred_id)
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "credential deleted",
    })))
}

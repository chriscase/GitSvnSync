//! Authentication endpoints (simple password-based sessions).

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    expires_at: String,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    pub token: String,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/verify", post(verify))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let configured_password = state.config.web.admin_password.as_deref().unwrap_or("");

    // If no admin password is configured, authentication is disabled
    if configured_password.is_empty() {
        return Err(AppError::BadRequest(
            "authentication is not configured (no admin password set)".into(),
        ));
    }

    // Constant-time comparison to prevent timing attacks.
    let password_matches = body.password.len() == configured_password.len()
        && body
            .password
            .bytes()
            .zip(configured_password.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;

    if !password_matches {
        return Err(AppError::Unauthorized("invalid password".into()));
    }

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(token.clone(), expires_at);
    }

    Ok(Json(LoginResponse {
        token,
        expires_at: expires_at.to_rfc3339(),
    }))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut sessions = state.sessions.write().await;
    sessions.remove(&body.token);

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "logged out",
    })))
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub token: String,
}

async fn verify(
    State(state): State<Arc<AppState>>,
    Json(body): Json<VerifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sessions = state.sessions.read().await;

    if let Some(expires_at) = sessions.get(&body.token) {
        if *expires_at > Utc::now() {
            return Ok(Json(serde_json::json!({
                "valid": true,
                "expires_at": expires_at.to_rfc3339(),
            })));
        }
    }

    Ok(Json(serde_json::json!({
        "valid": false,
    })))
}

/// Middleware helper to validate a session token from the Authorization header.
///
/// Call this from handlers that require authentication. Returns `Ok(())` if
/// the token is valid, or `Err(AppError::Unauthorized)` otherwise.
///
/// Also opportunistically prunes expired sessions to prevent unbounded growth.
pub async fn validate_session(
    state: &Arc<AppState>,
    auth_header: Option<&str>,
) -> Result<(), AppError> {
    // If no admin password is configured, skip authentication entirely
    if state.config.web.admin_password.is_none() {
        return Ok(());
    }

    let token = auth_header
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("missing or invalid Authorization header".into()))?;

    let now = Utc::now();

    // Validate and opportunistically prune expired sessions
    let mut sessions = state.sessions.write().await;
    sessions.retain(|_, expiry| *expiry > now);

    if let Some(expires_at) = sessions.get(token) {
        if *expires_at > now {
            return Ok(());
        }
    }

    Err(AppError::Unauthorized("session expired or invalid".into()))
}

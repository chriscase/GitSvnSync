//! Webhook receiver endpoints for GitHub and SVN push notifications.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// GitHub webhook types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GitHubPushPayload {
    #[serde(rename = "ref")]
    git_ref: String,
    commits: Option<Vec<GitHubCommitPayload>>,
    repository: Option<GitHubRepoPayload>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubCommitPayload {
    id: String,
    message: String,
    author: GitHubAuthorPayload,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubAuthorPayload {
    name: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepoPayload {
    full_name: String,
}

// ---------------------------------------------------------------------------
// SVN webhook types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SvnPostCommitPayload {
    revision: i64,
    author: String,
    message: String,
}

#[derive(Serialize)]
struct WebhookResponse {
    ok: bool,
    message: String,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/webhook/github", post(github_webhook))
        .route("/webhook/svn", post(svn_webhook))
}

async fn github_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>, AppError> {
    // Verify webhook signature if a secret is configured
    if let Some(ref _secret_env) = state.config.github.webhook_secret_env {
        let signature = headers
            .get("x-hub-signature-256")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("missing X-Hub-Signature-256 header".into()))?;

        verify_github_signature(
            &body,
            signature,
            state.config.github.webhook_secret.as_deref(),
        )
        .map_err(|e| AppError::Unauthorized(format!("webhook verification failed: {}", e)))?;
    }

    // Parse the event type
    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    if event_type != "push" {
        info!(event_type, "ignoring non-push GitHub webhook event");
        return Ok(Json(WebhookResponse {
            ok: true,
            message: format!("event type '{}' ignored", event_type),
        }));
    }

    // Parse the payload
    let payload: GitHubPushPayload = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON payload: {}", e)))?;

    let repo_name = payload
        .repository
        .as_ref()
        .map(|r| r.full_name.as_str())
        .unwrap_or("unknown");

    let commit_count = payload.commits.as_ref().map(|c| c.len()).unwrap_or(0);

    info!(
        repo = repo_name,
        git_ref = %payload.git_ref,
        commits = commit_count,
        "received GitHub push webhook"
    );

    // Trigger an immediate sync
    if let Err(e) = state.sync_trigger.send(()).await {
        warn!("failed to trigger sync from webhook: {}", e);
    }

    // Broadcast notification
    let update = serde_json::json!({
        "type": "webhook_received",
        "source": "github",
        "ref": payload.git_ref,
        "commits": commit_count,
    });
    let _ = state.ws_broadcast.send(update.to_string());

    Ok(Json(WebhookResponse {
        ok: true,
        message: format!(
            "push event received, {} commits, sync triggered",
            commit_count
        ),
    }))
}

async fn svn_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SvnPostCommitPayload>,
) -> Result<Json<WebhookResponse>, AppError> {
    info!(
        revision = payload.revision,
        author = %payload.author,
        "received SVN post-commit webhook"
    );

    // Trigger an immediate sync
    if let Err(e) = state.sync_trigger.send(()).await {
        warn!("failed to trigger sync from SVN webhook: {}", e);
    }

    // Broadcast notification
    let update = serde_json::json!({
        "type": "webhook_received",
        "source": "svn",
        "revision": payload.revision,
        "author": payload.author,
    });
    let _ = state.ws_broadcast.send(update.to_string());

    Ok(Json(WebhookResponse {
        ok: true,
        message: format!("SVN revision {} received, sync triggered", payload.revision),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn verify_github_signature(
    payload: &[u8],
    signature: &str,
    secret: Option<&str>,
) -> Result<(), String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let secret = secret.ok_or("webhook secret not configured")?;

    let sig_hex = signature
        .strip_prefix("sha256=")
        .ok_or("signature must start with sha256=")?;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("HMAC init failed: {}", e))?;
    mac.update(payload);

    let expected = hex::decode(sig_hex).map_err(|e| format!("invalid hex in signature: {}", e))?;

    mac.verify_slice(&expected)
        .map_err(|_| "signature mismatch".to_string())
}

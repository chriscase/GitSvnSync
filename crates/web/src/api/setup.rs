//! Setup wizard API endpoints for testing connections.

use std::sync::Arc;

use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::status::AppError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct TestSvnRequest {
    pub url: String,
    pub username: String,
}

#[derive(Deserialize)]
pub struct TestGitRequest {
    pub api_url: String,
    pub repo: String,
    pub provider: String,
}

#[derive(Serialize)]
pub struct TestConnectionResponse {
    pub ok: bool,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/setup/test-svn", post(test_svn_connection))
        .route("/api/setup/test-git", post(test_git_connection))
}

async fn test_svn_connection(
    Json(body): Json<TestSvnRequest>,
) -> Result<Json<TestConnectionResponse>, AppError> {
    let url = body.url.trim().to_string();
    let username = body.username.trim().to_string();

    if url.is_empty() {
        return Ok(Json(TestConnectionResponse {
            ok: false,
            message: "URL is empty".into(),
        }));
    }

    // Try to run `svn info` to test connectivity
    let result = tokio::process::Command::new("svn")
        .args(["info", "--non-interactive", "--username", &username, &url])
        .output()
        .await;

    match result {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Extract repository root or UUID for a friendly message
                let info = stdout
                    .lines()
                    .find(|l| l.starts_with("Repository Root:") || l.starts_with("URL:"))
                    .map(|l| l.trim().to_string())
                    .unwrap_or_else(|| "SVN server responded successfully".into());
                Ok(Json(TestConnectionResponse {
                    ok: true,
                    message: info,
                }))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let msg = stderr
                    .lines()
                    .find(|l| l.contains("E1") || l.contains("Unable"))
                    .unwrap_or("SVN command failed")
                    .trim()
                    .to_string();
                Ok(Json(TestConnectionResponse {
                    ok: false,
                    message: msg,
                }))
            }
        }
        Err(e) => Ok(Json(TestConnectionResponse {
            ok: false,
            message: format!("Failed to run svn command: {}", e),
        })),
    }
}

async fn test_git_connection(
    Json(body): Json<TestGitRequest>,
) -> Result<Json<TestConnectionResponse>, AppError> {
    let api_url = body.api_url.trim().trim_end_matches('/').to_string();
    let repo = body.repo.trim().to_string();

    if api_url.is_empty() || repo.is_empty() {
        return Ok(Json(TestConnectionResponse {
            ok: false,
            message: "API URL and repository are required".into(),
        }));
    }

    // Build the URL based on provider
    let check_url = if body.provider == "gitea" {
        format!("{}/repos/{}", api_url, repo)
    } else {
        // GitHub or GitHub Enterprise
        format!("{}/repos/{}", api_url, repo)
    };

    // Make HTTP request to check if repo exists
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("http client error: {}", e)))?;

    match client.get(&check_url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                // Try to extract repo name from response
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let name = json
                        .get("full_name")
                        .or_else(|| json.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&repo);
                    Ok(Json(TestConnectionResponse {
                        ok: true,
                        message: format!("Repository found: {}", name),
                    }))
                } else {
                    Ok(Json(TestConnectionResponse {
                        ok: true,
                        message: "Repository is accessible".into(),
                    }))
                }
            } else if status.as_u16() == 404 {
                Ok(Json(TestConnectionResponse {
                    ok: false,
                    message: "Repository not found (404). Check the repo name and API URL.".into(),
                }))
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                Ok(Json(TestConnectionResponse {
                    ok: false,
                    message: format!("Authentication required ({}). The API URL is reachable but the repo may be private.", status),
                }))
            } else {
                Ok(Json(TestConnectionResponse {
                    ok: false,
                    message: format!("Server returned status {}", status),
                }))
            }
        }
        Err(e) => {
            let msg = if e.is_connect() {
                format!("Cannot connect to {}: connection refused or host not reachable", api_url)
            } else if e.is_timeout() {
                "Connection timed out after 10 seconds".into()
            } else {
                format!("Request failed: {}", e)
            };
            Ok(Json(TestConnectionResponse {
                ok: false,
                message: msg,
            }))
        }
    }
}

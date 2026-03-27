//! Setup wizard API endpoints.
//!
//! - Test SVN / Git connections
//! - Apply configuration from wizard data (generates TOML server-side)
//! - Trigger full SVN→Git history import with progress tracking
//! - Poll import status

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use gitsvnsync_core::config::AppConfig;
use gitsvnsync_core::db::Database;
use gitsvnsync_core::file_policy::FilePolicy;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::identity::IdentityMapper;
use gitsvnsync_core::import::{self, ImportConfig, ImportProgress, ImportState};
use gitsvnsync_core::svn::SvnClient;

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

#[derive(Deserialize)]
pub struct ApplyConfigRequest {
    // SVN
    pub svn_url: String,
    pub svn_username: String,
    pub svn_password_env: Option<String>,
    pub svn_layout: Option<String>,
    pub svn_trunk_path: Option<String>,
    pub svn_branches_path: Option<String>,
    pub svn_tags_path: Option<String>,

    // Git
    pub git_provider: Option<String>,
    pub git_api_url: String,
    pub git_repo: String,
    pub git_token_env: String,
    pub git_default_branch: Option<String>,

    // Sync
    pub sync_mode: Option<String>,
    pub sync_auto_merge: Option<bool>,
    pub sync_tags: Option<bool>,

    // File policy
    pub max_file_size: Option<u64>,
    pub lfs_threshold: Option<u64>,
    pub lfs_patterns: Option<Vec<String>>,
    pub ignore_patterns: Option<Vec<String>>,

    // Identity
    pub identity_email_domain: Option<String>,
    pub identity_mapping_file: Option<String>,
    pub identity_mappings: Option<Vec<IdentityMappingEntry>>,

    // Daemon
    pub daemon_poll_interval: Option<u64>,
    pub daemon_log_level: Option<String>,
    pub daemon_data_dir: Option<String>,

    // Web
    pub web_listen: Option<String>,
    pub web_admin_password_env: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct IdentityMappingEntry {
    pub svn_username: String,
    pub name: String,
    pub email: String,
}

#[derive(Serialize)]
pub struct ApplyConfigResponse {
    pub ok: bool,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct ImportActionResponse {
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
        .route("/api/setup/apply", post(apply_config))
        .route("/api/setup/import", post(start_import))
        .route("/api/setup/import/status", get(import_status))
        .route("/api/setup/import/cancel", post(cancel_import))
}

// ---------------------------------------------------------------------------
// Test connections
// ---------------------------------------------------------------------------

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

    let result = tokio::process::Command::new("svn")
        .args(["info", "--non-interactive", "--username", &username, &url])
        .output()
        .await;

    match result {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
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

    let check_url = if body.provider == "gitea" {
        format!("{}/repos/{}", api_url, repo)
    } else {
        format!("{}/repos/{}", api_url, repo)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("http client error: {}", e)))?;

    match client.get(&check_url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
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
                    message: "Repository not found (404). Check the repo name and API URL."
                        .into(),
                }))
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                Ok(Json(TestConnectionResponse {
                    ok: false,
                    message: format!(
                        "Authentication required ({}). The API URL is reachable but the repo may be private.",
                        status
                    ),
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
                format!(
                    "Cannot connect to {}: connection refused or host not reachable",
                    api_url
                )
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

// ---------------------------------------------------------------------------
// Apply configuration
// ---------------------------------------------------------------------------

fn generate_toml(data: &ApplyConfigRequest) -> String {
    let mut lines = Vec::new();

    lines.push("[daemon]".into());
    lines.push(format!(
        "poll_interval_secs = {}",
        data.daemon_poll_interval.unwrap_or(60)
    ));
    lines.push(format!(
        "log_level = \"{}\"",
        data.daemon_log_level.as_deref().unwrap_or("info")
    ));
    lines.push(format!(
        "data_dir = \"{}\"",
        data.daemon_data_dir
            .as_deref()
            .unwrap_or("/var/lib/gitsvnsync")
    ));

    lines.push(String::new());
    lines.push("[svn]".into());
    lines.push(format!("url = \"{}\"", data.svn_url));
    lines.push(format!("username = \"{}\"", data.svn_username));
    if let Some(ref pe) = data.svn_password_env {
        if !pe.is_empty() {
            lines.push(format!("password_env = \"{}\"", pe));
        }
    }
    lines.push(format!(
        "layout = \"{}\"",
        data.svn_layout.as_deref().unwrap_or("standard")
    ));
    lines.push(format!(
        "trunk_path = \"{}\"",
        data.svn_trunk_path.as_deref().unwrap_or("trunk")
    ));
    if let Some(ref bp) = data.svn_branches_path {
        if !bp.is_empty() {
            lines.push(format!("branches_path = \"{}\"", bp));
        }
    }
    if let Some(ref tp) = data.svn_tags_path {
        if !tp.is_empty() {
            lines.push(format!("tags_path = \"{}\"", tp));
        }
    }

    lines.push(String::new());
    lines.push("[github]".into());
    lines.push(format!("api_url = \"{}\"", data.git_api_url));
    lines.push(format!("repo = \"{}\"", data.git_repo));
    lines.push(format!("token_env = \"{}\"", data.git_token_env));
    lines.push(format!(
        "default_branch = \"{}\"",
        data.git_default_branch.as_deref().unwrap_or("main")
    ));
    if let Some(ref p) = data.git_provider {
        if p != "github" {
            lines.push(format!("provider = \"{}\"", p));
        }
    }

    // Identity
    let has_identity = data.identity_email_domain.is_some()
        || data.identity_mapping_file.is_some();
    if has_identity {
        lines.push(String::new());
        lines.push("[identity]".into());
        if let Some(ref ed) = data.identity_email_domain {
            if !ed.is_empty() {
                lines.push(format!("email_domain = \"{}\"", ed));
            }
        }
        if let Some(ref mf) = data.identity_mapping_file {
            if !mf.is_empty() {
                lines.push(format!("mapping_file = \"{}\"", mf));
            }
        }
    }

    lines.push(String::new());
    lines.push("[web]".into());
    lines.push(format!(
        "listen = \"{}\"",
        data.web_listen.as_deref().unwrap_or("0.0.0.0:8080")
    ));
    lines.push("auth_mode = \"simple\"".into());
    if let Some(ref ape) = data.web_admin_password_env {
        if !ape.is_empty() {
            lines.push(format!("admin_password_env = \"{}\"", ape));
        }
    }

    lines.push(String::new());
    lines.push("[sync]".into());
    lines.push(format!(
        "mode = \"{}\"",
        data.sync_mode.as_deref().unwrap_or("direct")
    ));
    lines.push(format!(
        "auto_merge = {}",
        data.sync_auto_merge.unwrap_or(true)
    ));
    lines.push(format!(
        "sync_tags = {}",
        data.sync_tags.unwrap_or(true)
    ));

    // File policy fields
    if let Some(mfs) = data.max_file_size {
        if mfs > 0 {
            lines.push(format!("max_file_size = {}", mfs));
        }
    }
    if let Some(lt) = data.lfs_threshold {
        if lt > 0 {
            lines.push(format!("lfs_threshold = {}", lt));
        }
    }
    if let Some(ref lp) = data.lfs_patterns {
        if !lp.is_empty() {
            let patterns: Vec<String> = lp.iter().map(|p| format!("\"{}\"", p)).collect();
            lines.push(format!("lfs_patterns = [{}]", patterns.join(", ")));
        }
    }
    if let Some(ref ip) = data.ignore_patterns {
        if !ip.is_empty() {
            let patterns: Vec<String> = ip.iter().map(|p| format!("\"{}\"", p)).collect();
            lines.push(format!("ignore_patterns = [{}]", patterns.join(", ")));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

async fn apply_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ApplyConfigRequest>,
) -> Result<Json<ApplyConfigResponse>, AppError> {
    let mut warnings = Vec::new();

    // Generate TOML from structured data
    let toml_content = generate_toml(&body);

    // Validate by parsing
    match toml::from_str::<AppConfig>(&toml_content) {
        Ok(_) => {}
        Err(e) => {
            return Ok(Json(ApplyConfigResponse {
                ok: false,
                message: format!("Generated config is invalid: {}", e),
                warnings: vec![],
            }));
        }
    }

    // Write identity mapping file if mappings are provided
    if let Some(ref mappings) = body.identity_mappings {
        if !mappings.is_empty() {
            let mapping_path = body
                .identity_mapping_file
                .as_deref()
                .unwrap_or("identity-mappings.toml");
            let mut mapping_lines = Vec::new();
            for m in mappings {
                mapping_lines.push(format!("[mappings.\"{}\"]", m.svn_username));
                mapping_lines.push(format!("name = \"{}\"", m.name));
                mapping_lines.push(format!("email = \"{}\"", m.email));
                mapping_lines.push(String::new());
            }
            let mapping_content = mapping_lines.join("\n");

            // Resolve path relative to config file directory
            let config_dir = state.config_path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let full_mapping_path = if std::path::Path::new(mapping_path).is_absolute() {
                std::path::PathBuf::from(mapping_path)
            } else {
                config_dir.join(mapping_path)
            };

            if let Err(e) = std::fs::write(&full_mapping_path, &mapping_content) {
                warnings.push(format!(
                    "Failed to write identity mapping file: {}",
                    e
                ));
            } else {
                info!(
                    path = %full_mapping_path.display(),
                    count = mappings.len(),
                    "Wrote identity mappings file"
                );
            }

            // Also store in DB for the dashboard to display
            let db = state
                .db
                .lock()
                .map_err(|e| AppError::Internal(format!("db lock error: {}", e)))?;
            let json_val = serde_json::to_string(mappings).unwrap_or_default();
            let now = chrono::Utc::now().to_rfc3339();
            let _ = db.conn().execute(
                "INSERT OR REPLACE INTO kv_state (key, value, updated_at) VALUES ('identity_mappings', ?1, ?2)",
                rusqlite::params![json_val, now],
            );
        }
    }

    // Write config file atomically
    let tmp_path = state.config_path.with_extension("toml.tmp");
    if let Err(e) = std::fs::write(&tmp_path, &toml_content) {
        return Ok(Json(ApplyConfigResponse {
            ok: false,
            message: format!("Failed to write config: {}", e),
            warnings,
        }));
    }
    if let Err(e) = std::fs::rename(&tmp_path, &state.config_path) {
        return Ok(Json(ApplyConfigResponse {
            ok: false,
            message: format!("Failed to save config: {}", e),
            warnings,
        }));
    }

    // LFS check
    if body.lfs_threshold.unwrap_or(0) > 0 {
        match gitsvnsync_core::lfs::preflight_check() {
            Ok(version) => {
                info!("LFS preflight passed: {}", version);
            }
            Err(e) => {
                warnings.push(format!(
                    "Git LFS is configured but not available: {}. Large files will be committed directly.",
                    e
                ));
            }
        }
    }

    info!(path = %state.config_path.display(), "Configuration saved via setup wizard");

    Ok(Json(ApplyConfigResponse {
        ok: true,
        message: "Configuration saved successfully".into(),
        warnings,
    }))
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

async fn start_import(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ImportActionResponse>, AppError> {
    // Check if already running
    {
        let p = state.import_progress.read().await;
        if p.state == ImportState::Running {
            return Ok(Json(ImportActionResponse {
                ok: false,
                message: "An import is already running".into(),
            }));
        }
    }

    // Load config from file
    let config_content = std::fs::read_to_string(&state.config_path)
        .map_err(|e| AppError::Internal(format!("failed to read config: {}", e)))?;
    let mut config: AppConfig = toml::from_str(&config_content)
        .map_err(|e| AppError::Internal(format!("failed to parse config: {}", e)))?;
    config
        .resolve_env_vars()
        .map_err(|e| AppError::Internal(format!("failed to resolve env vars: {}", e)))?;

    // Reset progress
    {
        let mut p = state.import_progress.write().await;
        *p = ImportProgress::default();
        p.state = ImportState::Running;
        p.started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    // Build clients for the import
    let svn_password = config.svn.password.clone().unwrap_or_default();
    let svn_client = SvnClient::new(&config.svn.url, &config.svn.username, &svn_password);

    let git_repo_path = config.daemon.data_dir.join("git-repo");

    // Init or open Git repo
    let git_client = if git_repo_path.join(".git").exists() {
        GitClient::new(&git_repo_path)
            .map_err(|e| AppError::Internal(format!("failed to open git repo: {}", e)))?
    } else {
        let clone_url = config.github.clone_url();
        let token = config.github.token.as_deref();
        GitClient::clone_repo(&clone_url, &git_repo_path, token)
            .map_err(|e| AppError::Internal(format!("failed to clone git repo: {}", e)))?
    };

    // Ensure credentials are embedded
    git_client
        .ensure_remote_credentials("origin", config.github.token.as_deref())
        .map_err(|e| AppError::Internal(format!("failed to set git credentials: {}", e)))?;

    let git_client = Arc::new(tokio::sync::Mutex::new(git_client));

    let identity_mapper = IdentityMapper::new(&config.identity)
        .map_err(|e| AppError::Internal(format!("failed to init identity mapper: {}", e)))?;

    let file_policy = FilePolicy::from(&config.sync);

    let db_path = config.daemon.data_dir.join("gitsvnsync.db");
    let import_db = Database::new(&db_path)
        .map_err(|e| AppError::Internal(format!("failed to open db: {}", e)))?;

    let import_config = ImportConfig {
        committer_name: "RepoSync".into(),
        committer_email: "reposync@localhost".into(),
        remote_name: "origin".into(),
        branch: config.github.default_branch.clone(),
        push_token: config.github.token.clone(),
        message_prefix: None,
    };

    let progress = state.import_progress.clone();
    let ws_broadcast = Some(state.ws_broadcast.clone());

    // Spawn the import in a background task
    tokio::spawn(async move {
        let result = import::run_full_import(
            &svn_client,
            &git_client,
            &identity_mapper,
            &import_db,
            &file_policy,
            &import_config,
            progress.clone(),
            ws_broadcast.clone(),
        )
        .await;

        let mut p = progress.write().await;
        match result {
            Ok(count) => {
                if p.state != ImportState::Cancelled {
                    p.state = ImportState::Completed;
                }
                p.completed_at = Some(chrono::Utc::now().to_rfc3339());
                p.push_log(format!(
                    "[info] Import complete: {} commits created",
                    count
                ));
                info!(count, "import completed successfully");
            }
            Err(e) => {
                p.state = ImportState::Failed;
                p.completed_at = Some(chrono::Utc::now().to_rfc3339());
                let msg = format!("[error] Import failed: {}", e);
                p.push_log(msg.clone());
                p.errors.push(msg);
                error!("import failed: {}", e);
            }
        }

        // Send final broadcast
        if let Some(ref sender) = ws_broadcast {
            let json = serde_json::json!({
                "type": "import_progress",
                "state": format!("{:?}", p.state).to_lowercase(),
                "current_rev": p.current_rev,
                "total_revs": p.total_revs,
                "commits_created": p.commits_created,
            });
            let _ = sender.send(json.to_string());
        }
    });

    Ok(Json(ImportActionResponse {
        ok: true,
        message: "Import started".into(),
    }))
}

async fn import_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ImportProgress>, AppError> {
    let p = state.import_progress.read().await;
    Ok(Json(p.clone()))
}

async fn cancel_import(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ImportActionResponse>, AppError> {
    let mut p = state.import_progress.write().await;
    if p.state == ImportState::Running {
        p.cancel_requested = true;
        Ok(Json(ImportActionResponse {
            ok: true,
            message: "Cancellation requested".into(),
        }))
    } else {
        Ok(Json(ImportActionResponse {
            ok: false,
            message: "No import is currently running".into(),
        }))
    }
}

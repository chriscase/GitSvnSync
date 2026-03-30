//! GitSvnSync web server and REST API.
//!
//! Provides an Axum-based HTTP server with:
//! - Status and health endpoints
//! - Conflict management API
//! - Configuration and identity mapping API
//! - Audit log API
//! - GitHub / SVN webhook receivers
//! - WebSocket endpoint for live updates
//! - Simple session-based authentication

pub mod api;
pub mod ws;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, Method};
use axum::Router;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::info;

use gitsvnsync_core::config::AppConfig;
use gitsvnsync_core::db::Database;
use gitsvnsync_core::import::ImportProgress;
use gitsvnsync_core::sync_engine::SyncEngine;

/// Shared application state accessible from all handlers.
pub struct AppState {
    pub db: std::sync::Mutex<Database>,
    /// Per-repository sync engines keyed by repository ID.
    pub engines: tokio::sync::RwLock<HashMap<String, Arc<SyncEngine>>>,
    /// Backward-compatible single sync engine reference (points to the default engine).
    pub sync_engine: Arc<SyncEngine>,
    pub config: AppConfig,
    /// Channel for triggering immediate sync cycles.
    pub sync_trigger: tokio::sync::mpsc::Sender<()>,
    /// Broadcast channel for live WebSocket updates.
    pub ws_broadcast: broadcast::Sender<String>,
    /// Active sessions (token -> expiry timestamp).
    pub sessions:
        tokio::sync::RwLock<std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>>,
    /// Per-repository import progress tracking keyed by repository ID.
    pub import_progress: tokio::sync::RwLock<HashMap<String, Arc<tokio::sync::RwLock<ImportProgress>>>>,
    /// Path to the TOML config file on disk.
    pub config_path: std::path::PathBuf,
    /// Previous network byte counters for rate calculation.
    pub prev_net_snapshot: std::sync::Mutex<Option<(u64, u64, std::time::Instant)>>,
}

/// Key used for the default (first/migrated) repository in the engines map.
pub const DEFAULT_REPO_KEY: &str = "default";

impl AppState {
    /// Get a sync engine by repository ID, falling back to the default engine.
    pub async fn get_engine(&self, repo_id: Option<&str>) -> Arc<SyncEngine> {
        if let Some(id) = repo_id {
            let engines = self.engines.read().await;
            if let Some(engine) = engines.get(id) {
                return engine.clone();
            }
        }
        self.sync_engine.clone()
    }

    /// Get (or create) the import progress tracker for a given repo.
    /// Uses `DEFAULT_REPO_KEY` when no repo_id is specified.
    pub async fn get_import_progress(
        &self,
        repo_id: Option<&str>,
    ) -> Arc<tokio::sync::RwLock<ImportProgress>> {
        let key = repo_id.unwrap_or(DEFAULT_REPO_KEY);
        // Fast path: read lock
        {
            let map = self.import_progress.read().await;
            if let Some(p) = map.get(key) {
                return p.clone();
            }
        }
        // Slow path: insert a new entry
        let mut map = self.import_progress.write().await;
        map.entry(key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::RwLock::new(ImportProgress::default())))
            .clone()
    }
}

/// The web server.
pub struct WebServer {
    state: Arc<AppState>,
}

impl WebServer {
    /// Create a new web server with the given dependencies.
    ///
    /// `engines` maps repository IDs to their sync engines.
    /// `default_engine` is kept for backward-compatible single-engine access.
    pub fn new(
        config: AppConfig,
        db: Database,
        engines: HashMap<String, Arc<SyncEngine>>,
        default_engine: Arc<SyncEngine>,
        sync_trigger: tokio::sync::mpsc::Sender<()>,
        config_path: std::path::PathBuf,
    ) -> Self {
        let (ws_tx, _) = broadcast::channel(256);

        // Build per-repo import progress map with a default entry
        let mut import_map = HashMap::new();
        for repo_id in engines.keys() {
            import_map.insert(
                repo_id.clone(),
                Arc::new(tokio::sync::RwLock::new(ImportProgress::default())),
            );
        }

        let state = Arc::new(AppState {
            db: std::sync::Mutex::new(db),
            engines: tokio::sync::RwLock::new(engines),
            sync_engine: default_engine,
            config,
            sync_trigger,
            ws_broadcast: ws_tx,
            sessions: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            import_progress: tokio::sync::RwLock::new(import_map),
            config_path,
            prev_net_snapshot: std::sync::Mutex::new(None),
        });
        Self { state }
    }

    /// Get a clone of the broadcast sender for pushing events.
    pub fn broadcast_sender(&self) -> broadcast::Sender<String> {
        self.state.ws_broadcast.clone()
    }

    /// Start the web server, listening on the given address.
    pub async fn start(self, listen_addr: &str) -> anyhow::Result<()> {
        let addr: SocketAddr = listen_addr.parse()?;

        // CORS: allow the bundled web-ui (same origin) and localhost dev.
        // In production, restrict to the actual frontend origin.
        let cors = CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

        // Serve the React SPA from the "static" directory next to the binary.
        // All unknown routes fall back to index.html for client-side routing.
        let static_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("static")))
            .unwrap_or_else(|| std::path::PathBuf::from("static"));
        let serve_spa = ServeDir::new(&static_dir)
            .not_found_service(ServeFile::new(static_dir.join("index.html")));

        let app = Router::new()
            // API routes
            .merge(api::status::routes())
            .merge(api::conflicts::routes())
            .merge(api::config::routes())
            .merge(api::auth::routes())
            .merge(api::audit::routes())
            .merge(api::sync_history::routes())
            .merge(api::seed::routes())
            .merge(api::webhooks::routes())
            .merge(api::setup::routes())
            .merge(api::users::routes())
            .merge(api::repos::routes())
            // WebSocket
            .merge(ws::routes())
            // React SPA (fallback for everything else)
            .fallback_service(serve_spa)
            // Middleware
            .layer(DefaultBodyLimit::max(2 * 1024 * 1024)) // 2 MB max request body
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            .with_state(self.state);

        info!(addr = %addr, "starting web server");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

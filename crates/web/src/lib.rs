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

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::Router;
use tokio::sync::broadcast;
use axum::http::{header, Method};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use gitsvnsync_core::config::AppConfig;
use gitsvnsync_core::db::Database;
use gitsvnsync_core::sync_engine::SyncEngine;

/// Shared application state accessible from all handlers.
pub struct AppState {
    pub db: std::sync::Mutex<Database>,
    pub sync_engine: Arc<SyncEngine>,
    pub config: AppConfig,
    /// Channel for triggering immediate sync cycles.
    pub sync_trigger: tokio::sync::mpsc::Sender<()>,
    /// Broadcast channel for live WebSocket updates.
    pub ws_broadcast: broadcast::Sender<String>,
    /// Active sessions (token -> expiry timestamp).
    pub sessions:
        tokio::sync::RwLock<std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>>,
}

/// The web server.
pub struct WebServer {
    state: Arc<AppState>,
}

impl WebServer {
    /// Create a new web server with the given dependencies.
    pub fn new(
        config: AppConfig,
        db: Database,
        sync_engine: Arc<SyncEngine>,
        sync_trigger: tokio::sync::mpsc::Sender<()>,
    ) -> Self {
        let (ws_tx, _) = broadcast::channel(256);
        let state = Arc::new(AppState {
            db: std::sync::Mutex::new(db),
            sync_engine,
            config,
            sync_trigger,
            ws_broadcast: ws_tx,
            sessions: tokio::sync::RwLock::new(std::collections::HashMap::new()),
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

        let app = Router::new()
            // API routes
            .merge(api::status::routes())
            .merge(api::conflicts::routes())
            .merge(api::config::routes())
            .merge(api::auth::routes())
            .merge(api::audit::routes())
            .merge(api::webhooks::routes())
            // WebSocket
            .merge(ws::routes())
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

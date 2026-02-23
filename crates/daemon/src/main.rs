//! GitSvnSync daemon entry point.
//!
//! Loads configuration, initializes all subsystems, starts the web server
//! and sync scheduler, and handles graceful shutdown.

mod scheduler;
mod signals;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use gitsvnsync_core::config::AppConfig;
use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::identity::IdentityMapper;
use gitsvnsync_core::svn::SvnClient;
use gitsvnsync_core::sync_engine::SyncEngine;
use gitsvnsync_web::WebServer;

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

/// GitSvnSync synchronization daemon.
#[derive(Parser, Debug)]
#[command(
    name = "gitsvnsync-daemon",
    version,
    about = "Bidirectional SVN/Git synchronization daemon"
)]
struct Args {
    /// Path to the TOML configuration file.
    #[arg(short, long)]
    config: PathBuf,

    /// Override the log level from the config file (trace, debug, info, warn, error).
    #[arg(long)]
    log_level: Option<String>,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load and resolve configuration
    let mut config =
        AppConfig::load_from_file(&args.config).context("failed to load configuration file")?;
    config
        .resolve_env_vars()
        .context("failed to resolve environment variables in config")?;
    config
        .validate()
        .context("configuration validation failed")?;

    // Initialize tracing
    let log_level = args
        .log_level
        .as_deref()
        .unwrap_or(&config.daemon.log_level);

    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .init();

    // Startup banner
    info!("========================================");
    info!("  GitSvnSync Daemon v{}", env!("CARGO_PKG_VERSION"));
    info!("========================================");
    info!("Config file   : {}", args.config.display());
    info!("SVN URL       : {}", config.svn.url);
    info!("GitHub repo   : {}", config.github.repo);
    info!("Poll interval : {}s", config.daemon.poll_interval_secs);
    info!("Web listen    : {}", config.web.listen);
    info!("Data dir      : {}", config.daemon.data_dir.display());
    info!("Log level     : {}", log_level);
    info!("========================================");

    // Ensure data directory exists
    std::fs::create_dir_all(&config.daemon.data_dir).context("failed to create data directory")?;

    // Initialize database
    let db_path = config.daemon.data_dir.join("gitsvnsync.db");
    let db = Database::new(&db_path).context("failed to open database")?;
    db.initialize()
        .context("failed to initialize database schema")?;
    // Open a second connection for the web server (SQLite supports multiple readers with WAL)
    let web_db = Database::new(&db_path).context("failed to open web database connection")?;
    info!("Database initialized at {}", db_path.display());

    // Initialize SVN client
    let svn_password = config.svn.password.clone().unwrap_or_default();
    let svn_client = SvnClient::new(&config.svn.url, &config.svn.username, &svn_password);
    info!("SVN client initialized for {}", config.svn.url);

    // Initialize Git client
    let git_repo_path = config.daemon.data_dir.join("git-repo");
    let git_client = if git_repo_path.join(".git").exists() {
        GitClient::new(&git_repo_path).context("failed to open existing Git repository")?
    } else {
        let clone_url = config.github.clone_url();
        let token = config.github.token.as_deref();
        GitClient::clone_repo(&clone_url, &git_repo_path, token)
            .context("failed to clone Git repository")?
    };
    info!("Git client initialized at {}", git_repo_path.display());

    // Initialize identity mapper
    let identity_mapper = Arc::new(
        IdentityMapper::new(&config.identity).context("failed to initialize identity mapper")?,
    );
    info!("Identity mapper initialized");

    // Initialize sync engine
    let sync_engine = Arc::new(SyncEngine::new(
        config.clone(),
        db,
        svn_client,
        git_client,
        identity_mapper,
    ));
    info!("Sync engine initialized");

    // Create sync trigger channel (webhook -> scheduler)
    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel::<()>(16);

    // Initialize web server
    let web_server = WebServer::new(config.clone(), web_db, sync_engine.clone(), sync_tx.clone());
    let ws_broadcast = web_server.broadcast_sender();
    let listen_addr = config.web.listen.clone();

    // Start web server in background
    let web_handle = tokio::spawn(async move {
        if let Err(e) = web_server.start(&listen_addr).await {
            error!("Web server error: {}", e);
        }
    });

    // Create a shutdown notify for cooperative cancellation
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let scheduler_shutdown = shutdown.clone();

    // Create and start the scheduler
    let poll_interval = std::time::Duration::from_secs(config.daemon.poll_interval_secs);
    let mut sched =
        scheduler::Scheduler::new(sync_engine.clone(), poll_interval, sync_rx, ws_broadcast);

    // Start the scheduler in a background task
    let scheduler_handle = tokio::spawn(async move {
        sched.run(scheduler_shutdown).await;
    });

    // Wait for shutdown signal
    signals::wait_for_shutdown().await;

    info!("Shutdown signal received, stopping...");

    // Signal cooperative shutdown to the scheduler
    shutdown.notify_waiters();

    // Wait for the scheduler to finish its current cycle (up to 10s)
    match tokio::time::timeout(std::time::Duration::from_secs(10), scheduler_handle).await {
        Ok(Ok(())) => info!("scheduler stopped gracefully"),
        Ok(Err(e)) => warn!("scheduler task error: {}", e),
        Err(_) => warn!("scheduler did not stop within 10s, forcing shutdown"),
    }

    // Abort the web server
    web_handle.abort();

    info!("GitSvnSync daemon stopped.");
    Ok(())
}

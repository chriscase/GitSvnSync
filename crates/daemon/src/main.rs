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

#[tokio::main(flavor = "multi_thread", worker_threads = 16)]
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

    // Resolve secrets from DB (fallback when env vars are absent)
    config.resolve_secrets_from_db(&db);

    // Auto-bootstrap admin user if users table is empty
    match db.count_users() {
        Ok(0) => {
            let admin_password = std::env::var("REPOSYNC_ADMIN_PASSWORD")
                .unwrap_or_else(|_| {
                    warn!("No REPOSYNC_ADMIN_PASSWORD set and no users exist — creating admin user with default password 'changeme'. CHANGE THIS IMMEDIATELY!");
                    "changeme".to_string()
                });
            match gitsvnsync_core::crypto::hash_password(&admin_password) {
                Ok(hash) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let admin = gitsvnsync_core::models::User {
                        id: uuid::Uuid::new_v4().to_string(),
                        username: "admin".to_string(),
                        display_name: "Administrator".to_string(),
                        email: "admin@localhost".to_string(),
                        password_hash: hash,
                        role: "admin".to_string(),
                        enabled: true,
                        created_at: now.clone(),
                        updated_at: now,
                    };
                    match db.insert_user(&admin) {
                        Ok(()) => info!("Created bootstrap admin user (username: admin)"),
                        Err(e) => error!("Failed to create bootstrap admin user: {}", e),
                    }
                }
                Err(e) => error!("Failed to hash admin password: {}", e),
            }
        }
        Ok(n) => info!("{} user(s) found in database, skipping admin bootstrap", n),
        Err(e) => warn!("Failed to check users table (may not exist yet): {}", e),
    }

    // Auto-migrate existing single-repo config into the repositories table
    // so that existing deployments continue to work without manual changes.
    {
        let repos = db.list_repositories().unwrap_or_default();
        if repos.is_empty() && !config.svn.url.is_empty() {
            let now = chrono::Utc::now().to_rfc3339();
            let provider = match config.github.provider {
                gitsvnsync_core::config::GitProvider::GitHub => "github",
                gitsvnsync_core::config::GitProvider::Gitea => "gitea",
            };
            let sync_mode = match config.sync.mode {
                gitsvnsync_core::config::SyncMode::Direct => "direct",
                gitsvnsync_core::config::SyncMode::Pr => "pr",
            };
            let default_repo = gitsvnsync_core::models::Repository {
                id: uuid::Uuid::new_v4().to_string(),
                name: config.github.repo.clone(),
                svn_url: config.svn.url.clone(),
                svn_branch: config.svn.trunk_path.clone(),
                svn_username: config.svn.username.clone(),
                git_provider: provider.to_string(),
                git_api_url: config.github.api_url.clone(),
                git_repo: config.github.repo.clone(),
                git_branch: config.github.default_branch.clone(),
                sync_mode: sync_mode.to_string(),
                poll_interval_secs: config.daemon.poll_interval_secs as i64,
                lfs_threshold_mb: 0,
                auto_merge: config.sync.auto_merge,
                enabled: true,
                created_by: None,
                created_at: now.clone(),
                updated_at: now,
            };
            match db.insert_repository(&default_repo) {
                Ok(()) => info!(
                    "Auto-migrated existing config to repository: {}",
                    default_repo.name
                ),
                Err(e) => warn!("Failed to auto-migrate config to repository: {}", e),
            }
        }
    }

    // Initialize SVN client
    let svn_password = config.svn.password.clone().unwrap_or_default();
    let svn_client = SvnClient::new(&config.svn.url, &config.svn.username, &svn_password);
    info!("SVN client initialized for {}", config.svn.url);

    // Initialize Git client (graceful: create empty repo if clone fails)
    let git_repo_path = config.daemon.data_dir.join("git-repo");
    let git_client = if git_repo_path.join(".git").exists() {
        GitClient::new(&git_repo_path).context("failed to open existing Git repository")?
    } else {
        let clone_url = config.github.clone_url();
        let token = config.github.token.as_deref();
        match GitClient::clone_repo(&clone_url, &git_repo_path, token) {
            Ok(client) => {
                info!("Git client cloned at {}", git_repo_path.display());
                client
            }
            Err(e) => {
                // Clone failed — init an empty repo so the daemon can start.
                // The import wizard will populate it later.
                warn!("Clone failed ({}), initializing empty git repo", e);
                std::fs::create_dir_all(&git_repo_path)
                    .context("failed to create git repo directory")?;
                let init_output = std::process::Command::new("git")
                    .args(["init", "--initial-branch", &config.github.default_branch])
                    .current_dir(&git_repo_path)
                    .output();
                if init_output.is_err() || !init_output.as_ref().unwrap().status.success() {
                    // Fallback for older git without --initial-branch
                    let _ = std::process::Command::new("git")
                        .args(["init"])
                        .current_dir(&git_repo_path)
                        .output();
                }
                let _ = std::process::Command::new("git")
                    .args(["remote", "add", "origin", &clone_url])
                    .current_dir(&git_repo_path)
                    .output();
                GitClient::new(&git_repo_path)
                    .context("failed to open newly initialized Git repository")?
            }
        }
    };
    // Ensure the remote URL has embedded credentials for reliable HTTP auth.
    git_client
        .ensure_remote_credentials("origin", config.github.token.as_deref())
        .ok(); // Don't crash if this fails

    // Initialize identity mapper
    let identity_mapper = Arc::new(
        IdentityMapper::new(&config.identity).context("failed to initialize identity mapper")?,
    );
    info!("Identity mapper initialized");

    // Initialize sync engine
    let mut engine = SyncEngine::new(
        config.clone(),
        db,
        svn_client,
        git_client,
        identity_mapper,
    );
    // Set repo_id from the first enabled repository for per-repo keys
    if let Ok(repos) = engine.db().list_repositories() {
        if let Some(repo) = repos.into_iter().find(|r| r.enabled) {
            engine.set_repo_id(repo.id);
        }
    }
    let sync_engine = Arc::new(engine);
    info!("Sync engine initialized");

    // Create sync trigger channel (webhook -> scheduler)
    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel::<()>(16);

    // Create shared import progress — shared between web server and scheduler
    // so the scheduler can pause sync cycles during an active import.
    let import_progress = std::sync::Arc::new(tokio::sync::RwLock::new(
        gitsvnsync_core::import::ImportProgress::default(),
    ));

    // Initialize web server
    let web_server = WebServer::new(
        config.clone(),
        web_db,
        sync_engine.clone(),
        sync_tx.clone(),
        args.config.clone(),
        import_progress.clone(),
    );
    let ws_broadcast = web_server.broadcast_sender();
    let listen_addr = config.web.listen.clone();

    // Start web server — runs on the main tokio runtime directly
    // (not spawned) to ensure it gets immediate access to worker threads.
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
    let mut sched = scheduler::Scheduler::new(
        sync_engine.clone(),
        poll_interval,
        sync_rx,
        ws_broadcast,
        import_progress,
    );

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

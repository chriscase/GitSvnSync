//! GitSvnSync Personal Branch Mode daemon.
//!
//! A lightweight, single-developer tool that mirrors an SVN repo to a personal
//! GitHub repository. Supports feature branches, PRs, and automatic bidirectional
//! commit sync.

mod commit_format;
mod daemon;
mod engine;
mod git_to_svn;
mod initial_import;
mod pr_monitor;
mod scheduler;
mod signals;
mod svn_to_git;

use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;

use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::github::GitHubClient;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::personal_config::PersonalConfig;
use gitsvnsync_core::svn::SvnClient;

use crate::engine::PersonalSyncEngine;
use crate::initial_import::{ImportMode, InitialImport};

/// GitSvnSync Personal Branch Mode — individual SVN↔Git sync daemon.
#[derive(Parser)]
#[command(name = "gitsvnsync-personal", version, about)]
struct Cli {
    /// Path to the personal config file.
    #[arg(short, long, default_value = "~/.config/gitsvnsync/personal.toml")]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the sync daemon.
    Start {
        /// Run in the foreground (default is background).
        #[arg(long)]
        foreground: bool,
    },

    /// Stop the running daemon.
    Stop,

    /// Check daemon status.
    Status,

    /// Run a single sync cycle and exit.
    Sync,

    /// Import SVN history into Git.
    Import {
        /// Import only a snapshot of HEAD (one commit).
        #[arg(long, conflicts_with = "full")]
        snapshot: bool,

        /// Import full SVN history (one commit per revision).
        #[arg(long, conflicts_with = "snapshot")]
        full: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Resolve config path (expand ~).
    let config_path = expand_tilde(&cli.config);

    match cli.command {
        Commands::Start { foreground } => cmd_start(&config_path, foreground).await,
        Commands::Stop => cmd_stop(&config_path),
        Commands::Status => cmd_status(&config_path),
        Commands::Sync => cmd_sync(&config_path).await,
        Commands::Import { snapshot, full: _ } => {
            let mode = if snapshot {
                ImportMode::Snapshot
            } else {
                // Default to full if neither flag specified
                ImportMode::Full
            };
            cmd_import(&config_path, mode).await
        }
    }
}

/// Load config and build the sync engine.
async fn build_engine(config_path: &str) -> Result<(PersonalSyncEngine, PersonalConfig)> {
    let config =
        PersonalConfig::load_and_resolve(config_path).context("failed to load personal config")?;
    config.validate().context("invalid personal config")?;

    // Ensure data directory exists.
    let data_dir = &config.personal.data_dir;
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create data directory: {}", data_dir.display()))?;

    // Initialize database.
    let db_path = data_dir.join("personal.db");
    let db = Database::new(&db_path).context("failed to initialize database")?;

    // Create SVN client.
    let svn_client = SvnClient::new(
        &config.svn.url,
        &config.svn.username,
        config.svn.password.as_deref().unwrap_or(""),
    );

    // Set up Git repository.
    let git_repo_path = data_dir.join("git-repo");
    let git_client = if git_repo_path.exists() {
        GitClient::new(&git_repo_path).context("failed to open git repository")?
    } else {
        std::fs::create_dir_all(&git_repo_path).context("failed to create git repo directory")?;
        let remote_url = format!("https://github.com/{}.git", config.github.repo);
        GitClient::clone_repo(&remote_url, &git_repo_path, config.github.token.as_deref())
            .context("failed to clone git repository")?
    };

    // Create GitHub client.
    let github_token = config.github.token.as_deref().unwrap_or("");
    let github_client = GitHubClient::new(&config.github.api_url, github_token);

    let engine = PersonalSyncEngine::new(config.clone(), db, svn_client, git_client, github_client);
    Ok((engine, config))
}

/// Start the sync daemon.
async fn cmd_start(config_path: &str, foreground: bool) -> Result<()> {
    let (engine, config) = build_engine(config_path).await?;
    let data_dir = &config.personal.data_dir;

    // Check if already running.
    if let Some(pid) = daemon::is_running(data_dir)? {
        anyhow::bail!("daemon is already running (PID {})", pid);
    }

    if foreground {
        info!("starting personal sync daemon in foreground mode");
        daemon::write_pid_file(&daemon::pid_file_path(data_dir))?;

        let shutdown = signals::setup_signal_handlers();
        let interval = Duration::from_secs(config.personal.poll_interval_secs);

        let result = scheduler::run_polling_loop(&engine, interval, shutdown).await;

        daemon::remove_pid_file(&daemon::pid_file_path(data_dir))?;
        result
    } else {
        info!("starting personal sync daemon in background mode");
        daemon::write_pid_file(&daemon::pid_file_path(data_dir))?;

        let shutdown = signals::setup_signal_handlers();
        let interval = Duration::from_secs(config.personal.poll_interval_secs);

        let result = scheduler::run_polling_loop(&engine, interval, shutdown).await;

        daemon::remove_pid_file(&daemon::pid_file_path(data_dir))?;
        result
    }
}

/// Stop the daemon.
fn cmd_stop(config_path: &str) -> Result<()> {
    let config =
        PersonalConfig::load_and_resolve(config_path).context("failed to load personal config")?;
    let data_dir = &config.personal.data_dir;

    match daemon::stop_daemon(data_dir)? {
        true => {
            info!("daemon stopped successfully");
            println!("✓ Daemon stopped gracefully");
        }
        false => {
            println!("Daemon is not running");
        }
    }
    Ok(())
}

/// Show daemon status.
fn cmd_status(config_path: &str) -> Result<()> {
    let config =
        PersonalConfig::load_and_resolve(config_path).context("failed to load personal config")?;
    let data_dir = &config.personal.data_dir;

    match daemon::is_running(data_dir)? {
        Some(pid) => println!("● Running (PID {})", pid),
        None => println!("○ Not running"),
    }
    Ok(())
}

/// Run a single sync cycle.
async fn cmd_sync(config_path: &str) -> Result<()> {
    let (engine, _config) = build_engine(config_path).await?;

    info!("running single sync cycle");
    let stats = engine.run_cycle().await?;

    println!(
        "Sync complete: SVN→Git: {} commits, Git→SVN: {} commits ({} PRs)",
        stats.svn_to_git_count, stats.git_to_svn_count, stats.prs_processed
    );
    Ok(())
}

/// Import SVN history into Git.
async fn cmd_import(config_path: &str, mode: ImportMode) -> Result<()> {
    let config =
        PersonalConfig::load_and_resolve(config_path).context("failed to load personal config")?;
    config.validate().context("invalid personal config")?;

    let data_dir = &config.personal.data_dir;
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create data directory: {}", data_dir.display()))?;

    // Initialize database.
    let db_path = data_dir.join("personal.db");
    let db = Database::new(&db_path).context("failed to initialize database")?;

    // Create SVN client.
    let svn_client = SvnClient::new(
        &config.svn.url,
        &config.svn.username,
        config.svn.password.as_deref().unwrap_or(""),
    );

    // GitHub client.
    let github_token = config.github.token.as_deref().unwrap_or("");
    let github_client = GitHubClient::new(&config.github.api_url, github_token);

    // Initialize Git repo for import.
    let git_repo_path = data_dir.join("git-repo");
    let git_client = if git_repo_path.exists() {
        GitClient::new(&git_repo_path).context("failed to open git repository")?
    } else {
        std::fs::create_dir_all(&git_repo_path).context("failed to create git repo directory")?;
        // For import, try to clone first; if repo doesn't exist yet, init locally
        let remote_url = format!("https://github.com/{}.git", config.github.repo);
        match GitClient::clone_repo(&remote_url, &git_repo_path, config.github.token.as_deref()) {
            Ok(client) => client,
            Err(_) => {
                // Repo might not exist yet (auto-create will handle it)
                GitClient::init(&git_repo_path).context("failed to init git repository")?
            }
        }
    };
    let git_client = std::sync::Arc::new(tokio::sync::Mutex::new(git_client));

    let commit_format = crate::commit_format::CommitFormatter::new(&config.commit_format);

    let importer = InitialImport {
        svn_client: &svn_client,
        git_client: &git_client,
        github_client: &github_client,
        db: &db,
        config: &config,
        formatter: &commit_format,
    };

    let count = importer.import(mode).await?;
    println!("✓ Import complete! {} commits created.", count);
    Ok(())
}

/// Expand `~` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), rest);
        }
    }
    path.to_string()
}

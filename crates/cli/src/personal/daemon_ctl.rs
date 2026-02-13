//! Daemon start/stop CLI commands for personal branch mode.

use std::time::Duration;

use anyhow::{Context, Result};

use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::git::github::GitHubClient;
use gitsvnsync_core::personal_config::PersonalConfig;
use gitsvnsync_core::svn::SvnClient;

use super::style;

/// Start the personal sync daemon.
pub async fn run_start(config: &PersonalConfig, foreground: bool) -> Result<()> {
    let data_dir = &config.personal.data_dir;

    // Check if already running
    if let Some(pid) = gitsvnsync_personal::daemon::is_running(data_dir)? {
        println!("{}", style::warn(&format!("Daemon is already running (PID {})", pid)));
        return Ok(());
    }

    println!("Starting GitSvnSync Personal daemon...");

    // Ensure data directory exists
    std::fs::create_dir_all(data_dir)
        .context("failed to create data directory")?;

    // Initialize components
    let db_path = data_dir.join("personal.db");
    let db = Database::new(db_path.to_str().unwrap_or(""))
        .context("failed to open database")?;

    let svn_client = SvnClient::new(
        &config.svn.url,
        &config.svn.username,
        config.svn.password.as_deref().unwrap_or(""),
    );

    let git_repo_path = data_dir.join("git-repo");
    let git_client = GitClient::new(&git_repo_path)
        .context("failed to open git repository")?;

    let github_token = config.github.token.as_deref().unwrap_or("");
    let github_client = GitHubClient::new(&config.github.api_url, github_token);

    let engine = gitsvnsync_personal::engine::PersonalSyncEngine::new(
        config.clone(),
        db,
        svn_client,
        git_client,
        github_client,
    );

    let pid_path = gitsvnsync_personal::daemon::pid_file_path(data_dir);
    gitsvnsync_personal::daemon::write_pid_file(&pid_path)?;

    println!("{}", style::success(&format!("Daemon started (PID {})", std::process::id())));
    println!("{}", style::success(&format!("Polling SVN every {} seconds", config.personal.poll_interval_secs)));
    println!("{}", style::success(&format!("Watching for merged PRs on {}", config.github.repo)));

    if !foreground {
        println!();
        println!("  Logs: {}", data_dir.join("personal.log").display());
        println!("  Stop: gitsvnsync personal stop");
    }

    let shutdown = gitsvnsync_personal::signals::setup_signal_handlers();
    let interval = Duration::from_secs(config.personal.poll_interval_secs);

    let result = gitsvnsync_personal::scheduler::run_polling_loop(&engine, interval, shutdown).await;

    gitsvnsync_personal::daemon::remove_pid_file(&pid_path)?;
    result
}

/// Stop the personal sync daemon.
pub fn run_stop(config: &PersonalConfig) -> Result<()> {
    let data_dir = &config.personal.data_dir;

    match gitsvnsync_personal::daemon::stop_daemon(data_dir)? {
        true => {
            println!("{}", style::success("Daemon stopped gracefully"));
        }
        false => {
            println!("Daemon is not running");
        }
    }
    Ok(())
}

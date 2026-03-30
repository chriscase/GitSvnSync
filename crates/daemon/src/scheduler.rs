//! Sync scheduler that runs sync cycles on a configurable interval and
//! supports webhook-triggered immediate syncs.
//!
//! Supports multiple repositories: on each tick the scheduler iterates over
//! all registered engines and spawns a sync cycle for each idle engine.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, Notify, RwLock};
use tokio::time;
use tracing::{error, info, warn};

use gitsvnsync_core::sync_engine::SyncEngine;

/// Tracks aggregate statistics across sync cycles.
pub struct SchedulerStats {
    pub total_cycles: AtomicU64,
    pub total_conflicts: AtomicU64,
    pub total_errors: AtomicU64,
    pub consecutive_errors: AtomicU64,
}

impl SchedulerStats {
    fn new() -> Self {
        Self {
            total_cycles: AtomicU64::new(0),
            total_conflicts: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            consecutive_errors: AtomicU64::new(0),
        }
    }
}

/// The sync scheduler.
///
/// Runs sync cycles on a timer and also listens for webhook-triggered
/// immediate sync requests. Each tick iterates over all registered engines
/// and spawns a sync cycle for each idle engine. The sync engine's own lock
/// prevents concurrent cycles per engine.
pub struct Scheduler {
    /// Per-repository sync engines keyed by repository ID.
    engines: Arc<RwLock<HashMap<String, Arc<SyncEngine>>>>,
    poll_interval: Duration,
    sync_rx: mpsc::Receiver<()>,
    ws_broadcast: broadcast::Sender<String>,
    stats: Arc<SchedulerStats>,
}

impl Scheduler {
    /// Create a new scheduler with a map of repository engines.
    pub fn new(
        engines: HashMap<String, Arc<SyncEngine>>,
        poll_interval: Duration,
        sync_rx: mpsc::Receiver<()>,
        ws_broadcast: broadcast::Sender<String>,
    ) -> Self {
        Self {
            engines: Arc::new(RwLock::new(engines)),
            poll_interval,
            sync_rx,
            ws_broadcast,
            stats: Arc::new(SchedulerStats::new()),
        }
    }

    /// Main scheduler loop.
    ///
    /// Runs until the `shutdown` notify fires, then returns so the caller
    /// can perform a clean shutdown.
    pub async fn run(&mut self, shutdown: Arc<Notify>) {
        info!(
            poll_interval_secs = self.poll_interval.as_secs(),
            "scheduler started"
        );

        let mut interval = time::interval(self.poll_interval);
        // The first tick fires immediately; consume it to allow the system
        // time to fully start before the first sync.
        interval.tick().await;

        loop {
            tokio::select! {
                // Shutdown signal takes priority
                _ = shutdown.notified() => {
                    info!("scheduler received shutdown signal");
                    break;
                }
                // Regular polling interval
                _ = interval.tick() => {
                    self.maybe_run_cycle("scheduled").await;
                }
                // Webhook-triggered immediate sync
                Some(()) = self.sync_rx.recv() => {
                    info!("immediate sync requested via webhook");
                    self.maybe_run_cycle("webhook").await;
                    // Reset the interval so we don't sync again too soon
                    interval.reset();
                }
            }
        }

        info!("scheduler stopped");
    }

    /// Attempt to run a sync cycle for all registered engines.
    /// Engines that are already running are skipped. Each engine runs in its
    /// own spawned task so that multiple repos sync concurrently.
    async fn maybe_run_cycle(&self, trigger: &str) {
        let engines = self.engines.read().await;
        for (repo_id, engine) in engines.iter() {
            // The sync engine has its own atomic lock; check it first.
            if engine.is_running() {
                warn!(trigger, repo_id, "skipping sync cycle: previous cycle still running");
                continue;
            }

            let cycle_num = self.stats.total_cycles.fetch_add(1, Ordering::SeqCst) + 1;
            info!(cycle = cycle_num, trigger, repo_id, "starting sync cycle");

            // Broadcast sync started
            let start_msg = serde_json::json!({
                "type": "sync_started",
                "cycle": cycle_num,
                "trigger": trigger,
                "repo_id": repo_id,
            });
            let _ = self.ws_broadcast.send(start_msg.to_string());

            let engine = engine.clone();
            let repo_id = repo_id.clone();
            let stats = self.stats.clone();
            let ws = self.ws_broadcast.clone();

            // Spawn the sync cycle. It runs async but contains blocking I/O
            // (std::process::Command for svn/git). Tokio's spawn uses the
            // multi-thread runtime so blocking calls on one thread don't
            // prevent other tasks from running — as long as the runtime has
            // enough worker threads (default = num_cpus).
            tokio::spawn(async move {
                match engine.run_sync_cycle().await {
                    Ok(sync_stats) => {
                        stats.consecutive_errors.store(0, Ordering::SeqCst);

                        stats
                            .total_conflicts
                            .fetch_add(sync_stats.conflicts_detected as u64, Ordering::SeqCst);

                        info!(
                            cycle = cycle_num,
                            repo_id,
                            svn_to_git = sync_stats.svn_to_git_count,
                            git_to_svn = sync_stats.git_to_svn_count,
                            conflicts = sync_stats.conflicts_detected,
                            auto_resolved = sync_stats.conflicts_auto_resolved,
                            "sync cycle completed successfully"
                        );

                        // Broadcast sync completed
                        let end_msg = serde_json::json!({
                            "type": "sync_completed",
                            "cycle": cycle_num,
                            "repo_id": repo_id,
                            "svn_to_git": sync_stats.svn_to_git_count,
                            "git_to_svn": sync_stats.git_to_svn_count,
                            "conflicts": sync_stats.conflicts_detected,
                            "conflicts_auto_resolved": sync_stats.conflicts_auto_resolved,
                            "started_at": sync_stats.started_at,
                            "completed_at": sync_stats.completed_at,
                        });
                        let _ = ws.send(end_msg.to_string());
                    }
                    Err(e) => {
                        let errors = stats.total_errors.fetch_add(1, Ordering::SeqCst) + 1;
                        let consecutive = stats.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;
                        error!(
                            cycle = cycle_num,
                            repo_id,
                            error = %e,
                            total_errors = errors,
                            consecutive_errors = consecutive,
                            "sync cycle failed"
                        );

                        // Broadcast failure
                        let err_msg = serde_json::json!({
                            "type": "sync_failed",
                            "cycle": cycle_num,
                            "repo_id": repo_id,
                            "error": e.to_string(),
                        });
                        let _ = ws.send(err_msg.to_string());
                    }
                }
            });
        }
    }
}

//! Sync scheduler that runs sync cycles on a configurable interval and
//! supports webhook-triggered immediate syncs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, Notify};
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
/// immediate sync requests. The sync engine's own lock prevents concurrent
/// cycles, so the scheduler simply skips if the engine reports already running.
pub struct Scheduler {
    sync_engine: Arc<SyncEngine>,
    poll_interval: Duration,
    sync_rx: mpsc::Receiver<()>,
    ws_broadcast: broadcast::Sender<String>,
    stats: Arc<SchedulerStats>,
}

impl Scheduler {
    pub fn new(
        sync_engine: Arc<SyncEngine>,
        poll_interval: Duration,
        sync_rx: mpsc::Receiver<()>,
        ws_broadcast: broadcast::Sender<String>,
    ) -> Self {
        Self {
            sync_engine,
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

    /// Attempt to run a sync cycle. If the engine is already running, skip.
    async fn maybe_run_cycle(&self, trigger: &str) {
        // The sync engine has its own atomic lock; check it first.
        if self.sync_engine.is_running() {
            warn!(trigger, "skipping sync cycle: previous cycle still running");
            return;
        }

        let cycle_num = self.stats.total_cycles.fetch_add(1, Ordering::SeqCst) + 1;
        info!(cycle = cycle_num, trigger, "starting sync cycle");

        // Broadcast sync started
        let start_msg = serde_json::json!({
            "type": "sync_started",
            "cycle": cycle_num,
            "trigger": trigger,
        });
        let _ = self.ws_broadcast.send(start_msg.to_string());

        // Run the sync cycle in a spawned task so blocking I/O
        // (libgit2 fetch, svn commands) doesn't block the scheduler
        // or starve the tokio runtime.
        let engine = self.sync_engine.clone();
        let sched_stats = self.stats.clone();
        let ws = self.ws_broadcast.clone();

        // Run the sync cycle in a tokio::spawn task. The sync engine
        // uses tokio::sync::Mutex internally, so it must run on the
        // same runtime. Blocking I/O (libgit2, CLI commands) will
        // temporarily occupy a worker thread, but with worker_threads=16
        // the web server has plenty of capacity.
        //
        // The key stability fix: we check is_running() above so at most
        // ONE sync cycle runs at a time, using at most ONE worker thread.
        tokio::spawn(async move {
            match engine.run_sync_cycle().await {
                Ok(sync_stats) => {
                    sched_stats.consecutive_errors.store(0, Ordering::SeqCst);
                    sched_stats
                        .total_conflicts
                        .fetch_add(sync_stats.conflicts_detected as u64, Ordering::SeqCst);

                    info!(
                        cycle = cycle_num,
                        svn_to_git = sync_stats.svn_to_git_count,
                        git_to_svn = sync_stats.git_to_svn_count,
                        conflicts = sync_stats.conflicts_detected,
                        auto_resolved = sync_stats.conflicts_auto_resolved,
                        "sync cycle completed successfully"
                    );

                    let end_msg = serde_json::json!({
                        "type": "sync_completed",
                        "cycle": cycle_num,
                        "svn_to_git": sync_stats.svn_to_git_count,
                        "git_to_svn": sync_stats.git_to_svn_count,
                        "conflicts": sync_stats.conflicts_detected,
                    });
                    let _ = ws.send(end_msg.to_string());
                }
                Err(e) => {
                    let errors = sched_stats.total_errors.fetch_add(1, Ordering::SeqCst) + 1;
                    let consecutive = sched_stats.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;
                    error!(
                        cycle = cycle_num,
                        error = %e,
                        total_errors = errors,
                        consecutive_errors = consecutive,
                        "sync cycle failed"
                    );

                    let err_msg = serde_json::json!({
                        "type": "sync_failed",
                        "cycle": cycle_num,
                        "error": e.to_string(),
                    });
                    let _ = ws.send(err_msg.to_string());
                }
            }
        });
    }
}

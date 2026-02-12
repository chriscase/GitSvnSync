//! Sync scheduler that runs sync cycles on a configurable interval and
//! supports webhook-triggered immediate syncs.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
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
/// immediate sync requests. If a sync cycle is already running, the
/// scheduler skips the next cycle rather than queuing up.
pub struct Scheduler {
    sync_engine: Arc<SyncEngine>,
    poll_interval: Duration,
    sync_rx: mpsc::Receiver<()>,
    ws_broadcast: broadcast::Sender<String>,
    running: Arc<AtomicBool>,
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
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(SchedulerStats::new()),
        }
    }

    /// Main scheduler loop.
    ///
    /// This method runs until the task is cancelled (via abort or shutdown).
    pub async fn run(&mut self) {
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
    }

    /// Attempt to run a sync cycle. If one is already running, skip.
    async fn maybe_run_cycle(&self, trigger: &str) {
        // Check if a sync is already in progress (try_lock semantics)
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            warn!(
                trigger,
                "skipping sync cycle: previous cycle still running"
            );
            return;
        }

        let cycle_num =
            self.stats.total_cycles.fetch_add(1, Ordering::SeqCst) + 1;
        info!(cycle = cycle_num, trigger, "starting sync cycle");

        // Broadcast sync started
        let start_msg = serde_json::json!({
            "type": "sync_started",
            "cycle": cycle_num,
            "trigger": trigger,
        });
        let _ = self.ws_broadcast.send(start_msg.to_string());

        // Run the sync cycle
        match self.sync_engine.run_sync_cycle().await {
            Ok(stats) => {
                self.stats.consecutive_errors.store(0, Ordering::SeqCst);

                self.stats
                    .total_conflicts
                    .fetch_add(stats.conflicts_detected as u64, Ordering::SeqCst);

                info!(
                    cycle = cycle_num,
                    svn_to_git = stats.svn_to_git_count,
                    git_to_svn = stats.git_to_svn_count,
                    conflicts = stats.conflicts_detected,
                    auto_resolved = stats.conflicts_auto_resolved,
                    "sync cycle completed successfully"
                );

                // Broadcast sync completed
                let end_msg = serde_json::json!({
                    "type": "sync_completed",
                    "cycle": cycle_num,
                    "svn_to_git": stats.svn_to_git_count,
                    "git_to_svn": stats.git_to_svn_count,
                    "conflicts": stats.conflicts_detected,
                    "conflicts_auto_resolved": stats.conflicts_auto_resolved,
                    "started_at": stats.started_at,
                    "completed_at": stats.completed_at,
                });
                let _ = self.ws_broadcast.send(end_msg.to_string());
            }
            Err(e) => {
                let errors =
                    self.stats.total_errors.fetch_add(1, Ordering::SeqCst) + 1;
                let consecutive = self
                    .stats
                    .consecutive_errors
                    .fetch_add(1, Ordering::SeqCst)
                    + 1;
                error!(
                    cycle = cycle_num,
                    error = %e,
                    total_errors = errors,
                    consecutive_errors = consecutive,
                    "sync cycle failed"
                );

                // Broadcast failure
                let err_msg = serde_json::json!({
                    "type": "sync_failed",
                    "cycle": cycle_num,
                    "error": e.to_string(),
                });
                let _ = self.ws_broadcast.send(err_msg.to_string());
            }
        }

        self.running.store(false, Ordering::SeqCst);
    }
}

//! Status and health check endpoints.

use std::path::Path;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Optional query parameters for repo-scoped endpoints.
#[derive(Debug, Deserialize)]
struct RepoQuery {
    repo_id: Option<String>,
}

/// Health check response.
#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    version: String,
}

/// Status response wrapping the core SyncStatus.
#[derive(Serialize)]
struct StatusResponse {
    state: String,
    last_sync_at: Option<String>,
    last_svn_revision: Option<i64>,
    last_git_hash: Option<String>,
    total_syncs: i64,
    total_conflicts: i64,
    active_conflicts: i64,
    total_errors: i64,
    last_error_at: Option<String>,
    uptime_secs: u64,
}

/// Real-time system metrics for display during import operations.
#[derive(Serialize)]
struct SystemMetrics {
    disk_free_bytes: u64,
    disk_total_bytes: u64,
    disk_usage_percent: f64,
    mem_used_bytes: u64,
    mem_total_bytes: u64,
    mem_usage_percent: f64,
    cpu_load_1m: f64,
    cpu_load_5m: f64,
    cpu_load_15m: f64,
    git_push_active: bool,
    git_push_pid: Option<u32>,
    git_push_elapsed_secs: Option<u64>,
    data_dir_size_bytes: u64,
    /// Network bytes sent since boot (from /proc/net/dev)
    net_bytes_sent: u64,
    /// Network bytes received since boot
    net_bytes_recv: u64,
    /// Network upload rate (bytes/sec), computed server-side
    net_up_bytes_per_sec: f64,
    /// Network download rate (bytes/sec), computed server-side
    net_down_bytes_per_sec: f64,
    /// SVN process active (svn export/log/info running)
    svn_active: bool,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/status/health", get(health_check))
        .route("/api/status/system", get(get_system_metrics))
        .route("/api/status/reset-errors", post(reset_errors))
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn get_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<RepoQuery>,
) -> Result<Json<StatusResponse>, AppError> {
    crate::api::auth::validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let engine = state.get_engine(query.repo_id.as_deref()).await;
    let status = engine
        .get_status()
        .map_err(|e| AppError::Internal(format!("failed to get sync status: {}", e)))?;

    Ok(Json(StatusResponse {
        state: status.state.to_string(),
        last_sync_at: status.last_sync_at.map(|t| t.to_rfc3339()),
        last_svn_revision: status.last_svn_revision,
        last_git_hash: status.last_git_hash,
        total_syncs: status.total_syncs,
        total_conflicts: status.total_conflicts,
        active_conflicts: status.active_conflicts,
        total_errors: status.total_errors,
        last_error_at: status.last_error_at,
        uptime_secs: status.uptime_secs,
    }))
}

async fn reset_errors(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    crate::api::auth::validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Internal(format!("db lock: {}", e)))?;

    let cleared = db
        .clear_errors()
        .map_err(|e| AppError::Internal(format!("database error: {}", e)))?;

    let _ = db.insert_audit_log(
        "errors_cleared",
        None,
        None,
        None,
        None,
        Some(&format!("Cleared {} error entries", cleared)),
        true,
    );

    Ok(Json(serde_json::json!({
        "ok": true,
        "cleared": cleared
    })))
}

async fn get_system_metrics(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SystemMetrics>, AppError> {
    crate::api::auth::validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let data_dir = &state.config.daemon.data_dir;

    // Disk metrics for the data directory.
    let (disk_free_bytes, disk_total_bytes) = disk_usage(data_dir);
    let disk_usage_percent = if disk_total_bytes > 0 {
        ((disk_total_bytes - disk_free_bytes) as f64 / disk_total_bytes as f64) * 100.0
    } else {
        0.0
    };

    // Memory metrics.
    let (mem_used_bytes, mem_total_bytes) = mem_usage();
    let mem_usage_percent = if mem_total_bytes > 0 {
        (mem_used_bytes as f64 / mem_total_bytes as f64) * 100.0
    } else {
        0.0
    };

    // CPU load averages.
    let (cpu_load_1m, cpu_load_5m, cpu_load_15m) = cpu_load();

    // Git push process detection.
    let (git_push_active, git_push_pid, git_push_elapsed_secs) = find_git_push_process();

    // Data directory (git-repo) size.
    let git_repo_path = data_dir.join("git-repo");
    let data_dir_size_bytes = dir_size(&git_repo_path);

    // Network I/O from /proc/net/dev
    let (net_bytes_sent, net_bytes_recv) = read_net_bytes();

    // Compute server-side network rates by diffing with previous snapshot
    let (net_up_bytes_per_sec, net_down_bytes_per_sec) = {
        let mut prev = state.prev_net_snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        let rates = if let Some((prev_sent, prev_recv, prev_time)) = prev.as_ref() {
            let dt = now.duration_since(*prev_time).as_secs_f64();
            if dt > 0.5 {
                let up = (net_bytes_sent.saturating_sub(*prev_sent)) as f64 / dt;
                let down = (net_bytes_recv.saturating_sub(*prev_recv)) as f64 / dt;
                (up, down)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };
        *prev = Some((net_bytes_sent, net_bytes_recv, now));
        rates
    };

    // SVN process detection + import awareness
    let svn_proc = is_process_running("svn");
    // Check all per-repo import progress entries for active imports
    let (any_importing, any_final_push) = {
        let map = state.import_progress.read().await;
        let mut importing = false;
        let mut pushing = false;
        for progress_lock in map.values() {
            let p = progress_lock.read().await;
            if matches!(p.phase, gitsvnsync_core::import::ImportPhase::Importing) {
                importing = true;
            }
            if matches!(p.phase, gitsvnsync_core::import::ImportPhase::FinalPush) {
                pushing = true;
            }
        }
        (importing, pushing)
    };
    let svn_active = svn_proc || any_importing;
    // Also detect push from import state
    let git_push_active = git_push_active || any_final_push;

    Ok(Json(SystemMetrics {
        disk_free_bytes,
        disk_total_bytes,
        disk_usage_percent,
        mem_used_bytes,
        mem_total_bytes,
        mem_usage_percent,
        cpu_load_1m,
        cpu_load_5m,
        cpu_load_15m,
        git_push_active,
        git_push_pid,
        git_push_elapsed_secs,
        data_dir_size_bytes,
        net_bytes_sent,
        net_bytes_recv,
        net_up_bytes_per_sec,
        net_down_bytes_per_sec,
        svn_active,
    }))
}

// ---------------------------------------------------------------------------
// System metric helpers
// ---------------------------------------------------------------------------

/// Return (free_bytes, total_bytes) for the filesystem containing `path`.
#[cfg(target_os = "linux")]
fn disk_usage(path: &Path) -> (u64, u64) {
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    let c_path = match CString::new(path.to_string_lossy().as_bytes()) {
        Ok(p) => p,
        Err(_) => return (0, 0),
    };

    unsafe {
        let mut buf = MaybeUninit::<libc::statvfs>::uninit();
        if libc::statvfs(c_path.as_ptr(), buf.as_mut_ptr()) == 0 {
            let stat = buf.assume_init();
            let total = stat.f_blocks as u64 * stat.f_frsize as u64;
            let free = stat.f_bavail as u64 * stat.f_frsize as u64;
            (free, total)
        } else {
            (0, 0)
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn disk_usage(_path: &Path) -> (u64, u64) {
    // Fallback: no disk info available on non-Linux platforms.
    (0, 0)
}

/// Return (used_bytes, total_bytes) from `/proc/meminfo`.
#[cfg(target_os = "linux")]
fn mem_usage() -> (u64, u64) {
    let content = match std::fs::read_to_string("/proc/meminfo") {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = parse_meminfo_kb(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available_kb = parse_meminfo_kb(rest);
        }
    }

    let total = total_kb * 1024;
    let used = total.saturating_sub(available_kb * 1024);
    (used, total)
}

#[cfg(target_os = "linux")]
fn parse_meminfo_kb(s: &str) -> u64 {
    s.trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn mem_usage() -> (u64, u64) {
    // Fallback defaults: report 0 on non-Linux.
    (0, 0)
}

/// Return (load_1m, load_5m, load_15m) from `/proc/loadavg`.
#[cfg(target_os = "linux")]
fn cpu_load() -> (f64, f64, f64) {
    let content = match std::fs::read_to_string("/proc/loadavg") {
        Ok(c) => c,
        Err(_) => return (0.0, 0.0, 0.0),
    };

    let mut parts = content.split_whitespace();
    let load_1m = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let load_5m = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let load_15m = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    (load_1m, load_5m, load_15m)
}

#[cfg(not(target_os = "linux"))]
fn cpu_load() -> (f64, f64, f64) {
    (0.0, 0.0, 0.0)
}

/// Scan `/proc` for a running `git push` process.
/// Returns (active, pid, elapsed_secs).
#[cfg(target_os = "linux")]
fn find_git_push_process() -> (bool, Option<u32>, Option<u64>) {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return (false, None, None);
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only look at numeric (PID) directories.
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let cmdline_path = entry.path().join("cmdline");
        if let Ok(cmdline) = std::fs::read_to_string(&cmdline_path) {
            // cmdline uses NUL separators; replace for easy matching.
            let cmdline_readable = cmdline.replace('\0', " ");
            if cmdline_readable.contains("git push")
                || cmdline_readable.contains("git-push")
            {
                // Try to read process start time from /proc/<pid>/stat for elapsed.
                let elapsed = process_elapsed_secs(pid);
                return (true, Some(pid), elapsed);
            }
        }
    }

    (false, None, None)
}

/// Compute how many seconds a process has been running using `/proc/<pid>/stat`
/// and `/proc/uptime`.
#[cfg(target_os = "linux")]
fn process_elapsed_secs(pid: u32) -> Option<u64> {
    // Read system uptime in seconds.
    let uptime_str = std::fs::read_to_string("/proc/uptime").ok()?;
    let uptime_secs: f64 = uptime_str.split_whitespace().next()?.parse().ok()?;

    // Read clock ticks per second (USER_HZ, typically 100).
    let clk_tck: f64 = 100.0;

    // Read /proc/<pid>/stat — field 22 (1-indexed) is starttime in clock ticks.
    let stat_str = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    // The comm field (field 2) may contain spaces/parens; find the closing paren
    // then split the rest.
    let after_comm = stat_str.rfind(')')?.checked_add(1)?;
    let fields: Vec<&str> = stat_str[after_comm..].split_whitespace().collect();
    // After the closing paren, field index 0 = state (field 3), so starttime
    // is at index 19 (field 22 − 3).
    let starttime_ticks: f64 = fields.get(19)?.parse().ok()?;

    let start_secs = starttime_ticks / clk_tck;
    let elapsed = uptime_secs - start_secs;
    if elapsed >= 0.0 {
        Some(elapsed as u64)
    } else {
        Some(0)
    }
}

#[cfg(not(target_os = "linux"))]
fn find_git_push_process() -> (bool, Option<u32>, Option<u64>) {
    (false, None, None)
}

/// Recursively compute the total size of a directory in bytes.
fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    dir_size_inner(path)
}

fn dir_size_inner(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            total += dir_size_inner(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

/// Read total network bytes sent/received from /proc/net/dev (Linux only).
/// Returns (bytes_sent, bytes_recv) summed across all non-loopback interfaces.
#[cfg(target_os = "linux")]
fn read_net_bytes() -> (u64, u64) {
    let content = match std::fs::read_to_string("/proc/net/dev") {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    let mut total_sent = 0u64;
    let mut total_recv = 0u64;
    for line in content.lines().skip(2) {
        let line = line.trim();
        if line.starts_with("lo:") {
            continue; // skip loopback
        }
        if let Some(data) = line.split(':').nth(1) {
            let fields: Vec<&str> = data.split_whitespace().collect();
            if fields.len() >= 10 {
                if let Ok(recv) = fields[0].parse::<u64>() {
                    total_recv += recv;
                }
                if let Ok(sent) = fields[8].parse::<u64>() {
                    total_sent += sent;
                }
            }
        }
    }
    (total_sent, total_recv)
}

#[cfg(not(target_os = "linux"))]
fn read_net_bytes() -> (u64, u64) {
    (0, 0)
}

/// Check if any process with the given name is running (Linux only).
#[cfg(target_os = "linux")]
fn is_process_running(name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if fname_str.chars().all(|c| c.is_ascii_digit()) {
            let cmdline_path = entry.path().join("cmdline");
            if let Ok(cmdline) = std::fs::read_to_string(&cmdline_path) {
                if cmdline.contains(name) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "linux"))]
fn is_process_running(_name: &str) -> bool {
    false
}

// ---------------------------------------------------------------------------
// Shared error type for API handlers
// ---------------------------------------------------------------------------

/// Simple API error type that converts to an Axum response.
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Unauthorized(String),
    Internal(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (axum::http::StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized(msg) => (axum::http::StatusCode::UNAUTHORIZED, msg),
            AppError::Internal(msg) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = serde_json::json!({ "error": message });
        (status, Json(body)).into_response()
    }
}

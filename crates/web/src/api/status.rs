//! Status and health check endpoints.

use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::AppState;

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
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/status/health", get(health_check))
        .route("/api/status/system", get(get_system_metrics))
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
) -> Result<Json<StatusResponse>, AppError> {
    crate::api::auth::validate_session(
        &state,
        headers.get("authorization").and_then(|v| v.to_str().ok()),
    )
    .await?;

    let status = state
        .sync_engine
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
        uptime_secs: status.uptime_secs,
    }))
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

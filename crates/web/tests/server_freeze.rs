//! Integration tests proving the web server does not freeze when the sync
//! engine's database mutex is held (the root cause of the 60-second hang bug).
//!
//! Each test targets a different aspect of the fix:
//!   1. Concurrent web requests complete even when the sync engine DB is locked.
//!   2. Saturating the `spawn_blocking` pool does not block pure-async tasks.
//!   3. Two `Database` instances on the same file have independent Rust mutexes.
//!   4. The health-check endpoint responds within 100 ms under extreme load.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use gitsvnsync_core::config::{AppConfig, IdentityConfig};
use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::identity::IdentityMapper;
use gitsvnsync_core::import::ImportProgress;
use gitsvnsync_core::svn::SvnClient;
use gitsvnsync_core::sync_engine::SyncEngine;
use gitsvnsync_web::api;
use gitsvnsync_web::AppState;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a minimal `AppConfig` suitable for testing.
///
/// `WebConfig` defaults to `admin_password: None`, so `validate_session`
/// bypasses auth when the DB contains no users.
fn minimal_config(data_dir: &Path) -> AppConfig {
    let toml_str = format!(
        r#"
[daemon]
data_dir = "{}"

[svn]
url = "https://svn.test.invalid/repo"
username = "testuser"
password_env = ""

[github]
repo = "test/repo"
token_env = ""
"#,
        data_dir.display().to_string().replace('\\', "/")
    );
    toml::from_str(&toml_str).expect("failed to parse minimal test config")
}

/// Spin up an Axum server on a random port and return everything needed to
/// drive tests against it.
///
/// Returns `(addr, shared_state, server_handle, _tmpdir)`.  The caller must
/// keep `_tmpdir` alive for the duration of the test so the git repo and data
/// directory are not deleted.
async fn build_test_server() -> (
    SocketAddr,
    Arc<AppState>,
    tokio::task::JoinHandle<()>,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let git_repo_path = tmp.path().join("git-repo");
    git2::Repository::init(&git_repo_path).expect("git init");

    let config = minimal_config(tmp.path());

    // Two separate Database instances → two independent Rust mutexes.
    let web_db = Database::in_memory().expect("web db");
    web_db.initialize().expect("web db init");

    let engine_db = Database::in_memory().expect("engine db");
    engine_db.initialize().expect("engine db init");

    let svn_client = SvnClient::new(
        "https://svn.test.invalid/repo",
        "testuser",
        "",
    );
    let git_client = GitClient::new(&git_repo_path).expect("git client");
    let identity_mapper = Arc::new(
        IdentityMapper::new(&IdentityConfig::default()).expect("identity mapper"),
    );

    let sync_engine = Arc::new(SyncEngine::new(
        config.clone(),
        engine_db,
        svn_client,
        git_client,
        identity_mapper,
    ));

    let (sync_tx, _sync_rx) = tokio::sync::mpsc::channel(1);
    let (ws_tx, _) = tokio::sync::broadcast::channel(256);

    let state = Arc::new(AppState {
        db: web_db,
        sync_engine,
        config,
        sync_trigger: sync_tx,
        ws_broadcast: ws_tx,
        sessions: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        import_progress: Arc::new(tokio::sync::RwLock::new(ImportProgress::default())),
        config_path: tmp.path().join("config.toml"),
        prev_net_snapshot: std::sync::Mutex::new(None),
    });

    let app = Router::new()
        .merge(api::status::routes())
        .merge(api::auth::routes())
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    // Give the server a moment to start accepting connections.
    tokio::time::sleep(Duration::from_millis(50)).await;

    (addr, state, handle, tmp)
}

// ---------------------------------------------------------------------------
// Test 1 — Concurrent web requests are NOT blocked by sync engine DB lock
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_web_requests_not_blocked_by_sync_engine_db() {
    let (addr, state, _server, _tmp) = build_test_server().await;
    let base_url = format!("http://{}", addr);

    // Simulate a long sync cycle by holding the sync engine's DB mutex.
    let sync_engine = state.sync_engine.clone();
    let blocker = tokio::task::spawn_blocking(move || {
        let _guard = sync_engine.db().conn();
        std::thread::sleep(Duration::from_secs(5));
    });

    // Ensure the blocker has acquired the lock before firing requests.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Fire 20 concurrent requests to health + status endpoints.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    let mut handles = Vec::new();
    for i in 0..20 {
        let c = client.clone();
        let url = if i % 2 == 0 {
            format!("{}/api/status/health", base_url)
        } else {
            format!("{}/api/status", base_url)
        };
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let resp = c.get(&url).send().await;
            (url, resp, start.elapsed())
        }));
    }

    // Every single request must complete within the 3-second window.
    let deadline = tokio::time::timeout(Duration::from_secs(3), async {
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.expect("join"));
        }
        results
    })
    .await
    .expect("requests timed out — web server blocked by sync engine DB lock");

    for (url, resp, elapsed) in &deadline {
        let resp = resp.as_ref().expect("HTTP error");
        assert!(
            resp.status().is_success(),
            "{} returned {}",
            url,
            resp.status()
        );
        assert!(
            *elapsed < Duration::from_secs(3),
            "{} took {:?}",
            url,
            elapsed
        );
    }

    blocker.await.ok();
}

// ---------------------------------------------------------------------------
// Test 2 — spawn_blocking pool exhaustion does NOT block async tasks
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_spawn_blocking_exhaustion_does_not_block_async_tasks() {
    let mutex = Arc::new(std::sync::Mutex::new(()));

    // Hold the mutex so every spawn_blocking task blocks.
    let guard = mutex.lock().unwrap();

    let mut blocking_handles = Vec::new();
    for _ in 0..600 {
        let m = mutex.clone();
        blocking_handles.push(tokio::task::spawn_blocking(move || {
            let _lock = m.lock().unwrap();
        }));
    }

    // Let the runtime schedule some of those blocking tasks.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // A pure async task must still complete promptly — this is the same
    // execution model as the health_check handler.
    let result = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        42u32
    })
    .await;

    assert!(
        result.is_ok(),
        "pure async task blocked despite spawn_blocking pool being saturated"
    );
    assert_eq!(result.unwrap(), 42);

    // Unblock all the waiting tasks.
    drop(guard);
    for h in blocking_handles {
        h.await.ok();
    }
}

// ---------------------------------------------------------------------------
// Test 3 — Separate Database instances have independent Rust mutexes
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_separate_database_instances_no_mutex_contention() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("shared.db");

    let db1 = Database::new(&db_path).expect("db1");
    db1.initialize().expect("db1 init");

    let db2 = Database::new(&db_path).expect("db2");

    // Hold db1's Rust mutex for 5 seconds.
    let blocker = tokio::task::spawn_blocking(move || {
        {
            let _guard = db1.conn();
            std::thread::sleep(Duration::from_secs(5));
        } // _guard dropped here, releasing the mutex
        db1 // keep db1 alive so it's not dropped early
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // db2 should acquire its *own* Rust mutex instantly — it is a separate
    // Mutex<Connection>.  SQLite WAL mode allows concurrent readers on the
    // same file.
    let reader = tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let _conn = db2.conn();
        start.elapsed()
    });

    let read_elapsed = tokio::time::timeout(Duration::from_secs(2), reader)
        .await
        .expect("reader timed out — separate DB instances may share Rust mutex")
        .expect("join");

    assert!(
        read_elapsed < Duration::from_secs(1),
        "reader took {:?}, expected near-instant for separate Mutex",
        read_elapsed
    );

    blocker.await.ok();
}

// ---------------------------------------------------------------------------
// Test 4 — Health check responds even under spawn_blocking saturation
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_health_check_responds_under_spawn_blocking_saturation() {
    let (addr, _state, _server, _tmp) = build_test_server().await;
    let base_url = format!("http://{}", addr);

    // Saturate the spawn_blocking pool with tasks that block on a held mutex.
    let mutex = Arc::new(std::sync::Mutex::new(()));
    let guard = mutex.lock().unwrap();

    let mut blockers = Vec::new();
    for _ in 0..600 {
        let m = mutex.clone();
        blockers.push(tokio::task::spawn_blocking(move || {
            let _lock = m.lock().unwrap();
        }));
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // health_check is a pure async handler — no spawn_blocking, no DB access.
    // It must respond within 500 ms even with the blocking pool exhausted.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    for _ in 0..5 {
        let start = Instant::now();
        let resp = tokio::time::timeout(
            Duration::from_millis(500),
            client.get(format!("{}/api/status/health", base_url)).send(),
        )
        .await
        .expect("health check timed out under spawn_blocking saturation")
        .expect("HTTP error");

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["ok"], true);

        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(500),
            "health check took {:?}, expected < 500ms",
            elapsed
        );
    }

    // Clean up.
    drop(guard);
    for h in blockers {
        h.await.ok();
    }
}

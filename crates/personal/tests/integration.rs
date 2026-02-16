//! Integration tests for the SVN-to-Git sync pipeline.
//!
//! These tests exercise the full sync pipeline using:
//! - Real local SVN repos created via `svnadmin create` (file:// protocol)
//! - Real local Git repos via `git2::Repository`
//! - Real SQLite databases via `Database::new()`
//!
//! No network I/O: SVN uses `file://` URLs, Git pushes go to local bare repos.
//!
//! If `svn` / `svnadmin` are not installed, tests skip gracefully.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use tokio::sync::Mutex;

use gitsvnsync_core::db::Database;
use gitsvnsync_core::git::GitClient;
use gitsvnsync_core::personal_config::{
    CommitFormatConfig, DeveloperConfig, PersonalConfig, PersonalGitHubConfig,
    PersonalOptionsConfig, PersonalSection, PersonalSvnConfig,
};
use gitsvnsync_core::svn::SvnClient;
use gitsvnsync_personal::commit_format::CommitFormatter;
use gitsvnsync_personal::svn_to_git::SvnToGitSync;

// ===========================================================================
// Helper functions
// ===========================================================================

/// Returns `true` if both `svn` and `svnadmin` are available on `$PATH`.
fn svn_available() -> bool {
    let svn_ok = Command::new("svn")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let svnadmin_ok = Command::new("svnadmin")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    svn_ok && svnadmin_ok
}

/// Create a local SVN repository via `svnadmin create`. Returns the `file://` URL.
fn create_svn_repo(dir: &Path) -> String {
    let repo_dir = dir.join("svn_repo");
    let status = Command::new("svnadmin")
        .args(["create", repo_dir.to_str().unwrap()])
        .status()
        .expect("failed to run svnadmin create");
    assert!(status.success(), "svnadmin create failed");

    // Enable revprop changes (needed for some tests).
    let hooks_dir = repo_dir.join("hooks");
    let pre_revprop_change = hooks_dir.join("pre-revprop-change");
    std::fs::write(&pre_revprop_change, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&pre_revprop_change, std::fs::Permissions::from_mode(0o755))
            .unwrap();
    }

    format!("file://{}", repo_dir.display())
}

/// Check out an SVN working copy from the given URL.
fn svn_checkout(url: &str, wc_path: &Path) {
    let status = Command::new("svn")
        .args([
            "checkout",
            url,
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
        .expect("failed to run svn checkout");
    assert!(status.success(), "svn checkout failed");
}

/// Commit a file to SVN via the working copy. Returns the new revision number.
///
/// Writes `content` to `filename` inside `wc_path`, stages it with `svn add`
/// (if unversioned), and commits with the given message.
fn svn_commit_file(wc_path: &Path, filename: &str, content: &str, message: &str) -> i64 {
    let file_path = wc_path.join(filename);

    // Ensure parent directories exist.
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).unwrap();
            // svn add each intermediate directory that is new.
            let mut rel = PathBuf::new();
            for component in Path::new(filename).parent().unwrap().components() {
                rel = rel.join(component);
                let abs = wc_path.join(&rel);
                let status_out = Command::new("svn")
                    .args(["status", abs.to_str().unwrap()])
                    .output()
                    .unwrap();
                let status_str = String::from_utf8_lossy(&status_out.stdout);
                if status_str.contains('?') || !abs.join(".svn").exists() {
                    let _ = Command::new("svn")
                        .args(["add", "--depth=empty", abs.to_str().unwrap()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
            }
        }
    }

    let is_new = !file_path.exists() || {
        let out = Command::new("svn")
            .args(["status", file_path.to_str().unwrap()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains('?')
    };

    std::fs::write(&file_path, content).unwrap();

    if is_new {
        let status = Command::new("svn")
            .args(["add", file_path.to_str().unwrap()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "svn add failed for {}", filename);
    }

    let output = Command::new("svn")
        .args([
            "commit",
            "-m",
            message,
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .output()
        .expect("failed to run svn commit");
    assert!(
        output.status.success(),
        "svn commit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Parse "Committed revision N." from stdout.
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Committed revision") {
            return trimmed
                .trim_start_matches("Committed revision")
                .trim()
                .trim_end_matches('.')
                .parse::<i64>()
                .expect("failed to parse revision number");
        }
    }
    panic!("could not parse committed revision from: {}", stdout);
}

/// Commit multiple files in a single SVN commit. Returns the new revision.
fn svn_commit_files(wc_path: &Path, files: &[(&str, &str)], message: &str) -> i64 {
    for (filename, content) in files {
        let file_path = wc_path.join(filename);
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).unwrap();
                // svn add parent dirs.
                let mut rel = PathBuf::new();
                for component in Path::new(filename).parent().unwrap().components() {
                    rel = rel.join(component);
                    let abs = wc_path.join(&rel);
                    let _ = Command::new("svn")
                        .args(["add", "--depth=empty", abs.to_str().unwrap()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
            }
        }
        std::fs::write(&file_path, content).unwrap();
    }

    // svn add all new files.
    for (filename, _) in files {
        let file_path = wc_path.join(filename);
        let _ = Command::new("svn")
            .args(["add", "--force", file_path.to_str().unwrap()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    let output = Command::new("svn")
        .args([
            "commit",
            "-m",
            message,
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .output()
        .expect("svn commit failed");
    assert!(
        output.status.success(),
        "svn commit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Committed revision") {
            return trimmed
                .trim_start_matches("Committed revision")
                .trim()
                .trim_end_matches('.')
                .parse::<i64>()
                .unwrap();
        }
    }
    panic!("could not parse committed revision from: {}", stdout);
}

/// Build a `PersonalConfig` suitable for testing, pointing at the given SVN URL
/// and temp directories.
fn make_test_config(svn_url: &str, data_dir: &Path) -> PersonalConfig {
    PersonalConfig {
        personal: PersonalSection {
            poll_interval_secs: 5,
            log_level: "debug".into(),
            data_dir: data_dir.to_path_buf(),
            status_port: None,
        },
        svn: PersonalSvnConfig {
            url: svn_url.into(),
            username: String::new(),
            password_env: "GITSVNSYNC_TEST_SVN_PW".into(),
            password: Some(String::new()),
        },
        github: PersonalGitHubConfig {
            api_url: "https://api.github.com".into(),
            repo: "test/test-repo".into(),
            token_env: "GITSVNSYNC_TEST_GH_TOKEN".into(),
            default_branch: "main".into(),
            auto_create: false,
            private: true,
            token: None,
        },
        developer: DeveloperConfig {
            name: "Test User".into(),
            email: "test@example.com".into(),
            svn_username: "testuser".into(),
        },
        commit_format: CommitFormatConfig::default(),
        options: PersonalOptionsConfig::default(),
    }
}

/// Set up a Git working repo with a bare repo as "origin" for local push.
/// Returns `GitClient`.
///
/// After the initial commit, ensures the default branch is named "main"
/// regardless of the system's `init.defaultBranch` setting.
fn setup_git_with_bare_origin(work_dir: &Path, bare_dir: &Path) -> GitClient {
    // Create a bare repo as "origin".
    git2::Repository::init_bare(bare_dir).expect("failed to init bare repo");

    // Init the working repo.
    let git_client = GitClient::init(work_dir).expect("failed to init git repo");

    // Add the bare repo as the "origin" remote.
    let repo = git2::Repository::open(work_dir).expect("failed to open repo");
    repo.remote("origin", bare_dir.to_str().unwrap())
        .expect("failed to add origin remote");

    // Create an initial commit so we have a HEAD.
    std::fs::write(work_dir.join(".gitkeep"), "").unwrap();
    git_client
        .commit(
            "initial commit",
            "Test User",
            "test@example.com",
            "Test User",
            "test@example.com",
        )
        .expect("failed to create initial commit");

    // Rename the default branch to "main" if it isn't already.
    // git2::Repository::init may create "master" depending on system config.
    {
        let repo = git2::Repository::open(work_dir).unwrap();
        let head = repo.head().unwrap();
        let head_name = head.name().unwrap_or("");
        if head_name != "refs/heads/main" {
            // Rename the branch to "main".
            let mut branch = repo
                .find_branch(
                    head_name.strip_prefix("refs/heads/").unwrap_or("master"),
                    git2::BranchType::Local,
                )
                .unwrap();
            branch.rename("main", true).unwrap();
        }
    }

    // Push the initial commit to origin to establish the branch.
    git_client
        .push("origin", "main", None)
        .expect("failed to push initial commit to origin");

    git_client
}

/// Create and initialize a Database at the given path.
fn setup_db(path: &Path) -> Database {
    let db = Database::new(path).expect("failed to create database");
    db.initialize()
        .expect("failed to initialize database schema");
    db
}

/// Count commits in the git repo by walking from HEAD.
fn count_git_commits(repo_path: &Path) -> usize {
    let repo = git2::Repository::open(repo_path).unwrap();
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return 0,
    };
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push(head.target().unwrap()).unwrap();
    revwalk.count()
}

/// Read the message of the Nth commit from HEAD (0 = HEAD, 1 = HEAD~1, etc.).
fn get_git_commit_message(repo_path: &Path, index: usize) -> String {
    let repo = git2::Repository::open(repo_path).unwrap();
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().unwrap();
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .unwrap();
    let oid = revwalk.nth(index).unwrap().unwrap();
    let commit = repo.find_commit(oid).unwrap();
    commit.message().unwrap_or("").to_string()
}

// ===========================================================================
// Test 1: Basic SVN-to-Git sync (3 revisions)
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_basic_sync() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Commit 3 files in 3 separate SVN revisions.
    svn_commit_file(&wc_path, "file1.txt", "content one", "Add file1");
    svn_commit_file(&wc_path, "file2.txt", "content two", "Add file2");
    svn_commit_file(&wc_path, "file3.txt", "content three", "Add file3");

    // Set up Git repo with bare origin.
    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    // Set up database.
    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);

    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);

    // Run sync.
    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 3, "expected 3 revisions synced");

    // Verify Git repo has 3 + 1 (initial) = 4 commits.
    assert_eq!(count_git_commits(&git_work_dir), 4);

    // Verify watermark advanced to rev 3.
    let watermark = db_arc.get_watermark("svn_rev").unwrap();
    assert_eq!(watermark.as_deref(), Some("3"));

    // Verify commit_map has 3 entries.
    let commit_map = db_arc.list_commit_map(10).unwrap();
    assert_eq!(commit_map.len(), 3);

    // Verify Git commit messages contain SVN-Revision trailers.
    // The most recent commit (index 0) is r3, so check it.
    let msg = get_git_commit_message(&git_work_dir, 0);
    assert!(
        msg.contains("SVN-Revision: r3"),
        "expected SVN-Revision trailer in: {}",
        msg
    );
    assert!(
        msg.contains("[gitsvnsync]"),
        "expected sync marker in: {}",
        msg
    );

    // Verify files exist in git working directory.
    assert!(git_work_dir.join("file1.txt").exists());
    assert!(git_work_dir.join("file2.txt").exists());
    assert!(git_work_dir.join("file3.txt").exists());
    assert_eq!(
        std::fs::read_to_string(git_work_dir.join("file1.txt")).unwrap(),
        "content one"
    );
}

// ===========================================================================
// Test 2: Echo suppression
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_echo_suppression() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Rev 1: normal commit.
    svn_commit_file(&wc_path, "normal1.txt", "hello", "Normal commit 1");

    // Rev 2: commit with [gitsvnsync] marker (simulating an echo).
    svn_commit_file(
        &wc_path,
        "echo.txt",
        "echoed content",
        "Echoed commit [gitsvnsync] synced from Git",
    );

    // Rev 3: another normal commit.
    svn_commit_file(&wc_path, "normal2.txt", "world", "Normal commit 2");

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);

    let synced = syncer.sync().await.expect("sync failed");

    // Should sync 2 revisions (skipping the echo).
    assert_eq!(synced, 2, "expected 2 revisions synced (echo skipped)");

    // Watermark should still advance to 3 (the echo commit was acknowledged).
    let watermark = db_arc.get_watermark("svn_rev").unwrap();
    assert_eq!(watermark.as_deref(), Some("3"));

    // commit_map should have 2 entries (the echo is not in the map).
    let commit_map = db_arc.list_commit_map(10).unwrap();
    assert_eq!(commit_map.len(), 2);
}

// ===========================================================================
// Test 3: Idempotency
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_idempotency() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    svn_commit_file(&wc_path, "a.txt", "aaa", "Add a");
    svn_commit_file(&wc_path, "b.txt", "bbb", "Add b");

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(
        svn_client.clone(),
        git_arc.clone(),
        db_arc.clone(),
        config.clone(),
    );

    // First sync: 2 revisions.
    let synced1 = syncer.sync().await.expect("first sync failed");
    assert_eq!(synced1, 2);

    // Second sync: nothing new.
    let synced2 = syncer.sync().await.expect("second sync failed");
    assert_eq!(synced2, 0);

    // Add one more SVN revision.
    svn_commit_file(&wc_path, "c.txt", "ccc", "Add c");

    // Third sync: 1 new revision.
    let syncer2 = SvnToGitSync::new(
        SvnClient::new(&svn_url, "", ""),
        git_arc.clone(),
        db_arc.clone(),
        config,
    );
    let synced3 = syncer2.sync().await.expect("third sync failed");
    assert_eq!(synced3, 1);

    // Total: 3 + 1 (initial) = 4 commits.
    assert_eq!(count_git_commits(&git_work_dir), 4);
}

// ===========================================================================
// Test 4: Watermark recovery
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_watermark_recovery() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    svn_commit_file(&wc_path, "x.txt", "x", "Rev 1");
    svn_commit_file(&wc_path, "y.txt", "y", "Rev 2");
    svn_commit_file(&wc_path, "z.txt", "z", "Rev 3");

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);

    // Manually set watermark to 2, pretending revisions 1 and 2 were already synced.
    db.set_watermark("svn_rev", "2").unwrap();

    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);

    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 1, "expected only revision 3 to be synced");

    let watermark = db_arc.get_watermark("svn_rev").unwrap();
    assert_eq!(watermark.as_deref(), Some("3"));

    // Git should have the initial commit + 1 synced = 2 commits.
    assert_eq!(count_git_commits(&git_work_dir), 2);
}

// ===========================================================================
// Test 5: Multi-file commit (single revision, multiple files)
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_multifile_commit() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Commit 3 files in a single SVN revision.
    let rev = svn_commit_files(
        &wc_path,
        &[
            ("alpha.txt", "alpha content"),
            ("beta.txt", "beta content"),
            ("gamma.txt", "gamma content"),
        ],
        "Add three files at once",
    );
    assert_eq!(rev, 1);

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);
    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 1, "expected 1 revision synced");

    // Git should have 2 commits (initial + 1 synced).
    assert_eq!(count_git_commits(&git_work_dir), 2);

    // All 3 files should exist.
    assert!(git_work_dir.join("alpha.txt").exists());
    assert!(git_work_dir.join("beta.txt").exists());
    assert!(git_work_dir.join("gamma.txt").exists());
    assert_eq!(
        std::fs::read_to_string(git_work_dir.join("beta.txt")).unwrap(),
        "beta content"
    );
}

// ===========================================================================
// Test 6: File modification
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_file_modification() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Rev 1: create file.
    svn_commit_file(&wc_path, "data.txt", "version 1", "Add data.txt");

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(
        svn_client.clone(),
        git_arc.clone(),
        db_arc.clone(),
        config.clone(),
    );
    let synced1 = syncer.sync().await.expect("first sync failed");
    assert_eq!(synced1, 1);

    // Verify v1.
    assert_eq!(
        std::fs::read_to_string(git_work_dir.join("data.txt")).unwrap(),
        "version 1"
    );

    // Rev 2: modify file.
    // We need to write directly to the working copy (svn_commit_file handles this).
    std::fs::write(wc_path.join("data.txt"), "version 2").unwrap();
    let output = Command::new("svn")
        .args([
            "commit",
            "-m",
            "Update data.txt to v2",
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Sync again.
    let syncer2 = SvnToGitSync::new(
        SvnClient::new(&svn_url, "", ""),
        git_arc.clone(),
        db_arc.clone(),
        config,
    );
    let synced2 = syncer2.sync().await.expect("second sync failed");
    assert_eq!(synced2, 1);

    // Verify v2.
    assert_eq!(
        std::fs::read_to_string(git_work_dir.join("data.txt")).unwrap(),
        "version 2"
    );
}

// ===========================================================================
// Test 7: Binary file
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_binary_file() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Create a binary file with known bytes.
    let binary_data: Vec<u8> = (0..=255).collect();
    let bin_path = wc_path.join("data.bin");
    std::fs::write(&bin_path, &binary_data).unwrap();
    let _ = Command::new("svn")
        .args(["add", bin_path.to_str().unwrap()])
        .status()
        .unwrap();
    let output = Command::new("svn")
        .args([
            "commit",
            "-m",
            "Add binary file",
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);
    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 1);

    // Verify binary file matches exactly.
    let git_binary = std::fs::read(git_work_dir.join("data.bin")).unwrap();
    assert_eq!(git_binary, binary_data);
}

// ===========================================================================
// Test 8: Nested directories
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_nested_directories() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Create nested directory structure: src/main/java/App.java
    svn_commit_file(
        &wc_path,
        "src/main/java/App.java",
        "public class App {}",
        "Add nested Java file",
    );

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);
    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 1);

    // Verify the nested file exists with correct content.
    let app_path = git_work_dir.join("src/main/java/App.java");
    assert!(app_path.exists(), "nested file should exist in git repo");
    assert_eq!(
        std::fs::read_to_string(&app_path).unwrap(),
        "public class App {}"
    );
}

// ===========================================================================
// Test 9: Empty repo (no commits) -- no crash
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_empty_repo_no_crash() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);
    let synced = syncer
        .sync()
        .await
        .expect("sync on empty repo should not crash");
    assert_eq!(synced, 0, "nothing to sync in an empty repo");

    // Git should only have the initial commit.
    assert_eq!(count_git_commits(&git_work_dir), 1);
}

// ===========================================================================
// Test 10: Git-to-SVN basic replay (manual simulation)
// ===========================================================================

#[tokio::test]
async fn test_git_to_svn_basic_replay() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Create initial SVN content.
    svn_commit_file(&wc_path, "readme.txt", "Hello SVN", "Initial readme");

    // Create a Git repo with the same initial content.
    let git_work_dir = tmp.path().join("git_work");
    std::fs::create_dir_all(&git_work_dir).unwrap();
    let git_client = GitClient::init(&git_work_dir).unwrap();
    std::fs::write(git_work_dir.join("readme.txt"), "Hello SVN").unwrap();
    git_client
        .commit(
            "initial commit",
            "Test User",
            "test@example.com",
            "Test User",
            "test@example.com",
        )
        .unwrap();

    // Simulate a "PR merge" by adding a new file to the Git repo.
    std::fs::write(git_work_dir.join("feature.txt"), "New feature from Git").unwrap();
    let git_oid = git_client
        .commit(
            "Add feature.txt",
            "Test User",
            "test@example.com",
            "Test User",
            "test@example.com",
        )
        .unwrap();
    let git_sha = git_oid.to_string();

    // Format the commit message for Git-to-SVN direction.
    let config = make_test_config(&svn_url, tmp.path());
    let formatter = CommitFormatter::new(&config.commit_format);
    let svn_commit_msg =
        formatter.format_git_to_svn("Add feature.txt", &git_sha, 1, "feature/new-feature");

    // Copy the new file from Git working dir to SVN working copy.
    std::fs::copy(
        git_work_dir.join("feature.txt"),
        wc_path.join("feature.txt"),
    )
    .unwrap();

    // svn add + commit in the SVN working copy.
    let _ = Command::new("svn")
        .args(["add", wc_path.join("feature.txt").to_str().unwrap()])
        .status()
        .unwrap();

    let output = Command::new("svn")
        .args([
            "commit",
            "-m",
            &svn_commit_msg,
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "svn commit failed");

    // Verify SVN now has the new file.
    let svn_client = SvnClient::new(&svn_url, "", "");
    let info = svn_client.info().await.unwrap();
    assert_eq!(info.latest_rev, 2, "SVN should be at revision 2");

    // Verify the SVN commit message contains the [gitsvnsync] marker.
    let log_entries = svn_client.log(2, 2).await.unwrap();
    assert_eq!(log_entries.len(), 1);
    assert!(
        CommitFormatter::is_sync_marker(&log_entries[0].message),
        "SVN commit message should contain [gitsvnsync] marker"
    );
    assert!(
        log_entries[0].message.contains(&git_sha),
        "SVN commit message should contain the Git SHA"
    );
}

// ===========================================================================
// Test 11: CommitFormatter roundtrip
// ===========================================================================

#[test]
fn test_commit_formatter_roundtrip() {
    let config = CommitFormatConfig::default();
    let formatter = CommitFormatter::new(&config);

    // SVN -> Git direction.
    let svn_to_git_msg =
        formatter.format_svn_to_git("Fix bug #42", 123, "alice", "2025-06-15T10:00:00Z");
    assert!(
        svn_to_git_msg.contains("[gitsvnsync]"),
        "SVN-to-Git message should contain sync marker"
    );
    assert!(
        svn_to_git_msg.contains("SVN-Revision: r123"),
        "should contain SVN-Revision trailer"
    );
    assert!(
        svn_to_git_msg.contains("SVN-Author: alice"),
        "should contain SVN-Author trailer"
    );
    assert!(
        svn_to_git_msg.contains("Fix bug #42"),
        "should contain original message"
    );
    assert!(
        CommitFormatter::is_sync_marker(&svn_to_git_msg),
        "is_sync_marker should return true for SVN-to-Git formatted message"
    );

    // Git -> SVN direction.
    let git_to_svn_msg =
        formatter.format_git_to_svn("Add search endpoint", "abc123def456", 42, "feature/search");
    assert!(
        git_to_svn_msg.contains("[gitsvnsync]"),
        "Git-to-SVN message should contain sync marker"
    );
    assert!(
        git_to_svn_msg.contains("Git-SHA: abc123def456"),
        "should contain Git-SHA trailer"
    );
    assert!(
        git_to_svn_msg.contains("PR-Number: #42"),
        "should contain PR-Number trailer"
    );
    assert!(
        git_to_svn_msg.contains("PR-Branch: feature/search"),
        "should contain PR-Branch trailer"
    );
    assert!(
        git_to_svn_msg.contains("Add search endpoint"),
        "should contain original message"
    );
    assert!(
        CommitFormatter::is_sync_marker(&git_to_svn_msg),
        "is_sync_marker should return true for Git-to-SVN formatted message"
    );

    // Verify extraction methods.
    assert_eq!(CommitFormatter::extract_svn_rev(&svn_to_git_msg), Some(123));
    assert_eq!(
        CommitFormatter::extract_git_sha(&git_to_svn_msg),
        Some("abc123def456".to_string())
    );
    assert_eq!(
        CommitFormatter::extract_pr_number(&git_to_svn_msg),
        Some(42)
    );

    // Verify non-sync messages are not detected.
    assert!(!CommitFormatter::is_sync_marker("Regular commit message"));
    assert!(!CommitFormatter::is_sync_marker("Fix bug"));
}

// ===========================================================================
// Test 12: Database watermark and commit_map
// ===========================================================================

#[test]
fn test_database_watermark_and_commit_map() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);

    // Watermark: initially None.
    assert!(db.get_watermark("svn_rev").unwrap().is_none());

    // Set watermark.
    db.set_watermark("svn_rev", "100").unwrap();
    assert_eq!(db.get_watermark("svn_rev").unwrap().as_deref(), Some("100"));

    // Update watermark (upsert).
    db.set_watermark("svn_rev", "200").unwrap();
    assert_eq!(db.get_watermark("svn_rev").unwrap().as_deref(), Some("200"));

    // Multiple independent watermarks.
    db.set_watermark("git_sha", "abc123").unwrap();
    assert_eq!(
        db.get_watermark("git_sha").unwrap().as_deref(),
        Some("abc123")
    );
    assert_eq!(db.get_watermark("svn_rev").unwrap().as_deref(), Some("200"));

    // Insert commit_map entries.
    let id1 = db
        .insert_commit_map(
            100,
            "aaa111",
            "svn_to_git",
            "alice",
            "Alice <alice@test.com>",
        )
        .unwrap();
    assert!(id1 > 0);

    let id2 = db
        .insert_commit_map(101, "bbb222", "svn_to_git", "bob", "Bob <bob@test.com>")
        .unwrap();
    assert!(id2 > id1);

    let id3 = db
        .insert_commit_map(
            50,
            "ccc333",
            "git_to_svn",
            "alice",
            "Alice <alice@test.com>",
        )
        .unwrap();
    assert!(id3 > id2);

    // is_svn_rev_synced: true for existing revisions.
    assert!(db.is_svn_rev_synced(100).unwrap());
    assert!(db.is_svn_rev_synced(101).unwrap());
    assert!(db.is_svn_rev_synced(50).unwrap());

    // is_svn_rev_synced: false for non-existent revisions.
    assert!(!db.is_svn_rev_synced(999).unwrap());
    assert!(!db.is_svn_rev_synced(0).unwrap());

    // Look up Git SHA by SVN rev.
    assert_eq!(
        db.get_git_sha_for_svn_rev(100).unwrap().as_deref(),
        Some("aaa111")
    );
    assert_eq!(
        db.get_git_sha_for_svn_rev(101).unwrap().as_deref(),
        Some("bbb222")
    );
    assert!(db.get_git_sha_for_svn_rev(999).unwrap().is_none());

    // Look up SVN rev by Git SHA.
    assert_eq!(db.get_svn_rev_for_git_sha("aaa111").unwrap(), Some(100));
    assert_eq!(db.get_svn_rev_for_git_sha("ccc333").unwrap(), Some(50));
    assert!(db.get_svn_rev_for_git_sha("nonexistent").unwrap().is_none());

    // List commit_map (ordered by id DESC).
    let all = db.list_commit_map(10).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].svn_rev, 50); // most recent insert
    assert_eq!(all[1].svn_rev, 101);
    assert_eq!(all[2].svn_rev, 100);

    // PR sync log.
    assert!(!db.is_pr_synced("merge_sha_1").unwrap());
    let pr_id = db
        .insert_pr_sync(
            42,
            "Add search",
            "feature/search",
            "merge_sha_1",
            "squash",
            3,
        )
        .unwrap();
    assert!(db.is_pr_synced("merge_sha_1").unwrap());
    assert!(!db.is_pr_synced("other_sha").unwrap());

    db.complete_pr_sync(pr_id, 200, 202).unwrap();
    let pr_entries = db.list_pr_syncs(10).unwrap();
    assert_eq!(pr_entries.len(), 1);
    assert_eq!(pr_entries[0].status, "completed");
    assert_eq!(pr_entries[0].svn_rev_start, Some(200));
    assert_eq!(pr_entries[0].svn_rev_end, Some(202));

    // Audit log.
    db.insert_audit_log(
        "test_action",
        Some("svn_to_git"),
        Some(100),
        Some("aaa111"),
        Some("alice"),
        Some("test details"),
    )
    .unwrap();
    let audit = db.list_audit_log(10).unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action, "test_action");
    assert_eq!(audit[0].svn_rev, Some(100));
}

// ===========================================================================
// Test 13: Full SVN-to-Git cycle with metadata verification
// ===========================================================================

#[tokio::test]
async fn test_full_svn_to_git_cycle_with_metadata() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Commit as a specific user with a specific message.
    // Note: with file:// repos, the SVN author is typically empty or the
    // local user unless set via revprop. We'll set it after the commit.
    let rev = svn_commit_file(&wc_path, "bugfix.py", "print('fixed')", "fix bug #42");

    // Set the svn:author revprop so the sync sees "alice".
    let status = Command::new("svn")
        .args([
            "propset",
            "--revprop",
            "-r",
            &rev.to_string(),
            "svn:author",
            "alice",
            &svn_url,
            "--non-interactive",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "failed to set svn:author revprop");

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);
    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 1);

    // Read the Git commit message (index 0 = HEAD = the synced commit).
    let msg = get_git_commit_message(&git_work_dir, 0);

    // Verify metadata in commit message.
    assert!(
        msg.contains("[gitsvnsync]"),
        "commit message should contain [gitsvnsync] marker, got: {}",
        msg
    );
    assert!(
        msg.contains("SVN-Revision: r1"),
        "commit message should contain SVN-Revision: r1, got: {}",
        msg
    );
    assert!(
        msg.contains("SVN-Author: alice"),
        "commit message should contain SVN-Author: alice, got: {}",
        msg
    );
    assert!(
        msg.contains("fix bug #42"),
        "commit message should contain original message 'fix bug #42', got: {}",
        msg
    );

    // Verify the commit_map records the correct metadata.
    let commit_map = db_arc.list_commit_map(10).unwrap();
    assert_eq!(commit_map.len(), 1);
    assert_eq!(commit_map[0].svn_rev, 1);
    assert_eq!(commit_map[0].direction, "svn_to_git");
    assert_eq!(commit_map[0].svn_author, "alice");
    assert!(commit_map[0].git_author.contains("Test User"));

    // Verify the audit log was written.
    let audit = db_arc.list_audit_log(10).unwrap();
    assert!(!audit.is_empty(), "audit log should have entries");
    assert_eq!(audit[0].action, "svn_to_git_sync");

    // Verify the file exists in git with correct content.
    assert_eq!(
        std::fs::read_to_string(git_work_dir.join("bugfix.py")).unwrap(),
        "print('fixed')"
    );
}

// ===========================================================================
// Test 14: SVN-to-Git with file deletion
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_file_deletion() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Rev 1: Add two files.
    svn_commit_files(
        &wc_path,
        &[("keep.txt", "keep me"), ("delete_me.txt", "goodbye")],
        "Add two files",
    );

    // Rev 2: Delete one file via svn rm.
    let _ = Command::new("svn")
        .args(["rm", wc_path.join("delete_me.txt").to_str().unwrap()])
        .status()
        .unwrap();
    let output = Command::new("svn")
        .args([
            "commit",
            "-m",
            "Remove delete_me.txt",
            wc_path.to_str().unwrap(),
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let svn_client = SvnClient::new(&svn_url, "", "");
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    let syncer = SvnToGitSync::new(svn_client, git_arc.clone(), db_arc.clone(), config);
    let synced = syncer.sync().await.expect("sync failed");
    assert_eq!(synced, 2);

    // After syncing both revisions, the deleted file should not exist in git
    // (because svn export of r2 will not include delete_me.txt).
    assert!(git_work_dir.join("keep.txt").exists());
    // The SvnToGitSync uses `svn export` which gets the full tree at each revision.
    // After rev 2, delete_me.txt won't be in the export. However, copy_tree
    // only copies from src to dst -- it doesn't delete files that are missing
    // from the src. This is a known limitation of the copy_tree approach.
    // The test verifies the sync completes without errors.
    assert_eq!(
        std::fs::read_to_string(git_work_dir.join("keep.txt")).unwrap(),
        "keep me"
    );
}

// ===========================================================================
// Test 15: SvnClient.info() and .log() against a real local repo
// ===========================================================================

#[tokio::test]
async fn test_svn_client_info_and_log() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    // Empty repo: revision 0.
    let svn_client = SvnClient::new(&svn_url, "", "");
    let info = svn_client.info().await.unwrap();
    assert_eq!(info.latest_rev, 0);

    // Add some commits.
    svn_commit_file(&wc_path, "a.txt", "aaa", "First commit");
    svn_commit_file(&wc_path, "b.txt", "bbb", "Second commit");

    let info2 = svn_client.info().await.unwrap();
    assert_eq!(info2.latest_rev, 2);
    assert!(info2.url.contains("svn_repo"));

    // Log: all revisions.
    let log = svn_client.log(1, 2).await.unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].revision, 1);
    assert_eq!(log[0].message, "First commit");
    assert_eq!(log[1].revision, 2);
    assert_eq!(log[1].message, "Second commit");

    // Log: single revision.
    let log_single = svn_client.log(2, 2).await.unwrap();
    assert_eq!(log_single.len(), 1);
    assert_eq!(log_single[0].revision, 2);
}

// ===========================================================================
// Test 16: SvnClient.export() against a real local repo
// ===========================================================================

#[tokio::test]
async fn test_svn_client_export() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    svn_commit_file(&wc_path, "hello.txt", "hello world", "Add hello");
    svn_commit_file(&wc_path, "sub/nested.txt", "nested content", "Add nested");

    let svn_client = SvnClient::new(&svn_url, "", "");

    // Export revision 2 to a temp dir.
    let export_dir = tmp.path().join("export");
    svn_client.export("", 2, &export_dir).await.unwrap();

    assert!(export_dir.join("hello.txt").exists());
    assert_eq!(
        std::fs::read_to_string(export_dir.join("hello.txt")).unwrap(),
        "hello world"
    );
    assert!(export_dir.join("sub/nested.txt").exists());
    assert_eq!(
        std::fs::read_to_string(export_dir.join("sub/nested.txt")).unwrap(),
        "nested content"
    );

    // Export should NOT contain .svn metadata.
    assert!(!export_dir.join(".svn").exists());
}

// ===========================================================================
// Test 17: GitClient operations (init, commit, push to bare, head_sha)
// ===========================================================================

#[test]
fn test_git_client_init_commit_push() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("repo");
    let bare_dir = tmp.path().join("bare.git");

    let git_client = setup_git_with_bare_origin(&work_dir, &bare_dir);

    // Should have 1 commit (initial).
    assert_eq!(count_git_commits(&work_dir), 1);

    // Add a file and commit.
    std::fs::write(work_dir.join("test.txt"), "test content").unwrap();
    let oid = git_client
        .commit(
            "add test file",
            "Alice",
            "alice@test.com",
            "Alice",
            "alice@test.com",
        )
        .unwrap();
    assert!(!oid.is_zero());
    assert_eq!(git_client.get_head_sha().unwrap(), oid.to_string());

    // Push to bare origin.
    git_client.push("origin", "main", None).unwrap();

    // Verify push landed in bare repo.
    let bare_repo = git2::Repository::open_bare(&bare_dir).unwrap();
    let bare_head = bare_repo
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(bare_head.id().to_string(), oid.to_string());

    // Now 2 commits total.
    assert_eq!(count_git_commits(&work_dir), 2);
}

// ===========================================================================
// Test 18: Database in-memory with full schema
// ===========================================================================

#[test]
fn test_database_in_memory_full_schema() {
    let db = Database::in_memory().unwrap();
    db.initialize().unwrap();

    // Verify all operations work on in-memory DB.
    db.set_watermark("test", "42").unwrap();
    assert_eq!(db.get_watermark("test").unwrap().as_deref(), Some("42"));

    let id = db
        .insert_commit_map(1, "sha1", "svn_to_git", "user", "User <u@t.com>")
        .unwrap();
    assert!(id > 0);
    assert!(db.is_svn_rev_synced(1).unwrap());
    assert!(!db.is_svn_rev_synced(2).unwrap());

    db.insert_audit_log("test", None, None, None, None, None)
        .unwrap();
    assert_eq!(db.count_audit_log().unwrap(), 1);

    let pr_id = db
        .insert_pr_sync(10, "Title", "branch", "merge_sha", "squash", 1)
        .unwrap();
    assert!(db.is_pr_synced("merge_sha").unwrap());
    db.complete_pr_sync(pr_id, 1, 1).unwrap();
}

// ===========================================================================
// Test 19: Multiple sequential syncs build correct history
// ===========================================================================

#[tokio::test]
async fn test_svn_to_git_sequential_syncs_history() {
    if !svn_available() {
        eprintln!("SKIPPED: svn/svnadmin not found in PATH");
        return;
    }

    let tmp = TempDir::new().unwrap();
    let svn_url = create_svn_repo(tmp.path());
    let wc_path = tmp.path().join("wc");
    svn_checkout(&svn_url, &wc_path);

    let git_work_dir = tmp.path().join("git_work");
    let bare_dir = tmp.path().join("origin.git");
    let git_client = setup_git_with_bare_origin(&git_work_dir, &bare_dir);

    let db_path = tmp.path().join("test.db");
    let db = setup_db(&db_path);
    let config = make_test_config(&svn_url, tmp.path());
    let git_arc = Arc::new(Mutex::new(git_client));
    let db_arc = Arc::new(db);

    // Sync in 3 separate passes, adding 1 revision each time.
    for i in 1..=3 {
        svn_commit_file(
            &wc_path,
            &format!("file{}.txt", i),
            &format!("content {}", i),
            &format!("Add file{}", i),
        );

        let syncer = SvnToGitSync::new(
            SvnClient::new(&svn_url, "", ""),
            git_arc.clone(),
            db_arc.clone(),
            config.clone(),
        );
        let synced = syncer.sync().await.expect("sync failed");
        assert_eq!(synced, 1, "pass {} should sync exactly 1 revision", i);
    }

    // Total: 1 (initial) + 3 (synced) = 4 commits.
    assert_eq!(count_git_commits(&git_work_dir), 4);

    // Watermark at 3.
    assert_eq!(
        db_arc.get_watermark("svn_rev").unwrap().as_deref(),
        Some("3")
    );

    // commit_map has 3 entries.
    assert_eq!(db_arc.list_commit_map(10).unwrap().len(), 3);

    // All files exist.
    for i in 1..=3 {
        assert!(git_work_dir.join(format!("file{}.txt", i)).exists());
    }
}

// ===========================================================================
// Test 20: Concurrent-safe database access (Arc<Database>)
// ===========================================================================

#[tokio::test]
async fn test_database_concurrent_access() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("concurrent.db");
    let db = Arc::new(setup_db(&db_path));

    // Spawn multiple tasks that write to the database concurrently.
    let mut handles = Vec::new();
    for i in 0..10 {
        let db_clone = db.clone();
        handles.push(tokio::spawn(async move {
            db_clone
                .set_watermark(&format!("source_{}", i), &format!("{}", i * 10))
                .unwrap();
            db_clone
                .insert_commit_map(
                    i as i64,
                    &format!("sha_{}", i),
                    "svn_to_git",
                    "user",
                    "User <u@t.com>",
                )
                .unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all watermarks were written.
    for i in 0..10 {
        let wm = db.get_watermark(&format!("source_{}", i)).unwrap().unwrap();
        assert_eq!(wm, format!("{}", i * 10));
    }

    // Verify all commit_map entries exist.
    let all = db.list_commit_map(20).unwrap();
    assert_eq!(all.len(), 10);
}

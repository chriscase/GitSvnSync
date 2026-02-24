//! File-policy enforcement for sync operations.
//!
//! Provides [`FilePolicy`] which encapsulates `max_file_size` and
//! `ignore_patterns` from [`PersonalOptionsConfig`] and evaluates candidate
//! files before they are copied, committed, or replayed.
//!
//! # Decision model
//!
//! For each candidate file the policy returns a [`FilePolicyDecision`]:
//!
//! | Condition | Decision |
//! |-----------|----------|
//! | Path matches an ignore pattern | `Ignored` |
//! | Size exceeds `max_file_size` (when > 0) | `Oversize` |
//! | Size exceeds `lfs_threshold` and LFS enabled | `LfsTrack` |
//! | None of the above | `Allow` |
//!
//! All decisions carry the relative path and size for auditing.

use std::path::Path;

use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Decision enum
// ---------------------------------------------------------------------------

/// The outcome of evaluating a file against the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePolicyDecision {
    /// File passes all checks — sync it normally.
    Allow,
    /// File matches an ignore pattern — skip it.
    Ignored { pattern: String },
    /// File exceeds the configured `max_file_size` — skip it.
    Oversize { size: u64, limit: u64 },
    /// File qualifies for LFS tracking (above lfs_threshold, LFS enabled).
    LfsTrack { size: u64, threshold: u64 },
}

impl FilePolicyDecision {
    /// `true` if the file should be synced (either `Allow` or `LfsTrack`).
    pub fn should_sync(&self) -> bool {
        matches!(self, Self::Allow | Self::LfsTrack { .. })
    }

    /// `true` if the file is blocked (ignored or oversize).
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Ignored { .. } | Self::Oversize { .. })
    }

    /// Short human-readable label for audit/logging.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ignored { .. } => "ignored",
            Self::Oversize { .. } => "oversize",
            Self::LfsTrack { .. } => "lfs-track",
        }
    }
}

// ---------------------------------------------------------------------------
// FilePolicy
// ---------------------------------------------------------------------------

/// Evaluates candidate files against size limits and ignore patterns.
///
/// Constructed from the personal-mode `[options]` config section. Thread-safe
/// and cheap to clone (all data is owned strings/u64).
#[derive(Debug, Clone)]
pub struct FilePolicy {
    /// Maximum allowed file size in bytes. 0 = no limit.
    max_file_size: u64,
    /// Glob patterns to exclude. Matched against the *relative* path.
    ignore_patterns: Vec<String>,
    /// LFS size threshold in bytes. 0 = LFS disabled.
    lfs_threshold: u64,
    /// Whether LFS is enabled.
    lfs_enabled: bool,
}

impl FilePolicy {
    /// Create a new `FilePolicy` from config values.
    pub fn new(max_file_size: u64, ignore_patterns: Vec<String>) -> Self {
        Self {
            max_file_size,
            ignore_patterns,
            lfs_threshold: 0,
            lfs_enabled: false,
        }
    }

    /// Create a `FilePolicy` with LFS support.
    pub fn with_lfs(
        max_file_size: u64,
        ignore_patterns: Vec<String>,
        lfs_threshold: u64,
        lfs_patterns: &[String],
    ) -> Self {
        // Merge LFS patterns into ignore patterns? No — LFS patterns are
        // for *tracking*, not ignoring.  We keep them separate conceptually.
        let _ = lfs_patterns; // Reserved for future pattern-based LFS matching.
        Self {
            max_file_size,
            ignore_patterns,
            lfs_threshold,
            lfs_enabled: lfs_threshold > 0,
        }
    }

    /// Evaluate a file.
    ///
    /// `rel_path` is the file's path relative to the repo root (forward-slash
    /// separated). `size` is the file size in bytes.
    pub fn evaluate(&self, rel_path: &str, size: u64) -> FilePolicyDecision {
        // 1. Check ignore patterns first.
        for pattern in &self.ignore_patterns {
            if self.matches_pattern(rel_path, pattern) {
                debug!(
                    path = rel_path,
                    pattern = pattern.as_str(),
                    "file matches ignore pattern"
                );
                return FilePolicyDecision::Ignored {
                    pattern: pattern.clone(),
                };
            }
        }

        // 2. Check max_file_size (0 = unlimited).
        if self.max_file_size > 0 && size > self.max_file_size {
            warn!(
                path = rel_path,
                size,
                limit = self.max_file_size,
                "file exceeds max_file_size — skipping"
            );
            return FilePolicyDecision::Oversize {
                size,
                limit: self.max_file_size,
            };
        }

        // 3. Check LFS threshold.
        if self.lfs_enabled && size > self.lfs_threshold {
            info!(
                path = rel_path,
                size,
                threshold = self.lfs_threshold,
                "file qualifies for LFS tracking"
            );
            return FilePolicyDecision::LfsTrack {
                size,
                threshold: self.lfs_threshold,
            };
        }

        FilePolicyDecision::Allow
    }

    /// Evaluate a file on disk (reads metadata for size).
    ///
    /// `base_dir` is the repo/export root, `rel_path` is relative to it.
    /// If the file cannot be stat'd, returns `Allow` (let the copy/commit
    /// operation itself report the I/O error).
    pub fn evaluate_path(&self, base_dir: &Path, rel_path: &str) -> FilePolicyDecision {
        let full = base_dir.join(rel_path);
        let size = match std::fs::metadata(&full) {
            Ok(m) => m.len(),
            Err(_) => return FilePolicyDecision::Allow,
        };
        self.evaluate(rel_path, size)
    }

    /// Whether the policy has any constraints at all.
    pub fn has_constraints(&self) -> bool {
        self.max_file_size > 0 || !self.ignore_patterns.is_empty() || self.lfs_enabled
    }

    /// Max file size (for display/logging).
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    /// Whether LFS is enabled.
    pub fn lfs_enabled(&self) -> bool {
        self.lfs_enabled
    }

    /// LFS threshold (for display/logging).
    pub fn lfs_threshold(&self) -> u64 {
        self.lfs_threshold
    }

    // -----------------------------------------------------------------------
    // Pattern matching
    // -----------------------------------------------------------------------

    /// Test whether `rel_path` matches a glob `pattern`.
    ///
    /// Supports:
    /// - `*` — match any single path component segment
    /// - `**` — match zero or more path segments
    /// - `*.ext` — match by extension
    /// - `dir/**` — match everything under a directory
    fn matches_pattern(&self, rel_path: &str, pattern: &str) -> bool {
        // Normalize to forward slashes for consistent matching.
        let path = rel_path.replace('\\', "/");
        let pat = pattern.replace('\\', "/");

        // Use glob-match crate for proper glob semantics.
        glob_match::glob_match(&pat, &path)
    }
}

// ---------------------------------------------------------------------------
// Construct from PersonalOptionsConfig
// ---------------------------------------------------------------------------

impl From<&crate::personal_config::PersonalOptionsConfig> for FilePolicy {
    fn from(opts: &crate::personal_config::PersonalOptionsConfig) -> Self {
        if opts.lfs_threshold > 0 {
            Self::with_lfs(
                opts.max_file_size,
                opts.ignore_patterns.clone(),
                opts.lfs_threshold,
                &opts.lfs_patterns,
            )
        } else {
            Self::new(opts.max_file_size, opts.ignore_patterns.clone())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_when_no_constraints() {
        let policy = FilePolicy::new(0, vec![]);
        let decision = policy.evaluate("src/main.rs", 1024);
        assert_eq!(decision, FilePolicyDecision::Allow);
        assert!(decision.should_sync());
        assert!(!decision.is_blocked());
        assert_eq!(decision.label(), "allow");
    }

    #[test]
    fn test_oversize_blocks_file() {
        let policy = FilePolicy::new(1000, vec![]);
        let decision = policy.evaluate("bigfile.bin", 2000);
        assert!(matches!(
            decision,
            FilePolicyDecision::Oversize {
                size: 2000,
                limit: 1000
            }
        ));
        assert!(!decision.should_sync());
        assert!(decision.is_blocked());
        assert_eq!(decision.label(), "oversize");
    }

    #[test]
    fn test_under_limit_allowed() {
        let policy = FilePolicy::new(5000, vec![]);
        let decision = policy.evaluate("small.txt", 500);
        assert_eq!(decision, FilePolicyDecision::Allow);
    }

    #[test]
    fn test_exact_limit_allowed() {
        let policy = FilePolicy::new(1000, vec![]);
        // Exactly at the limit should pass (only > limit is blocked).
        let decision = policy.evaluate("exact.txt", 1000);
        assert_eq!(decision, FilePolicyDecision::Allow);
    }

    #[test]
    fn test_ignore_pattern_star_ext() {
        let policy = FilePolicy::new(0, vec!["*.log".into()]);
        let decision = policy.evaluate("app.log", 100);
        assert!(matches!(decision, FilePolicyDecision::Ignored { .. }));
        assert!(decision.is_blocked());
        assert_eq!(decision.label(), "ignored");
    }

    #[test]
    fn test_ignore_pattern_no_match() {
        let policy = FilePolicy::new(0, vec!["*.log".into()]);
        let decision = policy.evaluate("app.txt", 100);
        assert_eq!(decision, FilePolicyDecision::Allow);
    }

    #[test]
    fn test_ignore_pattern_double_star() {
        let policy = FilePolicy::new(0, vec!["build/**".into()]);
        let decision = policy.evaluate("build/out/main.o", 100);
        assert!(matches!(decision, FilePolicyDecision::Ignored { .. }));
    }

    #[test]
    fn test_ignore_pattern_nested_ext() {
        let policy = FilePolicy::new(0, vec!["**/*.class".into()]);
        let d1 = policy.evaluate("com/example/Main.class", 100);
        assert!(matches!(d1, FilePolicyDecision::Ignored { .. }));
        let d2 = policy.evaluate("com/example/Main.java", 100);
        assert_eq!(d2, FilePolicyDecision::Allow);
    }

    #[test]
    fn test_ignore_checked_before_size() {
        // If a file matches ignore AND is oversize, the decision should be Ignored
        // (ignore patterns take priority).
        let policy = FilePolicy::new(100, vec!["*.tmp".into()]);
        let decision = policy.evaluate("data.tmp", 5000);
        assert!(matches!(decision, FilePolicyDecision::Ignored { .. }));
    }

    #[test]
    fn test_multiple_ignore_patterns() {
        let policy = FilePolicy::new(
            0,
            vec!["*.log".into(), "build/**".into(), ".idea/**".into()],
        );
        assert!(policy.evaluate("server.log", 10).is_blocked());
        assert!(policy.evaluate("build/output.o", 10).is_blocked());
        assert!(policy.evaluate(".idea/workspace.xml", 10).is_blocked());
        assert!(!policy.evaluate("src/main.rs", 10).is_blocked());
    }

    #[test]
    fn test_has_constraints() {
        assert!(!FilePolicy::new(0, vec![]).has_constraints());
        assert!(FilePolicy::new(1000, vec![]).has_constraints());
        assert!(FilePolicy::new(0, vec!["*.log".into()]).has_constraints());
    }

    #[test]
    fn test_from_options_config() {
        let opts = crate::personal_config::PersonalOptionsConfig {
            max_file_size: 5000,
            ignore_patterns: vec!["*.tmp".into()],
            ..Default::default()
        };
        let policy = FilePolicy::from(&opts);
        assert_eq!(policy.max_file_size(), 5000);
        assert!(policy.evaluate("data.tmp", 100).is_blocked());
    }

    #[test]
    fn test_evaluate_path_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("small.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("big.bin"), vec![0u8; 2000]).unwrap();

        let policy = FilePolicy::new(1000, vec![]);
        assert_eq!(
            policy.evaluate_path(dir.path(), "small.txt"),
            FilePolicyDecision::Allow
        );
        assert!(matches!(
            policy.evaluate_path(dir.path(), "big.bin"),
            FilePolicyDecision::Oversize { .. }
        ));
        // Non-existent file → Allow (let I/O error surface elsewhere).
        assert_eq!(
            policy.evaluate_path(dir.path(), "missing.txt"),
            FilePolicyDecision::Allow
        );
    }

    #[test]
    fn test_lfs_threshold() {
        let policy = FilePolicy::with_lfs(0, vec![], 1000, &[]);
        // Under threshold → Allow.
        assert_eq!(policy.evaluate("small.txt", 500), FilePolicyDecision::Allow);
        // Over threshold → LfsTrack.
        let decision = policy.evaluate("model.bin", 2000);
        assert!(matches!(
            decision,
            FilePolicyDecision::LfsTrack {
                size: 2000,
                threshold: 1000
            }
        ));
        assert!(decision.should_sync());
        assert!(!decision.is_blocked());
        assert_eq!(decision.label(), "lfs-track");
    }

    #[test]
    fn test_oversize_takes_precedence_over_lfs() {
        // If max_file_size is set AND lfs_threshold, oversize wins.
        let policy = FilePolicy::with_lfs(5000, vec![], 1000, &[]);
        // 3000 is above LFS threshold but under max_file_size.
        let d1 = policy.evaluate("med.bin", 3000);
        assert!(matches!(d1, FilePolicyDecision::LfsTrack { .. }));
        // 6000 is above both. max_file_size blocks it.
        let d2 = policy.evaluate("huge.bin", 6000);
        assert!(matches!(d2, FilePolicyDecision::Oversize { .. }));
    }
}

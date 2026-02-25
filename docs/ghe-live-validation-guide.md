# GHE Live Validation Guide

Real end-to-end bidirectional validation of GitSvnSync against a live GitHub Enterprise instance and a live SVN repository. Unlike `controlled-validation.sh` (which uses local `file://` SVN repos), this script exercises the **actual network path** your daemon will use in production.

## Prerequisites

| Tool | Minimum | Check |
|------|---------|-------|
| Rust | 1.70+ | `rustc --version` |
| Cargo | 1.70+ | `cargo --version` |
| SVN | 1.14+ | `svn --version` |
| Git | 2.30+ | `git --version` |
| curl | 7.x+ | `curl --version` |
| jq | 1.6+ | `jq --version` |

All tools must be on `$PATH`.

## Environment Variables

### Required (for live run)

| Variable | Description | Example |
|----------|-------------|---------|
| `GHE_API_URL` | GitHub Enterprise API base URL | `https://github.example.com/api/v3` |
| `GHE_TOKEN` | GitHub PAT with `repo` scope | `ghp_abc123...` |
| `GHE_OWNER` | Repository owner or organization | `myorg` |
| `GHE_REPO` | Repository name (created if missing) | `gitsvnsync-canary` |
| `SVN_URL` | SVN repository URL (must be writable) | `https://svn.example.com/repos/trunk` |
| `SVN_USERNAME` | SVN username | `svc-gitsvnsync` |
| `SVN_PASSWORD` | SVN password | (secret) |

### Optional

| Variable | Description | Default |
|----------|-------------|---------|
| `GHE_WEB_URL` | GitHub Enterprise web base URL | Derived from `GHE_API_URL` |
| `GITSVNSYNC_CONFIG` | Path to gitsvnsync personal config | Auto-generated |

## Quick Start

```bash
# 1. Preflight only (no live API calls, no env vars needed)
scripts/ghe-live-validation.sh --dry-run

# 2. Export your credentials
export GHE_API_URL="https://github.example.com/api/v3"
export GHE_TOKEN="ghp_your_token_here"
export GHE_OWNER="myorg"
export GHE_REPO="gitsvnsync-canary"
export SVN_URL="https://svn.example.com/repos/trunk"
export SVN_USERNAME="svc-gitsvnsync"
export SVN_PASSWORD="your_svn_password"

# 3. Single-cycle live run (recommended first time)
scripts/ghe-live-validation.sh --cycles 1

# 4. Multi-cycle with interval
scripts/ghe-live-validation.sh --cycles 5 --interval 10

# 5. Strict mode (abort on first failure)
scripts/ghe-live-validation.sh --strict --cycles 3
```

Or via Makefile:

```bash
make validate-ghe-live-dry-run          # preflight
make validate-ghe-live                  # 1-cycle live run
```

## Scenario Matrix (12 Scenarios per Cycle)

Each cycle executes these scenarios in order:

| # | Scenario | Direction | What It Tests |
|---|----------|-----------|---------------|
| S1 | SVN add file | SVN→ | Create a new file via `svn commit`, verify content with `svn cat` |
| S2 | SVN modify file | SVN→ | Modify an existing file, verify updated content |
| S3 | SVN delete file | SVN→ | Delete a file via `svn rm`, verify it's gone |
| S4 | SVN nested dirs | SVN→ | Create deeply nested directory structure, verify leaf file |
| S4b | **SVN→Git sync** | SVN→Git | **Invoke `gitsvnsync-personal sync`, pull Git repo, verify SVN files appear in Git** |
| S5 | Git add file | →Git | Create a file via GHE Contents API, verify via GET |
| S6 | Git modify file | →Git | Update a file via GHE Contents API (with SHA), verify |
| S7 | Git delete file | →Git | Delete a file via GHE Contents API, verify 404 |
| S7b | **Git→SVN sync** | Git→SVN | **Invoke `gitsvnsync-personal sync`, update SVN WC, verify sync completed** |
| S8 | Echo marker | SVN→ | Commit with `[gitsvnsync]` marker, verify in `svn log --xml` |
| S9 | API rate limit | →Git | Check `/rate_limit` endpoint, verify >100 requests remaining |
| S10 | Log-probe | Local | Spawn `gitsvnsync-personal log-probe`, verify `personal.log` written |

> **S4b and S7b are the critical cross-system sync scenarios.** They invoke the actual GitSvnSync
> sync engine (`gitsvnsync-personal sync`) and verify that changes made on one side arrive on the
> other side. Without these, the script would only be validating raw SVN/Git CLI operations, not
> the sync logic itself. Sync engine logs are captured in `cycle-NNN/sync-engine-data/`.

## CLI Options

```
Usage: scripts/ghe-live-validation.sh [OPTIONS]

Options:
  --dry-run          Preflight checks only (no live API calls)
  --cycles N         Number of validation cycles (default: 1)
  --interval N       Seconds between cycles (default: 5)
  --strict           Fail immediately on any scenario failure
  --config PATH      Path to gitsvnsync personal config
  --artifacts-dir D  Override artifact output directory
  --help             Show this help
```

## Output Artifacts

Each run produces a timestamped artifact bundle:

```
artifacts/ghe-live-validation/<UTC_TIMESTAMP>/
├── timeline.log            # Human-readable real-time progress
├── events.ndjson           # Machine-readable event stream
├── summary.md              # Go/No-Go report with scenario table
├── manifest.json           # Full artifact listing with sizes
├── env-snapshot.txt        # Sanitized environment (no secrets)
├── tool-versions.txt       # Tool versions
├── verification/
│   ├── svn-info.txt        # SVN connection info (dry-run)
│   ├── svn-checkout.log    # SVN checkout output
│   ├── git-clone.log       # Git clone output
│   └── leak-scan.log       # Secret scan results
└── cycle-001/              # Per-cycle artifacts
    ├── s1-commit.log       # SVN commit output per scenario
    ├── s4b-sync-stdout.log # SVN→Git sync engine stdout
    ├── s4b-sync-stderr.log # SVN→Git sync engine stderr
    ├── s4b-git-pull.log    # Git pull after sync
    ├── s5-git-sha.txt      # Git commit SHA
    ├── s7b-sync-stdout.log # Git→SVN sync engine stdout
    ├── s7b-sync-stderr.log # Git→SVN sync engine stderr
    ├── s7b-svn-update.log  # SVN update after sync
    ├── s9-rate-limit.json  # GHE rate limit response
    ├── s10-probe-stdout.log
    ├── s10-probe-stderr.log
    ├── daemon.log          # Captured personal.log
    └── sync-engine-data/   # GitSvnSync data dir (DB, logs)
```

### events.ndjson Format

```json
{"timestamp":"2026-02-24T12:00:00Z","phase":"scenario","action":"s1-svn-add","status":"pass","duration_ms":0}
{"timestamp":"2026-02-24T12:00:01Z","phase":"cycle-1","action":"complete","status":"pass","duration_ms":15000}
```

## Go/No-Go Criteria

### Automatic (script enforced)

| Criterion | Threshold | Behavior |
|-----------|-----------|----------|
| Any scenario failure | > 0 failures | Exit code 1 (NO-GO) |
| Strict mode failure | First failure | Immediate abort |
| Secret leakage | Any match | Flagged in summary |

### Manual Review

Before declaring production readiness, verify:

- [ ] All 10 scenarios PASS for every cycle
- [ ] No secret patterns in artifact files
- [ ] Rate limit headroom is sufficient (>100 remaining)
- [ ] SVN commit latency is acceptable
- [ ] GHE API response times are acceptable
- [ ] `personal.log` output is well-formed
- [ ] No unexpected error patterns in `events.ndjson`

## Failure Triage

| Scenario | Common Causes | What to Check |
|----------|---------------|---------------|
| S1-S4 (SVN) | Auth failure, read-only repo, network | `cycle-NNN/sN-commit.log`, SVN access |
| S5-S7 (Git) | Token scope, repo permissions, 404 | `cycle-NNN/sN-error.json`, token scopes |
| S8 (echo) | SVN log format differs | `svn log --xml` output manually |
| S9 (rate limit) | Token exhausted | `cycle-NNN/s9-rate-limit.json` |
| S10 (log-probe) | Binary not built, config error | `cycle-NNN/s10-probe-stderr.log` |

## Rollback Procedure

If issues are found post-enablement:

1. **Stop the daemon immediately:**
   ```bash
   gitsvnsync-personal --config <path> stop
   ```

2. **Verify it stopped:**
   ```bash
   gitsvnsync-personal --config <path> status
   # Should show "○ Not running"
   ```

3. **Capture incident artifacts:**
   ```bash
   cp personal.db personal.db.incident-$(date +%Y%m%d)
   cp personal.log personal.log.incident-$(date +%Y%m%d)
   ```

4. **Review audit log for last known-good state:**
   ```bash
   sqlite3 personal.db "SELECT * FROM audit_log ORDER BY id DESC LIMIT 10;"
   sqlite3 personal.db "SELECT * FROM watermarks;"
   ```

5. **Reset watermark if needed:**
   ```bash
   sqlite3 personal.db "UPDATE watermarks SET value='<last_good_rev>' WHERE key='svn_rev';"
   ```

6. **Restart with corrected config:**
   ```bash
   gitsvnsync-personal --config <path> start --foreground
   ```

## Relationship to Other Validation Scripts

| Script | Scope | Network Required | Use Case |
|--------|-------|-----------------|----------|
| `controlled-validation.sh` | Local only | No | CI gating, pre-merge checks |
| `enterprise-soak.sh` | Local only | No | Repeated-cycle stability testing |
| `ghe-live-validation.sh` | Live GHE+SVN | **Yes** | Pre-production readiness gate |

Run them in order: controlled → soak → GHE live.

# Enterprise Soak/Canary Validation Runbook

Staged soak protocol for validating GitSvnSync before production enablement on GitHub Enterprise (Cloud/Server) with legacy SVN.

## Environment Topology

```
┌─────────────┐     ┌────────────────┐     ┌──────────────┐
│ SVN Server   │◄───►│ GitSvnSync     │◄───►│ GitHub       │
│ (on-prem/    │     │ daemon         │     │ Enterprise   │
│  hosted)     │     │                │     │ (Cloud/      │
│              │     │ personal.db    │     │  Server)     │
│              │     │ personal.log   │     │              │
└─────────────┘     └────────────────┘     └──────────────┘
```

## Prerequisites

### Required Tokens/Scopes

| Token | Required Scopes | Purpose |
|-------|----------------|---------|
| GitHub PAT | `repo`, `read:org` | API access for sync |
| SVN credentials | Read/write to target path | SVN checkout, commit, log |

### Required Permissions

- **SVN**: Read/write access to the synchronization path
- **GitHub**: Push to target repository, create/merge PRs
- **Network**: Connectivity between daemon host, SVN server, and GitHub API

### Pre-soak Checklist

- [ ] SVN server accessible from daemon host (`svn info <url>`)
- [ ] GitHub API accessible (`curl -H "Authorization: token <pat>" https://<api>/user`)
- [ ] Target GitHub repo exists and is writable
- [ ] Personal config file validated (`gitsvnsync-personal --config <path> status`)
- [ ] Data directory writable with sufficient disk space (100MB minimum for soak)

## Running the Soak

### Local/Staging Soak (no external services needed)

```bash
# Quick dry-run — preflight only, verify tools and build
scripts/enterprise-soak.sh --dry-run

# Short soak (5 cycles, 2s interval) — default
scripts/enterprise-soak.sh

# Extended soak (50 cycles, 10s interval)
scripts/enterprise-soak.sh --cycles 50 --interval 10

# Strict threshold (0% failure tolerance)
scripts/enterprise-soak.sh --cycles 20 --max-error-rate 0
```

### What Each Cycle Does

1. **Synthetic canary commit**: Injects a unique file into SVN with timestamped content
2. **Content verification**: Reads back the committed file and verifies byte-exact match
3. **Log probe**: Runs `gitsvnsync-personal log-probe` to verify logging subsystem health
4. **Health snapshot**: Records SVN head revision, disk usage, and timing

### Enterprise-Specific Validation

For GitHub Enterprise environments, additionally verify:

1. **Webhook delivery**: If using webhooks, verify webhook payloads arrive and are verified
2. **Branch protection**: Confirm sync doesn't violate branch protection rules
3. **Rate limiting**: GHE Server has different rate limits than GitHub.com — watch for 429s
4. **Audit log**: Enterprise audit logs should show API calls from the sync PAT

## Output Artifacts

```
artifacts/enterprise-soak/<UTC_TIMESTAMP>/
├── timeline.log              # Real-time progress log
├── events.ndjson             # Machine-readable event stream
├── summary.md                # Go/No-Go report
├── manifest.json             # Full artifact listing
├── env-snapshot.txt          # Sanitized environment
├── tool-versions.txt         # Tool versions
├── health-snapshots/
│   ├── snapshot-001.txt      # Per-cycle health data
│   ├── snapshot-002.txt
│   └── ...
└── cycle-001/                # Per-cycle logs
    ├── probe-stdout.log
    ├── probe-stderr.log
    └── svn-commit-stderr.log
```

### events.ndjson Fields

```json
{"timestamp":"2026-02-24T12:00:00Z","phase":"cycle-1","action":"svn-commit","status":"pass","duration_ms":150,"svn_rev":"42"}
```

## Go/No-Go Criteria

### Automatic Gating (script enforced)

| Criterion | Threshold | Action if exceeded |
|-----------|-----------|-------------------|
| Cycle error rate | ≤ 20% (configurable) | Script exits non-zero |
| SVN commit failures | 0 in preflight | Abort soak |
| Build failure | 0 | Abort soak |
| Secret leakage | 0 instances | Flagged in summary |

### Manual Review Criteria

Before production enablement, a human reviewer should verify:

- [ ] All soak cycles show consistent timing (no degradation trend)
- [ ] Health snapshots show stable SVN revision progression
- [ ] No unexpected error patterns in `events.ndjson`
- [ ] `personal.log` output is well-formed (no garbled output)
- [ ] Disk usage is stable (no unbounded growth during soak)
- [ ] SVN working copy stays clean between cycles
- [ ] No secret patterns in any artifact file

## Acceptance Thresholds

| Metric | Acceptable | Needs investigation |
|--------|-----------|-------------------|
| Cycle pass rate | ≥ 95% | < 95% |
| SVN commit latency | < 5s per cycle | > 5s |
| Log probe success | 100% | < 100% |
| Disk growth per cycle | < 1MB | > 5MB |

## Rollback Procedure

If issues are discovered after production enablement:

### Immediate (within minutes)

1. Stop the daemon:
   ```bash
   gitsvnsync-personal --config <path> stop
   ```

2. Verify daemon stopped:
   ```bash
   gitsvnsync-personal --config <path> status
   # Should show "○ Not running"
   ```

### Recovery

3. Review the last known-good state:
   ```bash
   # Check audit log for last successful sync
   sqlite3 personal.db "SELECT * FROM audit_log ORDER BY id DESC LIMIT 5;"

   # Check current watermarks
   sqlite3 personal.db "SELECT * FROM watermarks;"
   ```

4. If needed, reset watermark to a known-good revision:
   ```bash
   sqlite3 personal.db "UPDATE watermarks SET value='<last_good_rev>' WHERE key='svn_rev';"
   ```

5. Review `personal.log` for the failure root cause.

6. Restart with corrected configuration:
   ```bash
   gitsvnsync-personal --config <path> start --foreground
   # Watch output until satisfied, then restart in background mode
   ```

### Post-Incident

7. Capture forensic artifacts:
   ```bash
   cp personal.db personal.db.incident-$(date +%Y%m%d)
   cp personal.log personal.log.incident-$(date +%Y%m%d)
   ```

8. File an issue with the captured artifacts for root cause analysis.

## Incident Capture Checklist

When investigating a soak or production failure, collect:

- [ ] `personal.log` (full contents, not truncated)
- [ ] `personal.db` (SQLite database with audit log and watermarks)
- [ ] SVN server logs (if accessible)
- [ ] GitHub API rate limit status (`X-RateLimit-*` headers)
- [ ] Network connectivity test results
- [ ] Environment variables (sanitized — no tokens)
- [ ] Daemon process status (`ps aux | grep gitsvnsync`)
- [ ] Disk space (`df -h`)
- [ ] Soak artifact bundle (if from a scripted run)

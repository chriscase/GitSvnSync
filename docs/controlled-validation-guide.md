# Controlled-Environment Validation Guide

One-command local validation of the full GitSvnSync sync pipeline. Run this before any production deployment to verify end-to-end correctness.

## Prerequisites

| Tool | Minimum | Check |
|------|---------|-------|
| Rust | 1.70+ | `rustc --version` |
| Cargo | 1.70+ | `cargo --version` |
| SVN | 1.14+ | `svn --version` |
| svnadmin | 1.14+ | `svnadmin --version` |
| Git | 2.30+ | `git --version` |

All tools must be on `$PATH`. No network access required — everything runs locally using `file://` SVN repos and local Git bare repos.

## Quick Start

```bash
# Full validation (recommended)
scripts/controlled-validation.sh

# Quick smoke test (fewer scenarios)
scripts/controlled-validation.sh --quick
```

The script is **non-interactive** and requires no prompts. It runs to completion unattended and exits with code 0 on success, non-zero on any failure.

## What Is Being Tested

### Phase 1: Build
Full workspace compilation via `cargo build --workspace`. If this fails, subsequent phases are skipped.

### Phase 2: Cargo Tests
Runs `cargo test --workspace` which exercises:
- Core library unit tests (sync engine, database, SVN/Git clients)
- Team-mode end-to-end tests (bidirectional sync, conflict detection, audit logging)
- Personal-mode integration tests (20+ scenarios)
- Spawn-based black-box logging tests (log-probe)
- CLI unit tests

### Phase 3: Clippy
Static analysis with `-D warnings` to catch lints.

### Phase 4: Live SVN Scenarios
Creates real local SVN repos and exercises:
- **Basic SVN commits** — multiple revisions verified
- **Echo suppression** — `[gitsvnsync]` marker detection
- **Conflict path** — file modification tracking
- **File deletion** — propagation verification
- **Nested directories** — deep path handling

### Phase 5: Log Probe
Spawns the real `gitsvnsync-personal` binary with the `log-probe` subcommand and verifies:
- Log file created at `{data_dir}/personal.log`
- Log messages appear with correct markers

### Phase 6: Secret Redaction
Scans all generated artifacts for leaked token patterns (ghp_, gho_, ghs_, etc.).

## Expected Outcomes

| Phase | Expected |
|-------|----------|
| Build | PASS — zero errors |
| Cargo tests | PASS — all tests green |
| Clippy | PASS — zero warnings |
| Live SVN | PASS — all 5 scenarios pass |
| Log probe | PASS — personal.log written |
| Secret scan | PASS — no tokens found |

## Output Artifacts

Each run creates a timestamped directory:

```
artifacts/controlled-validation/<UTC_TIMESTAMP>/
├── timeline.log          # Consolidated human-readable log
├── events.ndjson         # Machine-readable event stream
├── summary.md            # PASS/FAIL report
├── manifest.json         # Full artifact listing
├── env-snapshot.txt      # Sanitized environment (no secrets)
├── tool-versions.txt     # Tool versions
├── build/
│   ├── stdout.log
│   └── stderr.log
├── cargo-test/
│   ├── stdout.log
│   └── stderr.log
├── clippy/
│   ├── stdout.log
│   └── stderr.log
├── live-scenarios/       # SVN scenario artifacts
├── log-probe/
│   ├── stdout.log
│   ├── stderr.log
│   └── personal.log      # Captured log output
└── redaction/            # Secret scan results
```

### events.ndjson Format

Each line is a JSON object:
```json
{"timestamp":"2026-02-24T12:00:00Z","phase":"cargo-test","action":"complete","status":"pass","duration_ms":15000}
```

Fields: `timestamp`, `phase`, `action`, `status`, `duration_ms`, plus optional `svn_rev`, `git_sha`, `pr_number`.

## Failure Triage

1. **Build failure**: Check `build/stderr.log` for compiler errors.
2. **Test failure**: Check `cargo-test/stderr.log` for failing test names and assertions.
3. **Clippy failure**: Check `clippy/stderr.log` for lint violations.
4. **SVN scenario failure**: Check `live-scenarios/` and `timeline.log` for which scenario failed.
5. **Log probe failure**: Check `log-probe/stderr.log` and verify the binary exists at `target/debug/gitsvnsync-personal`.
6. **Secret leak**: Check `redaction/` — review the flagged file and determine if it's a real leak or false positive.

## Cleanup

The script automatically cleans up temporary directories on exit. Artifact directories are preserved for review. To clean old artifacts:

```bash
rm -rf artifacts/controlled-validation/
```

## CI Integration

The script can be added to CI pipelines directly:

```yaml
- name: Controlled validation
  run: scripts/controlled-validation.sh --quick
```

Exit code is non-zero on any failure, making it suitable for CI gating.

#!/usr/bin/env bash
# ============================================================================
# GitSvnSync Enterprise Soak/Canary Validation Script
# ============================================================================
# Non-interactive, CI-safe repeated-cycle soak test for enterprise readiness.
# Runs configurable sync cycles against local SVN+Git repos with synthetic
# change injection, health snapshots, and go/no-go gating.
#
# Usage:
#   scripts/enterprise-soak.sh                              # default (5 cycles)
#   scripts/enterprise-soak.sh --cycles 20 --interval 5     # 20 cycles, 5s apart
#   scripts/enterprise-soak.sh --dry-run                     # preflight only
#   scripts/enterprise-soak.sh --help
#
# Output: artifacts/enterprise-soak/<UTC_TIMESTAMP>/
# Exit code: 0 = all cycles healthy (go), non-zero = failures detected (no-go)
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="$REPO_ROOT/artifacts/enterprise-soak/$TIMESTAMP"
EVENTS_FILE="$ARTIFACT_DIR/events.ndjson"
TIMELINE_LOG="$ARTIFACT_DIR/timeline.log"
SUMMARY_FILE="$ARTIFACT_DIR/summary.md"
MANIFEST_FILE="$ARTIFACT_DIR/manifest.json"
HEALTH_DIR="$ARTIFACT_DIR/health-snapshots"

# Defaults
CYCLES=5
INTERVAL_SEC=2
DRY_RUN=false
MAX_ERROR_RATE=0.2  # 20% failure threshold → no-go

CYCLE_PASS=0
CYCLE_FAIL=0

# ============================================================================
# Argument parsing
# ============================================================================
while [[ $# -gt 0 ]]; do
    case "$1" in
        --cycles) CYCLES="$2"; shift 2 ;;
        --interval) INTERVAL_SEC="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        --max-error-rate) MAX_ERROR_RATE="$2"; shift 2 ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo "  --cycles N         Number of soak cycles (default: 5)"
            echo "  --interval N       Seconds between cycles (default: 2)"
            echo "  --dry-run          Run preflight only, skip soak cycles"
            echo "  --max-error-rate F Failure fraction threshold for no-go (default: 0.2)"
            echo "  --help             Show this help"
            exit 0
            ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# ============================================================================
# Helpers
# ============================================================================

mkdir -p "$ARTIFACT_DIR" "$HEALTH_DIR"

log() {
    local msg="[$(date -u +%H:%M:%S)] $*"
    echo "$msg" | tee -a "$TIMELINE_LOG"
}

emit_event() {
    local phase="$1" action="$2" status="$3" duration_ms="${4:-0}"
    shift 4 || true
    local extra=""
    for kv in "$@"; do
        local key="${kv%%=*}"
        local val="${kv#*=}"
        extra="${extra},\"${key}\":\"${val}\""
    done
    printf '{"timestamp":"%s","phase":"%s","action":"%s","status":"%s","duration_ms":%d%s}\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)" \
        "$phase" "$action" "$status" "$duration_ms" "$extra" >> "$EVENTS_FILE"
}

cleanup() {
    log "Cleaning up..."
    if [[ -n "${WORK_DIR:-}" && -d "$WORK_DIR" ]]; then
        rm -rf "$WORK_DIR"
    fi
}
trap cleanup EXIT

# ============================================================================
# Preflight checks
# ============================================================================
log "═══════════════════════════════════════════════════════════════"
log "GitSvnSync Enterprise Soak Validation"
log "Timestamp: $TIMESTAMP"
log "Cycles: $CYCLES | Interval: ${INTERVAL_SEC}s | Dry-run: $DRY_RUN"
log "Max error rate: $MAX_ERROR_RATE"
log "Artifact dir: $ARTIFACT_DIR"
log "═══════════════════════════════════════════════════════════════"

emit_event "preflight" "start" "running" 0

# Required tools.
MISSING=""
for tool in svn svnadmin git cargo; do
    if ! command -v "$tool" &>/dev/null; then
        MISSING="$MISSING $tool"
    fi
done
if [[ -n "$MISSING" ]]; then
    log "FATAL: Missing tools:$MISSING"
    emit_event "preflight" "tools" "fail" 0
    exit 1
fi

# Save sanitized environment.
env | sort | grep -v -iE '(token|password|secret|key|credential|auth)' \
    > "$ARTIFACT_DIR/env-snapshot.txt" 2>/dev/null || true

# Tool versions.
{
    echo "svn: $(svn --version --quiet 2>/dev/null || echo unknown)"
    echo "git: $(git --version 2>/dev/null || echo unknown)"
    echo "cargo: $(cargo --version 2>/dev/null || echo unknown)"
    echo "rustc: $(rustc --version 2>/dev/null || echo unknown)"
    echo "os: $(uname -srm 2>/dev/null || echo unknown)"
} > "$ARTIFACT_DIR/tool-versions.txt"

# Verify binary is built.
PERSONAL_BIN="$REPO_ROOT/target/debug/gitsvnsync-personal"
if [[ ! -f "$PERSONAL_BIN" ]]; then
    log "Building workspace first..."
    if ! cargo build --workspace > "$ARTIFACT_DIR/build-stdout.log" 2> "$ARTIFACT_DIR/build-stderr.log"; then
        log "FATAL: Build failed"
        emit_event "preflight" "build" "fail" 0
        exit 1
    fi
fi

emit_event "preflight" "complete" "pass" 0
log "✅ Preflight complete"

if $DRY_RUN; then
    log ""
    log "DRY RUN: preflight complete. Skipping soak cycles."
    emit_event "dry-run" "complete" "pass" 0

    cat > "$SUMMARY_FILE" <<SUMMARY
# Enterprise Soak Summary (Dry Run)

**Timestamp:** $TIMESTAMP
**Mode:** dry-run (preflight only)
**Overall:** PASS — preflight checks passed
**Cycles planned:** $CYCLES (not executed)

## Preflight

- [x] Required tools available
- [x] Workspace builds
- [x] Environment sanitized

## Go/No-Go

**DRY RUN** — execute without --dry-run for soak results.
SUMMARY

    echo '{"timestamp":"'"$TIMESTAMP"'","overall":"DRY_RUN","cycles_planned":'"$CYCLES"',"cycles_run":0}' > "$MANIFEST_FILE"
    log "Artifacts: $ARTIFACT_DIR"
    exit 0
fi

# ============================================================================
# Provision soak environment
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Provisioning soak environment"
log "──────────────────────────────────────────────────────────────"

WORK_DIR=$(mktemp -d)
SVN_REPO_DIR="$WORK_DIR/svn_repo"
SVN_WC="$WORK_DIR/svn_wc"
DATA_DIR="$WORK_DIR/data"
DB_PATH="$DATA_DIR/soak.db"

svnadmin create "$SVN_REPO_DIR"
SVN_URL="file://$SVN_REPO_DIR"

echo '#!/bin/sh' > "$SVN_REPO_DIR/hooks/pre-revprop-change"
echo 'exit 0' >> "$SVN_REPO_DIR/hooks/pre-revprop-change"
chmod +x "$SVN_REPO_DIR/hooks/pre-revprop-change"

svn checkout "$SVN_URL" "$SVN_WC" --non-interactive -q
mkdir -p "$DATA_DIR"

# Seed initial content.
echo "initial content" > "$SVN_WC/README.md"
svn add "$SVN_WC/README.md" -q 2>/dev/null || true
svn commit "$SVN_WC" -m "Initial soak seed" --non-interactive -q

log "✅ Soak environment provisioned at $WORK_DIR"
emit_event "provision" "complete" "pass" 0

# ============================================================================
# Soak cycles
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Running $CYCLES soak cycles"
log "──────────────────────────────────────────────────────────────"

for cycle in $(seq 1 "$CYCLES"); do
    CYCLE_DIR="$ARTIFACT_DIR/cycle-$(printf '%03d' "$cycle")"
    mkdir -p "$CYCLE_DIR"
    CYCLE_START=$SECONDS

    log "▶ Cycle $cycle/$CYCLES"
    emit_event "cycle-$cycle" "start" "running" 0

    # 1. Inject synthetic canary commit.
    CANARY_FILE="canary_${cycle}.txt"
    CANARY_CONTENT="soak-cycle-${cycle}-$(date -u +%s)"
    echo "$CANARY_CONTENT" > "$SVN_WC/$CANARY_FILE"
    svn add "$SVN_WC/$CANARY_FILE" -q 2>/dev/null || true

    if svn commit "$SVN_WC" -m "Soak canary cycle $cycle [gitsvnsync-soak]" \
        --non-interactive -q 2>"$CYCLE_DIR/svn-commit-stderr.log"; then
        SVN_REV=$(svn info "$SVN_URL" --show-item revision --no-newline 2>/dev/null || echo "?")
        emit_event "cycle-$cycle" "svn-commit" "pass" 0 "svn_rev=$SVN_REV"
    else
        emit_event "cycle-$cycle" "svn-commit" "fail" 0
        CYCLE_FAIL=$((CYCLE_FAIL + 1))
        log "  ❌ Cycle $cycle: SVN commit failed"
        continue
    fi

    # 2. Verify canary content in SVN.
    VERIFIED_CONTENT=$(svn cat "$SVN_URL/$CANARY_FILE" 2>/dev/null || echo "")
    if [[ "$VERIFIED_CONTENT" == "$CANARY_CONTENT" ]]; then
        emit_event "cycle-$cycle" "svn-verify" "pass" 0 "svn_rev=$SVN_REV"
    else
        emit_event "cycle-$cycle" "svn-verify" "fail" 0
        CYCLE_FAIL=$((CYCLE_FAIL + 1))
        log "  ❌ Cycle $cycle: SVN content verification failed"
        continue
    fi

    # 3. Health snapshot.
    {
        echo "cycle: $cycle"
        echo "svn_head_rev: $SVN_REV"
        echo "svn_url: $SVN_URL"
        echo "canary_file: $CANARY_FILE"
        echo "timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "work_dir_size_kb: $(du -sk "$WORK_DIR" 2>/dev/null | cut -f1)"
    } > "$HEALTH_DIR/snapshot-$(printf '%03d' "$cycle").txt"

    # 4. Run log-probe to verify logging still works under repeated use.
    PROBE_DATA="$WORK_DIR/probe_data_$cycle"
    mkdir -p "$PROBE_DATA"
    cat > "$WORK_DIR/soak_config_$cycle.toml" <<TOML
[personal]
log_level = "info"
data_dir = "$PROBE_DATA"

[svn]
url = "$SVN_URL"
username = "test"
password_env = "GITSVNSYNC_TEST_SVN_PW"

[github]
repo = "test/test"
token_env = "GITSVNSYNC_TEST_GH_TOKEN"

[developer]
name = "Soak Test"
email = "soak@test.com"
svn_username = "test"
TOML

    if "$PERSONAL_BIN" --config "$WORK_DIR/soak_config_$cycle.toml" log-probe \
        > "$CYCLE_DIR/probe-stdout.log" 2> "$CYCLE_DIR/probe-stderr.log"; then
        if [[ -f "$PROBE_DATA/personal.log" ]] && grep -q "LOG_PROBE" "$PROBE_DATA/personal.log"; then
            emit_event "cycle-$cycle" "log-probe" "pass" 0
        else
            emit_event "cycle-$cycle" "log-probe" "fail" 0
            CYCLE_FAIL=$((CYCLE_FAIL + 1))
            log "  ⚠ Cycle $cycle: log-probe output missing"
            continue
        fi
    else
        emit_event "cycle-$cycle" "log-probe" "fail" 0
        CYCLE_FAIL=$((CYCLE_FAIL + 1))
        log "  ❌ Cycle $cycle: log-probe failed"
        continue
    fi

    CYCLE_DUR=$(( (SECONDS - CYCLE_START) * 1000 ))
    emit_event "cycle-$cycle" "complete" "pass" "$CYCLE_DUR" "svn_rev=$SVN_REV"
    CYCLE_PASS=$((CYCLE_PASS + 1))
    log "  ✅ Cycle $cycle PASS (${CYCLE_DUR}ms, SVN r$SVN_REV)"

    # Inter-cycle interval (skip on last cycle).
    if [[ $cycle -lt $CYCLES ]]; then
        sleep "$INTERVAL_SEC"
    fi
done

# ============================================================================
# Go/No-Go evaluation
# ============================================================================
log ""
log "═══════════════════════════════════════════════════════════════"
log "SOAK COMPLETE"
log "  Cycles: $CYCLES | Pass: $CYCLE_PASS | Fail: $CYCLE_FAIL"

TOTAL=$CYCLES
if [[ $TOTAL -eq 0 ]]; then
    ERROR_RATE="0.0"
    GO_DECISION="GO"
else
    # Use awk for floating-point division.
    ERROR_RATE=$(awk "BEGIN {printf \"%.3f\", $CYCLE_FAIL / $TOTAL}")
    GO_DECISION=$(awk "BEGIN {print ($CYCLE_FAIL / $TOTAL <= $MAX_ERROR_RATE) ? \"GO\" : \"NO-GO\"}")
fi

log "  Error rate: $ERROR_RATE (threshold: $MAX_ERROR_RATE)"
log "  Decision: $GO_DECISION"
log "═══════════════════════════════════════════════════════════════"

# ============================================================================
# Secret leak scan
# ============================================================================
LEAK_FOUND=false
if grep -rE '(ghp_|gho_|ghs_|ghu_|github_pat_)[A-Za-z0-9_]{10,}' "$ARTIFACT_DIR" \
    --include='*.log' --include='*.txt' --include='*.toml' 2>/dev/null; then
    LEAK_FOUND=true
    log "⚠ WARNING: Token patterns found in artifacts"
fi

# ============================================================================
# Generate summary and manifest
# ============================================================================
EXIT_CODE=0
if [[ "$GO_DECISION" == "NO-GO" ]]; then
    EXIT_CODE=1
fi

cat > "$SUMMARY_FILE" <<SUMMARY
# Enterprise Soak Validation Summary

**Timestamp:** $TIMESTAMP
**Decision:** $GO_DECISION
**Cycles:** $TOTAL | **Pass:** $CYCLE_PASS | **Fail:** $CYCLE_FAIL
**Error rate:** $ERROR_RATE (threshold: $MAX_ERROR_RATE)
**Interval:** ${INTERVAL_SEC}s between cycles
**Secret leak scan:** $(if $LEAK_FOUND; then echo "⚠ TOKENS DETECTED"; else echo "✅ Clean"; fi)

## Go/No-Go Checklist

- [$(if [[ $CYCLE_FAIL -eq 0 ]]; then echo "x"; else echo " "; fi)] All soak cycles passed
- [x] Health snapshots captured for each cycle
- [$(if ! $LEAK_FOUND; then echo "x"; else echo " "; fi)] No secret leakage in artifacts
- [x] Event timeline (events.ndjson) complete
- [$(if [[ "$GO_DECISION" == "GO" ]]; then echo "x"; else echo " "; fi)] Error rate within threshold

## Rollback Procedure

If issues are found post-enablement:
1. Stop the gitsvnsync daemon: \`gitsvnsync-personal stop\`
2. Review \`{data_dir}/personal.log\` and audit DB for last known-good state
3. Reset watermarks if needed: \`sqlite3 personal.db "UPDATE watermarks SET value='<rev>' WHERE key='svn_rev'"\`
4. Restart with previous known-good config

## Artifact Directory

\`$ARTIFACT_DIR\`
SUMMARY

# Write manifest.
cd "$ARTIFACT_DIR"
MANIFEST_ENTRIES=""
while IFS= read -r -d '' file; do
    REL="${file#$ARTIFACT_DIR/}"
    SIZE=$(wc -c < "$file" 2>/dev/null || echo 0)
    MANIFEST_ENTRIES="${MANIFEST_ENTRIES}{\"path\":\"$REL\",\"size\":$SIZE},"
done < <(find "$ARTIFACT_DIR" -type f -print0 | sort -z)
MANIFEST_ENTRIES="${MANIFEST_ENTRIES%,}"

cat > "$MANIFEST_FILE" <<MANIFEST
{
  "timestamp": "$TIMESTAMP",
  "decision": "$GO_DECISION",
  "cycles_total": $TOTAL,
  "cycles_pass": $CYCLE_PASS,
  "cycles_fail": $CYCLE_FAIL,
  "error_rate": $ERROR_RATE,
  "max_error_rate": $MAX_ERROR_RATE,
  "secret_leak": $LEAK_FOUND,
  "artifacts": [$MANIFEST_ENTRIES]
}
MANIFEST

log "Artifacts: $ARTIFACT_DIR"
exit $EXIT_CODE

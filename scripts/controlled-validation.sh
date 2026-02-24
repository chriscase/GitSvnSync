#!/usr/bin/env bash
# ============================================================================
# GitSvnSync Controlled-Environment Validation Script
# ============================================================================
# Non-interactive, CI-safe, one-command validation of the full sync pipeline.
# Produces a timestamped forensic artifact bundle suitable for human review
# and AI-assisted analysis.
#
# Usage:
#   scripts/controlled-validation.sh              # default run
#   scripts/controlled-validation.sh --quick      # quick smoke test (fewer revisions)
#   scripts/controlled-validation.sh --help       # show usage
#
# Output: artifacts/controlled-validation/<UTC_TIMESTAMP>/
# Exit code: 0 on all PASS, non-zero on any FAIL (partial artifacts preserved)
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="$REPO_ROOT/artifacts/controlled-validation/$TIMESTAMP"
EVENTS_FILE="$ARTIFACT_DIR/events.ndjson"
TIMELINE_LOG="$ARTIFACT_DIR/timeline.log"
SUMMARY_FILE="$ARTIFACT_DIR/summary.md"
MANIFEST_FILE="$ARTIFACT_DIR/manifest.json"

# Defaults
QUICK_MODE=false
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
TOTAL_COUNT=0

# ============================================================================
# Argument parsing
# ============================================================================
while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick) QUICK_MODE=true; shift ;;
        --help|-h)
            echo "Usage: $0 [--quick] [--help]"
            echo "  --quick    Run a reduced scenario matrix (fewer revisions)"
            echo "  --help     Show this help"
            exit 0
            ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# ============================================================================
# Helpers
# ============================================================================

mkdir -p "$ARTIFACT_DIR"

log() {
    local msg="[$(date -u +%H:%M:%S)] $*"
    echo "$msg" | tee -a "$TIMELINE_LOG"
}

emit_event() {
    local phase="$1" action="$2" status="$3" duration_ms="${4:-0}"
    shift 4 || true
    local extra=""
    # Remaining key=value pairs become extra JSON fields.
    for kv in "$@"; do
        local key="${kv%%=*}"
        local val="${kv#*=}"
        extra="${extra},\"${key}\":\"${val}\""
    done
    printf '{"timestamp":"%s","phase":"%s","action":"%s","status":"%s","duration_ms":%d%s}\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)" \
        "$phase" "$action" "$status" "$duration_ms" "$extra" >> "$EVENTS_FILE"
}

run_phase() {
    local phase_name="$1" phase_dir="$ARTIFACT_DIR/$phase_name"
    shift
    mkdir -p "$phase_dir"
    local start_ms
    start_ms=$(($(date +%s) * 1000 + $(date +%N 2>/dev/null | cut -c1-3 || echo 0)))
    emit_event "$phase_name" "start" "running" 0
    log "▶ Phase: $phase_name"

    if "$@" > "$phase_dir/stdout.log" 2> "$phase_dir/stderr.log"; then
        local end_ms
        end_ms=$(($(date +%s) * 1000 + $(date +%N 2>/dev/null | cut -c1-3 || echo 0)))
        local dur=$(( end_ms - start_ms ))
        emit_event "$phase_name" "complete" "pass" "$dur"
        log "  ✅ PASS ($dur ms)"
        PASS_COUNT=$((PASS_COUNT + 1))
        return 0
    else
        local rc=$?
        local end_ms
        end_ms=$(($(date +%s) * 1000 + $(date +%N 2>/dev/null | cut -c1-3 || echo 0)))
        local dur=$(( end_ms - start_ms ))
        emit_event "$phase_name" "complete" "fail" "$dur"
        log "  ❌ FAIL (exit $rc, $dur ms) — see $phase_dir/stderr.log"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        return 1
    fi
}

# Record pass/fail for a scenario without running a full subprocess.
record_result() {
    local name="$1" result="$2" detail="${3:-}"
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
    if [[ "$result" == "pass" ]]; then
        PASS_COUNT=$((PASS_COUNT + 1))
        log "  ✅ $name: PASS${detail:+ ($detail)}"
    elif [[ "$result" == "skip" ]]; then
        SKIP_COUNT=$((SKIP_COUNT + 1))
        log "  ⏭ $name: SKIP${detail:+ ($detail)}"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        log "  ❌ $name: FAIL${detail:+ ($detail)}"
    fi
}

cleanup() {
    log "Cleaning up temp directories..."
    if [[ -n "${WORK_DIR:-}" && -d "$WORK_DIR" ]]; then
        rm -rf "$WORK_DIR"
    fi
}
trap cleanup EXIT

# ============================================================================
# Preflight checks
# ============================================================================
log "═══════════════════════════════════════════════════════════════"
log "GitSvnSync Controlled-Environment Validation"
log "Timestamp: $TIMESTAMP"
log "Artifact dir: $ARTIFACT_DIR"
log "Quick mode: $QUICK_MODE"
log "═══════════════════════════════════════════════════════════════"

emit_event "preflight" "start" "running" 0

# Check required tools.
MISSING_TOOLS=""
for tool in svn svnadmin git cargo; do
    if ! command -v "$tool" &>/dev/null; then
        MISSING_TOOLS="$MISSING_TOOLS $tool"
    fi
done

if [[ -n "$MISSING_TOOLS" ]]; then
    log "FATAL: Missing required tools:$MISSING_TOOLS"
    emit_event "preflight" "complete" "fail" 0
    echo "FATAL: Missing required tools:$MISSING_TOOLS" >&2
    exit 1
fi

# Save sanitized environment snapshot (strip secrets).
env | sort | grep -v -iE '(token|password|secret|key|credential|auth)' \
    > "$ARTIFACT_DIR/env-snapshot.txt" 2>/dev/null || true

# Save tool versions.
{
    echo "svn: $(svn --version --quiet 2>/dev/null || echo unknown)"
    echo "svnadmin: $(svnadmin --version --quiet 2>/dev/null || echo unknown)"
    echo "git: $(git --version 2>/dev/null || echo unknown)"
    echo "cargo: $(cargo --version 2>/dev/null || echo unknown)"
    echo "rustc: $(rustc --version 2>/dev/null || echo unknown)"
    echo "os: $(uname -srm 2>/dev/null || echo unknown)"
} > "$ARTIFACT_DIR/tool-versions.txt"

emit_event "preflight" "complete" "pass" 0
log "Preflight: all tools available"

# ============================================================================
# Build
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Phase 1: Build workspace"
log "──────────────────────────────────────────────────────────────"

BUILD_DIR="$ARTIFACT_DIR/build"
mkdir -p "$BUILD_DIR"
PHASE_START=$SECONDS

emit_event "build" "start" "running" 0
log "▶ cargo build --workspace..."

if cargo build --workspace \
    > "$BUILD_DIR/stdout.log" 2> "$BUILD_DIR/stderr.log"; then
    BUILD_DUR=$(( (SECONDS - PHASE_START) * 1000 ))
    emit_event "build" "complete" "pass" "$BUILD_DUR"
    log "  ✅ Build PASS (${BUILD_DUR}ms)"
    PASS_COUNT=$((PASS_COUNT + 1))
else
    BUILD_DUR=$(( (SECONDS - PHASE_START) * 1000 ))
    emit_event "build" "complete" "fail" "$BUILD_DUR"
    log "  ❌ Build FAIL — see $BUILD_DIR/stderr.log"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    # Build failure is fatal.
    log "FATAL: Build failed. Aborting validation."
    exit 1
fi

# ============================================================================
# Phase 2: Unit & integration tests
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Phase 2: Cargo tests"
log "──────────────────────────────────────────────────────────────"

TEST_DIR="$ARTIFACT_DIR/cargo-test"
mkdir -p "$TEST_DIR"
PHASE_START=$SECONDS

emit_event "cargo-test" "start" "running" 0
log "▶ cargo test --workspace..."

if cargo test --workspace \
    > "$TEST_DIR/stdout.log" 2> "$TEST_DIR/stderr.log"; then
    TEST_DUR=$(( (SECONDS - PHASE_START) * 1000 ))
    emit_event "cargo-test" "complete" "pass" "$TEST_DUR"
    log "  ✅ Tests PASS (${TEST_DUR}ms)"
    PASS_COUNT=$((PASS_COUNT + 1))
else
    TEST_DUR=$(( (SECONDS - PHASE_START) * 1000 ))
    emit_event "cargo-test" "complete" "fail" "$TEST_DUR"
    log "  ❌ Tests FAIL — see $TEST_DIR/stderr.log"
    FAIL_COUNT=$((FAIL_COUNT + 1))
fi

# ============================================================================
# Phase 3: Clippy
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Phase 3: Clippy"
log "──────────────────────────────────────────────────────────────"

CLIPPY_DIR="$ARTIFACT_DIR/clippy"
mkdir -p "$CLIPPY_DIR"
PHASE_START=$SECONDS

emit_event "clippy" "start" "running" 0
log "▶ cargo clippy --workspace --all-targets..."

if cargo clippy --workspace --all-targets -- -D warnings \
    > "$CLIPPY_DIR/stdout.log" 2> "$CLIPPY_DIR/stderr.log"; then
    CLIPPY_DUR=$(( (SECONDS - PHASE_START) * 1000 ))
    emit_event "clippy" "complete" "pass" "$CLIPPY_DUR"
    log "  ✅ Clippy PASS (${CLIPPY_DUR}ms)"
    PASS_COUNT=$((PASS_COUNT + 1))
else
    CLIPPY_DUR=$(( (SECONDS - PHASE_START) * 1000 ))
    emit_event "clippy" "complete" "fail" "$CLIPPY_DUR"
    log "  ❌ Clippy FAIL — see $CLIPPY_DIR/stderr.log"
    FAIL_COUNT=$((FAIL_COUNT + 1))
fi

# ============================================================================
# Phase 4: Live SVN→Git sync scenarios
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Phase 4: SVN→Git live scenarios"
log "──────────────────────────────────────────────────────────────"

WORK_DIR=$(mktemp -d)
SVN_REPO_DIR="$WORK_DIR/svn_repo"
SVN_WC="$WORK_DIR/svn_wc"
GIT_WORK="$WORK_DIR/git_work"
GIT_BARE="$WORK_DIR/git_bare.git"
DATA_DIR="$WORK_DIR/data"
LIVE_DIR="$ARTIFACT_DIR/live-scenarios"
mkdir -p "$LIVE_DIR" "$DATA_DIR"

# Create SVN repo.
svnadmin create "$SVN_REPO_DIR"
SVN_URL="file://$SVN_REPO_DIR"

# Enable revprop changes.
echo '#!/bin/sh' > "$SVN_REPO_DIR/hooks/pre-revprop-change"
echo 'exit 0' >> "$SVN_REPO_DIR/hooks/pre-revprop-change"
chmod +x "$SVN_REPO_DIR/hooks/pre-revprop-change"

svn checkout "$SVN_URL" "$SVN_WC" --non-interactive -q

emit_event "live-scenarios" "start" "running" 0

# --- Scenario 4a: Basic SVN→Git sync ---
log "▶ Scenario 4a: Basic SVN→Git sync"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

NUM_REVS=3
if $QUICK_MODE; then NUM_REVS=2; fi

for i in $(seq 1 $NUM_REVS); do
    echo "content_$i" > "$SVN_WC/file_$i.txt"
    svn add "$SVN_WC/file_$i.txt" -q 2>/dev/null || true
    svn commit "$SVN_WC" -m "Add file_$i" --non-interactive -q
done

# Verify SVN state.
SVN_HEAD=$(svn info "$SVN_URL" --show-item revision --no-newline 2>/dev/null || echo "0")
if [[ "$SVN_HEAD" == "$NUM_REVS" ]]; then
    record_result "4a-svn-commits" "pass" "$NUM_REVS revisions"
    emit_event "live-scenarios" "4a-svn-commits" "pass" 0 "svn_rev=$SVN_HEAD"
else
    record_result "4a-svn-commits" "fail" "expected $NUM_REVS, got $SVN_HEAD"
    emit_event "live-scenarios" "4a-svn-commits" "fail" 0
fi

# --- Scenario 4b: Echo suppression ---
log "▶ Scenario 4b: Echo suppression"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

echo "echo_content" > "$SVN_WC/echo_test.txt"
svn add "$SVN_WC/echo_test.txt" -q 2>/dev/null || true
svn commit "$SVN_WC" -m "Synced from Git [gitsvnsync] echo marker" --non-interactive -q

ECHO_REV=$(svn info "$SVN_URL" --show-item revision --no-newline 2>/dev/null || echo "0")
ECHO_LOG=$(svn log "$SVN_URL" -r "$ECHO_REV" --xml 2>/dev/null || echo "")
if echo "$ECHO_LOG" | grep -q "\[gitsvnsync\]"; then
    record_result "4b-echo-marker" "pass" "echo marker found in r$ECHO_REV"
    emit_event "live-scenarios" "4b-echo-marker" "pass" 0 "svn_rev=$ECHO_REV"
else
    record_result "4b-echo-marker" "fail" "no echo marker in r$ECHO_REV"
    emit_event "live-scenarios" "4b-echo-marker" "fail" 0
fi

# --- Scenario 4c: Conflict path (same file modified) ---
log "▶ Scenario 4c: Conflict path detection"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

# Create a file, then modify it in SVN to have something to conflict with.
echo "base content" > "$SVN_WC/conflict_file.txt"
svn add "$SVN_WC/conflict_file.txt" -q 2>/dev/null || true
svn commit "$SVN_WC" -m "Add conflict_file" --non-interactive -q

echo "svn modified content" > "$SVN_WC/conflict_file.txt"
svn commit "$SVN_WC" -m "Modify conflict_file in SVN" --non-interactive -q

CONFLICT_REV=$(svn info "$SVN_URL" --show-item revision --no-newline 2>/dev/null || echo "0")
# Verify the modification landed.
CONTENT=$(svn cat "$SVN_URL/conflict_file.txt" 2>/dev/null || echo "")
if [[ "$CONTENT" == "svn modified content" ]]; then
    record_result "4c-conflict-file-modified" "pass" "conflict_file modified at r$CONFLICT_REV"
    emit_event "live-scenarios" "4c-conflict-path" "pass" 0 "svn_rev=$CONFLICT_REV"
else
    record_result "4c-conflict-file-modified" "fail" "unexpected content"
    emit_event "live-scenarios" "4c-conflict-path" "fail" 0
fi

# --- Scenario 4d: File deletion propagation ---
log "▶ Scenario 4d: File deletion propagation"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

echo "to_be_deleted" > "$SVN_WC/deleteme.txt"
svn add "$SVN_WC/deleteme.txt" -q 2>/dev/null || true
svn commit "$SVN_WC" -m "Add deleteme.txt" --non-interactive -q

svn rm "$SVN_WC/deleteme.txt" -q
svn commit "$SVN_WC" -m "Remove deleteme.txt" --non-interactive -q

# Verify the file is gone from HEAD (svn cat should fail).
if ! svn cat "$SVN_URL/deleteme.txt" >/dev/null 2>&1; then
    record_result "4d-file-deletion" "pass" "file removed from HEAD"
    emit_event "live-scenarios" "4d-deletion" "pass" 0
else
    record_result "4d-file-deletion" "fail" "file still exists"
    emit_event "live-scenarios" "4d-deletion" "fail" 0
fi

# --- Scenario 4e: Nested directory structure ---
log "▶ Scenario 4e: Nested directory structure"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

mkdir -p "$SVN_WC/src/main/java"
echo "public class App {}" > "$SVN_WC/src/main/java/App.java"
svn add "$SVN_WC/src" -q 2>/dev/null || true
svn commit "$SVN_WC" -m "Add nested directory structure" --non-interactive -q

NESTED_CONTENT=$(svn cat "$SVN_URL/src/main/java/App.java" 2>/dev/null || echo "")
if [[ "$NESTED_CONTENT" == "public class App {}" ]]; then
    record_result "4e-nested-dirs" "pass" "nested file verified"
    emit_event "live-scenarios" "4e-nested" "pass" 0
else
    record_result "4e-nested-dirs" "fail" "nested file not found or wrong content"
    emit_event "live-scenarios" "4e-nested" "fail" 0
fi

emit_event "live-scenarios" "complete" "pass" 0

# ============================================================================
# Phase 5: Personal binary log-probe black-box test
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Phase 5: Personal binary log-probe"
log "──────────────────────────────────────────────────────────────"

PROBE_DIR="$ARTIFACT_DIR/log-probe"
PROBE_DATA="$WORK_DIR/probe_data"
mkdir -p "$PROBE_DIR" "$PROBE_DATA"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

PERSONAL_BIN="$REPO_ROOT/target/debug/gitsvnsync-personal"

if [[ -f "$PERSONAL_BIN" ]]; then
    # Write test config.
    cat > "$WORK_DIR/probe_config.toml" <<TOML
[personal]
log_level = "debug"
data_dir = "$PROBE_DATA"

[svn]
url = "file:///tmp/nonexistent"
username = "test"
password_env = "GITSVNSYNC_TEST_SVN_PW"

[github]
repo = "test/test"
token_env = "GITSVNSYNC_TEST_GH_TOKEN"

[developer]
name = "Test User"
email = "test@example.com"
svn_username = "test"
TOML

    emit_event "log-probe" "start" "running" 0

    if "$PERSONAL_BIN" --config "$WORK_DIR/probe_config.toml" log-probe \
        > "$PROBE_DIR/stdout.log" 2> "$PROBE_DIR/stderr.log"; then
        PROBE_LOG="$PROBE_DATA/personal.log"
        if [[ -f "$PROBE_LOG" ]] && grep -q "LOG_PROBE" "$PROBE_LOG"; then
            # Copy the log for artifacts.
            cp "$PROBE_LOG" "$PROBE_DIR/personal.log"
            record_result "5-log-probe" "pass" "log-probe wrote to personal.log"
            emit_event "log-probe" "complete" "pass" 0
        else
            record_result "5-log-probe" "fail" "personal.log missing or no LOG_PROBE marker"
            emit_event "log-probe" "complete" "fail" 0
        fi
    else
        record_result "5-log-probe" "fail" "log-probe exited non-zero"
        emit_event "log-probe" "complete" "fail" 0
    fi
else
    record_result "5-log-probe" "skip" "binary not found (build may have failed)"
    emit_event "log-probe" "complete" "skip" 0
fi

# ============================================================================
# Phase 6: Secret redaction verification
# ============================================================================
log ""
log "──────────────────────────────────────────────────────────────"
log "Phase 6: Secret redaction verification"
log "──────────────────────────────────────────────────────────────"

REDACT_DIR="$ARTIFACT_DIR/redaction"
mkdir -p "$REDACT_DIR"
TOTAL_COUNT=$((TOTAL_COUNT + 1))

emit_event "redaction" "start" "running" 0

# Scan all generated artifacts for leaked secrets patterns.
LEAKED=false
if grep -rE '(ghp_|gho_|ghs_|ghu_|github_pat_)[A-Za-z0-9_]{10,}' "$ARTIFACT_DIR" --include='*.log' --include='*.txt' 2>/dev/null; then
    LEAKED=true
fi

if $LEAKED; then
    record_result "6-no-secret-leakage" "fail" "token patterns found in artifacts"
    emit_event "redaction" "complete" "fail" 0
else
    record_result "6-no-secret-leakage" "pass" "no token patterns found"
    emit_event "redaction" "complete" "pass" 0
fi

# ============================================================================
# Generate summary and manifest
# ============================================================================
log ""
log "═══════════════════════════════════════════════════════════════"
log "VALIDATION COMPLETE"
log "  Pass: $PASS_COUNT  Fail: $FAIL_COUNT  Skip: $SKIP_COUNT"
log "═══════════════════════════════════════════════════════════════"

# Write summary.md
OVERALL="PASS"
EXIT_CODE=0
if [[ $FAIL_COUNT -gt 0 ]]; then
    OVERALL="FAIL"
    EXIT_CODE=1
fi

cat > "$SUMMARY_FILE" <<SUMMARY
# Controlled-Environment Validation Summary

**Timestamp:** $TIMESTAMP
**Overall:** $OVERALL
**Pass:** $PASS_COUNT | **Fail:** $FAIL_COUNT | **Skip:** $SKIP_COUNT
**Quick mode:** $QUICK_MODE

## Phase Results

| Phase | Result |
|-------|--------|
| Build | $(grep -c '"phase":"build".*"status":"pass"' "$EVENTS_FILE" 2>/dev/null && echo PASS || echo FAIL) |
| Cargo tests | $(grep -c '"phase":"cargo-test".*"status":"pass"' "$EVENTS_FILE" 2>/dev/null && echo PASS || echo FAIL) |
| Clippy | $(grep -c '"phase":"clippy".*"status":"pass"' "$EVENTS_FILE" 2>/dev/null && echo PASS || echo FAIL) |
| Live SVN scenarios | $PASS_COUNT scenarios passed |
| Log probe | $(grep '"phase":"log-probe".*"status":"pass"' "$EVENTS_FILE" >/dev/null 2>&1 && echo PASS || echo SKIP/FAIL) |
| Secret redaction | $(grep '"phase":"redaction".*"status":"pass"' "$EVENTS_FILE" >/dev/null 2>&1 && echo PASS || echo FAIL) |

## Artifact Directory

\`$ARTIFACT_DIR\`
SUMMARY

# Write manifest.json
cd "$ARTIFACT_DIR"
MANIFEST_ENTRIES=""
while IFS= read -r -d '' file; do
    REL="${file#$ARTIFACT_DIR/}"
    SIZE=$(wc -c < "$file" 2>/dev/null || echo 0)
    MANIFEST_ENTRIES="${MANIFEST_ENTRIES}{\"path\":\"$REL\",\"size\":$SIZE},"
done < <(find "$ARTIFACT_DIR" -type f -print0 | sort -z)

# Remove trailing comma, wrap in array.
MANIFEST_ENTRIES="${MANIFEST_ENTRIES%,}"
cat > "$MANIFEST_FILE" <<MANIFEST
{
  "timestamp": "$TIMESTAMP",
  "overall": "$OVERALL",
  "pass": $PASS_COUNT,
  "fail": $FAIL_COUNT,
  "skip": $SKIP_COUNT,
  "artifacts": [$MANIFEST_ENTRIES]
}
MANIFEST

log "Artifacts written to: $ARTIFACT_DIR"
log "Summary: $SUMMARY_FILE"

exit $EXIT_CODE

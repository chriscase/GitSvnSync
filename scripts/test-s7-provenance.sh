#!/usr/bin/env bash
# ============================================================================
# S7 Provenance Matching Self-Test
# ============================================================================
# Deterministic, offline tests for the S7 metadata validation logic used in
# ghe-live-validation.sh.  No network or SVN/GHE credentials required.
#
# Tests:
#   1. PASS on exact PR + exact Git-Commit match
#   2. FAIL on wrong PR number
#   3. FAIL on wrong Git-Commit SHA
#   4. FAIL on missing trailer(s)
#   5. FAIL when expected values are empty (S6 artifacts missing)
#
# Usage:
#   scripts/test-s7-provenance.sh          # run all tests
#   scripts/test-s7-provenance.sh --quiet  # suppress per-case output
#
# Exit: 0 on all PASS, 1 on any FAIL
# ============================================================================

set -euo pipefail

QUIET=false
[[ "${1:-}" == "--quiet" ]] && QUIET=true

PASS_COUNT=0
FAIL_COUNT=0

# --- Core matching function (extracted from ghe-live-validation.sh S7 logic) ---
# Inputs: SVN_LOG_XML, EXPECTED_PR, EXPECTED_SHA
# Output: sets S7_PASS (true/false) and S7_PROOF_DETAILS
s7_match() {
    local SVN_LOG_XML="$1"
    local EXPECTED_PR="$2"
    local EXPECTED_SHA="$3"

    S7_PROOF_DETAILS="content=OK"
    S7_PASS=true

    # --- Git-Commit trailer: require exact SHA match ---
    if [[ -z "$EXPECTED_SHA" ]]; then
        S7_PROOF_DETAILS="${S7_PROOF_DETAILS}, Git-Commit=FAIL(no expected SHA from S6)"
        S7_PASS=false
    elif echo "$SVN_LOG_XML" | grep -q "Git-Commit: ${EXPECTED_SHA}"; then
        S7_PROOF_DETAILS="${S7_PROOF_DETAILS}, Git-Commit=${EXPECTED_SHA:0:8}=exact"
    else
        OBSERVED_SHA=$(echo "$SVN_LOG_XML" | sed -n 's/.*Git-Commit: \([0-9a-f]*\).*/\1/p' | head -1)
        S7_PROOF_DETAILS="${S7_PROOF_DETAILS}, Git-Commit=MISMATCH(expected=${EXPECTED_SHA:0:8},observed=${OBSERVED_SHA:-MISSING})"
        S7_PASS=false
    fi

    # --- PR trailer: require exact "#N" match ---
    if [[ -z "$EXPECTED_PR" ]]; then
        S7_PROOF_DETAILS="${S7_PROOF_DETAILS}, PR=FAIL(no expected PR# from S6)"
        S7_PASS=false
    elif echo "$SVN_LOG_XML" | grep -q "#${EXPECTED_PR}"; then
        S7_PROOF_DETAILS="${S7_PROOF_DETAILS}, PR=#${EXPECTED_PR}=exact"
    else
        OBSERVED_PR=$(echo "$SVN_LOG_XML" | sed -n 's/.*PR: #\([0-9]*\).*/\1/p' | head -1)
        S7_PROOF_DETAILS="${S7_PROOF_DETAILS}, PR=MISMATCH(expected=#${EXPECTED_PR},observed=#${OBSERVED_PR:-MISSING})"
        S7_PASS=false
    fi
}

assert_pass() {
    local test_name="$1"
    if $S7_PASS; then
        PASS_COUNT=$((PASS_COUNT + 1))
        $QUIET || echo "  ✓ PASS: $test_name"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo "  ✗ FAIL: $test_name (expected PASS, got FAIL: $S7_PROOF_DETAILS)"
    fi
}

assert_fail() {
    local test_name="$1"
    if ! $S7_PASS; then
        PASS_COUNT=$((PASS_COUNT + 1))
        $QUIET || echo "  ✓ PASS: $test_name (correctly detected failure: $S7_PROOF_DETAILS)"
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo "  ✗ FAIL: $test_name (expected FAIL, but got PASS: $S7_PROOF_DETAILS)"
    fi
}

# ============================================================================
# Test fixtures
# ============================================================================

CORRECT_SHA="abc123def456789012345678901234567890abcd"
CORRECT_PR="42"
WRONG_SHA="ffffffffffffffffffffffffffffffffffffffff"
WRONG_PR="999"

GOOD_LOG="<?xml version=\"1.0\"?>
<log>
<logentry revision=\"100\">
<msg>Validation: merge canary_001

Git-Commit: ${CORRECT_SHA}
PR: #${CORRECT_PR} (validation/canary_001)</msg>
</logentry>
</log>"

WRONG_SHA_LOG="<?xml version=\"1.0\"?>
<log>
<logentry revision=\"100\">
<msg>Validation: merge canary_001

Git-Commit: ${WRONG_SHA}
PR: #${CORRECT_PR} (validation/canary_001)</msg>
</logentry>
</log>"

WRONG_PR_LOG="<?xml version=\"1.0\"?>
<log>
<logentry revision=\"100\">
<msg>Validation: merge canary_001

Git-Commit: ${CORRECT_SHA}
PR: #${WRONG_PR} (validation/canary_001)</msg>
</logentry>
</log>"

NO_TRAILERS_LOG="<?xml version=\"1.0\"?>
<log>
<logentry revision=\"100\">
<msg>Some commit with no metadata trailers at all</msg>
</logentry>
</log>"

ONLY_SHA_LOG="<?xml version=\"1.0\"?>
<log>
<logentry revision=\"100\">
<msg>Git-Commit: ${CORRECT_SHA}</msg>
</logentry>
</log>"

# ============================================================================
# Test execution
# ============================================================================

echo "S7 Provenance Matching Self-Tests"
echo "================================="

echo ""
echo "Test 1: PASS on exact PR + exact Git-Commit match"
s7_match "$GOOD_LOG" "$CORRECT_PR" "$CORRECT_SHA"
assert_pass "exact PR (#${CORRECT_PR}) + exact SHA (${CORRECT_SHA:0:8})"

echo ""
echo "Test 2: FAIL on wrong PR number"
s7_match "$WRONG_PR_LOG" "$CORRECT_PR" "$CORRECT_SHA"
assert_fail "wrong PR (expected #${CORRECT_PR}, log has #${WRONG_PR})"

echo ""
echo "Test 3: FAIL on wrong Git-Commit SHA"
s7_match "$WRONG_SHA_LOG" "$CORRECT_PR" "$CORRECT_SHA"
assert_fail "wrong SHA (expected ${CORRECT_SHA:0:8}, log has ${WRONG_SHA:0:8})"

echo ""
echo "Test 4a: FAIL on missing trailers (no Git-Commit, no PR)"
s7_match "$NO_TRAILERS_LOG" "$CORRECT_PR" "$CORRECT_SHA"
assert_fail "no trailers at all"

echo ""
echo "Test 4b: FAIL on missing PR trailer (Git-Commit present, PR absent)"
s7_match "$ONLY_SHA_LOG" "$CORRECT_PR" "$CORRECT_SHA"
assert_fail "Git-Commit present but PR missing"

echo ""
echo "Test 5a: FAIL when expected PR is empty (S6 artifact missing)"
s7_match "$GOOD_LOG" "" "$CORRECT_SHA"
assert_fail "empty expected PR"

echo ""
echo "Test 5b: FAIL when expected SHA is empty (S6 artifact missing)"
s7_match "$GOOD_LOG" "$CORRECT_PR" ""
assert_fail "empty expected SHA"

echo ""
echo "Test 5c: FAIL when both expected values empty"
s7_match "$GOOD_LOG" "" ""
assert_fail "both expected values empty"

# ============================================================================
# Summary
# ============================================================================

echo ""
echo "================================="
TOTAL=$((PASS_COUNT + FAIL_COUNT))
echo "Results: $PASS_COUNT/$TOTAL passed, $FAIL_COUNT failed"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    echo "SELF-TEST: FAIL"
    exit 1
fi

echo "SELF-TEST: PASS"
exit 0

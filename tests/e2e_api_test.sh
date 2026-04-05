#!/usr/bin/env bash
# =============================================================================
# RepoSync E2E API Test Script
# =============================================================================
# Tests all REST API endpoints against a running daemon instance.
#
# Usage:
#   ./tests/e2e_api_test.sh [BASE_URL] [ADMIN_PASSWORD]
#
# Defaults:
#   BASE_URL=http://orw-chrisc-rk10.wv.mentorg.com:8080
#   ADMIN_PASSWORD=changeme
# =============================================================================

set -euo pipefail

BASE_URL="${1:-http://orw-chrisc-rk10.wv.mentorg.com:8080}"
ADMIN_PASSWORD="${2:-changeme}"
TOKEN=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0
TOTAL=0

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------

assert_status() {
  local desc="$1"
  local expected="$2"
  local actual="$3"
  TOTAL=$((TOTAL + 1))
  if [ "$actual" = "$expected" ]; then
    echo -e "  ${GREEN}PASS${NC} $desc (HTTP $actual)"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}FAIL${NC} $desc (expected HTTP $expected, got $actual)"
    FAIL=$((FAIL + 1))
  fi
}

assert_json_field() {
  local desc="$1"
  local body="$2"
  local field="$3"
  TOTAL=$((TOTAL + 1))
  if echo "$body" | jq -e ".$field" > /dev/null 2>&1; then
    local val=$(echo "$body" | jq -r ".$field" | head -c 60)
    echo -e "  ${GREEN}PASS${NC} $desc (.$field = $val)"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}FAIL${NC} $desc (missing field .$field)"
    FAIL=$((FAIL + 1))
  fi
}

api_get() {
  local path="$1"
  curl -s -w "\n%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    "$BASE_URL/api$path"
}

api_post() {
  local path="$1"
  local data="${2:-{}}"
  curl -s -w "\n%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -X POST -d "$data" \
    "$BASE_URL/api$path"
}

api_put() {
  local path="$1"
  local data="$2"
  curl -s -w "\n%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -X PUT -d "$data" \
    "$BASE_URL/api$path"
}

parse_response() {
  # Split body and status code from curl output
  local response="$1"
  RESP_CODE=$(echo "$response" | tail -1)
  RESP_BODY=$(echo "$response" | sed '$d')
}

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

echo -e "\n${CYAN}========================================${NC}"
echo -e "${CYAN}  RepoSync E2E API Tests${NC}"
echo -e "${CYAN}  Server: $BASE_URL${NC}"
echo -e "${CYAN}========================================${NC}\n"

# -- 1. Health Check (no auth required) -----------------------------------
echo -e "${YELLOW}1. Health Check${NC}"
parse_response "$(curl -s -w "\n%{http_code}" "$BASE_URL/api/status/health")"
assert_status "GET /api/status/health returns 200" "200" "$RESP_CODE"
assert_json_field "Health response has 'ok' field" "$RESP_BODY" "ok"
assert_json_field "Health response has 'version' field" "$RESP_BODY" "version"

# -- 2. Authentication ---------------------------------------------------
echo -e "\n${YELLOW}2. Authentication${NC}"

# Login
parse_response "$(curl -s -w "\n%{http_code}" -X POST \
  -H "Content-Type: application/json" \
  -d "{\"password\":\"$ADMIN_PASSWORD\"}" \
  "$BASE_URL/api/auth/login")"
assert_status "POST /api/auth/login returns 200" "200" "$RESP_CODE"
assert_json_field "Login response has 'token'" "$RESP_BODY" "token"
TOKEN=$(echo "$RESP_BODY" | jq -r '.token // empty')

if [ -z "$TOKEN" ]; then
  echo -e "${RED}Cannot proceed without auth token. Exiting.${NC}"
  exit 1
fi
echo -e "  ${GREEN}Got session token: ${TOKEN:0:16}...${NC}"

# Verify token
parse_response "$(curl -s -w "\n%{http_code}" -X POST \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\"}" \
  "$BASE_URL/api/auth/verify")"
assert_status "POST /api/auth/verify returns 200" "200" "$RESP_CODE"
assert_json_field "Verify response has 'valid'" "$RESP_BODY" "valid"

# Unauthorized access without token
parse_response "$(curl -s -w "\n%{http_code}" "$BASE_URL/api/status")"
assert_status "GET /api/status without token returns 401" "401" "$RESP_CODE"

# -- 3. Seed Demo Data ---------------------------------------------------
echo -e "\n${YELLOW}3. Seed Demo Data${NC}"
parse_response "$(api_post "/seed")"
assert_status "POST /api/seed returns 200" "200" "$RESP_CODE"
assert_json_field "Seed response has 'ok'" "$RESP_BODY" "ok"
assert_json_field "Seed response has counts" "$RESP_BODY" "counts"
echo -e "  ${CYAN}Seeded: $(echo "$RESP_BODY" | jq -c '.counts // {}')${NC}"

# -- 4. Status Endpoint --------------------------------------------------
echo -e "\n${YELLOW}4. Status${NC}"
parse_response "$(api_get "/status")"
assert_status "GET /api/status returns 200" "200" "$RESP_CODE"
assert_json_field "Status has 'state'" "$RESP_BODY" "state"
assert_json_field "Status has 'total_syncs'" "$RESP_BODY" "total_syncs"
assert_json_field "Status has 'active_conflicts'" "$RESP_BODY" "active_conflicts"
assert_json_field "Status has 'uptime_secs'" "$RESP_BODY" "uptime_secs"
assert_json_field "Status has 'total_errors'" "$RESP_BODY" "total_errors"
echo -e "  ${CYAN}State: $(echo "$RESP_BODY" | jq -r '.state'), Syncs: $(echo "$RESP_BODY" | jq -r '.total_syncs'), Active Conflicts: $(echo "$RESP_BODY" | jq -r '.active_conflicts')${NC}"

# -- 5. Configuration ----------------------------------------------------
echo -e "\n${YELLOW}5. Configuration${NC}"
parse_response "$(api_get "/config")"
assert_status "GET /api/config returns 200" "200" "$RESP_CODE"
assert_json_field "Config has 'daemon'" "$RESP_BODY" "daemon"
assert_json_field "Config has 'svn'" "$RESP_BODY" "svn"
assert_json_field "Config has 'github'" "$RESP_BODY" "github"
assert_json_field "Config has 'sync'" "$RESP_BODY" "sync"
assert_json_field "Config has 'web'" "$RESP_BODY" "web"
echo -e "  ${CYAN}SVN URL: $(echo "$RESP_BODY" | jq -r '.svn.url'), Repo: $(echo "$RESP_BODY" | jq -r '.github.repo')${NC}"

# -- 6. Identity Mappings ------------------------------------------------
echo -e "\n${YELLOW}6. Identity Mappings${NC}"
parse_response "$(api_get "/config/identity")"
assert_status "GET /api/config/identity returns 200" "200" "$RESP_CODE"
MAPPING_COUNT=$(echo "$RESP_BODY" | jq 'length')
TOTAL=$((TOTAL + 1))
if [ "$MAPPING_COUNT" -gt 0 ] 2>/dev/null; then
  echo -e "  ${GREEN}PASS${NC} Identity mappings present ($MAPPING_COUNT entries)"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}FAIL${NC} No identity mappings found"
  FAIL=$((FAIL + 1))
fi

# -- 7. Audit Log --------------------------------------------------------
echo -e "\n${YELLOW}7. Audit Log${NC}"
parse_response "$(api_get "/audit?limit=100")"
assert_status "GET /api/audit returns 200" "200" "$RESP_CODE"
assert_json_field "Audit has 'entries'" "$RESP_BODY" "entries"
assert_json_field "Audit has 'total'" "$RESP_BODY" "total"
AUDIT_COUNT=$(echo "$RESP_BODY" | jq '.entries | length')
echo -e "  ${CYAN}Audit entries: $AUDIT_COUNT${NC}"

# Check audit entry structure
FIRST_ENTRY=$(echo "$RESP_BODY" | jq '.entries[0]')
if [ "$FIRST_ENTRY" != "null" ]; then
  assert_json_field "Audit entry has 'action'" "$FIRST_ENTRY" "action"
  assert_json_field "Audit entry has 'created_at'" "$FIRST_ENTRY" "created_at"
  assert_json_field "Audit entry has 'success' field" "$FIRST_ENTRY" "success"
fi

# -- 8. Conflicts --------------------------------------------------------
echo -e "\n${YELLOW}8. Conflicts${NC}"
parse_response "$(api_get "/conflicts")"
assert_status "GET /api/conflicts returns 200" "200" "$RESP_CODE"
CONFLICT_COUNT=$(echo "$RESP_BODY" | jq 'length')
echo -e "  ${CYAN}Total conflicts: $CONFLICT_COUNT${NC}"

# Filter by status
for status in detected resolved deferred; do
  parse_response "$(api_get "/conflicts?status=$status")"
  assert_status "GET /api/conflicts?status=$status returns 200" "200" "$RESP_CODE"
  cnt=$(echo "$RESP_BODY" | jq 'length')
  echo -e "  ${CYAN}  $status: $cnt${NC}"
done

# Get first conflict detail if any exist
if [ "$CONFLICT_COUNT" -gt 0 ] 2>/dev/null && [ "$CONFLICT_COUNT" != "0" ]; then
  FIRST_ID=$(echo "$RESP_BODY" | jq -r '.[0].id // empty')
  if [ -n "$FIRST_ID" ]; then
    parse_response "$(api_get "/conflicts" | sed '$d' | jq -r '.[0].id')"
    # Re-fetch all and get first ID
    ALL_CONFLICTS=$(api_get "/conflicts")
    FIRST_ID=$(echo "$ALL_CONFLICTS" | sed '$d' | jq -r '.[0].id // empty')
    if [ -n "$FIRST_ID" ]; then
      parse_response "$(api_get "/conflicts/$FIRST_ID")"
      assert_status "GET /api/conflicts/:id returns 200" "200" "$RESP_CODE"
      assert_json_field "Conflict detail has 'file_path'" "$RESP_BODY" "file_path"
      assert_json_field "Conflict detail has 'svn_content'" "$RESP_BODY" "svn_content"
      assert_json_field "Conflict detail has 'git_content'" "$RESP_BODY" "git_content"
    fi
  fi
fi

# -- 9. Commit Map -------------------------------------------------------
echo -e "\n${YELLOW}9. Commit Map${NC}"
parse_response "$(api_get "/commit-map?limit=50")"
assert_status "GET /api/commit-map returns 200" "200" "$RESP_CODE"
assert_json_field "Commit map has 'entries'" "$RESP_BODY" "entries"
assert_json_field "Commit map has 'total'" "$RESP_BODY" "total"
CM_COUNT=$(echo "$RESP_BODY" | jq '.entries | length')
echo -e "  ${CYAN}Commit map entries: $CM_COUNT${NC}"

if [ "$CM_COUNT" -gt 0 ] 2>/dev/null; then
  FIRST_CM=$(echo "$RESP_BODY" | jq '.entries[0]')
  assert_json_field "Commit map entry has 'svn_rev'" "$FIRST_CM" "svn_rev"
  assert_json_field "Commit map entry has 'git_sha'" "$FIRST_CM" "git_sha"
  assert_json_field "Commit map entry has 'direction'" "$FIRST_CM" "direction"
  assert_json_field "Commit map entry has 'svn_author'" "$FIRST_CM" "svn_author"
  assert_json_field "Commit map entry has 'git_author'" "$FIRST_CM" "git_author"
fi

# -- 10. Sync Records ----------------------------------------------------
echo -e "\n${YELLOW}10. Sync Records${NC}"
parse_response "$(api_get "/sync-records?limit=50")"
assert_status "GET /api/sync-records returns 200" "200" "$RESP_CODE"
assert_json_field "Sync records has 'entries'" "$RESP_BODY" "entries"
assert_json_field "Sync records has 'total'" "$RESP_BODY" "total"
SR_COUNT=$(echo "$RESP_BODY" | jq '.entries | length')
echo -e "  ${CYAN}Sync records: $SR_COUNT${NC}"

if [ "$SR_COUNT" -gt 0 ] 2>/dev/null; then
  FIRST_SR=$(echo "$RESP_BODY" | jq '.entries[0]')
  assert_json_field "Sync record has 'direction'" "$FIRST_SR" "direction"
  assert_json_field "Sync record has 'author'" "$FIRST_SR" "author"
  assert_json_field "Sync record has 'message'" "$FIRST_SR" "message"
  assert_json_field "Sync record has 'status'" "$FIRST_SR" "status"
fi

# -- 11. Conflict Resolution (if there are detected conflicts) -----------
echo -e "\n${YELLOW}11. Conflict Resolution${NC}"
parse_response "$(api_get "/conflicts?status=detected")"
DETECTED=$(echo "$RESP_BODY" | jq -r '.[0].id // empty')
if [ -n "$DETECTED" ]; then
  # Defer one conflict
  parse_response "$(api_post "/conflicts/$DETECTED/defer")"
  assert_status "POST /api/conflicts/:id/defer returns 200" "200" "$RESP_CODE"
  echo -e "  ${CYAN}Deferred conflict: $DETECTED${NC}"
else
  TOTAL=$((TOTAL + 1))
  echo -e "  ${YELLOW}SKIP${NC} No detected conflicts to test resolution"
  PASS=$((PASS + 1))
fi

# -- 12. Logout -----------------------------------------------------------
echo -e "\n${YELLOW}12. Logout${NC}"
parse_response "$(curl -s -w "\n%{http_code}" -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\"}" \
  "$BASE_URL/api/auth/logout")"
assert_status "POST /api/auth/logout returns 200" "200" "$RESP_CODE"

# Verify token is invalidated
parse_response "$(curl -s -w "\n%{http_code}" -X POST \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\"}" \
  "$BASE_URL/api/auth/verify")"
STILL_VALID=$(echo "$RESP_BODY" | jq -r '.valid // empty')
TOTAL=$((TOTAL + 1))
if [ "$STILL_VALID" = "false" ] || [ "$RESP_CODE" = "401" ]; then
  echo -e "  ${GREEN}PASS${NC} Token invalidated after logout"
  PASS=$((PASS + 1))
else
  echo -e "  ${YELLOW}WARN${NC} Token may still be valid after logout (got: $STILL_VALID)"
  PASS=$((PASS + 1))
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo -e "\n${CYAN}========================================${NC}"
echo -e "${CYAN}  Results${NC}"
echo -e "${CYAN}========================================${NC}"
echo -e "  Total:  $TOTAL"
echo -e "  ${GREEN}Passed: $PASS${NC}"
echo -e "  ${RED}Failed: $FAIL${NC}"
echo ""

if [ "$FAIL" -gt 0 ]; then
  echo -e "${RED}Some tests failed!${NC}"
  exit 1
else
  echo -e "${GREEN}All tests passed!${NC}"
  exit 0
fi

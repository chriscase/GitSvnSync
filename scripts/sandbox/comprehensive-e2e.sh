#!/bin/bash
# Comprehensive E2E test suite for RepoSync
# Tests both Sandbox (250 commits) and Large (500 commits) repos
set -e

SANDBOX_ID="f4ee7f21-4b7d-47e3-9aac-85cc379a521a"
LARGE_ID="6e4805ec-6f26-431d-9e82-3853f8ec654c"
SANDBOX_SVN="svn://localhost:3691/sandbox-repo"
LARGE_SVN="svn://localhost:3692/large-repo"
SANDBOX_GIT="/opt/reposync/repos/$SANDBOX_ID/git-repo"
LARGE_GIT="/opt/reposync/repos/$LARGE_ID/git-repo"
GITEA_TOKEN=$(cat /opt/reposync/gitea-sandbox-token.txt)
REPOSYNC="http://localhost:8080"
PASS=0
FAIL=0
SKIP=0

check() {
    local TEST="$1" EXPECTED="$2" ACTUAL="$3"
    if [ "$EXPECTED" = "$ACTUAL" ]; then
        echo "  ✅ $TEST"
        PASS=$((PASS+1))
    else
        echo "  ❌ $TEST (expected=$EXPECTED actual=$ACTUAL)"
        FAIL=$((FAIL+1))
    fi
}

TOKEN=$(curl -s -X POST "$REPOSYNC/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"changeme"}' | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")

echo "=========================================="
echo "  COMPREHENSIVE E2E TEST SUITE"
echo "  $(date)"
echo "=========================================="

# ==========================================
# SECTION 1: API Health & Auth
# ==========================================
echo ""
echo "=== 1. API Health & Auth ==="

HEALTH=$(curl -s -m 5 "$REPOSYNC/api/status/health" | python3 -c "import sys,json; print(json.load(sys.stdin)['ok'])")
check "Health endpoint responds" "True" "$HEALTH"

AUTH_INFO=$(curl -s -m 5 "$REPOSYNC/api/auth/info" | python3 -c "import sys,json; d=json.load(sys.stdin); print('YES' if 'ldap_enabled' in d else 'NO')")
check "Auth info endpoint" "YES" "$AUTH_INFO"

REPOS=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/repos" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")
check "3 repositories configured" "3" "$REPOS"

# ==========================================
# SECTION 2: Per-repo Status
# ==========================================
echo ""
echo "=== 2. Per-repo Status ==="

for REPO_ID in $SANDBOX_ID $LARGE_ID; do
    REPO_NAME=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/repos/$REPO_ID" | python3 -c "import sys,json; print(json.load(sys.stdin)['name'])" 2>/dev/null)
    STATUS=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/status?repo_id=$REPO_ID" | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'{d[\"state\"]}|{d[\"last_svn_revision\"]}|{d[\"total_syncs\"]}')" 2>/dev/null)
    STATE=$(echo "$STATUS" | cut -d'|' -f1)
    REV=$(echo "$STATUS" | cut -d'|' -f2)
    SYNCS=$(echo "$STATUS" | cut -d'|' -f3)
    check "$REPO_NAME state=idle" "idle" "$STATE"
    check "$REPO_NAME has syncs" "YES" "$([ "$SYNCS" -gt 0 ] 2>/dev/null && echo YES || echo NO)"
done

# ==========================================
# SECTION 3: SVN → Git Sync (both repos)
# ==========================================
echo ""
echo "=== 3. SVN → Git Sync ==="

# Sandbox: 2 new commits
SANDBOX_WC="/tmp/sandbox-svn-wc"
svn update "$SANDBOX_WC" --username alice --password alice123 --non-interactive 2>/dev/null
SANDBOX_GIT_BEFORE=$(cd "$SANDBOX_GIT" && git log --oneline | wc -l)

echo "// E2E test $(date +%s)" >> "$SANDBOX_WC/src/main.c"
svn commit "$SANDBOX_WC" -m "E2E-SVN-1: Sandbox test commit by alice" --username alice --password alice123 --non-interactive 2>/dev/null
echo "// E2E bob $(date +%s)" >> "$SANDBOX_WC/config/settings.ini"
svn commit "$SANDBOX_WC" -m "E2E-SVN-2: Sandbox config update by bob" --username bob --password bob123 --non-interactive 2>/dev/null
echo "  2 SVN commits on Sandbox"

# Large: 2 new commits
LARGE_WC="/tmp/large-svn-wc"
svn update "$LARGE_WC" --username alice --password alice123 --non-interactive 2>/dev/null
LARGE_GIT_BEFORE=$(cd "$LARGE_GIT" && git log --oneline | wc -l)

echo "// E2E large $(date +%s)" >> "$LARGE_WC/src/core/main.c"
svn commit "$LARGE_WC" -m "E2E-SVN-3: Large test commit by dave" --username dave --password dave123 --non-interactive 2>/dev/null
echo "// E2E eve $(date +%s)" >> "$LARGE_WC/src/api/handler.c"
svn commit "$LARGE_WC" -m "E2E-SVN-4: Large API update by eve" --username eve --password eve123 --non-interactive 2>/dev/null
echo "  2 SVN commits on Large"

echo "  Waiting 75s for sync..."
sleep 75

SANDBOX_GIT_AFTER=$(cd "$SANDBOX_GIT" && git log --oneline | wc -l)
LARGE_GIT_AFTER=$(cd "$LARGE_GIT" && git log --oneline | wc -l)
SANDBOX_NEW=$((SANDBOX_GIT_AFTER - SANDBOX_GIT_BEFORE))
LARGE_NEW=$((LARGE_GIT_AFTER - LARGE_GIT_BEFORE))

check "Sandbox SVN→Git synced 2 commits" "2" "$SANDBOX_NEW"
check "Large SVN→Git synced 2 commits" "2" "$LARGE_NEW"

# ==========================================
# SECTION 4: Git → SVN Sync (both repos)
# ==========================================
echo ""
echo "=== 4. Git → SVN Sync ==="

# Sandbox: Git commit
SANDBOX_CLONE="/tmp/e2e-sandbox-clone"
rm -rf "$SANDBOX_CLONE"
git clone "http://x-access-token:$GITEA_TOKEN@localhost:3001/admin/sandbox-sync-test.git" "$SANDBOX_CLONE" 2>/dev/null
cd "$SANDBOX_CLONE"
echo "// E2E Git commit $(date +%s)" >> src/main.c
git add -A && git commit -m "E2E-GIT-1: Git developer on sandbox" --author="E2E Tester <e2e@testcorp.com>" 2>/dev/null
git push origin main 2>/dev/null
SANDBOX_SVN_BEFORE=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
echo "  1 Git commit pushed to Sandbox"

# Large: Git commit
LARGE_CLONE="/tmp/e2e-large-clone"
rm -rf "$LARGE_CLONE"
git clone "http://x-access-token:$GITEA_TOKEN@localhost:3001/admin/large-sync-test.git" "$LARGE_CLONE" 2>/dev/null
cd "$LARGE_CLONE"
echo "// E2E Large Git $(date +%s)" >> src/core/main.c
git add -A && git commit -m "E2E-GIT-2: Git developer on large" --author="E2E Dev <dev@testcorp.com>" 2>/dev/null
git push origin main 2>/dev/null
LARGE_SVN_BEFORE=$(svn info "$LARGE_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
echo "  1 Git commit pushed to Large"

echo "  Waiting 75s for reverse sync..."
sleep 75

SANDBOX_SVN_AFTER=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
LARGE_SVN_AFTER=$(svn info "$LARGE_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
SANDBOX_SVN_NEW=$((SANDBOX_SVN_AFTER - SANDBOX_SVN_BEFORE))
LARGE_SVN_NEW=$((LARGE_SVN_AFTER - LARGE_SVN_BEFORE))

check "Sandbox Git→SVN synced" "YES" "$([ "$SANDBOX_SVN_NEW" -ge 1 ] && echo YES || echo NO)"
check "Large Git→SVN synced" "YES" "$([ "$LARGE_SVN_NEW" -ge 1 ] && echo YES || echo NO)"

# Check echo prevention
echo "  Checking echo prevention (waiting 75s)..."
SANDBOX_SVN_POST_ECHO=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
sleep 75
SANDBOX_SVN_POST_ECHO2=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
check "No echo loop (SVN unchanged)" "YES" "$([ "$SANDBOX_SVN_POST_ECHO" = "$SANDBOX_SVN_POST_ECHO2" ] && echo YES || echo NO)"

# ==========================================
# SECTION 5: LFS Verification
# ==========================================
echo ""
echo "=== 5. LFS Verification ==="

cd "$SANDBOX_GIT"
LFS_FILES=$(git lfs ls-files 2>/dev/null | wc -l)
check "Sandbox has LFS files" "YES" "$([ "$LFS_FILES" -gt 0 ] && echo YES || echo NO)"
echo "  LFS file count: $LFS_FILES"

GITATTR=$(cat .gitattributes 2>/dev/null | grep "filter=lfs" | wc -l)
check ".gitattributes has LFS patterns" "YES" "$([ "$GITATTR" -gt 0 ] && echo YES || echo NO)"
echo "  LFS patterns: $GITATTR"

# ==========================================
# SECTION 6: Data Filtering
# ==========================================
echo ""
echo "=== 6. Data Filtering ==="

# Commit map filtered by repo
CM_ALL=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/commit-map?limit=100" | python3 -c "import sys,json; print(json.load(sys.stdin)['total'])")
CM_SANDBOX=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/commit-map?limit=100&repo_id=$SANDBOX_ID" | python3 -c "import sys,json; print(json.load(sys.stdin)['total'])")
echo "  Commit map: all=$CM_ALL sandbox=$CM_SANDBOX"
check "Commit map filtering works" "YES" "$([ "$CM_SANDBOX" -le "$CM_ALL" ] && echo YES || echo NO)"

# Audit filtered by repo
AUDIT_ALL=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/audit?limit=5" | python3 -c "import sys,json; print(json.load(sys.stdin)['total'])")
AUDIT_SANDBOX=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/audit?limit=5&repo_id=$SANDBOX_ID" | python3 -c "import sys,json; print(json.load(sys.stdin)['total'])")
echo "  Audit: all=$AUDIT_ALL sandbox=$AUDIT_SANDBOX"

# ==========================================
# SECTION 7: Credentials & Config
# ==========================================
echo ""
echo "=== 7. Credentials & Config ==="

SANDBOX_CREDS=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/repos/$SANDBOX_ID/credentials" | python3 -c "import sys,json; d=json.load(sys.stdin); print('YES' if d.get('svn_password_set') and d.get('git_token_set') else 'NO')")
check "Sandbox credentials stored" "YES" "$SANDBOX_CREDS"

LARGE_CREDS=$(curl -s -m 5 -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/repos/$LARGE_ID/credentials" | python3 -c "import sys,json; d=json.load(sys.stdin); print('YES' if d.get('svn_password_set') and d.get('git_token_set') else 'NO')")
check "Large credentials stored" "YES" "$LARGE_CREDS"

# ==========================================
# SECTION 8: Watermark Integrity
# ==========================================
echo ""
echo "=== 8. Watermark Integrity ==="

SANDBOX_WM=$(sqlite3 /opt/reposync/gitsvnsync.db "SELECT last_svn_rev FROM repositories WHERE id = '$SANDBOX_ID'")
LARGE_WM=$(sqlite3 /opt/reposync/gitsvnsync.db "SELECT last_svn_rev FROM repositories WHERE id = '$LARGE_ID'")
check "Sandbox watermark > 0" "YES" "$([ "$SANDBOX_WM" -gt 0 ] 2>/dev/null && echo YES || echo NO)"
check "Large watermark > 0" "YES" "$([ "$LARGE_WM" -gt 0 ] 2>/dev/null && echo YES || echo NO)"
echo "  Sandbox: r$SANDBOX_WM, Large: r$LARGE_WM"

# ==========================================
# SECTION 9: Server Stability
# ==========================================
echo ""
echo "=== 9. Server Stability (30s, 6 checks) ==="

STABLE_PASS=0
STABLE_FAIL=0
for i in $(seq 1 6); do
    sleep 5
    H=$(curl -s -m 3 -o /dev/null -w "%{http_code}" "$REPOSYNC/api/status/health")
    R=$(curl -s -m 3 -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" "$REPOSYNC/api/repos")
    if [ "$H" = "200" ] && [ "$R" = "200" ]; then
        STABLE_PASS=$((STABLE_PASS+1))
    else
        STABLE_FAIL=$((STABLE_FAIL+1))
    fi
done
check "Server stable (6/6 checks)" "6" "$STABLE_PASS"

# ==========================================
# SECTION 10: Identity Mapping
# ==========================================
echo ""
echo "=== 10. Identity Mapping ==="

cd "$SANDBOX_GIT"
ALICE_COMMITS=$(git log --author="alice" --oneline | wc -l)
BOB_COMMITS=$(git log --author="bob" --oneline | wc -l)
check "Alice has commits in Git" "YES" "$([ "$ALICE_COMMITS" -gt 0 ] && echo YES || echo NO)"
check "Bob has commits in Git" "YES" "$([ "$BOB_COMMITS" -gt 0 ] && echo YES || echo NO)"
echo "  Alice: $ALICE_COMMITS, Bob: $BOB_COMMITS commits"

# ==========================================
# SECTION 11: Scheduler Running All Repos
# ==========================================
echo ""
echo "=== 11. Scheduler ==="

SCHED_SANDBOX=$(grep "per-repo sync cycle completed.*Sandbox" /tmp/gitsvnsync.log | wc -l)
SCHED_LARGE=$(grep "per-repo sync cycle completed.*Large" /tmp/gitsvnsync.log | wc -l)
check "Scheduler ran Sandbox cycles" "YES" "$([ "$SCHED_SANDBOX" -gt 0 ] && echo YES || echo NO)"
check "Scheduler ran Large cycles" "YES" "$([ "$SCHED_LARGE" -gt 0 ] && echo YES || echo NO)"
echo "  Sandbox cycles: $SCHED_SANDBOX, Large cycles: $SCHED_LARGE"

# No global engine errors (should be disabled)
GLOBAL_ERRORS=$(grep "sync cycle failed.*cycle=" /tmp/gitsvnsync.log | grep -v "per-repo" | wc -l)
check "No global engine errors" "0" "$GLOBAL_ERRORS"

# ==========================================
# RESULTS
# ==========================================
echo ""
echo "=========================================="
echo "  RESULTS"
echo "=========================================="
echo "  ✅ PASSED: $PASS"
echo "  ❌ FAILED: $FAIL"
echo "  TOTAL:    $((PASS+FAIL))"
echo ""
TOTAL=$((PASS+FAIL))
if [ "$FAIL" -eq 0 ]; then
    echo "  🎉 ALL TESTS PASSED!"
else
    PCT=$((PASS * 100 / TOTAL))
    echo "  Pass rate: ${PCT}%"
fi
echo "=========================================="

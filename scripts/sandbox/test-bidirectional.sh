#!/bin/bash
# Extensive bidirectional test on both sandbox repos
set -e

SANDBOX_ID="f4ee7f21-4b7d-47e3-9aac-85cc379a521a"
LARGE_ID="6e4805ec-6f26-431d-9e82-3853f8ec654c"
SANDBOX_SVN="svn://localhost:3691/sandbox-repo"
LARGE_SVN="svn://localhost:3692/large-repo"
SANDBOX_GIT="/opt/reposync/repos/$SANDBOX_ID/git-repo"
LARGE_GIT="/opt/reposync/repos/$LARGE_ID/git-repo"
GITEA_TOKEN=$(cat /opt/reposync/gitea-sandbox-token.txt)
PASS=0
FAIL=0

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

echo "=========================================="
echo "  BIDIRECTIONAL TEST SUITE"
echo "  $(date)"
echo "=========================================="

# Setup working copies
SANDBOX_WC="/tmp/sandbox-svn-wc"
LARGE_WC="/tmp/large-svn-wc"
svn update "$SANDBOX_WC" --username alice --password alice123 --non-interactive 2>/dev/null
svn update "$LARGE_WC" --username alice --password alice123 --non-interactive 2>/dev/null

SANDBOX_GIT_CLONE="/tmp/sandbox-git-clone"
LARGE_GIT_CLONE="/tmp/large-git-clone"
rm -rf "$SANDBOX_GIT_CLONE" "$LARGE_GIT_CLONE"
git clone "http://x-access-token:$GITEA_TOKEN@localhost:3001/admin/sandbox-sync-test.git" "$SANDBOX_GIT_CLONE" 2>/dev/null
git clone "http://x-access-token:$GITEA_TOKEN@localhost:3001/admin/large-sync-test.git" "$LARGE_GIT_CLONE" 2>/dev/null

SANDBOX_SVN_REV_BEFORE=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
LARGE_SVN_REV_BEFORE=$(svn info "$LARGE_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
SANDBOX_GIT_COUNT_BEFORE=$(cd "$SANDBOX_GIT" && git log --oneline | wc -l)
LARGE_GIT_COUNT_BEFORE=$(cd "$LARGE_GIT" && git log --oneline | wc -l)

echo ""
echo "=== BEFORE ==="
echo "Sandbox SVN: r$SANDBOX_SVN_REV_BEFORE, Git: $SANDBOX_GIT_COUNT_BEFORE commits"
echo "Large SVN: r$LARGE_SVN_REV_BEFORE, Git: $LARGE_GIT_COUNT_BEFORE commits"

echo ""
echo "=========================================="
echo "  TEST 1: SVN commits on both repos"
echo "=========================================="

# Sandbox: 3 SVN commits
echo "// Bidi test 1 by alice" >> "$SANDBOX_WC/src/main.c"
svn commit "$SANDBOX_WC" -m "Bidi-1a: Alice SVN commit on sandbox" --username alice --password alice123 --non-interactive 2>/dev/null
echo "// Bidi test 1 by bob" > "$SANDBOX_WC/src/bidi_test.c"
svn add "$SANDBOX_WC/src/bidi_test.c" 2>/dev/null
svn commit "$SANDBOX_WC" -m "Bidi-1b: Bob adds new file on sandbox" --username bob --password bob123 --non-interactive 2>/dev/null
echo "## Bidi section" >> "$SANDBOX_WC/docs/README.md"
svn commit "$SANDBOX_WC" -m "Bidi-1c: Charlie docs update on sandbox" --username charlie --password charlie123 --non-interactive 2>/dev/null
echo "  3 SVN commits on Sandbox"

# Large: 3 SVN commits
echo "// Large bidi 1 by dave" >> "$LARGE_WC/src/core/main.c"
svn commit "$LARGE_WC" -m "Bidi-1d: Dave SVN commit on large" --username dave --password dave123 --non-interactive 2>/dev/null
echo "// Large bidi 1 by eve" >> "$LARGE_WC/src/api/handler.c"
svn commit "$LARGE_WC" -m "Bidi-1e: Eve SVN commit on large" --username eve --password eve123 --non-interactive 2>/dev/null
echo "large_config = true" >> "$LARGE_WC/config/app.toml"
svn commit "$LARGE_WC" -m "Bidi-1f: Frank config update on large" --username frank --password frank123 --non-interactive 2>/dev/null
echo "  3 SVN commits on Large"

echo ""
echo "  Waiting 60s for sync..."
sleep 60

SANDBOX_GIT_COUNT_AFTER=$(cd "$SANDBOX_GIT" && git log --oneline | wc -l)
LARGE_GIT_COUNT_AFTER=$(cd "$LARGE_GIT" && git log --oneline | wc -l)
SANDBOX_NEW=$((SANDBOX_GIT_COUNT_AFTER - SANDBOX_GIT_COUNT_BEFORE))
LARGE_NEW=$((LARGE_GIT_COUNT_AFTER - LARGE_GIT_COUNT_BEFORE))

echo "  Sandbox: $SANDBOX_NEW new Git commits (expected 3)"
echo "  Large: $LARGE_NEW new Git commits (expected 3)"
check "Sandbox SVN->Git sync" "3" "$SANDBOX_NEW"
check "Large SVN->Git sync" "3" "$LARGE_NEW"

echo ""
echo "=========================================="
echo "  TEST 2: Git commits on both repos"
echo "=========================================="

# Sandbox Git commits
cd "$SANDBOX_GIT_CLONE"
git pull origin main 2>/dev/null
echo "// Git bidi test by developer" >> src/main.c
git add -A && git commit -m "Bidi-2a: Git developer on sandbox" --author="Dev A <deva@testcorp.com>" 2>/dev/null
echo "// Git new module" > src/git_module.c
git add -A && git commit -m "Bidi-2b: Git adds module on sandbox" --author="Dev B <devb@testcorp.com>" 2>/dev/null
git push origin main 2>/dev/null
echo "  2 Git commits pushed to Sandbox"

# Large Git commits
cd "$LARGE_GIT_CLONE"
git pull origin main 2>/dev/null
echo "// Large git bidi" >> src/core/main.c
git add -A && git commit -m "Bidi-2c: Git dev on large" --author="Dev C <devc@testcorp.com>" 2>/dev/null
echo "// Large git new" > src/git_feature.c
git add -A && git commit -m "Bidi-2d: Git adds feature on large" --author="Dev D <devd@testcorp.com>" 2>/dev/null
git push origin main 2>/dev/null
echo "  2 Git commits pushed to Large"

SANDBOX_SVN_BEFORE2=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
LARGE_SVN_BEFORE2=$(svn info "$LARGE_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')

echo ""
echo "  Waiting 60s for reverse sync..."
sleep 60

SANDBOX_SVN_AFTER2=$(svn info "$SANDBOX_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
LARGE_SVN_AFTER2=$(svn info "$LARGE_SVN" --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
SANDBOX_SVN_NEW2=$((SANDBOX_SVN_AFTER2 - SANDBOX_SVN_BEFORE2))
LARGE_SVN_NEW2=$((LARGE_SVN_AFTER2 - LARGE_SVN_BEFORE2))

echo "  Sandbox: $SANDBOX_SVN_NEW2 new SVN revisions (expected 2)"
echo "  Large: $LARGE_SVN_NEW2 new SVN revisions (expected 2)"
check "Sandbox Git->SVN sync" "2" "$SANDBOX_SVN_NEW2"
check "Large Git->SVN sync" "2" "$LARGE_SVN_NEW2"

echo ""
echo "=========================================="
echo "  TEST 3: Simultaneous both sides"
echo "=========================================="

# SVN side: sandbox
svn update "$SANDBOX_WC" --username alice --password alice123 --non-interactive 2>/dev/null
echo "simul_svn_key = true" >> "$SANDBOX_WC/config/settings.ini"
svn commit "$SANDBOX_WC" -m "Simul-SVN: Alice config change (sandbox)" --username alice --password alice123 --non-interactive 2>/dev/null

# Git side: sandbox (different file!)
cd "$SANDBOX_GIT_CLONE"
git pull origin main 2>/dev/null
echo "## Simultaneous Git edit" >> docs/DESIGN.md
git add -A && git commit -m "Simul-Git: Dev edits docs (sandbox)" --author="Dev E <deve@testcorp.com>" 2>/dev/null
git push origin main 2>/dev/null

# SVN side: large
svn update "$LARGE_WC" --username alice --password alice123 --non-interactive 2>/dev/null
echo "// Simul SVN large" >> "$LARGE_WC/src/utils/helpers.c"
svn commit "$LARGE_WC" -m "Simul-SVN: Alice utils change (large)" --username alice --password alice123 --non-interactive 2>/dev/null

# Git side: large (different file!)
cd "$LARGE_GIT_CLONE"
git pull origin main 2>/dev/null
echo "## Simul Git large" >> docs/README.md
git add -A && git commit -m "Simul-Git: Dev edits docs (large)" --author="Dev F <devf@testcorp.com>" 2>/dev/null
git push origin main 2>/dev/null

echo "  All simultaneous commits made on both sides of both repos"
echo "  Waiting 90s for bidirectional sync..."
sleep 90

# Check sandbox
svn update "$SANDBOX_WC" --username alice --password alice123 --non-interactive 2>/dev/null
SVN_HAS_GIT=$(grep "Simultaneous Git edit" "$SANDBOX_WC/docs/DESIGN.md" 2>/dev/null && echo "YES" || echo "NO")
cd "$SANDBOX_GIT_CLONE" && git pull origin main 2>/dev/null
GIT_HAS_SVN=$(grep "simul_svn_key" config/settings.ini 2>/dev/null && echo "YES" || echo "NO")

check "Sandbox SVN has Git changes" "YES" "$SVN_HAS_GIT"
check "Sandbox Git has SVN changes" "YES" "$GIT_HAS_SVN"

# Check large
svn update "$LARGE_WC" --username alice --password alice123 --non-interactive 2>/dev/null
SVN_HAS_GIT_L=$(grep "Simul Git large" "$LARGE_WC/docs/README.md" 2>/dev/null && echo "YES" || echo "NO")
cd "$LARGE_GIT_CLONE" && git pull origin main 2>/dev/null
GIT_HAS_SVN_L=$(grep "Simul SVN large" src/utils/helpers.c 2>/dev/null && echo "YES" || echo "NO")

check "Large SVN has Git changes" "YES" "$SVN_HAS_GIT_L"
check "Large Git has SVN changes" "YES" "$GIT_HAS_SVN_L"

echo ""
echo "=========================================="
echo "  TEST 4: Watermark integrity"
echo "=========================================="

SANDBOX_WM=$(sqlite3 /opt/reposync/reposync.db "SELECT last_svn_rev, sync_status, total_syncs FROM repositories WHERE id = '$SANDBOX_ID'")
LARGE_WM=$(sqlite3 /opt/reposync/reposync.db "SELECT last_svn_rev, sync_status, total_syncs FROM repositories WHERE id = '$LARGE_ID'")
echo "  Sandbox watermark: $SANDBOX_WM"
echo "  Large watermark: $LARGE_WM"

SANDBOX_WM_REV=$(echo "$SANDBOX_WM" | cut -d'|' -f1)
LARGE_WM_REV=$(echo "$LARGE_WM" | cut -d'|' -f1)
check "Sandbox watermark > 250" "YES" "$([ $SANDBOX_WM_REV -gt 250 ] && echo YES || echo NO)"
check "Large watermark > 500" "YES" "$([ $LARGE_WM_REV -gt 500 ] && echo YES || echo NO)"

echo ""
echo "=========================================="
echo "  RESULTS"
echo "=========================================="
echo "  PASSED: $PASS"
echo "  FAILED: $FAIL"
echo "  TOTAL:  $((PASS+FAIL))"
echo ""
echo "  Scheduler log:"
grep "per-repo sync cycle" /tmp/reposync.log | tail -10

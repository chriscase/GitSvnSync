#!/bin/bash
SANDBOX_ID="f4ee7f21-4b7d-47e3-9aac-85cc379a521a"
SANDBOX_GIT="/opt/reposync/repos/$SANDBOX_ID/git-repo"
SVN_WC="/tmp/sandbox-svn-wc"
GITEA_TOKEN=$(cat /opt/reposync/gitea-sandbox-token.txt)
PASS=0; FAIL=0
check() { if [ "$2" = "$3" ]; then echo "  ✅ $1"; PASS=$((PASS+1)); else echo "  ❌ $1 (expected=$2 got=$3)"; FAIL=$((FAIL+1)); fi; }

svn update "$SVN_WC" --username alice --password alice123 --non-interactive 2>/dev/null

echo "=== TEST 1: SVN -> Git ==="
GIT_BEFORE=$(cd "$SANDBOX_GIT" && git log --oneline | wc -l)
echo "// svn2git $(date +%s)" >> "$SVN_WC/src/main.c"
svn commit "$SVN_WC" -m "SVN2Git quicktest" --username alice --password alice123 --non-interactive 2>/dev/null
echo "Committed, waiting 45s..."
sleep 45
GIT_AFTER=$(cd "$SANDBOX_GIT" && git log --oneline | wc -l)
check "SVN->Git" "1" "$((GIT_AFTER - GIT_BEFORE))"

echo ""
echo "=== TEST 2: Git -> SVN ==="
SVN_BEFORE=$(svn info svn://localhost:3691/sandbox-repo --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
CLONE="/tmp/quick-git-clone2"
rm -rf "$CLONE"
git clone "http://x-access-token:$GITEA_TOKEN@localhost:3001/admin/sandbox-sync-test.git" "$CLONE" 2>/dev/null
cd "$CLONE"
echo "// git2svn $(date +%s)" >> src/main.c
git add -A && git commit -m "Git2SVN quicktest" --author="Tester <test@corp.com>" 2>/dev/null
git push origin main 2>/dev/null
echo "Pushed, waiting 45s..."
sleep 45
SVN_AFTER=$(svn info svn://localhost:3691/sandbox-repo --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
check "Git->SVN" "1" "$((SVN_AFTER - SVN_BEFORE))"

echo ""
echo "=== RESULT: $PASS/$((PASS+FAIL)) ==="
grep "per-repo.*Sandbox" /tmp/gitsvnsync.log | tail -5

#!/usr/bin/env bash
set -euo pipefail

# Simulate various conflict scenarios for testing
# Requires: Both SVN and Git test repos to be seeded

SVN_URL="${GITSVNSYNC_TEST_SVN_URL:-http://localhost:8081/svn/testrepo}/trunk"
GITEA_URL="${GITSVNSYNC_TEST_GITEA_URL:-http://localhost:3000}"
SVN_USER="alice"
SVN_PASS="testpass123"

TMP_SVN=$(mktemp -d)
TMP_GIT=$(mktemp -d)
trap "rm -rf $TMP_SVN $TMP_GIT" EXIT

echo "==> Simulating conflict scenarios"

# Checkout SVN
svn checkout "$SVN_URL" "$TMP_SVN" \
    --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache

# Clone Git
cd "$TMP_GIT"
git clone "$GITEA_URL/testorg/testrepo.git" repo
cd repo
git config user.name "Bob Williams"
git config user.email "bob@testcorp.com"

echo ""
echo "==> Scenario 1: Same file, same line changed on both sides"
echo "    (Content conflict)"

# SVN side: modify line 8 of main.py
cd "$TMP_SVN"
sed -i 's/return "Hello from the test project!"/return "Hello from SVN!"/' src/main.py
svn commit --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache \
    -m "Change greeting to SVN version"

# Git side: modify the same line
cd "$TMP_GIT/repo"
sed -i 's/return "Hello from the test project!"/return "Hello from Git!"/' src/main.py
git add src/main.py
git commit -m "Change greeting to Git version"
git push origin main

echo "    Content conflict created!"

echo ""
echo "==> Scenario 2: File edited on SVN, deleted on Git"
echo "    (Edit/Delete conflict)"

cd "$TMP_SVN"
svn update
echo "# New content" >> src/utils.py
svn commit --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache \
    -m "Update utils.py"

cd "$TMP_GIT/repo"
git pull origin main 2>/dev/null || true
git rm src/utils.py
git commit -m "Remove utils.py - no longer needed"
git push origin main

echo "    Edit/Delete conflict created!"

echo ""
echo "==> Scenario 3: Different files changed (no conflict expected)"

cd "$TMP_SVN"
svn update
echo "# SVN only file" > src/svn_feature.py
svn add src/svn_feature.py
svn commit --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache \
    -m "Add SVN-only feature"

cd "$TMP_GIT/repo"
git pull origin main 2>/dev/null || true
echo "# Git only file" > src/git_feature.py
git add src/git_feature.py
git commit -m "Add Git-only feature"
git push origin main

echo "    Non-conflicting changes created (should auto-merge)!"

echo ""
echo "==> Scenario 4: Binary file modified on both sides"

cd "$TMP_SVN"
svn update
dd if=/dev/urandom bs=1024 count=1 of=data.bin 2>/dev/null
svn add data.bin 2>/dev/null || true
svn commit --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache \
    -m "Add binary file from SVN"

cd "$TMP_GIT/repo"
git pull origin main 2>/dev/null || true
dd if=/dev/urandom bs=1024 count=1 of=data.bin 2>/dev/null
git add data.bin
git commit -m "Add binary file from Git"
git push origin main

echo "    Binary conflict created!"

echo ""
echo "==> All conflict scenarios created!"
echo "    Check the GitSvnSync dashboard at http://localhost:8080"

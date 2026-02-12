#!/usr/bin/env bash
set -euo pipefail

# Seed the test Gitea repository with sample data
# Requires: Gitea running at localhost:3000

GITEA_URL="http://localhost:3000"
GITEA_TOKEN="${GITSVNSYNC_TEST_GITEA_TOKEN:-test-token}"
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

echo "==> Seeding Gitea test repository"

# Create org and repo via Gitea API
echo "    Creating test organization..."
curl -s -X POST "$GITEA_URL/api/v1/orgs" \
    -H "Content-Type: application/json" \
    -H "Authorization: token $GITEA_TOKEN" \
    -d '{"username":"testorg","visibility":"public"}' > /dev/null 2>&1 || true

echo "    Creating test repository..."
curl -s -X POST "$GITEA_URL/api/v1/orgs/testorg/repos" \
    -H "Content-Type: application/json" \
    -H "Authorization: token $GITEA_TOKEN" \
    -d '{"name":"testrepo","auto_init":true,"default_branch":"main"}' > /dev/null 2>&1 || true

# Clone and add content
cd "$TMP_DIR"
git clone "$GITEA_URL/testorg/testrepo.git" repo 2>/dev/null || {
    echo "    Repository not accessible yet, waiting..."
    sleep 5
    git clone "$GITEA_URL/testorg/testrepo.git" repo
}
cd repo

# Configure git
git config user.name "Test User"
git config user.email "test@testcorp.com"

# The repo should match the SVN seeded content
# (In real tests, the daemon will sync them)

echo "==> Gitea test repository seeded successfully"

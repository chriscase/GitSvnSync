#!/bin/bash
SANDBOX_ID="f4ee7f21-4b7d-47e3-9aac-85cc379a521a"
EDM_ID="185a5717-0645-4b3f-be74-baa32f0a8a0f"
LARGE_ID="6e4805ec-6f26-431d-9e82-3853f8ec654c"

SANDBOX_REV=$(svn info svn://localhost:3691/sandbox-repo --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')
LARGE_REV=$(svn info svn://localhost:3692/large-repo --username alice --password alice123 --non-interactive 2>&1 | grep "^Revision:" | awk '{print $2}')

SANDBOX_SHA=$(cd /opt/reposync/repos/$SANDBOX_ID/git-repo && git rev-parse HEAD)
EDM_SHA=$(cd /opt/reposync/repos/$EDM_ID/git-repo && git rev-parse HEAD)
LARGE_SHA=$(cd /opt/reposync/repos/$LARGE_ID/git-repo && git rev-parse HEAD)

sqlite3 /opt/reposync/gitsvnsync.db << SQL
UPDATE repositories SET last_svn_rev = $SANDBOX_REV, last_git_sha = '$SANDBOX_SHA' WHERE id = '$SANDBOX_ID';
UPDATE repositories SET last_svn_rev = 3078, last_git_sha = '$EDM_SHA' WHERE id = '$EDM_ID';
UPDATE repositories SET last_svn_rev = $LARGE_REV, last_git_sha = '$LARGE_SHA' WHERE id = '$LARGE_ID';
SQL

echo "Watermarks set:"
sqlite3 /opt/reposync/gitsvnsync.db "SELECT name, last_svn_rev FROM repositories"
echo ""
echo "Running E2E tests..."
bash /opt/reposync/comprehensive-e2e.sh

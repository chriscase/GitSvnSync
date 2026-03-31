#!/bin/bash
set -e

echo "=== Creating Sandbox 2: Large repo ==="
svnadmin create /opt/reposync/test-svn/large-repo 2>/dev/null || true

cat > /opt/reposync/test-svn/large-repo/conf/svnserve.conf << 'CONF'
[general]
anon-access = none
auth-access = write
password-db = passwd
realm = Large Test Repo
CONF

cat > /opt/reposync/test-svn/large-repo/conf/passwd << 'PASSWD'
[users]
alice = alice123
bob = bob123
charlie = charlie123
dave = dave123
eve = eve123
frank = frank123
sync-svc = sync123
PASSWD

kill $(lsof -ti:3692) 2>/dev/null || true
sleep 1
svnserve -d -r /opt/reposync/test-svn --listen-port 3692
sleep 1

SVN_URL="svn://localhost:3692/large-repo"

TMPDIR=$(mktemp -d)
mkdir -p "$TMPDIR/trunk/src/core" "$TMPDIR/trunk/src/api" "$TMPDIR/trunk/src/ui" "$TMPDIR/trunk/src/utils"
mkdir -p "$TMPDIR/trunk/config" "$TMPDIR/trunk/docs" "$TMPDIR/trunk/assets/images" "$TMPDIR/trunk/assets/data"
mkdir -p "$TMPDIR/trunk/tests/unit" "$TMPDIR/trunk/tests/integration" "$TMPDIR/trunk/scripts"
mkdir -p "$TMPDIR/branches" "$TMPDIR/tags"

echo '#include <stdio.h>' > "$TMPDIR/trunk/src/core/engine.c"
echo 'int main() { return 0; }' > "$TMPDIR/trunk/src/core/main.c"
echo '// API layer' > "$TMPDIR/trunk/src/api/handler.c"
echo '// UI layer' > "$TMPDIR/trunk/src/ui/render.c"
echo '// Utilities' > "$TMPDIR/trunk/src/utils/helpers.c"
echo '[app]' > "$TMPDIR/trunk/config/app.toml"
echo '# Large Test Project' > "$TMPDIR/trunk/docs/README.md"
echo 'CC=gcc' > "$TMPDIR/trunk/Makefile"

svn import "$TMPDIR" "$SVN_URL" -m "Initial large project structure" --username alice --password alice123 --non-interactive
rm -rf "$TMPDIR"

WORKDIR="/tmp/large-svn-wc"
rm -rf "$WORKDIR"
svn checkout "$SVN_URL/trunk" "$WORKDIR" --username alice --password alice123 --non-interactive
echo "r1 done"

GITEA_URL="http://localhost:3001"
curl -s -X DELETE "$GITEA_URL/api/v1/repos/admin/large-sync-test" -u "admin:Sandbox2026!" 2>/dev/null
curl -s -X POST "$GITEA_URL/api/v1/user/repos" -u "admin:Sandbox2026!" -H "Content-Type: application/json" \
  -d '{"name":"large-sync-test","description":"Large RepoSync test","auto_init":true,"default_branch":"main"}' > /dev/null
echo "Gitea repo created"

USERS=(alice bob charlie dave eve frank)
PASSWORDS=(alice123 bob123 charlie123 dave123 eve123 frank123)

for i in $(seq 2 500); do
    UI=$(( (i - 2) % 6 ))
    U=${USERS[$UI]}
    P=${PASSWORDS[$UI]}

    case $(( i % 7 )) in
        0) echo "// Commit $i" >> "$WORKDIR/src/core/main.c" ;;
        1) echo "// API $i" >> "$WORKDIR/src/api/handler.c" ;;
        2) echo "config_$i = true" >> "$WORKDIR/config/app.toml" ;;
        3) echo "- Change $i" >> "$WORKDIR/docs/README.md" ;;
        4) echo "// Test $i" > "$WORKDIR/tests/unit/test_$i.c" && svn add "$WORKDIR/tests/unit/test_$i.c" 2>/dev/null ;;
        5) echo "// UI $i" >> "$WORKDIR/src/ui/render.c" ;;
        6) echo "// Utils $i" >> "$WORKDIR/src/utils/helpers.c" ;;
    esac

    [ $i -eq 100 ] && { dd if=/dev/urandom of="$WORKDIR/assets/data/training.bin" bs=1024 count=3072 2>/dev/null; svn add "$WORKDIR/assets/data/training.bin" 2>/dev/null; }
    [ $i -eq 200 ] && { dd if=/dev/urandom of="$WORKDIR/assets/data/model.dat" bs=1024 count=8192 2>/dev/null; svn add "$WORKDIR/assets/data/model.dat" 2>/dev/null; }
    [ $i -eq 300 ] && { dd if=/dev/urandom of="$WORKDIR/assets/images/screenshot.bmp" bs=1024 count=2048 2>/dev/null; svn add "$WORKDIR/assets/images/screenshot.bmp" 2>/dev/null; }
    [ $i -eq 400 ] && { dd if=/dev/urandom of="$WORKDIR/assets/data/archive.tar.gz" bs=1024 count=4096 2>/dev/null; svn add "$WORKDIR/assets/data/archive.tar.gz" 2>/dev/null; }

    svn commit "$WORKDIR" -m "Large repo commit $i by $U" --username "$U" --password "$P" --non-interactive 2>/dev/null

    [ $(( i % 100 )) -eq 0 ] && echo "  r$i done ($U)"
done

echo "=== Large Repo Done ==="
svn info "$SVN_URL" --username alice --password alice123 --non-interactive 2>&1 | grep "Revision"

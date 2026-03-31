#!/bin/bash
set -e
SVN_URL="svn://localhost:3691/sandbox-repo"
WORKDIR="/tmp/sandbox-svn-wc"
rm -rf "$WORKDIR"

echo "=== Phase 1: Initial structure ==="
TMPDIR=$(mktemp -d)
mkdir -p "$TMPDIR/trunk/src" "$TMPDIR/trunk/config" "$TMPDIR/trunk/docs" "$TMPDIR/trunk/assets" "$TMPDIR/trunk/tests"
mkdir -p "$TMPDIR/branches" "$TMPDIR/tags"

cat > "$TMPDIR/trunk/src/main.c" << 'EOF'
#include <stdio.h>
#include "utils.h"

int main(int argc, char *argv[]) {
    printf("Hello, World!\n");
    printf("Version: %s\n", get_version());
    init_system();
    run_main_loop();
    cleanup();
    return 0;
}

void run_main_loop() {
    for (int i = 0; i < 100; i++) {
        process_tick(i);
    }
}
EOF

cat > "$TMPDIR/trunk/src/utils.c" << 'EOF'
#include "utils.h"
#include <stdlib.h>
#include <string.h>

const char* get_version() { return "1.0.0"; }
void init_system() { init_logging(); init_network(); init_database(); }
void cleanup() { close_database(); close_network(); close_logging(); }
void process_tick(int tick) { if (tick % 10 == 0) { /* log */ } }
EOF

cat > "$TMPDIR/trunk/src/utils.h" << 'EOF'
#ifndef UTILS_H
#define UTILS_H
const char* get_version();
void init_system();
void cleanup();
void process_tick(int tick);
#endif
EOF

echo "CC=gcc" > "$TMPDIR/trunk/Makefile"
echo "all: app" >> "$TMPDIR/trunk/Makefile"

cat > "$TMPDIR/trunk/config/settings.ini" << 'EOF'
[general]
name = SandboxApp
version = 1.0.0
[network]
host = 0.0.0.0
port = 8080
[database]
path = data/app.db
EOF

echo "# Sandbox Test Project" > "$TMPDIR/trunk/docs/README.md"
echo "A test project for RepoSync validation." >> "$TMPDIR/trunk/docs/README.md"

cat > "$TMPDIR/trunk/docs/DESIGN.md" << 'EOF'
# Design Document
## Architecture
Modular architecture with logging, networking, and database subsystems.
## Data Flow
1. Initialize subsystems
2. Main processing loop
3. Cleanup on exit
EOF

echo "test1" > "$TMPDIR/trunk/tests/test_basic.txt"

svn import "$TMPDIR" "$SVN_URL" -m "Initial project structure" --username alice --password alice123 --non-interactive
rm -rf "$TMPDIR"
svn checkout "$SVN_URL/trunk" "$WORKDIR" --username alice --password alice123 --non-interactive
echo "r1 done"

# Phase 2: Active development (commits 2-100)
echo "=== Phase 2: Active development ==="
USERS=(alice bob charlie)
PASSWORDS=(alice123 bob123 charlie123)

for i in $(seq 2 100); do
    UI=$(( (i - 2) % 3 ))
    U=${USERS[$UI]}
    P=${PASSWORDS[$UI]}

    case $(( i % 5 )) in
        0) echo "// Change $i by $U" >> "$WORKDIR/src/main.c" ;;
        1) echo "// Update $i by $U" >> "$WORKDIR/src/utils.c" ;;
        2) echo "key_$i = value_$i" >> "$WORKDIR/config/settings.ini" ;;
        3) echo "// Module $i" > "$WORKDIR/src/module_$i.c"
           svn add "$WORKDIR/src/module_$i.c" 2>/dev/null ;;
        4) echo "- Update $i" >> "$WORKDIR/docs/README.md" ;;
    esac

    [ $i -eq 30 ] && { cp "$WORKDIR/src/utils.c" "$WORKDIR/src/parser.c"; svn add "$WORKDIR/src/parser.c" 2>/dev/null; }
    [ $i -eq 50 ] && { echo "Deprecated" > "$WORKDIR/docs/DEPRECATED.md"; svn add "$WORKDIR/docs/DEPRECATED.md" 2>/dev/null; }

    svn commit "$WORKDIR" -m "Dev commit $i by $U" --username "$U" --password "$P" --non-interactive 2>/dev/null
    [ $(( i % 25 )) -eq 0 ] && echo "  r$i done ($U)"
done

# Phase 3: Binary and large files (101-130)
echo "=== Phase 3: Binary + large files ==="
dd if=/dev/urandom of="$WORKDIR/assets/logo.png" bs=1024 count=50 2>/dev/null
svn add "$WORKDIR/assets/logo.png" 2>/dev/null
svn commit "$WORKDIR" -m "Add logo (50KB)" --username bob --password bob123 --non-interactive 2>/dev/null

dd if=/dev/urandom of="$WORKDIR/assets/dataset.bin" bs=1024 count=2048 2>/dev/null
svn add "$WORKDIR/assets/dataset.bin" 2>/dev/null
svn commit "$WORKDIR" -m "Add dataset (2MB, LFS)" --username bob --password bob123 --non-interactive 2>/dev/null

dd if=/dev/urandom of="$WORKDIR/assets/model.dat" bs=1024 count=5120 2>/dev/null
svn add "$WORKDIR/assets/model.dat" 2>/dev/null
svn commit "$WORKDIR" -m "Add model (5MB, LFS)" --username bob --password bob123 --non-interactive 2>/dev/null

dd if=/dev/urandom of="$WORKDIR/assets/archive.tar.gz" bs=1024 count=1536 2>/dev/null
svn add "$WORKDIR/assets/archive.tar.gz" 2>/dev/null
svn commit "$WORKDIR" -m "Add archive (1.5MB, LFS)" --username bob --password bob123 --non-interactive 2>/dev/null

for i in $(seq 105 130); do
    dd if=/dev/urandom of="$WORKDIR/assets/dataset.bin" bs=1024 count=2048 2>/dev/null
    echo "// Binary update $i" >> "$WORKDIR/src/main.c"
    svn commit "$WORKDIR" -m "Update dataset iteration $i" --username bob --password bob123 --non-interactive 2>/dev/null
done
echo "  r101-130 done"

# Phase 4: Rapid iteration (131-200)
echo "=== Phase 4: Rapid iteration ==="
for i in $(seq 131 200); do
    UI=$(( (i - 131) % 3 ))
    U=${USERS[$UI]}
    P=${PASSWORDS[$UI]}
    echo "// Rapid $i by $U" >> "$WORKDIR/src/main.c"
    svn commit "$WORKDIR" -m "Rapid iteration $i" --username "$U" --password "$P" --non-interactive 2>/dev/null
    [ $(( i % 25 )) -eq 0 ] && echo "  r$i done"
done

# Phase 5: Edge cases (201-250)
echo "=== Phase 5: Edge cases ==="
touch "$WORKDIR/tests/empty_file.txt"
svn add "$WORKDIR/tests/empty_file.txt" 2>/dev/null
svn commit "$WORKDIR" -m "Add empty file" --username alice --password alice123 --non-interactive 2>/dev/null

echo "test" > "$WORKDIR/docs/design notes.txt"
svn add "$WORKDIR/docs/design notes.txt" 2>/dev/null
svn commit "$WORKDIR" -m "File with spaces in name" --username charlie --password charlie123 --non-interactive 2>/dev/null

mkdir -p "$WORKDIR/src/modules/core/handlers"
echo "// Deep" > "$WORKDIR/src/modules/core/handlers/request.c"
svn add "$WORKDIR/src/modules" 2>/dev/null
svn commit "$WORKDIR" -m "Deep nested module" --username alice --password alice123 --non-interactive 2>/dev/null

for i in $(seq 204 250); do
    UI=$(( (i - 204) % 3 ))
    U=${USERS[$UI]}
    P=${PASSWORDS[$UI]}
    echo "// Edge $i" >> "$WORKDIR/src/main.c"
    svn commit "$WORKDIR" -m "Edge case $i" --username "$U" --password "$P" --non-interactive 2>/dev/null
done

echo "=== DONE ==="
svn info "$SVN_URL" --username alice --password alice123 --non-interactive 2>&1 | grep "Revision"

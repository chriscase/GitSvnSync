#!/usr/bin/env bash
set -euo pipefail

# Setup the complete GitSvnSync test environment
# Usage: ./tests/scripts/setup-test-env.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
TESTS_DIR="$PROJECT_DIR/tests"

echo "==> GitSvnSync Test Environment Setup"
echo "    Project: $PROJECT_DIR"
echo ""

# Check prerequisites
for cmd in docker; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd is required but not installed."
        exit 1
    fi
done

# Check Docker Compose (v2 plugin or standalone)
if docker compose version &>/dev/null; then
    COMPOSE="docker compose"
elif command -v docker-compose &>/dev/null; then
    COMPOSE="docker-compose"
else
    echo "Error: docker compose is required but not installed."
    exit 1
fi

# Build the project first
echo "==> Building GitSvnSync..."
cd "$PROJECT_DIR"
cargo build 2>&1 | tail -5

# Start the test environment
echo "==> Starting Docker containers..."
cd "$TESTS_DIR"
$COMPOSE -f docker-compose.yml up -d --build

echo "==> Waiting for services to be healthy..."
for i in {1..30}; do
    if $COMPOSE -f docker-compose.yml ps | grep -q "unhealthy\|starting"; then
        echo "    Waiting... ($i/30)"
        sleep 2
    else
        break
    fi
done

# Seed test data
echo "==> Seeding SVN test repository..."
"$SCRIPT_DIR/seed-svn-repo.sh"

echo "==> Seeding Git test repository..."
"$SCRIPT_DIR/seed-git-repo.sh"

echo ""
echo "==> Test environment ready!"
echo ""
echo "    SVN Server:    http://localhost:8081/svn/testrepo"
echo "    Gitea:         http://localhost:3000"
echo "    GitSvnSync:    http://localhost:8080"
echo ""
echo "    Run tests:     make test-e2e"
echo "    Tear down:     make test-env-down"

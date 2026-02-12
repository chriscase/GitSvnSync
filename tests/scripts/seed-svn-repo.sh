#!/usr/bin/env bash
set -euo pipefail

# Seed the test SVN repository with sample data
# Requires: SVN server running at localhost:8081

SVN_URL="http://localhost:8081/svn/testrepo"
SVN_USER="alice"
SVN_PASS="testpass123"
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

echo "==> Seeding SVN test repository at $SVN_URL"

# Create standard layout
svn mkdir "$SVN_URL/trunk" "$SVN_URL/branches" "$SVN_URL/tags" \
    --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache \
    -m "Create standard SVN layout" 2>/dev/null || echo "    Layout already exists"

# Checkout trunk
svn checkout "$SVN_URL/trunk" "$TMP_DIR/trunk" \
    --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache

cd "$TMP_DIR/trunk"

# Add initial files
cat > README.md << 'EOF'
# Test Project

This is a test project for GitSvnSync E2E testing.
EOF

cat > src/main.py << 'PYEOF'
#!/usr/bin/env python3
"""Test project main module."""


def hello():
    """Say hello."""
    return "Hello from the test project!"


def add(a, b):
    """Add two numbers."""
    return a + b


if __name__ == "__main__":
    print(hello())
PYEOF
mkdir -p src
mv main.py src/ 2>/dev/null || true

cat > src/utils.py << 'PYEOF'
"""Utility functions."""


def format_name(first, last):
    """Format a full name."""
    return f"{first} {last}"


def parse_config(path):
    """Parse a config file."""
    with open(path) as f:
        return f.read()
PYEOF

cat > .svnignore << 'EOF'
__pycache__
*.pyc
.env
EOF

svn add --force . 2>/dev/null || true
svn commit --username "$SVN_USER" --password "$SVN_PASS" \
    --non-interactive --no-auth-cache \
    -m "Add initial project files" 2>/dev/null || echo "    Files already exist"

# Second commit by different user
cat > src/config.py << 'PYEOF'
"""Configuration management."""

DEFAULT_CONFIG = {
    "debug": False,
    "log_level": "info",
    "port": 8080,
}


def get_config(overrides=None):
    """Get config with optional overrides."""
    config = DEFAULT_CONFIG.copy()
    if overrides:
        config.update(overrides)
    return config
PYEOF

svn add src/config.py 2>/dev/null || true
svn commit --username bob --password testpass123 \
    --non-interactive --no-auth-cache \
    -m "Add configuration module" 2>/dev/null || echo "    Already committed"

echo "==> SVN test repository seeded successfully"

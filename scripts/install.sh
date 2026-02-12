#!/usr/bin/env bash
set -euo pipefail

# GitSvnSync installer
# Usage: curl -fsSL https://github.com/chriscase/GitSvnSync/releases/latest/download/install.sh | bash

VERSION="${GITSVNSYNC_VERSION:-latest}"
INSTALL_DIR="${GITSVNSYNC_INSTALL_DIR:-/usr/local/bin}"
CONFIG_DIR="/etc/gitsvnsync"
DATA_DIR="/var/lib/gitsvnsync"

REPO="chriscase/GitSvnSync"

# Detect platform
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "Error: unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    linux) PLATFORM="unknown-linux-gnu" ;;
    darwin) PLATFORM="apple-darwin" ;;
    *) echo "Error: unsupported OS: $OS"; exit 1 ;;
esac

TARGET="${ARCH}-${PLATFORM}"

echo "==> GitSvnSync Installer"
echo "    Platform: ${OS}/${ARCH}"
echo "    Version:  ${VERSION}"
echo ""

# Resolve latest version
if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    echo "    Resolved version: ${VERSION}"
fi

# Download
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/gitsvnsync-${VERSION}-${TARGET}.tar.gz"
echo "==> Downloading from ${DOWNLOAD_URL}"

TMP_DIR=$(mktemp -d)
trap "rm -rf ${TMP_DIR}" EXIT

curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/gitsvnsync.tar.gz"
tar -xzf "${TMP_DIR}/gitsvnsync.tar.gz" -C "${TMP_DIR}"

# Install binaries
echo "==> Installing binaries to ${INSTALL_DIR}"
sudo install -m 755 "${TMP_DIR}/gitsvnsync-daemon" "${INSTALL_DIR}/gitsvnsync-daemon"
sudo install -m 755 "${TMP_DIR}/gitsvnsync" "${INSTALL_DIR}/gitsvnsync"

# Create config directory
if [ ! -d "$CONFIG_DIR" ]; then
    echo "==> Creating config directory at ${CONFIG_DIR}"
    sudo mkdir -p "$CONFIG_DIR"
    sudo cp "${TMP_DIR}/config.example.toml" "${CONFIG_DIR}/config.toml"
    sudo cp "${TMP_DIR}/authors.example.toml" "${CONFIG_DIR}/authors.toml"
    echo "    Edit ${CONFIG_DIR}/config.toml with your settings"
fi

# Create data directory
if [ ! -d "$DATA_DIR" ]; then
    echo "==> Creating data directory at ${DATA_DIR}"
    sudo mkdir -p "$DATA_DIR"
fi

# Create system user if running as root
if [ "$(id -u)" = "0" ] || [ -n "${SUDO_USER:-}" ]; then
    if ! id -u gitsvnsync &>/dev/null; then
        echo "==> Creating gitsvnsync system user"
        sudo useradd -r -s /usr/sbin/nologin -d "$DATA_DIR" gitsvnsync || true
    fi
    sudo chown -R gitsvnsync:gitsvnsync "$DATA_DIR"
    sudo chown -R gitsvnsync:gitsvnsync "$CONFIG_DIR"
fi

# Install systemd service
if command -v systemctl &>/dev/null; then
    echo "==> Installing systemd service"
    sudo cp "${TMP_DIR}/gitsvnsync.service" /etc/systemd/system/gitsvnsync.service
    sudo systemctl daemon-reload
    echo "    Run: sudo systemctl enable --now gitsvnsync"
fi

echo ""
echo "==> GitSvnSync ${VERSION} installed successfully!"
echo ""
echo "Next steps:"
echo "  1. Edit ${CONFIG_DIR}/config.toml with your SVN and GitHub settings"
echo "  2. Edit ${CONFIG_DIR}/authors.toml with your author mappings"
echo "  3. Create ${CONFIG_DIR}/env with your secrets:"
echo "       GITSVNSYNC_SVN_PASSWORD=your-svn-password"
echo "       GITSVNSYNC_GITHUB_TOKEN=your-github-token"
echo "       GITSVNSYNC_ADMIN_PASSWORD=your-dashboard-password"
echo "       GITSVNSYNC_SESSION_SECRET=$(openssl rand -hex 32)"
echo "  4. Start the daemon: sudo systemctl enable --now gitsvnsync"
echo "  5. Open http://your-server:8080 for the dashboard"

# Deployment Guide

## Development Workflow (push to dev server)

For iterative development against `orw-chrisc-rk10.wv.mentorg.com`:

```bash
make deploy          # Full deploy: build web UI locally, push code, build on server, restart
make deploy-no-ui    # Rust-only: skip npm build (faster when only backend changed)
make deploy-dry-run  # Preview all steps without touching the server
```

`scripts/deploy.sh` handles the full cycle:
1. **Push** — `git push server <branch>`
2. **Web UI** — `npm run build` locally, then rsync `web-ui/dist/` to server staging area
3. **Remote** (single SSH session) — `git reset --hard`, `cargo build --release`, `sudo install` binaries to `/usr/local/bin/`, sudo-copy web UI to `/usr/local/bin/static/`, `systemctl restart gitsvnsync`

**SSH prerequisite** (one-time setup):
```bash
mkdir -p ~/.ssh/cm && chmod 700 ~/.ssh/cm
```

Ensure `~/.ssh/config` has ControlMaster configured for the `rk10` host — see the SSH config in the repo for the recommended settings.

> **Note:** The daemon serves the React UI as static files from a `static/` directory next to the binary. In the systemd deployment this resolves to `/usr/local/bin/static/`.

---

## systemd (Recommended for Linux)

### Install

```bash
# Install binaries
sudo install -m 755 gitsvnsync-daemon /usr/local/bin/
sudo install -m 755 gitsvnsync /usr/local/bin/

# Create system user
sudo useradd -r -s /usr/sbin/nologin -m -d /var/lib/gitsvnsync gitsvnsync

# Create directories
sudo mkdir -p /etc/gitsvnsync /var/lib/gitsvnsync
sudo chown gitsvnsync:gitsvnsync /var/lib/gitsvnsync

# Copy config files
sudo cp config.example.toml /etc/gitsvnsync/config.toml
sudo cp tests/fixtures/authors.toml /etc/gitsvnsync/authors.toml

# Create env file with secrets
sudo tee /etc/gitsvnsync/env <<EOF
GITSVNSYNC_SVN_PASSWORD=your-password
GITSVNSYNC_GITHUB_TOKEN=your-token
GITSVNSYNC_ADMIN_PASSWORD=your-admin-password
GITSVNSYNC_SESSION_SECRET=$(openssl rand -hex 32)
EOF
sudo chmod 600 /etc/gitsvnsync/env
sudo chown gitsvnsync:gitsvnsync /etc/gitsvnsync/env

# Install service
sudo cp scripts/gitsvnsync.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now gitsvnsync
```

### Manage

```bash
sudo systemctl status gitsvnsync    # Check status
sudo systemctl restart gitsvnsync   # Restart
journalctl -u gitsvnsync -f         # View logs
```

## Docker

### docker run

```bash
docker run -d \
  --name gitsvnsync \
  --restart=unless-stopped \
  -p 8080:8080 \
  -v /etc/gitsvnsync:/etc/gitsvnsync:ro \
  -v /var/lib/gitsvnsync:/var/lib/gitsvnsync \
  --env-file /etc/gitsvnsync/env \
  ghcr.io/chriscase/gitsvnsync:latest
```

### docker compose

```yaml
version: "3.8"
services:
  gitsvnsync:
    image: ghcr.io/chriscase/gitsvnsync:latest
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config:/etc/gitsvnsync:ro
      - gitsvnsync-data:/var/lib/gitsvnsync
    env_file:
      - ./secrets.env

volumes:
  gitsvnsync-data:
```

## Monitoring

### Health Check

```bash
curl http://localhost:8080/api/status/health
# Returns: {"ok": true}
```

### Prometheus Metrics (future)

Planned for a future release. Currently, monitor via:
- Web dashboard at port 8080
- `gitsvnsync status` CLI command
- journalctl / Docker logs

## Backup

The critical data is in the SQLite database at `<data_dir>/gitsvnsync.db`:

```bash
# Backup
sqlite3 /var/lib/gitsvnsync/gitsvnsync.db ".backup /backup/gitsvnsync-$(date +%Y%m%d).db"

# Or simply copy (safe with WAL mode when daemon is running)
cp /var/lib/gitsvnsync/gitsvnsync.db /backup/
```

## Security Checklist

- [ ] Run daemon as unprivileged user (`gitsvnsync`)
- [ ] Secrets in environment variables, not config files
- [ ] `/etc/gitsvnsync/env` has mode 600
- [ ] Web dashboard behind HTTPS (use a reverse proxy like nginx/caddy)
- [ ] Webhook secret configured for GitHub
- [ ] SVN service account has minimal required permissions
- [ ] GitHub token has minimal required scopes (`repo`)

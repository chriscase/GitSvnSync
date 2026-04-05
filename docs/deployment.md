# Deployment Guide

## systemd (Recommended for Linux)

### Install

```bash
# Install binaries
sudo install -m 755 reposync-daemon /usr/local/bin/
sudo install -m 755 reposync /usr/local/bin/

# Create system user
sudo useradd -r -s /usr/sbin/nologin -m -d /var/lib/reposync reposync

# Create directories
sudo mkdir -p /etc/reposync /var/lib/reposync
sudo chown reposync:reposync /var/lib/reposync

# Copy config files
sudo cp config.example.toml /etc/reposync/config.toml
sudo cp tests/fixtures/authors.toml /etc/reposync/authors.toml

# Create env file with secrets
sudo tee /etc/reposync/env <<EOF
REPOSYNC_SVN_PASSWORD=your-password
REPOSYNC_GITHUB_TOKEN=your-token
REPOSYNC_ADMIN_PASSWORD=your-admin-password
REPOSYNC_SESSION_SECRET=$(openssl rand -hex 32)
EOF
sudo chmod 600 /etc/reposync/env
sudo chown reposync:reposync /etc/reposync/env

# Install service
sudo cp scripts/reposync.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now reposync
```

### Manage

```bash
sudo systemctl status reposync    # Check status
sudo systemctl restart reposync   # Restart
journalctl -u reposync -f         # View logs
```

## Docker

### docker run

```bash
docker run -d \
  --name reposync \
  --restart=unless-stopped \
  -p 8080:8080 \
  -v /etc/reposync:/etc/reposync:ro \
  -v /var/lib/reposync:/var/lib/reposync \
  --env-file /etc/reposync/env \
  ghcr.io/chriscase/reposync:latest
```

### docker compose

```yaml
version: "3.8"
services:
  reposync:
    image: ghcr.io/chriscase/reposync:latest
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config:/etc/reposync:ro
      - reposync-data:/var/lib/reposync
    env_file:
      - ./secrets.env

volumes:
  reposync-data:
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
- `reposync status` CLI command
- journalctl / Docker logs

## Backup

The critical data is in the SQLite database at `<data_dir>/reposync.db`:

```bash
# Backup
sqlite3 /var/lib/reposync/reposync.db ".backup /backup/reposync-$(date +%Y%m%d).db"

# Or simply copy (safe with WAL mode when daemon is running)
cp /var/lib/reposync/reposync.db /backup/
```

## Security Checklist

- [ ] Run daemon as unprivileged user (`reposync`)
- [ ] Secrets in environment variables, not config files
- [ ] `/etc/reposync/env` has mode 600
- [ ] Web dashboard behind HTTPS (use a reverse proxy like nginx/caddy)
- [ ] Webhook secret configured for GitHub
- [ ] SVN service account has minimal required permissions
- [ ] GitHub token has minimal required scopes (`repo`)

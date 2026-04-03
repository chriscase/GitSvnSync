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

## Production Security

### Encryption Key Management

GitSvnSync encrypts stored credentials (SVN passwords, Git tokens) using
AES-256-GCM. The encryption key can be provided in two ways:

1. **Environment variable (recommended for production):**
   ```bash
   # Generate a key:
   openssl rand -hex 32

   # Set in your environment or systemd unit:
   REPOSYNC_ENCRYPTION_KEY=<hex-encoded-32-byte-key>
   ```

2. **Auto-generated (default):** If no environment variable is set, a key is
   generated automatically and stored in the SQLite database. This is
   convenient for development but means the key and encrypted data live in
   the same file — an attacker with database access can decrypt all secrets.

**For production deployments, always set `REPOSYNC_ENCRYPTION_KEY` via
environment variable** to ensure the key is stored separately from the data.

### Admin Password

The admin password is stored as a bcrypt hash. On first login after upgrade
from plaintext storage, the password is automatically migrated to bcrypt.

### CORS Configuration

By default, CORS origins are derived from the listen address. For production,
configure explicit origins in your `config.toml`:

```toml
[web]
cors_origins = ["https://gitsvnsync.example.com"]
```

### LDAP TLS Verification

LDAP TLS certificate verification is enabled by default. To disable it for
servers with self-signed certificates:

```toml
# Via the Admin UI → LDAP page, toggle "Verify TLS Certificates"
# Or in the database: ldap_tls_verify = "false"
```

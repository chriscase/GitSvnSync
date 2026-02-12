# Getting Started

This guide walks you through setting up GitSvnSync to synchronize an SVN repository with a GitHub repository.

## Prerequisites

- **SVN client** (`svn` CLI) version 1.8+
- **Git** version 2.20+
- **Network access** to both your SVN server and GitHub (Enterprise or .com)
- **A VM or server** to run the daemon (Linux recommended)
- **SVN admin access** to enable the `pre-revprop-change` hook (needed for author mapping)

## Step 1: Install GitSvnSync

### Option A: Binary Release (Recommended)

```bash
curl -fsSL https://github.com/chriscase/GitSvnSync/releases/latest/download/install.sh | bash
```

### Option B: Build from Source

```bash
git clone https://github.com/chriscase/GitSvnSync.git
cd GitSvnSync
cargo build --release
sudo install -m 755 target/release/gitsvnsync-daemon /usr/local/bin/
sudo install -m 755 target/release/gitsvnsync /usr/local/bin/
```

### Option C: Docker

```bash
docker pull ghcr.io/chriscase/gitsvnsync:latest
```

## Step 2: Prepare Your SVN Server

GitSvnSync needs the `pre-revprop-change` hook enabled to preserve author identity when syncing Git commits back to SVN.

On your SVN server, create this hook:

```bash
# /path/to/svn/repos/hooks/pre-revprop-change
#!/bin/sh
REPOS="$1"
REV="$2"
USER="$3"
PROPNAME="$4"
ACTION="$5"

# Allow GitSvnSync to set the original author
if [ "$PROPNAME" = "svn:author" ] && [ "$USER" = "sync-service" ]; then
    exit 0
fi

# Allow log message edits
if [ "$PROPNAME" = "svn:log" ]; then
    exit 0
fi

exit 1
```

Make it executable:
```bash
chmod +x /path/to/svn/repos/hooks/pre-revprop-change
```

Optionally, add a post-commit hook for instant sync (instead of polling):

```bash
# /path/to/svn/repos/hooks/post-commit
#!/bin/sh
REPOS="$1"
REV="$2"
curl -s -X POST http://your-gitsvnsync-server:8080/webhook/svn \
  -H "Content-Type: application/json" \
  -d "{\"repository\": \"$REPOS\", \"revision\": $REV}" || true
```

## Step 3: Prepare Your GitHub Repository

1. Create a **Personal Access Token** (or GitHub App) with `repo` scope
2. Set up a **webhook** on the GitHub repository:
   - URL: `http://your-gitsvnsync-server:8080/webhook/github`
   - Content type: `application/json`
   - Secret: (generate a random string)
   - Events: select "Just the push event"

## Step 4: Create the Configuration

```bash
sudo mkdir -p /etc/gitsvnsync /var/lib/gitsvnsync
gitsvnsync init --config /etc/gitsvnsync/config.toml
```

Edit `/etc/gitsvnsync/config.toml`:

```toml
[daemon]
poll_interval_secs = 15
data_dir = "/var/lib/gitsvnsync"

[svn]
url = "https://svn.company.com/repos/project"
username = "sync-service"
password_env = "GITSVNSYNC_SVN_PASSWORD"

[github]
api_url = "https://github.company.com/api/v3"  # or https://api.github.com
repo = "org/project"
token_env = "GITSVNSYNC_GITHUB_TOKEN"
webhook_secret_env = "GITSVNSYNC_WEBHOOK_SECRET"

[identity]
mapping_file = "/etc/gitsvnsync/authors.toml"
email_domain = "company.com"

[web]
listen = "0.0.0.0:8080"
auth_mode = "simple"
admin_password_env = "GITSVNSYNC_ADMIN_PASSWORD"
```

## Step 5: Set Up Author Mappings

Create `/etc/gitsvnsync/authors.toml`:

```toml
[authors]
jsmith = { name = "John Smith", email = "jsmith@company.com" }
janedoe = { name = "Jane Doe", email = "jane.doe@company.com" }
# Add all SVN users here

[defaults]
email_domain = "company.com"
```

Any unmapped SVN user `foo` will default to `foo <foo@company.com>`.

## Step 6: Set Up Secrets

Create `/etc/gitsvnsync/env`:

```bash
GITSVNSYNC_SVN_PASSWORD=your-svn-service-account-password
GITSVNSYNC_GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx
GITSVNSYNC_WEBHOOK_SECRET=your-webhook-secret
GITSVNSYNC_ADMIN_PASSWORD=your-dashboard-password
GITSVNSYNC_SESSION_SECRET=$(openssl rand -hex 32)
```

Secure the file:
```bash
sudo chmod 600 /etc/gitsvnsync/env
sudo chown gitsvnsync:gitsvnsync /etc/gitsvnsync/env
```

## Step 7: Start the Daemon

```bash
sudo systemctl enable --now gitsvnsync
```

Or run directly:
```bash
gitsvnsync-daemon --config /etc/gitsvnsync/config.toml
```

## Step 8: Verify

1. Open `http://your-server:8080` — you should see the dashboard
2. Make a commit to SVN — it should appear in Git within 15 seconds
3. Push a commit to Git — it should appear in SVN within 15 seconds
4. Check the audit log: `gitsvnsync audit --limit 5`

## Troubleshooting

- **Daemon won't start**: Check `journalctl -u gitsvnsync -f` for errors
- **SVN auth failure**: Verify credentials with `svn info --username sync-service <url>`
- **GitHub auth failure**: Verify token with `curl -H "Authorization: token <TOKEN>" <api_url>/user`
- **Author not mapped**: Check `gitsvnsync identity list` and add missing entries

See [troubleshooting.md](troubleshooting.md) for more.

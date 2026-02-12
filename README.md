# GitSvnSync

**Bidirectional SVN ↔ Git synchronization bridge that just works.**

GitSvnSync is a server daemon that continuously synchronizes commits between SVN and Git repositories. It runs on a VM, watches both systems, auto-merges non-conflicting changes, and alerts your team only when human intervention is needed.

## Why GitSvnSync?

Many enterprise teams are stuck on SVN but have access to GitHub Enterprise or GitHub.com. Existing tools don't solve the problem:

| Tool | Problem |
|------|---------|
| **git-svn** | Single-developer only, corrupts merge history, no daemon mode |
| **SubGit** | Commercial, requires disabling writes to both repos |
| **svn2git** | One-way migration only |

GitSvnSync fills this gap as a **free, open-source, production-grade** solution.

## Features

- **Bidirectional sync** — SVN commits appear in Git, Git pushes appear in SVN
- **Server daemon** — runs on a VM, watches both repos, syncs automatically
- **Author identity mapping** — SVN usernames ↔ Git name+email, seamlessly preserved
- **Automatic conflict resolution** — non-overlapping changes merged automatically
- **Web dashboard** — React-based UI for monitoring sync status and resolving conflicts
- **Conflict notifications** — Slack and email alerts when human intervention needed
- **GitHub Enterprise & GitHub.com** — first-class support via GitHub API and webhooks
- **Configurable SVN layouts** — standard (trunk/branches/tags) or custom paths
- **Two sync modes** — direct auto-sync or PR-gated for teams wanting review
- **Crash recovery** — transaction log ensures no data loss on restart
- **Docker & systemd** — deploy however you prefer

## Quick Start

### Prerequisites

- SVN client (`svn` CLI) installed
- Git installed
- Network access to both your SVN server and GitHub

### Install

**From binary release:**
```bash
curl -fsSL https://github.com/chriscase/GitSvnSync/releases/latest/download/install.sh | bash
```

**From source:**
```bash
git clone https://github.com/chriscase/GitSvnSync.git
cd GitSvnSync
cargo build --release
```

**Docker:**
```bash
docker pull ghcr.io/chriscase/gitsvnsync:latest
```

### Configure

```bash
# Generate a default config file
gitsvnsync init --config /etc/gitsvnsync/config.toml

# Edit with your SVN and GitHub details
$EDITOR /etc/gitsvnsync/config.toml

# Set up author mappings
$EDITOR /etc/gitsvnsync/authors.toml
```

### Run

**As a systemd service:**
```bash
sudo cp scripts/gitsvnsync.service /etc/systemd/system/
sudo systemctl enable gitsvnsync
sudo systemctl start gitsvnsync
```

**With Docker:**
```bash
docker run -d \
  --name gitsvnsync \
  -p 8080:8080 \
  -v /etc/gitsvnsync:/etc/gitsvnsync:ro \
  -v /var/lib/gitsvnsync:/var/lib/gitsvnsync \
  --env-file /etc/gitsvnsync/env \
  ghcr.io/chriscase/gitsvnsync:latest
```

**Directly:**
```bash
gitsvnsync-daemon --config /etc/gitsvnsync/config.toml
```

### Access the Dashboard

Open `http://your-server:8080` in your browser. Log in with the configured password or GitHub OAuth.

## Configuration

See [docs/configuration.md](docs/configuration.md) for the full configuration reference.

**Minimal config example:**
```toml
[daemon]
poll_interval_secs = 15
data_dir = "/var/lib/gitsvnsync"

[svn]
url = "https://svn.company.com/repos/project"
username = "sync-service"
password_env = "GITSVNSYNC_SVN_PASSWORD"

[github]
api_url = "https://github.company.com/api/v3"
repo = "org/project"
token_env = "GITSVNSYNC_GITHUB_TOKEN"

[identity]
mapping_file = "/etc/gitsvnsync/authors.toml"
email_domain = "company.com"

[web]
listen = "0.0.0.0:8080"
auth_mode = "simple"
admin_password_env = "GITSVNSYNC_ADMIN_PASSWORD"
```

## Author Mapping

GitSvnSync transparently maps identities between SVN and Git:

```toml
# authors.toml
[authors]
jsmith = { name = "John Smith", email = "jsmith@company.com" }
janedoe = { name = "Jane Doe", email = "jane.doe@company.com" }

[defaults]
email_domain = "company.com"
```

**SVN → Git:** SVN username `jsmith` becomes Git author `John Smith <jsmith@company.com>`
**Git → SVN:** Git author `John Smith` maps back to SVN user `jsmith`

The sync daemon is always visible as the Git committer for audit purposes.

## Architecture

```
┌─────────────────────────────────────────────────┐
│              GitSvnSync Daemon                  │
│                                                 │
│  SVN Watcher ──▶ Sync Engine ◀── Git Watcher   │
│  Identity Mapper   │   Conflict Resolution      │
│  Web UI (React) ◀──┘   Notifications            │
│                                                 │
│  SQLite: commit map, conflicts, audit log       │
└─────────┬───────────────────────────┬───────────┘
          │                           │
     SVN Server              GitHub Enterprise
                             or GitHub.com
```

See [docs/architecture.md](docs/architecture.md) for the full technical design.

## CLI

```bash
gitsvnsync status                              # Show sync status
gitsvnsync conflicts list                      # List active conflicts
gitsvnsync conflicts resolve <id> --accept git # Resolve from CLI
gitsvnsync sync now                            # Trigger immediate sync
gitsvnsync identity list                       # Show author mappings
gitsvnsync audit --limit 20                    # Recent sync history
```

## Development

### Test Environment

Spin up a complete isolated test environment with one command:

```bash
make test-env-up    # Starts SVN server + Gitea + daemon via Docker Compose
make test-all       # Runs the full E2E test suite
make test-env-down  # Tears everything down
```

### Building

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Unit tests
cargo clippy                   # Lint
```

### Project Structure

```
crates/
  core/       # Shared sync logic, SVN/Git clients, identity mapping, conflicts
  daemon/     # Server daemon binary
  web/        # Axum web server + REST API
  cli/        # Command-line management tool
web-ui/       # React frontend
tests/        # Integration & E2E tests with Docker Compose
docs/         # Documentation
scripts/      # Install scripts, systemd units
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License. See [LICENSE](LICENSE) for details.

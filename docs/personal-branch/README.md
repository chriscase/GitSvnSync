# Personal Branch Mode

## What Is Personal Branch Mode?

Personal Branch Mode is a lightweight, single-developer configuration of GitSvnSync. It lets you mirror an SVN repository to your own GitHub account, work on feature branches with pull requests, and have merged commits automatically sync back to SVN -- all from a daemon running on your local machine.

Instead of deploying a shared server with webhooks and identity mapping, Personal Branch Mode runs as a background process alongside your normal development tools. You get a full Git-native workflow (branches, PRs, code review) while your commits continue to land in SVN exactly where your team expects them.

## Who Is This For?

Personal Branch Mode is designed for individual developers who:

- Work on a team that uses SVN as the canonical source of truth
- Want to use Git branches, pull requests, and code review for their own work
- Need a private GitHub mirror for CI, backups, or just a better workflow
- Do not have the authority (or desire) to migrate the whole team to Git

If your entire team is ready to move to Git, look at the full [team deployment](../deployment.md) instead. Personal Branch Mode is for the situation where SVN is staying, and you want Git anyway.

## How It Differs from Team Mode

| Capability | Team Mode | Personal Branch Mode |
|---|---|---|
| Deployment | Server/VM daemon | Local daemon (your laptop/desktop) |
| Web dashboard | Yes (React UI via Axum) | No |
| Multi-user identity mapping | Yes (LDAP/file-based) | No (single SVN user) |
| Webhook receiver | Yes (GitHub webhooks) | No (polls GitHub API) |
| Conflict resolution UI | Web-based | CLI-based |
| Setup | Config files + server provisioning | Interactive CLI wizard |
| SVN hook required | `pre-revprop-change` | None |

Personal Branch Mode strips away everything you do not need when you are the only user. No server to maintain, no hooks to install on the SVN server, no ports to open. Just `gitsvnsync personal init` and you are running.

## Data Flow

```
┌──────────┐         ┌─────────────────┐         ┌──────────┐
│          │  poll   │  GitSvnSync     │  push   │          │
│   SVN    │────────>│  Personal       │────────>│  GitHub  │
│  Server  │<────────│  Daemon         │<────────│  Repo    │
│          │  commit │  (your laptop)  │  PR API │          │
└──────────┘         └─────────────────┘         └──────────┘
```

**SVN to Git:** The daemon polls your SVN repository on a configurable interval (default: 30 seconds). When new revisions appear, it translates them into Git commits and pushes to your GitHub repository's `main` branch.

**Git to SVN:** When you merge a pull request on GitHub, the daemon detects the new commits on `main` via the GitHub API, translates them back into SVN commits, and pushes them to the SVN server under your SVN credentials. The original Git author and commit message are preserved.

## Feature Highlights

### Bidirectional Sync

SVN revisions flow into Git as commits on `main`. Merged PRs flow back to SVN as new revisions. Both directions run continuously in the background.

### PR-Based Workflow

Create feature branches, open pull requests, request reviews, run CI -- the full GitHub workflow. When a PR is merged into `main`, the daemon picks up the resulting commits and syncs them to SVN. Merge commits, squash merges, and rebase merges are all supported.

### Automatic Echo Suppression

When the daemon pushes an SVN revision to GitHub, it records the mapping. When it later sees that same commit on `main` while polling GitHub, it recognizes the commit as its own and skips it. This prevents infinite sync loops without any manual bookkeeping.

### Interactive CLI Setup Wizard

Run `gitsvnsync personal init` to walk through configuration step by step. The wizard prompts for your SVN URL, credentials, GitHub token, and target repository. It validates connectivity at each step and writes the config file for you.

```bash
$ gitsvnsync personal init
? SVN repository URL: https://svn.example.com/repos/project/trunk
? SVN username: jdeveloper
? SVN password: ********
  Connecting to SVN... OK (r4521)
? GitHub personal access token: ghp_xxxxxxxxxxxx
? GitHub repository (owner/name): jdeveloper/project-mirror
  Repository does not exist. Create it? [Y/n] Y
  Created jdeveloper/project-mirror (private)
  Performing initial import (4521 revisions)...
  Config written to ~/.config/gitsvnsync/personal.toml
  Run `gitsvnsync personal start` to begin syncing.
```

### Health Check Doctor Command

The `gitsvnsync personal doctor` command validates your entire setup: SVN connectivity, GitHub token scopes, local Git state, daemon status, and sync lag. It reports problems with specific fix instructions.

```bash
$ gitsvnsync personal doctor
[OK] SVN connection to https://svn.example.com/repos/project/trunk
[OK] GitHub token has repo scope
[OK] Local Git repo is clean
[WARN] Daemon not running (last seen 2h ago)
  Fix: run `gitsvnsync personal start`
[OK] Sync lag: 0 revisions behind SVN, 0 commits behind GitHub
```

### Conflict Detection and Resolution

If someone commits to SVN while you have unsynced PR merges (or vice versa), the daemon detects the divergence and pauses sync. It presents the conflict in your terminal with options to rebase your changes on top, take one side, or manually resolve.

### Auto-Create GitHub Repository

During initial setup, if the target GitHub repository does not exist, the wizard offers to create it for you (private by default). The initial import translates your SVN history into Git commits so you start with full history.

### Crash Recovery

The daemon persists its state to a local SQLite database: a watermark of the last synced SVN revision, a watermark of the last synced Git commit SHA, and a full commit map linking SVN revisions to Git SHAs. If the daemon crashes or your machine restarts, it reads the watermarks on startup and resumes exactly where it left off. No duplicate commits, no missed revisions.

## Further Reading

- **[Quickstart](quickstart.md)** -- Go from zero to syncing in five minutes.
- **[Configuration](configuration.md)** -- All config options for `personal.toml`.
- **[Workflows](workflows.md)** -- Branching strategies, PR patterns, and CI integration.
- **[Troubleshooting](troubleshooting.md)** -- Common problems and how to fix them.
- **[Architecture](architecture.md)** -- How Personal Branch Mode works under the hood.
- **[FAQ](faq.md)** -- Frequently asked questions.

# Personal Branch Mode: Day-to-Day Workflows

This guide covers everyday usage of GitSvnSync Personal Branch Mode after initial setup is complete. For setup instructions, see the [getting started guide](../getting-started.md).

## Table of Contents

- [Creating Feature Branches](#creating-feature-branches)
- [Making Commits and Pushing](#making-commits-and-pushing)
- [Opening PRs and the Merge Workflow](#opening-prs-and-the-merge-workflow)
- [How SVN Commits Appear in Git](#how-svn-commits-appear-in-git)
- [How Git Commits Appear in SVN](#how-git-commits-appear-in-svn)
- [Handling Conflicts](#handling-conflicts)
- [Checking Sync Status](#checking-sync-status)
- [Viewing the Audit Trail](#viewing-the-audit-trail)
- [Daemon Management](#daemon-management)

---

## Creating Feature Branches

Work on your GitHub mirror just like any normal Git repository. Create feature branches from the default branch (typically `main`):

```bash
cd ~/my-project
git checkout main
git pull origin main
git checkout -b feature/add-search-endpoint
```

The daemon continuously syncs SVN changes to `main`, so always pull before branching to ensure you start from the latest SVN state.

## Making Commits and Pushing

Develop normally on your feature branch. Commit and push as you would with any Git project:

```bash
git add src/search.rs src/routes.rs
git commit -m "Add search endpoint with pagination"
git push -u origin feature/add-search-endpoint
```

Commits on feature branches are **not** synced to SVN. Only commits that reach the default branch via merged pull requests are synced back to SVN.

## Opening PRs and the Merge Workflow

When your feature is ready, open a pull request targeting the default branch on your GitHub repo:

```bash
gh pr create --base main --title "Add search endpoint" --body "Implements full-text search with pagination"
```

All three GitHub merge strategies are supported:

| Strategy | GitHub Button | What Happens in SVN |
|----------|--------------|---------------------|
| **Merge commit** | "Merge pull request" | Each PR commit is replayed individually to SVN |
| **Squash merge** | "Squash and merge" | A single SVN commit is created with the squashed content |
| **Rebase merge** | "Rebase and merge" | Each rebased commit is replayed individually to SVN |

After you merge the PR on GitHub, the daemon detects the merge on its next polling cycle (default: every 30 seconds) and replays the commits to SVN automatically.

### Merge Strategy Detection

The daemon detects which merge strategy was used by inspecting the merge commit:

- **2 parents** on the merge commit indicates a standard merge.
- **1 parent** with a single PR commit indicates a squash merge.
- **1 parent** with multiple PR commits indicates a rebase merge.

## How SVN Commits Appear in Git

When someone commits to SVN, the daemon picks up the new revision and creates a corresponding Git commit on `main`. The commit message preserves the original message and appends metadata trailers:

```
Fix null pointer in parser module

Synced-From: svn
SVN-Revision: r1042
SVN-Author: alice
SVN-Date: 2025-06-15T14:23:07Z
Sync-Marker: [gitsvnsync]
```

The trailers provide full traceability:

| Trailer | Purpose |
|---------|---------|
| `Synced-From: svn` | Identifies the commit as originating from SVN |
| `SVN-Revision: r1042` | The original SVN revision number |
| `SVN-Author: alice` | The SVN username that made the original commit |
| `SVN-Date: 2025-06-15T14:23:07Z` | The timestamp of the SVN commit |
| `Sync-Marker: [gitsvnsync]` | Echo suppression marker -- prevents the daemon from syncing this commit back to SVN |

The Git commit's author is set to your configured developer identity (from `[developer]` in the config), and the commit is pushed to `origin/main` automatically.

## How Git Commits Appear in SVN

When a PR is merged to `main` on GitHub, the daemon replays each commit to SVN. The SVN commit message preserves the original message and appends metadata:

```
Add search endpoint with pagination

[gitsvnsync] Synced from Git
Git-SHA: a3f7b2c9d1e4
PR-Number: #42
PR-Branch: feature/add-search-endpoint
```

The metadata fields provide traceability back to Git:

| Field | Purpose |
|-------|---------|
| `[gitsvnsync]` | Echo suppression marker -- prevents the daemon from syncing this commit back to Git |
| `Git-SHA: a3f7b2c9d1e4` | The original Git commit hash |
| `PR-Number: #42` | The GitHub pull request number |
| `PR-Branch: feature/add-search-endpoint` | The source branch of the pull request |

The SVN commit's author is set to your configured `developer.svn_username`.

## Handling Conflicts

Conflicts occur when both SVN and Git have changes to the same file between sync cycles. The daemon pauses syncing for the affected file and records the conflict.

### Listing Conflicts

```bash
gitsvnsync personal conflicts list
```

This displays a table of active conflicts:

```
┌──────────┬──────────────────┬─────────┬─────────┬────────────┐
│ ID       │ File             │ Type    │ SVN Rev │ Created    │
├──────────┼──────────────────┼─────────┼─────────┼────────────┤
│ abc12345 │ src/parser.rs    │ content │ r1043   │ 2025-06-15 │
│ def67890 │ docs/readme.md   │ content │ r1044   │ 2025-06-15 │
└──────────┴──────────────────┴─────────┴─────────┴────────────┘
```

### Resolving Conflicts

Resolve a conflict by accepting either the SVN version or the Git version:

```bash
# Accept the SVN version (discard Git changes)
gitsvnsync personal conflicts resolve abc12345 --accept svn

# Accept the Git version (discard SVN changes)
gitsvnsync personal conflicts resolve abc12345 --accept git
```

Once resolved, the daemon applies the chosen version on the next sync cycle and resumes normal operation.

### Conflict Types

| Type | Description |
|------|-------------|
| Content | Same lines changed differently on both sides |
| Edit/Delete | File edited on one side, deleted on the other |
| Rename | File renamed differently on both sides |
| Binary | Binary file modified on both sides |

If `auto_merge = true` (the default), the daemon attempts a 3-way merge automatically when changes are on different lines of the same file. Only overlapping changes produce a conflict that requires manual resolution.

## Checking Sync Status

View the current sync state at a glance:

```bash
gitsvnsync personal status
```

Example output:

```
GitSvnSync Personal Branch
══════════════════════════

  Status     Running (PID 12345)
  SVN        r1042
  Git        a3f7b2c

  Recent Activity
  ────────────────────────────────────────
  2025-06-15 14:23:07  SVN → Git
  2025-06-15 14:20:12  Git → SVN
  2025-06-15 14:15:00  SVN → Git
```

The status shows:

- Whether the daemon is running and its PID
- The current SVN and Git watermarks (last synced positions)
- Recent sync activity with timestamps and direction

## Viewing the Audit Trail

### Sync History Log

View a chronological log of all sync operations:

```bash
gitsvnsync personal log
gitsvnsync personal log --limit 50
```

Example output:

```
Sync History (last 20)

  2025-06-15 14:23:07  SVN → Git  synced SVN r1042 as Git a3f7b2c9
  2025-06-15 14:20:12  Git → SVN  PR #42: replayed commit a3f7b2c9 as r1041
  2025-06-15 14:15:00  SVN → Git  synced SVN r1040 as Git b8e1d3a5
```

### PR Sync History

View a detailed log of pull requests that have been synced to SVN:

```bash
gitsvnsync personal pr-log
gitsvnsync personal pr-log --limit 50
```

Example output:

```
PR Sync History

┌──────┬────────────────────────┬──────────┬─────────┬──────────┬──────────┐
│ PR # │ Branch                 │ Strategy │ Commits │ SVN Revs │ Status   │
├──────┼────────────────────────┼──────────┼─────────┼──────────┼──────────┤
│ #42  │ feature/add-search     │ squash   │ 1       │ r1041    │ synced   │
│ #41  │ fix/null-pointer       │ merge    │ 3       │ r1038-40 │ synced   │
│ #40  │ feature/auth-refactor  │ rebase   │ 5       │ r1033-37 │ synced   │
└──────┴────────────────────────┴──────────┴─────────┴──────────┴──────────┘
```

The PR log shows the merge strategy detected for each PR, the number of commits, the corresponding SVN revision range, and the sync status.

## Daemon Management

### Starting the Daemon

Start the daemon in the background:

```bash
gitsvnsync personal start
```

Output:

```
Daemon started (PID 12345)
Polling SVN every 30 seconds
Watching for merged PRs on yourname/project-mirror

  Logs: ~/.local/share/gitsvnsync/personal.log
  Stop: gitsvnsync personal stop
```

### Running in Foreground for Debugging

For troubleshooting, run the daemon in the foreground with live log output:

```bash
gitsvnsync personal start --foreground
```

In foreground mode, all log output goes to stdout/stderr instead of the log file. The daemon runs until you press Ctrl+C or send SIGTERM.

To increase log verbosity, set the `log_level` in your config to `debug` or `trace`:

```toml
[personal]
log_level = "debug"
```

Or set the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug gitsvnsync personal start --foreground
```

### Stopping the Daemon

```bash
gitsvnsync personal stop
```

The daemon receives SIGTERM and shuts down gracefully, completing any in-progress sync cycle before exiting. If the daemon does not exit within 5 seconds, the stop command reports an error.

### Restarting the Daemon

Stop and start the daemon in sequence:

```bash
gitsvnsync personal stop && gitsvnsync personal start
```

### Checking if the Daemon is Running

Use the status command to see whether the daemon is active:

```bash
gitsvnsync personal status
```

The first line shows "Running (PID XXXXX)" or "Stopped".

### Custom Config File Location

By default, the CLI looks for the config at `~/.config/gitsvnsync/personal.toml`. To use a different location:

```bash
gitsvnsync personal --personal-config /path/to/my-config.toml start
gitsvnsync personal --personal-config /path/to/my-config.toml status
```

This flag works with all personal subcommands.

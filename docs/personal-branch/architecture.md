# Personal Branch Mode - Architecture

Technical deep dive for contributors working on the personal branch sync engine.

## System Overview

Personal Branch Mode is a simplified, single-developer variant of the GitSvnSync daemon. It runs on your local machine (or a VM), continuously mirroring an SVN path to a private GitHub repository and replaying merged pull requests back to SVN.

```
┌─────────────────────────────────────────────────────────────────────┐
│               gitsvnsync-personal daemon (Rust)                     │
│                                                                     │
│  ┌───────────────┐   ┌──────────────────┐   ┌───────────────────┐  │
│  │ SvnToGitSync  │   │ PersonalSync     │   │ PrMonitor         │  │
│  │               │──▶│ Engine           │◀──│                   │  │
│  │ (svn export + │   │ (state machine + │   │ (GitHub API poll  │  │
│  │  git commit)  │   │  cycle runner)   │   │  for merged PRs)  │  │
│  └───────────────┘   └──────┬───────────┘   └───────────────────┘  │
│                              │                                      │
│  ┌───────────────┐   ┌──────┴───────────┐   ┌───────────────────┐  │
│  │ GitToSvnSync  │   │ CommitFormatter  │   │ Scheduler         │  │
│  │               │   │                  │   │ (tokio interval + │  │
│  │ (replay PR    │   │ (message format  │   │  signal handling) │  │
│  │  commits to   │   │  + echo markers) │   │                   │  │
│  │  SVN WC)      │   │                  │   │                   │  │
│  └───────────────┘   └──────────────────┘   └───────────────────┘  │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ SQLite DB (WAL mode): watermarks, commit_map, pr_sync_log,  │   │
│  │ conflicts, audit_log, sync_state, kv_state                  │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Sync Engine State Machine

The personal sync engine runs as a linear pipeline on each polling cycle. Every state transition is logged via `tracing`.

```
                    ┌─────────────────────────────────────────────────┐
                    │                                                 │
                    ▼                                                 │
              ┌──────────┐                                            │
              │   Idle   │◀──────────────────────────────────────┐    │
              └────┬─────┘                                       │    │
                   │ (poll timer fires)                          │    │
                   ▼                                             │    │
            ┌─────────────┐                                      │    │
            │ PollingSvn  │                                      │    │
            └──────┬──────┘                                      │    │
                   │ (new SVN revisions found)                   │    │
                   ▼                                             │    │
        ┌──────────────────┐                                     │    │
        │ ApplyingSvnToGit │                                     │    │
        │  (export, copy,  │                                     │    │
        │   commit, push)  │                                     │    │
        └──────┬───────────┘                                     │    │
               │ (done)                                          │    │
               ▼                                                 │    │
        ┌──────────────┐                                         │    │
        │ PollingGitPRs│                                         │    │
        │ (check for   │                                         │    │
        │  merged PRs) │                                         │    │
        └──────┬───────┘                                         │    │
               │ (unsynced PRs found)                            │    │
               ▼                                                 │    │
      ┌─────────────────────┐                                    │    │
      │ ApplyingGitToSvn    │                                    │    │
      │ (replay PR commits  │                                    │    │
      │  into SVN WC,       │                                    │    │
      │  svn commit)        │                                    │    │
      └──────────┬──────────┘                                    │    │
                 │ (success)                                     │    │
                 └───────────────────────────────────────────────┘    │
                                                                      │
    Any state ──(unrecoverable error)──▶ ┌───────┐                    │
                                         │ Error │────(next cycle)────┘
                                         └───────┘

    Any state ──(overlapping changes)──▶ ┌──────────────────┐
                                         │ ConflictDetected │
                                         │ (user resolves   │
                                         │  via CLI)        │
                                         └──────────────────┘
```

The cycle always runs both phases (SVN-to-Git and Git-to-SVN) in sequence. If SVN-to-Git fails, the engine logs the error and continues to the Git-to-SVN phase rather than aborting the entire cycle.

## Crate Structure

The workspace contains five crates:

```
gitsvnsync/
  crates/
    core/             # Shared library: SVN client, Git client, GitHub API,
                      # database, config, models, conflict detection/merging,
                      # identity mapping, notifications
                      # -> gitsvnsync-core

    personal/         # Personal sync daemon binary + engine library
                      # Modules: engine, svn_to_git, git_to_svn, pr_monitor,
                      # commit_format, scheduler, daemon, signals, initial_import
                      # -> gitsvnsync-personal (binary + lib)

    cli/              # Unified CLI binary
                      # Personal subcommands: init, import, start/stop/status,
                      # conflicts, doctor, log, pr-log
                      # -> gitsvnsync (binary)

    daemon/           # Team-mode daemon binary (server deployment)
                      # -> gitsvnsync-daemon

    web/              # Axum web server, REST API, WebSocket (team mode)
                      # -> gitsvnsync-web
```

Personal mode uses `core` + `personal` + `cli`. The `daemon` and `web` crates are team-mode only.

## Database Schema

SQLite with WAL mode for concurrent reads. Located at `{data_dir}/personal.db`.

### `watermarks` table

Key/value store tracking the last synced position for each source.

| Column       | Type | Description                          |
|-------------|------|--------------------------------------|
| `source`    | TEXT | Primary key (`svn_rev` or `git_sha`) |
| `value`     | TEXT | The watermark value                  |
| `updated_at`| TEXT | ISO 8601 timestamp of last update    |

Used keys:
- `svn_rev` -- last SVN revision successfully synced to Git
- `git_sha` -- last Git SHA successfully synced to SVN

### `commit_map` table

Bidirectional index linking SVN revisions to Git SHAs.

| Column       | Type    | Description                              |
|-------------|---------|------------------------------------------|
| `id`        | INTEGER | Auto-incrementing primary key            |
| `svn_rev`   | INTEGER | SVN revision number                      |
| `git_sha`   | TEXT    | Full Git commit SHA                      |
| `direction` | TEXT    | `svn_to_git` or `git_to_svn`            |
| `synced_at` | TEXT    | ISO 8601 timestamp                       |
| `svn_author`| TEXT    | SVN author username                      |
| `git_author`| TEXT    | Git author (Name \<email\>)              |

Indexed on both `svn_rev` and `git_sha` for fast lookups in either direction. The engine queries this table for echo suppression (checking if a revision/SHA has already been synced) and for the `doctor` and `log` CLI commands.

### `pr_sync_log` table

Tracks the processing status of each merged pull request (added in schema migration 2).

| Column          | Type    | Description                                |
|----------------|---------|--------------------------------------------|
| `id`           | INTEGER | Auto-incrementing primary key              |
| `pr_number`    | INTEGER | GitHub PR number                           |
| `pr_title`     | TEXT    | PR title                                   |
| `pr_branch`    | TEXT    | Source branch name                         |
| `merge_sha`    | TEXT    | SHA of the merge commit on the target branch |
| `merge_strategy`| TEXT   | `merge`, `squash`, `rebase`, or `unknown`  |
| `svn_rev_start`| INTEGER | First SVN revision created for this PR     |
| `svn_rev_end`  | INTEGER | Last SVN revision created for this PR      |
| `commit_count` | INTEGER | Number of commits replayed to SVN          |
| `status`       | TEXT    | `pending`, `completed`, or `failed`        |
| `error_message`| TEXT    | Error details (if status is `failed`)      |
| `detected_at`  | TEXT    | When the merged PR was first detected      |
| `completed_at` | TEXT    | When processing finished                   |

Indexed on `merge_sha` (for deduplication) and `status` (for retry queries).

### `audit_log` table

Complete history of all sync operations for debugging and traceability.

| Column       | Type    | Description                          |
|-------------|---------|--------------------------------------|
| `id`        | INTEGER | Auto-incrementing primary key        |
| `action`    | TEXT    | Action type (e.g. `svn_to_git_sync`, `git_to_svn_commit`, `error`) |
| `direction` | TEXT    | `svn_to_git`, `git_to_svn`, or NULL |
| `svn_rev`   | INTEGER | Related SVN revision (if applicable) |
| `git_sha`   | TEXT    | Related Git SHA (if applicable)      |
| `author`    | TEXT    | Author who triggered the action      |
| `details`   | TEXT    | Human-readable description           |
| `created_at`| TEXT    | ISO 8601 timestamp                   |

Indexed on `created_at` and `action`.

### Other tables

- `sync_state` -- state machine snapshots for crash recovery
- `conflicts` -- unresolved conflicts with full content for 3-way diff
- `sync_records` -- individual sync operation records
- `kv_state` -- general-purpose key/value store

## Echo Suppression

When the daemon syncs a commit from SVN to Git, that new Git commit could be detected on the next Git-to-SVN pass as "new work" and synced back, creating an infinite loop. The same applies in reverse.

GitSvnSync prevents this with a two-layer suppression mechanism:

### Layer 1: Commit message marker

Every synced commit message includes the `[gitsvnsync]` marker string, embedded in a metadata trailer block. The `CommitFormatter::is_sync_marker()` function checks for this marker.

SVN-to-Git commit message example:

```
Fix bug in parser

Synced-From: svn
SVN-Revision: r42
SVN-Author: alice
SVN-Date: 2025-01-15T10:30:00Z
Sync-Marker: [gitsvnsync]
```

Git-to-SVN commit message example:

```
Add search endpoint

[gitsvnsync] Synced from Git
Git-SHA: abc123def456
PR-Number: #42
PR-Branch: feature/search
```

### Layer 2: commit_map lookup

Before syncing any revision or commit, the engine checks the `commit_map` table:

- **SVN-to-Git**: calls `db.is_svn_rev_synced(rev)` to check if the revision is already recorded
- **Git-to-SVN**: calls `db.is_pr_synced(merge_sha)` to check if the PR merge has been processed

Both layers must agree. A revision is skipped if **either** the marker is present in the commit message **or** it is already recorded in the database. This makes the system resilient to template customization (if someone removes the marker from their template, the database still catches it).

## Watermark Mechanism

Watermarks track the "last known good position" in each repository. They answer the question: "What is the newest revision/SHA we have successfully synced?"

### How watermarks work

1. **On startup**, the engine reads `svn_rev` from the `watermarks` table. If no watermark exists, it starts from revision 0 (initial import handles bootstrapping).

2. **During SVN-to-Git sync**, the engine queries SVN for the HEAD revision. If HEAD > watermark, it fetches log entries for revisions `(watermark + 1)..HEAD` and processes each one in order.

3. **After each successful commit and push**, the watermark is advanced to that revision number. This is the critical ordering constraint: the watermark is only updated **after** the commit is recorded in the commit_map and pushed to the remote.

4. **During Git-to-SVN sync**, the PR monitor uses a timestamp-based approach (`get_last_pr_sync_time`) rather than a SHA watermark, querying for PRs merged since the last processed PR's completion time.

### Crash recovery

The watermark design provides automatic crash recovery:

- If the daemon crashes **before** committing to Git, the watermark is not advanced. On restart, the same SVN revision is fetched and processed again. The commit_map check prevents duplicate commits if the Git commit succeeded but the watermark update failed.

- If the daemon crashes **before** committing to SVN, the PR sync log entry remains in `pending` status. On restart, the same PR is detected as unsynced and replayed.

- If the daemon crashes **after** committing but **before** advancing the watermark, the idempotency check (commit_map lookup) skips the already-synced revision and advances the watermark.

## Conflict Resolution Flow

```
Change detected on both sides
  (SVN and Git modified same file since last sync)
         │
         ▼
  3-way merge attempt
  (base = last synced version of the file)
         │
    ┌────┴────┐
    │         │
 SUCCESS   FAILURE
    │         │
    ▼         ▼
 Auto-apply  Enter ConflictDetected state
 the merge   Record in `conflicts` table
             │
             ▼
     User resolves via CLI:
     ┌────────────────────────────────────┐
     │ gitsvnsync personal conflicts list │
     │ gitsvnsync personal conflicts      │
     │   resolve <id> --accept svn        │
     │   resolve <id> --accept git        │
     └────────────────────────────────────┘
             │
             ▼
     Resolution applied to both repos
     Conflict marked as "resolved" in DB
```

Conflict types and their handling:

| Type        | Description                          | Auto-resolvable? |
|-------------|--------------------------------------|------------------|
| Content     | Same lines changed differently       | No               |
| Edit/Delete | One side edited, other deleted        | No               |
| Binary      | Binary file modified on both sides   | No (choose one side) |
| Property    | SVN properties vs Git attributes     | Sometimes        |

The `auto_merge` option in `[options]` controls whether the engine attempts 3-way merge for non-overlapping changes to the same file. When disabled, any file modified on both sides becomes a conflict.

## PR Merge Strategy Detection

When the PR monitor detects a merged pull request, it inspects the merge commit to determine the strategy used. This metadata is stored in `pr_sync_log.merge_strategy` and affects how commits are replayed to SVN.

### Detection algorithm

```
Fetch merge commit details from GitHub API
         │
         ▼
  Count parent commits
         │
    ┌────┴────────────────────┐
    │                         │
  2 parents               1 parent
    │                         │
    ▼                         ▼
  MERGE                 Count PR commits
  (standard merge          │
   commit)            ┌────┴────┐
                      │         │
                   1 commit   N commits
                      │         │
                      ▼         ▼
                   SQUASH    REBASE
```

### Replay behavior by strategy

- **Merge** (2 parents): The PR contained a merge commit. The individual commits from the PR branch are replayed to SVN one by one, preserving granular history.

- **Squash** (1 parent, 1 commit): GitHub combined all PR commits into a single commit. That single commit is replayed to SVN as one SVN revision.

- **Rebase** (1 parent, multiple commits): GitHub rebased the PR commits onto the target branch. Each rebased commit is replayed to SVN individually, preserving the original commit structure.

- **Unknown**: Parent count could not be determined (API error). The engine falls back to replaying whatever commits are found.

## Data Flow: SVN-to-Git

Step-by-step sequence for syncing one SVN revision to Git:

1. Read `svn_rev` watermark from database
2. Query SVN HEAD revision via `svn info`
3. If HEAD <= watermark, return (nothing to do)
4. Fetch `svn log` entries for range `(watermark+1)..HEAD`
5. For each entry:
   a. **Echo check**: skip if message contains `[gitsvnsync]`
   b. **Idempotency check**: skip if `svn_rev` exists in `commit_map`
   c. `svn export` the revision to a temp directory
   d. Copy exported files into the Git working tree (skips `.git/` at root)
   e. Format the commit message using the `svn_to_git` template
   f. Create a Git commit with the developer's identity
   g. `git push` to origin
   h. Insert record into `commit_map`
   i. Advance the `svn_rev` watermark
   j. Write audit log entry

## Data Flow: Git-to-SVN

Step-by-step sequence for syncing merged PRs back to SVN:

1. Query `pr_sync_log` for the last completed sync timestamp
2. Fetch merged PRs from GitHub API (merged since last timestamp)
3. For each merged PR:
   a. **Dedup check**: skip if `merge_sha` exists in `pr_sync_log`
   b. Fetch the PR's commits from GitHub API
   c. Detect merge strategy (merge/squash/rebase)
   d. Insert a `pending` record into `pr_sync_log`
   e. Filter out echo commits (containing `[gitsvnsync]`)
   f. For each non-echo commit:
      - `svn update` the working copy to HEAD
      - Copy changed files from Git repo to SVN working copy
      - Remove stale files (exist in SVN WC but not in Git repo)
      - Run `svn status` to detect added (`?`) and deleted (`!`) files
      - Execute `svn add` / `svn rm` as needed
      - Format the commit message using the `git_to_svn` template
      - `svn commit` with the developer's SVN username
      - Insert record into `commit_map`
      - Write audit log entry
   g. Mark PR sync as `completed` (or `failed` on error)

## File Synchronization

Both sync directions use recursive directory copy with VCS metadata exclusion:

- **SVN-to-Git** (`SvnToGitSync::copy_tree`): Skips all dotfiles/dotdirs at the export root to protect `.git/`. Nested dotfiles within subdirectories are copied normally.

- **Git-to-SVN** (`git_to_svn::copy_tree`): Skips `.git` and `.svn` directories at every level. Additionally runs `remove_stale_files` to delete files present in the SVN working copy but absent from the Git repo (triggering `svn rm` via status detection).

## Configuration Resolution

Personal mode configuration (`PersonalConfig`) follows a three-step loading process:

1. **Parse**: Load TOML from the config file path
2. **Resolve**: Read `*_env` fields to populate runtime secrets from environment variables (SVN password, GitHub token)
3. **Validate**: Check all required fields are present and well-formed

Sensitive values (passwords, tokens) are never stored in the config file. The `_env` suffix convention means the config value is the **name** of an environment variable that holds the secret.

## Scheduler and Signal Handling

The personal daemon runs a tokio interval timer that fires every `poll_interval_secs` (default: 30). On each tick, it calls `engine.run_cycle()`.

Signal handling (Unix only):
- `SIGTERM` / `SIGINT`: graceful shutdown (finish current cycle, then exit)
- `SIGHUP`: reload configuration from disk

The daemon writes a PID file to `{data_dir}/daemon.pid` for the CLI to check daemon status and send signals.

## CLI Commands (Personal Mode)

All personal commands live under `gitsvnsync personal`:

| Command      | Description                                      |
|-------------|--------------------------------------------------|
| `init`      | Interactive setup wizard, generates config file  |
| `import`    | Initial SVN-to-Git import (bootstraps watermarks)|
| `start`     | Start the background daemon                      |
| `stop`      | Stop the running daemon                          |
| `status`    | Show daemon and sync status                      |
| `log`       | Show recent sync activity from audit_log         |
| `pr-log`    | Show PR sync history from pr_sync_log            |
| `conflicts` | List and resolve active conflicts                |
| `doctor`    | Run health checks on the entire setup            |

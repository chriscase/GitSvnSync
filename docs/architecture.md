# Architecture

## System Overview

GitSvnSync is a server daemon that provides bidirectional synchronization between SVN and Git (GitHub) repositories. It runs as a background service on a VM, continuously monitoring both repositories for changes.

```
┌─────────────────────────────────────────────────────────────────┐
│                    GitSvnSync Daemon (Rust)                     │
│                                                                 │
│  ┌──────────┐   ┌──────────────┐   ┌────────────────────────┐  │
│  │ SVN      │   │ Sync Engine  │   │ GitHub                 │  │
│  │ Watcher  │──▶│ (State       │◀──│ Watcher                │  │
│  │ (polling │   │  Machine)    │   │ (webhooks + polling    │  │
│  │  + hooks)│   │              │   │  via GitHub API)       │  │
│  └──────────┘   └──────┬───────┘   └────────────────────────┘  │
│                         │                                       │
│  ┌──────────┐   ┌──────┴───────┐   ┌────────────────────────┐  │
│  │ Identity │   │ Conflict     │   │ Web UI                 │  │
│  │ Mapper   │   │ Resolution   │   │ (React dashboard       │  │
│  │ (LDAP/   │   │ Pipeline     │   │  served by Axum)       │  │
│  │  file)   │   │              │   │                        │  │
│  └──────────┘   └──────────────┘   └────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ SQLite DB: commit map, sync state, conflicts, audit log │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Sync Engine State Machine

The sync engine operates as a state machine with crash recovery:

```
IDLE ──(change detected)──▶ DETECTING
  ▲                            │
  │                     ┌──────┴──────┐
  │                     ▼             ▼
  │               NO_CONFLICT    CONFLICT_FOUND
  │                     │             │
  │                     ▼             ▼
  │               APPLYING      QUEUED_FOR_RESOLUTION
  │                     │             │
  │                     ▼             ▼ (user resolves)
  └────────────── COMMITTED    RESOLUTION_APPLIED ──▶ COMMITTED
```

Every state transition is logged to SQLite. On crash/restart, the daemon reads the last state and resumes.

## Echo Suppression

When the daemon syncs a commit from SVN to Git, that Git push triggers a webhook. The daemon must recognize this as its own work and skip it. This is tracked via the commit mapping table — if a Git push matches a known synced commit SHA, it's suppressed.

## Conflict Resolution Pipeline

```
Change Detected on Both Sides
     │
     ▼
 Same files changed?
     │          │
    NO         YES
     │          │
     ▼          ▼
 Auto-apply   3-way merge attempt (base = last synced version)
 both sides        │
                   │
            ┌──────┴──────┐
         SUCCESS       CONFLICT
            │              │
            ▼              ▼
       Auto-apply    Queue for manual resolution
                           │
                     ┌─────┼─────┐
                     ▼     ▼     ▼
                   Slack  Email  Web UI
                           │
                     User resolves
                           │
                     Apply resolution
```

## Identity Mapping

SVN and Git represent author identity differently:

- **SVN**: Simple username string (`jsmith`)
- **Git**: Name + email (`John Smith <jsmith@company.com>`)
- **Git also has**: Separate Author vs Committer fields

GitSvnSync uses the Author/Committer distinction to preserve audit trail:

| Direction | Author field | Committer field |
|-----------|-------------|-----------------|
| SVN → Git | Original developer (mapped from SVN username) | GitSvnSync daemon |
| Git → SVN | `svn:author` set to mapped SVN username | N/A (SVN has no committer) |

## Database Schema

SQLite with WAL mode for concurrent reads:

- **commit_map**: Links SVN revisions to Git SHAs (bidirectional lookup)
- **sync_state**: Current sync engine state for crash recovery
- **conflicts**: Queue of unresolved conflicts with full diff content
- **watermarks**: Last synced position for each source (SVN rev, Git SHA)
- **audit_log**: Complete history of all sync operations

## Web Server

Axum-based HTTP server embedded in the daemon:

- **REST API**: JSON endpoints for status, conflicts, configuration, audit
- **WebSocket**: Real-time updates pushed to connected browsers
- **Static files**: Serves the React build from embedded assets
- **Webhooks**: Receives push events from GitHub and SVN hooks

## Crate Structure

```
gitsvnsync-core    # Shared library: sync logic, clients, identity, conflicts, DB
gitsvnsync-daemon  # Binary: daemon entry point, scheduler, signal handling
gitsvnsync-web     # Library: Axum server, REST API, WebSocket
gitsvnsync-cli     # Binary: management CLI (status, conflicts, identity, etc.)
```

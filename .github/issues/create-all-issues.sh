#!/usr/bin/env bash
###############################################################################
# GitSvnSync — Verification & Hardening Issue Creator
#
# Usage:
#   export GITHUB_TOKEN=ghp_...
#   bash .github/issues/create-all-issues.sh
#
# This script creates all labels first, then creates issues in dependency
# order (Phase 0 → Phase 5). Issues reference each other via "depends on"
# links once created.
#
# Requirements: gh CLI (https://cli.github.com) authenticated
###############################################################################
set -euo pipefail

REPO="chriscase/GitSvnSync"

echo "=== GitSvnSync Verification Issue Creator ==="
echo "Repository: $REPO"
echo ""

###############################################################################
# STEP 1: Create Labels
###############################################################################
echo "--- Creating labels ---"

create_label() {
  local name="$1" color="$2" desc="$3"
  gh label create "$name" --repo "$REPO" --color "$color" --description "$desc" --force 2>/dev/null \
    && echo "  ✓ $name" \
    || echo "  ⚠ $name (may already exist)"
}

# Phase labels
create_label "phase:0-foundation"    "0e8a16" "Phase 0: Build & compile verification"
create_label "phase:1-core"          "1d76db" "Phase 1: Core module verification"
create_label "phase:2-integration"   "5319e7" "Phase 2: Integration & cross-module verification"
create_label "phase:3-api-web"       "d93f0b" "Phase 3: API, web, and UI verification"
create_label "phase:4-infra"         "f9d0c4" "Phase 4: CI/CD, Docker, deployment verification"
create_label "phase:5-security"      "b60205" "Phase 5: Security, hardening, final audit"

# Model recommendation labels
create_label "model:claude-haiku"    "c5def5" "Best solved by Claude Haiku — fast, straightforward tasks"
create_label "model:claude-sonnet"   "bfd4f2" "Best solved by Claude Sonnet — moderate complexity"
create_label "model:claude-opus"     "d4c5f9" "Best solved by Claude Opus — deep analysis, complex reasoning"

# Agent execution environment labels
create_label "agent:local"           "fbca04" "Must be run on a local agent (needs filesystem, CLI tools, Docker)"
create_label "agent:cloud"           "0075ca" "Can be run by GitHub Copilot cloud agent or remote agent"
create_label "agent:either"          "006b75" "Can be run locally or in the cloud"

# Type labels
create_label "type:verification"     "e4e669" "Verification task — check existing code for correctness"
create_label "type:fix-required"     "d73a4a" "Fix required — problem was found and needs correction"
create_label "type:test-gap"         "fef2c0" "Test gap — missing test coverage identified"

# Priority labels
create_label "priority:critical"     "b60205" "Blocks other work — must be done first"
create_label "priority:high"         "d93f0b" "Important, do soon after critical items"
create_label "priority:medium"       "fbca04" "Standard priority"
create_label "priority:low"          "0e8a16" "Nice to have, can be deferred"

# Ordering labels
create_label "order:01"              "cccccc" "Execution order 01"
create_label "order:02"              "cccccc" "Execution order 02"
create_label "order:03"              "cccccc" "Execution order 03"
create_label "order:04"              "cccccc" "Execution order 04"
create_label "order:05"              "cccccc" "Execution order 05"
create_label "order:06"              "cccccc" "Execution order 06"
create_label "order:07"              "cccccc" "Execution order 07"
create_label "order:08"              "cccccc" "Execution order 08"
create_label "order:09"              "cccccc" "Execution order 09"
create_label "order:10"              "cccccc" "Execution order 10"
create_label "order:11"              "cccccc" "Execution order 11"
create_label "order:12"              "cccccc" "Execution order 12"
create_label "order:13"              "cccccc" "Execution order 13"
create_label "order:14"              "cccccc" "Execution order 14"
create_label "order:15"              "cccccc" "Execution order 15"
create_label "order:16"              "cccccc" "Execution order 16"
create_label "order:17"              "cccccc" "Execution order 17"
create_label "order:18"              "cccccc" "Execution order 18"
create_label "order:19"              "cccccc" "Execution order 19"
create_label "order:20"              "cccccc" "Execution order 20"
create_label "order:21"              "cccccc" "Execution order 21"
create_label "order:22"              "cccccc" "Execution order 22"

echo ""
echo "--- Creating issues ---"
echo ""

###############################################################################
# Helper: create_issue "title" "body" "label1,label2,..."
# Prints the issue number after creation
###############################################################################
declare -A ISSUE_NUMBERS

create_issue() {
  local title="$1"
  local body="$2"
  local labels="$3"
  local key="$4"  # short key for dependency references

  local num
  num=$(gh issue create --repo "$REPO" \
    --title "$title" \
    --body "$body" \
    --label "$labels" 2>&1 | grep -oE '[0-9]+$')

  ISSUE_NUMBERS["$key"]="$num"
  echo "  ✓ #${num}: ${title}"
}

###############################################################################
# PHASE 0 — Foundation: Build & Compile Verification
###############################################################################

#---------------------------------------------------------------------------
# Issue 1: Verify workspace compiles cleanly
#---------------------------------------------------------------------------
create_issue \
  "[Phase 0] Verify Rust workspace compiles with zero warnings" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify that the entire Rust workspace compiles cleanly on all supported targets with zero warnings.

## Recommended Model

**Claude Haiku** — straightforward compilation check, fast turnaround.

## Execution Environment

**Local agent** — requires Rust toolchain, cargo, and optionally cross-compilation targets.

## Phase & Ordering

| Field        | Value                   |
|-------------|-------------------------|
| Phase       | 0 — Foundation          |
| Order       | 01 (first)              |
| Depends on  | Nothing                 |
| Blocks      | All Phase 1+ issues     |

## Tasks

- [ ] Run `cargo check --workspace` — must succeed with zero errors
- [ ] Run `cargo check --workspace` with `RUSTFLAGS="-D warnings"` — must produce zero warnings
- [ ] Run `cargo clippy --workspace -- -D warnings` — must pass cleanly
- [ ] Run `cargo fmt --all -- --check` — must pass (no formatting issues)
- [ ] Verify `Cargo.lock` is up to date (no unexpected changes after build)
- [ ] Check for any `#[allow(...)]` attributes that may be masking issues — list them

## When Problems Are Found

**If any compilation errors, warnings, or clippy lints are found:**
1. Fix them directly in this issue's PR if they are straightforward (typos, unused imports, missing derives)
2. For non-trivial fixes, create a new issue titled `[Fix] <description of problem>` with:
   - The exact error/warning message
   - The file and line number
   - Label it `type:fix-required` and the appropriate phase label
   - Reference this issue

## Acceptance Criteria

- [ ] `cargo check --workspace` exits 0
- [ ] `cargo clippy --workspace -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] All `#[allow(...)]` usages are documented and justified
ISSUE_BODY
)" \
  "phase:0-foundation,model:claude-haiku,agent:local,type:verification,priority:critical,order:01" \
  "compile"

#---------------------------------------------------------------------------
# Issue 2: Verify all unit tests pass
#---------------------------------------------------------------------------
create_issue \
  "[Phase 0] Verify all 72 unit tests pass" \
  "$(cat <<'ISSUE_BODY'
## Summary

Run the full unit test suite and verify all 72 tests pass. Document any failures with root cause analysis.

## Recommended Model

**Claude Sonnet** — needs to analyze test failures and potentially understand complex test logic.

## Execution Environment

**Local agent** — requires Rust toolchain, SQLite, and `svn` CLI installed for SVN client tests.

## Phase & Ordering

| Field        | Value                                   |
|-------------|----------------------------------------|
| Phase       | 0 — Foundation                          |
| Order       | 02                                      |
| Depends on  | #COMPILE (workspace must compile first) |
| Blocks      | All Phase 1+ issues                     |

## Tasks

- [ ] Run `cargo test --workspace` — record all results
- [ ] Run `cargo test --workspace -- --nocapture` for any failures to see full output
- [ ] For each failing test, document:
  - Test name and file location
  - Error message
  - Root cause analysis
- [ ] Verify test count matches expected 72 tests
- [ ] Run tests with `RUST_LOG=debug` to check for any logged errors during passing tests
- [ ] Check for any `#[ignore]` tests and document why they are ignored

## Test Inventory (72 tests expected)

| Module | Count | Tests |
|--------|-------|-------|
| config.rs | 7 | parse_full_config, load_from_file, file_not_found, validate_rejects_empty_url, validate_rejects_bad_repo_format, resolve_env_vars, defaults |
| errors.rs | 2 | error_display_messages, core_error_from_subsystem |
| sync_engine.rs | 2 | is_echo_commit, sync_state_display |
| conflict/detector.rs | 7 | no_conflicts_disjoint, content_conflict, edit_delete_conflict, both_deleted_no_conflict, binary_conflict, rename_conflict, multiple_conflicts |
| conflict/merger.rs | 8 | identical_files, only_ours_changed, only_theirs_changed, non_overlapping_changes, conflicting_changes, can_auto_merge, cannot_auto_merge, same_change_both_sides |
| conflict/resolver.rs | 7 | accept_svn, accept_git, accept_merged, defer, cannot_resolve_twice, not_found, resolved_content |
| db/mod.rs | 4 | in_memory_database, file_database, transaction_commit, transaction_rollback |
| db/queries.rs | 6 | commit_map_crud, sync_state, conflict_crud, watermark_crud, audit_log, kv_state |
| db/schema.rs | 2 | migrations_run_idempotently, tables_created |
| git/client.rs | 3 | init_and_commit, create_and_delete_branch, repo_not_found |
| git/github.rs | 2 | verify_webhook_signature_valid, verify_webhook_signature_invalid |
| svn/client.rs | 2 | parse_committed_revision, client_construction |
| svn/parser.rs | 2 | parse_svn_info, parse_svn_log |
| identity/mapper.rs | 6 | svn_to_git_from_file, svn_to_git_fallback, svn_to_git_no_fallback, git_to_svn_from_cache, git_to_svn_fallback, reload |
| identity/mapping_file.rs | 4 | load_mapping_file, save_and_reload, load_nonexistent, load_empty_file |
| identity/ldap.rs | 2 | stub_lookup_returns_none, connection_state |
| notify/mod.rs | 4 | notifier_not_configured, format_conflict_slack, format_conflict_email, html_escape |
| notify/email.rs | 1 | email_notifier_construction |
| notify/slack.rs | 1 | slack_notifier_construction |

## When Problems Are Found

**If any tests fail:**
1. Fix the failing test or the code under test directly if the fix is straightforward
2. For complex failures, create a new issue titled `[Fix] Test failure: <test_name> — <brief description>` with:
   - Full error output
   - Root cause analysis
   - Label: `type:fix-required`, `phase:0-foundation`, `priority:critical`
   - Reference this issue

## Acceptance Criteria

- [ ] `cargo test --workspace` exits 0
- [ ] All 72 tests pass
- [ ] No `#[ignore]` tests without documented justification
- [ ] Any failures have been fixed or tracked in separate issues
ISSUE_BODY
)" \
  "phase:0-foundation,model:claude-sonnet,agent:local,type:verification,priority:critical,order:02" \
  "tests"

#---------------------------------------------------------------------------
# Issue 3: Verify web-ui builds
#---------------------------------------------------------------------------
create_issue \
  "[Phase 0] Verify web-ui React/TypeScript app builds cleanly" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify the React/TypeScript web-ui builds without errors and all TypeScript types check correctly.

## Recommended Model

**Claude Haiku** — straightforward build verification.

## Execution Environment

**Local agent** — requires Node.js, npm.

## Phase & Ordering

| Field        | Value                           |
|-------------|--------------------------------|
| Phase       | 0 — Foundation                  |
| Order       | 03 (can run parallel with 01-02)|
| Depends on  | Nothing                         |
| Blocks      | Phase 3 web UI issues           |

## Tasks

- [ ] Run `cd web-ui && npm install` — must succeed
- [ ] Run `npx tsc --noEmit` — TypeScript type check must pass with zero errors
- [ ] Run `npx vite build` — production build must succeed
- [ ] Check for any TypeScript `@ts-ignore` or `any` type usage — document each
- [ ] Verify `package.json` dependencies are reasonable and not outdated
- [ ] Check bundle size of production build

## When Problems Are Found

**If build fails or type errors exist:**
1. Fix directly if straightforward (missing types, import issues)
2. For complex issues, create a new issue titled `[Fix] Web UI: <description>` with:
   - Full error output
   - Label: `type:fix-required`, `phase:0-foundation`
   - Reference this issue

## Acceptance Criteria

- [ ] `npm install` exits 0
- [ ] `tsc --noEmit` exits 0
- [ ] `vite build` exits 0
- [ ] All `@ts-ignore` and `any` usages documented
ISSUE_BODY
)" \
  "phase:0-foundation,model:claude-haiku,agent:local,type:verification,priority:critical,order:03" \
  "webui-build"

###############################################################################
# PHASE 1 — Core Module Verification (each module independently)
###############################################################################

#---------------------------------------------------------------------------
# Issue 4: Verify SVN client & parser
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify SVN client wrapper and XML parser correctness" \
  "$(cat <<'ISSUE_BODY'
## Summary

Deep verification of `crates/core/src/svn/client.rs` and `crates/core/src/svn/parser.rs`. These wrap the SVN CLI and parse its XML output — errors here corrupt sync data.

## Recommended Model

**Claude Opus** — requires deep analysis of XML parsing edge cases, command injection risks, and error handling paths.

## Execution Environment

**Local agent** — needs `svn` CLI installed for integration testing, filesystem access.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 04                             |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #SYNC_ENGINE, #E2E            |

## Files to Verify

- `crates/core/src/svn/client.rs` — `SvnClient` struct, all CLI wrapper methods
- `crates/core/src/svn/parser.rs` — XML parsing for `svn info`, `svn log`, `svn diff --summarize`
- `crates/core/src/svn/mod.rs` — module exports

## Verification Checklist

### Command Safety
- [ ] Verify all `Command::new("svn")` calls properly escape/quote arguments
- [ ] Check for command injection vectors in URL, username, password, path parameters
- [ ] Verify SVN credentials are not logged or leaked in error messages
- [ ] Check timeout handling — what happens if SVN command hangs?

### XML Parsing Robustness
- [ ] Verify `parse_svn_info()` handles malformed XML gracefully
- [ ] Verify `parse_svn_log()` handles empty log (no revisions), single revision, many revisions
- [ ] Verify `parse_svn_diff_summarize()` handles binary files, property changes, empty diffs
- [ ] Check for XML entity injection or billion laughs attack vectors in parsed XML
- [ ] Verify Unicode path handling in SVN output parsing

### Error Handling
- [ ] Verify all `Command` failures produce meaningful `SvnError` variants
- [ ] Check non-zero exit code handling for each SVN subcommand
- [ ] Verify stderr is captured and included in errors
- [ ] Check behavior when SVN server is unreachable

### Edge Cases
- [ ] SVN repository with no commits
- [ ] Paths with spaces, special characters, Unicode
- [ ] Very large revision numbers (u64 overflow?)
- [ ] Empty diff output
- [ ] SVN properties (svn:externals, svn:mergeinfo) handling

## Existing Tests (4 total — likely insufficient)

- `test_parse_committed_revision` — verify this actually tests real SVN output
- `test_client_construction` — verify constructor validation
- `test_parse_svn_info` — verify against real SVN XML output
- `test_parse_svn_log` — verify against real SVN XML output

## When Problems Are Found

**If issues are discovered:**
1. Fix directly: missing error handling, unsafe argument passing, parse failures
2. Create new issues for:
   - Command injection vulnerabilities → `[Fix][Security] SVN command injection in <method>`
   - Parse failures → `[Fix] SVN parser fails on <edge case>`
   - Missing tests → `[Test Gap] SVN client: <scenario>`
   - Label with `type:fix-required` or `type:test-gap`, `priority:high`

## Acceptance Criteria

- [ ] No command injection vectors found (or all fixed)
- [ ] All XML parsing handles malformed input without panicking
- [ ] Credentials never appear in logs or error messages
- [ ] All edge cases documented and tested (or issues created for missing tests)
ISSUE_BODY
)" \
  "phase:1-core,model:claude-opus,agent:local,type:verification,priority:high,order:04" \
  "svn"

#---------------------------------------------------------------------------
# Issue 5: Verify Git client wrapper
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify Git client wrapper (git2 bindings) correctness" \
  "$(cat <<'ISSUE_BODY'
## Summary

Deep verification of `crates/core/src/git/client.rs`. This wraps `git2` (libgit2) for local Git operations — the critical path for applying synced changes.

## Recommended Model

**Claude Opus** — requires understanding of git2/libgit2 API semantics, reference handling, and merge strategies.

## Execution Environment

**Local agent** — needs git installed, filesystem access for test repo creation.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 05 (can parallel with 04)      |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #SYNC_ENGINE, #E2E            |

## Files to Verify

- `crates/core/src/git/client.rs` — `GitClient` struct, all git2 wrapper methods
- `crates/core/src/git/mod.rs` — module exports

## Verification Checklist

### Core Operations
- [ ] `GitClient::new()` — verify repository discovery, error on invalid path
- [ ] `GitClient::clone_repo()` — verify auth handling, shallow vs full clone
- [ ] `GitClient::fetch()` — verify remote resolution, refspec handling
- [ ] `GitClient::pull()` — verify fetch + merge, fast-forward handling
- [ ] `GitClient::push()` — verify remote auth, push refspec, force push safety
- [ ] `GitClient::create_commit()` — verify signature, parent chain, tree building
- [ ] `GitClient::create_branch()` / `delete_branch()` — ref safety
- [ ] `GitClient::merge()` — verify merge strategy, conflict detection passthrough

### Safety & Correctness
- [ ] Verify no data loss scenarios (e.g., force push without user consent)
- [ ] Check reference locking (concurrent access to same repo)
- [ ] Verify commit signature handling (GPG/SSH signing)
- [ ] Check that credential callbacks don't leak tokens
- [ ] Verify `HEAD` detached state handling
- [ ] Check behavior with bare repositories

### Error Handling
- [ ] All git2 errors mapped to `GitError` variants
- [ ] Network failures produce retryable errors
- [ ] Auth failures produce clear error messages (not generic "failed to authenticate")
- [ ] Merge conflicts properly surfaced

### Edge Cases
- [ ] Empty repository (no commits)
- [ ] Repository with submodules
- [ ] Binary files in commits
- [ ] Very large files (>100MB)
- [ ] Branch names with special characters
- [ ] Unicode filenames

## Existing Tests (3 total — likely insufficient)

- `test_init_and_commit` — verify creates real git objects
- `test_create_and_delete_branch` — verify ref manipulation
- `test_repo_not_found` — verify error handling

## When Problems Are Found

**If issues are discovered:**
1. Fix directly: missing error mapping, unsafe operations, credential leaks
2. Create new issues for:
   - Data loss risks → `[Fix][Critical] Git client: <scenario> can cause data loss`
   - Missing error handling → `[Fix] Git client: <method> doesn't handle <error>`
   - Missing tests → `[Test Gap] Git client: <scenario>`
   - Label with appropriate type and priority

## Acceptance Criteria

- [ ] No data loss scenarios found (or all fixed)
- [ ] All git2 errors properly mapped
- [ ] Credential handling verified secure
- [ ] Edge cases documented and tested (or issues created)
ISSUE_BODY
)" \
  "phase:1-core,model:claude-opus,agent:local,type:verification,priority:high,order:05" \
  "git"

#---------------------------------------------------------------------------
# Issue 6: Verify GitHub API client
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify GitHub API client — auth, PRs, webhooks" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/git/github.rs` for correct API usage, webhook signature verification security, and error handling.

## Recommended Model

**Claude Sonnet** — HTTP API verification, webhook HMAC validation logic.

## Execution Environment

**Cloud agent** — code review only, no external API calls needed for verification.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 06 (can parallel with 04-05)   |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #WEBHOOKS, #E2E               |

## Files to Verify

- `crates/core/src/git/github.rs` — `GitHubClient`, PR creation, webhook verification

## Verification Checklist

### Authentication
- [ ] Verify token is sent correctly in Authorization header
- [ ] Verify token is not logged or exposed in error messages
- [ ] Check token validation on construction

### Pull Request Operations
- [ ] Verify `create_pull_request()` sends correct payload
- [ ] Check error handling for rate limiting (HTTP 429)
- [ ] Check error handling for auth failures (HTTP 401/403)
- [ ] Verify response parsing handles GitHub API changes gracefully

### Webhook Signature Verification
- [ ] **CRITICAL**: Verify HMAC-SHA256 signature comparison is constant-time (timing attack prevention)
- [ ] Verify `X-Hub-Signature-256` header parsing
- [ ] Verify empty/missing signature handling
- [ ] Verify payload body is used correctly (raw bytes, not re-serialized JSON)

### Error Handling
- [ ] All HTTP errors mapped to `GitHubError` variants
- [ ] Rate limit info extracted from response headers
- [ ] Network timeout handling

## Existing Tests (2 total)

- `test_verify_webhook_signature_valid` — verify correct HMAC validation
- `test_verify_webhook_signature_invalid` — verify rejection of bad signatures

## When Problems Are Found

**If issues are discovered:**
1. **Security issues** (timing attacks, token leaks): Fix immediately, create `[Fix][Security]` issue
2. Other issues: Fix or create `[Fix]` / `[Test Gap]` issues with reference to this issue

## Acceptance Criteria

- [ ] Webhook signature verification is constant-time
- [ ] No token/secret leakage in logs or errors
- [ ] Rate limiting handled gracefully
- [ ] All HTTP error codes produce meaningful errors
ISSUE_BODY
)" \
  "phase:1-core,model:claude-sonnet,agent:cloud,type:verification,priority:high,order:06" \
  "github"

#---------------------------------------------------------------------------
# Issue 7: Verify database layer
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify SQLite database layer — schema, migrations, queries" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/db/` — schema migrations, query correctness, transaction safety, and concurrent access handling.

## Recommended Model

**Claude Sonnet** — SQL analysis, migration ordering, transaction semantics.

## Execution Environment

**Local agent** — needs to run SQLite operations and test with real database files.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 07 (can parallel with 04-06)   |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #SYNC_ENGINE, #API_STATUS      |

## Files to Verify

- `crates/core/src/db/mod.rs` — `Database` struct, connection management, transactions
- `crates/core/src/db/schema.rs` — `run_migrations()`, table definitions
- `crates/core/src/db/queries.rs` — all CRUD operations

## Verification Checklist

### Schema
- [ ] Verify all tables have appropriate PRIMARY KEYs and indexes
- [ ] Verify foreign key constraints are correct (if used)
- [ ] Check that WAL mode is enabled correctly
- [ ] Verify migrations are idempotent (re-running doesn't break)
- [ ] Check for SQL injection in migration strings (unlikely but verify)

### Queries
- [ ] Verify all SQL uses parameterized queries (no string interpolation)
- [ ] Check commit_map CRUD: insert, lookup by svn_rev, lookup by git_sha
- [ ] Check sync_state: get/set state transitions
- [ ] Check conflict CRUD: insert, update resolution, list by status
- [ ] Check watermark CRUD: get/set per-direction watermarks
- [ ] Check audit_log: insert, paginated query
- [ ] Check kv_state: get/set arbitrary key-value pairs

### Transaction Safety
- [ ] Verify `transaction()` properly commits on success
- [ ] Verify `transaction()` properly rolls back on error
- [ ] Check for deadlock potential with concurrent access
- [ ] Verify WAL mode allows concurrent reads during writes

### Edge Cases
- [ ] Database file permissions (read-only filesystem)
- [ ] Disk full during write
- [ ] Very large audit logs (pagination correctness)
- [ ] Unicode in stored values
- [ ] NULL handling in optional fields

## Existing Tests (12 total)

| Test | Verify |
|------|--------|
| test_in_memory_database | DB creation and initialization |
| test_file_database | File-based DB persistence |
| test_transaction_commit | Commit semantics |
| test_transaction_rollback | Rollback semantics |
| test_commit_map_crud | Commit mapping operations |
| test_sync_state | State machine persistence |
| test_conflict_crud | Conflict lifecycle |
| test_watermark_crud | Watermark tracking |
| test_audit_log | Audit entry storage |
| test_kv_state | Key-value storage |
| test_migrations_run_idempotently | Migration safety |
| test_tables_created | Schema completeness |

## When Problems Are Found

1. SQL injection: Fix immediately → `[Fix][Security] SQL injection in <query>`
2. Missing indexes: Fix directly
3. Transaction bugs: `[Fix][Critical] Database: <scenario>`
4. Test gaps: `[Test Gap] Database: <scenario>`

## Acceptance Criteria

- [ ] All queries use parameterized statements
- [ ] Migrations are idempotent
- [ ] Transactions commit/rollback correctly
- [ ] No SQL injection vectors
ISSUE_BODY
)" \
  "phase:1-core,model:claude-sonnet,agent:local,type:verification,priority:high,order:07" \
  "db"

#---------------------------------------------------------------------------
# Issue 8: Verify conflict detection, merge, and resolution
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify conflict detection, 3-way merge, and resolution engine" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/conflict/` — the conflict detection algorithm, three-way merge engine, and resolution workflow.

## Recommended Model

**Claude Opus** — requires deep understanding of diff algorithms, merge strategies, and state machine correctness.

## Execution Environment

**Cloud agent** — pure logic verification, no external dependencies.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 08 (can parallel with 04-07)   |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #SYNC_ENGINE, #API_CONFLICTS   |

## Files to Verify

- `crates/core/src/conflict/detector.rs` — `ConflictDetector::detect()`
- `crates/core/src/conflict/merger.rs` — `Merger::three_way_merge()`, `can_auto_merge()`
- `crates/core/src/conflict/resolver.rs` — `ConflictResolver` resolution workflow

## Verification Checklist

### Conflict Detection
- [ ] Verify `detect()` correctly identifies overlapping file changes
- [ ] Verify all `ConflictType` variants are properly detected: Content, EditDelete, Rename, Property, Branch, Binary
- [ ] Check for false positives (changes to different files flagged as conflicts)
- [ ] Check for false negatives (real conflicts missed)
- [ ] Verify detection with empty changesets (one or both sides)

### Three-Way Merge
- [ ] Verify `three_way_merge()` produces correct output for non-overlapping changes
- [ ] Verify conflict markers are correctly placed for overlapping changes
- [ ] Verify `can_auto_merge()` accurately predicts mergeability
- [ ] Check handling of binary files (should not attempt text merge)
- [ ] Check handling of files with no common ancestor (new file on both sides)
- [ ] Verify line ending handling (CRLF vs LF)
- [ ] Verify Unicode text handling in merge

### Resolution Workflow
- [ ] Verify `accept_svn()` correctly selects SVN version
- [ ] Verify `accept_git()` correctly selects Git version
- [ ] Verify `resolve_with_content()` accepts custom merge result
- [ ] Verify `defer()` keeps conflict in unresolved state
- [ ] Verify double-resolution is rejected
- [ ] Verify resolution updates conflict status atomically

### Edge Cases
- [ ] Conflict in file with only whitespace differences
- [ ] Very large files (performance)
- [ ] Files with mixed encodings
- [ ] Symlinks
- [ ] Empty files

## Existing Tests (22 total — good coverage)

Detector: 7 tests, Merger: 8 tests, Resolver: 7 tests

## When Problems Are Found

1. False negative (missed conflict): `[Fix][Critical] Conflict detector misses <scenario>`
2. Incorrect merge: `[Fix][Critical] Three-way merge produces wrong output for <scenario>`
3. State machine bugs: `[Fix] Conflict resolver: <scenario>`
4. Missing edge case tests: `[Test Gap] Conflict: <scenario>`

## Acceptance Criteria

- [ ] No false negatives in conflict detection
- [ ] Three-way merge produces correct output for all test scenarios
- [ ] Resolution workflow state machine is sound
- [ ] Binary files handled safely
ISSUE_BODY
)" \
  "phase:1-core,model:claude-opus,agent:cloud,type:verification,priority:high,order:08" \
  "conflict"

#---------------------------------------------------------------------------
# Issue 9: Verify identity mapper
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify identity mapper — TOML file, LDAP, bidirectional mapping" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/identity/` — the bidirectional SVN↔Git author identity mapping system.

## Recommended Model

**Claude Sonnet** — file format parsing, mapping logic, LDAP integration review.

## Execution Environment

**Cloud agent** — code review, no LDAP server needed for verification.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 09 (can parallel with 04-08)   |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #SYNC_ENGINE                   |

## Files to Verify

- `crates/core/src/identity/mapper.rs` — `IdentityMapper` bidirectional mapping
- `crates/core/src/identity/mapping_file.rs` — `MappingFile` TOML persistence
- `crates/core/src/identity/ldap.rs` — `LdapResolver` stub
- `tests/fixtures/authors.toml` — test fixture

## Verification Checklist

### Mapping Logic
- [ ] Verify `svn_to_git()` correctly resolves from file, then LDAP, then fallback
- [ ] Verify `git_to_svn()` correctly reverse-maps
- [ ] Verify `reload()` picks up file changes without restart
- [ ] Check for race conditions during reload (concurrent mapping + reload)
- [ ] Verify fallback behavior is configurable (fail vs generate default)

### TOML File Handling
- [ ] Verify `MappingFile::load()` handles malformed TOML
- [ ] Verify `MappingFile::save()` preserves existing entries
- [ ] Verify file locking during save (concurrent saves)
- [ ] Check Unicode in author names and emails
- [ ] Verify empty file handling

### LDAP Integration
- [ ] Verify `LdapResolver` stub returns `None` (not panic)
- [ ] Check LDAP connection string validation
- [ ] Verify LDAP query injection prevention (if real implementation exists)
- [ ] Document what's needed to complete LDAP integration

### Edge Cases
- [ ] SVN username with spaces or special characters
- [ ] Git email with non-standard format
- [ ] Multiple SVN users mapping to same Git identity
- [ ] Case sensitivity in username matching

## Existing Tests (12 total)

Mapper: 6 tests, MappingFile: 4 tests, LDAP: 2 tests

## When Problems Are Found

1. Fix directly: missing validation, race conditions
2. Create issues: `[Fix] Identity mapper: <issue>` or `[Test Gap] Identity: <scenario>`

## Acceptance Criteria

- [ ] Bidirectional mapping is correct and consistent
- [ ] File reload is safe under concurrent access
- [ ] LDAP stub documented for future completion
- [ ] All edge cases tested or documented
ISSUE_BODY
)" \
  "phase:1-core,model:claude-sonnet,agent:cloud,type:verification,priority:medium,order:09" \
  "identity"

#---------------------------------------------------------------------------
# Issue 10: Verify config loading and validation
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify configuration loading, validation, and env var resolution" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/config.rs` — TOML config loading, validation rules, and environment variable secret resolution.

## Recommended Model

**Claude Sonnet** — config parsing, validation logic, secret handling patterns.

## Execution Environment

**Cloud agent** — code review, no external dependencies.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 10 (can parallel with 04-09)   |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #DAEMON, #CLI                  |

## Files to Verify

- `crates/core/src/config.rs` — all config structs and validation
- `config.example.toml` — example config documentation
- `tests/fixtures/test-config.toml` — test fixture accuracy

## Verification Checklist

- [ ] Verify all required fields are validated (reject empty URLs, bad repo formats)
- [ ] Verify `resolve_env_vars()` correctly substitutes `${ENV_VAR}` patterns
- [ ] Verify missing env vars produce clear errors (not empty strings)
- [ ] Verify default values are sensible
- [ ] Verify `config.example.toml` matches all `AppConfig` struct fields
- [ ] Verify `test-config.toml` is a valid config that passes validation
- [ ] Check for secrets in default values or error messages
- [ ] Verify config file path resolution (relative vs absolute)

## Existing Tests (7 total)

parse_full_config, load_from_file, file_not_found, validate_rejects_empty_url, validate_rejects_bad_repo_format, resolve_env_vars, defaults

## When Problems Are Found

1. Fix directly: validation gaps, default value issues
2. Create issues: `[Fix] Config: <issue>` for complex problems

## Acceptance Criteria

- [ ] All required fields validated
- [ ] Env var resolution works correctly
- [ ] Example config is complete and accurate
- [ ] No secrets in default values or error messages
ISSUE_BODY
)" \
  "phase:1-core,model:claude-sonnet,agent:cloud,type:verification,priority:medium,order:10" \
  "config"

#---------------------------------------------------------------------------
# Issue 11: Verify error types and notification system
#---------------------------------------------------------------------------
create_issue \
  "[Phase 1] Verify error hierarchy and notification system (Slack + Email)" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/errors.rs` and `crates/core/src/notify/` — error type coverage and notification delivery.

## Recommended Model

**Claude Haiku** — relatively straightforward code review.

## Execution Environment

**Cloud agent** — code review, no SMTP or Slack needed.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 1 — Core module verification   |
| Order       | 11 (can parallel with 04-10)   |
| Depends on  | #COMPILE, #TESTS              |
| Blocks      | #SYNC_ENGINE                   |

## Files to Verify

- `crates/core/src/errors.rs` — error type hierarchy
- `crates/core/src/notify/mod.rs` — `Notifier` dispatcher
- `crates/core/src/notify/slack.rs` — `SlackNotifier`
- `crates/core/src/notify/email.rs` — `EmailNotifier`

## Verification Checklist

### Errors
- [ ] Verify all error types implement `Display` and `Error`
- [ ] Verify `From` conversions between error types are correct
- [ ] Check no sensitive information in error display strings
- [ ] Verify error types cover all failure modes

### Notifications
- [ ] Verify Slack webhook URL handling (HTTPS only)
- [ ] Verify email SMTP connection security (STARTTLS/TLS)
- [ ] Verify no secrets in notification message bodies
- [ ] Verify HTML escaping in email notifications (XSS prevention)
- [ ] Verify notification failure doesn't crash the sync engine
- [ ] Check rate limiting awareness (don't spam on repeated failures)

## Existing Tests (8 total)

Errors: 2 tests, Notify: 4 tests, Email: 1 test, Slack: 1 test

## When Problems Are Found

1. Fix directly: missing Display impls, HTML escaping, error message leaks
2. Create issues for complex notification delivery bugs

## Acceptance Criteria

- [ ] Error hierarchy is complete and correct
- [ ] No sensitive data in error messages
- [ ] HTML properly escaped in email notifications
- [ ] Notification failures are non-fatal
ISSUE_BODY
)" \
  "phase:1-core,model:claude-haiku,agent:cloud,type:verification,priority:medium,order:11" \
  "errors-notify"

###############################################################################
# PHASE 2 — Integration & Cross-Module Verification
###############################################################################

#---------------------------------------------------------------------------
# Issue 12: Verify sync engine orchestration
#---------------------------------------------------------------------------
create_issue \
  "[Phase 2] Verify sync engine — state machine, echo suppression, bidirectional flow" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/core/src/sync_engine.rs` — the central orchestrator that coordinates SVN↔Git bidirectional sync. This is the most critical module.

## Recommended Model

**Claude Opus** — complex state machine analysis, race condition detection, data flow verification.

## Execution Environment

**Local agent** — may need to trace through complex call chains with actual dependencies.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 2 — Integration verification   |
| Order       | 12                             |
| Depends on  | #SVN, #GIT, #DB, #CONFLICT, #IDENTITY, #ERRORS_NOTIFY |
| Blocks      | #DAEMON, #E2E                  |

## Files to Verify

- `crates/core/src/sync_engine.rs` — `SyncEngine`, `run_sync_cycle()`, echo detection

## Verification Checklist

### State Machine
- [ ] Map all state transitions: Idle → Detecting → Applying → Committed (or ConflictFound → Queued → ResolutionApplied)
- [ ] Verify no invalid state transitions are possible
- [ ] Verify state is persisted to database at each transition
- [ ] Check for stuck states (what if crash during Applying?)

### Echo Suppression
- [ ] Verify `is_echo_commit()` correctly identifies commits that were synced by this tool
- [ ] Verify echo detection works in both directions (SVN→Git and Git→SVN)
- [ ] Check for edge cases: commit message modified during sync, amend, rebase
- [ ] Verify echo detection doesn't cause infinite loops

### Sync Cycle
- [ ] Verify watermark advancement is atomic with commit application
- [ ] Check: what happens if SVN commit succeeds but watermark update fails?
- [ ] Verify conflict detection runs before commit application
- [ ] Verify notification dispatch on conflict/completion
- [ ] Check concurrent sync cycle prevention (two cycles running simultaneously)

### Error Recovery
- [ ] Verify sync engine recovers from transient SVN errors
- [ ] Verify sync engine recovers from transient Git errors
- [ ] Verify sync engine recovers from database errors
- [ ] Check for data corruption on partial failure

## Existing Tests (2 total — **insufficient for this critical module**)

- `test_is_echo_commit`
- `test_sync_state_display`

## When Problems Are Found

1. State machine bugs: `[Fix][Critical] Sync engine: invalid state transition <from> → <to>`
2. Data loss risks: `[Fix][Critical] Sync engine: <scenario> can cause data loss`
3. Missing tests: `[Test Gap][Critical] Sync engine: <scenario>`
4. **All sync engine bugs are high/critical priority**

## Acceptance Criteria

- [ ] State machine is proven sound (no stuck/invalid states)
- [ ] Echo suppression correctly prevents infinite loops
- [ ] Watermark advancement is atomic
- [ ] Concurrent cycle prevention verified
- [ ] Crash recovery path documented and tested
ISSUE_BODY
)" \
  "phase:2-integration,model:claude-opus,agent:local,type:verification,priority:critical,order:12" \
  "sync-engine"

#---------------------------------------------------------------------------
# Issue 13: Verify daemon scheduler and signal handling
#---------------------------------------------------------------------------
create_issue \
  "[Phase 2] Verify daemon scheduler, graceful shutdown, and signal handling" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/daemon/src/` — the daemon entry point, periodic sync scheduling, and graceful shutdown.

## Recommended Model

**Claude Sonnet** — async runtime analysis, signal handling, shutdown ordering.

## Execution Environment

**Local agent** — needs to verify signal handling and process lifecycle.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 2 — Integration verification   |
| Order       | 13                             |
| Depends on  | #SYNC_ENGINE, #CONFIG          |
| Blocks      | #E2E                           |

## Files to Verify

- `crates/daemon/src/main.rs` — entry point, initialization
- `crates/daemon/src/scheduler.rs` — periodic sync scheduling
- `crates/daemon/src/signals.rs` — SIGTERM/SIGINT handling

## Verification Checklist

- [ ] Verify initialization order: config → logging → database → engine → web → scheduler
- [ ] Verify graceful shutdown: stop scheduler → drain web requests → close database
- [ ] Verify SIGTERM and SIGINT both trigger graceful shutdown
- [ ] Verify double-SIGTERM forces immediate exit
- [ ] Verify scheduler respects configured polling interval
- [ ] Verify scheduler doesn't start new sync cycle during shutdown
- [ ] Verify panic handling (what happens if sync cycle panics?)
- [ ] Verify logging initialization (tracing subscriber setup)

## When Problems Are Found

1. Fix directly: shutdown ordering, signal handling
2. Create issues: `[Fix] Daemon: <issue>`

## Acceptance Criteria

- [ ] Graceful shutdown completes without data loss
- [ ] All signals handled correctly
- [ ] Scheduler respects configuration
- [ ] Panic in sync cycle doesn't crash daemon
ISSUE_BODY
)" \
  "phase:2-integration,model:claude-sonnet,agent:local,type:verification,priority:high,order:13" \
  "daemon"

#---------------------------------------------------------------------------
# Issue 14: Verify CLI commands
#---------------------------------------------------------------------------
create_issue \
  "[Phase 2] Verify CLI tool — all commands, argument parsing, output format" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/cli/src/main.rs` — all CLI commands parse arguments correctly and produce expected output.

## Recommended Model

**Claude Sonnet** — clap derive API verification, command dispatching.

## Execution Environment

**Local agent** — needs to run CLI binary and check output.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 2 — Integration verification   |
| Order       | 14 (can parallel with 13)      |
| Depends on  | #COMPILE, #CONFIG, #DB         |
| Blocks      | #E2E                           |

## Files to Verify

- `crates/cli/src/main.rs` — all subcommands

## Verification Checklist

- [ ] `status` — correct output format, handles no-database case
- [ ] `conflicts list` — pagination, filtering by status
- [ ] `conflicts show <id>` — full conflict details
- [ ] `conflicts resolve <id> <strategy>` — resolution application
- [ ] `sync` — triggers immediate sync cycle
- [ ] `identity add/remove/list` — CRUD operations
- [ ] `init` — generates valid config template
- [ ] `validate` — validates config file correctly
- [ ] `audit` — paginated audit log display
- [ ] Verify `--help` output for all commands
- [ ] Verify error messages for invalid arguments
- [ ] Verify exit codes (0 for success, non-zero for errors)

## When Problems Are Found

1. Fix directly: argument parsing issues, missing error handling
2. Create issues: `[Fix] CLI: <command> <issue>`

## Acceptance Criteria

- [ ] All commands parse arguments correctly
- [ ] All commands produce expected output format
- [ ] Error cases produce helpful messages with non-zero exit codes
ISSUE_BODY
)" \
  "phase:2-integration,model:claude-sonnet,agent:local,type:verification,priority:medium,order:14" \
  "cli"

###############################################################################
# PHASE 3 — API, Web, and UI Verification
###############################################################################

#---------------------------------------------------------------------------
# Issue 15: Verify REST API endpoints
#---------------------------------------------------------------------------
create_issue \
  "[Phase 3] Verify REST API endpoints — status, conflicts, config, audit" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify all Axum HTTP API endpoints in `crates/web/src/api/` for correctness, input validation, and error handling.

## Recommended Model

**Claude Opus** — API security analysis, input validation, auth bypass detection.

## Execution Environment

**Cloud agent** — code review of API handlers.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 3 — API & Web verification     |
| Order       | 15                             |
| Depends on  | #COMPILE, #DB, #CONFLICT       |
| Blocks      | #E2E                           |

## Files to Verify

- `crates/web/src/lib.rs` — `WebServer`, `AppState`, router setup
- `crates/web/src/api/status.rs` — `/api/status`, `/api/health`
- `crates/web/src/api/conflicts.rs` — conflict management endpoints
- `crates/web/src/api/config.rs` — configuration endpoints
- `crates/web/src/api/audit.rs` — audit log endpoints
- `crates/web/src/api/auth.rs` — authentication endpoints
- `crates/web/src/api/mod.rs` — route registration

## Verification Checklist

### Input Validation
- [ ] Verify all path parameters are validated (IDs, pagination)
- [ ] Verify request body JSON deserialization handles malformed input
- [ ] Verify query string parameters are validated
- [ ] Check for integer overflow in pagination parameters

### Authentication & Authorization
- [ ] Verify auth middleware is applied to all non-public endpoints
- [ ] Verify `/api/health` is publicly accessible (no auth required)
- [ ] Verify session token validation
- [ ] Check for auth bypass via path traversal or method override

### Response Format
- [ ] Verify all endpoints return consistent JSON format
- [ ] Verify error responses include appropriate HTTP status codes
- [ ] Verify pagination response includes total count and page info
- [ ] Check for sensitive data leakage in error responses

### CORS & Security Headers
- [ ] Verify CORS configuration is appropriate
- [ ] Verify Content-Type headers are set correctly
- [ ] Check for missing security headers (X-Content-Type-Options, etc.)

## When Problems Are Found

1. Auth bypass: `[Fix][Security][Critical] API: auth bypass via <method>`
2. Input validation: `[Fix] API: missing validation on <endpoint>`
3. Other: `[Fix] API: <endpoint> <issue>`

## Acceptance Criteria

- [ ] All endpoints validate input
- [ ] Authentication cannot be bypassed
- [ ] Error responses don't leak sensitive data
- [ ] CORS properly configured
ISSUE_BODY
)" \
  "phase:3-api-web,model:claude-opus,agent:cloud,type:verification,priority:high,order:15" \
  "api"

#---------------------------------------------------------------------------
# Issue 16: Verify webhook handlers
#---------------------------------------------------------------------------
create_issue \
  "[Phase 3] Verify webhook handlers — GitHub push events, SVN post-commit" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/web/src/api/webhooks.rs` — incoming webhook processing for both GitHub and SVN events.

## Recommended Model

**Claude Opus** — webhook security is critical (signature verification, payload validation).

## Execution Environment

**Cloud agent** — code review of webhook handlers.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 3 — API & Web verification     |
| Order       | 16 (can parallel with 15)      |
| Depends on  | #GITHUB, #SYNC_ENGINE          |
| Blocks      | #E2E                           |

## Files to Verify

- `crates/web/src/api/webhooks.rs` — GitHub and SVN webhook endpoints

## Verification Checklist

- [ ] **CRITICAL**: Verify webhook signature verification happens BEFORE any payload processing
- [ ] Verify GitHub push event payload parsing is correct
- [ ] Verify SVN post-commit hook payload parsing
- [ ] Verify webhook handler returns 200 quickly (doesn't block on sync)
- [ ] Check for replay attack prevention (nonce/timestamp checking)
- [ ] Verify webhook handler is idempotent (duplicate deliveries are safe)
- [ ] Verify unknown event types are gracefully ignored (not 500)
- [ ] Check rate limiting on webhook endpoint

## When Problems Are Found

1. Security issues: Fix immediately → `[Fix][Security] Webhook: <issue>`
2. Other: `[Fix] Webhook: <issue>`

## Acceptance Criteria

- [ ] Signature verification before payload processing
- [ ] Idempotent handling
- [ ] Quick response (async processing)
ISSUE_BODY
)" \
  "phase:3-api-web,model:claude-opus,agent:cloud,type:verification,priority:high,order:16" \
  "webhooks"

#---------------------------------------------------------------------------
# Issue 17: Verify WebSocket real-time updates
#---------------------------------------------------------------------------
create_issue \
  "[Phase 3] Verify WebSocket endpoint for real-time sync status updates" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify `crates/web/src/ws.rs` — WebSocket connection handling and broadcast.

## Recommended Model

**Claude Sonnet** — async WebSocket patterns, connection lifecycle.

## Execution Environment

**Cloud agent** — code review.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 3 — API & Web verification     |
| Order       | 17 (can parallel with 15-16)   |
| Depends on  | #COMPILE                       |
| Blocks      | #WEBUI_VERIFY                  |

## Files to Verify

- `crates/web/src/ws.rs` — WebSocket handler

## Verification Checklist

- [ ] Verify WebSocket upgrade handshake is correct
- [ ] Verify authentication on WebSocket connection
- [ ] Verify broadcast to all connected clients works
- [ ] Verify disconnection cleanup (no memory leaks from abandoned connections)
- [ ] Verify message format matches what web-ui expects
- [ ] Check for DoS via excessive WebSocket connections
- [ ] Verify ping/pong keep-alive handling

## When Problems Are Found

1. Fix directly or create `[Fix] WebSocket: <issue>`

## Acceptance Criteria

- [ ] Connection lifecycle is correct
- [ ] No memory leaks
- [ ] Auth enforced on WebSocket upgrade
ISSUE_BODY
)" \
  "phase:3-api-web,model:claude-sonnet,agent:cloud,type:verification,priority:medium,order:17" \
  "ws"

#---------------------------------------------------------------------------
# Issue 18: Verify web-ui React application
#---------------------------------------------------------------------------
create_issue \
  "[Phase 3] Verify web-ui React pages — Dashboard, Conflicts, AuditLog, Config, Identity" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify all React pages and components in `web-ui/src/` for correctness, API integration, and error handling.

## Recommended Model

**Claude Sonnet** — React/TypeScript verification, API contract matching.

## Execution Environment

**Cloud agent** — code review of React components.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 3 — API & Web verification     |
| Order       | 18                             |
| Depends on  | #WEBUI_BUILD, #API, #WS        |
| Blocks      | #E2E                           |

## Files to Verify

- `web-ui/src/App.tsx` — routing, layout
- `web-ui/src/main.tsx` — entry point
- `web-ui/src/pages/Dashboard.tsx` — sync status display
- `web-ui/src/pages/Conflicts.tsx` — conflict management UI
- `web-ui/src/pages/AuditLog.tsx` — audit log viewer
- `web-ui/src/pages/Config.tsx` — config viewer/editor
- `web-ui/src/pages/Identity.tsx` — identity mapping management

## Verification Checklist

- [ ] Verify API endpoints called match backend routes exactly
- [ ] Verify error handling for failed API calls (show user-friendly messages)
- [ ] Verify loading states are shown during data fetching
- [ ] Verify WebSocket connection is established for real-time updates
- [ ] Verify pagination is implemented correctly
- [ ] Verify conflict resolution UI sends correct payloads
- [ ] Check for XSS vulnerabilities in rendered data (especially user-supplied content)
- [ ] Verify all pages are reachable via routing
- [ ] Check for missing `key` props on list items
- [ ] Verify responsive design (mobile-friendly)

## When Problems Are Found

1. XSS: `[Fix][Security] Web UI: XSS in <component>`
2. API mismatch: `[Fix] Web UI: <page> calls wrong endpoint`
3. Other: `[Fix] Web UI: <issue>`

## Acceptance Criteria

- [ ] API contracts match between frontend and backend
- [ ] No XSS vulnerabilities
- [ ] Error states handled gracefully
- [ ] All pages render correctly
ISSUE_BODY
)" \
  "phase:3-api-web,model:claude-sonnet,agent:cloud,type:verification,priority:medium,order:18" \
  "webui-verify"

###############################################################################
# PHASE 4 — Infrastructure & Deployment Verification
###############################################################################

#---------------------------------------------------------------------------
# Issue 19: Verify CI/CD workflows
#---------------------------------------------------------------------------
create_issue \
  "[Phase 4] Verify CI/CD workflows — ci.yml, e2e.yml, release.yml" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify all GitHub Actions workflow files for correctness, security, and completeness.

## Recommended Model

**Claude Sonnet** — GitHub Actions YAML verification, security best practices.

## Execution Environment

**Cloud agent** — YAML review, no need to run workflows.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 4 — Infrastructure             |
| Order       | 19                             |
| Depends on  | #COMPILE, #TESTS               |
| Blocks      | Nothing (advisory)             |

## Files to Verify

- `.github/workflows/ci.yml` — check, test, clippy, fmt, web-ui
- `.github/workflows/e2e.yml` — end-to-end tests with Docker
- `.github/workflows/release.yml` — multi-platform release

## Verification Checklist

### ci.yml
- [ ] Verify triggers (push to main, PRs) are correct
- [ ] Verify Rust toolchain version is pinned
- [ ] Verify `subversion` apt package is installed (needed for SVN client tests)
- [ ] Verify caching is configured correctly
- [ ] Verify all steps run in correct order

### e2e.yml
- [ ] Verify Docker Compose services match `tests/docker-compose.yml`
- [ ] Verify test environment setup scripts are called
- [ ] Verify log collection on failure
- [ ] Verify timeout settings are reasonable

### release.yml
- [ ] Verify all 4 build targets: x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos
- [ ] Verify binary naming convention
- [ ] Verify GitHub Release creation
- [ ] Verify Docker image push to GHCR
- [ ] **Security**: Verify no secrets leaked in build logs
- [ ] Verify version tag extraction is correct

### General Security
- [ ] Verify all actions use pinned SHA versions (not `@main` or `@v3`)
- [ ] Verify `GITHUB_TOKEN` permissions are minimal
- [ ] Verify no third-party actions with excessive permissions
- [ ] Check for script injection in workflow expressions

## When Problems Are Found

1. Security: `[Fix][Security] CI/CD: <workflow> <issue>`
2. Correctness: `[Fix] CI/CD: <workflow> <issue>`

## Acceptance Criteria

- [ ] All workflows use pinned action versions
- [ ] No secret leakage risks
- [ ] Build matrix covers all targets
- [ ] Test workflows properly collect failure artifacts
ISSUE_BODY
)" \
  "phase:4-infra,model:claude-sonnet,agent:cloud,type:verification,priority:medium,order:19" \
  "cicd"

#---------------------------------------------------------------------------
# Issue 20: Verify Dockerfile and container config
#---------------------------------------------------------------------------
create_issue \
  "[Phase 4] Verify Dockerfile — multi-stage build, security, runtime config" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify the Dockerfile for build correctness, image security, and runtime configuration.

## Recommended Model

**Claude Sonnet** — Dockerfile best practices, container security.

## Execution Environment

**Local agent** — needs Docker to test build.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 4 — Infrastructure             |
| Order       | 20 (can parallel with 19)      |
| Depends on  | #COMPILE                       |
| Blocks      | #E2E                           |

## Files to Verify

- `Dockerfile` — multi-stage build
- `tests/docker-compose.yml` — test environment
- `tests/Dockerfile.svn-server` — SVN test server

## Verification Checklist

- [ ] Verify multi-stage build doesn't leak build tools/source into runtime image
- [ ] Verify runtime image uses minimal base (e.g., distroless, alpine)
- [ ] Verify non-root user is used in runtime
- [ ] Verify all required runtime dependencies are installed (svn, git, SQLite)
- [ ] Verify health check is configured
- [ ] Verify exposed ports match application config
- [ ] Verify volume mounts for persistent data (database, config)
- [ ] **Security**: No secrets baked into image layers
- [ ] Verify `.dockerignore` excludes sensitive files
- [ ] Test: `docker build .` succeeds

## When Problems Are Found

1. Security: `[Fix][Security] Docker: <issue>`
2. Build: `[Fix] Docker: <issue>`

## Acceptance Criteria

- [ ] Build succeeds
- [ ] Runtime image is minimal
- [ ] No secrets in image
- [ ] Non-root user configured
ISSUE_BODY
)" \
  "phase:4-infra,model:claude-sonnet,agent:local,type:verification,priority:medium,order:20" \
  "docker"

#---------------------------------------------------------------------------
# Issue 21: Verify test environment and E2E scripts
#---------------------------------------------------------------------------
create_issue \
  "[Phase 4] Verify E2E test environment — Docker Compose, seed scripts, conflict simulation" \
  "$(cat <<'ISSUE_BODY'
## Summary

Verify the end-to-end test infrastructure: Docker Compose services, seed scripts, and conflict simulation.

## Recommended Model

**Claude Sonnet** — shell script analysis, Docker Compose verification.

## Execution Environment

**Local agent** — needs Docker, docker-compose, svn, git to test scripts.

## Phase & Ordering

| Field        | Value                          |
|-------------|-------------------------------|
| Phase       | 4 — Infrastructure             |
| Order       | 21                             |
| Depends on  | #DOCKER, #DAEMON               |
| Blocks      | Nothing (final verification)   |

## Files to Verify

- `tests/docker-compose.yml` — service definitions
- `tests/Dockerfile.svn-server` — SVN server image
- `tests/svn-apache.conf` — SVN Apache config
- `tests/scripts/setup-test-env.sh` — environment initialization
- `tests/scripts/seed-svn-repo.sh` — SVN test data
- `tests/scripts/seed-git-repo.sh` — Git test data
- `tests/scripts/simulate-conflicts.sh` — conflict scenario creation
- `tests/fixtures/test-config.toml` — test configuration
- `tests/fixtures/authors.toml` — test author mappings

## Verification Checklist

### Shell Scripts
- [ ] All scripts have proper shebang lines
- [ ] All scripts use `set -euo pipefail`
- [ ] All scripts handle errors (check return codes)
- [ ] No hardcoded paths that won't work in CI
- [ ] Scripts are idempotent (safe to re-run)
- [ ] **Security**: No command injection via variables

### Docker Compose
- [ ] All services have health checks
- [ ] Network configuration allows inter-service communication
- [ ] Volume mounts are correct
- [ ] Port mappings don't conflict

### Test Data
- [ ] Seed scripts create representative test data
- [ ] Conflict simulation creates detectable conflicts
- [ ] Test config references correct service hostnames

## When Problems Are Found

1. Fix directly: script bugs, missing error handling
2. Create issues: `[Fix] E2E: <script> <issue>`

## Acceptance Criteria

- [ ] `docker-compose up` starts all services
- [ ] Seed scripts populate test data
- [ ] Conflict simulation creates expected conflicts
- [ ] All scripts are safe and idempotent
ISSUE_BODY
)" \
  "phase:4-infra,model:claude-sonnet,agent:local,type:verification,priority:medium,order:21" \
  "e2e"

###############################################################################
# PHASE 5 — Security & Final Audit
###############################################################################

#---------------------------------------------------------------------------
# Issue 22: Security audit
#---------------------------------------------------------------------------
create_issue \
  "[Phase 5] Full security audit — OWASP Top 10, credential handling, input validation" \
  "$(cat <<'ISSUE_BODY'
## Summary

Comprehensive security audit of the entire codebase, focusing on OWASP Top 10, credential management, and attack surface analysis.

## Recommended Model

**Claude Opus** — deep security analysis, attack vector identification, threat modeling.

## Execution Environment

**Local agent** — needs to run `cargo audit`, check dependencies, test for vulnerabilities.

## Phase & Ordering

| Field        | Value                               |
|-------------|-------------------------------------|
| Phase       | 5 — Security & Final Audit          |
| Order       | 22 (last — after all other issues)  |
| Depends on  | ALL previous issues                 |
| Blocks      | Nothing (final gate)                |

## Verification Checklist

### Dependency Audit
- [ ] Run `cargo audit` — check for known vulnerabilities in dependencies
- [ ] Run `npm audit` in web-ui — check for known vulnerabilities
- [ ] Review dependency tree for unnecessary/risky transitive dependencies
- [ ] Verify all dependencies are from trusted sources

### Credential Management
- [ ] Verify SVN credentials are never logged
- [ ] Verify GitHub tokens are never logged
- [ ] Verify SMTP credentials are never logged
- [ ] Verify Slack webhook URLs are never logged
- [ ] Verify database doesn't store plaintext secrets
- [ ] Verify environment variable resolution doesn't log resolved values
- [ ] Check for credentials in error messages, stack traces, audit logs

### Input Validation (OWASP A03)
- [ ] Verify all API endpoints validate input
- [ ] Verify SVN XML parsing is safe from XXE attacks
- [ ] Verify SQL queries use parameterized statements
- [ ] Verify config file parsing handles malicious input
- [ ] Verify webhook payload size limits

### Authentication & Authorization (OWASP A01, A07)
- [ ] Verify auth middleware covers all protected endpoints
- [ ] Verify session tokens are generated with sufficient entropy
- [ ] Verify password hashing uses bcrypt/argon2 (not MD5/SHA1)
- [ ] Verify OAuth flow follows best practices
- [ ] Verify webhook signature verification is constant-time

### Injection (OWASP A03)
- [ ] Command injection in SVN CLI calls
- [ ] SQL injection in database queries
- [ ] XSS in web UI (user-supplied content rendering)
- [ ] SSRF in webhook URL configuration
- [ ] Path traversal in file operations

### Cryptographic Failures (OWASP A02)
- [ ] Verify HMAC comparison is constant-time
- [ ] Verify HTTPS is enforced for external communications
- [ ] Verify no use of weak hashing algorithms for security purposes

### Security Misconfiguration (OWASP A05)
- [ ] Verify default config doesn't expose sensitive endpoints
- [ ] Verify error messages don't reveal stack traces in production
- [ ] Verify CORS is not wildcard in production
- [ ] Verify security headers are set

## When Problems Are Found

**For every security issue found:**
1. Create a separate issue: `[Fix][Security] <OWASP category>: <description>`
2. Label with `type:fix-required`, `priority:critical`, and `phase:5-security`
3. Include: vulnerability description, affected file/line, severity (Critical/High/Medium/Low), remediation steps
4. Reference this audit issue

## Acceptance Criteria

- [ ] `cargo audit` passes with no known vulnerabilities
- [ ] `npm audit` passes (or all advisories acknowledged)
- [ ] No credential leakage paths found
- [ ] No injection vectors found
- [ ] All findings tracked in separate issues
- [ ] Final security report summarizing all findings
ISSUE_BODY
)" \
  "phase:5-security,model:claude-opus,agent:local,type:verification,priority:critical,order:22" \
  "security"

###############################################################################
# STEP 3: Print Summary with Cross-References
###############################################################################

echo ""
echo "============================================================"
echo "  ALL ISSUES CREATED SUCCESSFULLY"
echo "============================================================"
echo ""
echo "Issue Map (for updating cross-references):"
echo ""
for key in "${!ISSUE_NUMBERS[@]}"; do
  echo "  $key = #${ISSUE_NUMBERS[$key]}"
done
echo ""
echo "Total issues created: ${#ISSUE_NUMBERS[@]}"
echo ""
echo "Phase Execution Order:"
echo ""
echo "  PHASE 0 (Foundation) — Do first, some can run in parallel:"
echo "    Order 01: #${ISSUE_NUMBERS[compile]:-??} — Compile verification"
echo "    Order 02: #${ISSUE_NUMBERS[tests]:-??} — Unit test verification"
echo "    Order 03: #${ISSUE_NUMBERS[webui-build]:-??} — Web UI build (parallel with 01-02)"
echo ""
echo "  PHASE 1 (Core Modules) — After Phase 0, all can run in parallel:"
echo "    Order 04: #${ISSUE_NUMBERS[svn]:-??} — SVN client"
echo "    Order 05: #${ISSUE_NUMBERS[git]:-??} — Git client"
echo "    Order 06: #${ISSUE_NUMBERS[github]:-??} — GitHub API"
echo "    Order 07: #${ISSUE_NUMBERS[db]:-??} — Database"
echo "    Order 08: #${ISSUE_NUMBERS[conflict]:-??} — Conflict engine"
echo "    Order 09: #${ISSUE_NUMBERS[identity]:-??} — Identity mapper"
echo "    Order 10: #${ISSUE_NUMBERS[config]:-??} — Config system"
echo "    Order 11: #${ISSUE_NUMBERS[errors-notify]:-??} — Errors & notifications"
echo ""
echo "  PHASE 2 (Integration) — After Phase 1:"
echo "    Order 12: #${ISSUE_NUMBERS[sync-engine]:-??} — Sync engine (CRITICAL)"
echo "    Order 13: #${ISSUE_NUMBERS[daemon]:-??} — Daemon"
echo "    Order 14: #${ISSUE_NUMBERS[cli]:-??} — CLI (parallel with 13)"
echo ""
echo "  PHASE 3 (API & Web) — After Phase 2:"
echo "    Order 15: #${ISSUE_NUMBERS[api]:-??} — REST API"
echo "    Order 16: #${ISSUE_NUMBERS[webhooks]:-??} — Webhooks (parallel with 15)"
echo "    Order 17: #${ISSUE_NUMBERS[ws]:-??} — WebSocket (parallel with 15-16)"
echo "    Order 18: #${ISSUE_NUMBERS[webui-verify]:-??} — Web UI"
echo ""
echo "  PHASE 4 (Infrastructure) — After Phase 2:"
echo "    Order 19: #${ISSUE_NUMBERS[cicd]:-??} — CI/CD"
echo "    Order 20: #${ISSUE_NUMBERS[docker]:-??} — Docker (parallel with 19)"
echo "    Order 21: #${ISSUE_NUMBERS[e2e]:-??} — E2E tests"
echo ""
echo "  PHASE 5 (Security) — After ALL others:"
echo "    Order 22: #${ISSUE_NUMBERS[security]:-??} — Security audit"
echo ""
echo "============================================================"
echo "  AGENT ASSIGNMENT SUMMARY"
echo "============================================================"
echo ""
echo "  LOCAL agents (need filesystem, Docker, CLI tools):"
echo "    - #${ISSUE_NUMBERS[compile]:-??}, #${ISSUE_NUMBERS[tests]:-??}, #${ISSUE_NUMBERS[webui-build]:-??}"
echo "    - #${ISSUE_NUMBERS[svn]:-??}, #${ISSUE_NUMBERS[git]:-??}, #${ISSUE_NUMBERS[db]:-??}"
echo "    - #${ISSUE_NUMBERS[sync-engine]:-??}, #${ISSUE_NUMBERS[daemon]:-??}, #${ISSUE_NUMBERS[cli]:-??}"
echo "    - #${ISSUE_NUMBERS[docker]:-??}, #${ISSUE_NUMBERS[e2e]:-??}, #${ISSUE_NUMBERS[security]:-??}"
echo ""
echo "  CLOUD agents (GitHub Copilot / remote code review):"
echo "    - #${ISSUE_NUMBERS[github]:-??}, #${ISSUE_NUMBERS[conflict]:-??}"
echo "    - #${ISSUE_NUMBERS[identity]:-??}, #${ISSUE_NUMBERS[config]:-??}, #${ISSUE_NUMBERS[errors-notify]:-??}"
echo "    - #${ISSUE_NUMBERS[api]:-??}, #${ISSUE_NUMBERS[webhooks]:-??}, #${ISSUE_NUMBERS[ws]:-??}"
echo "    - #${ISSUE_NUMBERS[webui-verify]:-??}, #${ISSUE_NUMBERS[cicd]:-??}"
echo ""
echo "  MODEL RECOMMENDATIONS:"
echo "    Claude Haiku (fast, cheap):  #${ISSUE_NUMBERS[compile]:-??}, #${ISSUE_NUMBERS[webui-build]:-??}, #${ISSUE_NUMBERS[errors-notify]:-??}"
echo "    Claude Sonnet (balanced):    #${ISSUE_NUMBERS[tests]:-??}, #${ISSUE_NUMBERS[github]:-??}, #${ISSUE_NUMBERS[db]:-??}, #${ISSUE_NUMBERS[identity]:-??}, #${ISSUE_NUMBERS[config]:-??}, #${ISSUE_NUMBERS[daemon]:-??}, #${ISSUE_NUMBERS[cli]:-??}, #${ISSUE_NUMBERS[ws]:-??}, #${ISSUE_NUMBERS[webui-verify]:-??}, #${ISSUE_NUMBERS[cicd]:-??}, #${ISSUE_NUMBERS[docker]:-??}, #${ISSUE_NUMBERS[e2e]:-??}"
echo "    Claude Opus (deep analysis): #${ISSUE_NUMBERS[svn]:-??}, #${ISSUE_NUMBERS[git]:-??}, #${ISSUE_NUMBERS[conflict]:-??}, #${ISSUE_NUMBERS[sync-engine]:-??}, #${ISSUE_NUMBERS[api]:-??}, #${ISSUE_NUMBERS[webhooks]:-??}, #${ISSUE_NUMBERS[security]:-??}"
echo ""
echo "Done! Review issues at: https://github.com/chriscase/GitSvnSync/issues"

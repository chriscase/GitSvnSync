# RepoSync Known Issues

## Critical

### 1. ~~Data not scoped by repository~~ FIXED
- ~~Commit Map, Sync Records, Audit Log showed entries from ALL repos~~
- **Fixed**: Backend already had `?repo_id=` support. Frontend RepoDetail.tsx now passes `id` to all API calls
- **Note**: Dashboard aggregate view still shows all repos (by design) — could use dropdown filter

### 2. ~~Import is global, not per-repo~~ FIXED
- ~~Cannot trigger import for a specific repository~~
- **Fixed**: Added `POST /api/repos/:id/import` endpoint
- **Validated**: Sandbox import completed 250/250 commits via per-repo endpoint

### 3. Sync trigger is stubbed
- `POST /api/repos/:id/sync` only logs, doesn't actually run sync
- **Fix**: Wire to sync engine or at minimum return useful status

### 4. Setup Wizard overwrites TOML config
- Saving the wizard regenerates the entire TOML file, destroying data_dir, watermarks, and other settings
- Password saving through wizard is broken (dirty flag bug)
- "Exit to Dashboard" used to navigate to /login (partially fixed)
- **Fix**: Remove wizard dependency, manage repos via /repos page only

### 5. Watermark auto-detection reads wrong table — IN PROGRESS
- Sync engine reads `last_svn_rev` from `kv_state` which gets wiped
- **Fixing**: Moving watermarks to `repositories` table columns (last_svn_rev, last_git_sha)
- **Fixing**: Dual-write to both repo table and kv_state for backward compat
- **Fixing**: Auto-detect from git history on startup when watermark is 0
- **Fixing**: Remove TOML generation from apply_config that destroys config

## UX Issues

### 6. No per-repo credential management on repo detail page
- SVN password and Git token fields missing from inline editor
- No "Test Connection" buttons on repo detail or add-repo modal
- Credentials can only be set via API or broken wizard

### 7. ~~Dashboard repo filter doesn't filter data~~ FIXED
- ~~Sync records, commit map, and audit log showed unfiltered data~~
- **Fixed**: Dashboard already passes `activeRepoId` to all API calls with proper query keys
- **Note**: Need to verify visually via Chrome agent

### 8. Server Monitor shows "Loading metrics..." until auth
- The `/api/status/system` endpoint requires auth but ServerMonitor doesn't always have the token
- Sometimes shows perpetual loading state

### 9. Audit log only shows errors
- Successful sync cycles weren't logged to audit_log (fixed in server agent commit)
- But still need validation that success entries appear

### 10. Identical activity entries not collapsed
- Consecutive same-action audit entries should be grouped/collapsed (code exists but needs validation)

### 11. Credential hot-reload not validated
- `reload_credentials()` in sync_engine.rs reads from kv_state before each cycle
- Not clear if per-repo keys (`secret_svn_password_{repo_id}`) are used by scheduler

## Infrastructure

### 12. TOML file should be minimal bootstrap only
- Currently contains SVN/Git/Sync config that should live in the DB
- Wizard overwrites it, destroying working config
- **Fix**: TOML should only have daemon.data_dir, web.listen, daemon.log_level

### 13. No graceful shutdown
- Force-kill (kill -9) corrupts SQLite WAL
- Need SIGTERM handler that checkpoints WAL and exits cleanly

### 14. Session expiry during wizard flow
- Long wizard flows can result in 401, which redirects to login (partially fixed for /setup)

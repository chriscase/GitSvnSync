# RepoSync Known Issues

## Resolved

### 1. ~~Data not scoped by repository~~ FIXED
- Frontend RepoDetail.tsx now passes repo_id to all API calls

### 2. ~~Import is global, not per-repo~~ FIXED
- Added `POST /api/repos/:id/import` endpoint, validated with 250+500 commit sandbox imports

### 3. ~~Scheduler only syncs global repo~~ FIXED
- Per-repo scheduler creates SyncEngine for each enabled repo
- SVN→Git and Git→SVN both working (verified on Large Test repo)

### 4. ~~Setup Wizard overwrites TOML config~~ FIXED
- TOML generation removed from apply_config
- /setup redirects to /repos
- Setup removed from navigation

### 5. ~~Watermark auto-detection reads wrong table~~ FIXED
- Watermarks now stored as columns on repositories table (last_svn_rev, last_git_sha)
- Dual-write to both repo table and kv_state for backward compat
- Auto-detect from git history on daemon startup when watermark is 0

### 6. ~~No per-repo credential management~~ FIXED
- SVN password and Git token fields on repo detail edit form
- Test Connection buttons on repo detail
- POST /api/repos/:id/credentials endpoint

### 7. ~~Dashboard repo filter doesn't filter~~ FIXED
- Dashboard passes activeRepoId to all API calls with proper query keys

### 8. ~~Server Monitor auth~~ FIXED
- getSystemMetrics uses fetchJson which includes Authorization header

### 9. ~~Audit log only shows errors~~ FIXED
- Successful sync cycles now logged to audit_log

### 12. ~~TOML file overwrite~~ FIXED
- TOML generation removed, config lives in DB repositories table

### 13. ~~No graceful shutdown~~ FIXED
- SIGTERM handler with WAL checkpoint on shutdown
- stop-daemon.sh script for graceful stop

## Active

### 10. Activity grouping/collapsing
- Consecutive same-action audit entries should be collapsed
- Code exists but needs visual validation

### 11. Per-repo credential hot-reload
- reload_credentials() reads from kv_state with per-repo keys
- Working for Large repo, needs broader validation

### 14. Session expiry UX
- 401 redirects to /login, clearing credentials
- Could be improved with a toast notification instead of hard redirect
- Partially mitigated: login page doesn't re-redirect

### 15. SVN commit revision parsing (NEW — from sandbox testing)
- Sandbox repo Git→SVN fails: "could not parse committed revision"
- svn commit exits 0 but output format differs from expected
- Fixed with case-insensitive parsing and fallback patterns
- Needs re-validation after deploy

### 16. LFS not tracked during per-repo import
- lfs_unique_count shows 0 on both sandbox imports despite >1MB files
- FilePolicy may not be reading lfs_threshold_mb from repo config correctly
- Needs investigation

### 17. Status endpoint shows global data when repo filtered
- /api/status returns global sync engine state
- When filtering by repo on dashboard, status cards show wrong data (e.g., r3078 for sandbox)
- Needs per-repo status from repositories table

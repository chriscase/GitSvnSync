# Personal Branch Mode: Configuration Reference

Personal Branch Mode is configured via a TOML file. The default location is:

```
~/.config/gitsvnsync/personal.toml
```

You can override the config file path with the `--config` flag on any command:

```bash
gitsvnsync personal start --config /path/to/my-config.toml
```

Or set the `GITSVNSYNC_CONFIG` environment variable:

```bash
export GITSVNSYNC_CONFIG="/path/to/my-config.toml"
```

## Full Configuration File

```toml
[personal]
data_dir = "~/.local/share/gitsvnsync"
poll_interval_secs = 30

[svn]
url = "https://svn.company.com/repos/project/trunk"
username = "jdoe"
# Use ONE of the following (password_env is recommended):
# password = "secret"
password_env = "SVN_PASSWORD"

[github]
api_url = "https://api.github.com"
# Use ONE of the following (token_env is recommended):
# token = "ghp_xxxx"
token_env = "GITHUB_TOKEN"
repo = "jdoe/project-mirror"

[developer]
name = "John Doe"
email = "jdoe@company.com"
svn_username = "jdoe"

[commit_format]
svn_to_git_template = "{original_message}\n\nSVN-Revision: r{svn_rev}\nSVN-Author: {svn_author}\nSVN-Date: {svn_date}"
git_to_svn_template = "{original_message}\n\nGit-Commit: {git_sha}\nPR: #{pr_number} ({pr_branch})"

[options]
normalize_line_endings = false
sync_executable_bit = true
max_file_size = 52428800
ignore_patterns = []
sync_direct_pushes = false
```

---

## Section Reference

### [personal]

General settings for the Personal Branch Mode daemon.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `data_dir` | string | `"~/.local/share/gitsvnsync"` | Directory where GitSvnSync stores its SQLite database, cloned repositories, and sync state. Tilde (`~`) is expanded to your home directory. |
| `poll_interval_secs` | integer | `30` | How often (in seconds) the daemon checks SVN for new revisions and GitHub for merged PRs. Lower values give faster sync but increase load on both servers. |

### [svn]

Connection details for the SVN repository you want to mirror.

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `url` | string | yes | -- | Full SVN URL to mirror. This should point to the specific path you want (e.g., `trunk`, a branch, or the repo root). |
| `username` | string | yes | -- | Your SVN username. |
| `password` | string | no | -- | SVN password in plaintext. **Not recommended** -- use `password_env` instead. If both are set, `password` takes precedence. |
| `password_env` | string | no | -- | Name of an environment variable that contains your SVN password. This is the recommended approach. |

You must provide either `password` or `password_env`. If neither is set, the daemon will fail to start.

### [github]

Connection details for the GitHub repository used as the Git mirror.

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `api_url` | string | no | `"https://api.github.com"` | GitHub API base URL. Use `https://api.github.com` for GitHub.com. For GitHub Enterprise Server, use `https://github.yourcompany.com/api/v3`. |
| `token` | string | no | -- | GitHub Personal Access Token in plaintext. **Not recommended** -- use `token_env` instead. Requires `repo` scope. If both are set, `token` takes precedence. |
| `token_env` | string | no | -- | Name of an environment variable that contains your GitHub token. This is the recommended approach. |
| `repo` | string | yes | -- | Target GitHub repository in `owner/name` format (e.g., `jdoe/project-mirror`). |

You must provide either `token` or `token_env`. If neither is set, the daemon will fail to start.

### [developer]

Your identity for commits created by the sync process.

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `name` | string | yes | -- | Your full name, used as the Git commit author name when syncing SVN commits to Git. |
| `email` | string | yes | -- | Your email address, used as the Git commit author email. |
| `svn_username` | string | yes | -- | Your SVN username. Used to identify which SVN commits are yours when syncing. Should match the `username` in `[svn]` for single-user setups. |

### [commit_format]

Templates that control how commit messages are formatted when crossing between systems.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `svn_to_git_template` | string | (see below) | Template for Git commit messages created from SVN revisions. |
| `git_to_svn_template` | string | (see below) | Template for SVN commit messages created from merged Git PRs. |

#### SVN-to-Git Template Placeholders

| Placeholder | Description |
|-------------|-------------|
| `{original_message}` | The original SVN commit log message |
| `{svn_rev}` | The SVN revision number (e.g., `847`) |
| `{svn_author}` | The SVN commit author username |
| `{svn_date}` | The SVN commit timestamp in ISO 8601 format |

**Default template:**

```
{original_message}

SVN-Revision: r{svn_rev}
SVN-Author: {svn_author}
SVN-Date: {svn_date}
```

#### Git-to-SVN Template Placeholders

| Placeholder | Description |
|-------------|-------------|
| `{original_message}` | The original Git commit message (or PR merge commit message) |
| `{git_sha}` | The full Git commit SHA |
| `{pr_number}` | The GitHub PR number that was merged |
| `{pr_branch}` | The source branch name of the merged PR |

**Default template:**

```
{original_message}

Git-Commit: {git_sha}
PR: #{pr_number} ({pr_branch})
```

### [options]

Behavioral options that control how files and changes are synced.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `normalize_line_endings` | boolean | `false` | When `true`, convert line endings to match the target system's convention (LF for Git, native for SVN). When `false`, line endings are preserved as-is. |
| `sync_executable_bit` | boolean | `true` | Sync the executable permission bit (`svn:executable` property) between SVN and Git. |
| `max_file_size` | integer | `52428800` | Maximum file size in bytes (default 50 MB). Files larger than this are skipped during sync with a warning. Set to `0` to disable the limit. |
| `ignore_patterns` | array of strings | `[]` | List of glob patterns for files to exclude from sync. Patterns are matched against paths relative to the repository root. Example: `["*.log", "build/**", ".idea/**"]` |
| `sync_direct_pushes` | boolean | `false` | When `false` (the default), only merged PRs are synced from Git to SVN. When `true`, any push to the default branch is synced, including direct pushes without a PR. |

---

## Environment Variables

Fields ending in `_env` tell GitSvnSync to read the actual value from an environment variable at startup. This keeps secrets out of config files.

| Config Field | Recommended Env Var | Description |
|--------------|-------------------|-------------|
| `svn.password_env` | `SVN_PASSWORD` | Your SVN password |
| `github.token_env` | `GITHUB_TOKEN` | Your GitHub Personal Access Token |

You can use any environment variable name you want. The config field value is the **name** of the variable, not the secret itself.

**Example:** if your config contains `password_env = "MY_SVN_PASS"`, then set the variable:

```bash
export MY_SVN_PASS="my-actual-password"
```

### Setting Environment Variables

Add these to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.) so they persist across sessions:

```bash
# GitSvnSync secrets
export SVN_PASSWORD="your-svn-password"
export GITHUB_TOKEN="ghp_xxxxxxxxxxxxxxxxxxxx"
```

Reload your shell or run `source ~/.zshrc` before starting the daemon.

---

## Config File Location

The daemon searches for the config file in this order:

1. Path specified by `--config` flag
2. Path in the `GITSVNSYNC_CONFIG` environment variable
3. `~/.config/gitsvnsync/personal.toml` (default)

The `gitsvnsync personal init` wizard writes to the default location. The parent directory is created automatically if it does not exist.

---

## Example Configurations

### GitHub.com (minimal)

The simplest setup for mirroring an SVN trunk to a personal GitHub.com repo:

```toml
[personal]
poll_interval_secs = 30

[svn]
url = "https://svn.company.com/repos/project/trunk"
username = "jdoe"
password_env = "SVN_PASSWORD"

[github]
repo = "jdoe/project-mirror"
token_env = "GITHUB_TOKEN"

[developer]
name = "John Doe"
email = "jdoe@company.com"
svn_username = "jdoe"
```

All other fields use their defaults. The `api_url` defaults to `https://api.github.com`.

### GitHub Enterprise Server

For organizations running GitHub Enterprise on their own infrastructure:

```toml
[personal]
data_dir = "~/.local/share/gitsvnsync"
poll_interval_secs = 15

[svn]
url = "https://svn.internal.corp/repos/main-product/trunk"
username = "john.doe"
password_env = "SVN_PASSWORD"

[github]
api_url = "https://github.corp.com/api/v3"
repo = "john-doe/main-product-mirror"
token_env = "GHE_TOKEN"

[developer]
name = "John Doe"
email = "john.doe@corp.com"
svn_username = "john.doe"

[options]
ignore_patterns = [".svn/**", "*.class", "target/**"]
max_file_size = 104857600  # 100 MB
```

Note the custom `api_url` pointing to the Enterprise API endpoint.

### Custom SVN Layout

Some SVN repositories do not follow the standard `trunk/branches/tags` layout. If your project lives at a non-standard path, point the `url` directly to it:

```toml
[svn]
# Mirror only the "main" directory (no standard trunk/branches/tags)
url = "https://svn.company.com/repos/products/widget/main"
username = "jdoe"
password_env = "SVN_PASSWORD"
```

If your repository uses alternate names for trunk (e.g., the project root is the working directory itself, or the trunk is named something else), simply set the `url` to the exact path you want to sync.

### Aggressive Polling with Custom Commit Format

For workflows where low latency matters and you want minimal metadata in commit messages:

```toml
[personal]
poll_interval_secs = 10

[svn]
url = "https://svn.company.com/repos/project/trunk"
username = "jdoe"
password_env = "SVN_PASSWORD"

[github]
repo = "jdoe/project-mirror"
token_env = "GITHUB_TOKEN"

[developer]
name = "John Doe"
email = "jdoe@company.com"
svn_username = "jdoe"

[commit_format]
svn_to_git_template = "{original_message}\n\n(svn r{svn_rev})"
git_to_svn_template = "{original_message}\n\n(from {git_sha})"

[options]
sync_direct_pushes = true
normalize_line_endings = true
```

### Filtered Sync with Ignore Patterns

Exclude build artifacts, IDE files, and large binaries from sync:

```toml
[personal]
poll_interval_secs = 60

[svn]
url = "https://svn.company.com/repos/project/trunk"
username = "jdoe"
password_env = "SVN_PASSWORD"

[github]
repo = "jdoe/project-mirror"
token_env = "GITHUB_TOKEN"

[developer]
name = "John Doe"
email = "jdoe@company.com"
svn_username = "jdoe"

[options]
ignore_patterns = [
    "build/**",
    "dist/**",
    ".idea/**",
    "*.iml",
    "*.class",
    "*.jar",
    "*.war",
    "node_modules/**",
    "*.log",
]
max_file_size = 26214400  # 25 MB
sync_executable_bit = false
```

---

## Default Values Summary

For quick reference, here are all fields and their defaults:

| Field | Default |
|-------|---------|
| `personal.data_dir` | `"~/.local/share/gitsvnsync"` |
| `personal.poll_interval_secs` | `30` |
| `svn.url` | *(required, no default)* |
| `svn.username` | *(required, no default)* |
| `svn.password` | *(none)* |
| `svn.password_env` | *(none)* |
| `github.api_url` | `"https://api.github.com"` |
| `github.token` | *(none)* |
| `github.token_env` | *(none)* |
| `github.repo` | *(required, no default)* |
| `developer.name` | *(required, no default)* |
| `developer.email` | *(required, no default)* |
| `developer.svn_username` | *(required, no default)* |
| `commit_format.svn_to_git_template` | `"{original_message}\n\nSVN-Revision: r{svn_rev}\nSVN-Author: {svn_author}\nSVN-Date: {svn_date}"` |
| `commit_format.git_to_svn_template` | `"{original_message}\n\nGit-Commit: {git_sha}\nPR: #{pr_number} ({pr_branch})"` |
| `options.normalize_line_endings` | `false` |
| `options.sync_executable_bit` | `true` |
| `options.max_file_size` | `52428800` (50 MB) |
| `options.ignore_patterns` | `[]` |
| `options.sync_direct_pushes` | `false` |

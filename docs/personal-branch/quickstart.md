# Personal Branch Mode: 5-Minute Quickstart

Personal Branch Mode lets a single developer mirror an SVN repository to a personal GitHub repo. You work in Git with branches and PRs, and GitSvnSync handles syncing changes back to SVN when PRs are merged. No server required -- it runs as a local daemon on your machine.

## Prerequisites

Before you begin, make sure you have:

- **SVN CLI** (`svn`) version 1.8+ installed and on your PATH
- **Git** version 2.20+ installed
- **Rust toolchain** (if building from source) -- install via [rustup](https://rustup.rs/)
- **A GitHub account** with a repository to use as the mirror (or the daemon will create one)
- **A GitHub Personal Access Token** with `repo` scope -- [create one here](https://github.com/settings/tokens)
- **SVN credentials** for the repository you want to mirror

## Step 1: Install

### Option A: Install from crates.io

```bash
cargo install gitsvnsync-cli
```

### Option B: Build from source

```bash
git clone https://github.com/chriscase/GitSvnSync.git
cd GitSvnSync
cargo build --release
# Copy the binary to your PATH
cp target/release/gitsvnsync ~/.cargo/bin/
```

Verify the installation:

```bash
gitsvnsync --version
```

## Step 2: Initialize Configuration

Run the interactive setup wizard:

```bash
gitsvnsync personal init
```

The wizard will prompt you for:

1. **SVN repository URL** -- the full URL to the path you want to mirror (e.g., `https://svn.company.com/repos/project/trunk`)
2. **SVN username** -- your SVN login
3. **SVN password** -- stored in an environment variable (the wizard will tell you which one to set)
4. **GitHub token** -- your Personal Access Token with `repo` scope
5. **GitHub repository** -- in `owner/repo` format (e.g., `jdoe/project-mirror`)
6. **Poll interval** -- how often to check SVN for new commits (default: 30 seconds)

The wizard writes its configuration to `~/.config/gitsvnsync/personal.toml`. You can edit this file directly afterward.

**Set your secrets as environment variables** (add these to your shell profile):

```bash
export SVN_PASSWORD="your-svn-password"
export GITHUB_TOKEN="ghp_xxxxxxxxxxxxxxxxxxxx"
```

## Step 3: Import SVN History

Pull the full SVN history into your GitHub mirror:

```bash
gitsvnsync personal import --full
```

This clones the SVN repository, converts the history to Git commits, and pushes everything to your GitHub repository. Depending on the size of your SVN history, this may take anywhere from a few seconds to several hours.

You will see progress output as revisions are converted:

```
Importing SVN history...
  r1 -> git abc1234 (Initial project structure)
  r2 -> git def5678 (Add build system)
  ...
  r847 -> git 9fa3b21 (Latest commit)
Import complete: 847 revisions -> 847 commits
Pushed to github.com/jdoe/project-mirror
```

## Step 4: Start the Sync Daemon

Start the background sync process:

```bash
gitsvnsync personal start
```

The daemon runs in the foreground by default. To run it in the background:

```bash
gitsvnsync personal start --daemon
```

Check that it is running:

```bash
gitsvnsync personal status
```

You should see output like:

```
Personal sync daemon: running (PID 12345)
SVN URL:    https://svn.company.com/repos/project/trunk
GitHub:     jdoe/project-mirror
Last SVN poll:  2 seconds ago
Last sync:      r847 <-> abc1234
```

To stop the daemon later:

```bash
gitsvnsync personal stop
```

## Step 5: Work with Branches and PRs

Now that sync is running, use your normal Git workflow:

### 1. Clone your mirror repository

```bash
git clone git@github.com:jdoe/project-mirror.git
cd project-mirror
```

### 2. Create a feature branch

```bash
git checkout -b feature/add-login-page
```

### 3. Make changes and commit

```bash
# Edit files...
git add .
git commit -m "Add login page with form validation"
```

### 4. Push and open a Pull Request

```bash
git push -u origin feature/add-login-page
```

Open a PR on GitHub through the web UI or the CLI:

```bash
gh pr create --title "Add login page" --body "Implements the login form with validation"
```

### 5. Merge the PR

Once you are satisfied, merge the PR on GitHub (via the web UI or CLI). GitSvnSync detects the merge and commits the changes to SVN automatically.

### 6. Watch it sync

Within 30 seconds (or your configured poll interval), the merged changes appear as a new SVN revision:

```
SVN r848: Add login page with form validation
  Git-Commit: 7e2f9a1
  PR: #12 (feature/add-login-page)
  Merged-By: jdoe
```

Meanwhile, if someone else commits directly to SVN, those changes appear in your GitHub repo's `main` branch within the same poll interval.

## What to Expect

- **First sync latency**: new SVN commits and merged PRs sync within your configured poll interval (default 30 seconds).
- **SVN commit metadata**: when Git commits are synced to SVN, the SVN commit message includes trailers with the Git SHA, PR number, and branch name.
- **Git commit metadata**: when SVN commits are synced to Git, the Git commit message includes trailers with the SVN revision number, author, and timestamp.
- **Only merged PRs are synced to SVN**: direct pushes to the default branch are ignored. This prevents work-in-progress pushes from polluting SVN. (The `sync_direct_pushes` option exists in the config schema but is **not yet implemented**; the daemon rejects `sync_direct_pushes = true` at startup.)
- **No SVN server changes needed**: Personal Branch Mode uses your regular SVN credentials. There are no hooks or admin access required on the SVN side.

## Next Steps

- Read the full [Configuration Reference](configuration.md) to customize commit templates, ignore patterns, and more.
- See the main [Troubleshooting Guide](../troubleshooting.md) if anything goes wrong.

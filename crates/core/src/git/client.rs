//! Local Git repository operations via `git2`.

use std::path::{Path, PathBuf};

use git2::{
    BranchType, Cred, FetchOptions, IndexAddOption, Oid, PushOptions, RemoteCallbacks,
    Repository, Signature,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

use crate::errors::GitError;

/// High-level Git client wrapping a `git2::Repository`.
pub struct GitClient {
    repo: Repository,
    repo_path: PathBuf,
}

/// Information about a single Git commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitInfo {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
    pub committer_name: String,
    pub committer_email: String,
}

impl GitClient {
    /// Open an existing Git repository at `repo_path`.
    pub fn new<P: AsRef<Path>>(repo_path: P) -> Result<Self, GitError> {
        let path = repo_path.as_ref();
        info!(path = %path.display(), "opening git repository");
        let repo = Repository::open(path)
            .map_err(|_| GitError::RepositoryNotFound(path.display().to_string()))?;
        Ok(Self { repo, repo_path: path.to_path_buf() })
    }

    /// Clone a remote repository to `path`.
    #[instrument(skip(token), fields(url = %url, path = %path.display()))]
    pub fn clone_repo(url: &str, path: &Path, token: Option<&str>) -> Result<Self, GitError> {
        info!("cloning git repository");
        let mut callbacks = RemoteCallbacks::new();
        if let Some(tok) = token {
            let tok = tok.to_string();
            callbacks.credentials(move |_url, _username, _allowed| {
                Cred::userpass_plaintext("x-access-token", &tok)
            });
        }
        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);
        let repo = builder.clone(url, path)?;
        info!("clone completed");
        Ok(Self { repo, repo_path: path.to_path_buf() })
    }

    pub fn repo_path(&self) -> &Path { &self.repo_path }
    pub fn repo(&self) -> &Repository { &self.repo }

    /// Fetch from a named remote.
    #[instrument(skip(self, token))]
    pub fn fetch(&self, remote_name: &str, token: Option<&str>) -> Result<(), GitError> {
        info!(remote = remote_name, "fetching");
        let mut remote = self.repo.find_remote(remote_name)?;
        let mut callbacks = RemoteCallbacks::new();
        if let Some(tok) = token {
            let tok = tok.to_string();
            callbacks.credentials(move |_url, _username, _allowed| {
                Cred::userpass_plaintext("x-access-token", &tok)
            });
        }
        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)?;
        debug!("fetch completed");
        Ok(())
    }

    /// Fetch and fast-forward merge.
    #[instrument(skip(self, token))]
    pub fn pull(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<(), GitError> {
        self.fetch(remote_name, token)?;
        let fetch_head_ref = format!("refs/remotes/{}/{}", remote_name, branch);
        let fetch_commit = self.repo.find_reference(&fetch_head_ref)?.peel_to_commit()?;
        let head_ref = self.repo.head()?;
        if head_ref.is_branch() {
            let mut head_ref_mut = self.repo.find_reference(head_ref.name().unwrap_or("HEAD"))?;
            head_ref_mut.set_target(fetch_commit.id(), "gitsvnsync: fast-forward pull")?;
            self.repo.set_head(head_ref.name().unwrap_or("HEAD"))?;
            self.repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        }
        info!("pull completed");
        Ok(())
    }

    /// Stage all changes and create a commit.
    #[instrument(skip(self, message))]
    pub fn commit(
        &self, message: &str, author_name: &str, author_email: &str,
        committer_name: &str, committer_email: &str,
    ) -> Result<Oid, GitError> {
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let tree_oid = index.write_tree()?;
        let tree = self.repo.find_tree(tree_oid)?;
        let author = Signature::now(author_name, author_email)?;
        let committer = Signature::now(committer_name, committer_email)?;
        let parent_commit = match self.repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None,
        };
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        let oid = self.repo.commit(Some("HEAD"), &author, &committer, message, &tree, &parents)?;
        info!(sha = %oid, "created commit");
        Ok(oid)
    }

    /// Push a local branch to a remote.
    #[instrument(skip(self, token))]
    pub fn push(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<(), GitError> {
        info!(remote = remote_name, branch, "pushing");
        let mut remote = self.repo.find_remote(remote_name)?;
        let mut callbacks = RemoteCallbacks::new();
        if let Some(tok) = token {
            let tok = tok.to_string();
            callbacks.credentials(move |_url, _username, _allowed| {
                Cred::userpass_plaintext("x-access-token", &tok)
            });
        }
        let push_error = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let push_error_clone = push_error.clone();
        callbacks.push_update_reference(move |refname, status| {
            if let Some(msg) = status {
                warn!(refname, msg, "push rejected");
                *push_error_clone.lock().unwrap() = Some(msg.to_string());
            }
            Ok(())
        });
        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        remote.push(&[&refspec], Some(&mut push_opts))?;
        if let Some(err_msg) = push_error.lock().unwrap().take() {
            return Err(GitError::PushRejected { branch: branch.to_string(), detail: err_msg });
        }
        info!("push completed");
        Ok(())
    }

    /// Return the SHA of HEAD.
    pub fn get_head_sha(&self) -> Result<String, GitError> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        Ok(commit.id().to_string())
    }

    /// Walk commits from HEAD backwards until we reach `since_sha`.
    pub fn get_commits_since(&self, since_sha: Option<&str>) -> Result<Vec<GitCommitInfo>, GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        let since_oid = since_sha.map(|s| Oid::from_str(s)).transpose()?;
        let mut commits = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result?;
            if Some(oid) == since_oid { break; }
            let commit = self.repo.find_commit(oid)?;
            commits.push(GitCommitInfo {
                sha: oid.to_string(),
                message: commit.message().unwrap_or("").to_string(),
                author_name: commit.author().name().unwrap_or("").to_string(),
                author_email: commit.author().email().unwrap_or("").to_string(),
                author_time: commit.author().when().seconds(),
                committer_name: commit.committer().name().unwrap_or("").to_string(),
                committer_email: commit.committer().email().unwrap_or("").to_string(),
            });
            if commits.len() >= 1000 {
                warn!("reached 1000 commit limit");
                break;
            }
        }
        debug!(count = commits.len(), "collected commits");
        Ok(commits)
    }

    /// Create a new branch pointing at `from_sha`.
    #[instrument(skip(self))]
    pub fn create_branch(&self, name: &str, from_sha: &str) -> Result<(), GitError> {
        let oid = Oid::from_str(from_sha)?;
        let commit = self.repo.find_commit(oid)?;
        self.repo.branch(name, &commit, false)?;
        info!(name, from_sha, "created branch");
        Ok(())
    }

    /// Delete a local branch.
    #[instrument(skip(self))]
    pub fn delete_branch(&self, name: &str) -> Result<(), GitError> {
        let mut branch = self.repo.find_branch(name, BranchType::Local)?;
        branch.delete()?;
        info!(name, "deleted branch");
        Ok(())
    }

    /// List all local branch names.
    pub fn list_branches(&self) -> Result<Vec<String>, GitError> {
        let branches = self.repo.branches(Some(BranchType::Local))?;
        let mut names = Vec::new();
        for branch_result in branches {
            let (branch, _) = branch_result?;
            if let Some(name) = branch.name()? { names.push(name.to_string()); }
        }
        Ok(names)
    }

    /// Apply a unified diff to the working tree.
    #[instrument(skip(self, diff_content))]
    pub async fn apply_diff(&self, diff_content: &str) -> Result<(), GitError> {
        use tokio::process::Command;
        use std::process::Stdio;
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_path)
            .args(["apply", "--3way", "-"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(GitError::IoError)?;
        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(diff_content.as_bytes()).await.map_err(GitError::IoError)?;
        }
        let output = child.wait_with_output().await.map_err(GitError::IoError)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            warn!(%stderr, "git apply failed");
            return Err(GitError::ApplyFailed(stderr));
        }
        info!("diff applied successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        Repository::init(dir.path()).unwrap();
        let client = GitClient::new(dir.path()).unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
        let oid = client.commit("initial commit", "Test", "test@test.com", "Test", "test@test.com").unwrap();
        assert!(!oid.is_zero());
        assert_eq!(client.get_head_sha().unwrap(), oid.to_string());
    }

    #[test]
    fn test_create_and_delete_branch() {
        let dir = tempfile::tempdir().unwrap();
        Repository::init(dir.path()).unwrap();
        let client = GitClient::new(dir.path()).unwrap();
        std::fs::write(dir.path().join("f.txt"), "c").unwrap();
        let oid = client.commit("init", "T", "t@t.com", "T", "t@t.com").unwrap();
        client.create_branch("feature", &oid.to_string()).unwrap();
        assert!(client.list_branches().unwrap().contains(&"feature".to_string()));
        client.delete_branch("feature").unwrap();
        assert!(!client.list_branches().unwrap().contains(&"feature".to_string()));
    }

    #[test]
    fn test_repo_not_found() {
        assert!(matches!(GitClient::new("/nonexistent"), Err(GitError::RepositoryNotFound(_))));
    }
}

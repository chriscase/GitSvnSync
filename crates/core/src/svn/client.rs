//! Asynchronous SVN CLI client.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;
use tracing::{debug, info, instrument, warn};

use super::parser::{
    parse_svn_diff_summarize, parse_svn_info, parse_svn_log, SvnDiffEntry, SvnInfo, SvnLogEntry,
};
use crate::errors::SvnError;

/// Asynchronous client for interacting with an SVN repository via the CLI.
#[derive(Debug, Clone)]
pub struct SvnClient {
    url: String,
    username: String,
    password: String,
}

impl SvnClient {
    /// Create a new SVN client targeting `url` with the given credentials.
    pub fn new(
        url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let client = Self {
            url: url.into(),
            username: username.into(),
            password: password.into(),
        };
        info!(url = %client.url, username = %client.username, "created SvnClient");
        client
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    #[instrument(skip(self), fields(url = %self.url))]
    pub async fn info(&self) -> Result<SvnInfo, SvnError> {
        let output = self.run_svn(&["info", "--xml", &self.url]).await?;
        parse_svn_info(&output)
    }

    #[instrument(skip(self), fields(url = %self.url))]
    pub async fn log(&self, start_rev: i64, end_rev: i64) -> Result<Vec<SvnLogEntry>, SvnError> {
        let end_str = if end_rev < 0 {
            "HEAD".to_string()
        } else {
            end_rev.to_string()
        };
        let rev_range = format!("{}:{}", start_rev, end_str);
        let output = self
            .run_svn(&["log", "--xml", "--verbose", "-r", &rev_range, &self.url])
            .await?;
        parse_svn_log(&output)
    }

    #[instrument(skip(self), fields(url = %self.url, rev))]
    pub async fn diff(&self, rev: i64) -> Result<Vec<SvnDiffEntry>, SvnError> {
        let rev_range = format!("{}:{}", rev - 1, rev);
        let output = self
            .run_svn(&["diff", "--summarize", "--xml", "-r", &rev_range, &self.url])
            .await?;
        parse_svn_diff_summarize(&output)
    }

    #[instrument(skip(self), fields(url = %self.url, rev))]
    pub async fn diff_full(&self, rev: i64) -> Result<String, SvnError> {
        let rev_range = format!("{}:{}", rev - 1, rev);
        self.run_svn(&["diff", "-r", &rev_range, &self.url]).await
    }

    #[instrument(skip(self), fields(url = %self.url, rev))]
    pub async fn checkout(&self, path: &Path, rev: i64) -> Result<(), SvnError> {
        let rev_str = rev.to_string();
        let path_str = path.to_string_lossy().to_string();
        self.run_svn(&["checkout", "-r", &rev_str, &self.url, &path_str])
            .await?;
        info!(path = %path.display(), rev, "svn checkout completed");
        Ok(())
    }

    #[instrument(skip(self, message), fields(path = %path.display()))]
    pub async fn commit(&self, path: &Path, message: &str, _author: &str) -> Result<i64, SvnError> {
        let path_str = path.to_string_lossy().to_string();
        let output = self
            .run_svn_in_dir(path, &["commit", "-m", message, &path_str])
            .await?;
        let rev = parse_committed_revision(&output).ok_or_else(|| SvnError::CommandFailed {
            exit_code: 0,
            stderr: format!("could not parse committed revision from: {}", output),
        })?;
        info!(rev, "svn commit succeeded");
        Ok(rev)
    }

    #[instrument(skip(self, prop_value), fields(url = %self.url, rev, prop_name))]
    pub async fn set_rev_prop(
        &self,
        rev: i64,
        prop_name: &str,
        prop_value: &str,
    ) -> Result<(), SvnError> {
        let rev_str = rev.to_string();
        self.run_svn(&[
            "propset",
            "--revprop",
            "-r",
            &rev_str,
            prop_name,
            prop_value,
            &self.url,
        ])
        .await?;
        debug!(rev, prop_name, "set revision property");
        Ok(())
    }

    #[instrument(skip(self), fields(url = %self.url))]
    pub async fn list_branches(&self, branches_path: &str) -> Result<Vec<String>, SvnError> {
        let branches_url = format!("{}/{}", self.url, branches_path);
        let output = self.run_svn(&["list", &branches_url]).await?;
        let branches: Vec<String> = output
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.trim_end_matches('/').to_string())
            .collect();
        debug!(count = branches.len(), "listed branches");
        Ok(branches)
    }

    #[instrument(skip(self), fields(url = %self.url))]
    pub async fn create_branch(
        &self,
        name: &str,
        source_path: &str,
        branches_path: &str,
        source_rev: i64,
    ) -> Result<(), SvnError> {
        let src_url = format!("{}/{}", self.url, source_path);
        let dest_url = format!("{}/{}/{}", self.url, branches_path, name);
        let rev_str = source_rev.to_string();
        let message = format!(
            "Create branch {} from {} at r{}",
            name, source_path, source_rev
        );
        self.run_svn(&["copy", "-r", &rev_str, &src_url, &dest_url, "-m", &message])
            .await?;
        info!(name, source_rev, "created branch");
        Ok(())
    }

    #[instrument(skip(self), fields(url = %self.url, rev))]
    pub async fn export(&self, path: &str, rev: i64, dest: &Path) -> Result<(), SvnError> {
        let src_url = if path.is_empty() {
            self.url.clone()
        } else {
            format!("{}/{}", self.url, path)
        };
        let rev_str = rev.to_string();
        let dest_str = dest.to_string_lossy().to_string();
        self.run_svn(&["export", "--force", "-r", &rev_str, &src_url, &dest_str])
            .await?;
        info!(dest = %dest.display(), rev, "svn export completed");
        Ok(())
    }

    // -- Working copy methods (personal branch mode) -------------------------

    /// Run `svn update` on a working copy.
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn update(&self, path: &Path) -> Result<String, SvnError> {
        let output = self.run_svn_in_dir(path, &["update"]).await?;
        info!(path = %path.display(), "svn update completed");
        Ok(output)
    }

    /// Run `svn add` on files in a working copy.
    #[instrument(skip(self, files), fields(path = %path.display()))]
    pub async fn add(&self, path: &Path, files: &[&str]) -> Result<(), SvnError> {
        if files.is_empty() {
            return Ok(());
        }
        let mut args = vec!["add", "--force"];
        args.extend(files);
        self.run_svn_in_dir(path, &args).await?;
        debug!(count = files.len(), "svn add completed");
        Ok(())
    }

    /// Run `svn rm` on files in a working copy.
    #[instrument(skip(self, files), fields(path = %path.display()))]
    pub async fn rm(&self, path: &Path, files: &[&str]) -> Result<(), SvnError> {
        if files.is_empty() {
            return Ok(());
        }
        let mut args = vec!["rm", "--force"];
        args.extend(files);
        self.run_svn_in_dir(path, &args).await?;
        debug!(count = files.len(), "svn rm completed");
        Ok(())
    }

    /// Get the working copy status (modified, added, deleted files).
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn status(&self, path: &Path) -> Result<String, SvnError> {
        self.run_svn_in_dir(path, &["status"]).await
    }

    /// Get the content of a file at a specific revision.
    #[instrument(skip(self), fields(file_path = %file_path, rev))]
    pub async fn cat(&self, file_path: &str, rev: i64) -> Result<String, SvnError> {
        let rev_str = rev.to_string();
        let url = if file_path.starts_with("http://") || file_path.starts_with("https://") {
            file_path.to_string()
        } else {
            format!("{}/{}", self.url, file_path)
        };
        self.run_svn(&["cat", "-r", &rev_str, &url]).await
    }

    // -- Internal helpers ----------------------------------------------------

    async fn run_svn(&self, args: &[&str]) -> Result<String, SvnError> {
        let mut cmd = Command::new("svn");
        cmd.args(args)
            .arg("--non-interactive")
            .arg("--no-auth-cache")
            .arg("--username")
            .arg(&self.username)
            .arg("--password")
            .arg(&self.password)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        debug!(cmd = ?format!("svn {}", args.join(" ")), "running svn command");
        let output = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SvnError::BinaryNotFound("svn".into())
            } else {
                SvnError::IoError(e)
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            warn!(exit_code, %stderr, "svn command failed");
            return Err(SvnError::CommandFailed { exit_code, stderr });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn run_svn_in_dir(&self, dir: &Path, args: &[&str]) -> Result<String, SvnError> {
        let mut cmd = Command::new("svn");
        cmd.current_dir(dir)
            .args(args)
            .arg("--non-interactive")
            .arg("--no-auth-cache")
            .arg("--username")
            .arg(&self.username)
            .arg("--password")
            .arg(&self.password)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SvnError::BinaryNotFound("svn".into())
            } else {
                SvnError::IoError(e)
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);
            return Err(SvnError::CommandFailed { exit_code, stderr });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

fn parse_committed_revision(output: &str) -> Option<i64> {
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("Committed revision") {
            return line
                .trim_start_matches("Committed revision")
                .trim()
                .trim_end_matches('.')
                .parse::<i64>()
                .ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_committed_revision() {
        assert_eq!(
            parse_committed_revision("Committed revision 42.\n"),
            Some(42)
        );
        assert_eq!(parse_committed_revision("No output"), None);
    }

    #[test]
    fn test_client_construction() {
        let client = SvnClient::new("https://svn.example.com/repo", "user", "pass");
        assert_eq!(client.url(), "https://svn.example.com/repo");
    }
}

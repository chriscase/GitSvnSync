//! GitHub REST API client.

use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{debug, info, instrument, warn};

use crate::errors::GitHubError;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommit {
    pub sha: String,
    pub commit: GitHubCommitDetail,
    pub author: Option<GitHubUserSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommitDetail {
    pub message: String,
    pub author: GitHubGitActor,
    pub committer: GitHubGitActor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubGitActor {
    pub name: String,
    pub email: String,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUserSummary {
    pub login: String,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub state: String,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    pub merged: Option<bool>,
    pub merge_commit_sha: Option<String>,
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

/// Detailed commit info from `GET /repos/{owner}/{repo}/commits/{sha}`.
/// Includes the `parents` array needed for merge strategy detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommitDetail2 {
    pub sha: String,
    pub commit: GitHubCommitDetail,
    pub author: Option<GitHubUserSummary>,
    pub parents: Vec<GitHubCommitParent>,
}

/// A parent reference in a commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCommitParent {
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitStatusState {
    Pending,
    Success,
    Failure,
    Error,
}

impl std::fmt::Display for CommitStatusState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Success => write!(f, "success"),
            Self::Failure => write!(f, "failure"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Asynchronous GitHub REST API client.
#[derive(Clone)]
pub struct GitHubClient {
    http: reqwest::Client,
    api_url: String,
    token: String,
}

impl GitHubClient {
    pub fn new(api_url: impl Into<String>, token: impl Into<String>) -> Self {
        let api_url = api_url.into().trim_end_matches('/').to_string();
        let token = token.into();
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("gitsvnsync/0.1"));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build reqwest client");
        info!(api_url = %api_url, "created GitHubClient");
        Self {
            http,
            api_url,
            token,
        }
    }

    #[instrument(skip(self))]
    pub async fn get_commits(
        &self,
        repo: &str,
        since_sha: Option<&str>,
    ) -> Result<Vec<GitHubCommit>, GitHubError> {
        let url = format!("{}/repos/{}/commits", self.api_url, repo);
        let mut req = self.http.get(&url).bearer_auth(&self.token);
        if let Some(sha) = since_sha {
            req = req.query(&[("sha", sha)]);
        }
        req = req.query(&[("per_page", "100")]);
        let resp = req.send().await?;
        self.check_response(&resp)?;
        let commits: Vec<GitHubCommit> = resp.json().await?;
        debug!(count = commits.len(), "fetched commits");
        Ok(commits)
    }

    #[instrument(skip(self, secret))]
    pub async fn create_webhook(
        &self,
        repo: &str,
        callback_url: &str,
        secret: &str,
    ) -> Result<serde_json::Value, GitHubError> {
        let url = format!("{}/repos/{}/hooks", self.api_url, repo);
        let body = serde_json::json!({
            "name": "web", "active": true, "events": ["push", "pull_request"],
            "config": { "url": callback_url, "content_type": "json", "secret": secret, "insecure_ssl": "0" }
        });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        self.check_response(&resp)?;
        let hook: serde_json::Value = resp.json().await?;
        info!(hook_id = %hook["id"], "created webhook");
        Ok(hook)
    }

    /// Verify a GitHub webhook signature.
    pub fn verify_webhook_signature(payload: &[u8], signature: &str, secret: &str) -> bool {
        let hex_sig = match signature.strip_prefix("sha256=") {
            Some(s) => s,
            None => {
                warn!("webhook signature missing sha256= prefix");
                return false;
            }
        };
        let expected_bytes = match hex::decode(hex_sig) {
            Ok(b) => b,
            Err(_) => {
                warn!("webhook signature is not valid hex");
                return false;
            }
        };
        let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
            Ok(m) => m,
            Err(_) => {
                warn!("failed to create HMAC");
                return false;
            }
        };
        mac.update(payload);
        mac.verify_slice(&expected_bytes).is_ok()
    }

    #[instrument(skip(self, body))]
    pub async fn create_pull_request(
        &self,
        repo: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequest, GitHubError> {
        let url = format!("{}/repos/{}/pulls", self.api_url, repo);
        let payload =
            serde_json::json!({ "title": title, "body": body, "head": head, "base": base });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?;
        self.check_response(&resp)?;
        let pr: PullRequest = resp.json().await?;
        info!(number = pr.number, "created pull request");
        Ok(pr)
    }

    #[instrument(skip(self))]
    pub async fn merge_pull_request(&self, repo: &str, pr_number: u64) -> Result<(), GitHubError> {
        let url = format!("{}/repos/{}/pulls/{}/merge", self.api_url, repo, pr_number);
        let payload = serde_json::json!({ "merge_method": "merge" });
        let resp = self
            .http
            .put(&url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?;
        self.check_response(&resp)?;
        info!(pr_number, "merged pull request");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_user(&self, username: &str) -> Result<GitHubUser, GitHubError> {
        let url = format!("{}/users/{}", self.api_url, username);
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        self.check_response(&resp)?;
        let user: GitHubUser = resp.json().await?;
        debug!(login = %user.login, "fetched user");
        Ok(user)
    }

    #[instrument(skip(self))]
    pub async fn post_commit_status(
        &self,
        repo: &str,
        sha: &str,
        state: CommitStatusState,
        description: &str,
    ) -> Result<(), GitHubError> {
        let url = format!("{}/repos/{}/statuses/{}", self.api_url, repo, sha);
        let payload = serde_json::json!({ "state": state.to_string(), "description": description, "context": "gitsvnsync" });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?;
        self.check_response(&resp)?;
        debug!(sha, state = %state, "posted commit status");
        Ok(())
    }

    // -- Personal Branch Mode methods -----------------------------------------

    /// List recently merged pull requests targeting `base` branch.
    #[instrument(skip(self))]
    pub async fn get_merged_pull_requests(
        &self,
        repo: &str,
        base: &str,
        since: Option<&str>,
    ) -> Result<Vec<PullRequest>, GitHubError> {
        let url = format!("{}/repos/{}/pulls", self.api_url, repo);
        let mut req = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .query(&[("state", "closed"), ("base", base), ("per_page", "50")]);
        if let Some(since_dt) = since {
            req = req.query(&[("sort", "updated"), ("direction", "desc")]);
            // Filter will be done client-side since GitHub doesn't support `since` on /pulls
            let _ = since_dt; // used below
        }
        let resp = req.send().await?;
        self.check_response(&resp)?;
        let prs: Vec<PullRequest> = resp.json().await?;
        // Filter to only merged PRs, optionally after a timestamp
        let merged: Vec<PullRequest> = prs
            .into_iter()
            .filter(|pr| pr.merged == Some(true))
            .filter(|pr| {
                if let Some(since_dt) = since {
                    pr.merged_at
                        .as_deref()
                        .map(|m| m >= since_dt)
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .collect();
        debug!(count = merged.len(), base, "fetched merged pull requests");
        Ok(merged)
    }

    /// Get commits for a specific pull request.
    #[instrument(skip(self))]
    pub async fn get_pr_commits(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<GitHubCommit>, GitHubError> {
        let url = format!(
            "{}/repos/{}/pulls/{}/commits",
            self.api_url, repo, pr_number
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .query(&[("per_page", "100")])
            .send()
            .await?;
        self.check_response(&resp)?;
        let commits: Vec<GitHubCommit> = resp.json().await?;
        debug!(count = commits.len(), pr_number, "fetched PR commits");
        Ok(commits)
    }

    /// Get a single pull request by number.
    #[instrument(skip(self))]
    pub async fn get_pull_request(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<PullRequest, GitHubError> {
        let url = format!("{}/repos/{}/pulls/{}", self.api_url, repo, pr_number);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;
        self.check_response(&resp)?;
        let pr: PullRequest = resp.json().await?;
        debug!(number = pr.number, state = %pr.state, "fetched pull request");
        Ok(pr)
    }

    /// Get a single commit by SHA (includes parent count for merge detection).
    #[instrument(skip(self))]
    pub async fn get_commit(
        &self,
        repo: &str,
        sha: &str,
    ) -> Result<GitHubCommitDetail2, GitHubError> {
        let url = format!("{}/repos/{}/commits/{}", self.api_url, repo, sha);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;
        self.check_response(&resp)?;
        let commit: GitHubCommitDetail2 = resp.json().await?;
        debug!(sha, parents = commit.parents.len(), "fetched commit details");
        Ok(commit)
    }

    /// Check whether a repository exists.
    #[instrument(skip(self))]
    pub async fn repo_exists(&self, repo: &str) -> Result<bool, GitHubError> {
        let url = format!("{}/repos/{}", self.api_url, repo);
        let resp = self
            .http
            .head(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    /// Create a new GitHub repository.
    #[instrument(skip(self))]
    pub async fn create_repo(
        &self,
        name: &str,
        private: bool,
        description: &str,
    ) -> Result<serde_json::Value, GitHubError> {
        let url = format!("{}/user/repos", self.api_url);
        let payload = serde_json::json!({
            "name": name,
            "private": private,
            "description": description,
            "auto_init": false,
        });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .await?;
        self.check_response(&resp)?;
        let repo: serde_json::Value = resp.json().await?;
        info!(repo_name = name, private, "created repository");
        Ok(repo)
    }

    /// Get the authenticated user's login.
    #[instrument(skip(self))]
    pub async fn get_authenticated_user(&self) -> Result<GitHubUser, GitHubError> {
        let url = format!("{}/user", self.api_url);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await?;
        self.check_response(&resp)?;
        let user: GitHubUser = resp.json().await?;
        debug!(login = %user.login, "fetched authenticated user");
        Ok(user)
    }

    fn check_response(&self, resp: &reqwest::Response) -> Result<(), GitHubError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(GitHubError::AuthenticationFailed(format!(
                "HTTP {}",
                status
            )));
        }
        if status.as_u16() == 429 {
            let reset = resp
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();
            return Err(GitHubError::RateLimited { reset_at: reset });
        }
        Err(GitHubError::ApiError {
            status: status.as_u16(),
            body: format!("HTTP {}", status),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_webhook_signature_valid() {
        let secret = "my-secret";
        let payload = b"hello world";
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        let hex_sig = hex::encode(mac.finalize().into_bytes());
        let signature = format!("sha256={}", hex_sig);
        assert!(GitHubClient::verify_webhook_signature(
            payload, &signature, secret
        ));
    }

    #[test]
    fn test_verify_webhook_signature_invalid() {
        assert!(!GitHubClient::verify_webhook_signature(
            b"payload",
            "sha256=0000000000000000000000000000000000000000000000000000000000000000",
            "secret"
        ));
    }
}

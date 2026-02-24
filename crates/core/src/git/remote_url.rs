//! Host-aware Git remote URL derivation.
//!
//! Constructs the HTTPS clone/push URL for a GitHub repository by deriving
//! the Git base URL from the configured API URL, or from an explicit override.
//!
//! This enables theoretical compatibility with GitHub Enterprise Server
//! (GHES) and GitHub Enterprise Cloud (GHEC) without requiring live
//! enterprise tenant tests.

/// Derive the HTTPS clone URL for a GitHub repository.
///
/// Resolution order:
/// 1. If `git_base_url` is `Some(non-empty)`, use it as the base.
/// 2. Otherwise derive from `api_url`:
///    - `https://api.github.com` → `https://github.com`
///    - `https://<host>/api/v3`  → `https://<host>`
///    - `https://<host>/api/v3/` → `https://<host>` (trailing slash)
///    - Anything else            → strip trailing slash, use as-is
///
/// The resulting URL is `{base}/{repo}.git` where `repo` is in `owner/name`
/// format.
pub fn derive_git_remote_url(api_url: &str, git_base_url: Option<&str>, repo: &str) -> String {
    let base = derive_git_base_url(api_url, git_base_url);
    format!("{}/{}.git", base, repo)
}

/// Derive just the Git base URL (without repo path).
///
/// See [`derive_git_remote_url`] for resolution rules.
pub fn derive_git_base_url(api_url: &str, git_base_url: Option<&str>) -> String {
    // 1. Explicit override takes precedence.
    if let Some(explicit) = git_base_url {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    // 2. Derive from api_url.
    let url = api_url.trim().trim_end_matches('/');

    // GitHub.com: "https://api.github.com" → "https://github.com"
    if url.eq_ignore_ascii_case("https://api.github.com") {
        return "https://github.com".to_string();
    }

    // Enterprise: "https://<host>/api/v3" → "https://<host>"
    if let Some(base) = url.strip_suffix("/api/v3") {
        return base.to_string();
    }

    // Fallback: use the API URL itself (already stripped of trailing slash).
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // derive_git_base_url tests
    // -------------------------------------------------------------------

    #[test]
    fn test_github_com_default() {
        assert_eq!(
            derive_git_base_url("https://api.github.com", None),
            "https://github.com"
        );
    }

    #[test]
    fn test_github_com_trailing_slash() {
        assert_eq!(
            derive_git_base_url("https://api.github.com/", None),
            "https://github.com"
        );
    }

    #[test]
    fn test_github_com_case_insensitive() {
        assert_eq!(
            derive_git_base_url("HTTPS://API.GITHUB.COM", None),
            "https://github.com"
        );
    }

    #[test]
    fn test_enterprise_api_v3() {
        assert_eq!(
            derive_git_base_url("https://github.company.com/api/v3", None),
            "https://github.company.com"
        );
    }

    #[test]
    fn test_enterprise_api_v3_trailing_slash() {
        assert_eq!(
            derive_git_base_url("https://github.company.com/api/v3/", None),
            "https://github.company.com"
        );
    }

    #[test]
    fn test_explicit_git_base_url_overrides() {
        assert_eq!(
            derive_git_base_url(
                "https://api.github.com",
                Some("https://custom-git.company.com")
            ),
            "https://custom-git.company.com"
        );
    }

    #[test]
    fn test_explicit_git_base_url_strips_trailing_slash() {
        assert_eq!(
            derive_git_base_url(
                "https://api.github.com",
                Some("https://custom-git.company.com/")
            ),
            "https://custom-git.company.com"
        );
    }

    #[test]
    fn test_explicit_empty_string_falls_through() {
        assert_eq!(
            derive_git_base_url("https://api.github.com", Some("")),
            "https://github.com"
        );
    }

    #[test]
    fn test_explicit_whitespace_only_falls_through() {
        assert_eq!(
            derive_git_base_url("https://api.github.com", Some("  ")),
            "https://github.com"
        );
    }

    #[test]
    fn test_unknown_api_url_used_as_is() {
        assert_eq!(
            derive_git_base_url("https://git.internal.io", None),
            "https://git.internal.io"
        );
    }

    #[test]
    fn test_unknown_api_url_strips_trailing_slash() {
        assert_eq!(
            derive_git_base_url("https://git.internal.io/", None),
            "https://git.internal.io"
        );
    }

    // -------------------------------------------------------------------
    // derive_git_remote_url tests
    // -------------------------------------------------------------------

    #[test]
    fn test_remote_url_github_com() {
        assert_eq!(
            derive_git_remote_url("https://api.github.com", None, "acme/project"),
            "https://github.com/acme/project.git"
        );
    }

    #[test]
    fn test_remote_url_enterprise() {
        assert_eq!(
            derive_git_remote_url("https://github.company.com/api/v3", None, "org/repo"),
            "https://github.company.com/org/repo.git"
        );
    }

    #[test]
    fn test_remote_url_explicit_override() {
        assert_eq!(
            derive_git_remote_url(
                "https://api.github.com",
                Some("https://ghes.internal.net"),
                "team/project"
            ),
            "https://ghes.internal.net/team/project.git"
        );
    }
}

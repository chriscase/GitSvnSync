//! LDAP authentication support.
//!
//! When configured, users can authenticate against a corporate LDAP/Active Directory
//! server. On first login, a local user account is auto-provisioned from LDAP attributes.

use ldap3::{LdapConnAsync, LdapConnSettings, Scope, SearchEntry};
use native_tls::TlsConnector;
use serde::{Deserialize, Serialize};
use tracing::debug;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// LDAP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapConfig {
    /// LDAP server URL (e.g. "ldaps://ldap.example.com:3269").
    pub url: String,
    /// Base DN for user searches (e.g. "dc=corp,dc=example,dc=com").
    pub base_dn: String,
    /// Search filter template. `{0}` is replaced with the username.
    pub search_filter: String,
    /// LDAP attribute containing the user's display name.
    pub display_name_attr: String,
    /// LDAP attribute containing the user's email address.
    pub email_attr: String,
    /// LDAP attribute containing group membership.
    pub group_attr: String,
    /// Optional service account DN used to search for users.
    pub bind_dn: Option<String>,
    /// Optional service account password.
    pub bind_password: Option<String>,
}

/// A user record returned after successful LDAP authentication.
#[derive(Debug, Clone)]
pub struct LdapUser {
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub groups: Vec<String>,
}

/// Errors from LDAP operations.
#[derive(Debug, thiserror::Error)]
pub enum LdapAuthError {
    #[error("LDAP connection failed: {0}")]
    ConnectionFailed(String),

    #[error("LDAP bind failed: {0}")]
    BindFailed(String),

    #[error("LDAP search failed: {0}")]
    SearchFailed(String),

    #[error("user not found in LDAP")]
    UserNotFound,

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("LDAP error: {0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl LdapConfig {
    /// Authenticate a user via LDAP bind.
    ///
    /// 1. Connect to the LDAP server.
    /// 2. If `bind_dn` is set, bind as service account and search for the user DN.
    /// 3. Otherwise, construct a DN from the username and base DN.
    /// 4. Rebind with the user's DN and password to verify credentials.
    /// 5. Search for the user's attributes.
    /// 6. Return an `LdapUser` on success.
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<LdapUser, LdapAuthError> {
        // Build connection settings — accept internal/self-signed CA certs
        // commonly used by corporate LDAP/AD servers.
        let tls_connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|e| LdapAuthError::ConnectionFailed(format!("TLS setup failed: {}", e)))?;
        let settings = LdapConnSettings::new()
            .set_connector(tls_connector);
        let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &self.url)
            .await
            .map_err(|e| LdapAuthError::ConnectionFailed(e.to_string()))?;

        // Drive the connection in the background.
        ldap3::drive!(conn);

        // Build the search filter by replacing `{0}` with the username.
        let search_filter = self.search_filter.replace("{0}", username);

        // Step 1: Find the user DN.
        let user_dn = if let (Some(bind_dn), Some(bind_pw)) =
            (self.bind_dn.as_deref(), self.bind_password.as_deref())
        {
            // Bind as service account first.
            ldap.simple_bind(bind_dn, bind_pw)
                .await
                .map_err(|e| LdapAuthError::BindFailed(format!("service account bind: {}", e)))?
                .success()
                .map_err(|e| LdapAuthError::BindFailed(format!("service account bind rejected: {}", e)))?;

            debug!("LDAP: bound as service account, searching for user '{}'", username);

            let (entries, _result) = ldap
                .search(&self.base_dn, Scope::Subtree, &search_filter, vec!["dn"])
                .await
                .map_err(|e| LdapAuthError::SearchFailed(e.to_string()))?
                .success()
                .map_err(|e| LdapAuthError::SearchFailed(e.to_string()))?;

            if entries.is_empty() {
                return Err(LdapAuthError::UserNotFound);
            }

            let entry = SearchEntry::construct(entries.into_iter().next().unwrap());
            entry.dn
        } else {
            // No service account — construct DN directly.
            format!("cn={},{}", username, self.base_dn)
        };

        debug!("LDAP: attempting bind for user DN '{}'", user_dn);

        // Step 2: Bind as the user to verify credentials.
        let bind_result = ldap
            .simple_bind(&user_dn, password)
            .await
            .map_err(|e| LdapAuthError::BindFailed(e.to_string()))?;

        if bind_result.rc != 0 {
            debug!("LDAP: user bind failed with rc={}", bind_result.rc);
            return Err(LdapAuthError::InvalidCredentials);
        }

        // Step 3: Search for user attributes.
        let attrs = vec![
            self.display_name_attr.as_str(),
            self.email_attr.as_str(),
            self.group_attr.as_str(),
        ];

        let (entries, _result) = ldap
            .search(&self.base_dn, Scope::Subtree, &search_filter, attrs)
            .await
            .map_err(|e| LdapAuthError::SearchFailed(e.to_string()))?
            .success()
            .map_err(|e| LdapAuthError::SearchFailed(e.to_string()))?;

        let _ = ldap.unbind().await;

        if entries.is_empty() {
            return Err(LdapAuthError::UserNotFound);
        }

        let entry = SearchEntry::construct(entries.into_iter().next().unwrap());

        let display_name = entry
            .attrs
            .get(&self.display_name_attr)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_else(|| username.to_string());

        let email = entry
            .attrs
            .get(&self.email_attr)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default();

        let groups = entry
            .attrs
            .get(&self.group_attr)
            .cloned()
            .unwrap_or_default();

        Ok(LdapUser {
            username: username.to_string(),
            display_name,
            email,
            groups,
        })
    }

    /// Test LDAP connectivity by attempting a bind (service account or anonymous).
    pub async fn test_connection(&self) -> Result<String, LdapAuthError> {
        let settings = LdapConnSettings::new();
        let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &self.url)
            .await
            .map_err(|e| LdapAuthError::ConnectionFailed(e.to_string()))?;

        ldap3::drive!(conn);

        if let (Some(bind_dn), Some(bind_pw)) =
            (self.bind_dn.as_deref(), self.bind_password.as_deref())
        {
            ldap.simple_bind(bind_dn, bind_pw)
                .await
                .map_err(|e| LdapAuthError::BindFailed(format!("service account bind: {}", e)))?
                .success()
                .map_err(|e| LdapAuthError::BindFailed(format!("service account bind rejected: {}", e)))?;
        }

        let _ = ldap.unbind().await;

        Ok("LDAP connection successful".to_string())
    }
}

//! LDAP-based identity resolution (stub implementation).
//!
//! Provides the [`LdapResolver`] trait and a stub implementation. When LDAP
//! support is needed in production, the stub can be replaced with a real
//! LDAP client (e.g. `ldap3` crate) without changing the rest of the codebase.

use crate::errors::IdentityError;
use crate::identity::mapper::GitIdentity;
use tracing::{debug, info, warn};

/// LDAP resolver interface for looking up identities.
///
/// This is implemented as a struct with methods rather than a trait to keep
/// things simple, but the interface is designed to be mockable.
pub struct LdapResolver {
    url: String,
    base_dn: String,
    bind_dn: String,
    #[allow(dead_code)]
    bind_password: String,
    connected: bool,
}

impl LdapResolver {
    /// Create a new LDAP resolver.
    ///
    /// This does not immediately connect -- connection is deferred until the
    /// first lookup.
    pub fn new(
        url: impl Into<String>,
        base_dn: impl Into<String>,
        bind_dn: impl Into<String>,
        bind_password: impl Into<String>,
    ) -> Self {
        let resolver = Self {
            url: url.into(),
            base_dn: base_dn.into(),
            bind_dn: bind_dn.into(),
            bind_password: bind_password.into(),
            connected: false,
        };
        info!(
            url = %resolver.url,
            base_dn = %resolver.base_dn,
            "created LdapResolver (stub)"
        );
        resolver
    }

    /// Ensure we have an active LDAP connection.
    ///
    /// In a real implementation this would establish a TLS connection and bind.
    fn ensure_connected(&mut self) -> Result<(), IdentityError> {
        if self.connected {
            return Ok(());
        }
        debug!(
            url = %self.url,
            bind_dn = %self.bind_dn,
            "connecting to LDAP server (stub -- no actual connection)"
        );
        // Stub: mark as connected without doing anything.
        self.connected = true;
        Ok(())
    }

    /// Look up a Git identity by SVN / LDAP username.
    ///
    /// In a real implementation this would search `(uid=username)` under
    /// `base_dn` and extract `cn` + `mail` attributes.
    pub fn lookup_by_username(
        &mut self,
        username: &str,
    ) -> Result<Option<GitIdentity>, IdentityError> {
        self.ensure_connected()?;
        debug!(username, "LDAP lookup by username (stub)");

        // Stub: always returns None.
        // A real implementation would execute an LDAP search here:
        //   filter: (uid={username})
        //   attrs: ["cn", "mail"]
        warn!(
            username,
            "LDAP lookup is stubbed -- returning None. \
             Replace with real LDAP client for production use."
        );
        Ok(None)
    }

    /// Reverse lookup: find an SVN username by Git email.
    ///
    /// In a real implementation this would search `(mail=email)` under
    /// `base_dn` and extract the `uid` attribute.
    pub fn lookup_by_email(
        &mut self,
        email: &str,
    ) -> Result<Option<String>, IdentityError> {
        self.ensure_connected()?;
        debug!(email, "LDAP reverse lookup by email (stub)");

        warn!(
            email,
            "LDAP reverse lookup is stubbed -- returning None."
        );
        Ok(None)
    }

    /// Return whether this resolver is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_lookup_returns_none() {
        let mut resolver = LdapResolver::new(
            "ldap://ldap.example.com",
            "dc=example,dc=com",
            "cn=admin,dc=example,dc=com",
            "password",
        );

        let result = resolver.lookup_by_username("jdoe").unwrap();
        assert!(result.is_none());

        let result = resolver.lookup_by_email("jdoe@example.com").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_connection_state() {
        let mut resolver = LdapResolver::new(
            "ldap://localhost",
            "dc=test",
            "cn=admin",
            "pass",
        );

        assert!(!resolver.is_connected());
        resolver.lookup_by_username("test").unwrap();
        assert!(resolver.is_connected());
    }
}

//! Identity mapping engine for translating between SVN usernames and Git
//! author identities.
//!
//! The mapping hierarchy is:
//! 1. Explicit TOML mapping file (highest priority)
//! 2. LDAP lookup (if configured)
//! 3. Fallback: derive email from SVN username + email domain

pub mod ldap;
pub mod mapper;
pub mod mapping_file;

pub use mapper::{GitIdentity, IdentityMapper};

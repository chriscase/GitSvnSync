//! GitSvnSync core library.
//!
//! This crate provides the foundational components for bidirectional SVN/Git
//! synchronization: configuration, database persistence, identity mapping,
//! conflict detection and resolution, repository clients, and the sync engine.

pub mod config;
pub mod conflict;
pub mod db;
pub mod errors;
pub mod git;
pub mod identity;
pub mod models;
pub mod notify;
pub mod personal_config;
pub mod svn;
pub mod sync_engine;

// Re-exports for convenience.
pub use config::AppConfig;
pub use db::Database;
pub use identity::IdentityMapper;
pub use personal_config::PersonalConfig;
pub use sync_engine::SyncEngine;

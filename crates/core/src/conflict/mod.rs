//! Conflict detection, three-way merging, and resolution management.
//!
//! The conflict subsystem is responsible for:
//! 1. **Detection** -- comparing SVN and Git change sets to find overlapping edits.
//! 2. **Merging** -- attempting automatic three-way merges where possible.
//! 3. **Resolution** -- tracking manual / automatic resolution and applying it.

pub mod detector;
pub mod merger;
pub mod resolver;

pub use detector::{Conflict, ConflictDetector, ConflictStatus, ConflictType};
pub use merger::{MergeResult, Merger};
pub use resolver::ConflictResolver;

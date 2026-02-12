//! TOML-based identity mapping file reader/writer.
//!
//! The mapping file format:
//!
//! ```toml
//! [authors]
//! jdoe = { name = "John Doe", email = "jdoe@example.com" }
//! alice = { name = "Alice Smith", email = "alice@example.com" }
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::errors::IdentityError;

/// A single author entry in the mapping file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorEntry {
    /// Git display name.
    pub name: String,
    /// Git email address.
    pub email: String,
}

/// Wrapper around the TOML mapping file structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MappingFileData {
    /// The `[authors]` table mapping SVN username -> AuthorEntry.
    #[serde(default)]
    pub authors: HashMap<String, AuthorEntry>,
}

/// Utilities for loading and saving the identity mapping file.
pub struct MappingFile;

impl MappingFile {
    /// Load the mapping file from disk and return the author map.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<HashMap<String, AuthorEntry>, IdentityError> {
        let path = path.as_ref();
        info!(path = %path.display(), "loading identity mapping file");

        if !path.exists() {
            return Err(IdentityError::MappingFileError {
                path: path.display().to_string(),
                detail: "file not found".into(),
            });
        }

        let contents = std::fs::read_to_string(path).map_err(IdentityError::IoError)?;
        let data: MappingFileData =
            toml::from_str(&contents).map_err(|e| IdentityError::ParseError(e.to_string()))?;

        debug!(count = data.authors.len(), "loaded author mappings");
        Ok(data.authors)
    }

    /// Save the author map back to disk in TOML format.
    pub fn save<P: AsRef<Path>>(
        path: P,
        mappings: &HashMap<String, AuthorEntry>,
    ) -> Result<(), IdentityError> {
        let path = path.as_ref();
        info!(path = %path.display(), "saving identity mapping file");

        let data = MappingFileData {
            authors: mappings.clone(),
        };

        let toml_str =
            toml::to_string_pretty(&data).map_err(|e| IdentityError::ParseError(e.to_string()))?;
        std::fs::write(path, toml_str).map_err(IdentityError::IoError)?;

        debug!(count = mappings.len(), "saved author mappings");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_mapping_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("authors.toml");

        let content = r#"
[authors]
[authors.jdoe]
name = "John Doe"
email = "jdoe@example.com"

[authors.alice]
name = "Alice Smith"
email = "alice@example.com"
"#;
        std::fs::write(&path, content).unwrap();

        let mappings = MappingFile::load(&path).unwrap();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings["jdoe"].name, "John Doe");
        assert_eq!(mappings["alice"].email, "alice@example.com");
    }

    #[test]
    fn test_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("authors.toml");

        let mut mappings = HashMap::new();
        mappings.insert(
            "bob".to_string(),
            AuthorEntry {
                name: "Bob Builder".to_string(),
                email: "bob@example.com".to_string(),
            },
        );

        MappingFile::save(&path, &mappings).unwrap();

        let reloaded = MappingFile::load(&path).unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded["bob"].name, "Bob Builder");
    }

    #[test]
    fn test_load_nonexistent() {
        let result = MappingFile::load("/nonexistent/authors.toml");
        assert!(matches!(
            result,
            Err(IdentityError::MappingFileError { .. })
        ));
    }

    #[test]
    fn test_load_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.toml");
        std::fs::write(&path, "").unwrap();

        let mappings = MappingFile::load(&path).unwrap();
        assert!(mappings.is_empty());
    }
}

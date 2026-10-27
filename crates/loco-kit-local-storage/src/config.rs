//! The `initializers.storage` config block.
//!
//! ```yaml
//! initializers:
//!   storage:
//!     driver: local        # local | memory
//!     root: storage        # local: created when missing
//! ```

use std::path::PathBuf;

use loco_rs::{
    config::Config,
    storage::{
        drivers::{local, mem},
        Storage,
    },
    Error, Result,
};
use serde::Deserialize;

/// The key of the block under `initializers:`.
pub const INITIALIZER_KEY: &str = "storage";

/// Which store `ctx.storage` uses.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "driver", rename_all = "snake_case")]
pub enum StorageConfig {
    /// Files under `root` (relative to the working directory), created when
    /// missing.
    Local {
        /// The folder files are stored under.
        root: PathBuf,
    },
    /// In memory, gone when the process exits. For tests.
    Memory,
}

impl StorageConfig {
    /// Reads `initializers.storage`.
    ///
    /// # Errors
    ///
    /// When the block is missing or does not describe a driver.
    pub fn from_config(config: &Config) -> Result<Self> {
        let block = config
            .initializers
            .as_ref()
            .and_then(|initializers| initializers.get(INITIALIZER_KEY))
            .ok_or_else(|| {
                Error::string(
                    "missing `initializers.storage` in the config \
                     (`driver: local` or `memory`)",
                )
            })?;
        Self::deserialize(block)
            .map_err(|err| Error::Message(format!("initializers.storage: {err}")))
    }

    /// Builds the store.
    ///
    /// # Errors
    ///
    /// When the local root cannot be created.
    pub fn build(&self) -> Result<Storage> {
        let driver = match self {
            Self::Local { root } => {
                std::fs::create_dir_all(root).map_err(|err| {
                    Error::Message(format!(
                        "initializers.storage: cannot create {}: {err}",
                        root.display()
                    ))
                })?;
                local::new_with_prefix(root)?
            }
            Self::Memory => mem::new(),
        };
        Ok(Storage::single(driver))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(block: serde_json::Value) -> Result<StorageConfig> {
        StorageConfig::deserialize(block)
            .map_err(|err| Error::Message(format!("initializers.storage: {err}")))
    }

    #[test]
    fn parses_each_driver() {
        assert_eq!(
            parse(json!({ "driver": "local", "root": "storage" })).unwrap(),
            StorageConfig::Local {
                root: PathBuf::from("storage")
            }
        );
        assert_eq!(
            parse(json!({ "driver": "memory" })).unwrap(),
            StorageConfig::Memory
        );
    }

    #[test]
    fn rejects_an_unknown_driver_or_missing_fields() {
        assert!(parse(json!({ "driver": "ftp" })).is_err());
        assert!(parse(json!({ "driver": "s3", "bucket": "b", "region": "r" })).is_err());
        assert!(parse(json!({ "driver": "local" })).is_err());
        assert!(parse(json!({ "root": "storage" })).is_err());
    }

    #[tokio::test]
    async fn local_creates_its_root_and_stores_under_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("nested/storage");
        let storage = StorageConfig::Local { root: root.clone() }.build().unwrap();

        let path = std::path::Path::new("ab/cd/file");
        storage
            .upload(path, &bytes::Bytes::from_static(b"hi"))
            .await
            .unwrap();

        assert_eq!(std::fs::read(root.join(path)).unwrap(), b"hi");
    }
}

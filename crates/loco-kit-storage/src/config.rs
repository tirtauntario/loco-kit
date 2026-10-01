//! The `initializers.storage` config block.
//!
//! ```yaml
//! initializers:
//!   storage:
//!     driver: local        # local | memory | s3
//!     root: storage        # local: created when missing
//! ```
//!
//! ```yaml
//! initializers:
//!   storage:
//!     driver: s3           # needs the `s3` feature
//!     bucket: docsys-production
//!     region: ap-southeast-1
//!     endpoint: https://…  # optional: R2, MinIO
//!     # Optional; without them the AWS environment variables or instance
//!     # role are used.
//!     access_key_id: <%= get_env(name="S3_ACCESS_KEY_ID", default="credential:s3.access_key_id") %>
//!     secret_access_key: <%= get_env(name="S3_SECRET_ACCESS_KEY", default="credential:s3.secret_access_key") %>
//! ```

use std::{fmt, path::PathBuf};

use loco_rs::{
    config::Config,
    storage::{
        drivers::{local, mem, StoreDriver},
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
    Local { root: PathBuf },
    /// In memory, gone when the process exits. For tests.
    Memory,
    /// S3 or an S3-compatible store. Needs the `s3` feature.
    S3(S3Config),
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub access_key_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
}

impl fmt::Debug for S3Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Config")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
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
                     (`driver: local`, `memory` or `s3`)",
                )
            })?;
        Self::deserialize(block)
            .map_err(|err| Error::Message(format!("initializers.storage: {err}")))
    }

    /// Builds the store.
    ///
    /// # Errors
    ///
    /// When the local root cannot be created, or the S3 settings are
    /// incomplete or the `s3` feature is off.
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
            Self::S3(s3) => s3.driver()?,
        };
        Ok(Storage::single(driver))
    }
}

/// `None` for a missing value and for the empty string a `get_env` default of
/// `""` renders.
#[cfg(feature = "s3")]
fn given(value: Option<&String>) -> Option<&str> {
    value.map(String::as_str).filter(|value| !value.is_empty())
}

impl S3Config {
    #[cfg(feature = "s3")]
    fn driver(&self) -> Result<Box<dyn StoreDriver>> {
        use loco_rs::storage::drivers::aws;

        let credential = match (
            given(self.access_key_id.as_ref()),
            given(self.secret_access_key.as_ref()),
        ) {
            (Some(key_id), Some(secret_key)) => Some(aws::Credential {
                key_id: key_id.to_string(),
                secret_key: secret_key.to_string(),
                token: None,
            }),
            (None, None) => None,
            _ => {
                return Err(Error::string(
                    "initializers.storage: set both `access_key_id` and \
                     `secret_access_key`, or neither",
                ))
            }
        };
        let (bucket, region) = (self.bucket.as_str(), self.region.as_str());
        Ok(match (credential, given(self.endpoint.as_ref())) {
            (Some(credential), Some(endpoint)) => {
                aws::with_credentials_and_endpoint(bucket, region, endpoint, credential)?
            }
            (Some(credential), None) => aws::with_credentials(bucket, region, credential)?,
            (None, None) => aws::new(bucket, region)?,
            (None, Some(_)) => {
                return Err(Error::string(
                    "initializers.storage: an `endpoint` needs `access_key_id` \
                     and `secret_access_key`",
                ))
            }
        })
    }

    #[cfg(not(feature = "s3"))]
    #[allow(clippy::unused_self)]
    fn driver(&self) -> Result<Box<dyn StoreDriver>> {
        Err(Error::string(
            "initializers.storage: the `s3` driver needs the `s3` feature of \
             loco-kit-storage",
        ))
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
        let StorageConfig::S3(s3) =
            parse(json!({ "driver": "s3", "bucket": "b", "region": "r" })).unwrap()
        else {
            panic!("not s3");
        };
        assert_eq!((s3.bucket.as_str(), s3.endpoint), ("b", None));
    }

    #[test]
    fn rejects_an_unknown_driver_or_missing_fields() {
        assert!(parse(json!({ "driver": "ftp" })).is_err());
        assert!(parse(json!({ "driver": "local" })).is_err());
        assert!(parse(json!({ "root": "storage" })).is_err());
    }

    #[test]
    fn debug_hides_the_s3_keys() {
        let block = json!({
            "driver": "s3", "bucket": "b", "region": "r",
            "access_key_id": "AKIA", "secret_access_key": "s3cret"
        });
        let printed = format!("{:?}", parse(block).unwrap());
        assert!(
            !printed.contains("AKIA") && !printed.contains("s3cret"),
            "{printed}"
        );
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

    #[cfg(not(feature = "s3"))]
    #[test]
    fn s3_without_the_feature_says_so() {
        let config = parse(json!({ "driver": "s3", "bucket": "b", "region": "r" })).unwrap();
        let err = config.build().err().unwrap().to_string();
        assert!(err.contains("needs the `s3` feature"), "{err}");
    }

    #[cfg(feature = "s3")]
    #[test]
    fn s3_needs_both_keys_or_neither() {
        let config = parse(json!({
            "driver": "s3", "bucket": "b", "region": "r", "access_key_id": "AKIA"
        }))
        .unwrap();
        let err = config.build().err().unwrap().to_string();
        assert!(err.contains("set both"), "{err}");

        let config = parse(json!({
            "driver": "s3", "bucket": "b", "region": "r",
            "endpoint": "http://localhost:9000",
            "access_key_id": "AKIA", "secret_access_key": "s"
        }))
        .unwrap();
        assert!(config.build().is_ok());
    }
}

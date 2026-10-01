//! Random storage keys and where they are stored.

use std::{fmt, path::PathBuf, str::FromStr};

use loco_rs::{Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const LEN: usize = 32;

/// A random key for a stored file: 32 lowercase hex characters.
///
/// Save it in the database next to the file's name and content type, and
/// read the file back with [`StorageKey::path`]. A key read back from the
/// database or a request goes through [`StorageKey::parse`], so it can never
/// name a path outside its shard (`..`, `/`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StorageKey(String);

impl StorageKey {
    /// A new random key. A v4 UUID, not v7: v7 starts with a timestamp, so
    /// every key made around the same time would land in the same shard.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    /// Accepts exactly 32 lowercase hex characters.
    ///
    /// # Errors
    ///
    /// For anything else.
    pub fn parse(key: &str) -> Result<Self> {
        let valid = key.len() == LEN && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if valid {
            Ok(Self(key.to_string()))
        } else {
            Err(Error::Message(format!(
                "invalid storage key `{key}`: expected {LEN} lowercase hex characters"
            )))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Where the file is stored: `ab/cd/abcd…`. Two levels of 256 directories
    /// keep 100 million files to about 1,500 per directory. Join it under a
    /// prefix to group files: `Path::new("documents").join(key.path())`.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        [&self.0[0..2], &self.0[2..4], &self.0].iter().collect()
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for StorageKey {
    type Err = Error;

    fn from_str(key: &str) -> Result<Self> {
        Self::parse(key)
    }
}

impl TryFrom<String> for StorageKey {
    type Error = Error;

    fn try_from(key: String) -> Result<Self> {
        Self::parse(&key)
    }
}

impl From<StorageKey> for String {
    fn from(key: StorageKey) -> Self {
        key.0
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn generated_keys_are_32_hex_characters() {
        let key = StorageKey::generate();
        assert_eq!(key.as_str().len(), 32);
        assert_eq!(StorageKey::parse(key.as_str()).unwrap(), key);
        assert_ne!(StorageKey::generate(), key);
    }

    #[test]
    fn the_path_is_sharded_two_levels_deep() {
        let key = StorageKey::parse("abcdef0123456789abcdef0123456789").unwrap();
        assert_eq!(
            key.path(),
            Path::new("ab/cd/abcdef0123456789abcdef0123456789")
        );
    }

    #[test]
    fn rejects_anything_but_32_lowercase_hex_characters() {
        for key in [
            "",
            "abc",
            "ABCDEF0123456789ABCDEF0123456789",
            "../../../../etc/passwd/00000000000",
            "abcdef0123456789abcdef012345678",
            "abcdef0123456789abcdef01234567890",
            "abcdef0123456789abcdef012345678g",
        ] {
            assert!(StorageKey::parse(key).is_err(), "{key}");
        }
    }

    #[test]
    fn serializes_as_a_string_and_validates_on_the_way_in() {
        let key = StorageKey::generate();
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, format!("\"{key}\""));
        assert_eq!(serde_json::from_str::<StorageKey>(&json).unwrap(), key);
        assert!(serde_json::from_str::<StorageKey>("\"../x\"").is_err());
    }
}

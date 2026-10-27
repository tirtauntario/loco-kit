//! Where credentials live on disk, and reading/writing them.
//!
//! For environment `E` the app uses `config/credentials/E.yml.enc` when it
//! exists, else `config/credentials.yml.enc`; the per-environment file
//! *replaces* the global one, it is not merged. Each file has a key beside it
//! (`credentials/E.key` / `master.key`); `LOCO_MASTER_KEY` takes precedence
//! over either.
//!
//! This is the one place that reads the process environment directly
//! (`LOCO_MASTER_KEY`, `LOCO_CONFIG_FOLDER`): it runs while the config is
//! being built, so there is no `ctx.config` to read from yet.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use loco_rs::{Error, Result};
use serde_json::Value;

use super::cipher::{self, DecryptError, MasterKey};

/// Overrides the key file of whichever credentials file is in use.
pub const MASTER_KEY_ENV: &str = "LOCO_MASTER_KEY";

/// Loco's override of the config folder (`loco_rs::env_vars` is private).
const CONFIG_FOLDER_ENV: &str = "LOCO_CONFIG_FOLDER";

/// Loco's config folder: `LOCO_CONFIG_FOLDER`, else `config`.
#[must_use]
pub fn config_folder() -> PathBuf {
    std::env::var_os(CONFIG_FOLDER_ENV).map_or_else(|| PathBuf::from("config"), PathBuf::from)
}

/// `LOCO_MASTER_KEY`, when set and not blank.
#[must_use]
pub fn master_key_from_env() -> Option<String> {
    std::env::var(MASTER_KEY_ENV)
        .ok()
        .filter(|key| !key.trim().is_empty())
}

/// One encrypted credentials file and the key file beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The encrypted file, `*.yml.enc`.
    pub file: PathBuf,
    /// Its key file, `master.key` or `<env>.key`.
    pub key: PathBuf,
}

impl Location {
    /// `config/credentials.yml.enc` + `config/master.key`.
    #[must_use]
    pub fn global(folder: &Path) -> Self {
        Self {
            file: folder.join("credentials.yml.enc"),
            key: folder.join("master.key"),
        }
    }

    /// `config/credentials/<env>.yml.enc` + `config/credentials/<env>.key`.
    #[must_use]
    pub fn for_env(folder: &Path, env: &str) -> Self {
        let dir = folder.join("credentials");
        Self {
            file: dir.join(format!("{env}.yml.enc")),
            key: dir.join(format!("{env}.key")),
        }
    }

    /// The file environment `env` boots with: its own when it exists, else
    /// the global one.
    #[must_use]
    pub fn resolve(folder: &Path, env: &str) -> Self {
        let own = Self::for_env(folder, env);
        if own.file.exists() {
            own
        } else {
            Self::global(folder)
        }
    }

    /// The key: `env_key` (from `LOCO_MASTER_KEY`) when given, else the key
    /// file. `Ok(None)` when neither exists.
    ///
    /// # Errors
    ///
    /// When the key is not 64 hex characters, or the key file is unreadable.
    pub fn read_key(&self, env_key: Option<&str>) -> Result<Option<MasterKey>> {
        let (hex_key, source) = match env_key {
            Some(key) => (key.to_string(), format!("`{MASTER_KEY_ENV}`")),
            None if self.key.exists() => (
                fs::read_to_string(&self.key)?,
                self.key.display().to_string(),
            ),
            None => return Ok(None),
        };
        MasterKey::parse(&hex_key)
            .map(Some)
            .ok_or_else(|| Error::Message(format!("key must be 64 hex characters ({source})")))
    }

    /// The key, which must exist because the file does.
    ///
    /// # Errors
    ///
    /// When there is no key, or it is malformed.
    pub fn require_key(&self, env_key: Option<&str>) -> Result<MasterKey> {
        self.read_key(env_key)?.ok_or_else(|| {
            Error::Message(format!(
                "cannot decrypt {}: `{MASTER_KEY_ENV}` is unset and {} does not exist",
                self.file.display(),
                self.key.display()
            ))
        })
    }

    /// The decrypted plaintext; `Ok(None)` when the file does not exist.
    ///
    /// # Errors
    ///
    /// When the file exists but has no usable key, is not a credentials file,
    /// or does not decrypt with the key.
    pub fn read(&self, env_key: Option<&str>) -> Result<Option<String>> {
        if !self.file.exists() {
            return Ok(None);
        }
        let key = self.require_key(env_key)?;
        self.decrypt(&key).map(Some)
    }

    /// Decrypts the (existing) file with `key`.
    ///
    /// # Errors
    ///
    /// When the file is unreadable, not a credentials file, or `key` does not
    /// open it.
    pub fn decrypt(&self, key: &MasterKey) -> Result<String> {
        let contents = fs::read_to_string(&self.file)?;
        cipher::decrypt(key, &contents).map_err(|err| {
            let file = self.file.display();
            Error::Message(match err {
                DecryptError::Format => format!("{file} is not a credentials file"),
                DecryptError::Mismatch => {
                    format!("cannot decrypt {file}: wrong key or tampered file")
                }
            })
        })
    }

    /// Encrypts `plaintext` into the file, atomically (temp file + rename).
    ///
    /// # Errors
    ///
    /// When the folder cannot be created or the file written.
    pub fn write(&self, key: &MasterKey, plaintext: &str) -> Result<()> {
        let dir = parent_dir(&self.file);
        fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        writeln!(tmp, "{}", cipher::encrypt(key, plaintext))?;
        tmp.persist(&self.file)
            .map_err(|err| Error::from(err.error))?;
        Ok(())
    }

    /// Writes `key` to the key file, readable only by its owner. Refuses to
    /// overwrite an existing key file.
    ///
    /// # Errors
    ///
    /// When the key file already exists or cannot be written.
    pub fn write_key(&self, key: &MasterKey) -> Result<()> {
        fs::create_dir_all(parent_dir(&self.key))?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&self.key)?;
        writeln!(file, "{}", key.to_hex())?;
        Ok(())
    }
}

fn parent_dir(path: &Path) -> &Path {
    path.parent().unwrap_or_else(|| Path::new("."))
}

/// Parses decrypted credentials: YAML with a mapping at the top. An empty or
/// comment-only document is an empty mapping.
///
/// # Errors
///
/// When the YAML is invalid, or its top level is not a mapping.
pub fn parse(plaintext: &str) -> Result<Value> {
    let value: Value = serde_yaml_ng::from_str(plaintext)
        .map_err(|err| Error::Message(format!("invalid credentials YAML: {err}")))?;
    match value {
        Value::Null => Ok(Value::Object(serde_json::Map::new())),
        Value::Object(_) => Ok(value),
        _ => Err(Error::string(
            "invalid credentials YAML: the top level must be a mapping (`key: value`)",
        )),
    }
}

/// Credentials as loaded for one environment. `Debug` never shows the values.
#[derive(Clone, Default)]
pub struct Loaded {
    /// The decrypted mapping; empty when there is no file.
    pub values: Value,
    /// The file they came from, if any.
    pub file: Option<PathBuf>,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loaded")
            .field("file", &self.file)
            .finish_non_exhaustive()
    }
}

/// Loads the credentials environment `env` boots with.
///
/// # Errors
///
/// When the resolved file exists but cannot be decrypted or parsed.
pub fn load(folder: &Path, env: &str, env_key: Option<&str>) -> Result<Loaded> {
    let location = Location::resolve(folder, env);
    let Some(plaintext) = location.read(env_key)? else {
        return Ok(Loaded {
            values: Value::Object(serde_json::Map::new()),
            file: None,
        });
    };
    let values = parse(&plaintext)
        .map_err(|err| Error::Message(format!("{}: {err}", location.file.display())))?;
    Ok(Loaded {
        values,
        file: Some(location.file),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn seal(location: &Location, plaintext: &str) -> MasterKey {
        let key = MasterKey::generate();
        location.write_key(&key).unwrap();
        location.write(&key, plaintext).unwrap();
        key
    }

    #[test]
    fn the_environments_own_file_replaces_the_global_one() {
        let dir = tempfile::tempdir().unwrap();
        seal(&Location::global(dir.path()), "shared: global");
        seal(&Location::for_env(dir.path(), "production"), "own: prod");

        let loaded = load(dir.path(), "production", None).unwrap();
        assert_eq!(loaded.values, json!({ "own": "prod" }));
        assert_eq!(
            loaded.file,
            Some(dir.path().join("credentials/production.yml.enc"))
        );
    }

    #[test]
    fn falls_back_to_the_global_file() {
        let dir = tempfile::tempdir().unwrap();
        seal(&Location::global(dir.path()), "shared: global");

        let loaded = load(dir.path(), "development", None).unwrap();
        assert_eq!(loaded.values, json!({ "shared": "global" }));
        assert_eq!(loaded.file, Some(dir.path().join("credentials.yml.enc")));
    }

    #[test]
    fn no_file_is_empty_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load(dir.path(), "test", None).unwrap();
        assert_eq!(loaded.values, json!({}));
        assert_eq!(loaded.file, None);
    }

    #[test]
    fn the_env_key_beats_the_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        seal(&location, "a: 1");
        let other = MasterKey::generate();
        location.write(&other, "a: 2").unwrap();

        let loaded = load(dir.path(), "test", Some(&other.to_hex())).unwrap();
        assert_eq!(loaded.values, json!({ "a": 2 }));
    }

    #[test]
    fn a_file_without_a_key_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        seal(&location, "a: 1");
        fs::remove_file(&location.key).unwrap();

        let err = load(dir.path(), "test", None).unwrap_err().to_string();
        assert!(err.contains("`LOCO_MASTER_KEY` is unset"), "{err}");
        assert!(err.contains("master.key does not exist"), "{err}");
    }

    #[test]
    fn a_wrong_key_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        seal(&Location::global(dir.path()), "a: 1");

        let wrong = MasterKey::generate().to_hex();
        let err = load(dir.path(), "test", Some(&wrong))
            .unwrap_err()
            .to_string();
        assert!(err.contains("wrong key or tampered file"), "{err}");
    }

    #[test]
    fn a_malformed_key_names_its_source() {
        let dir = tempfile::tempdir().unwrap();
        seal(&Location::global(dir.path()), "a: 1");

        let err = load(dir.path(), "test", Some("nope"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("64 hex characters (`LOCO_MASTER_KEY`)"),
            "{err}"
        );
    }

    #[test]
    fn never_overwrites_a_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        location.write_key(&MasterKey::generate()).unwrap();
        assert!(location.write_key(&MasterKey::generate()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn key_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        location.write_key(&MasterKey::generate()).unwrap();
        let mode = fs::metadata(&location.key).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn parse_requires_a_mapping() {
        assert_eq!(parse("# only a comment\n").unwrap(), json!({}));
        assert_eq!(parse("a: { b: 1 }").unwrap(), json!({ "a": { "b": 1 } }));
        assert!(parse("- a\n- b").is_err());
        assert!(parse("a: [").is_err());
    }
}

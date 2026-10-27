//! Encrypted, environment-aware credentials — Rails' `credentials.yml.enc`.
//!
//! Secrets live in an encrypted YAML file committed to git and are decrypted
//! at boot with a key that is not (`store` has the file layout). They reach
//! the app two ways:
//!
//! * **config**: a `credential:<path>` string in the config is replaced by
//!   that credential (`overlay`); written as a `get_env` default, the
//!   environment variable wins;
//! * **code**: [`install`] puts them in `ctx.shared_store` and
//!   [`Credentials::from_context`] reads any credential by path.
//!
//! The decrypted values never enter [`Config`]: only the fields a placeholder
//! names are filled, so `cargo loco doctor --config` shows no other secret.
//!
//! Edit them with `cargo loco credentials edit [-e <env>]` (`cli`).

pub mod cipher;
pub mod cli;
pub mod overlay;
pub mod store;

use std::{fmt, path::Path};

use loco_rs::{app::AppContext, config::Config, environment::Environment, Error, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Loads `config/<env>.yaml` and fills its credential placeholders. This is
/// `App::load_config`.
///
/// # Errors
///
/// When the config does not load, the credentials cannot be decrypted, or a
/// placeholder names a missing credential.
pub fn load_config(env: &Environment) -> Result<Config> {
    let folder = store::config_folder();
    load_config_from(env, &folder, store::master_key_from_env().as_deref())
}

/// [`load_config`] with the config folder and master key passed in.
///
/// # Errors
///
/// As [`load_config`].
pub fn load_config_from(
    env: &Environment,
    folder: &Path,
    master_key: Option<&str>,
) -> Result<Config> {
    let config = env.load_from_folder(folder)?;
    let credentials = store::load(folder, &env.to_string(), master_key)?;
    overlay::apply(config, &credentials)
}

/// Decrypts the credentials of `ctx.environment` and puts them in
/// `ctx.shared_store` for [`Credentials::from_context`]. Call it from
/// `App::after_context`.
///
/// # Errors
///
/// When the credentials cannot be decrypted or parsed.
pub fn install(ctx: &AppContext) -> Result<()> {
    let credentials = Credentials::load_from(
        &ctx.environment,
        &store::config_folder(),
        store::master_key_from_env().as_deref(),
    )?;
    ctx.shared_store.insert(credentials);
    Ok(())
}

/// The decrypted credentials of the running environment. `Debug` never shows
/// the values.
#[derive(Clone)]
pub struct Credentials {
    values: Value,
    environment: String,
}

impl Credentials {
    /// Credentials from already-decrypted `values`, for `environment`. Useful
    /// in tests; apps get theirs from [`Credentials::from_context`].
    #[must_use]
    pub fn new(values: Value, environment: impl Into<String>) -> Self {
        Self {
            values,
            environment: environment.into(),
        }
    }

    /// Decrypts the credentials environment `env` boots with, from `folder`;
    /// empty when there is no credentials file.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be decrypted or parsed.
    pub fn load_from(env: &Environment, folder: &Path, master_key: Option<&str>) -> Result<Self> {
        let env = env.to_string();
        let loaded = store::load(folder, &env, master_key)?;
        Ok(Self::new(loaded.values, env))
    }

    /// The credentials [`install`] put in `ctx.shared_store`.
    ///
    /// # Errors
    ///
    /// When `install` was not called in `App::after_context`.
    pub fn from_context(ctx: &AppContext) -> Result<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            Error::string(
                "credentials are not installed: call `loco_kit_credentials::install(&ctx)` \
                 in `App::after_context`",
            )
        })
    }

    /// The credential at a dotted `path`, e.g. `stripe.secret_key`.
    ///
    /// # Errors
    ///
    /// When it is missing or does not deserialize into `T`.
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.get_opt(path)?.ok_or_else(|| {
            Error::Message(format!(
                "missing credential `{path}` for environment `{}` \
                 (add it with `cargo loco credentials edit`)",
                self.environment
            ))
        })
    }

    /// The environment these credentials were loaded for.
    #[must_use]
    pub fn environment(&self) -> &str {
        &self.environment
    }

    /// The credential at `path`, or `None` when it is missing.
    ///
    /// # Errors
    ///
    /// When it exists but does not deserialize into `T`.
    pub fn get_opt<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        overlay::lookup(&self.values, path)
            .map(|value| {
                T::deserialize(value).map_err(|err| {
                    Error::Message(format!("credential `{path}` has the wrong type: {err}"))
                })
            })
            .transpose()
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("environment", &self.environment)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn credentials() -> Credentials {
        Credentials::new(
            json!({ "stripe": { "secret_key": "sk", "retries": 3 } }),
            "production",
        )
    }

    #[test]
    fn gets_by_dotted_path() {
        let creds = credentials();
        assert_eq!(creds.get::<String>("stripe.secret_key").unwrap(), "sk");
        assert_eq!(creds.get::<u8>("stripe.retries").unwrap(), 3);
        assert_eq!(creds.get_opt::<String>("stripe.webhook").unwrap(), None);
    }

    #[test]
    fn a_missing_credential_names_path_and_environment() {
        let err = credentials()
            .get::<String>("stripe.webhook")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("missing credential `stripe.webhook` for environment `production`"),
            "{err}"
        );
    }

    #[test]
    fn a_wrong_type_is_an_error() {
        let err = credentials()
            .get_opt::<u8>("stripe.secret_key")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("credential `stripe.secret_key` has the wrong type"),
            "{err}"
        );
    }

    #[test]
    fn knows_its_environment() {
        assert_eq!(credentials().environment(), "production");
    }

    #[test]
    fn debug_hides_the_values() {
        assert!(!format!("{:?}", credentials()).contains("sk"));
    }
}

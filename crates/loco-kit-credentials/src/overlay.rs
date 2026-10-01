//! Fills `credential:<path>` placeholders in the loaded config.
//!
//! This runs on the config Loco has already rendered and parsed, so it cannot
//! tell where a string came from: **every** string value starting with
//! `credential:` is replaced by that credential — a `get_env` default, a value
//! written literally in YAML, or an environment variable's value alike. Any
//! other value is left alone, and a missing credential stops boot.
//!
//! Write it as a `get_env` default so the environment variable wins:
//!
//! ```yaml
//! secret: <%= get_env(name="JWT_SECRET", default="credential:jwt.secret") %>
//! ```
//!
//! When `JWT_SECRET` is set, `get_env` returns it and no placeholder exists;
//! when it is unset, the default becomes the placeholder.
//!
//! Only string-typed config fields can hold a placeholder: Loco deserializes
//! the YAML before this runs, so a placeholder in a numeric field fails there.

use std::path::Path;

use loco_rs::{config::Config, Error, Result};
use serde_json::Value;

use super::store::Loaded;

pub const PLACEHOLDER_PREFIX: &str = "credential:";

/// Replaces every placeholder in `config` with its credential. Credentials no
/// placeholder names stay out of the config.
///
/// # Errors
///
/// When a placeholder names a credential that does not exist, or the result
/// no longer deserializes into [`Config`].
pub fn apply(config: Config, credentials: &Loaded) -> Result<Config> {
    let mut value = serde_json::to_value(&config)?;
    fill(&mut value, &mut Vec::new(), credentials)?;
    Ok(serde_json::from_value(value)?)
}

/// The value at a dotted `path` (`stripe.secret_key`).
#[must_use]
pub fn lookup<'a>(values: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(values, |node, segment| node.as_object()?.get(segment))
}

fn fill(value: &mut Value, at: &mut Vec<String>, credentials: &Loaded) -> Result<()> {
    match value {
        Value::String(text) => {
            if let Some(path) = text.strip_prefix(PLACEHOLDER_PREFIX) {
                *value = lookup(&credentials.values, path)
                    .cloned()
                    .ok_or_else(|| missing(&at.join("."), path, credentials.file.as_deref()))?;
            }
        }
        Value::Object(map) => {
            for (key, child) in map {
                at.push(key.clone());
                fill(child, at, credentials)?;
                at.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter_mut().enumerate() {
                at.push(index.to_string());
                fill(child, at, credentials)?;
                at.pop();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn missing(config_path: &str, credential: &str, file: Option<&Path>) -> Error {
    let source = file.map_or_else(
        || "no credentials file exists for this environment".to_string(),
        |file| format!("not found in {}", file.display()),
    );
    Error::Message(format!(
        "{config_path}: credential `{credential}` {source} \
         (set its environment variable, or add it with `cargo loco credentials edit`)"
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use loco_rs::environment::Environment;
    use serde_json::json;

    use super::*;

    fn loaded(values: Value) -> Loaded {
        Loaded {
            values,
            file: Some(PathBuf::from("config/credentials/test.yml.enc")),
        }
    }

    fn test_config() -> Config {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/config");
        Environment::Test.load_from_folder(&folder).unwrap()
    }

    fn with_jwt_secret(secret: &str) -> Config {
        let mut value = serde_json::to_value(test_config()).unwrap();
        value["auth"]["jwt"]["secret"] = json!(secret);
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn fills_a_placeholder() {
        let config = with_jwt_secret("credential:jwt.secret");
        let config = apply(config, &loaded(json!({ "jwt": { "secret": "s3cret" } }))).unwrap();
        assert_eq!(config.get_jwt_config().unwrap().secret, "s3cret");
    }

    #[test]
    fn leaves_other_values_alone() {
        let config = with_jwt_secret("literal");
        let config = apply(config, &loaded(json!({ "jwt": { "secret": "s3cret" } }))).unwrap();
        assert_eq!(config.get_jwt_config().unwrap().secret, "literal");
    }

    #[test]
    fn a_missing_credential_names_both_paths() {
        let config = with_jwt_secret("credential:jwt.secret");
        let err = apply(config, &loaded(json!({ "jwt": {} })))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(
                "auth.jwt.secret: credential `jwt.secret` not found in config/credentials/test.yml.enc"
            ),
            "{err}"
        );
    }

    #[test]
    fn without_a_file_says_so() {
        let config = with_jwt_secret("credential:jwt.secret");
        let err = apply(config, &Loaded::default()).unwrap_err().to_string();
        assert!(err.contains("no credentials file exists"), "{err}");
    }

    #[test]
    fn non_string_credentials_are_substituted_as_is() {
        let mut value = json!({ "a": "credential:n", "b": ["credential:m"] });
        fill(
            &mut value,
            &mut Vec::new(),
            &loaded(json!({ "n": 5, "m": { "x": true } })),
        )
        .unwrap();
        assert_eq!(value, json!({ "a": 5, "b": [{ "x": true }] }));
    }

    #[test]
    fn unreferenced_credentials_stay_out_of_the_config() {
        let config = with_jwt_secret("credential:jwt.secret");
        let credentials =
            json!({ "jwt": { "secret": "s3cret" }, "stripe": { "secret_key": "sk_live" } });
        let config = apply(config, &loaded(credentials)).unwrap();

        // What `cargo loco doctor --config` prints.
        let printed = config.to_string();
        assert!(printed.contains("s3cret"), "{printed}");
        assert!(!printed.contains("sk_live"), "{printed}");
        // `settings:` is untouched.
        assert_eq!(config.settings.unwrap()["uploads"]["driver"], "memory");
    }

    #[test]
    fn lookup_follows_dotted_paths() {
        let values = json!({ "a": { "b": { "c": 1 } } });
        assert_eq!(lookup(&values, "a.b.c"), Some(&json!(1)));
        assert_eq!(lookup(&values, "a.b"), Some(&json!({ "c": 1 })));
        assert_eq!(lookup(&values, "a.x"), None);
        assert_eq!(lookup(&values, "a.b.c.d"), None);
    }
}

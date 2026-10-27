//! `cargo loco credentials edit|show [-e <env>]`.
//!
//! Runs without booting the app, so a missing or broken credentials file can
//! always be repaired. Without `-e` it works on the global
//! `config/credentials.yml.enc`; with `-e <env>` on
//! `config/credentials/<env>.yml.enc` (created if absent), like
//! `rails credentials:edit --environment`.

use std::{
    ffi::OsString,
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::Command as Process,
};

use clap::{Parser, Subcommand};
use loco_rs::{Error, Result};

use super::{
    cipher::MasterKey,
    store::{self, Location, MASTER_KEY_ENV},
};

#[derive(Debug, Parser)]
#[command(
    bin_name = "cargo loco credentials",
    about = "Edit or show the encrypted credentials"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Decrypt into $VISUAL/$EDITOR, then re-encrypt. Creates the file (and
    /// its key) when it does not exist.
    Edit {
        /// Edit `config/credentials/<ENV>.yml.enc` instead of the global file.
        #[arg(short, long)]
        environment: Option<String>,
    },
    /// Print the decrypted credentials.
    Show {
        /// Show `config/credentials/<ENV>.yml.enc` instead of the global file.
        #[arg(short, long)]
        environment: Option<String>,
    },
}

/// Entry point for `src/bin/main.rs`. `args` starts at `credentials`.
///
/// # Errors
///
/// When the command fails; the error says why.
pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<()> {
    let cli = Cli::parse_from(args);
    let folder = store::config_folder();
    let master_key = store::master_key_from_env();

    match cli.command {
        Command::Edit { environment } => {
            let location = location(&folder, environment.as_deref());
            let outcome = edit(
                &location,
                master_key.as_deref(),
                &editor(),
                &mut reopen_prompt,
            )?;
            report(&location, &outcome);
        }
        Command::Show { environment } => {
            let location = location(&folder, environment.as_deref());
            print!("{}", show(&location, master_key.as_deref())?);
        }
    }
    Ok(())
}

fn location(folder: &std::path::Path, environment: Option<&str>) -> Location {
    environment.map_or_else(
        || Location::global(folder),
        |env| Location::for_env(folder, env),
    )
}

/// `$VISUAL`, else `$EDITOR`, else `vi`.
fn editor() -> String {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string())
}

fn reopen_prompt(err: &Error) -> bool {
    eprintln!("{err}");
    eprint!("Re-open editor? [Y/n] ");
    let _ = io::stderr().flush();
    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "" | "y" | "yes")
}

fn report(location: &Location, outcome: &Outcome) {
    if let Some(key) = &outcome.key_created {
        println!(
            "Generated key {} — store it somewhere safe and never commit it.\n\
             Deployments can pass it as `{MASTER_KEY_ENV}` instead.",
            key.display()
        );
    }
    match outcome.result {
        Saved::Written => println!("Saved {}", location.file.display()),
        Saved::Unchanged => println!("No changes to {}", location.file.display()),
        Saved::Aborted => println!("Discarded; {} is unchanged", location.file.display()),
    }
}

/// What [`edit`] did.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The key file it generated, for a brand-new credentials file.
    pub key_created: Option<PathBuf>,
    /// Whether the file was written.
    pub result: Saved,
}

/// What [`edit`] did with the credentials file.
#[derive(Debug, PartialEq, Eq)]
pub enum Saved {
    /// The edited content was encrypted and saved.
    Written,
    /// The content did not change, so the file was left byte for byte as it
    /// was.
    Unchanged,
    /// The edit was invalid and the user declined to re-open the editor.
    Aborted,
}

/// Decrypts `location` into a private temp file, opens `editor` on it and
/// re-encrypts the result. `reopen` decides, given the validation error,
/// whether to re-open the editor on invalid YAML.
///
/// # Errors
///
/// When the file exists without a usable key, cannot be decrypted, or the
/// editor fails.
pub fn edit(
    location: &Location,
    master_key: Option<&str>,
    editor: &str,
    reopen: &mut dyn FnMut(&Error) -> bool,
) -> Result<Outcome> {
    let exists = location.file.exists();
    let mut key_created = None;
    let key = if exists {
        location.require_key(master_key)?
    } else if let Some(key) = location.read_key(master_key)? {
        key
    } else {
        let key = MasterKey::generate();
        location.write_key(&key)?;
        key_created = Some(location.key.clone());
        key
    };

    let original = if exists {
        location.decrypt(&key)?
    } else {
        template(location)
    };

    let tmp = tempfile::Builder::new()
        .prefix("credentials-")
        .suffix(".yml")
        .tempfile()?;
    fs::write(tmp.path(), &original)?;

    loop {
        open_editor(editor, tmp.path())?;
        let edited = fs::read_to_string(tmp.path())?;
        if let Err(err) = store::parse(&edited) {
            if reopen(&err) {
                continue;
            }
            return Ok(Outcome {
                key_created,
                result: Saved::Aborted,
            });
        }
        if exists && edited == original {
            return Ok(Outcome {
                key_created,
                result: Saved::Unchanged,
            });
        }
        location.write(&key, &edited)?;
        return Ok(Outcome {
            key_created,
            result: Saved::Written,
        });
    }
}

/// The decrypted contents of `location`.
///
/// # Errors
///
/// When the file does not exist or cannot be decrypted.
pub fn show(location: &Location, master_key: Option<&str>) -> Result<String> {
    location
        .read(master_key)?
        .ok_or_else(|| Error::Message(format!("{} does not exist", location.file.display())))
}

/// Runs `editor` on `path` through `sh`, so `code --wait` works as well as
/// `vim`.
fn open_editor(editor: &str, path: &std::path::Path) -> Result<()> {
    let status = Process::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path)
        .status()
        .map_err(|err| Error::Message(format!("cannot run editor `{editor}`: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Message(format!(
            "editor `{editor}` exited with {status}; credentials unchanged"
        )))
    }
}

fn template(location: &Location) -> String {
    format!(
        "# Encrypted credentials, stored in {file}.\n\
         # Edit with `cargo loco credentials edit`; never commit the key ({key}).\n\
         #\n\
         # Code reads them with `Credentials::from_context(&ctx)?.get::<String>(\"stripe.secret_key\")`.\n\
         # Config reads them as a get_env default, so the environment variable wins:\n\
         #   secret: <%= get_env(name=\"JWT_SECRET\", default=\"credential:jwt.secret\") %>\n\
         #\n\
         # jwt:\n\
         #   secret: change-me\n",
        file = location.file.display(),
        key = location.key.display(),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// An "editor" that overwrites the file with `contents`.
    fn writes(dir: &std::path::Path, contents: &str) -> String {
        let script = dir.join(format!("editor-{}.sh", contents.len()));
        let source = dir.join(format!("contents-{}.yml", contents.len()));
        fs::write(&source, contents).unwrap();
        fs::write(&script, format!("cp '{}' \"$1\"\n", source.display())).unwrap();
        format!("sh '{}'", script.display())
    }

    fn never(_: &Error) -> bool {
        false
    }

    #[test]
    fn first_edit_creates_key_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::for_env(dir.path(), "production");

        let outcome = edit(&location, None, &writes(dir.path(), "a: 1\n"), &mut never).unwrap();

        assert_eq!(outcome.key_created, Some(location.key.clone()));
        assert_eq!(outcome.result, Saved::Written);
        let loaded = store::load(dir.path(), "production", None).unwrap();
        assert_eq!(loaded.values, json!({ "a": 1 }));
    }

    #[test]
    fn saving_the_template_as_is_still_creates_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());

        let outcome = edit(&location, None, "true", &mut never).unwrap();

        assert_eq!(outcome.result, Saved::Written);
        assert_eq!(show(&location, None).unwrap(), template(&location));
    }

    #[test]
    fn uses_the_env_key_instead_of_generating_one() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        let key = MasterKey::generate().to_hex();

        let outcome = edit(
            &location,
            Some(&key),
            &writes(dir.path(), "a: 1\n"),
            &mut never,
        )
        .unwrap();

        assert_eq!(outcome.key_created, None);
        assert!(!location.key.exists());
        assert_eq!(show(&location, Some(&key)).unwrap(), "a: 1\n");
    }

    #[test]
    fn no_changes_leaves_the_file_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        edit(&location, None, &writes(dir.path(), "a: 1\n"), &mut never).unwrap();
        let before = fs::read(&location.file).unwrap();

        let outcome = edit(&location, None, "true", &mut never).unwrap();

        assert_eq!(outcome.result, Saved::Unchanged);
        assert_eq!(fs::read(&location.file).unwrap(), before);
    }

    #[test]
    fn invalid_yaml_declined_leaves_the_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        edit(&location, None, &writes(dir.path(), "a: 1\n"), &mut never).unwrap();
        let before = fs::read(&location.file).unwrap();

        let mut asked = 0;
        let outcome = edit(&location, None, &writes(dir.path(), "a: ["), &mut |_| {
            asked += 1;
            false
        })
        .unwrap();

        assert_eq!(asked, 1);
        assert_eq!(outcome.result, Saved::Aborted);
        assert_eq!(fs::read(&location.file).unwrap(), before);
    }

    #[test]
    fn an_existing_file_without_its_key_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());
        edit(&location, None, &writes(dir.path(), "a: 1\n"), &mut never).unwrap();
        fs::remove_file(&location.key).unwrap();

        let err = edit(&location, None, "true", &mut never)
            .unwrap_err()
            .to_string();

        assert!(err.contains("master.key does not exist"), "{err}");
        assert!(!location.key.exists(), "must not generate a new key");
    }

    #[test]
    fn a_failing_editor_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let location = Location::global(dir.path());

        let err = edit(&location, None, "false", &mut never)
            .unwrap_err()
            .to_string();

        assert!(err.contains("credentials unchanged"), "{err}");
        assert!(!location.file.exists());
    }

    #[test]
    fn show_requires_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = show(&Location::global(dir.path()), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("credentials.yml.enc does not exist"), "{err}");
    }
}

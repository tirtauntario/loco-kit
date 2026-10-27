# loco-kit-credentials

Encrypted credentials for [Loco](https://loco.rs) apps, modeled after Rails'
`credentials.yml.enc`. Secrets live in an encrypted YAML file that you commit
to git. The key that decrypts it stays out of git: on your machine as a key
file, on a server as the `LOCO_MASTER_KEY` environment variable. Each
environment can have its own file and key, so a developer can hold the
development key without being able to read production secrets.

Part of [loco-kit](https://github.com/tirtauntario/loco-kit). Unofficial: not
part of the Loco project and not maintained by its authors.

## Install

The crate is not on crates.io yet. Depend on a release tag:

```toml
[dependencies]
loco-kit-credentials = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-credentials-v0.1.0" }
```

It needs loco-rs 1.2 and Rust 1.94 or newer.

## Setup

1. Load the config through the kit, and put the credentials in the shared
   store, in `src/app.rs`:

   ```rust
   impl Hooks for App {
       async fn load_config(env: &Environment) -> Result<Config> {
           loco_kit_credentials::load_config(env)
       }

       async fn after_context(ctx: AppContext) -> Result<AppContext> {
           loco_kit_credentials::install(&ctx)?;
           Ok(ctx)
       }
       // …
   }
   ```

2. Run the `credentials` command before Loco's CLI in `src/bin/main.rs`, so
   it works without booting the app:

   ```rust
   use loco_rs::cli;
   use migration::Migrator;
   use myapp::app::App;

   #[tokio::main]
   async fn main() -> loco_rs::Result<()> {
       if std::env::args_os().nth(1).is_some_and(|arg| arg == "credentials") {
           if let Err(err) = loco_kit_credentials::cli::run(std::env::args_os().skip(1)) {
               eprintln!("error: {err}");
               std::process::exit(1);
           }
           return Ok(());
       }
       cli::main::<App, Migrator>().await
   }
   ```

   Replace `myapp` with your app's crate name. An app without a database
   calls `cli::main::<App>()` instead.

3. Keep the keys out of git. Add to `.gitignore`:

   ```gitignore
   config/master.key
   config/credentials/*.key
   ```

4. Create the first file and its key:

   ```sh
   cargo loco credentials edit
   ```

## Files

Environment `E` boots with the first file that exists:

| File | Key (`LOCO_MASTER_KEY`, when set, beats both) |
|---|---|
| `config/credentials/E.yml.enc` | `config/credentials/E.key` |
| `config/credentials.yml.enc` | `config/master.key` |
| neither: no credentials | none needed |

- The per-environment file **replaces** the global one. They are never
  merged.
- Commit the `*.yml.enc` files. Never commit a `*.key` file. Share a key
  through a password manager.
- `LOCO_CONFIG_FOLDER` moves the `config/` folder, as it does for Loco.

## Editing

```sh
cargo loco credentials edit                  # config/credentials.yml.enc
cargo loco credentials edit -e production    # config/credentials/production.yml.enc
cargo loco credentials show -e production    # print the decrypted YAML
```

`edit` decrypts the file into a temporary file, opens it in `$VISUAL` or
`$EDITOR` (else `vi`), checks the YAML when the editor exits and encrypts it
again. An editor that runs in the background needs its wait flag, such as
`EDITOR="code --wait"`.

The content is YAML with a mapping at the top:

```yaml
jwt:
  secret: 9f3c…
stripe:
  secret_key: sk_live_…
```

## Using the credentials

**From config.** Any string value `credential:<path>` in the config is
replaced by that credential at boot. Write it as a `get_env` default, so the
environment variable wins when it is set:

```yaml
# config/production.yaml
auth:
  jwt:
    secret: <%= get_env(name="JWT_SECRET", default="credential:jwt.secret") %>
```

Precedence: the environment variable when set, else the credential, else boot
fails with an error that names both the config path and the credential.

**From code.** Read any credential by its dotted path:

```rust
use loco_kit_credentials::Credentials;

let credentials = Credentials::from_context(&ctx)?;
let key: String = credentials.get("stripe.secret_key")?;           // missing: error
let hook: Option<String> = credentials.get_opt("stripe.webhook")?;  // missing: None
```

`get` deserializes into any type, so a credential can be a number, a list or
a whole mapping (`credentials.get::<MyStripeConfig>("stripe")`).

## How it behaves

- **A file without a usable key stops boot**, in every environment. The
  error names the file and the key it looked for. A malformed key (not 64 hex
  characters), a wrong key and an altered file are errors too.
- **No file is fine.** An environment with no credentials file boots with
  empty credentials. Only a placeholder or a `get` that needs a value fails.
- **The first `edit` of a file creates its key**, unless `LOCO_MASTER_KEY` is
  set, in which case that key is used and no key file is written. Key files
  are created readable only by their owner, and an existing key file is never
  overwritten.
- **`edit` never writes broken content.** Invalid YAML, or a top level that is
  not a mapping, asks whether to reopen the editor. Saying no leaves the file
  unchanged. If the editor exits with an error, or an existing file's content
  did not change, the file is not touched.
- **`edit` and `show` run without booting the app**, so a broken file can
  always be repaired.
- **Secrets stay out of `ctx.config`.** Only the fields a placeholder names
  are filled, so `cargo loco doctor --config` shows no other credential.
  `Credentials` and `MasterKey` print no values in `Debug`.
- **Placeholders are matched on the parsed config.** The kit cannot tell
  where a string came from, so a `credential:` value written literally in
  YAML, or an environment variable whose value starts with `credential:`, is
  filled too.

## Deploys

Give the deployed app its key as `LOCO_MASTER_KEY`. Never deploy the `.key`
files. `LOCO_MASTER_KEY` decrypts whichever file the environment boots with:
`config/credentials/production.yml.enc` when it exists, else the global file.

To rotate a key:

1. `cargo loco credentials show -e production`, and copy the output.
2. Delete `config/credentials/production.yml.enc` and
   `config/credentials/production.key`.
3. `cargo loco credentials edit -e production` creates a new key. Paste the
   content in and save.
4. Update the key wherever it is stored (password manager, deploy secrets).

## Limits

- One file per environment, and one key per file. There is no merge of the
  global and per-environment files.
- Only string fields in the config can hold a placeholder. Loco
  deserializes the config before the kit fills placeholders, so a placeholder
  in a numeric or boolean field fails there.
- `edit` starts the editor through `sh`, so it needs a Unix-like shell.
- No command rotates a key; follow the steps above.

## Encrypting other data

`cipher::MasterKey`, `cipher::encrypt` and `cipher::decrypt` are public, for
an app that wants the same format for its own data: AES-256-GCM with a fresh
random nonce on every write, stored as one line,
`base64(nonce)--base64(ciphertext)`.

## Why it works this way

- `Hooks::load_config` is Loco's hook for building the config, so every entry
  point (`cargo loco start`, tasks, the test harness) gets the credentials,
  and `ctx.config` stays the one place settings come from.
- The kit reads `LOCO_MASTER_KEY`, `LOCO_CONFIG_FOLDER`, `VISUAL` and
  `EDITOR` from the process environment, because it runs before a config
  exists.
- `credentials` is not a Loco task. Tasks boot the app, and booting needs the
  credentials decrypted.

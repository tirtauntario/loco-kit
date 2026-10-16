# loco-kit-credentials

Rails' `credentials.yml.enc` for [Loco](https://loco.rs): secrets live in an
encrypted YAML file committed to git, and are decrypted at boot with a key
that is not. Part of [loco-kit](https://github.com/tirtauntario/loco-kit);
unofficial, not part of the Loco project.

## Setup

Three hooks in the app. In `src/app.rs`, load the config through the kit and
put the credentials in the shared store:

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

In `src/bin/main.rs`, run the `credentials` command before Loco's CLI, so it
works without booting the app:

```rust
#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    if std::env::args_os().nth(1).is_some_and(|arg| arg == "credentials") {
        if let Err(err) = loco_kit_credentials::cli::run(std::env::args_os().skip(1)) {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
        return Ok(());
    }
    loco_rs::cli::main::<App, Migrator>().await
}
```

Add `config/master.key` and `config/credentials/*.key` to `.gitignore`.

## Files

| Environment `E` boots with | Key (`LOCO_MASTER_KEY` beats both) |
|---|---|
| `config/credentials/E.yml.enc`, if it exists | `config/credentials/E.key` |
| else `config/credentials.yml.enc` | `config/master.key` |
| else no credentials | — |

- The per-environment file **replaces** the global one; they are never
  merged.
- `*.yml.enc` files are committed; `*.key` files must never be. Share a key
  through a password manager.
- A credentials file without a usable key stops boot, in every environment.
- `LOCO_CONFIG_FOLDER` moves the `config/` folder, as it does for Loco.

## Editing

```sh
cargo loco credentials edit                  # config/credentials.yml.enc
cargo loco credentials edit -e production    # config/credentials/production.yml.enc
cargo loco credentials show -e production    # print the decrypted YAML
```

`edit` decrypts into `$VISUAL` or `$EDITOR` (`code --wait` works), checks the
YAML on save and re-encrypts. The first edit of a file generates its key.
Both commands run without booting the app, so a broken file can always be
repaired.

The content is YAML with a mapping at the top:

```yaml
jwt:
  secret: 9f3c…
stripe:
  secret_key: sk_live_…
```

## Using them

**From config.** Any string config value `credential:<path>` is replaced by
that credential at boot. Write it as a `get_env` default so the environment
variable wins:

```yaml
auth:
  jwt:
    secret: <%= get_env(name="JWT_SECRET", default="credential:jwt.secret") %>
```

Precedence: the environment variable when set, else the credential, else boot
fails naming both. The replacement runs on the parsed config, so it cannot
tell where a string came from: a placeholder written literally in YAML, or an
environment variable whose value starts with `credential:`, is filled too.
Only string fields can hold a placeholder, because Loco deserializes the YAML
before the credentials are applied.

**From code.** Anything in the file, by dotted path:

```rust
use loco_kit_credentials::Credentials;

let credentials = Credentials::from_context(&ctx)?;
let key: String = credentials.get("stripe.secret_key")?;      // missing → error
let hook: Option<String> = credentials.get_opt("stripe.webhook")?;
```

The credentials never enter `ctx.config`, so `cargo loco doctor --config`
shows only the values a placeholder filled. `Credentials` never prints its
values in `Debug`.

`cipher::MasterKey`, `cipher::encrypt` and `cipher::decrypt` are public too,
for an app that wants to encrypt its own data with the same AES-256-GCM
format.

## Deploys

A deployed app gets its key as `LOCO_MASTER_KEY`; the `.key` files are never
deployed. To rotate a key: `show` the file, delete the `.yml.enc` and `.key`,
`edit` to recreate both, paste the content back, then update the key wherever
it is stored.

## Why it looks like this

- `Hooks::load_config` is Loco's hook for building the config, so every entry
  point (`cargo loco start`, tasks, the test harness) gets the credentials,
  and `ctx.config` stays the one place settings come from.
- It reads `LOCO_MASTER_KEY`, `LOCO_CONFIG_FOLDER`, `VISUAL` and `EDITOR` with
  `std::env`, because it runs before a config exists.
- `credentials` is not a Loco task: tasks boot the app, and booting needs the
  credentials decrypted.

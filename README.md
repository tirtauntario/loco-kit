# loco-kit

[![CI](https://github.com/tirtauntario/loco-kit/actions/workflows/ci.yaml/badge.svg)](https://github.com/tirtauntario/loco-kit/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Add-on crates for [Loco](https://loco.rs) apps. Each kit fills one gap in the
framework and depends on Loco only, never on a particular app. Add the ones
you need; they do not depend on each other.

> These kits are unofficial. They are not part of the Loco project and are not
> maintained by its authors.

| Crate | What it does |
|---|---|
| [`loco-kit-credentials`](crates/loco-kit-credentials) | Rails-style encrypted credentials: `config/credentials.yml.enc` (or one file per environment), a key that stays out of git, and a `cargo loco credentials edit` command |
| [`loco-kit-local-storage`](crates/loco-kit-local-storage) | `ctx.storage` on the local disk or in memory, configured from YAML, with random storage keys spread over nested subdirectories |
| [`loco-kit-uuidv7-primary-key`](crates/loco-kit-uuidv7-primary-key) | UUID v7 primary keys for models and migrations, the same on Postgres, SQLite and MySQL |

## Why

I build apps on Loco and kept missing a few things Rails gives me. Each kit
brings one of them over:

- **Storage.** Rails' Active Storage spreads files across nested
  subdirectories taken from the storage key, so no single directory has to
  hold every upload. `loco-kit-local-storage` does the same for `ctx.storage`,
  which keeps the disk fast as the file count grows.
- **UUID v7 primary keys.** I want UUID v7 as the primary key in the
  database: safe to expose in URLs, yet ordered by time, so inserts and
  indexes behave much like integer ids.
- **Credentials.** One encrypted credentials file per environment, modeled
  after Rails' credentials. The encrypted files live in git; only the key
  stays out.

## Requirements

- loco-rs 1.2 or a later 1.x. The kits depend on `loco-rs = "1.2"`, and CI
  tests them against the newest 1.x every week.
- Rust 1.94 or newer, the minimum loco-rs 1.2 itself needs.

## Installing

The kits are not on crates.io yet. Depend on a release tag in your app's
`Cargo.toml`. Each crate is versioned and tagged on its own, as
`<crate>-v<version>`:

```toml
[dependencies]
loco-kit-credentials = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-credentials-v0.1.0" }
loco-kit-local-storage = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-local-storage-v0.1.0" }
loco-kit-uuidv7-primary-key = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-uuidv7-primary-key-v0.1.0" }
```

Keep only the lines you need. Cargo finds a crate by name anywhere in this
repository, so the URL is the same for every kit. `loco-kit-uuidv7-primary-key`
also goes in `migration/Cargo.toml`. The [tags](https://github.com/tirtauntario/loco-kit/tags)
list every release.

## Quick look

Each crate's README has the full setup. In short:

**Credentials.** Load the config through the kit, then use secrets from
config or code:

```rust
// src/app.rs, in `impl Hooks for App`
async fn load_config(env: &Environment) -> Result<Config> {
    loco_kit_credentials::load_config(env)
}
async fn after_context(ctx: AppContext) -> Result<AppContext> {
    loco_kit_credentials::install(&ctx)?;
    Ok(ctx)
}
```

```yaml
# config/production.yaml: the environment variable wins when it is set
auth:
  jwt:
    secret: <%= get_env(name="JWT_SECRET", default="credential:jwt.secret") %>
```

```rust
use loco_kit_credentials::Credentials;

let key: String = Credentials::from_context(&ctx)?.get("stripe.secret_key")?;
```

Edit them with `cargo loco credentials edit -e production`, after a small
hook in `src/bin/main.rs` ([setup](crates/loco-kit-credentials#setup)).

**Local storage.** Pick the driver in the config and store files under
random keys:

```rust
// src/app.rs, in `impl Hooks for App`
async fn after_context(ctx: AppContext) -> Result<AppContext> {
    loco_kit_local_storage::install(ctx)
}
```

```yaml
initializers:
  storage:
    driver: local # or memory, for tests
    root: storage
```

```rust
use loco_kit_local_storage::StorageKey;

let key = StorageKey::generate();
ctx.storage.upload(&key.path(), &bytes).await?; // stored at ab/cd/abcd…
```

**UUID v7 primary keys.** Migrations use the kit's schema module, and models
fill the id before saving:

```rust
// migration/src/m20260101_000001_posts.rs
use loco_kit_uuidv7_primary_key::schema::*;

create_table(m, "posts", &[("id", ColType::PkUuid), ("title", ColType::String)], &[]).await?;
```

```rust
// src/models/posts.rs, in `before_save`
use loco_kit_uuidv7_primary_key::UuidV7PrimaryKey;

let mut this = self.with_uuidv7_pk()?;
```

To use credentials and local storage together, chain both in one hook:

```rust
async fn after_context(ctx: AppContext) -> Result<AppContext> {
    loco_kit_credentials::install(&ctx)?;
    loco_kit_local_storage::install(ctx)
}
```

## Developing alongside an app

To build an app against a local checkout of this repository, add a `[patch]`
to a `.cargo/config.toml` that neither repository tracks, for example in the
folder that holds both:

```toml
[patch."https://github.com/tirtauntario/loco-kit"]
loco-kit-credentials = { path = "loco-kit/crates/loco-kit-credentials" }
```

Paths are relative to the folder that holds `.cargo/`. While the patch is
active, don't commit the app's `Cargo.lock`: it records the kit as a path
dependency.

## Contributing

Bug reports, ideas and pull requests are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, the checks CI
runs and the rules for a new kit. Report security issues privately, as
described in [SECURITY.md](SECURITY.md). Questions go to
[Discussions](https://github.com/tirtauntario/loco-kit/discussions).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

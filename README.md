# loco-kit

Add-on crates for [Loco](https://loco.rs) apps. Each kit fills one gap in the
framework and depends on Loco only, never on a particular app.

> These kits are unofficial. They are not part of the Loco project and are not
> maintained by its authors.

| Crate | What it does |
|---|---|
| [`loco-kit-credentials`](crates/loco-kit-credentials) | Rails-style encrypted credentials: `config/credentials.yml.enc`, a key that stays out of git, and a `cargo loco credentials edit` command |
| [`loco-kit-local-storage`](crates/loco-kit-local-storage) | `ctx.storage` on the local disk or in memory, configured from YAML, with random sharded storage keys |
| [`loco-kit-uuidv7-primary-key`](crates/loco-kit-uuidv7-primary-key) | UUID v7 primary keys for models and migrations, the same on Postgres, SQLite and MySQL |

All kits target **loco-rs 1.2** and Rust 1.94 or newer.

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

## Installing

The kits are not on crates.io yet. Depend on a release tag:

```toml
[dependencies]
loco-kit-credentials = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-credentials-v0.1.0" }
```

Cargo finds a crate by name anywhere in this workspace, so the same URL works
for every kit. Each crate is versioned and tagged on its own
(`<crate>-v<version>`).

## Developing a kit alongside an app

To build an app against a local checkout of this repo, add a `[patch]` to a
`.cargo/config.toml` that neither repo tracks, for example in the folder that
holds both:

```toml
[patch."https://github.com/tirtauntario/loco-kit"]
loco-kit-credentials = { path = "loco-kit/crates/loco-kit-credentials" }
```

Paths are relative to the folder that holds `.cargo/`. While the patch is
active, don't commit the app's `Cargo.lock`: it records the kit as a path
dependency.

## Contributing

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Releases use [cargo-release](https://github.com/crate-ci/cargo-release), one
crate at a time:

```sh
cargo release minor -p loco-kit-credentials --execute
```

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

# loco-kit-local-storage

Puts a [Loco](https://loco.rs) app's `ctx.storage` on the local disk or in
memory, chosen in the config, and gives stored files random keys spread over
nested subdirectories. Rails' Active Storage stores files the same way, so no
single directory has to hold every upload and the disk stays fast as the file
count grows. Without a storage setup, Loco keeps its null driver, which
rejects every upload.

Part of [loco-kit](https://github.com/tirtauntario/loco-kit). Unofficial: not
part of the Loco project and not maintained by its authors.

S3 needs no kit: enable loco-rs's `storage_aws_s3` feature and build the
driver with `loco_rs::storage::drivers::aws`. `StorageKey` works with any
driver.

## Install

The crate is not on crates.io yet. Depend on a release tag:

```toml
[dependencies]
loco-kit-local-storage = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-local-storage-v0.1.0" }
```

It needs loco-rs 1.2 and Rust 1.94 or newer.

## Setup

1. Build `ctx.storage` from the config, and register the doctor check, in
   `src/app.rs`:

   ```rust
   impl Hooks for App {
       async fn after_context(ctx: AppContext) -> Result<AppContext> {
           loco_kit_local_storage::install(ctx)
       }

       async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
           Ok(vec![Box::new(loco_kit_local_storage::StorageCheck)])
       }
       // …
   }
   ```

2. Add an `initializers.storage` block to each environment's config:

   ```yaml
   # config/development.yaml
   initializers:
     storage:
       driver: local      # local | memory
       root: storage      # local only: created when missing
   ```

   ```yaml
   # config/test.yaml
   initializers:
     storage:
       driver: memory
   ```

   `root` is relative to the working directory. In production, point it at a
   persistent volume, for example
   `root: <%= get_env(name="STORAGE_ROOT", default="storage") %>`.

3. Add the local `root` to `.gitignore` (`storage/`).

## Usage

Store each file under a `StorageKey`, never under the name it was uploaded
with:

```rust
use loco_kit_local_storage::StorageKey;

// Upload. `bytes` is a `bytes::Bytes`.
let key = StorageKey::generate();                // 32 random hex characters
ctx.storage.upload(&key.path(), &bytes).await?;  // stored at ab/cd/abcd…
// Save key.to_string() in your table, with the file's name and content type.

// Download.
let key = StorageKey::parse(&document.storage_key)?;
let bytes: Vec<u8> = ctx.storage.download(&key.path()).await?;
```

To keep kinds of files apart, join the path under a prefix:
`Path::new("documents").join(key.path())`.

`StorageKey` implements `Serialize` and `Deserialize` as a string, and
`FromStr`, `Display` and `TryFrom<String>`. Each way in validates the key.

## How it behaves

- **Paths are sharded two levels deep.** `key.path()` is `ab/cd/<key>`, built
  from the key's first four characters. That is 65,536 leaf directories, so
  100 million files come to about 1,500 per directory.
- **Keys are random (UUID v4), not time-ordered.** A UUID v7 starts with a
  timestamp, so keys made around the same time would all land in the same
  directory.
- **A key read back cannot escape its directory.** `StorageKey::parse`
  accepts exactly 32 lowercase hex characters, so a value from the database
  or a request can never contain `..` or `/`.
- **A missing or invalid `initializers.storage` block stops boot.** So does
  an unknown driver, or a `local` driver without `root`. The error says what
  is wrong.
- **The `memory` driver** keeps files in the process. They are gone when it
  exits and are not shared between processes. It suits tests.
- **The layout is the same on every driver**, so files copy between a local
  folder and a bucket unchanged.

## Doctor

With `StorageCheck` registered, `cargo loco doctor` writes, reads back and
deletes a probe file under `.loco-kit-local-storage/` in the store, and
reports whether that worked.

## Limits

- Two drivers only: `local` and `memory`. Use Loco's own drivers for S3,
  Azure or GCP.
- The crate names and stores bytes. File names, content types, sizes and
  which records own which files belong in the app's own tables.
- `install` replaces `ctx.storage` with a single store. Loco's mirror and
  backup strategies are not configured from YAML.

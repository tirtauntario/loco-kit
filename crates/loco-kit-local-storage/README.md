# loco-kit-local-storage

Puts a [Loco](https://loco.rs) app's `ctx.storage` on the local disk or in
memory, from config, and names stored files with random, sharded keys. Part
of [loco-kit](https://github.com/tirtauntario/loco-kit); unofficial, not part
of the Loco project.

S3 needs no kit: enable Loco's `storage_aws_s3` feature and build the driver
with `loco_rs::storage::drivers::aws`.

## Setup

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

`install` builds `ctx.storage` from the `initializers.storage` block of the
config. A missing or invalid block stops boot; without the kit, Loco's null
driver would reject every upload.

```yaml
initializers:
  storage:
    driver: local        # local | memory
    root: storage        # local: created when missing
```

The `memory` driver suits tests.

## Keys

Store each file under a `StorageKey`, never under its uploaded name:

```rust
use loco_kit_local_storage::StorageKey;

let key = StorageKey::generate();                // 32 random hex characters
ctx.storage.upload(&key.path(), &bytes).await?;  // ab/cd/abcd…
// save key.to_string() with the file's name and content type

let key = StorageKey::parse(&document.storage_key)?;
let bytes: Vec<u8> = ctx.storage.download(&key.path()).await?;
```

- `key.path()` is `ab/cd/<key>`: two levels of 256 directories, so 100
  million files come to about 1,500 per directory. A local store never hits a
  filesystem's per-directory limits or slow listings. The layout is the same
  on every driver, so files copy between stores unchanged.
- `StorageKey::parse` accepts only 32 lowercase hex characters, so a key from
  the database or a request cannot name another path (`..`, `/`).
- Group files with a prefix: `Path::new("documents").join(key.path())`.

Which files exist, their names and content types belong in the app's own
tables; the crate only names and stores the bytes.

## Doctor

With `StorageCheck` registered, `cargo loco doctor` writes, reads back and
deletes a probe file under `.loco-kit-local-storage/` and reports whether the
store works.

# loco-kit-uuidv7-primary-key

UUID v7 primary keys for [Loco](https://loco.rs) models and migrations. A
UUID v7 is a millisecond timestamp followed by random bits. Ids sort by
creation time, so inserts and indexes behave much like an auto-increment key,
but they are safe to put in URLs: they cannot be guessed or counted.

The ids are made in Rust (`Uuid::now_v7()`), not by a database default, so
Postgres, SQLite and MySQL behave the same. Postgres only has `uuidv7()` from
version 18, and SQLite and MySQL have none.

Part of [loco-kit](https://github.com/tirtauntario/loco-kit). Unofficial: not
part of the Loco project and not maintained by its authors.

## Install

The crate is not on crates.io yet. Depend on a release tag, in both the app's
`Cargo.toml` and `migration/Cargo.toml`:

```toml
[dependencies]
loco-kit-uuidv7-primary-key = { git = "https://github.com/tirtauntario/loco-kit", tag = "loco-kit-uuidv7-primary-key-v0.2.0" }
```

It needs loco-rs 1.2 (with Sea-ORM 2.0) and Rust 1.94 or newer.

## Setup

1. In each migration, import the kit's schema module instead of
   `loco_rs::schema`, and use `ColType::PkUuid` for the id.
2. In each model, fill the id in `before_save`.
3. Optionally, copy the kit's generator templates into the app, so new
   migrations, models and scaffolds use UUIDs (see [Generators](#generators)).

## Migrations

The kit's `schema` module re-exports everything in `loco_rs::schema`
unchanged, except `add_reference`, which it replaces with one that adds a
UUID column:

```rust
use loco_kit_uuidv7_primary_key::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "posts",
            &[
                ("id", ColType::PkUuid),
                ("title", ColType::String),
                ("user_id", ColType::Uuid),       // declare each reference column...
                ("editor_id", ColType::UuidNull), // ...nullable for an optional one
            ],
            &[("user", ""), ("user?", "editor_id")], // ...and Loco adds only the foreign key
        )
        .await?;

        add_reference(m, "comments", "post", "").await // comments.post_id: uuid
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "posts").await
    }
}
```

**Always declare reference columns in `create_table` and
`create_join_table`.** Loco types a reference column as a 64-bit integer
unless `cols` already has a column by that name. When it does, Loco adds only
the foreign key, on every database. `add_reference` needs nothing extra: the
kit's version adds a `uuid` column (nullable for `"post?"`).

## Models

Fill the id in the model's `before_save`, in `src/models/<table>.rs`. Starting
from the file Loco generates, add one line:

```rust
use loco_kit_uuidv7_primary_key::UuidV7PrimaryKey;
use sea_orm::entity::prelude::*;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self.with_uuidv7_pk()?; // the added line
        if !insert && this.updated_at.is_unchanged() {
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        }
        Ok(this)
    }
}
```

For a table without timestamps, the body is just `self.with_uuidv7_pk()`.

`cargo loco db entities` writes this file without the call, so **add it to
every new model**. An insert without it fails with a NOT NULL error on `id`.

## How it behaves

- **`with_uuidv7_pk` sets the id only when it is unset.** Updates, and
  fixtures or seeds that carry their own ids, keep theirs.
- **Composite keys are left alone.** Join tables with a two-column primary
  key are not touched.
- **A primary key that is not a UUID is an error** (`DbErr`), so a model with
  an integer key fails loudly instead of silently.
- **`insert_many` skips `before_save`.** Set `id` yourself with
  `loco_kit_uuidv7_primary_key::new_id()`.
- **`add_reference` follows Loco's naming.** The column is `<table>_id`
  unless you pass a name, and the foreign key is named
  `fk-<from>-<column>-to-<to>`, so Loco's `remove_reference` drops it. A
  trailing `?` makes the column nullable with `ON DELETE SET NULL`;
  otherwise deletes and updates cascade.
- **On SQLite, `add_reference` adds only the column.** SQLite cannot add a
  foreign key to an existing table. Loco's own `add_reference` does the same.

## Generators

Loco's generators write integer keys. The kit ships template overrides in
[`templates/`](https://github.com/tirtauntario/loco-kit/tree/loco-kit-uuidv7-primary-key-v0.2.0/crates/loco-kit-uuidv7-primary-key/templates)
that make `cargo loco generate model`, `migration` and `scaffold` write UUID
keys and references. Loco uses a template from the app's `.loco-templates/`
folder whenever one sits at the same path as its built-in template. The
templates are left out of the crate package, so copy them from GitHub. From
the app root:

```sh
git clone --depth 1 --branch loco-kit-uuidv7-primary-key-v0.2.0 \
  https://github.com/tirtauntario/loco-kit /tmp/loco-kit
mkdir -p .loco-templates
cp -R /tmp/loco-kit/crates/loco-kit-uuidv7-primary-key/templates/. .loco-templates/
```

Each one is Loco 1.2's built-in template with only these changes:

| Template | Used by | Change |
| --- | --- | --- |
| `model/model.t` | `generate model`, `generate scaffold`, `generate migration CreatePosts` | Imports the kit's `schema`. The id is `ColType::PkUuid`, and each reference gets a `Uuid` column (`UuidNull` for `user:references?`). |
| `migration/add_references.t` | `generate migration AddUserRefToPosts` | Imports the kit's `schema`, so `add_reference` adds a UUID column. `down` also drops the column, and works for `user:references?`. |
| `migration/join_table.t` | `generate migration CreateJoinTableUsersAndGroups` | Imports the kit's `schema`, and each reference gets a `Uuid` column. |
| `model/test.t` | `generate model`, `generate scaffold` | Adds a test that fails until the model calls `with_uuidv7_pk()`. |
| `scaffold/api/dto.t` | `generate scaffold` | The DTO's `id` and reference fields are `Uuid`, typed `string` in TypeScript. |
| `scaffold/api/controller.t` | `generate scaffold` | The `/{id}` routes take a `Uuid`. |

A template cannot write the model file: `cargo loco db entities` does, after
the migration runs. So after `generate model` or `generate scaffold`, **add
`with_uuidv7_pk()` to `before_save`** in `src/models/<table>.rs` (see
[Models](#models)). The generator prints a reminder, and the generated model
test fails until you do.

Things to know:

- **Reference columns take the field name.** `author:references` adds
  `author_id`, the same name the scaffold's DTO uses. Name the column
  yourself with `author:references:reviewer_id`.
- **Reference a table with a UUID id.** Loco's starter `users` table keeps an
  integer id, so `user:references` only works once `users.id` is a UUID too.
- **The scaffold DTO guesses references by name.** It only sees each field's
  Rust type, so it treats an `int` field ending in `_id` as a reference and
  makes it a `Uuid`. Name such fields differently, or fix the DTO by hand.
- **The React frontend scaffold is not covered.** When the app has a
  `frontend/`, Loco also writes pages that expect numeric ids.
- **The templates follow loco-rs 1.2.** After a Loco upgrade, compare them
  with the new built-ins. `cargo loco generate override <path>` copies a
  built-in into `.loco-templates/`, but skips files that already exist, so run
  it in a scratch app.

## Limits

- Postgres, SQLite and MySQL only. `add_reference` returns an error on any
  other backend.
- Ids carry their creation time to the millisecond. Anyone who sees an id can
  read when the row was created.

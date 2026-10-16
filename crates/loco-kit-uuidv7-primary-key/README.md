# loco-kit-uuidv7-primary-key

UUID v7 primary keys for [Loco](https://loco.rs) models. A UUID v7 is a
millisecond timestamp followed by random bits: ids sort by creation time, so
they index like an auto-increment key, but they cannot be guessed or counted.
Part of [loco-kit](https://github.com/tirtauntario/loco-kit); unofficial, not
part of the Loco project.

Ids are made in Rust (`Uuid::now_v7()`), not by a database default, so
Postgres, SQLite and MySQL behave the same. Postgres only has `uuidv7()` from
version 18, and SQLite and MySQL have none.

Add the crate to both the app and `migration/`.

## Migrations

Import the kit's schema module instead of `loco_rs::schema`. It re-exports
Loco's helpers and replaces `add_reference` with one that adds a UUID
column:

```rust
use loco_kit_uuidv7_primary_key::schema::*;

create_table(
    m,
    "posts",
    &[
        ("id", ColType::PkUuid),
        ("title", ColType::String),
        ("user_id", ColType::Uuid),       // declare each reference column…
        ("editor_id", ColType::UuidNull), // …nullable for an optional one
    ],
    &[("user", ""), ("user?", "editor_id")], // …and Loco adds only the FK
)
.await?;

add_reference(m, "comments", "post", "").await?; // comments.post_id: uuid
```

Loco's `create_table` types a reference column as a 64-bit integer unless
`cols` already declares it, so always declare it.

## Models

Fill the id in the model's `before_save` (`src/models/<table>.rs`):

```rust
use loco_kit_uuidv7_primary_key::UuidV7PrimaryKey;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self.with_uuidv7_pk()?;
        if !insert && this.updated_at.is_unchanged() {
            this.updated_at = ActiveValue::Set(chrono::Utc::now().into());
        }
        Ok(this)
    }
}
```

`with_uuidv7_pk` sets the id only when it is unset, so updates and fixtures
with fixed ids keep theirs. Composite keys (join tables) are left alone.
`cargo loco db entities` writes this file without it, so **add it to every
new model**; an insert without it fails with a NOT NULL error on `id`.
`insert_many` skips `before_save`: set `id` yourself with
`loco_kit_uuidv7_primary_key::new_id()`.

## Generators

Loco's generators write integer keys. To make `cargo loco generate model`,
`migration` and `scaffold` write UUID keys, override their templates in the
app's `.loco-templates/` folder (see Loco's generator docs).

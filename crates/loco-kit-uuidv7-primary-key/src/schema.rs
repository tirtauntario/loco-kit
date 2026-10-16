//! `loco_rs::schema`, with UUID foreign keys.
//!
//! Glob-import this module in place of `loco_rs::schema`. Everything is
//! re-exported unchanged except [`add_reference`], which Loco types as a
//! 64-bit integer.
//!
//! For `create_table` and `create_join_table`, declare each reference column
//! yourself as `ColType::Uuid` (or `UuidNull` for an optional `table?`
//! reference). Loco adds its own integer column only when `cols` does not
//! already have one by that name, so it then adds just the foreign key — on
//! every backend, `SQLite` included:
//!
//! ```ignore
//! use loco_kit_uuidv7_primary_key::schema::*;
//!
//! create_table(
//!     m,
//!     "posts",
//!     &[("id", ColType::PkUuid), ("title", ColType::String), ("user_id", ColType::Uuid)],
//!     &[("user", "")],
//! )
//! .await?;
//! ```

use heck::ToSnakeCase;
pub use loco_rs::schema::*;
use sea_orm::{
    sea_query::{Alias, ForeignKeyAction, TableForeignKey},
    DatabaseBackend, DbErr,
};
use sea_orm_migration::SchemaManager;

/// `UserGroup` -> `user_groups`, as Loco names tables.
fn normalize_table(table: &str) -> String {
    cruet::to_plural(table).to_snake_case()
}

/// users -> `user_id`, as Loco names reference columns.
fn reference_id(table: &str) -> String {
    format!("{}_id", cruet::to_singular(table).to_snake_case())
}

/// Loco's `add_reference`, with a UUID column.
///
/// Adds `<totbl>_id` (or `refname`) to `fromtbl`, referencing `totbl.id`. A
/// trailing `?` on `totbl` makes it nullable with `ON DELETE SET NULL`;
/// otherwise deletes and updates cascade. `SQLite` cannot add a foreign key
/// to an existing table, so there only the column is added, as in Loco.
///
/// # Errors
///
/// When the column or constraint cannot be added.
pub async fn add_reference(
    m: &SchemaManager<'_>,
    fromtbl: &str,
    totbl: &str,
    refname: &str,
) -> Result<(), DbErr> {
    let from = normalize_table(fromtbl);
    let (totbl, nullable) = totbl
        .strip_suffix('?')
        .map_or((totbl, false), |table| (table, true));
    let to = normalize_table(totbl);
    let column = if refname.is_empty() {
        reference_id(&to)
    } else {
        refname.to_owned()
    };

    let col = if nullable {
        uuid_null(Alias::new(&column))
    } else {
        uuid(Alias::new(&column))
    };
    let mut stmt = alter(Alias::new(&from));
    stmt.add_column(col);

    match m.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            let (on_delete, on_update) = if nullable {
                (ForeignKeyAction::SetNull, ForeignKeyAction::NoAction)
            } else {
                (ForeignKeyAction::Cascade, ForeignKeyAction::Cascade)
            };
            stmt.add_foreign_key(
                TableForeignKey::new()
                    // Loco's name, so its `remove_reference` drops it.
                    .name(format!("fk-{from}-{column}-to-{to}"))
                    .from_tbl(Alias::new(&from))
                    .from_col(Alias::new(&column))
                    .to_tbl(Alias::new(&to))
                    .to_col(Alias::new("id"))
                    .on_delete(on_delete)
                    .on_update(on_update),
            );
        }
        DatabaseBackend::Sqlite => {}
        backend => {
            return Err(DbErr::BackendNotSupported {
                db: backend.as_str(),
                ctx: "add_reference",
            });
        }
    }
    m.alter_table(stmt).await
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

    use super::*;

    /// The declared type and NOT NULL flag of `table.name`.
    async fn column(db: &DatabaseConnection, table: &str, name: &str) -> (String, bool) {
        db.query_all_raw(Statement::from_string(
            db.get_database_backend(),
            format!("PRAGMA table_info({table})"),
        ))
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.try_get::<String>("", "name").unwrap() == name)
        .map_or_else(
            || panic!("{table}.{name} should exist"),
            |row| {
                (
                    row.try_get::<String>("", "type").unwrap(),
                    row.try_get::<i32>("", "notnull").unwrap() == 1,
                )
            },
        )
    }

    async fn with_users() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_table(
            &SchemaManager::new(&db),
            "users",
            &[("id", ColType::PkUuid)],
            &[],
        )
        .await
        .unwrap();
        db
    }

    #[test]
    fn names_references_like_loco() {
        assert_eq!(normalize_table("UserGroup"), "user_groups");
        assert_eq!(reference_id("users"), "user_id");
        assert_eq!(reference_id("user_groups"), "user_group_id");
    }

    #[tokio::test]
    async fn declared_reference_columns_stay_uuids() {
        let db = with_users().await;
        let m = SchemaManager::new(&db);
        create_table(
            &m,
            "posts",
            &[
                ("id", ColType::PkUuid),
                ("user_id", ColType::Uuid),
                ("editor_id", ColType::UuidNull),
            ],
            &[("user", ""), ("user?", "editor_id")],
        )
        .await
        .unwrap();

        let id = column(&db, "users", "id").await.0;
        assert_eq!(column(&db, "posts", "user_id").await, (id.clone(), true));
        assert_eq!(column(&db, "posts", "editor_id").await, (id, false));
    }

    #[tokio::test]
    async fn add_reference_adds_a_uuid_column() {
        let db = with_users().await;
        let m = SchemaManager::new(&db);
        create_table(&m, "posts", &[("id", ColType::PkUuid)], &[])
            .await
            .unwrap();
        add_reference(&m, "posts", "user?", "").await.unwrap();

        let id = column(&db, "users", "id").await.0;
        assert_eq!(column(&db, "posts", "user_id").await, (id, false));
    }
}

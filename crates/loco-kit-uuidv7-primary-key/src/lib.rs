//! UUID v7 primary keys for Loco models, on every database Sea-ORM supports.
//!
//! * **ids**: [`new_id`] is a time-ordered UUID v7, made in Rust. No database
//!   default is involved (Postgres needs 18+ for `uuidv7()`, `SQLite` and `MySQL`
//!   have none), so Postgres, `SQLite` and `MySQL` get the same ids.
//! * **models**: [`UuidV7PrimaryKey::with_uuidv7_pk`] fills an unset primary
//!   key; call it from the model's `before_save`.
//! * **migrations**: [`schema`] is `loco_rs::schema` with `create_table`,
//!   `create_join_table` and `add_reference` typing `<table>_id` foreign keys
//!   as UUIDs instead of 64-bit integers.

pub mod schema;

use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, Iterable, PrimaryKeyToColumn};
use uuid::Uuid;

/// A new UUID v7: a millisecond timestamp followed by random bits, so ids
/// sort by creation time and index like an auto-increment key.
#[must_use]
pub fn new_id() -> Uuid {
    Uuid::now_v7()
}

/// Fills a model's UUID primary key. Implemented for every active model.
pub trait UuidV7PrimaryKey: ActiveModelTrait + Sized {
    /// Sets the primary key to [`new_id`] when it is not set yet, and leaves
    /// it alone otherwise, so it is safe on updates and on fixtures that
    /// carry their own ids. Composite keys (join tables) are left alone.
    ///
    /// ```ignore
    /// #[async_trait::async_trait]
    /// impl ActiveModelBehavior for ActiveModel {
    ///     async fn before_save<C>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    ///     where
    ///         C: ConnectionTrait,
    ///     {
    ///         self.with_uuidv7_pk()
    ///     }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// When the primary key column does not hold a UUID.
    fn with_uuidv7_pk(mut self) -> Result<Self, DbErr> {
        let mut keys = <Self::Entity as EntityTrait>::PrimaryKey::iter();
        if let (Some(key), None) = (keys.next(), keys.next()) {
            let column = key.into_column();
            if self.is_not_set(column) {
                self.try_set(column, new_id().into())?;
            }
        }
        Ok(self)
    }
}

impl<A: ActiveModelTrait> UuidV7PrimaryKey for A {}

#[cfg(test)]
mod tests {
    use sea_orm::{
        ActiveModelBehavior, ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbErr,
    };

    use super::*;

    mod notes {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
        #[sea_orm(table_name = "notes")]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub id: Uuid,
            pub title: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}
    }

    #[async_trait::async_trait]
    impl ActiveModelBehavior for notes::ActiveModel {
        async fn before_save<C>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
        where
            C: ConnectionTrait,
        {
            self.with_uuidv7_pk()
        }
    }

    async fn db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = sea_orm::Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(notes::Entity))
            .await
            .unwrap();
        db
    }

    fn note(title: &str) -> notes::ActiveModel {
        notes::ActiveModel {
            title: Set(title.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn new_ids_are_v7_and_time_ordered() {
        let first = new_id();
        let second = new_id();
        assert_eq!(first.get_version_num(), 7);
        assert!(first < second);
    }

    #[test]
    fn keeps_an_id_that_is_already_set() {
        let id = new_id();
        let model = notes::ActiveModel {
            id: Set(id),
            ..note("kept")
        }
        .with_uuidv7_pk()
        .unwrap();
        assert_eq!(model.id, Set(id));
    }

    #[tokio::test]
    async fn inserts_get_a_v7_id() {
        let db = db().await;
        let first = note("first").insert(&db).await.unwrap();
        let second = note("second").insert(&db).await.unwrap();

        assert_eq!(first.id.get_version_num(), 7);
        assert!(first.id < second.id);
    }

    #[tokio::test]
    async fn updates_keep_the_id() {
        let db = db().await;
        let inserted = note("draft").insert(&db).await.unwrap();

        let mut model: notes::ActiveModel = inserted.clone().into();
        model.title = Set("final".to_owned());
        let updated = model.update(&db).await.unwrap();

        assert_eq!(updated.id, inserted.id);
    }
}

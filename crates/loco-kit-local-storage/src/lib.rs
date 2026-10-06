//! `ctx.storage` on the local disk or in memory, configured from YAML, and
//! sharded storage keys.
//!
//! * **config**: an `initializers.storage` block picks the driver
//!   ([`StorageConfig`]) and [`install`] builds it into `ctx.storage` from
//!   `App::after_context`. Without it Loco keeps its null driver, which
//!   rejects every upload.
//! * **keys**: a [`StorageKey`] is random and stored at `ab/cd/abcd…`, so no
//!   directory of a local store grows past a few thousand entries. The layout
//!   is the same on every driver, so files copy between stores unchanged.
//! * **doctor**: [`StorageCheck`] writes, reads back and deletes a probe file.

pub mod check;
pub mod config;
pub mod key;

use loco_rs::{app::AppContext, Result};

pub use check::StorageCheck;
pub use config::StorageConfig;
pub use key::StorageKey;

/// Replaces `ctx.storage` with the store `initializers.storage` configures.
/// Call it from `App::after_context`.
///
/// # Errors
///
/// When the block is missing or invalid, or its store cannot be built.
pub fn install(ctx: AppContext) -> Result<AppContext> {
    let storage = StorageConfig::from_config(&ctx.config)?.build()?;
    Ok(ctx.into_builder().storage(storage.into()).build())
}

//! `cargo loco doctor`: proves `ctx.storage` works end to end.

use std::path::Path;

use async_trait::async_trait;
use bytes::Bytes;
use loco_rs::{
    app::{AppContext, Initializer},
    doctor::{Check, CheckStatus},
    storage::{Storage, StorageError},
    Result,
};

use crate::StorageKey;

/// Where probe files go; each is deleted right after it is read back.
const PROBE_DIR: &str = ".loco-kit-local-storage";

/// Registered in `App::initializers`, it makes `cargo loco doctor` write,
/// read back and delete a probe file through `ctx.storage`.
pub struct StorageCheck;

#[async_trait]
impl Initializer for StorageCheck {
    fn name(&self) -> String {
        "loco-kit-local-storage".to_string()
    }

    async fn check(&self, ctx: &AppContext) -> Result<Option<Check>> {
        Ok(Some(probe(&ctx.storage).await))
    }
}

/// Writes, reads back and deletes a probe file.
pub async fn probe(storage: &Storage) -> Check {
    let path = Path::new(PROBE_DIR).join(StorageKey::generate().as_str());
    let content = Bytes::from_static(b"loco-kit-local-storage probe");
    let round_trip = async {
        storage.upload(&path, &content).await?;
        let read: Vec<u8> = storage.download(&path).await?;
        storage.delete(&path).await?;
        Ok::<_, StorageError>(read == content)
    };
    match round_trip.await {
        Ok(true) => Check {
            status: CheckStatus::Ok,
            message: "storage: write, read and delete work".to_string(),
            description: None,
        },
        Ok(false) => Check {
            status: CheckStatus::NotOk,
            message: "storage: a file read back differs from what was written".to_string(),
            description: None,
        },
        Err(err) => Check {
            status: CheckStatus::NotOk,
            message: format!("storage: {err}"),
            description: Some("Check `initializers.storage` in the config.".to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use loco_rs::storage::drivers::{mem, null};

    use super::*;

    #[tokio::test]
    async fn a_working_store_passes_and_keeps_nothing() {
        let storage = Storage::single(mem::new());
        let check = probe(&storage).await;
        assert_eq!(check.status, CheckStatus::Ok, "{}", check.message);
        let left = storage.list(Path::new(PROBE_DIR), true).await.unwrap();
        assert!(left.is_empty(), "{left:?}");
    }

    #[tokio::test]
    async fn the_null_store_fails() {
        let check = probe(&Storage::single(null::new())).await;
        assert_eq!(check.status, CheckStatus::NotOk);
        assert!(check.description.is_some());
    }
}

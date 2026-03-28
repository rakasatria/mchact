use std::path::PathBuf;

use async_trait::async_trait;
use tracing::instrument;

use crate::{ObjectStorage, StorageError, StorageResult};

/// A filesystem-backed object storage implementation.
///
/// Each object key is mapped to a file path relative to `base_dir`.
/// Nested keys (e.g. `"a/b/c"`) create the corresponding directory tree.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    base_dir: PathBuf,
}

impl LocalStorage {
    /// Create a new `LocalStorage` rooted at `base_dir`.
    ///
    /// The directory is created if it does not exist.
    pub async fn new(base_dir: impl Into<PathBuf>) -> StorageResult<Self> {
        let base_dir = base_dir.into();
        tokio::fs::create_dir_all(&base_dir).await?;
        Ok(Self { base_dir })
    }

    /// Create a new `LocalStorage` using synchronous I/O.
    ///
    /// Suitable for use in tests or sync contexts where async is not available.
    pub fn new_sync(base_dir: impl Into<PathBuf>) -> StorageResult<Self> {
        let base_dir = base_dir.into();
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    fn full_path(&self, key: &str) -> PathBuf {
        // Prevent path traversal by stripping any leading '/'
        let sanitized = key.trim_start_matches('/');
        self.base_dir.join(sanitized)
    }
}

#[async_trait]
impl ObjectStorage for LocalStorage {
    #[instrument(skip(self, data), fields(key = %key, bytes = data.len()))]
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()> {
        let path = self.full_path(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    #[instrument(skip(self), fields(key = %key))]
    async fn get(&self, key: &str) -> StorageResult<Vec<u8>> {
        let path = self.full_path(key);
        if !path.exists() {
            return Err(StorageError::NotFound(key.to_owned()));
        }
        let data = tokio::fs::read(&path).await?;
        Ok(data)
    }

    #[instrument(skip(self), fields(key = %key))]
    async fn delete(&self, key: &str) -> StorageResult<()> {
        let path = self.full_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        Ok(self.full_path(key).exists())
    }

    fn backend_name(&self) -> &'static str {
        "local"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_storage() -> LocalStorage {
        let dir = std::env::temp_dir().join(format!(
            "mchact-storage-test-{}",
            uuid::Uuid::new_v4()
        ));
        LocalStorage::new(dir).await.expect("failed to create temp storage")
    }

    #[tokio::test]
    async fn test_put_get_delete_cycle() {
        let storage = temp_storage().await;
        let key = "hello.txt";
        let data = b"hello, world!".to_vec();

        // Initially the key should not exist
        assert!(!storage.exists(key).await.unwrap());

        // Put data
        storage.put(key, data.clone()).await.unwrap();
        assert!(storage.exists(key).await.unwrap());

        // Get data back and verify it matches
        let retrieved = storage.get(key).await.unwrap();
        assert_eq!(retrieved, data);

        // Delete and verify it's gone
        storage.delete(key).await.unwrap();
        assert!(!storage.exists(key).await.unwrap());
    }

    #[tokio::test]
    async fn test_put_creates_parent_directories() {
        let storage = temp_storage().await;
        let key = "a/b/c/deep.bin";
        let data = vec![1u8, 2, 3, 4];

        storage.put(key, data.clone()).await.unwrap();
        let retrieved = storage.get(key).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_get_missing_key_returns_not_found() {
        let storage = temp_storage().await;
        let err = storage.get("does-not-exist.txt").await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_key_is_noop() {
        let storage = temp_storage().await;
        // Should not return an error
        storage.delete("phantom.txt").await.unwrap();
    }

    #[tokio::test]
    async fn test_backend_name() {
        let storage = temp_storage().await;
        assert_eq!(storage.backend_name(), "local");
    }
}

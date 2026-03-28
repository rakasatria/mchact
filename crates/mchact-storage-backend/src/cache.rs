use async_trait::async_trait;

use crate::{ObjectStorage, StorageResult};

/// A passthrough storage wrapper that will eventually host an LRU cache.
///
/// Currently all operations are delegated directly to the inner backend.
/// Full LRU caching is implemented in Task 5.
pub struct CachedStorage<B> {
    inner: B,
}

impl<B: ObjectStorage> CachedStorage<B> {
    /// Wrap `inner` with a (currently passthrough) cache layer.
    pub fn new(inner: B) -> Self {
        Self { inner }
    }

    /// Return a reference to the inner backend.
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

#[async_trait]
impl<B: ObjectStorage> ObjectStorage for CachedStorage<B> {
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()> {
        self.inner.put(key, data).await
    }

    async fn get(&self, key: &str) -> StorageResult<Vec<u8>> {
        self.inner.get(key).await
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.inner.delete(key).await
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.inner.exists(key).await
    }

    fn backend_name(&self) -> &'static str {
        self.inner.backend_name()
    }
}

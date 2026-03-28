use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::{ObjectStorage, StorageResult};

// ---------------------------------------------------------------------------
// Metadata types
// ---------------------------------------------------------------------------

const META_FILE: &str = ".cache_meta.json";

/// Per-entry metadata stored in the LRU cache index.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// Size of the cached file in bytes.
    size_bytes: u64,
    /// Unix epoch seconds of the last access, used for LRU eviction ordering.
    last_accessed_epoch: u64,
}

/// In-memory (and persisted) cache index.
#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheMetadata {
    /// key → entry metadata.
    entries: HashMap<String, CacheEntry>,
    /// Running total of all cached bytes.
    total_bytes: u64,
}

impl CacheMetadata {
    /// Insert or update an entry and adjust the running total.
    fn record(&mut self, key: &str, size_bytes: u64, epoch: u64) {
        if let Some(existing) = self.entries.get(key) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.size_bytes);
        }
        self.entries.insert(
            key.to_owned(),
            CacheEntry {
                size_bytes,
                last_accessed_epoch: epoch,
            },
        );
        self.total_bytes = self.total_bytes.saturating_add(size_bytes);
    }

    /// Remove an entry and adjust the running total.  Returns the evicted entry if found.
    fn remove(&mut self, key: &str) -> Option<CacheEntry> {
        let entry = self.entries.remove(key)?;
        self.total_bytes = self.total_bytes.saturating_sub(entry.size_bytes);
        Some(entry)
    }

    /// Update the last-accessed timestamp for an existing entry.
    fn touch(&mut self, key: &str, epoch: u64) {
        if let Some(e) = self.entries.get_mut(key) {
            e.last_accessed_epoch = epoch;
        }
    }

    /// Return the key with the oldest `last_accessed_epoch`, if any entries exist.
    fn lru_key(&self) -> Option<String> {
        self.entries
            .iter()
            .min_by_key(|(_, e)| e.last_accessed_epoch)
            .map(|(k, _)| k.clone())
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Sanitize a key so it is safe to use as a filename component inside
/// `cache_dir`.  Slashes become underscores; leading dots are prefixed.
fn key_to_filename(key: &str) -> String {
    let sanitized = key.replace('/', "__").replace('\\', "__");
    if sanitized.starts_with('.') {
        format!("_{sanitized}")
    } else {
        sanitized
    }
}

// ---------------------------------------------------------------------------
// CachedStorage
// ---------------------------------------------------------------------------

/// An [`ObjectStorage`] wrapper that adds a local LRU disk cache in front of
/// any backend storage implementation.
///
/// # Write-through behaviour
/// Every `put` writes to both the cache directory *and* the backend.
///
/// # Eviction
/// After each write the cache checks whether `total_bytes > max_bytes`.  If so
/// it removes the least-recently-used entry until the limit is satisfied.
///
/// # Persistence
/// Cache metadata (sizes + access times) is serialised as JSON to
/// `{cache_dir}/.cache_meta.json` so the index survives process restarts.
pub struct CachedStorage {
    backend: Box<dyn ObjectStorage>,
    cache_dir: PathBuf,
    max_bytes: u64,
    metadata: Mutex<CacheMetadata>,
}

impl CachedStorage {
    /// Create a new `CachedStorage`.
    ///
    /// * `backend`   — the actual remote/local storage to fall back to.
    /// * `cache_dir` — local directory used for cached files (created if absent).
    /// * `max_bytes` — maximum total size of all cached files before LRU eviction kicks in.
    ///
    /// Existing metadata is loaded from `{cache_dir}/.cache_meta.json` if present.
    pub async fn new(
        backend: Box<dyn ObjectStorage>,
        cache_dir: impl Into<PathBuf>,
        max_bytes: u64,
    ) -> StorageResult<Self> {
        let cache_dir = cache_dir.into();
        tokio::fs::create_dir_all(&cache_dir).await?;

        let metadata = Self::load_metadata(&cache_dir).await;

        Ok(Self {
            backend,
            cache_dir,
            max_bytes,
            metadata: Mutex::new(metadata),
        })
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn cache_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(key_to_filename(key))
    }

    fn meta_path(&self) -> PathBuf {
        self.cache_dir.join(META_FILE)
    }

    async fn load_metadata(cache_dir: &PathBuf) -> CacheMetadata {
        let path = cache_dir.join(META_FILE);
        match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => CacheMetadata::default(),
        }
    }

    async fn persist_metadata(&self, meta: &CacheMetadata) {
        match serde_json::to_vec(meta) {
            Ok(bytes) => {
                if let Err(e) = tokio::fs::write(self.meta_path(), bytes).await {
                    warn!("failed to persist cache metadata: {e}");
                }
            }
            Err(e) => warn!("failed to serialise cache metadata: {e}"),
        }
    }

    /// Write `data` to the cache directory for `key`.
    async fn write_cache_file(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let path = self.cache_path(key);
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    /// Read a cached file.  Returns `None` if the file does not exist.
    async fn read_cache_file(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.cache_path(key);
        tokio::fs::read(&path).await.ok()
    }

    /// Remove the cached file for `key`, ignoring errors if it is absent.
    async fn remove_cache_file(&self, key: &str) {
        let path = self.cache_path(key);
        let _ = tokio::fs::remove_file(&path).await;
    }

    /// Evict LRU entries until `total_bytes <= max_bytes`.
    ///
    /// Must be called while holding the metadata lock — the lock guard is
    /// passed in to avoid re-acquiring it.
    async fn evict_if_needed(&self, meta: &mut CacheMetadata) {
        while meta.total_bytes > self.max_bytes {
            match meta.lru_key() {
                None => break,
                Some(victim) => {
                    debug!(key = %victim, "LRU evicting cache entry");
                    meta.remove(&victim);
                    self.remove_cache_file(&victim).await;
                }
            }
        }
        self.persist_metadata(meta).await;
    }
}

#[async_trait]
impl ObjectStorage for CachedStorage {
    /// Write-through: store in cache dir **and** the backend, then evict if needed.
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()> {
        // Write to backend first so the data is durable regardless of cache state.
        self.backend.put(key, data.clone()).await?;

        // Write to cache dir.
        self.write_cache_file(key, &data).await?;

        let epoch = now_epoch();
        let size = data.len() as u64;
        let mut meta = self.metadata.lock().await;
        meta.record(key, size, epoch);
        self.evict_if_needed(&mut meta).await;

        Ok(())
    }

    /// Cache-first read.  On a miss, fetch from backend and populate the cache.
    async fn get(&self, key: &str) -> StorageResult<Vec<u8>> {
        // --- cache hit path ---
        if let Some(data) = self.read_cache_file(key).await {
            let epoch = now_epoch();
            let mut meta = self.metadata.lock().await;
            meta.touch(key, epoch);
            // If the entry somehow isn't in metadata yet, record it now.
            if !meta.entries.contains_key(key) {
                meta.record(key, data.len() as u64, epoch);
            }
            self.persist_metadata(&meta).await;
            return Ok(data);
        }

        // --- cache miss path ---
        let data = self.backend.get(key).await?;

        // Populate cache.
        self.write_cache_file(key, &data).await?;

        let epoch = now_epoch();
        let size = data.len() as u64;
        let mut meta = self.metadata.lock().await;
        meta.record(key, size, epoch);
        self.evict_if_needed(&mut meta).await;

        Ok(data)
    }

    /// Remove from both cache and backend.
    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.remove_cache_file(key).await;

        let mut meta = self.metadata.lock().await;
        meta.remove(key);
        self.persist_metadata(&meta).await;
        drop(meta);

        self.backend.delete(key).await?;
        Ok(())
    }

    /// Cache-first existence check; falls back to backend.
    async fn exists(&self, key: &str) -> StorageResult<bool> {
        if self.cache_path(key).exists() {
            return Ok(true);
        }
        self.backend.exists(key).await
    }

    /// Delegates to the inner backend; the cache layer has no index of all keys.
    async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>> {
        self.backend.list_keys(prefix).await
    }

    fn backend_name(&self) -> &'static str {
        "cached"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "local")]
    use crate::local::LocalStorage;

    async fn make_cached(max_bytes: u64) -> (CachedStorage, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "mchact-cache-test-{}",
            uuid::Uuid::new_v4()
        ));
        let backend_dir = base.join("backend");
        let cache_dir = base.join("cache");

        let backend = LocalStorage::new(backend_dir)
            .await
            .expect("backend LocalStorage");
        let cached = CachedStorage::new(Box::new(backend), cache_dir, max_bytes)
            .await
            .expect("CachedStorage");

        (cached, base)
    }

    /// Helpers to build a plain `LocalStorage` that shares the same `backend_dir`
    /// as a `CachedStorage` created by `make_cached`.  We do this by pointing
    /// directly at the backend sub-directory.
    async fn backend_only(base: &PathBuf) -> LocalStorage {
        LocalStorage::new(base.join("backend"))
            .await
            .expect("backend-only LocalStorage")
    }

    // -----------------------------------------------------------------------

    /// put via CachedStorage → delete from backend directly → get still
    /// returns data (served from cache).
    #[tokio::test]
    #[cfg(feature = "local")]
    async fn test_cache_hit() {
        let (cached, base) = make_cached(1024 * 1024).await;
        let backend = backend_only(&base).await;

        let key = "hello.txt";
        let data = b"cached content".to_vec();

        // Populate via the cache layer (write-through).
        cached.put(key, data.clone()).await.unwrap();

        // Remove from backend only — the cache file should still be on disk.
        backend.delete(key).await.unwrap();
        assert!(!backend.exists(key).await.unwrap());

        // get() should be served from cache without hitting the backend.
        let result = cached.get(key).await.unwrap();
        assert_eq!(result, data);
    }

    /// Write directly to backend (bypassing cache) → get via CachedStorage
    /// triggers a backend fetch and writes the data into the cache.
    #[tokio::test]
    #[cfg(feature = "local")]
    async fn test_cache_miss_fetches_from_backend() {
        let (cached, base) = make_cached(1024 * 1024).await;
        let backend = backend_only(&base).await;

        let key = "fresh.bin";
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];

        // Write directly to backend — cache has no knowledge of this key.
        backend.put(key, data.clone()).await.unwrap();
        assert!(!cached.cache_path(key).exists(), "cache file must not exist yet");

        // get() should trigger a miss, fetch from backend, and cache the result.
        let result = cached.get(key).await.unwrap();
        assert_eq!(result, data);

        // Now the cache file should exist on disk.
        assert!(cached.cache_path(key).exists(), "cache file must exist after miss");
    }

    /// With a 100-byte limit, putting a first 60-byte object then a second
    /// 60-byte object should evict the first from the cache — but the first
    /// remains readable via the backend.
    #[tokio::test]
    #[cfg(feature = "local")]
    async fn test_lru_eviction() {
        let (cached, base) = make_cached(100).await;
        let backend = backend_only(&base).await;

        let key_a = "a.bin";
        let key_b = "b.bin";
        let data_a = vec![0u8; 60];
        let data_b = vec![1u8; 60];

        // Put first object — 60 bytes, within limit.
        cached.put(key_a, data_a.clone()).await.unwrap();
        assert!(cached.cache_path(key_a).exists(), "a.bin should be cached");

        // Small sleep to ensure key_b gets a later timestamp.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        // Put second object — now 120 bytes total, exceeds 100-byte limit.
        // key_a is older so it should be evicted.
        cached.put(key_b, data_b.clone()).await.unwrap();

        // key_a must have been evicted from the cache directory.
        assert!(
            !cached.cache_path(key_a).exists(),
            "a.bin should have been LRU-evicted from cache"
        );

        // key_b should still be cached.
        assert!(cached.cache_path(key_b).exists(), "b.bin should still be cached");

        // key_a must still be retrievable from the backend.
        let result = backend.exists(key_a).await.unwrap();
        assert!(result, "a.bin should still exist in backend after eviction");

        let fetched = backend.get(key_a).await.unwrap();
        assert_eq!(fetched, data_a);
    }
}

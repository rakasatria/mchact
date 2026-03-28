# Media Storage Backends Implementation Plan (Plan D)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace mchact's hardcoded local filesystem media storage with a pluggable backend system (local/S3/Azure/GCS), add `media_objects` database tracking, LRU cache, and migrate tools from file paths to media_object_id references.

**Architecture:** New `mchact-storage-backend` crate with `ObjectStorage` trait and 4 implementations (feature-gated for cloud SDKs). `CachedStorage` wraps cloud backends with LRU eviction. `MediaManager` service coordinates storage + DB. Tools output `media_object_id` instead of raw paths. Migration v22 creates `media_objects` table and backfills existing files.

**Tech Stack:** Rust (async-trait, thiserror, sha2, aws-sdk-s3, azure_storage_blobs, google-cloud-storage), SQLite migration

**Spec:** `docs/superpowers/specs/2026-03-27-media-storage-backends-design.md`

---

### Task 1: Storage Backend Crate Scaffold + LocalStorage

**Files:**
- Create: `crates/mchact-storage-backend/Cargo.toml`
- Create: `crates/mchact-storage-backend/src/lib.rs`
- Create: `crates/mchact-storage-backend/src/local.rs`
- Modify: `Cargo.toml` (workspace members + dependency)

- [ ] **Step 1: Create crate directory**

```bash
mkdir -p crates/mchact-storage-backend/src
```

- [ ] **Step 2: Create `crates/mchact-storage-backend/Cargo.toml`**

```toml
[package]
name = "mchact-storage-backend"
version = "0.1.0"
edition = "2021"
license = "MIT"

[features]
default = ["local"]
local = []
s3 = ["aws-sdk-s3", "aws-config"]
azure = ["azure_storage_blobs", "azure_storage"]
gcs = ["google-cloud-storage"]

[dependencies]
async-trait = "0.1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"

# Cloud providers (optional)
aws-sdk-s3 = { version = "1", optional = true }
aws-config = { version = "1", optional = true }
azure_storage_blobs = { version = "0.21", optional = true }
azure_storage = { version = "0.21", optional = true }
google-cloud-storage = { version = "0.22", optional = true }

[dev-dependencies]
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 3: Create `crates/mchact-storage-backend/src/lib.rs`**

```rust
#[cfg(feature = "local")]
pub mod local;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "azure")]
pub mod azure;

#[cfg(feature = "gcs")]
pub mod gcs;

pub mod cache;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Object not found: {0}")]
    NotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Backend error: {0}")]
    Backend(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError>;
    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;
    fn backend_name(&self) -> &str;
}

/// Create storage backend from config.
pub fn create_storage(
    backend: &str,
    data_dir: &str,
    #[cfg(feature = "s3")] s3_config: Option<&S3Config>,
    #[cfg(feature = "azure")] azure_config: Option<&AzureConfig>,
    #[cfg(feature = "gcs")] gcs_config: Option<&GcsConfig>,
) -> Result<Box<dyn ObjectStorage>, StorageError> {
    match backend {
        "local" => Ok(Box::new(local::LocalStorage::new(data_dir))),
        #[cfg(feature = "s3")]
        "s3" => {
            let cfg = s3_config.ok_or_else(|| StorageError::Backend("S3 config required".into()))?;
            Ok(Box::new(s3::S3Storage::new(cfg)?))
        }
        #[cfg(feature = "azure")]
        "azure" => {
            let cfg = azure_config.ok_or_else(|| StorageError::Backend("Azure config required".into()))?;
            Ok(Box::new(azure::AzureBlobStorage::new(cfg)?))
        }
        #[cfg(feature = "gcs")]
        "gcs" => {
            let cfg = gcs_config.ok_or_else(|| StorageError::Backend("GCS config required".into()))?;
            Ok(Box::new(gcs::GcsStorage::new(cfg)?))
        }
        other => Err(StorageError::Backend(format!("Unknown backend: {other}"))),
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AzureConfig {
    pub container: String,
    pub connection_string: Option<String>,
    pub account_name: Option<String>,
    pub account_key: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GcsConfig {
    pub bucket: String,
    pub credentials_path: Option<String>,
}
```

Note: The `create_storage` function will be simplified in Task 8 when we wire config. For now the factory just needs to compile.

- [ ] **Step 4: Create `crates/mchact-storage-backend/src/local.rs`**

```rust
use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::{ObjectStorage, StorageError};

pub struct LocalStorage {
    base_dir: PathBuf,
}

impl LocalStorage {
    pub fn new(base_dir: &str) -> Self {
        Self {
            base_dir: PathBuf::from(base_dir),
        }
    }

    fn resolve_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(key)
    }
}

#[async_trait]
impl ObjectStorage for LocalStorage {
    async fn put(&self, key: &str, data: &[u8], _mime_type: Option<&str>) -> Result<(), StorageError> {
        let path = self.resolve_path(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, data).await?;
        tracing::debug!(key, bytes = data.len(), "LocalStorage: put");
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.resolve_path(key);
        if !path.exists() {
            return Err(StorageError::NotFound(key.to_string()));
        }
        let data = tokio::fs::read(&path).await?;
        tracing::debug!(key, bytes = data.len(), "LocalStorage: get");
        Ok(data)
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.resolve_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            tracing::debug!(key, "LocalStorage: delete");
        }
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        Ok(self.resolve_path(key).exists())
    }

    fn backend_name(&self) -> &str {
        "local"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_get_delete() {
        let dir = std::env::temp_dir().join(format!("mchact_local_storage_{}", uuid::Uuid::new_v4()));
        let storage = LocalStorage::new(&dir.to_string_lossy());

        // Put
        storage.put("test/file.txt", b"hello world", Some("text/plain")).await.unwrap();
        assert!(dir.join("test/file.txt").exists());

        // Get
        let data = storage.get("test/file.txt").await.unwrap();
        assert_eq!(data, b"hello world");

        // Exists
        assert!(storage.exists("test/file.txt").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Delete
        storage.delete("test/file.txt").await.unwrap();
        assert!(!storage.exists("test/file.txt").await.unwrap());

        // Get after delete
        assert!(matches!(
            storage.get("test/file.txt").await,
            Err(StorageError::NotFound(_))
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_put_creates_parent_dirs() {
        let dir = std::env::temp_dir().join(format!("mchact_local_dirs_{}", uuid::Uuid::new_v4()));
        let storage = LocalStorage::new(&dir.to_string_lossy());

        storage.put("deep/nested/path/file.bin", b"data", None).await.unwrap();
        assert!(dir.join("deep/nested/path/file.bin").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backend_name() {
        let storage = LocalStorage::new("/tmp");
        assert_eq!(storage.backend_name(), "local");
    }
}
```

- [ ] **Step 5: Create stub `crates/mchact-storage-backend/src/cache.rs`**

```rust
//! LRU cache wrapper — implemented in Task 5.

use async_trait::async_trait;
use crate::{ObjectStorage, StorageError};

/// Placeholder — full implementation in Task 5.
pub struct CachedStorage {
    backend: Box<dyn ObjectStorage>,
}

impl CachedStorage {
    pub fn new(backend: Box<dyn ObjectStorage>, _cache_dir: &str, _max_bytes: u64) -> Self {
        Self { backend }
    }

    pub fn into_inner(self) -> Box<dyn ObjectStorage> {
        self.backend
    }
}

#[async_trait]
impl ObjectStorage for CachedStorage {
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError> {
        self.backend.put(key, data, mime_type).await
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        self.backend.get(key).await
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.backend.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        self.backend.exists(key).await
    }

    fn backend_name(&self) -> &str {
        self.backend.backend_name()
    }
}
```

- [ ] **Step 6: Add to workspace**

In root `Cargo.toml`, add to `[workspace] members`:
```toml
"crates/mchact-storage-backend",
```

Add to `[dependencies]`:
```toml
mchact-storage-backend = { version = "0.1.0", path = "crates/mchact-storage-backend" }
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p mchact-storage-backend`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/mchact-storage-backend/ Cargo.toml
git commit -m "feat: scaffold mchact-storage-backend crate with ObjectStorage trait and LocalStorage"
```

---

### Task 2: S3 Storage Implementation

**Files:**
- Create: `crates/mchact-storage-backend/src/s3.rs`

- [ ] **Step 1: Create S3 storage implementation**

```rust
use async_trait::async_trait;
use crate::{ObjectStorage, S3Config, StorageError};

pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3Storage {
    pub fn new(config: &S3Config) -> Result<Self, StorageError> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| StorageError::Backend(format!("No tokio runtime: {e}")))?;

        // Build SDK config
        let sdk_config = rt.block_on(async {
            let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

            if let Some(region) = &config.region {
                loader = loader.region(aws_config::Region::new(region.clone()));
            }

            if let (Some(key_id), Some(secret)) = (&config.access_key_id, &config.secret_access_key) {
                loader = loader.credentials_provider(
                    aws_sdk_s3::config::Credentials::new(key_id, secret, None, None, "mchact-config"),
                );
            }

            loader.load().await
        });

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

        if let Some(endpoint) = &config.endpoint {
            s3_config_builder = s3_config_builder
                .endpoint_url(endpoint)
                .force_path_style(true); // Required for MinIO, R2, etc.
        }

        let client = aws_sdk_s3::Client::from_conf(s3_config_builder.build());

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
        })
    }
}

#[async_trait]
impl ObjectStorage for S3Storage {
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError> {
        let mut req = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(data.to_vec()));

        if let Some(mt) = mime_type {
            req = req.content_type(mt);
        }

        req.send()
            .await
            .map_err(|e| StorageError::Backend(format!("S3 put error: {e}")))?;

        tracing::debug!(key, bucket = %self.bucket, "S3Storage: put");
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                let msg = format!("{e}");
                if msg.contains("NoSuchKey") || msg.contains("404") {
                    StorageError::NotFound(key.to_string())
                } else {
                    StorageError::Backend(format!("S3 get error: {e}"))
                }
            })?;

        let data = resp
            .body
            .collect()
            .await
            .map_err(|e| StorageError::Backend(format!("S3 read body: {e}")))?
            .into_bytes()
            .to_vec();

        tracing::debug!(key, bytes = data.len(), "S3Storage: get");
        Ok(data)
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::Backend(format!("S3 delete error: {e}")))?;

        tracing::debug!(key, "S3Storage: delete");
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("NotFound") || msg.contains("404") {
                    Ok(false)
                } else {
                    Err(StorageError::Backend(format!("S3 exists error: {e}")))
                }
            }
        }
    }

    fn backend_name(&self) -> &str {
        "s3"
    }
}
```

- [ ] **Step 2: Verify build with S3 feature**

Run: `cargo build -p mchact-storage-backend --features s3`
Expected: Clean build (no runtime test — needs actual S3).

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-storage-backend/src/s3.rs
git commit -m "feat: add S3 storage backend (AWS, MinIO, R2, B2, DO Spaces)"
```

---

### Task 3: Azure Blob Storage Implementation

**Files:**
- Create: `crates/mchact-storage-backend/src/azure.rs`

- [ ] **Step 1: Create Azure Blob storage implementation**

```rust
use async_trait::async_trait;
use crate::{AzureConfig, ObjectStorage, StorageError};

pub struct AzureBlobStorage {
    container_client: azure_storage_blobs::prelude::ContainerClient,
}

impl AzureBlobStorage {
    pub fn new(config: &AzureConfig) -> Result<Self, StorageError> {
        let storage_credentials = if let Some(conn_str) = &config.connection_string {
            azure_storage::prelude::StorageCredentials::connection_string(conn_str)
                .map_err(|e| StorageError::Backend(format!("Azure connection string error: {e}")))?
        } else if let (Some(account), Some(key)) = (&config.account_name, &config.account_key) {
            azure_storage::prelude::StorageCredentials::access_key(account, key.clone())
        } else {
            return Err(StorageError::Backend(
                "Azure requires either connection_string or account_name + account_key".into(),
            ));
        };

        let account_name = config.account_name.clone().unwrap_or_else(|| {
            // Extract from connection string if not explicit
            config.connection_string.as_deref().unwrap_or("")
                .split(';')
                .find_map(|part| part.strip_prefix("AccountName="))
                .unwrap_or("unknown")
                .to_string()
        });

        let blob_service = azure_storage_blobs::prelude::BlobServiceClient::new(
            &account_name,
            storage_credentials,
        );
        let container_client = blob_service.container_client(&config.container);

        Ok(Self { container_client })
    }
}

#[async_trait]
impl ObjectStorage for AzureBlobStorage {
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError> {
        let blob_client = self.container_client.blob_client(key);
        let mut req = blob_client.put_block_blob(data.to_vec());

        if let Some(mt) = mime_type {
            req = req.content_type(mt);
        }

        req.await
            .map_err(|e| StorageError::Backend(format!("Azure put error: {e}")))?;

        tracing::debug!(key, "AzureBlobStorage: put");
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let blob_client = self.container_client.blob_client(key);

        let resp = blob_client
            .get_content()
            .await
            .map_err(|e| {
                let msg = format!("{e}");
                if msg.contains("BlobNotFound") || msg.contains("404") {
                    StorageError::NotFound(key.to_string())
                } else {
                    StorageError::Backend(format!("Azure get error: {e}"))
                }
            })?;

        tracing::debug!(key, bytes = resp.len(), "AzureBlobStorage: get");
        Ok(resp)
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let blob_client = self.container_client.blob_client(key);
        blob_client
            .delete()
            .await
            .map_err(|e| StorageError::Backend(format!("Azure delete error: {e}")))?;

        tracing::debug!(key, "AzureBlobStorage: delete");
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let blob_client = self.container_client.blob_client(key);
        match blob_client.get_properties().await {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("BlobNotFound") || msg.contains("404") {
                    Ok(false)
                } else {
                    Err(StorageError::Backend(format!("Azure exists error: {e}")))
                }
            }
        }
    }

    fn backend_name(&self) -> &str {
        "azure"
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p mchact-storage-backend --features azure`
Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-storage-backend/src/azure.rs
git commit -m "feat: add Azure Blob storage backend"
```

---

### Task 4: GCS Storage Implementation

**Files:**
- Create: `crates/mchact-storage-backend/src/gcs.rs`

- [ ] **Step 1: Create GCS storage implementation**

```rust
use async_trait::async_trait;
use crate::{GcsConfig, ObjectStorage, StorageError};

pub struct GcsStorage {
    client: google_cloud_storage::client::Client,
    bucket: String,
}

impl GcsStorage {
    pub fn new(config: &GcsConfig) -> Result<Self, StorageError> {
        let client = if let Some(cred_path) = &config.credentials_path {
            std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", cred_path);
            google_cloud_storage::client::Client::default()
        } else {
            google_cloud_storage::client::Client::default()
        };

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
        })
    }
}

#[async_trait]
impl ObjectStorage for GcsStorage {
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError> {
        use google_cloud_storage::http::objects::upload::UploadObjectRequest;

        let upload_type = google_cloud_storage::http::objects::upload::UploadType::Simple(
            google_cloud_storage::http::objects::upload::Media::new(key.to_string()),
        );

        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                data.to_vec(),
                &upload_type,
            )
            .await
            .map_err(|e| StorageError::Backend(format!("GCS put error: {e}")))?;

        tracing::debug!(key, "GcsStorage: put");
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        use google_cloud_storage::http::objects::get::GetObjectRequest;
        use google_cloud_storage::http::objects::download::Range;

        let data = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: key.to_string(),
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| {
                let msg = format!("{e}");
                if msg.contains("404") || msg.contains("No such object") {
                    StorageError::NotFound(key.to_string())
                } else {
                    StorageError::Backend(format!("GCS get error: {e}"))
                }
            })?;

        tracing::debug!(key, bytes = data.len(), "GcsStorage: get");
        Ok(data)
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        use google_cloud_storage::http::objects::delete::DeleteObjectRequest;

        self.client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: key.to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| StorageError::Backend(format!("GCS delete error: {e}")))?;

        tracing::debug!(key, "GcsStorage: delete");
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        use google_cloud_storage::http::objects::get::GetObjectRequest;

        match self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: key.to_string(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("404") || msg.contains("No such object") {
                    Ok(false)
                } else {
                    Err(StorageError::Backend(format!("GCS exists error: {e}")))
                }
            }
        }
    }

    fn backend_name(&self) -> &str {
        "gcs"
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p mchact-storage-backend --features gcs`
Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-storage-backend/src/gcs.rs
git commit -m "feat: add Google Cloud Storage backend"
```

---

### Task 5: LRU Cache Layer

**Files:**
- Modify: `crates/mchact-storage-backend/src/cache.rs`

- [ ] **Step 1: Implement full CachedStorage with LRU eviction**

Replace the stub `cache.rs` with:

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::{ObjectStorage, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    size_bytes: u64,
    last_accessed_epoch: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct CacheMetadata {
    entries: HashMap<String, CacheEntry>,
    total_bytes: u64,
}

pub struct CachedStorage {
    backend: Box<dyn ObjectStorage>,
    cache_dir: PathBuf,
    max_bytes: u64,
    metadata: Mutex<CacheMetadata>,
}

impl CachedStorage {
    pub fn new(backend: Box<dyn ObjectStorage>, cache_dir: &str, max_bytes: u64) -> Self {
        let cache_dir = PathBuf::from(cache_dir);
        let metadata = Self::load_metadata(&cache_dir).unwrap_or_default();

        Self {
            backend,
            cache_dir,
            max_bytes,
            metadata: Mutex::new(metadata),
        }
    }

    fn meta_path(cache_dir: &Path) -> PathBuf {
        cache_dir.join(".cache_meta.json")
    }

    fn load_metadata(cache_dir: &Path) -> Option<CacheMetadata> {
        let path = Self::meta_path(cache_dir);
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_metadata(&self) {
        if let Ok(meta) = self.metadata.lock() {
            let path = Self::meta_path(&self.cache_dir);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(&*meta) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    fn cache_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(key)
    }

    fn now_epoch() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn evict_if_needed(&self) {
        let mut meta = match self.metadata.lock() {
            Ok(m) => m,
            Err(_) => return,
        };

        while meta.total_bytes > self.max_bytes && !meta.entries.is_empty() {
            // Find least recently accessed
            let lru_key = meta
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_accessed_epoch)
                .map(|(k, _)| k.clone());

            if let Some(key) = lru_key {
                if let Some(entry) = meta.entries.remove(&key) {
                    meta.total_bytes = meta.total_bytes.saturating_sub(entry.size_bytes);
                    let path = self.cache_dir.join(&key);
                    let _ = std::fs::remove_file(&path);
                    tracing::debug!(key, bytes = entry.size_bytes, "Cache: evicted");
                }
            } else {
                break;
            }
        }
    }

    fn record_access(&self, key: &str, size: u64) {
        if let Ok(mut meta) = self.metadata.lock() {
            if let Some(entry) = meta.entries.get_mut(key) {
                entry.last_accessed_epoch = Self::now_epoch();
            } else {
                meta.entries.insert(
                    key.to_string(),
                    CacheEntry {
                        size_bytes: size,
                        last_accessed_epoch: Self::now_epoch(),
                    },
                );
                meta.total_bytes += size;
            }
        }
    }
}

#[async_trait]
impl ObjectStorage for CachedStorage {
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError> {
        // Write-through: cache + backend
        let cache_path = self.cache_path(key);
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&cache_path, data).await?;
        self.record_access(key, data.len() as u64);
        self.evict_if_needed();
        self.save_metadata();

        self.backend.put(key, data, mime_type).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        // Check cache first
        let cache_path = self.cache_path(key);
        if cache_path.exists() {
            let data = tokio::fs::read(&cache_path).await?;
            self.record_access(key, data.len() as u64);
            self.save_metadata();
            return Ok(data);
        }

        // Cache miss — fetch from backend
        let data = self.backend.get(key).await?;

        // Write to cache
        if let Some(parent) = cache_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = tokio::fs::write(&cache_path, &data).await;
        self.record_access(key, data.len() as u64);
        self.evict_if_needed();
        self.save_metadata();

        Ok(data)
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        // Remove from cache
        let cache_path = self.cache_path(key);
        if cache_path.exists() {
            let _ = tokio::fs::remove_file(&cache_path).await;
        }
        if let Ok(mut meta) = self.metadata.lock() {
            if let Some(entry) = meta.entries.remove(key) {
                meta.total_bytes = meta.total_bytes.saturating_sub(entry.size_bytes);
            }
        }
        self.save_metadata();

        // Remove from backend
        self.backend.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        if self.cache_path(key).exists() {
            return Ok(true);
        }
        self.backend.exists(key).await
    }

    fn backend_name(&self) -> &str {
        self.backend.backend_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::LocalStorage;

    #[tokio::test]
    async fn test_cache_hit() {
        let backend_dir = std::env::temp_dir().join(format!("mchact_cache_backend_{}", uuid::Uuid::new_v4()));
        let cache_dir = std::env::temp_dir().join(format!("mchact_cache_dir_{}", uuid::Uuid::new_v4()));

        let backend = Box::new(LocalStorage::new(&backend_dir.to_string_lossy()));
        let cached = CachedStorage::new(backend, &cache_dir.to_string_lossy(), 1024 * 1024);

        // Put goes to both
        cached.put("test.txt", b"hello", None).await.unwrap();
        assert!(backend_dir.join("test.txt").exists());
        assert!(cache_dir.join("test.txt").exists());

        // Delete from backend but not cache — simulates cloud-only scenario
        std::fs::remove_file(backend_dir.join("test.txt")).unwrap();

        // Get should hit cache
        let data = cached.get("test.txt").await.unwrap();
        assert_eq!(data, b"hello");

        let _ = std::fs::remove_dir_all(&backend_dir);
        let _ = std::fs::remove_dir_all(&cache_dir);
    }

    #[tokio::test]
    async fn test_cache_miss_fetches_from_backend() {
        let backend_dir = std::env::temp_dir().join(format!("mchact_cache_miss_{}", uuid::Uuid::new_v4()));
        let cache_dir = std::env::temp_dir().join(format!("mchact_cache_miss_c_{}", uuid::Uuid::new_v4()));

        let backend = Box::new(LocalStorage::new(&backend_dir.to_string_lossy()));

        // Write directly to backend (bypassing cache)
        std::fs::create_dir_all(&backend_dir).unwrap();
        std::fs::write(backend_dir.join("remote.txt"), b"from backend").unwrap();

        let cached = CachedStorage::new(backend, &cache_dir.to_string_lossy(), 1024 * 1024);

        // Get should fetch from backend and cache it
        let data = cached.get("remote.txt").await.unwrap();
        assert_eq!(data, b"from backend");
        assert!(cache_dir.join("remote.txt").exists());

        let _ = std::fs::remove_dir_all(&backend_dir);
        let _ = std::fs::remove_dir_all(&cache_dir);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let backend_dir = std::env::temp_dir().join(format!("mchact_evict_b_{}", uuid::Uuid::new_v4()));
        let cache_dir = std::env::temp_dir().join(format!("mchact_evict_c_{}", uuid::Uuid::new_v4()));

        let backend = Box::new(LocalStorage::new(&backend_dir.to_string_lossy()));
        // Max 100 bytes in cache
        let cached = CachedStorage::new(backend, &cache_dir.to_string_lossy(), 100);

        // Put 60 bytes
        cached.put("a.bin", &[0u8; 60], None).await.unwrap();
        // Put 60 more bytes — should evict a.bin
        cached.put("b.bin", &[1u8; 60], None).await.unwrap();

        // a.bin should be evicted from cache (but still in backend)
        assert!(!cache_dir.join("a.bin").exists());
        assert!(backend_dir.join("a.bin").exists());
        // b.bin should be in cache
        assert!(cache_dir.join("b.bin").exists());

        let _ = std::fs::remove_dir_all(&backend_dir);
        let _ = std::fs::remove_dir_all(&cache_dir);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mchact-storage-backend`
Expected: All tests pass (local + cache).

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-storage-backend/src/cache.rs
git commit -m "feat: add LRU cache layer with size-based eviction"
```

---

### Task 6: Database Migration v22

**Files:**
- Modify: `crates/mchact-storage/src/db.rs`

- [ ] **Step 1: Add migration v22 and media_objects CRUD**

Find the migration section in `db.rs` (search for the latest `"v21"` migration). Add v22:

```rust
("v22", &[
    "CREATE TABLE IF NOT EXISTS media_objects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        object_key TEXT NOT NULL UNIQUE,
        storage_backend TEXT NOT NULL DEFAULT 'local',
        original_chat_id INTEGER NOT NULL,
        mime_type TEXT,
        size_bytes INTEGER,
        sha256_hash TEXT,
        source TEXT NOT NULL,
        created_at TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_media_objects_chat ON media_objects(original_chat_id)",
    "CREATE INDEX IF NOT EXISTS idx_media_objects_hash ON media_objects(sha256_hash)",
]),
```

Add column to document_extractions (in a separate migration step since ALTER TABLE can fail if column exists):

```rust
("v22_doc_media", &[
    "ALTER TABLE document_extractions ADD COLUMN media_object_id INTEGER REFERENCES media_objects(id)",
    "CREATE INDEX IF NOT EXISTS idx_doc_extractions_media ON document_extractions(media_object_id)",
]),
```

- [ ] **Step 2: Add MediaObject struct and CRUD methods**

Add to `db.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaObject {
    pub id: i64,
    pub object_key: String,
    pub storage_backend: String,
    pub original_chat_id: i64,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub sha256_hash: Option<String>,
    pub source: String,
    pub created_at: String,
}

impl Database {
    pub fn insert_media_object(
        &self,
        key: &str,
        backend: &str,
        chat_id: i64,
        mime_type: Option<&str>,
        size_bytes: Option<i64>,
        hash: Option<&str>,
        source: &str,
    ) -> Result<i64, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO media_objects (object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![key, backend, chat_id, mime_type, size_bytes, hash, source, now],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn get_media_object(&self, id: i64) -> Result<Option<MediaObject>, rusqlite::Error> {
        let mut stmt = self.conn().prepare(
            "SELECT id, object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at FROM media_objects WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map([id], |row| {
            Ok(MediaObject {
                id: row.get(0)?,
                object_key: row.get(1)?,
                storage_backend: row.get(2)?,
                original_chat_id: row.get(3)?,
                mime_type: row.get(4)?,
                size_bytes: row.get(5)?,
                sha256_hash: row.get(6)?,
                source: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.next().transpose()
    }

    pub fn get_media_object_by_hash(&self, hash: &str) -> Result<Option<MediaObject>, rusqlite::Error> {
        let mut stmt = self.conn().prepare(
            "SELECT id, object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at FROM media_objects WHERE sha256_hash = ?1 LIMIT 1"
        )?;
        let mut rows = stmt.query_map([hash], |row| {
            Ok(MediaObject {
                id: row.get(0)?,
                object_key: row.get(1)?,
                storage_backend: row.get(2)?,
                original_chat_id: row.get(3)?,
                mime_type: row.get(4)?,
                size_bytes: row.get(5)?,
                sha256_hash: row.get(6)?,
                source: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.next().transpose()
    }

    pub fn list_media_objects_for_chat(&self, chat_id: i64) -> Result<Vec<MediaObject>, rusqlite::Error> {
        let mut stmt = self.conn().prepare(
            "SELECT id, object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at FROM media_objects WHERE original_chat_id = ?1 ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([chat_id], |row| {
            Ok(MediaObject {
                id: row.get(0)?,
                object_key: row.get(1)?,
                storage_backend: row.get(2)?,
                original_chat_id: row.get(3)?,
                mime_type: row.get(4)?,
                size_bytes: row.get(5)?,
                sha256_hash: row.get(6)?,
                source: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_media_object(&self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn().execute("DELETE FROM media_objects WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_document_extraction_media_id(&self, extraction_id: i64, media_object_id: i64) -> Result<(), rusqlite::Error> {
        self.conn().execute(
            "UPDATE document_extractions SET media_object_id = ?1 WHERE id = ?2",
            [media_object_id, extraction_id],
        )?;
        Ok(())
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib db::tests`
Expected: Existing tests pass (migration auto-runs on test DB creation).

- [ ] **Step 4: Commit**

```bash
git add crates/mchact-storage/src/db.rs
git commit -m "feat: add migration v22 with media_objects table and document_extractions FK"
```

---

### Task 7: MediaManager Service

**Files:**
- Create: `src/media_manager.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create MediaManager**

```rust
use mchact_storage::db::{Database, MediaObject};
use mchact_storage_backend::{ObjectStorage, StorageError};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub struct MediaManager {
    storage: Arc<dyn ObjectStorage>,
    db: Arc<Database>,
}

impl MediaManager {
    pub fn new(storage: Arc<dyn ObjectStorage>, db: Arc<Database>) -> Self {
        Self { storage, db }
    }

    pub fn backend_name(&self) -> &str {
        self.storage.backend_name()
    }

    /// Store a file: compute hash, dedup check, write to storage, insert DB row.
    /// Returns media_object_id.
    pub async fn store_file(
        &self,
        data: &[u8],
        filename: &str,
        mime_type: Option<&str>,
        chat_id: i64,
        source: &str,
    ) -> Result<i64, String> {
        let hash = Self::compute_hash(data);

        // Dedup: check if file with same hash exists
        if let Ok(Some(existing)) = self.db.get_media_object_by_hash(&hash) {
            tracing::debug!(
                media_id = existing.id,
                hash = %hash,
                "MediaManager: dedup hit"
            );
            return Ok(existing.id);
        }

        // Generate object key
        let ext = Self::extension_from_filename(filename);
        let prefix = Self::source_prefix(source);
        let uuid = uuid::Uuid::new_v4().to_string();
        let key = format!("{}{}.{}", prefix, &uuid[..8], ext);

        // Store in backend
        self.storage
            .put(&key, data, mime_type)
            .await
            .map_err(|e| format!("Storage put failed: {e}"))?;

        // Insert DB record
        let id = self
            .db
            .insert_media_object(
                &key,
                self.storage.backend_name(),
                chat_id,
                mime_type,
                Some(data.len() as i64),
                Some(&hash),
                source,
            )
            .map_err(|e| format!("DB insert failed: {e}"))?;

        tracing::info!(media_id = id, key, source, bytes = data.len(), "MediaManager: stored");
        Ok(id)
    }

    /// Retrieve file bytes by media_object_id.
    pub async fn get_file(&self, media_object_id: i64) -> Result<(Vec<u8>, MediaObject), String> {
        let obj = self
            .db
            .get_media_object(media_object_id)
            .map_err(|e| format!("DB lookup failed: {e}"))?
            .ok_or_else(|| format!("Media object {media_object_id} not found"))?;

        let data = self
            .storage
            .get(&obj.object_key)
            .await
            .map_err(|e| format!("Storage get failed: {e}"))?;

        Ok((data, obj))
    }

    /// Delete a media object (storage + DB).
    pub async fn delete_file(&self, media_object_id: i64) -> Result<(), String> {
        let obj = self
            .db
            .get_media_object(media_object_id)
            .map_err(|e| format!("DB lookup: {e}"))?
            .ok_or_else(|| format!("Media object {media_object_id} not found"))?;

        self.storage
            .delete(&obj.object_key)
            .await
            .map_err(|e| format!("Storage delete: {e}"))?;

        self.db
            .delete_media_object(media_object_id)
            .map_err(|e| format!("DB delete: {e}"))?;

        Ok(())
    }

    /// List media for a chat.
    pub fn list_for_chat(&self, chat_id: i64) -> Result<Vec<MediaObject>, String> {
        self.db
            .list_media_objects_for_chat(chat_id)
            .map_err(|e| format!("DB query: {e}"))
    }

    fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    fn extension_from_filename(filename: &str) -> &str {
        filename
            .rsplit('.')
            .next()
            .filter(|ext| ext.len() <= 10)
            .unwrap_or("bin")
    }

    fn source_prefix(source: &str) -> &str {
        match source {
            "upload" => "uploads/",
            "image_gen" => "media/img_",
            "video_gen" => "media/vid_",
            "tts" => "media/tts_",
            "document" => "documents/",
            _ => "media/",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let hash = MediaManager::compute_hash(b"hello world");
        assert_eq!(hash.len(), 64); // SHA256 hex
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_extension_from_filename() {
        assert_eq!(MediaManager::extension_from_filename("photo.png"), "png");
        assert_eq!(MediaManager::extension_from_filename("doc.pdf"), "pdf");
        assert_eq!(MediaManager::extension_from_filename("noext"), "bin");
        assert_eq!(MediaManager::extension_from_filename("a.b.c"), "c");
    }

    #[test]
    fn test_source_prefix() {
        assert_eq!(MediaManager::source_prefix("upload"), "uploads/");
        assert_eq!(MediaManager::source_prefix("image_gen"), "media/img_");
        assert_eq!(MediaManager::source_prefix("video_gen"), "media/vid_");
        assert_eq!(MediaManager::source_prefix("tts"), "media/tts_");
        assert_eq!(MediaManager::source_prefix("document"), "documents/");
        assert_eq!(MediaManager::source_prefix("unknown"), "media/");
    }
}
```

- [ ] **Step 2: Add `sha2` dependency**

In root `Cargo.toml`, `sha2 = "0.10"` is already present.

- [ ] **Step 3: Register module in `src/lib.rs`**

Add after `pub mod train_pipeline;`:
```rust
pub mod media_manager;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib media_manager`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/media_manager.rs src/lib.rs
git commit -m "feat: add MediaManager service with dedup, store, get, delete, list"
```

---

### Task 8: Storage Config Fields

**Files:**
- Modify: `src/config.rs`
- Modify: `tests/config_validation.rs`

- [ ] **Step 1: Add 13 storage config fields**

Add to Config struct:
```rust
#[serde(default = "default_storage_backend")]
pub storage_backend: String,
#[serde(default = "default_storage_cache_max_mb")]
pub storage_cache_max_size_mb: u64,
pub storage_s3_bucket: Option<String>,
pub storage_s3_region: Option<String>,
pub storage_s3_endpoint: Option<String>,
pub storage_s3_access_key_id: Option<String>,
pub storage_s3_secret_access_key: Option<String>,
pub storage_azure_container: Option<String>,
pub storage_azure_connection_string: Option<String>,
pub storage_azure_account_name: Option<String>,
pub storage_azure_account_key: Option<String>,
pub storage_gcs_bucket: Option<String>,
pub storage_gcs_credentials_path: Option<String>,
```

Default functions:
```rust
fn default_storage_backend() -> String { "local".into() }
fn default_storage_cache_max_mb() -> u64 { 1024 }
```

Add to `Config::test_defaults()` and `tests/config_validation.rs::minimal_config()`.

- [ ] **Step 2: Run build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add src/config.rs tests/config_validation.rs
git commit -m "feat: add storage backend config fields (S3, Azure, GCS)"
```

---

### Task 9: Tool Migration (replace fs::write with MediaManager)

**Files:**
- Modify: `src/tools/image_generate.rs`
- Modify: `src/tools/video_generate.rs`
- Modify: `src/tools/text_to_speech.rs`
- Modify: `src/tools/read_document.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Add MediaManager to tools that generate media**

Each tool that currently does `std::fs::write(path, bytes)` needs to be modified to:
1. Accept `Arc<MediaManager>` in constructor
2. Call `media_manager.store_file(bytes, filename, mime, chat_id, source)` instead of `fs::write`
3. Return `{"media_object_id": N, "type": "image"}` instead of path string

For `image_generate.rs`: Replace `self.data_dir` usage with `self.media_manager`. Change the execute method's file writing block from:
```rust
let media_dir = PathBuf::from(&self.data_dir).join("media");
std::fs::create_dir_all(&media_dir)?;
let file_path = media_dir.join(&filename);
std::fs::write(&file_path, &image.data)?;
```
To:
```rust
let auth = auth_context_from_input(&input);
let media_id = self.media_manager.store_file(&image.data, &filename, Some("image/png"), auth.caller_chat_id, "image_gen").await
    .map_err(|e| /* return error ToolResult */)?;
```

Apply the same pattern to `video_generate.rs` (source: "video_gen") and `text_to_speech.rs` (source: "tts").

For `read_document.rs`: After extraction, call `db.set_document_extraction_media_id(extraction_id, media_id)` to link the extraction to its media object.

Update `ToolRegistry::new()` in `mod.rs` to pass `Arc<MediaManager>` to the constructors.

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add src/tools/image_generate.rs src/tools/video_generate.rs src/tools/text_to_speech.rs src/tools/read_document.rs src/tools/mod.rs
git commit -m "feat: migrate media tools from fs::write to MediaManager"
```

---

### Task 10: Web API Migration

**Files:**
- Modify: `src/web.rs`

- [ ] **Step 1: Update `/api/upload` endpoint**

Change from writing to disk to using MediaManager:
```rust
// Before: write to {data_dir}/uploads/{uuid}.{ext}
// After: media_manager.store_file(bytes, filename, mime, chat_id, "upload")
// Return: {"media_id": N, "url": "/api/media/N", "mime_type": "...", "size": N}
```

- [ ] **Step 2: Update `/api/media/:id` endpoint**

Support both integer IDs (new) and filenames (legacy fallback):
```rust
// If id parses as integer → lookup media_objects → get from storage
// If id is a filename string → legacy path-based lookup on local disk
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/web.rs
git commit -m "feat: migrate web upload/media endpoints to MediaManager"
```

---

### Task 11: Runtime Wiring + Backfill + Final Verification

**Files:**
- Modify: `src/runtime.rs`

- [ ] **Step 1: Wire MediaManager into AppState**

In `src/runtime.rs`, create the storage backend from config and initialize MediaManager:
```rust
let storage: Arc<dyn ObjectStorage> = match config.storage_backend.as_str() {
    "local" => Arc::new(LocalStorage::new(&config.data_dir)),
    // Cloud backends handled when features enabled
    _ => Arc::new(LocalStorage::new(&config.data_dir)),  // fallback
};

let media_manager = Arc::new(MediaManager::new(storage, db.clone()));
```

Add `media_manager` to `AppState`.

- [ ] **Step 2: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 3: Verify full build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/runtime.rs
git commit -m "feat: wire MediaManager into AppState with storage backend from config"
```

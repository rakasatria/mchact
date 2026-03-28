use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod cache;

#[cfg(feature = "local")]
pub mod local;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "azure")]
pub mod azure;

#[cfg(feature = "gcs")]
pub mod gcs;

/// Errors that can occur during object storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("object not found: {0}")]
    NotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// A result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Trait for object storage backends.
///
/// Implementors provide a key-value store for arbitrary byte payloads,
/// identified by string keys (analogous to paths or object names).
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    /// Store `data` at `key`, overwriting any existing value.
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()>;

    /// Retrieve the bytes stored at `key`.
    ///
    /// Returns [`StorageError::NotFound`] if the key does not exist.
    async fn get(&self, key: &str) -> StorageResult<Vec<u8>>;

    /// Delete the object at `key`.
    ///
    /// Succeeds silently if the key does not exist.
    async fn delete(&self, key: &str) -> StorageResult<()>;

    /// Return `true` if an object exists at `key`.
    async fn exists(&self, key: &str) -> StorageResult<bool>;

    /// List all keys under the given prefix.
    ///
    /// Returns keys relative to the storage root (including the prefix in the key).
    /// An empty prefix lists all keys. If the prefix does not match anything,
    /// an empty `Vec` is returned rather than an error.
    async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>>;

    /// Human-readable name of this backend (e.g. "local", "s3", "azure").
    fn backend_name(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// Cloud backend config structs
// ---------------------------------------------------------------------------

/// Configuration for an AWS S3 (or S3-compatible) backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name.
    pub bucket: String,
    /// Optional key prefix applied to every object key.
    #[serde(default)]
    pub prefix: String,
    /// AWS region (e.g. "us-east-1").
    pub region: String,
    /// Override the endpoint URL for S3-compatible services (e.g. MinIO).
    #[serde(default)]
    pub endpoint_url: Option<String>,
}

/// Configuration for an Azure Blob Storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    /// Storage account name.
    pub account: String,
    /// Container name.
    pub container: String,
    /// Optional key prefix applied to every object key.
    #[serde(default)]
    pub prefix: String,
}

/// Configuration for a Google Cloud Storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsConfig {
    /// GCS bucket name.
    pub bucket: String,
    /// Optional key prefix applied to every object key.
    #[serde(default)]
    pub prefix: String,
}

// ---------------------------------------------------------------------------
// Backend factory
// ---------------------------------------------------------------------------

/// Configuration for creating an object storage backend from application config.
#[derive(Debug, Clone)]
pub struct StorageBackendConfig {
    /// Backend type: "local", "s3", "azure", "gcs".
    pub backend: String,
    /// Base directory for the local backend (also used as cache dir root).
    pub data_dir: String,
    /// Maximum cache size in bytes (0 to disable caching for cloud backends).
    pub cache_max_bytes: u64,
    /// S3 configuration (required when backend = "s3").
    pub s3: Option<S3Config>,
    /// Azure configuration (required when backend = "azure").
    pub azure: Option<AzureConfig>,
    /// GCS configuration (required when backend = "gcs").
    pub gcs: Option<GcsConfig>,
}

/// Create an [`ObjectStorage`] implementation based on the provided configuration.
///
/// For cloud backends (s3, azure, gcs), a [`cache::CachedStorage`] wrapper is
/// applied when `config.cache_max_bytes > 0`.
///
/// # Errors
///
/// Returns a [`StorageError::Backend`] if:
/// - The requested backend feature is not compiled in.
/// - Required configuration fields are missing.
/// - Backend initialisation fails.
pub async fn create_storage(
    config: &StorageBackendConfig,
) -> StorageResult<Box<dyn ObjectStorage>> {
    match config.backend.as_str() {
        "local" => create_local(config).await,
        "s3" => create_s3(config).await,
        "azure" => create_azure(config).await,
        "gcs" => create_gcs(config).await,
        other => Err(StorageError::Backend(format!(
            "unknown storage backend: '{other}' (expected local, s3, azure, or gcs)"
        ))),
    }
}

#[cfg(feature = "local")]
async fn create_local(config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    let storage = local::LocalStorage::new(&config.data_dir).await?;
    Ok(Box::new(storage))
}

#[cfg(not(feature = "local"))]
async fn create_local(_config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    Err(StorageError::Backend(
        "local storage backend is not compiled in (enable the 'local' feature)".into(),
    ))
}

#[cfg(feature = "s3")]
async fn create_s3(config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    let s3_config = config
        .s3
        .as_ref()
        .ok_or_else(|| StorageError::Backend("s3 backend requires s3 configuration (storage_s3_bucket, storage_s3_region)".into()))?;
    let storage = s3::S3Storage::new(s3_config).await?;
    wrap_with_cache(Box::new(storage), config).await
}

#[cfg(not(feature = "s3"))]
async fn create_s3(_config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    Err(StorageError::Backend(
        "s3 storage backend is not compiled in (enable the 's3' feature)".into(),
    ))
}

#[cfg(feature = "azure")]
async fn create_azure(config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    let azure_config = config
        .azure
        .as_ref()
        .ok_or_else(|| StorageError::Backend("azure backend requires azure configuration (storage_azure_account_name, storage_azure_container)".into()))?;
    let storage = azure::AzureBlobStorage::new(azure_config)?;
    wrap_with_cache(Box::new(storage), config).await
}

#[cfg(not(feature = "azure"))]
async fn create_azure(_config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    Err(StorageError::Backend(
        "azure storage backend is not compiled in (enable the 'azure' feature)".into(),
    ))
}

#[cfg(feature = "gcs")]
async fn create_gcs(config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    let gcs_config = config
        .gcs
        .as_ref()
        .ok_or_else(|| StorageError::Backend("gcs backend requires gcs configuration (storage_gcs_bucket)".into()))?;
    let storage = gcs::GcsStorage::new(gcs_config).await?;
    wrap_with_cache(Box::new(storage), config).await
}

#[cfg(not(feature = "gcs"))]
async fn create_gcs(_config: &StorageBackendConfig) -> StorageResult<Box<dyn ObjectStorage>> {
    Err(StorageError::Backend(
        "gcs storage backend is not compiled in (enable the 'gcs' feature)".into(),
    ))
}

/// Wrap a cloud backend with a local LRU disk cache when `cache_max_bytes > 0`.
#[cfg(any(feature = "s3", feature = "azure", feature = "gcs"))]
async fn wrap_with_cache(
    backend: Box<dyn ObjectStorage>,
    config: &StorageBackendConfig,
) -> StorageResult<Box<dyn ObjectStorage>> {
    if config.cache_max_bytes == 0 {
        return Ok(backend);
    }
    let cache_dir = std::path::PathBuf::from(&config.data_dir).join("storage_cache");
    let cached = cache::CachedStorage::new(backend, cache_dir, config.cache_max_bytes).await?;
    Ok(Box::new(cached))
}

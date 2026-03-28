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

use async_trait::async_trait;
use azure_core::StatusCode;
use azure_core::auth::Secret;
use azure_storage::ConnectionString;
use azure_storage::StorageCredentials;
use azure_storage_blobs::prelude::ContainerClient;
use futures::StreamExt;
use tracing::instrument;

use crate::{AzureConfig, ObjectStorage, StorageError, StorageResult};

/// Azure Blob Storage-backed object storage implementation.
///
/// Credentials are resolved from environment variables in the following order:
/// 1. `AZURE_STORAGE_CONNECTION_STRING` — full connection string
/// 2. `AZURE_STORAGE_KEY` — account key (combined with `config.account`)
/// 3. Anonymous access (no credentials)
pub struct AzureBlobStorage {
    container_client: ContainerClient,
    prefix: String,
}

impl AzureBlobStorage {
    /// Create a new `AzureBlobStorage` from the provided configuration.
    ///
    /// Credentials are read from environment variables at construction time.
    pub fn new(config: &AzureConfig) -> StorageResult<Self> {
        let credentials = Self::resolve_credentials(&config.account)?;
        let service_client =
            azure_storage_blobs::prelude::BlobServiceClient::new(&config.account, credentials);
        let container_client = service_client.container_client(&config.container);

        Ok(Self {
            container_client,
            prefix: config.prefix.clone(),
        })
    }

    /// Resolve storage credentials from the environment.
    fn resolve_credentials(account: &str) -> StorageResult<StorageCredentials> {
        // 1. Try connection string first
        if let Ok(conn_str) = std::env::var("AZURE_STORAGE_CONNECTION_STRING") {
            let parsed = ConnectionString::new(&conn_str)
                .map_err(|e| StorageError::Backend(format!("invalid connection string: {e}")))?;
            let creds = parsed
                .storage_credentials()
                .map_err(|e| StorageError::Backend(format!("credential error: {e}")))?;
            return Ok(creds);
        }

        // 2. Try account key
        if let Ok(key) = std::env::var("AZURE_STORAGE_KEY") {
            return Ok(StorageCredentials::access_key(
                account.to_owned(),
                Secret::new(key),
            ));
        }

        // 3. Fall back to anonymous access
        Ok(StorageCredentials::anonymous())
    }

    /// Build the full blob key by prepending the prefix when non-empty.
    fn full_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), key)
        }
    }

    /// Return `true` when the azure_core error indicates HTTP 404.
    fn is_not_found(err: &azure_core::Error) -> bool {
        err.as_http_error()
            .map(|e: &azure_core::error::HttpError| e.status() == StatusCode::NotFound)
            .unwrap_or(false)
    }
}

#[async_trait]
impl ObjectStorage for AzureBlobStorage {
    #[instrument(skip(self, data), fields(key = %key, bytes = data.len()))]
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()> {
        let blob_key = self.full_key(key);
        self.container_client
            .blob_client(&blob_key)
            .put_block_blob(data)
            .await
            .map_err(|e| StorageError::Backend(format!("azure put error for '{blob_key}': {e}")))?;
        Ok(())
    }

    #[instrument(skip(self), fields(key = %key))]
    async fn get(&self, key: &str) -> StorageResult<Vec<u8>> {
        let blob_key = self.full_key(key);
        self.container_client
            .blob_client(&blob_key)
            .get_content()
            .await
            .map_err(|e| {
                if Self::is_not_found(&e) {
                    StorageError::NotFound(key.to_owned())
                } else {
                    StorageError::Backend(format!("azure get error for '{blob_key}': {e}"))
                }
            })
    }

    #[instrument(skip(self), fields(key = %key))]
    async fn delete(&self, key: &str) -> StorageResult<()> {
        let blob_key = self.full_key(key);
        match self
            .container_client
            .blob_client(&blob_key)
            .delete()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) if Self::is_not_found(&e) => Ok(()),
            Err(e) => Err(StorageError::Backend(format!(
                "azure delete error for '{blob_key}': {e}"
            ))),
        }
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let blob_key = self.full_key(key);
        self.container_client
            .blob_client(&blob_key)
            .exists()
            .await
            .map_err(|e| {
                StorageError::Backend(format!("azure exists error for '{blob_key}': {e}"))
            })
    }

    async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>> {
        let full_prefix = self.full_key(prefix);
        let mut result = Vec::new();
        let mut stream = self
            .container_client
            .list_blobs()
            .prefix(full_prefix.clone())
            .into_stream();
        while let Some(page) = stream.next().await {
            let page = page.map_err(|e| {
                StorageError::Backend(format!("azure list_blobs error: {e}"))
            })?;
            for blob in page.blobs.blobs() {
                let key = blob.name.as_str();
                let relative = if self.prefix.is_empty() {
                    key.to_string()
                } else {
                    let prefix_with_slash = format!("{}/", self.prefix.trim_end_matches('/'));
                    key.strip_prefix(&prefix_with_slash)
                        .unwrap_or(key)
                        .to_string()
                };
                result.push(relative);
            }
        }
        result.sort();
        Ok(result)
    }

    fn backend_name(&self) -> &'static str {
        "azure"
    }
}

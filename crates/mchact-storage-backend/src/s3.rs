use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::config::Region;
use tracing::{debug, warn};

use crate::{ObjectStorage, S3Config, StorageError, StorageResult};

/// AWS S3 (and S3-compatible) object storage backend.
///
/// Supports AWS S3, MinIO, Cloudflare R2, Backblaze B2, DigitalOcean Spaces,
/// and any other service that implements the S3 API.
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
    prefix: String,
}

impl S3Storage {
    /// Create a new S3Storage from the given config.
    ///
    /// Credentials are resolved from the standard AWS credential chain:
    /// environment variables, shared credentials file, IAM role, etc.
    pub async fn new(config: &S3Config) -> StorageResult<Self> {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .load()
            .await;

        let mut s3_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

        if let Some(ref url) = config.endpoint_url {
            debug!("S3 using custom endpoint: {}", url);
            s3_builder = s3_builder
                .endpoint_url(url)
                .force_path_style(true);
        }

        let client = aws_sdk_s3::Client::from_conf(s3_builder.build());

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
        })
    }

    /// Resolve the full S3 object key, prepending the prefix if configured.
    fn full_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), key)
        }
    }
}

/// Determine whether an S3 SDK error represents a "not found" condition.
fn is_not_found<E: std::fmt::Debug>(err: &aws_sdk_s3::error::SdkError<E>) -> bool {
    use aws_sdk_s3::error::SdkError;
    match err {
        SdkError::ServiceError(service_err) => {
            let msg = format!("{:?}", service_err.err());
            msg.contains("NoSuchKey")
                || msg.contains("NotFound")
                || msg.contains("NoSuchBucket")
        }
        SdkError::ResponseError(resp_err) => {
            let raw = resp_err.raw();
            raw.status().as_u16() == 404
        }
        _ => false,
    }
}

#[async_trait]
impl ObjectStorage for S3Storage {
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()> {
        let full_key = self.full_key(key);
        debug!("S3 put: bucket={} key={}", self.bucket, full_key);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|e| {
                warn!("S3 put failed for key={}: {}", full_key, e);
                StorageError::Backend(format!("put failed: {e}"))
            })?;

        Ok(())
    }

    async fn get(&self, key: &str) -> StorageResult<Vec<u8>> {
        let full_key = self.full_key(key);
        debug!("S3 get: bucket={} key={}", self.bucket, full_key);

        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| {
                if is_not_found(&e) {
                    StorageError::NotFound(key.to_string())
                } else {
                    warn!("S3 get failed for key={}: {}", full_key, e);
                    StorageError::Backend(format!("get failed: {e}"))
                }
            })?;

        let bytes = resp
            .body
            .collect()
            .await
            .map_err(|e| {
                warn!("S3 body collect failed for key={}: {}", full_key, e);
                StorageError::Backend(format!("body collect failed: {e}"))
            })?
            .into_bytes()
            .to_vec();

        Ok(bytes)
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let full_key = self.full_key(key);
        debug!("S3 delete: bucket={} key={}", self.bucket, full_key);

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| {
                warn!("S3 delete failed for key={}: {}", full_key, e);
                StorageError::Backend(format!("delete failed: {e}"))
            })?;

        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let full_key = self.full_key(key);
        debug!("S3 exists: bucket={} key={}", self.bucket, full_key);

        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => {
                warn!("S3 exists check failed for key={}: {}", full_key, e);
                Err(StorageError::Backend(format!("exists check failed: {e}")))
            }
        }
    }

    fn backend_name(&self) -> &'static str {
        "s3"
    }
}

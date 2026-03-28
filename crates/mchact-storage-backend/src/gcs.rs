use async_trait::async_trait;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};

use crate::{GcsConfig, ObjectStorage, StorageError, StorageResult};

pub struct GcsStorage {
    client: Client,
    bucket: String,
    prefix: String,
}

impl GcsStorage {
    pub async fn new(config: &GcsConfig) -> StorageResult<Self> {
        let client_config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| StorageError::Backend(format!("GCS auth error: {e}")))?;
        let client = Client::new(client_config);
        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
        })
    }

    fn full_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), key)
        }
    }
}

#[async_trait]
impl ObjectStorage for GcsStorage {
    async fn put(&self, key: &str, data: Vec<u8>) -> StorageResult<()> {
        let object_name = self.full_key(key);
        let req = UploadObjectRequest {
            bucket: self.bucket.clone(),
            ..Default::default()
        };
        let upload_type = UploadType::Simple(Media::new(object_name));
        self.client
            .upload_object(&req, data, &upload_type)
            .await
            .map(|_| ())
            .map_err(|e| StorageError::Backend(format!("GCS put error: {e}")))
    }

    async fn get(&self, key: &str) -> StorageResult<Vec<u8>> {
        let object_name = self.full_key(key);
        let req = GetObjectRequest {
            bucket: self.bucket.clone(),
            object: object_name.clone(),
            ..Default::default()
        };
        self.client
            .download_object(&req, &Range::default())
            .await
            .map_err(|e| map_gcs_error(e, &object_name))
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let object_name = self.full_key(key);
        let req = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            object: object_name.clone(),
            ..Default::default()
        };
        match self.client.delete_object(&req).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Silently succeed if the object does not exist.
                if is_not_found(&e) {
                    Ok(())
                } else {
                    Err(StorageError::Backend(format!("GCS delete error: {e}")))
                }
            }
        }
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let object_name = self.full_key(key);
        let req = GetObjectRequest {
            bucket: self.bucket.clone(),
            object: object_name,
            ..Default::default()
        };
        match self.client.get_object(&req).await {
            Ok(_) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(StorageError::Backend(format!("GCS exists error: {e}"))),
        }
    }

    fn backend_name(&self) -> &'static str {
        "gcs"
    }
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn is_not_found(e: &google_cloud_storage::http::Error) -> bool {
    use google_cloud_storage::http::Error;
    match e {
        Error::Response(resp) => resp.code == 404,
        Error::HttpClient(re) => re.status().map(|s| s.as_u16() == 404).unwrap_or(false),
        _ => false,
    }
}

fn map_gcs_error(e: google_cloud_storage::http::Error, key: &str) -> StorageError {
    if is_not_found(&e) {
        StorageError::NotFound(key.to_string())
    } else {
        StorageError::Backend(format!("GCS error: {e}"))
    }
}

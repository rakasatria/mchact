use mchact_core::error::MchactError;

use crate::db::types::MediaObject;
use crate::traits::MediaObjectStore;

use super::PgDriver;

fn pg_err(e: impl std::fmt::Display) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn row_to_media(row: &tokio_postgres::Row) -> MediaObject {
    MediaObject {
        id: row.get("id"),
        object_key: row.get("object_key"),
        storage_backend: row.get("storage_backend"),
        original_chat_id: row.get("original_chat_id"),
        mime_type: row.get("mime_type"),
        size_bytes: row.get("size_bytes"),
        sha256_hash: row.get("sha256_hash"),
        source: row.get("source"),
        created_at: row.get("created_at"),
    }
}

impl MediaObjectStore for PgDriver {
    fn insert_media_object(
        &self,
        key: &str,
        backend: &str,
        chat_id: i64,
        mime_type: Option<&str>,
        size_bytes: Option<i64>,
        hash: Option<&str>,
        source: &str,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let key = key.to_string();
        let backend = backend.to_string();
        let mime_type = mime_type.map(|s| s.to_string());
        let hash = hash.map(|s| s.to_string());
        let source = source.to_string();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_one(
                    "INSERT INTO media_objects
                     (object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                     RETURNING id",
                    &[&key, &backend, &chat_id, &mime_type, &size_bytes, &hash, &source, &now],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_media_object(&self, id: i64) -> Result<Option<MediaObject>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT id, object_key, storage_backend, original_chat_id, mime_type,
                            size_bytes, sha256_hash, source, created_at
                     FROM media_objects WHERE id = $1",
                    &[&id],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.as_ref().map(row_to_media))
        })
    }

    fn get_media_object_by_hash(&self, hash: &str) -> Result<Option<MediaObject>, MchactError> {
        let pool = self.pool.clone();
        let hash = hash.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT id, object_key, storage_backend, original_chat_id, mime_type,
                            size_bytes, sha256_hash, source, created_at
                     FROM media_objects WHERE sha256_hash = $1",
                    &[&hash],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.as_ref().map(row_to_media))
        })
    }

    fn list_media_objects_for_chat(
        &self,
        chat_id: i64,
    ) -> Result<Vec<MediaObject>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT id, object_key, storage_backend, original_chat_id, mime_type,
                            size_bytes, sha256_hash, source, created_at
                     FROM media_objects
                     WHERE original_chat_id = $1
                     ORDER BY created_at DESC",
                    &[&chat_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_media).collect())
        })
    }

    fn delete_media_object(&self, id: i64) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            client
                .execute("DELETE FROM media_objects WHERE id = $1", &[&id])
                .await
                .map_err(pg_err)?;
            Ok(())
        })
    }
}

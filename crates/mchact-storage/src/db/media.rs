use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::MediaObject;
use crate::traits::MediaObjectStore;

impl MediaObjectStore for Database {
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
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO media_objects
             (object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![key, backend, chat_id, mime_type, size_bytes, hash, source, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_media_object(&self, id: i64) -> Result<Option<MediaObject>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at
             FROM media_objects
             WHERE id = ?1",
        )?;
        let result = stmt
            .query_row(params![id], |row| {
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
            })
            .optional()?;
        Ok(result)
    }

    fn get_media_object_by_hash(&self, hash: &str) -> Result<Option<MediaObject>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at
             FROM media_objects
             WHERE sha256_hash = ?1",
        )?;
        let result = stmt
            .query_row(params![hash], |row| {
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
            })
            .optional()?;
        Ok(result)
    }

    fn list_media_objects_for_chat(
        &self,
        chat_id: i64,
    ) -> Result<Vec<MediaObject>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, object_key, storage_backend, original_chat_id, mime_type, size_bytes, sha256_hash, source, created_at
             FROM media_objects
             WHERE original_chat_id = ?1
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
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
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    fn delete_media_object(&self, id: i64) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM media_objects WHERE id = ?1", params![id])?;
        Ok(())
    }
}

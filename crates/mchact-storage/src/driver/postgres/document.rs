use mchact_core::error::MchactError;

use crate::db::types::DocumentExtraction;
use crate::traits::DocumentStore;

use super::PgDriver;

fn pg_err(e: impl std::fmt::Display) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn row_to_doc(row: &tokio_postgres::Row) -> DocumentExtraction {
    DocumentExtraction {
        id: row.get("id"),
        chat_id: row.get("chat_id"),
        file_hash: row.get("file_hash"),
        filename: row.get("filename"),
        mime_type: row.get("mime_type"),
        file_size: row.get("file_size"),
        extracted_text: row.get("extracted_text"),
        char_count: row.get("char_count"),
        created_at: row.get("created_at"),
    }
}

impl DocumentStore for PgDriver {
    fn insert_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
        filename: &str,
        mime_type: Option<&str>,
        file_size: i64,
        extracted_text: &str,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let file_hash = file_hash.to_string();
        let filename = filename.to_string();
        let mime_type = mime_type.map(|s| s.to_string());
        let extracted_text = extracted_text.to_string();
        let char_count = extracted_text.len() as i64;
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_one(
                    "INSERT INTO document_extractions
                     (chat_id, file_hash, filename, mime_type, file_size, extracted_text, char_count, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                     ON CONFLICT (chat_id, file_hash) DO UPDATE SET
                         filename = EXCLUDED.filename,
                         mime_type = EXCLUDED.mime_type,
                         file_size = EXCLUDED.file_size,
                         extracted_text = EXCLUDED.extracted_text,
                         char_count = EXCLUDED.char_count,
                         created_at = EXCLUDED.created_at
                     RETURNING id",
                    &[
                        &chat_id,
                        &file_hash,
                        &filename,
                        &mime_type,
                        &file_size,
                        &extracted_text,
                        &char_count,
                        &now,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        let pool = self.pool.clone();
        let file_hash = file_hash.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                            extracted_text, char_count, created_at
                     FROM document_extractions
                     WHERE chat_id = $1 AND file_hash = $2",
                    &[&chat_id, &file_hash],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.as_ref().map(row_to_doc))
        })
    }

    fn search_document_extractions(
        &self,
        chat_id: Option<i64>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        let pool = self.pool.clone();
        let pattern = format!("%{}%", query.replace('%', "\\%"));
        let limit = limit as i64;
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                            extracted_text, char_count, created_at
                     FROM document_extractions
                     WHERE ($1::bigint IS NULL OR chat_id = $1)
                       AND extracted_text ILIKE $2
                     ORDER BY created_at DESC
                     LIMIT $3",
                    &[&chat_id, &pattern, &limit],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_doc).collect())
        })
    }

    fn list_document_extractions(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        let pool = self.pool.clone();
        let limit = limit as i64;
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                            extracted_text, char_count, created_at
                     FROM document_extractions
                     WHERE chat_id = $1
                     ORDER BY created_at DESC
                     LIMIT $2",
                    &[&chat_id, &limit],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_doc).collect())
        })
    }

    fn get_document_extraction_by_id(
        &self,
        id: i64,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                            extracted_text, char_count, created_at
                     FROM document_extractions WHERE id = $1",
                    &[&id],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.as_ref().map(row_to_doc))
        })
    }

    fn set_document_extraction_media_id(
        &self,
        extraction_id: i64,
        media_object_id: i64,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            client
                .execute(
                    "UPDATE document_extractions SET media_object_id = $1 WHERE id = $2",
                    &[&media_object_id, &extraction_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(())
        })
    }

    fn get_document_extraction_id_by_media_object_id(
        &self,
        media_object_id: i64,
    ) -> Result<Option<i64>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT id FROM document_extractions WHERE media_object_id = $1 LIMIT 1",
                    &[&media_object_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.map(|r| r.get::<_, i64>(0)))
        })
    }
}

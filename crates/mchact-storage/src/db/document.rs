use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::DocumentExtraction;
use crate::traits::DocumentStore;

impl DocumentStore for Database {
    fn insert_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
        filename: &str,
        mime_type: Option<&str>,
        file_size: i64,
        extracted_text: &str,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let char_count = extracted_text.len() as i64;
        conn.execute(
            "INSERT OR REPLACE INTO document_extractions
             (chat_id, file_hash, filename, mime_type, file_size, extracted_text, char_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![chat_id, file_hash, filename, mime_type, file_size, extracted_text, char_count, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions
             WHERE chat_id = ?1 AND file_hash = ?2",
        )?;
        let result = stmt
            .query_row(params![chat_id, file_hash], |row| {
                Ok(DocumentExtraction {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    file_hash: row.get(2)?,
                    filename: row.get(3)?,
                    mime_type: row.get(4)?,
                    file_size: row.get(5)?,
                    extracted_text: row.get(6)?,
                    char_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn search_document_extractions(
        &self,
        chat_id: Option<i64>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        let conn = self.lock_conn();
        let pattern = format!("%{}%", query.replace('%', "\\%"));
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions
             WHERE (?1 IS NULL OR chat_id = ?1)
               AND LOWER(extracted_text) LIKE LOWER(?2)
             ORDER BY created_at DESC
             LIMIT ?3",
        )?;
        let chat_id_param: Option<i64> = chat_id;
        let limit_param = limit as i64;
        let rows = stmt.query_map(params![chat_id_param, pattern, limit_param], |row| {
            Ok(DocumentExtraction {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                file_hash: row.get(2)?,
                filename: row.get(3)?,
                mime_type: row.get(4)?,
                file_size: row.get(5)?,
                extracted_text: row.get(6)?,
                char_count: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    fn list_document_extractions(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions
             WHERE chat_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![chat_id, limit as i64], |row| {
            Ok(DocumentExtraction {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                file_hash: row.get(2)?,
                filename: row.get(3)?,
                mime_type: row.get(4)?,
                file_size: row.get(5)?,
                extracted_text: row.get(6)?,
                char_count: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    fn get_document_extraction_by_id(
        &self,
        id: i64,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions WHERE id = ?1",
        )?;
        let result = stmt
            .query_row(params![id], |row| {
                Ok(DocumentExtraction {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    file_hash: row.get(2)?,
                    filename: row.get(3)?,
                    mime_type: row.get(4)?,
                    file_size: row.get(5)?,
                    extracted_text: row.get(6)?,
                    char_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn set_document_extraction_media_id(
        &self,
        extraction_id: i64,
        media_object_id: i64,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "UPDATE document_extractions SET media_object_id = ?1 WHERE id = ?2",
            params![media_object_id, extraction_id],
        )?;
        Ok(())
    }

    fn get_document_extraction_id_by_media_object_id(
        &self,
        media_object_id: i64,
    ) -> Result<Option<i64>, MchactError> {
        let conn = self.lock_conn();
        let result = conn
            .query_row(
                "SELECT id FROM document_extractions WHERE media_object_id = ?1 LIMIT 1",
                params![media_object_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result)
    }
}

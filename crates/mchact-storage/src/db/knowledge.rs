use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::{DocumentChunk, Knowledge};
use crate::traits::KnowledgeStore;

impl KnowledgeStore for Database {
    // ── Knowledge CRUD ────────────────────────────────────────────────────────

    fn create_knowledge(
        &self,
        name: &str,
        description: &str,
        owner_chat_id: i64,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO knowledge (name, description, owner_chat_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![name, description, owner_chat_id, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_knowledge_by_name(&self, name: &str) -> Result<Option<Knowledge>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, owner_chat_id, last_grouping_check_at,
                    document_count_at_last_check, created_at, updated_at
             FROM knowledge WHERE name = ?1",
        )?;
        let result = stmt
            .query_row(params![name], |row| {
                Ok(Knowledge {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    owner_chat_id: row.get(3)?,
                    last_grouping_check_at: row.get(4)?,
                    document_count_at_last_check: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn list_knowledge(&self) -> Result<Vec<Knowledge>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, owner_chat_id, last_grouping_check_at,
                    document_count_at_last_check, created_at, updated_at
             FROM knowledge ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                owner_chat_id: row.get(3)?,
                last_grouping_check_at: row.get(4)?,
                document_count_at_last_check: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn delete_knowledge(&self, id: i64) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM knowledge WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn update_knowledge_timestamp(&self, id: i64) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE knowledge SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    fn update_knowledge_grouping_check(
        &self,
        knowledge_id: i64,
        doc_count: i64,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE knowledge
             SET last_grouping_check_at = ?1, document_count_at_last_check = ?2, updated_at = ?1
             WHERE id = ?3",
            params![now, doc_count, knowledge_id],
        )?;
        Ok(())
    }

    fn get_knowledge_needing_grouping(
        &self,
        min_docs: i64,
    ) -> Result<Vec<Knowledge>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT k.id, k.name, k.description, k.owner_chat_id,
                    k.last_grouping_check_at, k.document_count_at_last_check,
                    k.created_at, k.updated_at
             FROM knowledge k
             WHERE (
                 SELECT COUNT(*) FROM knowledge_documents kd WHERE kd.knowledge_id = k.id
             ) >= ?1
             AND (
                 SELECT COUNT(*) FROM knowledge_documents kd WHERE kd.knowledge_id = k.id
             ) > k.document_count_at_last_check
             ORDER BY k.updated_at",
        )?;
        let rows = stmt.query_map(params![min_docs], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                owner_chat_id: row.get(3)?,
                last_grouping_check_at: row.get(4)?,
                document_count_at_last_check: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ── Knowledge Documents ───────────────────────────────────────────────────

    fn add_document_to_knowledge(
        &self,
        knowledge_id: i64,
        doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO knowledge_documents (knowledge_id, document_extraction_id, added_at)
             VALUES (?1, ?2, ?3)",
            params![knowledge_id, doc_extraction_id, now],
        )?;
        Ok(())
    }

    fn remove_document_from_knowledge(
        &self,
        knowledge_id: i64,
        doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "DELETE FROM knowledge_documents
             WHERE knowledge_id = ?1 AND document_extraction_id = ?2",
            params![knowledge_id, doc_extraction_id],
        )?;
        Ok(())
    }

    fn list_knowledge_documents(&self, knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT document_extraction_id FROM knowledge_documents
             WHERE knowledge_id = ?1 ORDER BY added_at",
        )?;
        let rows = stmt.query_map(params![knowledge_id], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn count_knowledge_documents(&self, knowledge_id: i64) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_documents WHERE knowledge_id = ?1",
            params![knowledge_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ── Knowledge Chat Access ─────────────────────────────────────────────────

    fn add_knowledge_chat_access(
        &self,
        knowledge_id: i64,
        chat_id: i64,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO knowledge_chat_access (knowledge_id, chat_id, attached_at)
             VALUES (?1, ?2, ?3)",
            params![knowledge_id, chat_id, now],
        )?;
        Ok(())
    }

    fn has_knowledge_chat_access(
        &self,
        knowledge_id: i64,
        chat_id: i64,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM knowledge_chat_access
             WHERE knowledge_id = ?1 AND chat_id = ?2",
            params![knowledge_id, chat_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn list_knowledge_for_chat(&self, chat_id: i64) -> Result<Vec<Knowledge>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT k.id, k.name, k.description, k.owner_chat_id,
                    k.last_grouping_check_at, k.document_count_at_last_check,
                    k.created_at, k.updated_at
             FROM knowledge k
             JOIN knowledge_chat_access kca ON kca.knowledge_id = k.id
             WHERE kca.chat_id = ?1
             ORDER BY k.created_at DESC",
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                owner_chat_id: row.get(3)?,
                last_grouping_check_at: row.get(4)?,
                document_count_at_last_check: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_knowledge_chat_ids(&self, knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT chat_id FROM knowledge_chat_access WHERE knowledge_id = ?1 ORDER BY attached_at",
        )?;
        let rows = stmt.query_map(params![knowledge_id], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ── Document Chunks ───────────────────────────────────────────────────────

    fn insert_document_chunk(
        &self,
        doc_extraction_id: i64,
        page_number: i64,
        text: &str,
        token_count: Option<i64>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO document_chunks
             (document_extraction_id, page_number, text, token_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![doc_extraction_id, page_number, text, token_count, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_chunks_by_status(
        &self,
        embedding_status: &str,
        limit: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, document_extraction_id, page_number, text, token_count,
                    embedding, embedding_status, observation_status, created_at
             FROM document_chunks
             WHERE embedding_status = ?1
             ORDER BY created_at
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![embedding_status, limit], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_extraction_id: row.get(1)?,
                page_number: row.get(2)?,
                text: row.get(3)?,
                token_count: row.get(4)?,
                embedding: row.get(5)?,
                embedding_status: row.get(6)?,
                observation_status: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn get_chunks_for_observation(&self, limit: i64) -> Result<Vec<DocumentChunk>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, document_extraction_id, page_number, text, token_count,
                    embedding, embedding_status, observation_status, created_at
             FROM document_chunks
             WHERE embedding_status = 'done' AND observation_status = 'pending'
             ORDER BY created_at
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_extraction_id: row.get(1)?,
                page_number: row.get(2)?,
                text: row.get(3)?,
                token_count: row.get(4)?,
                embedding: row.get(5)?,
                embedding_status: row.get(6)?,
                observation_status: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn update_chunk_embedding(
        &self,
        chunk_id: i64,
        embedding_bytes: &[u8],
        status: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "UPDATE document_chunks SET embedding = ?1, embedding_status = ?2 WHERE id = ?3",
            params![embedding_bytes, status, chunk_id],
        )?;
        Ok(())
    }

    fn update_chunk_observation_status(
        &self,
        chunk_id: i64,
        status: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "UPDATE document_chunks SET observation_status = ?1 WHERE id = ?2",
            params![status, chunk_id],
        )?;
        Ok(())
    }

    fn get_chunks_for_document(
        &self,
        doc_extraction_id: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, document_extraction_id, page_number, text, token_count,
                    embedding, embedding_status, observation_status, created_at
             FROM document_chunks
             WHERE document_extraction_id = ?1
             ORDER BY page_number",
        )?;
        let rows = stmt.query_map(params![doc_extraction_id], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_extraction_id: row.get(1)?,
                page_number: row.get(2)?,
                text: row.get(3)?,
                token_count: row.get(4)?,
                embedding: row.get(5)?,
                embedding_status: row.get(6)?,
                observation_status: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn reset_failed_chunks(&self, older_than_mins: i64) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let cutoff = (chrono::Utc::now()
            - chrono::Duration::minutes(older_than_mins))
        .to_rfc3339();
        let count = conn.execute(
            "UPDATE document_chunks
             SET embedding_status = 'pending'
             WHERE embedding_status = 'failed' AND created_at < ?1",
            params![cutoff],
        )?;
        Ok(count as i64)
    }

    /// Returns (total, embedded, pending, failed, obs_done, obs_pending) chunk
    /// counts for all document_chunks belonging to documents in the given
    /// knowledge collection.
    fn get_knowledge_chunk_stats(
        &self,
        knowledge_id: i64,
    ) -> Result<(i64, i64, i64, i64, i64, i64), MchactError> {
        let conn = self.lock_conn();
        let row = conn.query_row(
            "SELECT
                COUNT(*),
                SUM(CASE WHEN dc.embedding_status = 'done'    THEN 1 ELSE 0 END),
                SUM(CASE WHEN dc.embedding_status = 'pending' THEN 1 ELSE 0 END),
                SUM(CASE WHEN dc.embedding_status = 'failed'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN dc.observation_status = 'done'    THEN 1 ELSE 0 END),
                SUM(CASE WHEN dc.observation_status = 'pending' THEN 1 ELSE 0 END)
             FROM document_chunks dc
             JOIN knowledge_documents kd ON kd.document_extraction_id = dc.document_extraction_id
             WHERE kd.knowledge_id = ?1",
            params![knowledge_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                ))
            },
        )?;
        Ok(row)
    }
}

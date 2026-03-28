use mchact_core::error::MchactError;

use crate::db::types::{DocumentChunk, Knowledge};
use crate::traits::KnowledgeStore;

use super::PgDriver;

fn map_knowledge_row(row: &tokio_postgres::Row) -> Knowledge {
    Knowledge {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        owner_chat_id: row.get("owner_chat_id"),
        last_grouping_check_at: row.get("last_grouping_check_at"),
        document_count_at_last_check: row.get("document_count_at_last_check"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_chunk_row(row: &tokio_postgres::Row) -> DocumentChunk {
    DocumentChunk {
        id: row.get("id"),
        document_extraction_id: row.get("document_extraction_id"),
        page_number: row.get("page_number"),
        text: row.get("text"),
        token_count: row.get("token_count"),
        embedding: row.get("embedding"),
        embedding_status: row.get("embedding_status"),
        observation_status: row.get("observation_status"),
        created_at: row.get("created_at"),
    }
}

const KNOWLEDGE_COLS: &str =
    "id, name, description, owner_chat_id, last_grouping_check_at, \
     document_count_at_last_check, created_at, updated_at";

const CHUNK_COLS: &str =
    "id, document_extraction_id, page_number, text, token_count, \
     embedding, embedding_status, observation_status, created_at";

impl KnowledgeStore for PgDriver {
    // ── Knowledge CRUD ────────────────────────────────────────────────────────

    fn create_knowledge(
        &self,
        name: &str,
        description: &str,
        owner_chat_id: i64,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let name = name.to_string();
        let description = description.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "INSERT INTO knowledge(name, description, owner_chat_id, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $4)
                         RETURNING id",
                        &[&name, &description, &owner_chat_id, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("create_knowledge: {e}")))?;
                Ok(row.get::<_, i64>(0))
            })
        })
    }

    fn get_knowledge_by_name(&self, name: &str) -> Result<Option<Knowledge>, MchactError> {
        let pool = self.pool.clone();
        let name = name.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        &format!("SELECT {KNOWLEDGE_COLS} FROM knowledge WHERE name = $1"),
                        &[&name],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_knowledge_by_name: {e}")))?;
                Ok(row.as_ref().map(map_knowledge_row))
            })
        })
    }

    fn list_knowledge(&self) -> Result<Vec<Knowledge>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        &format!(
                            "SELECT {KNOWLEDGE_COLS} FROM knowledge ORDER BY created_at DESC"
                        ),
                        &[],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_knowledge: {e}")))?;
                Ok(rows.iter().map(map_knowledge_row).collect())
            })
        })
    }

    fn delete_knowledge(&self, id: i64) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute("DELETE FROM knowledge WHERE id = $1", &[&id])
                    .await
                    .map_err(|e| MchactError::Database(format!("delete_knowledge: {e}")))?;
                Ok(())
            })
        })
    }

    fn update_knowledge_timestamp(&self, id: i64) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE knowledge SET updated_at = $1 WHERE id = $2",
                        &[&now, &id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("update_knowledge_timestamp: {e}")))?;
                Ok(())
            })
        })
    }

    fn update_knowledge_grouping_check(
        &self,
        knowledge_id: i64,
        doc_count: i64,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE knowledge
                         SET last_grouping_check_at = $1,
                             document_count_at_last_check = $2,
                             updated_at = $1
                         WHERE id = $3",
                        &[&now, &doc_count, &knowledge_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("update_knowledge_grouping_check: {e}")))?;
                Ok(())
            })
        })
    }

    fn get_knowledge_needing_grouping(
        &self,
        min_docs: i64,
    ) -> Result<Vec<Knowledge>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        &format!(
                            "SELECT k.id, k.name, k.description, k.owner_chat_id,
                                    k.last_grouping_check_at, k.document_count_at_last_check,
                                    k.created_at, k.updated_at
                             FROM knowledge k
                             WHERE (
                                 SELECT COUNT(*) FROM knowledge_documents kd WHERE kd.knowledge_id = k.id
                             ) >= $1
                             AND (
                                 SELECT COUNT(*) FROM knowledge_documents kd WHERE kd.knowledge_id = k.id
                             ) > k.document_count_at_last_check
                             ORDER BY k.updated_at"
                        ),
                        &[&min_docs],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_knowledge_needing_grouping: {e}")))?;
                Ok(rows.iter().map(map_knowledge_row).collect())
            })
        })
    }

    // ── Knowledge Documents ───────────────────────────────────────────────────

    fn add_document_to_knowledge(
        &self,
        knowledge_id: i64,
        doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO knowledge_documents(knowledge_id, document_extraction_id, added_at)
                         VALUES ($1, $2, $3)
                         ON CONFLICT DO NOTHING",
                        &[&knowledge_id, &doc_extraction_id, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("add_document_to_knowledge: {e}")))?;
                Ok(())
            })
        })
    }

    fn remove_document_from_knowledge(
        &self,
        knowledge_id: i64,
        doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "DELETE FROM knowledge_documents
                         WHERE knowledge_id = $1 AND document_extraction_id = $2",
                        &[&knowledge_id, &doc_extraction_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("remove_document_from_knowledge: {e}")))?;
                Ok(())
            })
        })
    }

    fn list_knowledge_documents(&self, knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT document_extraction_id FROM knowledge_documents
                         WHERE knowledge_id = $1 ORDER BY added_at",
                        &[&knowledge_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_knowledge_documents: {e}")))?;
                Ok(rows.iter().map(|r| r.get::<_, i64>(0)).collect())
            })
        })
    }

    fn count_knowledge_documents(&self, knowledge_id: i64) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "SELECT COUNT(*) FROM knowledge_documents WHERE knowledge_id = $1",
                        &[&knowledge_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("count_knowledge_documents: {e}")))?;
                Ok(row.get::<_, i64>(0))
            })
        })
    }

    // ── Knowledge Chat Access ─────────────────────────────────────────────────

    fn add_knowledge_chat_access(
        &self,
        knowledge_id: i64,
        chat_id: i64,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO knowledge_chat_access(knowledge_id, chat_id, attached_at)
                         VALUES ($1, $2, $3)
                         ON CONFLICT DO NOTHING",
                        &[&knowledge_id, &chat_id, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("add_knowledge_chat_access: {e}")))?;
                Ok(())
            })
        })
    }

    fn has_knowledge_chat_access(
        &self,
        knowledge_id: i64,
        chat_id: i64,
    ) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "SELECT COUNT(*) FROM knowledge_chat_access
                         WHERE knowledge_id = $1 AND chat_id = $2",
                        &[&knowledge_id, &chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("has_knowledge_chat_access: {e}")))?;
                Ok(row.get::<_, i64>(0) > 0)
            })
        })
    }

    fn list_knowledge_for_chat(&self, chat_id: i64) -> Result<Vec<Knowledge>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT k.id, k.name, k.description, k.owner_chat_id,
                                k.last_grouping_check_at, k.document_count_at_last_check,
                                k.created_at, k.updated_at
                         FROM knowledge k
                         JOIN knowledge_chat_access kca ON kca.knowledge_id = k.id
                         WHERE kca.chat_id = $1
                         ORDER BY k.created_at DESC",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_knowledge_for_chat: {e}")))?;
                Ok(rows.iter().map(map_knowledge_row).collect())
            })
        })
    }

    fn list_knowledge_chat_ids(&self, knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT chat_id FROM knowledge_chat_access
                         WHERE knowledge_id = $1 ORDER BY attached_at",
                        &[&knowledge_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_knowledge_chat_ids: {e}")))?;
                Ok(rows.iter().map(|r| r.get::<_, i64>(0)).collect())
            })
        })
    }

    // ── Document Chunks ───────────────────────────────────────────────────────

    fn insert_document_chunk(
        &self,
        doc_extraction_id: i64,
        page_number: i64,
        text: &str,
        token_count: Option<i64>,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let text = text.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "INSERT INTO document_chunks
                         (document_extraction_id, page_number, text, token_count, created_at)
                         VALUES ($1, $2, $3, $4, $5)
                         RETURNING id",
                        &[&doc_extraction_id, &page_number, &text, &token_count, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("insert_document_chunk: {e}")))?;
                Ok(row.get::<_, i64>(0))
            })
        })
    }

    fn get_chunks_by_status(
        &self,
        embedding_status: &str,
        limit: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        let pool = self.pool.clone();
        let embedding_status = embedding_status.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        &format!(
                            "SELECT {CHUNK_COLS}
                             FROM document_chunks
                             WHERE embedding_status = $1
                             ORDER BY created_at
                             LIMIT $2"
                        ),
                        &[&embedding_status, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_chunks_by_status: {e}")))?;
                Ok(rows.iter().map(map_chunk_row).collect())
            })
        })
    }

    fn get_chunks_for_observation(&self, limit: i64) -> Result<Vec<DocumentChunk>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        &format!(
                            "SELECT {CHUNK_COLS}
                             FROM document_chunks
                             WHERE embedding_status = 'done' AND observation_status = 'pending'
                             ORDER BY created_at
                             LIMIT $1"
                        ),
                        &[&limit],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_chunks_for_observation: {e}")))?;
                Ok(rows.iter().map(map_chunk_row).collect())
            })
        })
    }

    fn update_chunk_embedding(
        &self,
        chunk_id: i64,
        embedding_bytes: &[u8],
        status: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let embedding_bytes = embedding_bytes.to_vec();
        let status = status.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE document_chunks
                         SET embedding = $1, embedding_status = $2
                         WHERE id = $3",
                        &[&embedding_bytes, &status, &chunk_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("update_chunk_embedding: {e}")))?;
                Ok(())
            })
        })
    }

    fn update_chunk_observation_status(
        &self,
        chunk_id: i64,
        status: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let status = status.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE document_chunks SET observation_status = $1 WHERE id = $2",
                        &[&status, &chunk_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("update_chunk_observation_status: {e}")))?;
                Ok(())
            })
        })
    }

    fn get_chunks_for_document(
        &self,
        doc_extraction_id: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        &format!(
                            "SELECT {CHUNK_COLS}
                             FROM document_chunks
                             WHERE document_extraction_id = $1
                             ORDER BY page_number"
                        ),
                        &[&doc_extraction_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_chunks_for_document: {e}")))?;
                Ok(rows.iter().map(map_chunk_row).collect())
            })
        })
    }

    fn reset_failed_chunks(&self, older_than_mins: i64) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let cutoff = (chrono::Utc::now() - chrono::Duration::minutes(older_than_mins))
            .to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let affected = client
                    .execute(
                        "UPDATE document_chunks
                         SET embedding_status = 'pending'
                         WHERE embedding_status = 'failed' AND created_at < $1",
                        &[&cutoff],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("reset_failed_chunks: {e}")))?;
                Ok(affected as i64)
            })
        })
    }

    fn get_knowledge_chunk_stats(
        &self,
        knowledge_id: i64,
    ) -> Result<(i64, i64, i64, i64, i64, i64), MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "SELECT
                             COUNT(*),
                             SUM(CASE WHEN dc.embedding_status = 'done'    THEN 1 ELSE 0 END),
                             SUM(CASE WHEN dc.embedding_status = 'pending' THEN 1 ELSE 0 END),
                             SUM(CASE WHEN dc.embedding_status = 'failed'  THEN 1 ELSE 0 END),
                             SUM(CASE WHEN dc.observation_status = 'done'    THEN 1 ELSE 0 END),
                             SUM(CASE WHEN dc.observation_status = 'pending' THEN 1 ELSE 0 END)
                         FROM document_chunks dc
                         JOIN knowledge_documents kd ON kd.document_extraction_id = dc.document_extraction_id
                         WHERE kd.knowledge_id = $1",
                        &[&knowledge_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_knowledge_chunk_stats: {e}")))?;
                Ok((
                    row.get::<_, Option<i64>>(0).unwrap_or(0),
                    row.get::<_, Option<i64>>(1).unwrap_or(0),
                    row.get::<_, Option<i64>>(2).unwrap_or(0),
                    row.get::<_, Option<i64>>(3).unwrap_or(0),
                    row.get::<_, Option<i64>>(4).unwrap_or(0),
                    row.get::<_, Option<i64>>(5).unwrap_or(0),
                ))
            })
        })
    }
}

use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::{
    Memory, MemoryInjectionLog, MemoryObservabilitySummary, MemoryReflectorRun,
};
use crate::traits::MemoryDbStore;

impl MemoryDbStore for Database {
    fn insert_memory(
        &self,
        chat_id: Option<i64>,
        content: &str,
        category: &str,
    ) -> Result<i64, MchactError> {
        self.insert_memory_with_metadata(chat_id, content, category, "tool", 0.80)
    }

    fn insert_memory_with_metadata(
        &self,
        chat_id: Option<i64>,
        content: &str,
        category: &str,
        source: &str,
        confidence: f64,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let (chat_channel, external_chat_id) = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT channel, external_chat_id FROM chats WHERE chat_id = ?1",
                params![cid],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional()?
            .unwrap_or((None, None))
        } else {
            (None, None)
        };
        conn.execute(
            "INSERT INTO memories (
                chat_id, content, category, created_at, updated_at, embedding_model,
                confidence, source, last_seen_at, is_archived, archived_at,
                chat_channel, external_chat_id
            ) VALUES (?1, ?2, ?3, ?4, ?4, NULL, ?5, ?6, ?4, 0, NULL, ?7, ?8)",
            params![
                chat_id,
                content,
                category,
                now,
                confidence.clamp(0.0, 1.0),
                source,
                chat_channel,
                external_chat_id
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get a single memory by id.
    fn get_memory_by_id(&self, id: i64) -> Result<Option<Memory>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                    confidence, source, last_seen_at, is_archived, archived_at
             FROM memories WHERE id = ?1",
            params![id],
            |row| {
                Ok(Memory {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    content: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    embedding_model: row.get(6)?,
                    confidence: row.get(7)?,
                    source: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    is_archived: row.get::<_, i64>(10)? != 0,
                    archived_at: row.get(11)?,
                })
            },
        );
        match result {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_memories_for_context(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                    confidence, source, last_seen_at, is_archived, archived_at
             FROM memories
             WHERE (chat_id = ?1 OR chat_id IS NULL)
               AND is_archived = 0
               AND confidence >= 0.45
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let memories = stmt
            .query_map(params![chat_id, limit as i64], |row| {
                Ok(Memory {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    content: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    embedding_model: row.get(6)?,
                    confidence: row.get(7)?,
                    source: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    is_archived: row.get::<_, i64>(10)? != 0,
                    archived_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(memories)
    }

    fn get_all_memories_for_chat(
        &self,
        chat_id: Option<i64>,
    ) -> Result<Vec<Memory>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                    confidence, source, last_seen_at, is_archived, archived_at
             FROM memories
             WHERE (chat_id = ?1 OR (?1 IS NULL AND chat_id IS NULL))",
        )?;
        let memories = stmt
            .query_map(params![chat_id], |row| {
                Ok(Memory {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    content: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    embedding_model: row.get(6)?,
                    confidence: row.get(7)?,
                    source: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    is_archived: row.get::<_, i64>(10)? != 0,
                    archived_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(memories)
    }

    fn get_active_chat_ids_since(&self, since: &str) -> Result<Vec<i64>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT chat_id FROM messages WHERE timestamp > ?1 AND is_from_bot = 0",
        )?;
        let ids = stmt
            .query_map(params![since], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Delete a memory row by id. Returns true if a row was deleted.
    fn delete_memory(&self, id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// Keyword search in memories visible to chat_id (own + global).
    fn search_memories(
        &self,
        chat_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        self.search_memories_with_options(chat_id, query, limit, false, true)
    }

    fn search_memories_with_options(
        &self,
        chat_id: i64,
        query: &str,
        limit: usize,
        include_archived: bool,
        broad_recall: bool,
    ) -> Result<Vec<Memory>, MchactError> {
        let conn = self.lock_conn();
        let pattern = format!("%{}%", query.to_lowercase());
        let mut sql = String::from(
            "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                    confidence, source, last_seen_at, is_archived, archived_at
             FROM memories
             WHERE (chat_id = ?1 OR chat_id IS NULL)
               AND LOWER(content) LIKE ?2",
        );
        if !include_archived {
            sql.push_str(" AND is_archived = 0");
        }
        if !broad_recall {
            sql.push_str(" AND confidence >= 0.45");
        }
        sql.push_str(" ORDER BY confidence DESC, updated_at DESC LIMIT ?3");
        let mut stmt = conn.prepare(&sql)?;
        let memories = stmt
            .query_map(params![chat_id, pattern, limit as i64], |row| {
                Ok(Memory {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    content: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    embedding_model: row.get(6)?,
                    confidence: row.get(7)?,
                    source: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    is_archived: row.get::<_, i64>(10)? != 0,
                    archived_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(memories)
    }

    /// Update content and category of an existing memory. Returns true if found.
    fn update_memory_content(
        &self,
        id: i64,
        content: &str,
        category: &str,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE memories
             SET content = ?1,
                 category = ?2,
                 updated_at = ?3,
                 embedding_model = NULL,
                 last_seen_at = ?3,
                 is_archived = 0,
                 archived_at = NULL
             WHERE id = ?4",
            params![content, category, now, id],
        )?;
        Ok(rows > 0)
    }

    #[allow(clippy::too_many_arguments)]
    fn update_memory_with_metadata(
        &self,
        id: i64,
        content: &str,
        category: &str,
        confidence: f64,
        source: &str,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE memories
             SET content = ?1,
                 category = ?2,
                 updated_at = ?3,
                 embedding_model = NULL,
                 confidence = ?4,
                 source = ?5,
                 last_seen_at = ?3,
                 is_archived = 0,
                 archived_at = NULL
             WHERE id = ?6",
            params![
                content,
                category,
                now,
                confidence.clamp(0.0, 1.0),
                source,
                id
            ],
        )?;
        Ok(rows > 0)
    }

    fn update_memory_embedding_model(
        &self,
        id: i64,
        model: &str,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute(
            "UPDATE memories SET embedding_model = ?1 WHERE id = ?2",
            params![model, id],
        )?;
        Ok(rows > 0)
    }

    fn touch_memory_last_seen(
        &self,
        id: i64,
        confidence_floor: Option<f64>,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = if let Some(floor) = confidence_floor {
            conn.execute(
                "UPDATE memories
                 SET last_seen_at = ?1,
                     confidence = MAX(confidence, ?2)
                 WHERE id = ?3",
                params![now, floor.clamp(0.0, 1.0), id],
            )?
        } else {
            conn.execute(
                "UPDATE memories SET last_seen_at = ?1 WHERE id = ?2",
                params![now, id],
            )?
        };
        Ok(rows > 0)
    }

    fn archive_memory(&self, id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE memories
             SET is_archived = 1, archived_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, id],
        )?;
        Ok(rows > 0)
    }

    fn archive_stale_memories(&self, stale_days: i64) -> Result<usize, MchactError> {
        let conn = self.lock_conn();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(stale_days.max(1))).to_rfc3339();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE memories
             SET is_archived = 1, archived_at = ?1, updated_at = ?1
             WHERE is_archived = 0
               AND confidence < 0.35
               AND COALESCE(last_seen_at, updated_at, created_at) < ?2",
            params![now, cutoff],
        )?;
        Ok(rows)
    }

    fn supersede_memory(
        &self,
        from_memory_id: i64,
        new_content: &str,
        category: &str,
        source: &str,
        confidence: f64,
        reason: Option<&str>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;
        let (chat_id, chat_channel, external_chat_id): (
            Option<i64>,
            Option<String>,
            Option<String>,
        ) = tx.query_row(
            "SELECT chat_id, chat_channel, external_chat_id FROM memories WHERE id = ?1",
            params![from_memory_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO memories (
                chat_id, content, category, created_at, updated_at, embedding_model,
                confidence, source, last_seen_at, is_archived, archived_at, chat_channel, external_chat_id
            ) VALUES (?1, ?2, ?3, ?4, ?4, NULL, ?5, ?6, ?4, 0, NULL, ?7, ?8)",
            params![
                chat_id,
                new_content,
                category,
                now,
                confidence.clamp(0.0, 1.0),
                source,
                chat_channel,
                external_chat_id
            ],
        )?;
        let to_memory_id = tx.last_insert_rowid();

        tx.execute(
            "UPDATE memories
             SET is_archived = 1, archived_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, from_memory_id],
        )?;
        tx.execute(
            "INSERT INTO memory_supersede_edges(from_memory_id, to_memory_id, reason, created_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![from_memory_id, to_memory_id, reason, now],
        )?;
        tx.commit()?;
        Ok(to_memory_id)
    }

    fn get_memories_without_embedding(
        &self,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        let conn = self.lock_conn();
        let mut query = String::from(
            "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model
             , confidence, source, last_seen_at, is_archived, archived_at
             FROM memories
             WHERE embedding_model IS NULL
               AND is_archived = 0",
        );
        if chat_id.is_some() {
            query.push_str(" AND chat_id = ?1");
        }
        query.push_str(" ORDER BY updated_at DESC LIMIT ");
        query.push_str(&limit.to_string());

        let mut stmt = conn.prepare(&query)?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(Memory {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                content: row.get(2)?,
                category: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                embedding_model: row.get(6)?,
                confidence: row.get(7)?,
                source: row.get(8)?,
                last_seen_at: row.get(9)?,
                is_archived: row.get::<_, i64>(10)? != 0,
                archived_at: row.get(11)?,
            })
        };

        let rows = if let Some(cid) = chat_id {
            stmt.query_map(params![cid], mapper)?
        } else {
            stmt.query_map([], mapper)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    #[cfg(feature = "sqlite-vec")]
    fn prepare_vector_index(&self, dimension: usize) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let dimension = dimension.max(1);
        conn.execute(
            "CREATE TABLE IF NOT EXISTS db_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )?;

        let current_dim: Option<String> = conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = 'embedding_dim'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = current_dim {
            if existing != dimension.to_string() {
                conn.execute("DROP TABLE IF EXISTS memories_vec", [])?;
                conn.execute("UPDATE memories SET embedding_model = NULL", [])?;
            }
        }

        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS memories_vec USING vec0(
                    embedding float[{dimension}] distance_metric=cosine
                )"
            ),
            [],
        )?;
        conn.execute(
            "INSERT INTO db_meta(key, value) VALUES('embedding_dim', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![dimension.to_string()],
        )?;
        Ok(())
    }

    #[cfg(feature = "sqlite-vec")]
    fn upsert_memory_vec(
        &self,
        memory_id: i64,
        embedding: &[f32],
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let vector_json = serde_json::to_string(embedding)?;
        conn.execute(
            "INSERT OR REPLACE INTO memories_vec(rowid, embedding) VALUES(?1, vec_f32(?2))",
            params![memory_id, vector_json],
        )?;
        Ok(())
    }

    fn get_all_active_memories(&self) -> Result<Vec<(i64, String)>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt =
            conn.prepare("SELECT id, content FROM memories WHERE is_archived = 0 ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    #[cfg(feature = "sqlite-vec")]
    fn knn_memories(
        &self,
        chat_id: i64,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<(i64, f32)>, MchactError> {
        let conn = self.lock_conn();
        let vector_json = serde_json::to_string(query_vec)?;
        let mut stmt = conn.prepare(
            "SELECT m.id, v.distance
             FROM (
                SELECT rowid, distance
                FROM memories_vec
                WHERE embedding MATCH vec_f32(?1) AND k = ?2
             ) v
             JOIN memories m ON m.id = v.rowid
             WHERE (m.chat_id = ?3 OR m.chat_id IS NULL)
             ORDER BY v.distance ASC",
        )?;
        let rows = stmt.query_map(params![vector_json, k as i64, chat_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f32>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn get_reflector_cursor(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT last_reflected_ts FROM memory_reflector_state WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(ts) => Ok(Some(ts)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_reflector_cursor(
        &self,
        chat_id: i64,
        last_reflected_ts: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO memory_reflector_state (chat_id, last_reflected_ts, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(chat_id) DO UPDATE SET
                last_reflected_ts = excluded.last_reflected_ts,
                updated_at = excluded.updated_at",
            params![chat_id, last_reflected_ts, now],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn log_reflector_run(
        &self,
        chat_id: i64,
        started_at: &str,
        finished_at: &str,
        extracted_count: usize,
        inserted_count: usize,
        updated_count: usize,
        skipped_count: usize,
        dedup_method: &str,
        parse_ok: bool,
        error_text: Option<&str>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO memory_reflector_runs (
                chat_id, started_at, finished_at, extracted_count, inserted_count, updated_count, skipped_count, dedup_method, parse_ok, error_text
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                chat_id,
                started_at,
                finished_at,
                extracted_count as i64,
                inserted_count as i64,
                updated_count as i64,
                skipped_count as i64,
                dedup_method,
                if parse_ok { 1 } else { 0 },
                error_text
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn log_memory_injection(
        &self,
        chat_id: i64,
        retrieval_method: &str,
        candidate_count: usize,
        selected_count: usize,
        omitted_count: usize,
        tokens_est: usize,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO memory_injection_logs (
                chat_id, created_at, retrieval_method, candidate_count, selected_count, omitted_count, tokens_est
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                chat_id,
                now,
                retrieval_method,
                candidate_count as i64,
                selected_count as i64,
                omitted_count as i64,
                tokens_est as i64
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_memory_observability_summary(
        &self,
        chat_id: Option<i64>,
    ) -> Result<MemoryObservabilitySummary, MchactError> {
        let conn = self.lock_conn();
        let since_24h = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();

        let (total, active, archived, low_confidence, avg_confidence) = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(CASE WHEN is_archived = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN is_archived != 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN confidence < 0.45 THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(confidence), 0.0)
                 FROM memories
                 WHERE chat_id = ?1 OR chat_id IS NULL",
                params![cid],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, f64>(4)?,
                    ))
                },
            )?
        } else {
            conn.query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(CASE WHEN is_archived = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN is_archived != 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN confidence < 0.45 THEN 1 ELSE 0 END), 0),
                    COALESCE(AVG(confidence), 0.0)
                 FROM memories",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, f64>(4)?,
                    ))
                },
            )?
        };

        let (
            reflector_runs_24h,
            reflector_inserted_24h,
            reflector_updated_24h,
            reflector_skipped_24h,
        ) = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT
                        COUNT(*),
                        COALESCE(SUM(inserted_count), 0),
                        COALESCE(SUM(updated_count), 0),
                        COALESCE(SUM(skipped_count), 0)
                     FROM memory_reflector_runs
                     WHERE chat_id = ?1 AND unixepoch(started_at) >= unixepoch(?2)",
                params![cid, &since_24h],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )?
        } else {
            conn.query_row(
                "SELECT
                        COUNT(*),
                        COALESCE(SUM(inserted_count), 0),
                        COALESCE(SUM(updated_count), 0),
                        COALESCE(SUM(skipped_count), 0)
                     FROM memory_reflector_runs
                     WHERE unixepoch(started_at) >= unixepoch(?1)",
                params![&since_24h],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )?
        };

        let (injection_events_24h, injection_selected_24h, injection_candidates_24h) =
            if let Some(cid) = chat_id {
                conn.query_row(
                    "SELECT
                        COUNT(*),
                        COALESCE(SUM(selected_count), 0),
                        COALESCE(SUM(candidate_count), 0)
                     FROM memory_injection_logs
                     WHERE chat_id = ?1 AND unixepoch(created_at) >= unixepoch(?2)",
                    params![cid, &since_24h],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )?
            } else {
                conn.query_row(
                    "SELECT
                        COUNT(*),
                        COALESCE(SUM(selected_count), 0),
                        COALESCE(SUM(candidate_count), 0)
                     FROM memory_injection_logs
                     WHERE unixepoch(created_at) >= unixepoch(?1)",
                    params![&since_24h],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )?
            };

        Ok(MemoryObservabilitySummary {
            total,
            active,
            archived,
            low_confidence,
            avg_confidence,
            reflector_runs_24h,
            reflector_inserted_24h,
            reflector_updated_24h,
            reflector_skipped_24h,
            injection_events_24h,
            injection_selected_24h,
            injection_candidates_24h,
        })
    }

    fn get_memory_reflector_runs(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryReflectorRun>, MchactError> {
        let conn = self.lock_conn();
        let mut query = String::from(
            "SELECT id, chat_id, started_at, finished_at, extracted_count, inserted_count, updated_count, skipped_count, dedup_method, parse_ok, error_text
             FROM memory_reflector_runs",
        );
        let mut where_parts: Vec<&str> = Vec::new();
        if chat_id.is_some() {
            where_parts.push("chat_id = ?1");
        }
        if since.is_some() {
            where_parts.push(if chat_id.is_some() {
                "unixepoch(started_at) >= unixepoch(?2)"
            } else {
                "unixepoch(started_at) >= unixepoch(?1)"
            });
        }
        if !where_parts.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_parts.join(" AND "));
        }
        query.push_str(" ORDER BY unixepoch(started_at) ASC LIMIT ");
        query.push_str(&limit.max(1).to_string());
        query.push_str(" OFFSET ");
        query.push_str(&offset.to_string());

        let mut stmt = conn.prepare(&query)?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(MemoryReflectorRun {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                extracted_count: row.get(4)?,
                inserted_count: row.get(5)?,
                updated_count: row.get(6)?,
                skipped_count: row.get(7)?,
                dedup_method: row.get(8)?,
                parse_ok: row.get::<_, i64>(9)? != 0,
                error_text: row.get(10)?,
            })
        };
        let rows = match (chat_id, since) {
            (Some(cid), Some(ts)) => stmt.query_map(params![cid, ts], mapper)?,
            (Some(cid), None) => stmt.query_map(params![cid], mapper)?,
            (None, Some(ts)) => stmt.query_map(params![ts], mapper)?,
            (None, None) => stmt.query_map([], mapper)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn get_memory_injection_logs(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryInjectionLog>, MchactError> {
        let conn = self.lock_conn();
        let mut query = String::from(
            "SELECT id, chat_id, created_at, retrieval_method, candidate_count, selected_count, omitted_count, tokens_est
             FROM memory_injection_logs",
        );
        let mut where_parts: Vec<&str> = Vec::new();
        if chat_id.is_some() {
            where_parts.push("chat_id = ?1");
        }
        if since.is_some() {
            where_parts.push(if chat_id.is_some() {
                "unixepoch(created_at) >= unixepoch(?2)"
            } else {
                "unixepoch(created_at) >= unixepoch(?1)"
            });
        }
        if !where_parts.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_parts.join(" AND "));
        }
        query.push_str(" ORDER BY unixepoch(created_at) ASC LIMIT ");
        query.push_str(&limit.max(1).to_string());
        query.push_str(" OFFSET ");
        query.push_str(&offset.to_string());

        let mut stmt = conn.prepare(&query)?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(MemoryInjectionLog {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                created_at: row.get(2)?,
                retrieval_method: row.get(3)?,
                candidate_count: row.get(4)?,
                selected_count: row.get(5)?,
                omitted_count: row.get(6)?,
                tokens_est: row.get(7)?,
            })
        };
        let rows = match (chat_id, since) {
            (Some(cid), Some(ts)) => stmt.query_map(params![cid, ts], mapper)?,
            (Some(cid), None) => stmt.query_map(params![cid], mapper)?,
            (None, Some(ts)) => stmt.query_map(params![ts], mapper)?,
            (None, None) => stmt.query_map([], mapper)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

use mchact_core::error::MchactError;
use tokio_postgres::types::ToSql;

use crate::db::types::{Memory, MemoryInjectionLog, MemoryObservabilitySummary, MemoryReflectorRun};
use crate::traits::MemoryDbStore;

use super::PgDriver;

fn pg_err(e: impl std::fmt::Display) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn row_to_memory(row: &tokio_postgres::Row) -> Memory {
    Memory {
        id: row.get("id"),
        chat_id: row.get("chat_id"),
        content: row.get("content"),
        category: row.get("category"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        embedding_model: row.get("embedding_model"),
        confidence: row.get("confidence"),
        source: row.get("source"),
        last_seen_at: row.get("last_seen_at"),
        is_archived: row.get("is_archived"),
        archived_at: row.get("archived_at"),
    }
}

impl MemoryDbStore for PgDriver {
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
        let pool = self.pool.clone();
        let content = content.to_string();
        let category = category.to_string();
        let source = source.to_string();
        let confidence = confidence.clamp(0.0, 1.0);
        let now = chrono::Utc::now().to_rfc3339();

        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;

            let (chat_channel, external_chat_id): (Option<String>, Option<String>) =
                if let Some(cid) = chat_id {
                    let row = client
                        .query_opt(
                            "SELECT channel, external_chat_id FROM chats WHERE chat_id = $1",
                            &[&cid],
                        )
                        .await
                        .map_err(pg_err)?;
                    row.map(|r| (r.get(0), r.get(1))).unwrap_or((None, None))
                } else {
                    (None, None)
                };

            let row = client
                .query_one(
                    "INSERT INTO memories (
                        chat_id, content, category, created_at, updated_at, embedding_model,
                        confidence, source, last_seen_at, is_archived, archived_at,
                        chat_channel, external_chat_id
                    ) VALUES ($1, $2, $3, $4, $4, NULL, $5, $6, $4, false, NULL, $7, $8)
                    RETURNING id",
                    &[
                        &chat_id,
                        &content,
                        &category,
                        &now,
                        &confidence,
                        &source,
                        &chat_channel,
                        &external_chat_id,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_memory_by_id(&self, id: i64) -> Result<Option<Memory>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                            confidence, source, last_seen_at, is_archived, archived_at
                     FROM memories WHERE id = $1",
                    &[&id],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.as_ref().map(row_to_memory))
        })
    }

    fn get_memories_for_context(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        let pool = self.pool.clone();
        let limit = limit as i64;
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                            confidence, source, last_seen_at, is_archived, archived_at
                     FROM memories
                     WHERE (chat_id = $1 OR chat_id IS NULL)
                       AND is_archived = false
                       AND confidence >= 0.45
                     ORDER BY updated_at DESC
                     LIMIT $2",
                    &[&chat_id, &limit],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_memory).collect())
        })
    }

    fn get_all_memories_for_chat(
        &self,
        chat_id: Option<i64>,
    ) -> Result<Vec<Memory>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                            confidence, source, last_seen_at, is_archived, archived_at
                     FROM memories
                     WHERE (chat_id = $1 OR ($1 IS NULL AND chat_id IS NULL))",
                    &[&chat_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_memory).collect())
        })
    }

    fn get_active_chat_ids_since(&self, since: &str) -> Result<Vec<i64>, MchactError> {
        let pool = self.pool.clone();
        let since = since.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT DISTINCT chat_id FROM messages WHERE timestamp > $1 AND is_from_bot = false",
                    &[&since],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(|r| r.get::<_, i64>(0)).collect())
        })
    }

    fn delete_memory(&self, id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = client
                .execute("DELETE FROM memories WHERE id = $1", &[&id])
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

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
        let pool = self.pool.clone();
        let pattern = format!("%{}%", query.to_lowercase());
        let limit = limit as i64;

        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;

            // Build SQL with dynamic WHERE clauses using numbered params
            // Fixed params: $1 = chat_id, $2 = pattern, next = limit
            let mut sql = String::from(
                "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                        confidence, source, last_seen_at, is_archived, archived_at
                 FROM memories
                 WHERE (chat_id = $1 OR chat_id IS NULL)
                   AND content ILIKE $2",
            );
            if !include_archived {
                sql.push_str(" AND is_archived = false");
            }
            if !broad_recall {
                sql.push_str(" AND confidence >= 0.45");
            }
            sql.push_str(" ORDER BY confidence DESC, updated_at DESC LIMIT $3");

            let params: Vec<&(dyn ToSql + Sync)> = vec![&chat_id, &pattern, &limit];
            let rows = client.query(&sql, &params).await.map_err(pg_err)?;
            Ok(rows.iter().map(row_to_memory).collect())
        })
    }

    fn update_memory_content(
        &self,
        id: i64,
        content: &str,
        category: &str,
    ) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let content = content.to_string();
        let category = category.to_string();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = client
                .execute(
                    "UPDATE memories
                     SET content = $1,
                         category = $2,
                         updated_at = $3,
                         embedding_model = NULL,
                         last_seen_at = $3,
                         is_archived = false,
                         archived_at = NULL
                     WHERE id = $4",
                    &[&content, &category, &now, &id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn update_memory_with_metadata(
        &self,
        id: i64,
        content: &str,
        category: &str,
        confidence: f64,
        source: &str,
    ) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let content = content.to_string();
        let category = category.to_string();
        let source = source.to_string();
        let confidence = confidence.clamp(0.0, 1.0);
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = client
                .execute(
                    "UPDATE memories
                     SET content = $1,
                         category = $2,
                         updated_at = $3,
                         embedding_model = NULL,
                         confidence = $4,
                         source = $5,
                         last_seen_at = $3,
                         is_archived = false,
                         archived_at = NULL
                     WHERE id = $6",
                    &[&content, &category, &now, &confidence, &source, &id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn update_memory_embedding_model(&self, id: i64, model: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let model = model.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = client
                .execute(
                    "UPDATE memories SET embedding_model = $1 WHERE id = $2",
                    &[&model, &id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn touch_memory_last_seen(
        &self,
        id: i64,
        confidence_floor: Option<f64>,
    ) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = if let Some(floor) = confidence_floor {
                let floor = floor.clamp(0.0, 1.0);
                client
                    .execute(
                        "UPDATE memories
                         SET last_seen_at = $1,
                             confidence = GREATEST(confidence, $2)
                         WHERE id = $3",
                        &[&now, &floor, &id],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .execute(
                        "UPDATE memories SET last_seen_at = $1 WHERE id = $2",
                        &[&now, &id],
                    )
                    .await
                    .map_err(pg_err)?
            };
            Ok(n > 0)
        })
    }

    fn archive_memory(&self, id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = client
                .execute(
                    "UPDATE memories
                     SET is_archived = true, archived_at = $1, updated_at = $1
                     WHERE id = $2",
                    &[&now, &id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn archive_stale_memories(&self, stale_days: i64) -> Result<usize, MchactError> {
        let pool = self.pool.clone();
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(stale_days.max(1))).to_rfc3339();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let n = client
                .execute(
                    "UPDATE memories
                     SET is_archived = true, archived_at = $1, updated_at = $1
                     WHERE is_archived = false
                       AND confidence < 0.35
                       AND COALESCE(last_seen_at, updated_at, created_at) < $2",
                    &[&now, &cutoff],
                )
                .await
                .map_err(pg_err)?;
            Ok(n as usize)
        })
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
        let pool = self.pool.clone();
        let new_content = new_content.to_string();
        let category = category.to_string();
        let source = source.to_string();
        let confidence = confidence.clamp(0.0, 1.0);
        let reason = reason.map(|s| s.to_string());
        let now = chrono::Utc::now().to_rfc3339();

        tokio::runtime::Handle::current().block_on(async move {
            let mut client = pool.get().await.map_err(pg_err)?;
            let tx = client.transaction().await.map_err(pg_err)?;

            let orig = tx
                .query_one(
                    "SELECT chat_id, chat_channel, external_chat_id FROM memories WHERE id = $1",
                    &[&from_memory_id],
                )
                .await
                .map_err(pg_err)?;
            let chat_id: Option<i64> = orig.get("chat_id");
            let chat_channel: Option<String> = orig.get("chat_channel");
            let external_chat_id: Option<String> = orig.get("external_chat_id");

            let new_row = tx
                .query_one(
                    "INSERT INTO memories (
                        chat_id, content, category, created_at, updated_at, embedding_model,
                        confidence, source, last_seen_at, is_archived, archived_at, chat_channel, external_chat_id
                    ) VALUES ($1, $2, $3, $4, $4, NULL, $5, $6, $4, false, NULL, $7, $8)
                    RETURNING id",
                    &[
                        &chat_id,
                        &new_content,
                        &category,
                        &now,
                        &confidence,
                        &source,
                        &chat_channel,
                        &external_chat_id,
                    ],
                )
                .await
                .map_err(pg_err)?;
            let to_memory_id: i64 = new_row.get("id");

            tx.execute(
                "UPDATE memories
                 SET is_archived = true, archived_at = $1, updated_at = $1
                 WHERE id = $2",
                &[&now, &from_memory_id],
            )
            .await
            .map_err(pg_err)?;

            tx.execute(
                "INSERT INTO memory_supersede_edges(from_memory_id, to_memory_id, reason, created_at)
                 VALUES($1, $2, $3, $4)",
                &[&from_memory_id, &to_memory_id, &reason, &now],
            )
            .await
            .map_err(pg_err)?;

            tx.commit().await.map_err(pg_err)?;
            Ok(to_memory_id)
        })
    }

    fn get_memories_without_embedding(
        &self,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        let pool = self.pool.clone();
        let limit = limit as i64;
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = if let Some(cid) = chat_id {
                client
                    .query(
                        "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                                confidence, source, last_seen_at, is_archived, archived_at
                         FROM memories
                         WHERE embedding_model IS NULL
                           AND is_archived = false
                           AND chat_id = $1
                         ORDER BY updated_at DESC LIMIT $2",
                        &[&cid, &limit],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .query(
                        "SELECT id, chat_id, content, category, created_at, updated_at, embedding_model,
                                confidence, source, last_seen_at, is_archived, archived_at
                         FROM memories
                         WHERE embedding_model IS NULL
                           AND is_archived = false
                         ORDER BY updated_at DESC LIMIT $1",
                        &[&limit],
                    )
                    .await
                    .map_err(pg_err)?
            };
            Ok(rows.iter().map(row_to_memory).collect())
        })
    }

    #[cfg(feature = "vector-search")]
    fn prepare_vector_index(&self, _dimension: usize) -> Result<(), MchactError> {
        Ok(())
    }

    #[cfg(feature = "vector-search")]
    fn upsert_memory_vec(&self, _memory_id: i64, _embedding: &[f32]) -> Result<(), MchactError> {
        Ok(())
    }

    fn get_all_active_memories(&self) -> Result<Vec<(i64, String)>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let rows = client
                .query(
                    "SELECT id, content FROM memories WHERE is_archived = false ORDER BY id",
                    &[],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows
                .iter()
                .map(|r| (r.get::<_, i64>("id"), r.get::<_, String>("content")))
                .collect())
        })
    }

    #[cfg(feature = "vector-search")]
    fn knn_memories(
        &self,
        _chat_id: i64,
        _query_vec: &[f32],
        _k: usize,
    ) -> Result<Vec<(i64, f32)>, MchactError> {
        Ok(vec![])
    }

    fn get_reflector_cursor(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_opt(
                    "SELECT last_reflected_ts FROM memory_reflector_state WHERE chat_id = $1",
                    &[&chat_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.map(|r| r.get::<_, String>(0)))
        })
    }

    fn set_reflector_cursor(
        &self,
        chat_id: i64,
        last_reflected_ts: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let last_reflected_ts = last_reflected_ts.to_string();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            client
                .execute(
                    "INSERT INTO memory_reflector_state (chat_id, last_reflected_ts, updated_at)
                     VALUES ($1, $2, $3)
                     ON CONFLICT (chat_id) DO UPDATE SET
                        last_reflected_ts = EXCLUDED.last_reflected_ts,
                        updated_at = EXCLUDED.updated_at",
                    &[&chat_id, &last_reflected_ts, &now],
                )
                .await
                .map_err(pg_err)?;
            Ok(())
        })
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
        let pool = self.pool.clone();
        let started_at = started_at.to_string();
        let finished_at = finished_at.to_string();
        let extracted_count = extracted_count as i64;
        let inserted_count = inserted_count as i64;
        let updated_count = updated_count as i64;
        let skipped_count = skipped_count as i64;
        let dedup_method = dedup_method.to_string();
        let error_text = error_text.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_one(
                    "INSERT INTO memory_reflector_runs (
                        chat_id, started_at, finished_at, extracted_count, inserted_count,
                        updated_count, skipped_count, dedup_method, parse_ok, error_text
                     ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                     RETURNING id",
                    &[
                        &chat_id,
                        &started_at,
                        &finished_at,
                        &extracted_count,
                        &inserted_count,
                        &updated_count,
                        &skipped_count,
                        &dedup_method,
                        &parse_ok,
                        &error_text,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
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
        let pool = self.pool.clone();
        let retrieval_method = retrieval_method.to_string();
        let candidate_count = candidate_count as i64;
        let selected_count = selected_count as i64;
        let omitted_count = omitted_count as i64;
        let tokens_est = tokens_est as i64;
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;
            let row = client
                .query_one(
                    "INSERT INTO memory_injection_logs (
                        chat_id, created_at, retrieval_method, candidate_count, selected_count,
                        omitted_count, tokens_est
                     ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                     RETURNING id",
                    &[
                        &chat_id,
                        &now,
                        &retrieval_method,
                        &candidate_count,
                        &selected_count,
                        &omitted_count,
                        &tokens_est,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_memory_observability_summary(
        &self,
        chat_id: Option<i64>,
    ) -> Result<MemoryObservabilitySummary, MchactError> {
        let pool = self.pool.clone();
        let since_24h = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;

            let mem_row = if let Some(cid) = chat_id {
                client
                    .query_one(
                        "SELECT
                            COUNT(*),
                            COALESCE(SUM(CASE WHEN is_archived = false THEN 1 ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN is_archived = true THEN 1 ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN confidence < 0.45 THEN 1 ELSE 0 END), 0),
                            COALESCE(AVG(confidence), 0.0)
                         FROM memories
                         WHERE chat_id = $1 OR chat_id IS NULL",
                        &[&cid],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .query_one(
                        "SELECT
                            COUNT(*),
                            COALESCE(SUM(CASE WHEN is_archived = false THEN 1 ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN is_archived = true THEN 1 ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN confidence < 0.45 THEN 1 ELSE 0 END), 0),
                            COALESCE(AVG(confidence), 0.0)
                         FROM memories",
                        &[],
                    )
                    .await
                    .map_err(pg_err)?
            };

            let total: i64 = mem_row.get::<_, i64>(0);
            let active: i64 = mem_row.get::<_, i64>(1);
            let archived: i64 = mem_row.get::<_, i64>(2);
            let low_confidence: i64 = mem_row.get::<_, i64>(3);
            let avg_confidence: f64 = mem_row.get::<_, f64>(4);

            let ref_row = if let Some(cid) = chat_id {
                client
                    .query_one(
                        "SELECT
                            COUNT(*),
                            COALESCE(SUM(inserted_count), 0),
                            COALESCE(SUM(updated_count), 0),
                            COALESCE(SUM(skipped_count), 0)
                         FROM memory_reflector_runs
                         WHERE chat_id = $1 AND started_at >= $2",
                        &[&cid, &since_24h],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .query_one(
                        "SELECT
                            COUNT(*),
                            COALESCE(SUM(inserted_count), 0),
                            COALESCE(SUM(updated_count), 0),
                            COALESCE(SUM(skipped_count), 0)
                         FROM memory_reflector_runs
                         WHERE started_at >= $1",
                        &[&since_24h],
                    )
                    .await
                    .map_err(pg_err)?
            };

            let reflector_runs_24h: i64 = ref_row.get::<_, i64>(0);
            let reflector_inserted_24h: i64 = ref_row.get::<_, i64>(1);
            let reflector_updated_24h: i64 = ref_row.get::<_, i64>(2);
            let reflector_skipped_24h: i64 = ref_row.get::<_, i64>(3);

            let inj_row = if let Some(cid) = chat_id {
                client
                    .query_one(
                        "SELECT
                            COUNT(*),
                            COALESCE(SUM(selected_count), 0),
                            COALESCE(SUM(candidate_count), 0)
                         FROM memory_injection_logs
                         WHERE chat_id = $1 AND created_at >= $2",
                        &[&cid, &since_24h],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .query_one(
                        "SELECT
                            COUNT(*),
                            COALESCE(SUM(selected_count), 0),
                            COALESCE(SUM(candidate_count), 0)
                         FROM memory_injection_logs
                         WHERE created_at >= $1",
                        &[&since_24h],
                    )
                    .await
                    .map_err(pg_err)?
            };

            let injection_events_24h: i64 = inj_row.get::<_, i64>(0);
            let injection_selected_24h: i64 = inj_row.get::<_, i64>(1);
            let injection_candidates_24h: i64 = inj_row.get::<_, i64>(2);

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
        })
    }

    fn get_memory_reflector_runs(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryReflectorRun>, MchactError> {
        let pool = self.pool.clone();
        let since = since.map(|s| s.to_string());
        let limit = limit.max(1) as i64;
        let offset = offset as i64;
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;

            let rows = match (chat_id, since.as_deref()) {
                (Some(cid), Some(ts)) => {
                    client
                        .query(
                            "SELECT id, chat_id, started_at, finished_at, extracted_count, inserted_count,
                                    updated_count, skipped_count, dedup_method, parse_ok, error_text
                             FROM memory_reflector_runs
                             WHERE chat_id = $1 AND started_at >= $2
                             ORDER BY started_at ASC LIMIT $3 OFFSET $4",
                            &[&cid, &ts, &limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
                (Some(cid), None) => {
                    client
                        .query(
                            "SELECT id, chat_id, started_at, finished_at, extracted_count, inserted_count,
                                    updated_count, skipped_count, dedup_method, parse_ok, error_text
                             FROM memory_reflector_runs
                             WHERE chat_id = $1
                             ORDER BY started_at ASC LIMIT $2 OFFSET $3",
                            &[&cid, &limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
                (None, Some(ts)) => {
                    client
                        .query(
                            "SELECT id, chat_id, started_at, finished_at, extracted_count, inserted_count,
                                    updated_count, skipped_count, dedup_method, parse_ok, error_text
                             FROM memory_reflector_runs
                             WHERE started_at >= $1
                             ORDER BY started_at ASC LIMIT $2 OFFSET $3",
                            &[&ts, &limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
                (None, None) => {
                    client
                        .query(
                            "SELECT id, chat_id, started_at, finished_at, extracted_count, inserted_count,
                                    updated_count, skipped_count, dedup_method, parse_ok, error_text
                             FROM memory_reflector_runs
                             ORDER BY started_at ASC LIMIT $1 OFFSET $2",
                            &[&limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
            };

            Ok(rows
                .iter()
                .map(|r| MemoryReflectorRun {
                    id: r.get("id"),
                    chat_id: r.get("chat_id"),
                    started_at: r.get("started_at"),
                    finished_at: r.get("finished_at"),
                    extracted_count: r.get("extracted_count"),
                    inserted_count: r.get("inserted_count"),
                    updated_count: r.get("updated_count"),
                    skipped_count: r.get("skipped_count"),
                    dedup_method: r.get("dedup_method"),
                    parse_ok: r.get("parse_ok"),
                    error_text: r.get("error_text"),
                })
                .collect())
        })
    }

    fn get_memory_injection_logs(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryInjectionLog>, MchactError> {
        let pool = self.pool.clone();
        let since = since.map(|s| s.to_string());
        let limit = limit.max(1) as i64;
        let offset = offset as i64;
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pg_err)?;

            let rows = match (chat_id, since.as_deref()) {
                (Some(cid), Some(ts)) => {
                    client
                        .query(
                            "SELECT id, chat_id, created_at, retrieval_method, candidate_count,
                                    selected_count, omitted_count, tokens_est
                             FROM memory_injection_logs
                             WHERE chat_id = $1 AND created_at >= $2
                             ORDER BY created_at ASC LIMIT $3 OFFSET $4",
                            &[&cid, &ts, &limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
                (Some(cid), None) => {
                    client
                        .query(
                            "SELECT id, chat_id, created_at, retrieval_method, candidate_count,
                                    selected_count, omitted_count, tokens_est
                             FROM memory_injection_logs
                             WHERE chat_id = $1
                             ORDER BY created_at ASC LIMIT $2 OFFSET $3",
                            &[&cid, &limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
                (None, Some(ts)) => {
                    client
                        .query(
                            "SELECT id, chat_id, created_at, retrieval_method, candidate_count,
                                    selected_count, omitted_count, tokens_est
                             FROM memory_injection_logs
                             WHERE created_at >= $1
                             ORDER BY created_at ASC LIMIT $2 OFFSET $3",
                            &[&ts, &limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
                (None, None) => {
                    client
                        .query(
                            "SELECT id, chat_id, created_at, retrieval_method, candidate_count,
                                    selected_count, omitted_count, tokens_est
                             FROM memory_injection_logs
                             ORDER BY created_at ASC LIMIT $1 OFFSET $2",
                            &[&limit, &offset],
                        )
                        .await
                        .map_err(pg_err)?
                }
            };

            Ok(rows
                .iter()
                .map(|r| MemoryInjectionLog {
                    id: r.get("id"),
                    chat_id: r.get("chat_id"),
                    created_at: r.get("created_at"),
                    retrieval_method: r.get("retrieval_method"),
                    candidate_count: r.get("candidate_count"),
                    selected_count: r.get("selected_count"),
                    omitted_count: r.get("omitted_count"),
                    tokens_est: r.get("tokens_est"),
                })
                .collect())
        })
    }
}

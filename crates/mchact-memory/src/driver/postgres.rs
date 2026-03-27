use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::{Config as PoolConfig, Pool, Runtime};
use tokio_postgres::Row;

use crate::types::{
    DeriverRun, DreamerRun, Finding, InjectionLog, NewObservation, Observation, ObservationLevel,
    ObservationUpdate, Peer, PeerCard, PeerKind, QueueItem, QueueTask, RankedResult, SearchScope,
};
use crate::{MemoryError, ObservationStore, Result};

// ---------------------------------------------------------------------------
// Schema SQL (split into individual statements by the connect fn)
// ---------------------------------------------------------------------------

const SCHEMA_SQL: &str = include_str!("../schema/postgres.sql");

// ---------------------------------------------------------------------------
// PgDriver
// ---------------------------------------------------------------------------

pub struct PgDriver {
    pool: Pool,
}

impl PgDriver {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let mut cfg = PoolConfig::new();
        cfg.url = Some(database_url.to_string());

        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        // Run schema migrations — execute each statement individually
        {
            let client = pool
                .get()
                .await
                .map_err(|e| MemoryError::Database(e.to_string()))?;

            for stmt in SCHEMA_SQL.split(';') {
                let trimmed = stmt.trim();
                if trimmed.is_empty() {
                    continue;
                }
                client
                    .execute(trimmed, &[])
                    .await
                    .map_err(|e| MemoryError::Database(format!("schema: {e}: {trimmed}")))?;
            }
        }

        Ok(Self { pool })
    }
}

// ---------------------------------------------------------------------------
// Row mapping helpers
// ---------------------------------------------------------------------------

fn row_to_peer(row: &Row) -> Result<Peer> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let workspace: String =
        row.try_get("workspace").map_err(|e| MemoryError::Database(e.to_string()))?;
    let name: String =
        row.try_get("name").map_err(|e| MemoryError::Database(e.to_string()))?;
    let kind_str: String =
        row.try_get("kind").map_err(|e| MemoryError::Database(e.to_string()))?;
    let peer_card_json: Option<serde_json::Value> =
        row.try_get("peer_card").map_err(|e| MemoryError::Database(e.to_string()))?;
    let metadata_json: Option<serde_json::Value> =
        row.try_get("metadata").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;
    let updated_at: DateTime<Utc> =
        row.try_get("updated_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    let kind = PeerKind::from_str(&kind_str).unwrap_or(PeerKind::User);
    let peer_card: Option<Vec<String>> =
        peer_card_json.and_then(|v| serde_json::from_value(v).ok());
    let metadata = metadata_json.unwrap_or(serde_json::Value::Null);

    Ok(Peer { id, workspace, name, kind, peer_card, metadata, created_at, updated_at })
}

fn row_to_observation(row: &Row) -> Result<Observation> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let workspace: String =
        row.try_get("workspace").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observer_peer_id: i64 =
        row.try_get("observer_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observed_peer_id: i64 =
        row.try_get("observed_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let chat_id: Option<String> =
        row.try_get("chat_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let level_str: String =
        row.try_get("level").map_err(|e| MemoryError::Database(e.to_string()))?;
    let content: String =
        row.try_get("content").map_err(|e| MemoryError::Database(e.to_string()))?;
    let category: Option<String> =
        row.try_get("category").map_err(|e| MemoryError::Database(e.to_string()))?;
    let confidence: f32 =
        row.try_get("confidence").map_err(|e| MemoryError::Database(e.to_string()))?;
    let source: Option<String> =
        row.try_get("source").map_err(|e| MemoryError::Database(e.to_string()))?;
    let source_ids_json: serde_json::Value =
        row.try_get("source_ids").map_err(|e| MemoryError::Database(e.to_string()))?;
    let message_ids_json: serde_json::Value =
        row.try_get("message_ids").map_err(|e| MemoryError::Database(e.to_string()))?;
    let times_derived: i32 =
        row.try_get("times_derived").map_err(|e| MemoryError::Database(e.to_string()))?;
    let is_archived: bool =
        row.try_get("is_archived").map_err(|e| MemoryError::Database(e.to_string()))?;
    let archived_at: Option<DateTime<Utc>> =
        row.try_get("archived_at").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;
    let updated_at: DateTime<Utc> =
        row.try_get("updated_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    let level = ObservationLevel::from_str(&level_str).unwrap_or(ObservationLevel::Explicit);
    let source_ids: Vec<i64> = serde_json::from_value(source_ids_json).unwrap_or_default();
    let message_ids: Vec<i64> = serde_json::from_value(message_ids_json).unwrap_or_default();

    Ok(Observation {
        id,
        workspace,
        observer_peer_id,
        observed_peer_id,
        chat_id,
        level,
        content,
        category,
        confidence: confidence as f64,
        source,
        source_ids,
        message_ids,
        times_derived: times_derived as i64,
        is_archived,
        archived_at,
        created_at,
        updated_at,
    })
}

fn row_to_queue_item(row: &Row) -> Result<QueueItem> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let task_type: String =
        row.try_get("task_type").map_err(|e| MemoryError::Database(e.to_string()))?;
    let workspace: String =
        row.try_get("workspace").map_err(|e| MemoryError::Database(e.to_string()))?;
    let chat_id: Option<String> =
        row.try_get("chat_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observer_peer_id: i64 =
        row.try_get("observer_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observed_peer_id: i64 =
        row.try_get("observed_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let payload_json: serde_json::Value =
        row.try_get("payload").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    Ok(QueueItem {
        id,
        task_type,
        workspace,
        chat_id,
        observer_peer_id,
        observed_peer_id,
        payload: payload_json,
        created_at,
    })
}

fn row_to_finding(row: &Row) -> Result<Finding> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let orchestration_id: String =
        row.try_get("orchestration_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let run_id: String =
        row.try_get("run_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let finding: String =
        row.try_get("finding").map_err(|e| MemoryError::Database(e.to_string()))?;
    let category: Option<String> =
        row.try_get("category").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    Ok(Finding { id, orchestration_id, run_id, finding, category, created_at })
}

fn row_to_deriver_run(row: &Row) -> Result<DeriverRun> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let orchestration_id: String =
        row.try_get("orchestration_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let workspace: String =
        row.try_get("workspace").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observer_peer_id: i64 =
        row.try_get("observer_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observed_peer_id: i64 =
        row.try_get("observed_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let chat_id: Option<String> =
        row.try_get("chat_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observations_in: i64 =
        row.try_get("observations_in").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observations_out: i64 =
        row.try_get("observations_out").map_err(|e| MemoryError::Database(e.to_string()))?;
    let duration_ms: i64 =
        row.try_get("duration_ms").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    Ok(DeriverRun {
        id,
        orchestration_id,
        workspace,
        observer_peer_id,
        observed_peer_id,
        chat_id,
        observations_in,
        observations_out,
        duration_ms,
        created_at,
    })
}

fn row_to_dreamer_run(row: &Row) -> Result<DreamerRun> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let orchestration_id: String =
        row.try_get("orchestration_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let workspace: String =
        row.try_get("workspace").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observer_peer_id: i64 =
        row.try_get("observer_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observed_peer_id: i64 =
        row.try_get("observed_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observations_in: i64 =
        row.try_get("observations_in").map_err(|e| MemoryError::Database(e.to_string()))?;
    let findings_out: i64 =
        row.try_get("findings_out").map_err(|e| MemoryError::Database(e.to_string()))?;
    let duration_ms: i64 =
        row.try_get("duration_ms").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    Ok(DreamerRun {
        id,
        orchestration_id,
        workspace,
        observer_peer_id,
        observed_peer_id,
        observations_in,
        findings_out,
        duration_ms,
        created_at,
    })
}

fn row_to_injection_log(row: &Row) -> Result<InjectionLog> {
    let id: i64 = row.try_get("id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let orchestration_id: String =
        row.try_get("orchestration_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let workspace: String =
        row.try_get("workspace").map_err(|e| MemoryError::Database(e.to_string()))?;
    let chat_id: String =
        row.try_get("chat_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observer_peer_id: i64 =
        row.try_get("observer_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observed_peer_id: i64 =
        row.try_get("observed_peer_id").map_err(|e| MemoryError::Database(e.to_string()))?;
    let observations_injected: i64 =
        row.try_get("observations_injected").map_err(|e| MemoryError::Database(e.to_string()))?;
    let token_estimate: i64 =
        row.try_get("token_estimate").map_err(|e| MemoryError::Database(e.to_string()))?;
    let created_at: DateTime<Utc> =
        row.try_get("created_at").map_err(|e| MemoryError::Database(e.to_string()))?;

    Ok(InjectionLog {
        id,
        orchestration_id,
        workspace,
        chat_id,
        observer_peer_id,
        observed_peer_id,
        observations_injected,
        token_estimate,
        created_at,
    })
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ObservationStore for PgDriver {
    // --- Peers ---

    async fn upsert_peer(
        &self,
        workspace: &str,
        name: &str,
        kind: PeerKind,
        metadata: Option<serde_json::Value>,
    ) -> Result<Peer> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();
        let metadata_val = metadata.unwrap_or(serde_json::Value::Null);

        client.execute(
            "INSERT INTO peers (workspace, name, kind, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $5)
             ON CONFLICT(workspace, name) DO UPDATE SET
                kind = excluded.kind,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at",
            &[&workspace, &name, &kind.as_str(), &metadata_val, &now],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let row = client.query_one(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE workspace = $1 AND name = $2",
            &[&workspace, &name],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_peer(&row)
    }

    async fn get_peer_by_name(&self, workspace: &str, name: &str) -> Result<Option<Peer>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = client.query_opt(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE workspace = $1 AND name = $2",
            &[&workspace, &name],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        result.map(|row| row_to_peer(&row)).transpose()
    }

    async fn get_peer_by_id(&self, id: i64) -> Result<Option<Peer>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = client.query_opt(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE id = $1",
            &[&id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        result.map(|row| row_to_peer(&row)).transpose()
    }

    async fn list_peers(&self, workspace: &str) -> Result<Vec<Peer>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let rows = client.query(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE workspace = $1 ORDER BY id ASC",
            &[&workspace],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_peer).collect()
    }

    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<PeerCard> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let facts_capped: Vec<String> = facts.into_iter().take(PeerCard::MAX_FACTS).collect();
        let card_json = serde_json::to_value(&facts_capped)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let now = Utc::now();

        client.execute(
            "UPDATE peers SET peer_card = $1, updated_at = $2 WHERE id = $3",
            &[&card_json, &now, &peer_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let row = client.query_opt("SELECT name FROM peers WHERE id = $1", &[&peer_id])
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .ok_or_else(|| MemoryError::NotFound(format!("peer {peer_id}")))?;

        let peer_name: String =
            row.try_get("name").map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(PeerCard { peer_id, peer_name, facts: facts_capped })
    }

    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = client.query_opt(
            "SELECT name, peer_card FROM peers WHERE id = $1",
            &[&peer_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        match result {
            None => Ok(None),
            Some(row) => {
                let peer_name: String =
                    row.try_get("name").map_err(|e| MemoryError::Database(e.to_string()))?;
                let card_json: Option<serde_json::Value> = row
                    .try_get("peer_card")
                    .map_err(|e| MemoryError::Database(e.to_string()))?;
                match card_json {
                    None => Ok(None),
                    Some(v) => {
                        let facts: Vec<String> = serde_json::from_value(v)
                            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
                        Ok(Some(PeerCard { peer_id, peer_name, facts }))
                    }
                }
            }
        }
    }

    // --- Observations CRUD ---

    async fn create_observation(&self, new_obs: NewObservation) -> Result<Observation> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();
        let source_ids_val = serde_json::to_value(&new_obs.source_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let message_ids_val = serde_json::to_value(&new_obs.message_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let confidence = new_obs.confidence as f32;

        let row = client.query_one(
            "INSERT INTO observations
             (workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 0, false, $12, $12)
             RETURNING id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
               content, category, confidence, source, source_ids, message_ids, times_derived,
               is_archived, archived_at, created_at, updated_at",
            &[
                &new_obs.workspace,
                &new_obs.observer_peer_id,
                &new_obs.observed_peer_id,
                &new_obs.chat_id,
                &new_obs.level.as_str(),
                &new_obs.content,
                &new_obs.category,
                &confidence,
                &new_obs.source,
                &source_ids_val,
                &message_ids_val,
                &now,
            ],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_observation(&row)
    }

    async fn get_observation(&self, id: i64) -> Result<Option<Observation>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = client.query_opt(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
              content, category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id = $1",
            &[&id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        result.map(|row| row_to_observation(&row)).transpose()
    }

    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<Observation> {
        let current = self
            .get_observation(id)
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("observation {id}")))?;

        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();
        let level = update.level.unwrap_or(current.level);
        let content = update.content.unwrap_or(current.content);
        let category = update.category.or(current.category);
        let confidence = update.confidence.unwrap_or(current.confidence) as f32;
        let source = update.source.or(current.source);
        let source_ids = update.source_ids.unwrap_or(current.source_ids);
        let message_ids = update.message_ids.unwrap_or(current.message_ids);
        let is_archived = update.is_archived.unwrap_or(current.is_archived);
        let archived_at = update.archived_at.or(current.archived_at);

        let source_ids_val = serde_json::to_value(&source_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let message_ids_val = serde_json::to_value(&message_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        let row = client.query_one(
            "UPDATE observations SET
               level = $1, content = $2, category = $3, confidence = $4, source = $5,
               source_ids = $6, message_ids = $7, is_archived = $8, archived_at = $9,
               updated_at = $10
             WHERE id = $11
             RETURNING id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
               content, category, confidence, source, source_ids, message_ids, times_derived,
               is_archived, archived_at, created_at, updated_at",
            &[
                &level.as_str(),
                &content,
                &category,
                &confidence,
                &source,
                &source_ids_val,
                &message_ids_val,
                &is_archived,
                &archived_at,
                &now,
                &id,
            ],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_observation(&row)
    }

    async fn delete_observation(&self, id: i64) -> Result<()> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        client.execute("DELETE FROM observations WHERE id = $1", &[&id])
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn archive_observation(&self, id: i64) -> Result<Observation> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let result = client.query_opt(
            "UPDATE observations SET is_archived = true, archived_at = $1, updated_at = $1
             WHERE id = $2
             RETURNING id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
               content, category, confidence, source, source_ids, message_ids, times_derived,
               is_archived, archived_at, created_at, updated_at",
            &[&now, &id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        match result {
            Some(row) => row_to_observation(&row),
            None => Err(MemoryError::NotFound(format!("observation {id}"))),
        }
    }

    async fn list_observations(
        &self,
        scope: SearchScope,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Observation>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut conditions: Vec<String> = vec!["workspace = $1".to_string()];
        let mut idx = 2usize;

        if scope.observer_peer_id.is_some() {
            conditions.push(format!("observer_peer_id = ${idx}"));
            idx += 1;
        }
        if scope.observed_peer_id.is_some() {
            conditions.push(format!("observed_peer_id = ${idx}"));
            idx += 1;
        }
        if scope.chat_id.is_some() {
            conditions.push(format!("chat_id = ${idx}"));
            idx += 1;
        }
        if scope.min_confidence.is_some() {
            conditions.push(format!("confidence >= ${idx}"));
            idx += 1;
        }
        if !scope.include_archived {
            conditions.push("is_archived = false".to_string());
        }

        let limit_idx = idx;
        let offset_idx = idx + 1;

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
              content, category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE {where_clause}
             ORDER BY created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"
        );

        // Build parameter list dynamically using trait objects
        let workspace_val = scope.workspace.clone();
        let observer_val = scope.observer_peer_id;
        let observed_val = scope.observed_peer_id;
        let chat_val = scope.chat_id.clone();
        let confidence_val = scope.min_confidence.map(|v| v as f32);

        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
        params.push(&workspace_val);
        if let Some(ref v) = observer_val {
            params.push(v);
        }
        if let Some(ref v) = observed_val {
            params.push(v);
        }
        if let Some(ref v) = chat_val {
            params.push(v);
        }
        if let Some(ref v) = confidence_val {
            params.push(v);
        }
        params.push(&limit);
        params.push(&offset);

        let rows = client
            .query(&sql as &str, &params)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_observation).collect()
    }

    // --- Search ---

    async fn keyword_search(
        &self,
        scope: &SearchScope,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Observation>> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut conditions: Vec<String> = vec!["o.workspace = $2".to_string()];
        let mut idx = 3usize;

        if scope.observer_peer_id.is_some() {
            conditions.push(format!("o.observer_peer_id = ${idx}"));
            idx += 1;
        }
        if scope.observed_peer_id.is_some() {
            conditions.push(format!("o.observed_peer_id = ${idx}"));
            idx += 1;
        }
        if scope.chat_id.is_some() {
            conditions.push(format!("o.chat_id = ${idx}"));
            idx += 1;
        }
        if scope.min_confidence.is_some() {
            conditions.push(format!("o.confidence >= ${idx}"));
            idx += 1;
        }
        if !scope.include_archived {
            conditions.push("o.is_archived = false".to_string());
        }

        let limit_idx = idx;
        let extra = if conditions.is_empty() {
            String::new()
        } else {
            format!("AND {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT o.id, o.workspace, o.observer_peer_id, o.observed_peer_id, o.chat_id,
              o.level, o.content, o.category, o.confidence, o.source, o.source_ids,
              o.message_ids, o.times_derived, o.is_archived, o.archived_at,
              o.created_at, o.updated_at
             FROM observations o
             WHERE o.tsv @@ plainto_tsquery('english', $1)
             {extra}
             ORDER BY ts_rank(o.tsv, plainto_tsquery('english', $1)) DESC
             LIMIT ${limit_idx}"
        );

        let workspace_val = scope.workspace.clone();
        let observer_val = scope.observer_peer_id;
        let observed_val = scope.observed_peer_id;
        let chat_val = scope.chat_id.clone();
        let confidence_val = scope.min_confidence.map(|v| v as f32);

        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
        params.push(&query);
        params.push(&workspace_val);
        if let Some(ref v) = observer_val {
            params.push(v);
        }
        if let Some(ref v) = observed_val {
            params.push(v);
        }
        if let Some(ref v) = chat_val {
            params.push(v);
        }
        if let Some(ref v) = confidence_val {
            params.push(v);
        }
        params.push(&limit);

        let rows = client
            .query(&sql as &str, &params)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_observation).collect()
    }

    async fn semantic_search(
        &self,
        _scope: &SearchScope,
        _embedding: Vec<f32>,
        _limit: i64,
    ) -> Result<Vec<Observation>> {
        Ok(vec![])
    }

    async fn hybrid_search(
        &self,
        scope: &SearchScope,
        query: &str,
        _embedding: Vec<f32>,
        limit: i64,
    ) -> Result<Vec<RankedResult>> {
        use crate::search::rrf_merge;

        let keyword_hits = self.keyword_search(scope, query, limit * 2).await?;
        let keyword_scored: Vec<(i64, f64)> = keyword_hits
            .iter()
            .enumerate()
            .map(|(rank, obs)| (obs.id, rank as f64))
            .collect();
        let semantic_scored: Vec<(i64, f64)> = vec![];
        let ranked = rrf_merge(&keyword_scored, &semantic_scored, limit as usize);
        Ok(ranked)
    }

    // --- DAG ---

    async fn link_observations(&self, parent_id: i64, child_id: i64) -> Result<()> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_opt(
            "SELECT source_ids FROM observations WHERE id = $1",
            &[&child_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?
        .ok_or_else(|| MemoryError::NotFound(format!("observation {child_id}")))?;

        let source_ids_val: serde_json::Value =
            row.try_get("source_ids").map_err(|e| MemoryError::Database(e.to_string()))?;
        let mut source_ids: Vec<i64> =
            serde_json::from_value(source_ids_val).unwrap_or_default();

        if !source_ids.contains(&parent_id) {
            source_ids.push(parent_id);
        }

        let new_val = serde_json::to_value(&source_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        client.execute(
            "UPDATE observations SET source_ids = $1, updated_at = $2 WHERE id = $3",
            &[&new_val, &now, &child_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn unlink_observations(&self, parent_id: i64, child_id: i64) -> Result<()> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_opt(
            "SELECT source_ids FROM observations WHERE id = $1",
            &[&child_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?
        .ok_or_else(|| MemoryError::NotFound(format!("observation {child_id}")))?;

        let source_ids_val: serde_json::Value =
            row.try_get("source_ids").map_err(|e| MemoryError::Database(e.to_string()))?;
        let source_ids: Vec<i64> =
            serde_json::from_value(source_ids_val).unwrap_or_default();
        let filtered: Vec<i64> =
            source_ids.into_iter().filter(|&id| id != parent_id).collect();

        let new_val = serde_json::to_value(&filtered)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        client.execute(
            "UPDATE observations SET source_ids = $1, updated_at = $2 WHERE id = $3",
            &[&new_val, &now, &child_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_children(&self, observation_id: i64) -> Result<Vec<Observation>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let id_json = serde_json::json!([observation_id]);
        let rows = client.query(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
              content, category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations
             WHERE source_ids @> $1::jsonb
             ORDER BY id ASC",
            &[&id_json],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_observation).collect()
    }

    async fn get_parents(&self, observation_id: i64) -> Result<Vec<Observation>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let row = client.query_opt(
            "SELECT source_ids FROM observations WHERE id = $1",
            &[&observation_id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let source_ids_val = match row {
            None => return Ok(vec![]),
            Some(r) => r
                .try_get::<_, serde_json::Value>("source_ids")
                .map_err(|e| MemoryError::Database(e.to_string()))?,
        };

        let source_ids: Vec<i64> =
            serde_json::from_value(source_ids_val).unwrap_or_default();

        if source_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: String = source_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level,
              content, category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id IN ({placeholders}) ORDER BY id ASC"
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            source_ids.iter().map(|id| id as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

        let rows = client
            .query(&sql as &str, &params)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_observation).collect()
    }

    // --- Queue ---

    async fn enqueue(&self, task: QueueTask) -> Result<QueueItem> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_one(
            "INSERT INTO observation_queue
             (task_type, workspace, chat_id, observer_peer_id, observed_peer_id,
              payload, processed, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, false, $7)
             RETURNING id, task_type, workspace, chat_id, observer_peer_id, observed_peer_id,
               payload, created_at",
            &[
                &task.task_type,
                &task.workspace,
                &task.chat_id,
                &task.observer_peer_id,
                &task.observed_peer_id,
                &task.payload,
                &now,
            ],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_queue_item(&row)
    }

    async fn dequeue(&self, limit: i64) -> Result<Vec<QueueItem>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let rows = client.query(
            "SELECT id, task_type, workspace, chat_id, observer_peer_id, observed_peer_id,
              payload, created_at
             FROM observation_queue
             WHERE processed = false
             ORDER BY id ASC
             LIMIT $1",
            &[&limit],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_queue_item).collect()
    }

    async fn ack_queue_item(&self, id: i64) -> Result<()> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        client.execute(
            "UPDATE observation_queue SET processed = true, processed_at = $1 WHERE id = $2",
            &[&now, &id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn nack_queue_item(&self, id: i64) -> Result<()> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        client.execute(
            "UPDATE observation_queue SET processed = false, processed_at = NULL WHERE id = $1",
            &[&id],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    // --- Findings ---

    async fn save_finding(&self, finding: Finding) -> Result<Finding> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_one(
            "INSERT INTO findings (orchestration_id, run_id, finding, category, created_at)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id, orchestration_id, run_id, finding, category, created_at",
            &[&finding.orchestration_id, &finding.run_id, &finding.finding, &finding.category, &now],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_finding(&row)
    }

    async fn list_findings(
        &self,
        orchestration_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Finding>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let rows = client.query(
            "SELECT id, orchestration_id, run_id, finding, category, created_at
             FROM findings
             WHERE orchestration_id = $1
             ORDER BY id ASC
             LIMIT $2 OFFSET $3",
            &[&orchestration_id, &limit, &offset],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_finding).collect()
    }

    // --- Embedding ---

    async fn store_embedding(&self, _observation_id: i64, _embedding: Vec<f32>) -> Result<()> {
        Ok(())
    }

    async fn get_embedding(&self, _observation_id: i64) -> Result<Option<Vec<f32>>> {
        Ok(None)
    }

    // --- Observability ---

    async fn save_deriver_run(&self, run: DeriverRun) -> Result<DeriverRun> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_one(
            "INSERT INTO deriver_runs
             (orchestration_id, workspace, observer_peer_id, observed_peer_id, chat_id,
              observations_in, observations_out, duration_ms, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
               chat_id, observations_in, observations_out, duration_ms, created_at",
            &[
                &run.orchestration_id,
                &run.workspace,
                &run.observer_peer_id,
                &run.observed_peer_id,
                &run.chat_id,
                &run.observations_in,
                &run.observations_out,
                &run.duration_ms,
                &now,
            ],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_deriver_run(&row)
    }

    async fn save_dreamer_run(&self, run: DreamerRun) -> Result<DreamerRun> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_one(
            "INSERT INTO dreamer_runs
             (orchestration_id, workspace, observer_peer_id, observed_peer_id,
              observations_in, findings_out, duration_ms, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
               observations_in, findings_out, duration_ms, created_at",
            &[
                &run.orchestration_id,
                &run.workspace,
                &run.observer_peer_id,
                &run.observed_peer_id,
                &run.observations_in,
                &run.findings_out,
                &run.duration_ms,
                &now,
            ],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_dreamer_run(&row)
    }

    async fn save_injection_log(&self, log: InjectionLog) -> Result<InjectionLog> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now();

        let row = client.query_one(
            "INSERT INTO injection_logs
             (orchestration_id, workspace, chat_id, observer_peer_id, observed_peer_id,
              observations_injected, token_estimate, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, orchestration_id, workspace, chat_id, observer_peer_id,
               observed_peer_id, observations_injected, token_estimate, created_at",
            &[
                &log.orchestration_id,
                &log.workspace,
                &log.chat_id,
                &log.observer_peer_id,
                &log.observed_peer_id,
                &log.observations_injected,
                &log.token_estimate,
                &now,
            ],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        row_to_injection_log(&row)
    }

    async fn list_deriver_runs(
        &self,
        workspace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DeriverRun>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let rows = client.query(
            "SELECT id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
              chat_id, observations_in, observations_out, duration_ms, created_at
             FROM deriver_runs
             WHERE workspace = $1
             ORDER BY id DESC
             LIMIT $2 OFFSET $3",
            &[&workspace, &limit, &offset],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_deriver_run).collect()
    }

    async fn list_dreamer_runs(
        &self,
        workspace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DreamerRun>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let rows = client.query(
            "SELECT id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
              observations_in, findings_out, duration_ms, created_at
             FROM dreamer_runs
             WHERE workspace = $1
             ORDER BY id DESC
             LIMIT $2 OFFSET $3",
            &[&workspace, &limit, &offset],
        )
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_dreamer_run).collect()
    }

    async fn list_injection_logs(
        &self,
        workspace: &str,
        chat_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InjectionLog>> {
        let client = self.pool.get().await.map_err(|e| MemoryError::Database(e.to_string()))?;

        let rows = if let Some(cid) = chat_id {
            client.query(
                "SELECT id, orchestration_id, workspace, chat_id, observer_peer_id,
                  observed_peer_id, observations_injected, token_estimate, created_at
                 FROM injection_logs
                 WHERE workspace = $1 AND chat_id = $2
                 ORDER BY id DESC
                 LIMIT $3 OFFSET $4",
                &[&workspace, &cid, &limit, &offset],
            )
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?
        } else {
            client.query(
                "SELECT id, orchestration_id, workspace, chat_id, observer_peer_id,
                  observed_peer_id, observations_injected, token_estimate, created_at
                 FROM injection_logs
                 WHERE workspace = $1
                 ORDER BY id DESC
                 LIMIT $2 OFFSET $3",
                &[&workspace, &limit, &offset],
            )
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?
        };

        rows.iter().map(row_to_injection_log).collect()
    }
}

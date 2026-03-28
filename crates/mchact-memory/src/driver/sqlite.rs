use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, Row, params};

use crate::{MemoryError, ObservationStore, Result};
use crate::types::{
    DeriverRun, DreamerRun, Finding, InjectionLog, NewObservation, Observation, ObservationLevel,
    ObservationUpdate, Peer, PeerCard, PeerKind, QueueItem, QueueTask, RankedResult, SearchScope,
};

// ---------------------------------------------------------------------------
// Schema SQL
// ---------------------------------------------------------------------------

const SCHEMA_SQL: &str = include_str!("../schema/sqlite.sql");

// ---------------------------------------------------------------------------
// SqliteDriver
// ---------------------------------------------------------------------------

pub struct SqliteDriver {
    conn: Mutex<Connection>,
}

impl SqliteDriver {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        let driver = Self { conn: Mutex::new(conn) };
        driver.initialize()?;
        Ok(driver)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        let driver = Self { conn: Mutex::new(conn) };
        driver.initialize()?;
        Ok(driver)
    }

    fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Row mapping helpers
// ---------------------------------------------------------------------------

fn row_to_peer(row: &Row<'_>) -> rusqlite::Result<Peer> {
    let id: i64 = row.get(0)?;
    let workspace: String = row.get(1)?;
    let name: String = row.get(2)?;
    let kind_str: String = row.get(3)?;
    let peer_card_json: Option<String> = row.get(4)?;
    let metadata_str: String = row.get(5)?;
    let created_at_str: String = row.get(6)?;
    let updated_at_str: String = row.get(7)?;

    let kind = PeerKind::parse(&kind_str).unwrap_or(PeerKind::User);

    let peer_card: Option<Vec<String>> = peer_card_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
        .unwrap_or(serde_json::Value::Null);

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    let updated_at = updated_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(Peer {
        id,
        workspace,
        name,
        kind,
        peer_card,
        metadata,
        created_at,
        updated_at,
    })
}

fn row_to_observation(row: &Row<'_>) -> rusqlite::Result<Observation> {
    let id: i64 = row.get(0)?;
    let workspace: String = row.get(1)?;
    let observer_peer_id: i64 = row.get(2)?;
    let observed_peer_id: i64 = row.get(3)?;
    let chat_id: Option<String> = row.get(4)?;
    let level_str: String = row.get(5)?;
    let content: String = row.get(6)?;
    let category: Option<String> = row.get(7)?;
    let confidence: f64 = row.get(8)?;
    let source: Option<String> = row.get(9)?;
    let source_ids_str: String = row.get(10)?;
    let message_ids_str: String = row.get(11)?;
    let times_derived: i64 = row.get(12)?;
    let is_archived_int: i64 = row.get(13)?;
    let archived_at_str: Option<String> = row.get(14)?;
    let created_at_str: String = row.get(15)?;
    let updated_at_str: String = row.get(16)?;

    let level = ObservationLevel::parse(&level_str)
        .unwrap_or(ObservationLevel::Explicit);

    let source_ids: Vec<i64> = serde_json::from_str(&source_ids_str).unwrap_or_default();
    let message_ids: Vec<i64> = serde_json::from_str(&message_ids_str).unwrap_or_default();

    let is_archived = is_archived_int != 0;

    let archived_at = archived_at_str
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    let updated_at = updated_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(Observation {
        id,
        workspace,
        observer_peer_id,
        observed_peer_id,
        chat_id,
        level,
        content,
        category,
        confidence,
        source,
        source_ids,
        message_ids,
        times_derived,
        is_archived,
        archived_at,
        created_at,
        updated_at,
    })
}

fn row_to_queue_item(row: &Row<'_>) -> rusqlite::Result<QueueItem> {
    let id: i64 = row.get(0)?;
    let task_type: String = row.get(1)?;
    let workspace: String = row.get(2)?;
    let chat_id: Option<String> = row.get(3)?;
    let observer_peer_id: i64 = row.get(4)?;
    let observed_peer_id: i64 = row.get(5)?;
    let payload_str: String = row.get(6)?;
    let created_at_str: String = row.get(7)?;

    let payload: serde_json::Value = serde_json::from_str(&payload_str)
        .unwrap_or(serde_json::Value::Null);

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(QueueItem {
        id,
        task_type,
        workspace,
        chat_id,
        observer_peer_id,
        observed_peer_id,
        payload,
        created_at,
    })
}

fn row_to_finding(row: &Row<'_>) -> rusqlite::Result<Finding> {
    let id: i64 = row.get(0)?;
    let orchestration_id: String = row.get(1)?;
    let run_id: String = row.get(2)?;
    let finding: String = row.get(3)?;
    let category: Option<String> = row.get(4)?;
    let created_at_str: String = row.get(5)?;

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(Finding {
        id,
        orchestration_id,
        run_id,
        finding,
        category,
        created_at,
    })
}

fn row_to_deriver_run(row: &Row<'_>) -> rusqlite::Result<DeriverRun> {
    let id: i64 = row.get(0)?;
    let orchestration_id: String = row.get(1)?;
    let workspace: String = row.get(2)?;
    let observer_peer_id: i64 = row.get(3)?;
    let observed_peer_id: i64 = row.get(4)?;
    let chat_id: Option<String> = row.get(5)?;
    let observations_in: i64 = row.get(6)?;
    let observations_out: i64 = row.get(7)?;
    let duration_ms: i64 = row.get(8)?;
    let created_at_str: String = row.get(9)?;

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

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

fn row_to_dreamer_run(row: &Row<'_>) -> rusqlite::Result<DreamerRun> {
    let id: i64 = row.get(0)?;
    let orchestration_id: String = row.get(1)?;
    let workspace: String = row.get(2)?;
    let observer_peer_id: i64 = row.get(3)?;
    let observed_peer_id: i64 = row.get(4)?;
    let observations_in: i64 = row.get(5)?;
    let findings_out: i64 = row.get(6)?;
    let duration_ms: i64 = row.get(7)?;
    let created_at_str: String = row.get(8)?;

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

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

fn row_to_injection_log(row: &Row<'_>) -> rusqlite::Result<InjectionLog> {
    let id: i64 = row.get(0)?;
    let orchestration_id: String = row.get(1)?;
    let workspace: String = row.get(2)?;
    let chat_id: String = row.get(3)?;
    let observer_peer_id: i64 = row.get(4)?;
    let observed_peer_id: i64 = row.get(5)?;
    let observations_injected: i64 = row.get(6)?;
    let token_estimate: i64 = row.get(7)?;
    let created_at_str: String = row.get(8)?;

    let created_at = created_at_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

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
impl ObservationStore for SqliteDriver {
    // --- Peers ---

    async fn upsert_peer(
        &self,
        workspace: &str,
        name: &str,
        kind: PeerKind,
        metadata: Option<serde_json::Value>,
    ) -> Result<Peer> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let metadata_str = serde_json::to_string(&metadata.unwrap_or(serde_json::Value::Null))
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        conn.execute(
            "INSERT INTO peers (workspace, name, kind, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(workspace, name) DO UPDATE SET
                kind = excluded.kind,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at",
            params![workspace, name, kind.as_str(), metadata_str, now],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let peer = conn.query_row(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE workspace = ?1 AND name = ?2",
            params![workspace, name],
            row_to_peer,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(peer)
    }

    async fn get_peer_by_name(&self, workspace: &str, name: &str) -> Result<Option<Peer>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE workspace = ?1 AND name = ?2",
            params![workspace, name],
            row_to_peer,
        );

        match result {
            Ok(peer) => Ok(Some(peer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MemoryError::Database(e.to_string())),
        }
    }

    async fn get_peer_by_id(&self, id: i64) -> Result<Option<Peer>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE id = ?1",
            params![id],
            row_to_peer,
        );

        match result {
            Ok(peer) => Ok(Some(peer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MemoryError::Database(e.to_string())),
        }
    }

    async fn list_peers(&self, workspace: &str) -> Result<Vec<Peer>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
             FROM peers WHERE workspace = ?1 ORDER BY id ASC",
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let peers: Vec<Peer> = stmt
            .query_map(params![workspace], row_to_peer)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(peers)
    }

    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<PeerCard> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let facts_capped: Vec<String> = facts.into_iter().take(PeerCard::MAX_FACTS).collect();
        let card_json = serde_json::to_string(&facts_capped)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE peers SET peer_card = ?1, updated_at = ?2 WHERE id = ?3",
            params![card_json, now, peer_id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let peer_name: String = conn.query_row(
            "SELECT name FROM peers WHERE id = ?1",
            params![peer_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => MemoryError::NotFound(format!("peer {peer_id}")),
            e => MemoryError::Database(e.to_string()),
        })?;

        Ok(PeerCard {
            peer_id,
            peer_name,
            facts: facts_capped,
        })
    }

    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let result: rusqlite::Result<(String, Option<String>)> = conn.query_row(
            "SELECT name, peer_card FROM peers WHERE id = ?1",
            params![peer_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok((peer_name, Some(card_json))) => {
                let facts: Vec<String> = serde_json::from_str(&card_json)
                    .map_err(|e| MemoryError::Serialization(e.to_string()))?;
                Ok(Some(PeerCard { peer_id, peer_name, facts }))
            }
            Ok((_, None)) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MemoryError::Database(e.to_string())),
        }
    }

    // --- Observations CRUD ---

    async fn create_observation(&self, new_obs: NewObservation) -> Result<Observation> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        let source_ids_str = serde_json::to_string(&new_obs.source_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let message_ids_str = serde_json::to_string(&new_obs.message_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        conn.execute(
            "INSERT INTO observations
             (workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, 0, ?12, ?12)",
            params![
                new_obs.workspace,
                new_obs.observer_peer_id,
                new_obs.observed_peer_id,
                new_obs.chat_id,
                new_obs.level.as_str(),
                new_obs.content,
                new_obs.category,
                new_obs.confidence,
                new_obs.source,
                source_ids_str,
                message_ids_str,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        let obs = conn.query_row(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id = ?1",
            params![id],
            row_to_observation,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(obs)
    }

    async fn get_observation(&self, id: i64) -> Result<Option<Observation>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id = ?1",
            params![id],
            row_to_observation,
        );

        match result {
            Ok(obs) => Ok(Some(obs)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(MemoryError::Database(e.to_string())),
        }
    }

    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<Observation> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        // Fetch current to apply partial updates
        let current = conn.query_row(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id = ?1",
            params![id],
            row_to_observation,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => MemoryError::NotFound(format!("observation {id}")),
            e => MemoryError::Database(e.to_string()),
        })?;

        let level = update.level.unwrap_or(current.level);
        let content = update.content.unwrap_or(current.content);
        let category = update.category.or(current.category);
        let confidence = update.confidence.unwrap_or(current.confidence);
        let source = update.source.or(current.source);
        let source_ids = update.source_ids.unwrap_or(current.source_ids);
        let message_ids = update.message_ids.unwrap_or(current.message_ids);
        let is_archived = update.is_archived.unwrap_or(current.is_archived);
        let archived_at = update.archived_at.or(current.archived_at);

        let source_ids_str = serde_json::to_string(&source_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let message_ids_str = serde_json::to_string(&message_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        let archived_at_str = archived_at.map(|dt| dt.to_rfc3339());

        conn.execute(
            "UPDATE observations SET
               level = ?1, content = ?2, category = ?3, confidence = ?4, source = ?5,
               source_ids = ?6, message_ids = ?7, is_archived = ?8, archived_at = ?9,
               updated_at = ?10
             WHERE id = ?11",
            params![
                level.as_str(),
                content,
                category,
                confidence,
                source,
                source_ids_str,
                message_ids_str,
                is_archived as i64,
                archived_at_str,
                now,
                id,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let obs = conn.query_row(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id = ?1",
            params![id],
            row_to_observation,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(obs)
    }

    async fn delete_observation(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        conn.execute("DELETE FROM observations WHERE id = ?1", params![id])
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn archive_observation(&self, id: i64) -> Result<Observation> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        let rows_affected = conn.execute(
            "UPDATE observations SET is_archived = 1, archived_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        if rows_affected == 0 {
            return Err(MemoryError::NotFound(format!("observation {id}")));
        }

        let obs = conn.query_row(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id = ?1",
            params![id],
            row_to_observation,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(obs)
    }

    async fn list_observations(
        &self,
        scope: SearchScope,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Observation>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut conditions: Vec<String> = vec!["workspace = ?1".to_string()];
        let mut idx = 2usize;

        let observer_filter = scope.observer_peer_id;
        let observed_filter = scope.observed_peer_id;
        let chat_filter = scope.chat_id.clone();
        let confidence_filter = scope.min_confidence;

        if observer_filter.is_some() {
            conditions.push(format!("observer_peer_id = ?{idx}"));
            idx += 1;
        }
        if observed_filter.is_some() {
            conditions.push(format!("observed_peer_id = ?{idx}"));
            idx += 1;
        }
        if chat_filter.is_some() {
            conditions.push(format!("chat_id = ?{idx}"));
            idx += 1;
        }
        if confidence_filter.is_some() {
            conditions.push(format!("confidence >= ?{idx}"));
            idx += 1;
        }
        if !scope.include_archived {
            conditions.push("is_archived = 0".to_string());
        }

        let limit_idx = idx;
        let offset_idx = idx + 1;

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE {where_clause}
             ORDER BY created_at DESC LIMIT ?{limit_idx} OFFSET ?{offset_idx}"
        );

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(scope.workspace.clone()));
        if let Some(v) = observer_filter { param_values.push(Box::new(v)); }
        if let Some(v) = observed_filter { param_values.push(Box::new(v)); }
        if let Some(v) = chat_filter { param_values.push(Box::new(v)); }
        if let Some(v) = confidence_filter { param_values.push(Box::new(v)); }
        param_values.push(Box::new(limit));
        param_values.push(Box::new(offset));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();

        let observations: Vec<Observation> = stmt
            .query_map(params_refs.as_slice(), row_to_observation)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(observations)
    }

    // --- Search ---

    async fn keyword_search(
        &self,
        scope: &SearchScope,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Observation>> {
        use crate::search::sanitize_fts_query;

        let sanitized = match sanitize_fts_query(query) {
            Some(q) => q,
            None => return Ok(vec![]),
        };

        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        // Build dynamic scope filters on top of the FTS join
        let mut conditions: Vec<String> = vec!["o.workspace = ?2".to_string()];
        let mut idx = 3usize;

        if scope.observer_peer_id.is_some() {
            conditions.push(format!("o.observer_peer_id = ?{idx}"));
            idx += 1;
        }
        if scope.observed_peer_id.is_some() {
            conditions.push(format!("o.observed_peer_id = ?{idx}"));
            idx += 1;
        }
        if scope.chat_id.is_some() {
            conditions.push(format!("o.chat_id = ?{idx}"));
            idx += 1;
        }
        if scope.min_confidence.is_some() {
            conditions.push(format!("o.confidence >= ?{idx}"));
            idx += 1;
        }
        if !scope.include_archived {
            conditions.push("o.is_archived = 0".to_string());
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
             FROM observations_fts fts
             JOIN observations o ON o.id = fts.rowid
             WHERE observations_fts MATCH ?1
             {extra}
             ORDER BY rank
             LIMIT ?{limit_idx}"
        );

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(sanitized));
        param_values.push(Box::new(scope.workspace.clone()));
        if let Some(v) = scope.observer_peer_id { param_values.push(Box::new(v)); }
        if let Some(v) = scope.observed_peer_id { param_values.push(Box::new(v)); }
        if let Some(v) = scope.chat_id.clone() { param_values.push(Box::new(v)); }
        if let Some(v) = scope.min_confidence { param_values.push(Box::new(v)); }
        param_values.push(Box::new(limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();

        let observations: Vec<Observation> = stmt
            .query_map(params_refs.as_slice(), row_to_observation)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(observations)
    }

    async fn semantic_search(
        &self,
        _scope: &SearchScope,
        _embedding: Vec<f32>,
        _limit: i64,
    ) -> Result<Vec<Observation>> {
        // Semantic search requires vector index; not implemented yet.
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

        // Semantic arm is empty for now; pass empty slice.
        let semantic_scored: Vec<(i64, f64)> = vec![];

        let ranked = rrf_merge(&keyword_scored, &semantic_scored, limit as usize);

        Ok(ranked)
    }

    // --- DAG ---

    async fn link_observations(&self, parent_id: i64, child_id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        // Fetch child's current source_ids
        let source_ids_str: String = conn.query_row(
            "SELECT source_ids FROM observations WHERE id = ?1",
            params![child_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => MemoryError::NotFound(format!("observation {child_id}")),
            e => MemoryError::Database(e.to_string()),
        })?;

        let mut source_ids: Vec<i64> = serde_json::from_str(&source_ids_str).unwrap_or_default();
        if !source_ids.contains(&parent_id) {
            source_ids.push(parent_id);
        }

        let new_source_ids_str = serde_json::to_string(&source_ids)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        conn.execute(
            "UPDATE observations SET source_ids = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_source_ids_str, now, child_id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn unlink_observations(&self, parent_id: i64, child_id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        let source_ids_str: String = conn.query_row(
            "SELECT source_ids FROM observations WHERE id = ?1",
            params![child_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => MemoryError::NotFound(format!("observation {child_id}")),
            e => MemoryError::Database(e.to_string()),
        })?;

        let source_ids: Vec<i64> = serde_json::from_str(&source_ids_str).unwrap_or_default();
        let filtered: Vec<i64> = source_ids.into_iter().filter(|&id| id != parent_id).collect();

        let new_source_ids_str = serde_json::to_string(&filtered)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        conn.execute(
            "UPDATE observations SET source_ids = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_source_ids_str, now, child_id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_children(&self, observation_id: i64) -> Result<Vec<Observation>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        // Find observations whose source_ids JSON array contains observation_id.
        // Use json_each to check membership.
        let sql = "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations
             WHERE EXISTS (
                 SELECT 1 FROM json_each(source_ids) WHERE value = ?1
             )
             ORDER BY id ASC";

        let mut stmt = conn.prepare(sql)
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        let observations: Vec<Observation> = stmt
            .query_map(params![observation_id], row_to_observation)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(observations)
    }

    async fn get_parents(&self, observation_id: i64) -> Result<Vec<Observation>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        // Get the observation's source_ids, then fetch all those observations.
        let source_ids_str: String = match conn.query_row(
            "SELECT source_ids FROM observations WHERE id = ?1",
            params![observation_id],
            |row| row.get(0),
        ) {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(vec![]),
            Err(e) => return Err(MemoryError::Database(e.to_string())),
        };

        let source_ids: Vec<i64> = serde_json::from_str(&source_ids_str).unwrap_or_default();

        if source_ids.is_empty() {
            return Ok(vec![]);
        }

        // Build parameterized IN clause
        let placeholders: String = source_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id, level, content,
              category, confidence, source, source_ids, message_ids, times_derived,
              is_archived, archived_at, created_at, updated_at
             FROM observations WHERE id IN ({placeholders}) ORDER BY id ASC"
        );

        let mut stmt = conn.prepare(&sql)
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        let param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
            source_ids.iter().map(|&id| Box::new(id) as Box<dyn rusqlite::types::ToSql>).collect();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|b| b.as_ref()).collect();

        let observations: Vec<Observation> = stmt
            .query_map(params_refs.as_slice(), row_to_observation)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(observations)
    }

    // --- Queue ---

    async fn enqueue(&self, task: QueueTask) -> Result<QueueItem> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        let payload_str = serde_json::to_string(&task.payload)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;

        conn.execute(
            "INSERT INTO observation_queue
             (task_type, workspace, chat_id, observer_peer_id, observed_peer_id,
              payload, processed, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                task.task_type,
                task.workspace,
                task.chat_id,
                task.observer_peer_id,
                task.observed_peer_id,
                payload_str,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        let item = conn.query_row(
            "SELECT id, task_type, workspace, chat_id, observer_peer_id, observed_peer_id,
              payload, created_at
             FROM observation_queue WHERE id = ?1",
            params![id],
            row_to_queue_item,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(item)
    }

    async fn dequeue(&self, limit: i64) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, task_type, workspace, chat_id, observer_peer_id, observed_peer_id,
              payload, created_at
             FROM observation_queue
             WHERE processed = 0
             ORDER BY id ASC
             LIMIT ?1",
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let items: Vec<QueueItem> = stmt
            .query_map(params![limit], row_to_queue_item)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    async fn ack_queue_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE observation_queue SET processed = 1, processed_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn nack_queue_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        // Re-enqueue: clear processed flag and processed_at so item will be picked up again.
        conn.execute(
            "UPDATE observation_queue SET processed = 0, processed_at = NULL WHERE id = ?1",
            params![id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    // --- Findings ---

    async fn save_finding(&self, finding: Finding) -> Result<Finding> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO findings (orchestration_id, run_id, finding, category, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                finding.orchestration_id,
                finding.run_id,
                finding.finding,
                finding.category,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        let saved = conn.query_row(
            "SELECT id, orchestration_id, run_id, finding, category, created_at
             FROM findings WHERE id = ?1",
            params![id],
            row_to_finding,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(saved)
    }

    async fn list_findings(
        &self,
        orchestration_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Finding>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, orchestration_id, run_id, finding, category, created_at
             FROM findings
             WHERE orchestration_id = ?1
             ORDER BY id ASC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let findings: Vec<Finding> = stmt
            .query_map(params![orchestration_id, limit, offset], row_to_finding)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(findings)
    }

    // --- Embedding ---

    async fn store_embedding(&self, _observation_id: i64, _embedding: Vec<f32>) -> Result<()> {
        // Vector storage not yet wired up; no-op placeholder.
        Ok(())
    }

    async fn get_embedding(&self, _observation_id: i64) -> Result<Option<Vec<f32>>> {
        Ok(None)
    }

    // --- Observability ---

    async fn save_deriver_run(&self, run: DeriverRun) -> Result<DeriverRun> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO deriver_runs
             (orchestration_id, workspace, observer_peer_id, observed_peer_id, chat_id,
              observations_in, observations_out, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.orchestration_id,
                run.workspace,
                run.observer_peer_id,
                run.observed_peer_id,
                run.chat_id,
                run.observations_in,
                run.observations_out,
                run.duration_ms,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        let saved = conn.query_row(
            "SELECT id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
              chat_id, observations_in, observations_out, duration_ms, created_at
             FROM deriver_runs WHERE id = ?1",
            params![id],
            row_to_deriver_run,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(saved)
    }

    async fn save_dreamer_run(&self, run: DreamerRun) -> Result<DreamerRun> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO dreamer_runs
             (orchestration_id, workspace, observer_peer_id, observed_peer_id,
              observations_in, findings_out, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.orchestration_id,
                run.workspace,
                run.observer_peer_id,
                run.observed_peer_id,
                run.observations_in,
                run.findings_out,
                run.duration_ms,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        let saved = conn.query_row(
            "SELECT id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
              observations_in, findings_out, duration_ms, created_at
             FROM dreamer_runs WHERE id = ?1",
            params![id],
            row_to_dreamer_run,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(saved)
    }

    async fn save_injection_log(&self, log: InjectionLog) -> Result<InjectionLog> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO injection_logs
             (orchestration_id, workspace, chat_id, observer_peer_id, observed_peer_id,
              observations_injected, token_estimate, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                log.orchestration_id,
                log.workspace,
                log.chat_id,
                log.observer_peer_id,
                log.observed_peer_id,
                log.observations_injected,
                log.token_estimate,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        let saved = conn.query_row(
            "SELECT id, orchestration_id, workspace, chat_id, observer_peer_id,
              observed_peer_id, observations_injected, token_estimate, created_at
             FROM injection_logs WHERE id = ?1",
            params![id],
            row_to_injection_log,
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(saved)
    }

    async fn list_deriver_runs(
        &self,
        workspace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DeriverRun>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
              chat_id, observations_in, observations_out, duration_ms, created_at
             FROM deriver_runs
             WHERE workspace = ?1
             ORDER BY id DESC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let runs: Vec<DeriverRun> = stmt
            .query_map(params![workspace, limit, offset], row_to_deriver_run)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(runs)
    }

    async fn list_dreamer_runs(
        &self,
        workspace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DreamerRun>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, orchestration_id, workspace, observer_peer_id, observed_peer_id,
              observations_in, findings_out, duration_ms, created_at
             FROM dreamer_runs
             WHERE workspace = ?1
             ORDER BY id DESC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let runs: Vec<DreamerRun> = stmt
            .query_map(params![workspace, limit, offset], row_to_dreamer_run)
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(runs)
    }

    async fn list_injection_logs(
        &self,
        workspace: &str,
        chat_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InjectionLog>> {
        let conn = self.conn.lock().map_err(|e| MemoryError::Database(e.to_string()))?;

        let sql;
        let logs: Vec<InjectionLog>;

        if let Some(cid) = chat_id {
            let mut stmt = conn.prepare(
                "SELECT id, orchestration_id, workspace, chat_id, observer_peer_id,
                  observed_peer_id, observations_injected, token_estimate, created_at
                 FROM injection_logs
                 WHERE workspace = ?1 AND chat_id = ?2
                 ORDER BY id DESC
                 LIMIT ?3 OFFSET ?4",
            )
            .map_err(|e| MemoryError::Database(e.to_string()))?;

            logs = stmt
                .query_map(params![workspace, cid, limit, offset], row_to_injection_log)
                .map_err(|e| MemoryError::Database(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();
        } else {
            sql = "SELECT id, orchestration_id, workspace, chat_id, observer_peer_id,
                  observed_peer_id, observations_injected, token_estimate, created_at
                 FROM injection_logs
                 WHERE workspace = ?1
                 ORDER BY id DESC
                 LIMIT ?2 OFFSET ?3"
                .to_string();

            let mut stmt = conn.prepare(&sql)
                .map_err(|e| MemoryError::Database(e.to_string()))?;

            logs = stmt
                .query_map(params![workspace, limit, offset], row_to_injection_log)
                .map_err(|e| MemoryError::Database(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();
        }

        Ok(logs)
    }
}

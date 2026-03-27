pub mod dag;
pub mod deriver;
pub mod dreamer;
pub mod driver;
pub mod injection;
pub mod migration;
pub mod quality;
pub mod queue;
pub mod search;
pub mod types;

use async_trait::async_trait;
use thiserror::Error;

use crate::types::{
    DeriverRun, DreamerRun, Finding, InjectionLog, NewObservation, Observation, ObservationUpdate,
    Peer, PeerCard, PeerKind, QueueItem, QueueTask, RankedResult, SearchScope,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("database error: {0}")]
    Database(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("embedding error: {0}")]
    Embedding(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

// ---------------------------------------------------------------------------
// ObservationStore trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ObservationStore: Send + Sync {
    // --- Peers ---

    async fn upsert_peer(
        &self,
        workspace: &str,
        name: &str,
        kind: PeerKind,
        metadata: Option<serde_json::Value>,
    ) -> Result<Peer>;

    async fn get_peer_by_name(&self, workspace: &str, name: &str) -> Result<Option<Peer>>;

    async fn get_peer_by_id(&self, id: i64) -> Result<Option<Peer>>;

    async fn list_peers(&self, workspace: &str) -> Result<Vec<Peer>>;

    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<PeerCard>;

    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>>;

    // --- Observations CRUD ---

    async fn create_observation(&self, new_obs: NewObservation) -> Result<Observation>;

    async fn get_observation(&self, id: i64) -> Result<Option<Observation>>;

    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<Observation>;

    async fn delete_observation(&self, id: i64) -> Result<()>;

    async fn archive_observation(&self, id: i64) -> Result<Observation>;

    async fn list_observations(&self, scope: SearchScope, limit: i64, offset: i64)
        -> Result<Vec<Observation>>;

    // --- Search ---

    async fn keyword_search(
        &self,
        scope: &SearchScope,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Observation>>;

    async fn semantic_search(
        &self,
        scope: &SearchScope,
        embedding: Vec<f32>,
        limit: i64,
    ) -> Result<Vec<Observation>>;

    async fn hybrid_search(
        &self,
        scope: &SearchScope,
        query: &str,
        embedding: Vec<f32>,
        limit: i64,
    ) -> Result<Vec<RankedResult>>;

    // --- DAG ---

    async fn link_observations(&self, parent_id: i64, child_id: i64) -> Result<()>;

    async fn unlink_observations(&self, parent_id: i64, child_id: i64) -> Result<()>;

    async fn get_children(&self, observation_id: i64) -> Result<Vec<Observation>>;

    async fn get_parents(&self, observation_id: i64) -> Result<Vec<Observation>>;

    // --- Queue ---

    async fn enqueue(&self, task: QueueTask) -> Result<QueueItem>;

    async fn dequeue(&self, limit: i64) -> Result<Vec<QueueItem>>;

    async fn ack_queue_item(&self, id: i64) -> Result<()>;

    async fn nack_queue_item(&self, id: i64) -> Result<()>;

    // --- Findings ---

    async fn save_finding(&self, finding: Finding) -> Result<Finding>;

    async fn list_findings(
        &self,
        orchestration_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Finding>>;

    // --- Embedding ---

    async fn store_embedding(&self, observation_id: i64, embedding: Vec<f32>) -> Result<()>;

    async fn get_embedding(&self, observation_id: i64) -> Result<Option<Vec<f32>>>;

    // --- Observability ---

    async fn save_deriver_run(&self, run: DeriverRun) -> Result<DeriverRun>;

    async fn save_dreamer_run(&self, run: DreamerRun) -> Result<DreamerRun>;

    async fn save_injection_log(&self, log: InjectionLog) -> Result<InjectionLog>;

    async fn list_deriver_runs(
        &self,
        workspace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DeriverRun>>;

    async fn list_dreamer_runs(
        &self,
        workspace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DreamerRun>>;

    async fn list_injection_logs(
        &self,
        workspace: &str,
        chat_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InjectionLog>>;
}

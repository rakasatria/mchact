use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ObservationLevel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationLevel {
    Explicit,
    Deductive,
    Inductive,
    Contradiction,
}

impl ObservationLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ObservationLevel::Explicit => "explicit",
            ObservationLevel::Deductive => "deductive",
            ObservationLevel::Inductive => "inductive",
            ObservationLevel::Contradiction => "contradiction",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "explicit" => Some(ObservationLevel::Explicit),
            "deductive" => Some(ObservationLevel::Deductive),
            "inductive" => Some(ObservationLevel::Inductive),
            "contradiction" => Some(ObservationLevel::Contradiction),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// PeerKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerKind {
    User,
    Agent,
}

impl PeerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PeerKind::User => "user",
            PeerKind::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(PeerKind::User),
            "agent" => Some(PeerKind::Agent),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Peer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: i64,
    pub workspace: String,
    pub name: String,
    pub kind: PeerKind,
    pub peer_card: Option<Vec<String>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// PeerCard
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCard {
    pub peer_id: i64,
    pub peer_name: String,
    pub facts: Vec<String>,
}

impl PeerCard {
    pub const MAX_FACTS: usize = 40;
}

// ---------------------------------------------------------------------------
// Observation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: i64,
    pub workspace: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub chat_id: Option<String>,
    pub level: ObservationLevel,
    pub content: String,
    pub category: Option<String>,
    pub confidence: f64,
    pub source: Option<String>,
    pub source_ids: Vec<i64>,
    pub message_ids: Vec<i64>,
    pub times_derived: i64,
    pub is_archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// NewObservation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewObservation {
    pub workspace: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub chat_id: Option<String>,
    pub level: ObservationLevel,
    pub content: String,
    pub category: Option<String>,
    pub confidence: f64,
    pub source: Option<String>,
    pub source_ids: Vec<i64>,
    pub message_ids: Vec<i64>,
}

// ---------------------------------------------------------------------------
// ObservationUpdate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationUpdate {
    pub level: Option<ObservationLevel>,
    pub content: Option<String>,
    pub category: Option<String>,
    pub confidence: Option<f64>,
    pub source: Option<String>,
    pub source_ids: Option<Vec<i64>>,
    pub message_ids: Option<Vec<i64>>,
    pub is_archived: Option<bool>,
    pub archived_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// SearchScope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchScope {
    pub workspace: String,
    pub observer_peer_id: Option<i64>,
    pub observed_peer_id: Option<i64>,
    pub chat_id: Option<String>,
    pub min_confidence: Option<f64>,
    pub include_archived: bool,
}

// ---------------------------------------------------------------------------
// QueueTask / QueueItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub task_type: String,
    pub workspace: String,
    pub chat_id: Option<String>,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i64,
    pub task_type: String,
    pub workspace: String,
    pub chat_id: Option<String>,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Finding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: i64,
    pub orchestration_id: String,
    pub run_id: String,
    pub finding: String,
    pub category: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Observability structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriverRun {
    pub id: i64,
    pub orchestration_id: String,
    pub workspace: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub chat_id: Option<String>,
    pub observations_in: i64,
    pub observations_out: i64,
    pub duration_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamerRun {
    pub id: i64,
    pub orchestration_id: String,
    pub workspace: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub observations_in: i64,
    pub findings_out: i64,
    pub duration_ms: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionLog {
    pub id: i64,
    pub orchestration_id: String,
    pub workspace: String,
    pub chat_id: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub observations_injected: i64,
    pub token_estimate: i64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// RankedResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedResult {
    pub observation_id: i64,
    pub keyword_rank: Option<i64>,
    pub semantic_rank: Option<i64>,
    pub rrf_score: f64,
}

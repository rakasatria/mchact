# mchact-memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Honcho-inspired observation engine as a Rust crate with dual SQLite/PostgreSQL drivers, replacing mchact's flat memory system.

**Architecture:** New `crates/mchact-memory/` crate exposes an `ObservationStore` async trait. Two feature-gated drivers (`sqlite`, `postgres`) implement the full model: 4-level observations, peers, peer cards, hybrid RRF search, DAG traversal, background task queue, and MoA findings. The crate integrates into mchact's runtime by replacing `MemoryBackend` / `MemoryProvider`.

**Tech Stack:** Rust 2021, async-trait, serde/serde_json, rusqlite (bundled + sqlite-vec + FTS5), sqlx (PostgreSQL + pgvector), chrono, tokio, thiserror, anyhow.

**Spec:** `docs/superpowers/specs/2026-03-27-mchact-memory-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/mchact-memory/Cargo.toml` | Crate manifest with `sqlite` / `postgres` feature flags |
| `crates/mchact-memory/src/lib.rs` | `ObservationStore` trait + crate re-exports |
| `crates/mchact-memory/src/types.rs` | All domain types: `Observation`, `Peer`, `PeerCard`, `ObservationLevel`, `NewObservation`, `ObservationUpdate`, `SearchScope`, `QueueTask`, `QueueItem`, `Finding`, `DeriverRun`, `DreamerRun`, `InjectionLog`, `PeerKind` |
| `crates/mchact-memory/src/quality.rs` | Quality gates, normalization, PII scan |
| `crates/mchact-memory/src/search.rs` | RRF hybrid merge (shared Rust logic, driver-agnostic) |
| `crates/mchact-memory/src/dag.rs` | Source attribution DAG traversal |
| `crates/mchact-memory/src/injection.rs` | Build memory context for prompts |
| `crates/mchact-memory/src/queue.rs` | Background task queue processing loop |
| `crates/mchact-memory/src/deriver.rs` | Multi-level observation extraction agent |
| `crates/mchact-memory/src/dreamer.rs` | Offline consolidation + induction agent |
| `crates/mchact-memory/src/migration.rs` | Legacy `memories` → `observations` migration |
| `crates/mchact-memory/src/driver/mod.rs` | Driver selection from config |
| `crates/mchact-memory/src/driver/sqlite.rs` | `SqliteDriver` implementing `ObservationStore` |
| `crates/mchact-memory/src/driver/postgres.rs` | `PgDriver` implementing `ObservationStore` |
| `crates/mchact-memory/src/schema/sqlite.sql` | Full SQLite DDL |
| `crates/mchact-memory/src/schema/postgres.sql` | Full PostgreSQL DDL |
| `crates/mchact-memory/tests/sqlite_integration.rs` | SQLite driver integration tests |
| `crates/mchact-memory/tests/postgres_integration.rs` | PostgreSQL driver integration tests |

### Modified files

| File | What changes |
|------|-------------|
| `Cargo.toml` (workspace root) | Add `crates/mchact-memory` to `[workspace.members]`, add `mchact-memory` dep |
| `src/config.rs` | Add `MemoryConfig` struct, wire into `Config` |
| `src/runtime.rs` | Initialize `ObservationStore`, replace `MemoryBackend` in `AppState` |
| `src/agent_engine.rs` | Use `injection::build_memory_context()` for prompt injection |
| `src/scheduler.rs` | Replace reflector loop with deriver + dreamer loops |
| `src/memory_service.rs` | Explicit remember fast path uses `ObservationStore` |
| `src/tools/structured_memory.rs` | Query `ObservationStore` instead of `MemoryProvider` |
| `src/tools/findings.rs` | Use `ObservationStore` findings methods |

---

## Task 1: Crate Scaffold + Types

**Files:**
- Create: `crates/mchact-memory/Cargo.toml`
- Create: `crates/mchact-memory/src/lib.rs`
- Create: `crates/mchact-memory/src/types.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml for the crate**

```toml
# crates/mchact-memory/Cargo.toml
[package]
name = "mchact-memory"
version = "0.1.0"
edition = "2021"
license = "MIT"

[features]
default = ["sqlite"]
sqlite = ["dep:rusqlite", "dep:sqlite-vec"]
postgres = ["dep:sqlx"]

[dependencies]
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

# SQLite driver (feature-gated)
rusqlite = { version = "0.37", features = ["bundled"], optional = true }
sqlite-vec = { version = "0.1.8-alpha.1", optional = true }

# PostgreSQL driver (feature-gated)
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "chrono", "json"], optional = true }

[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
```

- [ ] **Step 2: Create types.rs with all domain types**

```rust
// crates/mchact-memory/src/types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObservationLevel {
    Explicit,
    Deductive,
    Inductive,
    Contradiction,
}

impl ObservationLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Deductive => "deductive",
            Self::Inductive => "inductive",
            Self::Contradiction => "contradiction",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "explicit" => Some(Self::Explicit),
            "deductive" => Some(Self::Deductive),
            "inductive" => Some(Self::Inductive),
            "contradiction" => Some(Self::Contradiction),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerKind {
    User,
    Agent,
}

impl PeerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: i64,
    pub workspace: String,
    pub name: String,
    pub kind: PeerKind,
    pub peer_card: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCard {
    pub peer_id: i64,
    pub peer_name: String,
    pub facts: Vec<String>,
}

impl PeerCard {
    pub const MAX_FACTS: usize = 40;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: i64,
    pub workspace: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub chat_id: Option<i64>,
    pub level: ObservationLevel,
    pub content: String,
    pub category: Option<String>,
    pub confidence: f64,
    pub source: String,
    pub source_ids: Vec<i64>,
    pub message_ids: Vec<i64>,
    pub times_derived: i32,
    pub is_archived: bool,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewObservation {
    pub workspace: String,
    pub observer_peer_id: i64,
    pub observed_peer_id: i64,
    pub chat_id: Option<i64>,
    pub level: ObservationLevel,
    pub content: String,
    pub category: Option<String>,
    pub confidence: f64,
    pub source: String,
    pub source_ids: Vec<i64>,
    pub message_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct ObservationUpdate {
    pub content: Option<String>,
    pub category: Option<String>,
    pub confidence: Option<f64>,
    pub source_ids: Option<Vec<i64>>,
    pub times_derived: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct SearchScope {
    pub workspace: String,
    pub observer_peer_id: Option<i64>,
    pub observed_peer_id: Option<i64>,
    pub chat_id: Option<i64>,
    pub min_confidence: f64,
    pub include_archived: bool,
}

impl Default for SearchScope {
    fn default() -> Self {
        Self {
            workspace: "default".to_string(),
            observer_peer_id: None,
            observed_peer_id: None,
            chat_id: None,
            min_confidence: 0.45,
            include_archived: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub task_type: String,
    pub workspace: String,
    pub chat_id: Option<i64>,
    pub observer_peer_id: Option<i64>,
    pub observed_peer_id: Option<i64>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: i64,
    pub task_type: String,
    pub workspace: String,
    pub chat_id: Option<i64>,
    pub observer_peer_id: Option<i64>,
    pub observed_peer_id: Option<i64>,
    pub payload: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: i64,
    pub orchestration_id: String,
    pub run_id: String,
    pub finding: String,
    pub category: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DeriverRun {
    pub chat_id: Option<i64>,
    pub workspace: String,
    pub started_at: String,
    pub finished_at: String,
    pub messages_processed: i64,
    pub explicit_count: i64,
    pub deductive_count: i64,
    pub skipped_count: i64,
    pub error_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DreamerRun {
    pub workspace: String,
    pub observer_peer_id: Option<i64>,
    pub observed_peer_id: Option<i64>,
    pub deductions_created: i64,
    pub inductions_created: i64,
    pub contradictions_found: i64,
    pub consolidated: i64,
    pub peer_card_updated: i64,
    pub run_at: String,
}

#[derive(Debug, Clone)]
pub struct InjectionLog {
    pub chat_id: Option<i64>,
    pub retrieval_method: String,
    pub candidate_count: i64,
    pub selected_count: i64,
    pub omitted_count: i64,
    pub tokens_est: i64,
}

/// Ranked search result with RRF score for hybrid merge.
#[derive(Debug, Clone)]
pub struct RankedResult {
    pub observation_id: i64,
    pub keyword_rank: Option<f64>,
    pub semantic_rank: Option<f64>,
    pub rrf_score: f64,
}
```

- [ ] **Step 3: Create lib.rs with ObservationStore trait**

```rust
// crates/mchact-memory/src/lib.rs
pub mod types;
pub mod quality;
pub mod search;
pub mod dag;
pub mod injection;
pub mod queue;
pub mod deriver;
pub mod dreamer;
pub mod migration;
pub mod driver;

use async_trait::async_trait;
use types::*;

#[derive(Debug, thiserror::Error)]
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
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

#[async_trait]
pub trait ObservationStore: Send + Sync {
    // --- Core CRUD ---
    async fn store_observation(&self, obs: NewObservation) -> Result<i64>;
    async fn get_observation(&self, id: i64) -> Result<Option<Observation>>;
    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<()>;
    async fn archive_observation(&self, id: i64) -> Result<()>;

    // --- Search ---
    async fn search_hybrid(
        &self,
        query: &str,
        scope: SearchScope,
        limit: usize,
    ) -> Result<Vec<Observation>>;

    async fn get_for_context(
        &self,
        chat_id: i64,
        user_query: &str,
        token_budget: usize,
    ) -> Result<Vec<Observation>>;

    // --- Peers ---
    async fn get_or_create_peer(
        &self,
        workspace: &str,
        name: &str,
        kind: PeerKind,
    ) -> Result<Peer>;

    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>>;
    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<()>;

    // --- DAG ---
    async fn trace_reasoning(&self, observation_id: i64) -> Result<Vec<Observation>>;

    // --- Queue ---
    async fn enqueue(&self, task: QueueTask) -> Result<i64>;
    async fn dequeue(&self, task_type: &str, limit: usize) -> Result<Vec<QueueItem>>;
    async fn mark_processed(&self, queue_id: i64) -> Result<()>;

    // --- Findings ---
    async fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64>;
    async fn get_findings(&self, orchestration_id: &str) -> Result<Vec<Finding>>;
    async fn delete_findings(&self, orchestration_id: &str) -> Result<usize>;

    // --- Embedding ---
    async fn upsert_embedding(&self, observation_id: i64, embedding: &[f32]) -> Result<()>;

    // --- Observability ---
    async fn log_deriver_run(&self, run: DeriverRun) -> Result<()>;
    async fn log_dreamer_run(&self, run: DreamerRun) -> Result<()>;
    async fn log_injection(&self, log: InjectionLog) -> Result<()>;
}
```

- [ ] **Step 4: Add crate to workspace Cargo.toml**

In the workspace root `Cargo.toml`, add to the `[workspace] members` list:

```toml
members = [
    ".",
    "crates/mchact-core",
    "crates/mchact-clawhub",
    "crates/mchact-storage",
    "crates/mchact-tools",
    "crates/mchact-channels",
    "crates/mchact-app",
    "crates/mchact-observability",
    "crates/mchact-memory",
]
```

And add the dependency to the root `[dependencies]`:

```toml
mchact-memory = { version = "0.1.0", path = "crates/mchact-memory" }
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p mchact-memory`
Expected: Compiles with warnings about unused modules (they're empty stubs).

- [ ] **Step 6: Commit**

```bash
git add crates/mchact-memory/ Cargo.toml
git commit -m "feat(mchact-memory): scaffold crate with types and ObservationStore trait"
```

---

## Task 2: Quality Gates

**Files:**
- Create: `crates/mchact-memory/src/quality.rs`

- [ ] **Step 1: Write quality gate tests**

```rust
// crates/mchact-memory/src/quality.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_trims_and_collapses() {
        assert_eq!(
            normalize_content("  hello   world  ", 180),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn test_normalize_truncates() {
        assert_eq!(
            normalize_content("abcdefghij", 5),
            Some("abcde".to_string())
        );
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize_content("   ", 180), None);
    }

    #[test]
    fn test_quality_ok_valid() {
        assert!(quality_check("user prefers Rust for backend work").is_ok());
    }

    #[test]
    fn test_quality_rejects_short() {
        assert_eq!(quality_check("hi").unwrap_err(), "too short");
    }

    #[test]
    fn test_quality_rejects_small_talk() {
        assert_eq!(quality_check("thanks").unwrap_err(), "small talk");
    }

    #[test]
    fn test_quality_rejects_uncertain() {
        assert_eq!(
            quality_check("maybe they like Python").unwrap_err(),
            "uncertain statement"
        );
    }

    #[test]
    fn test_quality_rejects_no_signal() {
        assert_eq!(quality_check("........").unwrap_err(), "no signal");
    }

    #[test]
    fn test_pii_detects_email() {
        assert_eq!(
            pii_check("email is alice@example.com").unwrap_err(),
            "contains PII (email)"
        );
    }

    #[test]
    fn test_pii_detects_api_key() {
        assert_eq!(
            pii_check("my key is sk-1234567890abcdef").unwrap_err(),
            "contains secret"
        );
    }

    #[test]
    fn test_pii_clean_content_passes() {
        assert!(pii_check("user prefers dark mode").is_ok());
    }

    #[test]
    fn test_poisoning_guard_rejects_broken_behavior() {
        assert!(!poisoning_check("tool calls were broken and auth fails"));
    }

    #[test]
    fn test_poisoning_guard_allows_corrective() {
        assert!(poisoning_check("TODO: ensure auth tokens are refreshed"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mchact-memory quality`
Expected: FAIL — functions not defined yet.

- [ ] **Step 3: Implement quality.rs**

```rust
// crates/mchact-memory/src/quality.rs

/// Normalize observation content: collapse whitespace, trim, truncate.
pub fn normalize_content(input: &str, max_chars: usize) -> Option<String> {
    let cleaned = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let content = cleaned.trim().to_string();
    if content.is_empty() {
        return None;
    }
    if content.len() <= max_chars {
        return Some(content);
    }
    let cutoff = content
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_chars)
        .last()
        .unwrap_or(max_chars);
    Some(content[..cutoff].to_string())
}

/// Check content quality. Returns Ok(()) or Err with reason.
pub fn quality_check(content: &str) -> std::result::Result<(), &'static str> {
    let lower = content.to_ascii_lowercase();
    let trimmed = lower.trim();

    if trimmed.len() < 8 {
        return Err("too short");
    }

    let small_talk = [
        "hi", "hello", "thanks", "thank you", "ok", "okay", "lol", "haha",
    ];
    if small_talk.contains(&trimmed) {
        return Err("small talk");
    }

    if trimmed.contains("maybe")
        || trimmed.contains("i think")
        || trimmed.contains("not sure")
        || trimmed.contains("guess")
    {
        return Err("uncertain statement");
    }

    if !trimmed.chars().any(|c| c.is_alphanumeric()) {
        return Err("no signal");
    }

    Ok(())
}

/// Check for PII and secrets. Returns Ok(()) or Err with reason.
pub fn pii_check(content: &str) -> std::result::Result<(), &'static str> {
    // Email pattern: word@word.word
    if content
        .split_whitespace()
        .any(|w| w.contains('@') && w.contains('.') && w.len() > 5)
    {
        return Err("contains PII (email)");
    }

    let secret_prefixes = [
        "sk-", "pk-", "ghp_", "gho_", "xoxb-", "xapp-", "AKIA", "Bearer ",
    ];
    let lower = content.to_ascii_lowercase();
    for prefix in secret_prefixes {
        if lower.contains(&prefix.to_ascii_lowercase()) {
            return Err("contains secret");
        }
    }

    Ok(())
}

/// Check for memory poisoning (broken-behavior facts). Returns true if safe.
pub fn poisoning_check(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();

    let poison_phrases = [
        "tool calls were broken",
        "auth fails",
        "not following instructions",
        "tool execution failed",
        "api returned error",
    ];

    let is_poisoned = poison_phrases.iter().any(|phrase| lower.contains(phrase));
    if !is_poisoned {
        return true;
    }

    // Allow corrective action items
    let corrective_prefixes = ["todo:", "ensure", "fix:", "action:"];
    corrective_prefixes
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// Run all quality gates. Returns Ok(()) or first failure reason.
pub fn validate_observation(content: &str) -> std::result::Result<(), &'static str> {
    quality_check(content)?;
    pii_check(content)?;
    if !poisoning_check(content) {
        return Err("poisoning risk");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // ... tests from Step 1 above ...
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p mchact-memory quality`
Expected: All 11 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mchact-memory/src/quality.rs
git commit -m "feat(mchact-memory): quality gates with PII scan and poisoning guard"
```

---

## Task 3: Schema SQL Files

**Files:**
- Create: `crates/mchact-memory/src/schema/sqlite.sql`
- Create: `crates/mchact-memory/src/schema/postgres.sql`

- [ ] **Step 1: Write SQLite schema**

```sql
-- crates/mchact-memory/src/schema/sqlite.sql

CREATE TABLE IF NOT EXISTS peers (
    id          INTEGER PRIMARY KEY,
    workspace   TEXT NOT NULL DEFAULT 'default',
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'user',
    peer_card   TEXT,
    metadata    TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE(workspace, name)
);

CREATE TABLE IF NOT EXISTS observations (
    id                INTEGER PRIMARY KEY,
    workspace         TEXT NOT NULL DEFAULT 'default',
    observer_peer_id  INTEGER NOT NULL REFERENCES peers(id),
    observed_peer_id  INTEGER NOT NULL REFERENCES peers(id),
    chat_id           INTEGER,
    level             TEXT NOT NULL,
    content           TEXT NOT NULL,
    category          TEXT,
    confidence        REAL NOT NULL DEFAULT 0.8,
    source            TEXT NOT NULL DEFAULT 'deriver',
    source_ids        TEXT DEFAULT '[]',
    message_ids       TEXT DEFAULT '[]',
    times_derived     INTEGER DEFAULT 0,
    is_archived       INTEGER DEFAULT 0,
    archived_at       TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_obs_observer ON observations(observer_peer_id, is_archived);
CREATE INDEX IF NOT EXISTS idx_obs_observed ON observations(observed_peer_id, is_archived);
CREATE INDEX IF NOT EXISTS idx_obs_level ON observations(level);
CREATE INDEX IF NOT EXISTS idx_obs_chat ON observations(chat_id);
CREATE INDEX IF NOT EXISTS idx_obs_confidence ON observations(confidence);

CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
    content, content='observations', content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS observations_fts_ai AFTER INSERT ON observations BEGIN
    INSERT INTO observations_fts(rowid, content)
    VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS observations_fts_ad AFTER DELETE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, content)
    VALUES ('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS observations_fts_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, content)
    VALUES ('delete', old.id, old.content);
    INSERT INTO observations_fts(rowid, content)
    VALUES (new.id, new.content);
END;

CREATE TABLE IF NOT EXISTS observation_queue (
    id                INTEGER PRIMARY KEY,
    task_type         TEXT NOT NULL,
    workspace         TEXT NOT NULL DEFAULT 'default',
    chat_id           INTEGER,
    observer_peer_id  INTEGER,
    observed_peer_id  INTEGER,
    payload           TEXT,
    processed         INTEGER DEFAULT 0,
    created_at        TEXT NOT NULL,
    processed_at      TEXT
);

CREATE TABLE IF NOT EXISTS findings (
    id                INTEGER PRIMARY KEY,
    orchestration_id  TEXT NOT NULL,
    run_id            TEXT NOT NULL,
    finding           TEXT NOT NULL,
    category          TEXT DEFAULT 'general',
    created_at        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_findings_orch ON findings(orchestration_id);

CREATE TABLE IF NOT EXISTS deriver_runs (
    id                  INTEGER PRIMARY KEY,
    chat_id             INTEGER,
    workspace           TEXT,
    started_at          TEXT NOT NULL,
    finished_at         TEXT NOT NULL,
    messages_processed  INTEGER DEFAULT 0,
    explicit_count      INTEGER DEFAULT 0,
    deductive_count     INTEGER DEFAULT 0,
    skipped_count       INTEGER DEFAULT 0,
    error_text          TEXT
);

CREATE TABLE IF NOT EXISTS dreamer_runs (
    id                   INTEGER PRIMARY KEY,
    workspace            TEXT,
    observer_peer_id     INTEGER,
    observed_peer_id     INTEGER,
    deductions_created   INTEGER DEFAULT 0,
    inductions_created   INTEGER DEFAULT 0,
    contradictions_found INTEGER DEFAULT 0,
    consolidated         INTEGER DEFAULT 0,
    peer_card_updated    INTEGER DEFAULT 0,
    run_at               TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS injection_logs (
    id               INTEGER PRIMARY KEY,
    chat_id          INTEGER,
    created_at       TEXT NOT NULL,
    retrieval_method TEXT,
    candidate_count  INTEGER,
    selected_count   INTEGER,
    omitted_count    INTEGER,
    tokens_est       INTEGER
);
```

- [ ] **Step 2: Write PostgreSQL schema**

```sql
-- crates/mchact-memory/src/schema/postgres.sql

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS peers (
    id          BIGSERIAL PRIMARY KEY,
    workspace   TEXT NOT NULL DEFAULT 'default',
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'user',
    peer_card   JSONB,
    metadata    JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(workspace, name)
);

CREATE TABLE IF NOT EXISTS observations (
    id                BIGSERIAL PRIMARY KEY,
    workspace         TEXT NOT NULL DEFAULT 'default',
    observer_peer_id  BIGINT NOT NULL REFERENCES peers(id),
    observed_peer_id  BIGINT NOT NULL REFERENCES peers(id),
    chat_id           BIGINT,
    level             TEXT NOT NULL,
    content           TEXT NOT NULL,
    category          TEXT,
    confidence        REAL NOT NULL DEFAULT 0.8,
    source            TEXT NOT NULL DEFAULT 'deriver',
    source_ids        JSONB DEFAULT '[]',
    message_ids       JSONB DEFAULT '[]',
    times_derived     INTEGER DEFAULT 0,
    is_archived       BOOLEAN DEFAULT FALSE,
    archived_at       TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    embedding         vector(1536),
    tsv               tsvector GENERATED ALWAYS AS (to_tsvector('english', content)) STORED
);

CREATE INDEX IF NOT EXISTS idx_obs_observer ON observations(observer_peer_id, is_archived);
CREATE INDEX IF NOT EXISTS idx_obs_observed ON observations(observed_peer_id, is_archived);
CREATE INDEX IF NOT EXISTS idx_obs_level ON observations(level);
CREATE INDEX IF NOT EXISTS idx_obs_chat ON observations(chat_id);
CREATE INDEX IF NOT EXISTS idx_obs_confidence ON observations(confidence);
CREATE INDEX IF NOT EXISTS idx_obs_tsv ON observations USING GIN (tsv);
CREATE INDEX IF NOT EXISTS idx_obs_embedding ON observations USING hnsw (embedding vector_cosine_ops);

CREATE TABLE IF NOT EXISTS observation_queue (
    id                BIGSERIAL PRIMARY KEY,
    task_type         TEXT NOT NULL,
    workspace         TEXT NOT NULL DEFAULT 'default',
    chat_id           BIGINT,
    observer_peer_id  BIGINT,
    observed_peer_id  BIGINT,
    payload           JSONB,
    processed         BOOLEAN DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at      TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS findings (
    id                BIGSERIAL PRIMARY KEY,
    orchestration_id  TEXT NOT NULL,
    run_id            TEXT NOT NULL,
    finding           TEXT NOT NULL,
    category          TEXT DEFAULT 'general',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_findings_orch ON findings(orchestration_id);

CREATE TABLE IF NOT EXISTS deriver_runs (
    id                  BIGSERIAL PRIMARY KEY,
    chat_id             BIGINT,
    workspace           TEXT,
    started_at          TIMESTAMPTZ NOT NULL,
    finished_at         TIMESTAMPTZ NOT NULL,
    messages_processed  INTEGER DEFAULT 0,
    explicit_count      INTEGER DEFAULT 0,
    deductive_count     INTEGER DEFAULT 0,
    skipped_count       INTEGER DEFAULT 0,
    error_text          TEXT
);

CREATE TABLE IF NOT EXISTS dreamer_runs (
    id                   BIGSERIAL PRIMARY KEY,
    workspace            TEXT,
    observer_peer_id     BIGINT,
    observed_peer_id     BIGINT,
    deductions_created   INTEGER DEFAULT 0,
    inductions_created   INTEGER DEFAULT 0,
    contradictions_found INTEGER DEFAULT 0,
    consolidated         INTEGER DEFAULT 0,
    peer_card_updated    INTEGER DEFAULT 0,
    run_at               TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS injection_logs (
    id               BIGSERIAL PRIMARY KEY,
    chat_id          BIGINT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    retrieval_method TEXT,
    candidate_count  INTEGER,
    selected_count   INTEGER,
    omitted_count    INTEGER,
    tokens_est       INTEGER
);
```

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-memory/src/schema/
git commit -m "feat(mchact-memory): SQLite and PostgreSQL schema definitions"
```

---

## Task 4: SQLite Driver — Core CRUD + Peers

**Files:**
- Create: `crates/mchact-memory/src/driver/mod.rs`
- Create: `crates/mchact-memory/src/driver/sqlite.rs`

- [ ] **Step 1: Write integration test for SQLite CRUD**

```rust
// crates/mchact-memory/tests/sqlite_integration.rs
use mchact_memory::driver::sqlite::SqliteDriver;
use mchact_memory::types::*;
use mchact_memory::ObservationStore;

fn test_driver() -> SqliteDriver {
    SqliteDriver::open_in_memory().expect("open in-memory db")
}

#[tokio::test]
async fn test_peer_create_and_get() {
    let store = test_driver();
    let peer = store
        .get_or_create_peer("default", "alice", PeerKind::User)
        .await
        .unwrap();
    assert_eq!(peer.name, "alice");
    assert_eq!(peer.kind, PeerKind::User);

    // Second call returns same peer
    let peer2 = store
        .get_or_create_peer("default", "alice", PeerKind::User)
        .await
        .unwrap();
    assert_eq!(peer.id, peer2.id);
}

#[tokio::test]
async fn test_peer_card_update() {
    let store = test_driver();
    let peer = store
        .get_or_create_peer("default", "alice", PeerKind::User)
        .await
        .unwrap();

    let facts = vec!["backend developer".to_string(), "prefers Rust".to_string()];
    store.update_peer_card(peer.id, facts.clone()).await.unwrap();

    let card = store.get_peer_card(peer.id).await.unwrap().unwrap();
    assert_eq!(card.facts, facts);
    assert_eq!(card.peer_name, "alice");
}

#[tokio::test]
async fn test_store_and_get_observation() {
    let store = test_driver();
    let observer = store
        .get_or_create_peer("default", "bot", PeerKind::Agent)
        .await
        .unwrap();
    let observed = store
        .get_or_create_peer("default", "alice", PeerKind::User)
        .await
        .unwrap();

    let id = store
        .store_observation(NewObservation {
            workspace: "default".to_string(),
            observer_peer_id: observer.id,
            observed_peer_id: observed.id,
            chat_id: Some(42),
            level: ObservationLevel::Explicit,
            content: "alice is a backend developer".to_string(),
            category: Some("PROFILE".to_string()),
            confidence: 0.85,
            source: "deriver".to_string(),
            source_ids: vec![],
            message_ids: vec![100],
        })
        .await
        .unwrap();

    let obs = store.get_observation(id).await.unwrap().unwrap();
    assert_eq!(obs.content, "alice is a backend developer");
    assert_eq!(obs.level, ObservationLevel::Explicit);
    assert_eq!(obs.observer_peer_id, observer.id);
    assert_eq!(obs.observed_peer_id, observed.id);
    assert_eq!(obs.message_ids, vec![100]);
}

#[tokio::test]
async fn test_archive_observation() {
    let store = test_driver();
    let observer = store
        .get_or_create_peer("default", "bot", PeerKind::Agent)
        .await
        .unwrap();
    let observed = store
        .get_or_create_peer("default", "alice", PeerKind::User)
        .await
        .unwrap();

    let id = store
        .store_observation(NewObservation {
            workspace: "default".to_string(),
            observer_peer_id: observer.id,
            observed_peer_id: observed.id,
            chat_id: None,
            level: ObservationLevel::Explicit,
            content: "temporary fact".to_string(),
            category: None,
            confidence: 0.5,
            source: "test".to_string(),
            source_ids: vec![],
            message_ids: vec![],
        })
        .await
        .unwrap();

    store.archive_observation(id).await.unwrap();
    let obs = store.get_observation(id).await.unwrap().unwrap();
    assert!(obs.is_archived);
    assert!(obs.archived_at.is_some());
}

#[tokio::test]
async fn test_update_observation() {
    let store = test_driver();
    let observer = store
        .get_or_create_peer("default", "bot", PeerKind::Agent)
        .await
        .unwrap();
    let observed = store
        .get_or_create_peer("default", "alice", PeerKind::User)
        .await
        .unwrap();

    let id = store
        .store_observation(NewObservation {
            workspace: "default".to_string(),
            observer_peer_id: observer.id,
            observed_peer_id: observed.id,
            chat_id: None,
            level: ObservationLevel::Explicit,
            content: "original content".to_string(),
            category: None,
            confidence: 0.5,
            source: "test".to_string(),
            source_ids: vec![],
            message_ids: vec![],
        })
        .await
        .unwrap();

    store
        .update_observation(
            id,
            ObservationUpdate {
                content: Some("updated content".to_string()),
                confidence: Some(0.9),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let obs = store.get_observation(id).await.unwrap().unwrap();
    assert_eq!(obs.content, "updated content");
    assert_eq!(obs.confidence, 0.9);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mchact-memory --test sqlite_integration`
Expected: FAIL — `SqliteDriver` doesn't exist yet.

- [ ] **Step 3: Create driver/mod.rs**

```rust
// crates/mchact-memory/src/driver/mod.rs

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;
```

- [ ] **Step 4: Implement SqliteDriver core CRUD + peers**

```rust
// crates/mchact-memory/src/driver/sqlite.rs

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::types::*;
use crate::{MemoryError, ObservationStore, Result};

pub struct SqliteDriver {
    conn: Mutex<Connection>,
}

impl SqliteDriver {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        let driver = Self {
            conn: Mutex::new(conn),
        };
        driver.initialize()?;
        Ok(driver)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        let driver = Self {
            conn: Mutex::new(conn),
        };
        driver.initialize()?;
        Ok(driver)
    }

    fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let schema = include_str!("../schema/sqlite.sql");
        conn.execute_batch(schema)
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    fn now_iso() -> String {
        chrono::Utc::now().to_rfc3339()
    }
}

#[async_trait::async_trait]
impl ObservationStore for SqliteDriver {
    async fn store_observation(&self, obs: NewObservation) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        let source_ids_json = serde_json::to_string(&obs.source_ids)?;
        let message_ids_json = serde_json::to_string(&obs.message_ids)?;
        conn.execute(
            "INSERT INTO observations (workspace, observer_peer_id, observed_peer_id, chat_id,
             level, content, category, confidence, source, source_ids, message_ids,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                obs.workspace,
                obs.observer_peer_id,
                obs.observed_peer_id,
                obs.chat_id,
                obs.level.as_str(),
                obs.content,
                obs.category,
                obs.confidence,
                obs.source,
                source_ids_json,
                message_ids_json,
                now,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(conn.last_insert_rowid())
    }

    async fn get_observation(&self, id: i64) -> Result<Option<Observation>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id,
                    level, content, category, confidence, source, source_ids,
                    message_ids, times_derived, is_archived, archived_at,
                    created_at, updated_at
             FROM observations WHERE id = ?1",
            params![id],
            |row| Ok(row_to_observation(row)),
        )
        .optional()
        .map_err(|e| MemoryError::Database(e.to_string()))?
        .map(|r| r.map(Some))
        .unwrap_or(Ok(None))
    }

    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        let mut sets = vec!["updated_at = ?1".to_string()];
        let mut param_idx = 2u32;
        // Build dynamic SET clause — we'll use a simpler approach for SQLite
        // by reading current values and overwriting
        let current = conn
            .query_row(
                "SELECT content, category, confidence, source_ids, times_derived
                 FROM observations WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i32>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .ok_or_else(|| MemoryError::NotFound(format!("observation {id}")))?;

        let content = update.content.unwrap_or(current.0);
        let category = update.category.or(current.1);
        let confidence = update.confidence.unwrap_or(current.2);
        let source_ids = update
            .source_ids
            .map(|ids| serde_json::to_string(&ids).unwrap_or_default())
            .unwrap_or(current.3);
        let times_derived = update.times_derived.unwrap_or(current.4);

        conn.execute(
            "UPDATE observations SET content = ?1, category = ?2, confidence = ?3,
             source_ids = ?4, times_derived = ?5, updated_at = ?6
             WHERE id = ?7",
            params![content, category, confidence, source_ids, times_derived, now, id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn archive_observation(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        conn.execute(
            "UPDATE observations SET is_archived = 1, archived_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn get_or_create_peer(
        &self,
        workspace: &str,
        name: &str,
        kind: PeerKind,
    ) -> Result<Peer> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<Peer> = conn
            .query_row(
                "SELECT id, workspace, name, kind, peer_card, metadata, created_at, updated_at
                 FROM peers WHERE workspace = ?1 AND name = ?2",
                params![workspace, name],
                |row| Ok(row_to_peer(row)),
            )
            .optional()
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .transpose()
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        if let Some(peer) = existing {
            return Ok(peer);
        }

        let now = Self::now_iso();
        conn.execute(
            "INSERT INTO peers (workspace, name, kind, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![workspace, name, kind.as_str(), now, now],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();
        Ok(Peer {
            id,
            workspace: workspace.to_string(),
            name: name.to_string(),
            kind,
            peer_card: None,
            metadata: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, peer_card FROM peers WHERE id = ?1",
            params![peer_id],
            |row| {
                let name: String = row.get(1)?;
                let card_json: Option<String> = row.get(2)?;
                let facts: Vec<String> = card_json
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default();
                if facts.is_empty() {
                    return Ok(None);
                }
                Ok(Some(PeerCard {
                    peer_id,
                    peer_name: name,
                    facts,
                }))
            },
        )
        .optional()
        .map_err(|e| MemoryError::Database(e.to_string()))?
        .unwrap_or(Ok(None))
    }

    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        let truncated: Vec<String> = facts.into_iter().take(PeerCard::MAX_FACTS).collect();
        let json = serde_json::to_string(&truncated)?;
        conn.execute(
            "UPDATE peers SET peer_card = ?1, updated_at = ?2 WHERE id = ?3",
            params![json, now, peer_id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    // --- Stub implementations for remaining trait methods ---
    // These will be implemented in subsequent tasks.

    async fn search_hybrid(
        &self,
        _query: &str,
        _scope: SearchScope,
        _limit: usize,
    ) -> Result<Vec<Observation>> {
        Ok(vec![])
    }

    async fn get_for_context(
        &self,
        _chat_id: i64,
        _user_query: &str,
        _token_budget: usize,
    ) -> Result<Vec<Observation>> {
        Ok(vec![])
    }

    async fn trace_reasoning(&self, _observation_id: i64) -> Result<Vec<Observation>> {
        Ok(vec![])
    }

    async fn enqueue(&self, _task: QueueTask) -> Result<i64> {
        Ok(0)
    }

    async fn dequeue(&self, _task_type: &str, _limit: usize) -> Result<Vec<QueueItem>> {
        Ok(vec![])
    }

    async fn mark_processed(&self, _queue_id: i64) -> Result<()> {
        Ok(())
    }

    async fn insert_finding(
        &self,
        _orchestration_id: &str,
        _run_id: &str,
        _finding: &str,
        _category: &str,
    ) -> Result<i64> {
        Ok(0)
    }

    async fn get_findings(&self, _orchestration_id: &str) -> Result<Vec<Finding>> {
        Ok(vec![])
    }

    async fn delete_findings(&self, _orchestration_id: &str) -> Result<usize> {
        Ok(0)
    }

    async fn upsert_embedding(&self, _observation_id: i64, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }

    async fn log_deriver_run(&self, _run: DeriverRun) -> Result<()> {
        Ok(())
    }

    async fn log_dreamer_run(&self, _run: DreamerRun) -> Result<()> {
        Ok(())
    }

    async fn log_injection(&self, _log: InjectionLog) -> Result<()> {
        Ok(())
    }
}

fn row_to_observation(row: &rusqlite::Row) -> std::result::Result<Observation, rusqlite::Error> {
    let level_str: String = row.get(5)?;
    let source_ids_json: String = row.get(10)?;
    let message_ids_json: String = row.get(11)?;
    let is_archived_int: i32 = row.get(13)?;

    Ok(Observation {
        id: row.get(0)?,
        workspace: row.get(1)?,
        observer_peer_id: row.get(2)?,
        observed_peer_id: row.get(3)?,
        chat_id: row.get(4)?,
        level: ObservationLevel::from_str(&level_str).unwrap_or(ObservationLevel::Explicit),
        content: row.get(6)?,
        category: row.get(7)?,
        confidence: row.get(8)?,
        source: row.get(9)?,
        source_ids: serde_json::from_str(&source_ids_json).unwrap_or_default(),
        message_ids: serde_json::from_str(&message_ids_json).unwrap_or_default(),
        times_derived: row.get(12)?,
        is_archived: is_archived_int != 0,
        archived_at: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn row_to_peer(row: &rusqlite::Row) -> std::result::Result<Peer, rusqlite::Error> {
    let kind_str: String = row.get(3)?;
    let card_json: Option<String> = row.get(4)?;
    Ok(Peer {
        id: row.get(0)?,
        workspace: row.get(1)?,
        name: row.get(2)?,
        kind: PeerKind::from_str(&kind_str).unwrap_or(PeerKind::User),
        peer_card: card_json.and_then(|j| serde_json::from_str(&j).ok()),
        metadata: row
            .get::<_, Option<String>>(5)?
            .and_then(|j| serde_json::from_str(&j).ok()),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
```

- [ ] **Step 5: Run integration tests**

Run: `cargo test -p mchact-memory --test sqlite_integration`
Expected: All 5 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/mchact-memory/src/driver/ crates/mchact-memory/tests/
git commit -m "feat(mchact-memory): SQLite driver with core CRUD and peer management"
```

---

## Task 5: SQLite Driver — Queue + Findings + Observability

**Files:**
- Modify: `crates/mchact-memory/src/driver/sqlite.rs`
- Modify: `crates/mchact-memory/tests/sqlite_integration.rs`

- [ ] **Step 1: Write queue and findings tests**

Add to `crates/mchact-memory/tests/sqlite_integration.rs`:

```rust
#[tokio::test]
async fn test_queue_enqueue_dequeue() {
    let store = test_driver();
    let id = store
        .enqueue(QueueTask {
            task_type: "derive".to_string(),
            workspace: "default".to_string(),
            chat_id: Some(42),
            observer_peer_id: None,
            observed_peer_id: None,
            payload: Some(serde_json::json!({"msg_ids": [1, 2, 3]})),
        })
        .await
        .unwrap();
    assert!(id > 0);

    let items = store.dequeue("derive", 10).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].chat_id, Some(42));

    store.mark_processed(items[0].id).await.unwrap();

    let items2 = store.dequeue("derive", 10).await.unwrap();
    assert!(items2.is_empty());
}

#[tokio::test]
async fn test_findings_crud() {
    let store = test_driver();
    let id1 = store
        .insert_finding("orch-1", "worker-a", "Found API rate limit is 100/min", "technical")
        .await
        .unwrap();
    let id2 = store
        .insert_finding("orch-1", "worker-b", "Auth uses JWT tokens", "security")
        .await
        .unwrap();
    store
        .insert_finding("orch-2", "worker-x", "Unrelated finding", "general")
        .await
        .unwrap();

    let findings = store.get_findings("orch-1").await.unwrap();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].run_id, "worker-a");
    assert_eq!(findings[1].category, "security");

    let deleted = store.delete_findings("orch-1").await.unwrap();
    assert_eq!(deleted, 2);

    let remaining = store.get_findings("orch-1").await.unwrap();
    assert!(remaining.is_empty());

    // orch-2 untouched
    let other = store.get_findings("orch-2").await.unwrap();
    assert_eq!(other.len(), 1);
}

#[tokio::test]
async fn test_observability_logging() {
    let store = test_driver();
    store
        .log_deriver_run(DeriverRun {
            chat_id: Some(42),
            workspace: "default".to_string(),
            started_at: "2026-03-27T10:00:00Z".to_string(),
            finished_at: "2026-03-27T10:00:05Z".to_string(),
            messages_processed: 10,
            explicit_count: 3,
            deductive_count: 1,
            skipped_count: 6,
            error_text: None,
        })
        .await
        .unwrap();

    store
        .log_dreamer_run(DreamerRun {
            workspace: "default".to_string(),
            observer_peer_id: Some(1),
            observed_peer_id: Some(2),
            deductions_created: 2,
            inductions_created: 1,
            contradictions_found: 0,
            consolidated: 3,
            peer_card_updated: 1,
            run_at: "2026-03-27T12:00:00Z".to_string(),
        })
        .await
        .unwrap();

    store
        .log_injection(InjectionLog {
            chat_id: Some(42),
            retrieval_method: "rrf_hybrid".to_string(),
            candidate_count: 50,
            selected_count: 10,
            omitted_count: 40,
            tokens_est: 1200,
        })
        .await
        .unwrap();
    // No assertion beyond "doesn't panic" — observability is fire-and-forget
}
```

- [ ] **Step 2: Run tests to verify new ones fail**

Run: `cargo test -p mchact-memory --test sqlite_integration`
Expected: New tests FAIL (stubs return empty/0).

- [ ] **Step 3: Implement queue, findings, and observability methods in sqlite.rs**

Replace the stub implementations in `SqliteDriver`:

```rust
    // --- Queue ---
    async fn enqueue(&self, task: QueueTask) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        let payload_json = task
            .payload
            .map(|v| serde_json::to_string(&v).unwrap_or_default());
        conn.execute(
            "INSERT INTO observation_queue (task_type, workspace, chat_id,
             observer_peer_id, observed_peer_id, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                task.task_type,
                task.workspace,
                task.chat_id,
                task.observer_peer_id,
                task.observed_peer_id,
                payload_json,
                now,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(conn.last_insert_rowid())
    }

    async fn dequeue(&self, task_type: &str, limit: usize) -> Result<Vec<QueueItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, task_type, workspace, chat_id, observer_peer_id,
                        observed_peer_id, payload, created_at
                 FROM observation_queue
                 WHERE task_type = ?1 AND processed = 0
                 ORDER BY id ASC
                 LIMIT ?2",
            )
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        let items = stmt
            .query_map(params![task_type, limit as i64], |row| {
                let payload_str: Option<String> = row.get(6)?;
                Ok(QueueItem {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    workspace: row.get(2)?,
                    chat_id: row.get(3)?,
                    observer_peer_id: row.get(4)?,
                    observed_peer_id: row.get(5)?,
                    payload: payload_str.and_then(|s| serde_json::from_str(&s).ok()),
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(items)
    }

    async fn mark_processed(&self, queue_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        conn.execute(
            "UPDATE observation_queue SET processed = 1, processed_at = ?1 WHERE id = ?2",
            params![now, queue_id],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    // --- Findings ---
    async fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        conn.execute(
            "INSERT INTO findings (orchestration_id, run_id, finding, category, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![orchestration_id, run_id, finding, category, now],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(conn.last_insert_rowid())
    }

    async fn get_findings(&self, orchestration_id: &str) -> Result<Vec<Finding>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, orchestration_id, run_id, finding, category, created_at
                 FROM findings WHERE orchestration_id = ?1 ORDER BY id ASC",
            )
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        let findings = stmt
            .query_map(params![orchestration_id], |row| {
                Ok(Finding {
                    id: row.get(0)?,
                    orchestration_id: row.get(1)?,
                    run_id: row.get(2)?,
                    finding: row.get(3)?,
                    category: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(findings)
    }

    async fn delete_findings(&self, orchestration_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn
            .execute(
                "DELETE FROM findings WHERE orchestration_id = ?1",
                params![orchestration_id],
            )
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(count)
    }

    // --- Observability ---
    async fn log_deriver_run(&self, run: DeriverRun) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO deriver_runs (chat_id, workspace, started_at, finished_at,
             messages_processed, explicit_count, deductive_count, skipped_count, error_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.chat_id,
                run.workspace,
                run.started_at,
                run.finished_at,
                run.messages_processed,
                run.explicit_count,
                run.deductive_count,
                run.skipped_count,
                run.error_text,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn log_dreamer_run(&self, run: DreamerRun) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO dreamer_runs (workspace, observer_peer_id, observed_peer_id,
             deductions_created, inductions_created, contradictions_found,
             consolidated, peer_card_updated, run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.workspace,
                run.observer_peer_id,
                run.observed_peer_id,
                run.deductions_created,
                run.inductions_created,
                run.contradictions_found,
                run.consolidated,
                run.peer_card_updated,
                run.run_at,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn log_injection(&self, log: InjectionLog) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_iso();
        conn.execute(
            "INSERT INTO injection_logs (chat_id, created_at, retrieval_method,
             candidate_count, selected_count, omitted_count, tokens_est)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                log.chat_id,
                now,
                log.retrieval_method,
                log.candidate_count,
                log.selected_count,
                log.omitted_count,
                log.tokens_est,
            ],
        )
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }
```

- [ ] **Step 4: Run all integration tests**

Run: `cargo test -p mchact-memory --test sqlite_integration`
Expected: All 8 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mchact-memory/
git commit -m "feat(mchact-memory): SQLite driver queue, findings, and observability"
```

---

## Task 6: RRF Hybrid Search + FTS5 Keyword Search

**Files:**
- Create: `crates/mchact-memory/src/search.rs`
- Modify: `crates/mchact-memory/src/driver/sqlite.rs`
- Modify: `crates/mchact-memory/tests/sqlite_integration.rs`

- [ ] **Step 1: Write search tests**

Add to `crates/mchact-memory/tests/sqlite_integration.rs`:

```rust
#[tokio::test]
async fn test_keyword_search_via_fts5() {
    let store = test_driver();
    let observer = store.get_or_create_peer("default", "bot", PeerKind::Agent).await.unwrap();
    let observed = store.get_or_create_peer("default", "alice", PeerKind::User).await.unwrap();

    for (content, category) in [
        ("alice is a backend developer who uses Rust", "PROFILE"),
        ("alice is allergic to peanuts", "PROFILE"),
        ("alice loves Thai food and eats out frequently", "PROFILE"),
        ("the project uses axum for web framework", "KNOWLEDGE"),
    ] {
        store.store_observation(NewObservation {
            workspace: "default".to_string(),
            observer_peer_id: observer.id,
            observed_peer_id: observed.id,
            chat_id: Some(42),
            level: ObservationLevel::Explicit,
            content: content.to_string(),
            category: Some(category.to_string()),
            confidence: 0.85,
            source: "test".to_string(),
            source_ids: vec![],
            message_ids: vec![],
        }).await.unwrap();
    }

    let results = store.search_hybrid(
        "Rust developer",
        SearchScope { workspace: "default".to_string(), ..Default::default() },
        10,
    ).await.unwrap();

    assert!(!results.is_empty());
    assert!(results[0].content.contains("Rust"));
}

#[tokio::test]
async fn test_search_respects_archived() {
    let store = test_driver();
    let observer = store.get_or_create_peer("default", "bot", PeerKind::Agent).await.unwrap();
    let observed = store.get_or_create_peer("default", "alice", PeerKind::User).await.unwrap();

    let id = store.store_observation(NewObservation {
        workspace: "default".to_string(),
        observer_peer_id: observer.id,
        observed_peer_id: observed.id,
        chat_id: None,
        level: ObservationLevel::Explicit,
        content: "archived fact about databases".to_string(),
        category: None,
        confidence: 0.85,
        source: "test".to_string(),
        source_ids: vec![],
        message_ids: vec![],
    }).await.unwrap();

    store.archive_observation(id).await.unwrap();

    let results = store.search_hybrid(
        "databases",
        SearchScope { workspace: "default".to_string(), include_archived: false, ..Default::default() },
        10,
    ).await.unwrap();

    assert!(results.is_empty());
}
```

And write unit test for RRF merge in `crates/mchact-memory/src/search.rs`:

```rust
// crates/mchact-memory/src/search.rs
use crate::types::RankedResult;

/// Reciprocal Rank Fusion merge of keyword and semantic search results.
/// k = 60 is standard RRF constant.
pub fn rrf_merge(
    keyword_ids: &[(i64, f64)],
    semantic_ids: &[(i64, f64)],
    limit: usize,
) -> Vec<RankedResult> {
    use std::collections::HashMap;
    let k = 60.0;

    let mut scores: HashMap<i64, (Option<f64>, Option<f64>)> = HashMap::new();

    for (rank, (id, score)) in keyword_ids.iter().enumerate() {
        scores.entry(*id).or_insert((None, None)).0 = Some(rank as f64);
    }

    for (rank, (id, score)) in semantic_ids.iter().enumerate() {
        scores.entry(*id).or_insert((None, None)).1 = Some(rank as f64);
    }

    let mut results: Vec<RankedResult> = scores
        .into_iter()
        .map(|(id, (kw_rank, sem_rank))| {
            let kw_score = 1.0 / (k + kw_rank.unwrap_or(1000.0));
            let sem_score = 1.0 / (k + sem_rank.unwrap_or(1000.0));
            RankedResult {
                observation_id: id,
                keyword_rank: kw_rank,
                semantic_rank: sem_rank,
                rrf_score: kw_score + sem_score,
            }
        })
        .collect();

    results.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap());
    results.truncate(limit);
    results
}

/// Sanitize a raw query for FTS5 MATCH. Strips operators, quotes tokens.
pub fn sanitize_fts_query(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '"' | '*' | '(' | ')' | '+' | '-' | '^' | '~' | ':' | '{' | '}' | '[' | ']' => ' ',
            _ => c,
        })
        .collect();
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let expr = tokens
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");
    Some(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge_basic() {
        let keyword = vec![(1, 0.9), (2, 0.8), (3, 0.7)];
        let semantic = vec![(2, 0.95), (4, 0.85), (1, 0.75)];
        let merged = rrf_merge(&keyword, &semantic, 10);

        // ID 2 should be top (appears in both at good ranks)
        assert_eq!(merged[0].observation_id, 2);
        // ID 1 also in both
        assert_eq!(merged[1].observation_id, 1);
    }

    #[test]
    fn test_rrf_merge_empty() {
        let merged = rrf_merge(&[], &[], 10);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_rrf_merge_single_arm() {
        let keyword = vec![(1, 0.9), (2, 0.8)];
        let merged = rrf_merge(&keyword, &[], 10);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].observation_id, 1);
    }

    #[test]
    fn test_sanitize_fts_normal() {
        assert_eq!(
            sanitize_fts_query("hello world"),
            Some("\"hello\" \"world\"".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts_operators() {
        assert_eq!(
            sanitize_fts_query("hello AND (world)"),
            Some("\"hello\" \"AND\" \"world\"".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts_empty() {
        assert_eq!(sanitize_fts_query(""), None);
        assert_eq!(sanitize_fts_query("+-*()"), None);
    }
}
```

- [ ] **Step 2: Run tests to verify new ones fail**

Run: `cargo test -p mchact-memory`
Expected: Unit tests in search.rs PASS. Integration search tests FAIL (search_hybrid still returns empty).

- [ ] **Step 3: Implement search_hybrid in sqlite.rs**

Replace the `search_hybrid` stub:

```rust
    async fn search_hybrid(
        &self,
        query: &str,
        scope: SearchScope,
        limit: usize,
    ) -> Result<Vec<Observation>> {
        let sanitized = crate::search::sanitize_fts_query(query);
        let keyword_results = if let Some(ref fts_query) = sanitized {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT o.id, observations_fts.rank
                     FROM observations_fts
                     JOIN observations o ON o.id = observations_fts.rowid
                     WHERE observations_fts MATCH ?1
                       AND o.workspace = ?2
                       AND o.is_archived = ?3
                       AND o.confidence >= ?4
                     ORDER BY observations_fts.rank
                     LIMIT 20",
                )
                .map_err(|e| MemoryError::Database(e.to_string()))?;

            let is_archived = if scope.include_archived { 1 } else { 0 };
            stmt.query_map(
                params![fts_query, scope.workspace, is_archived, scope.min_confidence],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
            )
            .map_err(|e| MemoryError::Database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MemoryError::Database(e.to_string()))?
        } else {
            vec![]
        };

        // For now, semantic arm is empty (until embeddings are wired).
        // RRF merge with keyword-only.
        let merged = crate::search::rrf_merge(&keyword_results, &[], limit);

        // Fetch full observations for merged IDs
        let mut observations = Vec::new();
        for ranked in &merged {
            if let Some(obs) = self.get_observation(ranked.observation_id).await? {
                observations.push(obs);
            }
        }
        Ok(observations)
    }
```

Also implement `get_for_context`:

```rust
    async fn get_for_context(
        &self,
        chat_id: i64,
        user_query: &str,
        token_budget: usize,
    ) -> Result<Vec<Observation>> {
        // Use hybrid search scoped to this chat
        let scope = SearchScope {
            workspace: "default".to_string(),
            chat_id: Some(chat_id),
            min_confidence: 0.45,
            ..Default::default()
        };
        let candidates = self.search_hybrid(user_query, scope, 100).await?;

        // Also fetch non-chat (global) observations
        let global_scope = SearchScope {
            workspace: "default".to_string(),
            min_confidence: 0.45,
            ..Default::default()
        };
        let global = self.search_hybrid(user_query, global_scope, 50).await?;

        // Merge, dedup by ID, obey token budget
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut tokens_used = 0usize;

        for obs in candidates.into_iter().chain(global) {
            if !seen.insert(obs.id) {
                continue;
            }
            let est_tokens = obs.content.len() / 4 + 10;
            if tokens_used + est_tokens > token_budget {
                break;
            }
            tokens_used += est_tokens;
            result.push(obs);
        }
        Ok(result)
    }
```

Note: update the `search_hybrid` FTS5 query to also filter by `is_archived = 0` (not matching against scope.include_archived since FTS5 WHERE applies to the join). The corrected WHERE clause:

```sql
AND (?3 = 1 OR o.is_archived = 0)
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p mchact-memory`
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mchact-memory/src/search.rs crates/mchact-memory/src/driver/sqlite.rs crates/mchact-memory/tests/
git commit -m "feat(mchact-memory): RRF hybrid search with FTS5 keyword arm"
```

---

## Task 7: DAG Traversal

**Files:**
- Create: `crates/mchact-memory/src/dag.rs`
- Modify: `crates/mchact-memory/src/driver/sqlite.rs`
- Modify: `crates/mchact-memory/tests/sqlite_integration.rs`

- [ ] **Step 1: Write DAG tests**

Add to `crates/mchact-memory/tests/sqlite_integration.rs`:

```rust
#[tokio::test]
async fn test_trace_reasoning_dag() {
    let store = test_driver();
    let observer = store.get_or_create_peer("default", "bot", PeerKind::Agent).await.unwrap();
    let observed = store.get_or_create_peer("default", "alice", PeerKind::User).await.unwrap();

    // Explicit: peanut allergy
    let id_peanut = store.store_observation(NewObservation {
        workspace: "default".to_string(),
        observer_peer_id: observer.id,
        observed_peer_id: observed.id,
        chat_id: Some(42),
        level: ObservationLevel::Explicit,
        content: "alice is allergic to peanuts".to_string(),
        category: Some("PROFILE".to_string()),
        confidence: 0.95,
        source: "explicit".to_string(),
        source_ids: vec![],
        message_ids: vec![],
    }).await.unwrap();

    // Explicit: loves Thai food
    let id_thai = store.store_observation(NewObservation {
        workspace: "default".to_string(),
        observer_peer_id: observer.id,
        observed_peer_id: observed.id,
        chat_id: Some(42),
        level: ObservationLevel::Explicit,
        content: "alice loves Thai food".to_string(),
        category: Some("PROFILE".to_string()),
        confidence: 0.85,
        source: "deriver".to_string(),
        source_ids: vec![],
        message_ids: vec![],
    }).await.unwrap();

    // Deductive: needs peanut checks at Thai restaurants (from both above)
    let id_deductive = store.store_observation(NewObservation {
        workspace: "default".to_string(),
        observer_peer_id: observer.id,
        observed_peer_id: observed.id,
        chat_id: Some(42),
        level: ObservationLevel::Deductive,
        content: "needs to verify peanut content at Thai restaurants".to_string(),
        category: Some("KNOWLEDGE".to_string()),
        confidence: 0.70,
        source: "dreamer".to_string(),
        source_ids: vec![id_peanut, id_thai],
        message_ids: vec![],
    }).await.unwrap();

    // Trace reasoning for the deductive observation
    let chain = store.trace_reasoning(id_deductive).await.unwrap();
    assert_eq!(chain.len(), 3); // deductive + 2 premises
    assert_eq!(chain[0].id, id_deductive);
    // Premises should be in the chain
    let premise_ids: Vec<i64> = chain[1..].iter().map(|o| o.id).collect();
    assert!(premise_ids.contains(&id_peanut));
    assert!(premise_ids.contains(&id_thai));
}

#[tokio::test]
async fn test_trace_reasoning_leaf_node() {
    let store = test_driver();
    let observer = store.get_or_create_peer("default", "bot", PeerKind::Agent).await.unwrap();
    let observed = store.get_or_create_peer("default", "alice", PeerKind::User).await.unwrap();

    let id = store.store_observation(NewObservation {
        workspace: "default".to_string(),
        observer_peer_id: observer.id,
        observed_peer_id: observed.id,
        chat_id: None,
        level: ObservationLevel::Explicit,
        content: "leaf node with no sources".to_string(),
        category: None,
        confidence: 0.85,
        source: "test".to_string(),
        source_ids: vec![],
        message_ids: vec![],
    }).await.unwrap();

    let chain = store.trace_reasoning(id).await.unwrap();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].id, id);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mchact-memory --test sqlite_integration trace_reasoning`
Expected: FAIL — `trace_reasoning` returns empty.

- [ ] **Step 3: Implement dag.rs and trace_reasoning**

```rust
// crates/mchact-memory/src/dag.rs

use crate::types::Observation;
use crate::ObservationStore;

/// Traverse the source attribution DAG breadth-first.
/// Returns the root observation followed by all premises (transitive).
/// Stops at nodes with empty source_ids (leaf/explicit observations).
/// Guards against cycles with a visited set.
pub async fn trace_reasoning(
    store: &dyn ObservationStore,
    root_id: i64,
) -> crate::Result<Vec<Observation>> {
    let mut result = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    queue.push_back(root_id);

    while let Some(id) = queue.pop_front() {
        if !visited.insert(id) {
            continue; // Cycle guard
        }
        if let Some(obs) = store.get_observation(id).await? {
            for &premise_id in &obs.source_ids {
                queue.push_back(premise_id);
            }
            result.push(obs);
        }
    }

    Ok(result)
}
```

Replace the `trace_reasoning` stub in `sqlite.rs`:

```rust
    async fn trace_reasoning(&self, observation_id: i64) -> Result<Vec<Observation>> {
        crate::dag::trace_reasoning(self, observation_id).await
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p mchact-memory`
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mchact-memory/src/dag.rs crates/mchact-memory/src/driver/sqlite.rs crates/mchact-memory/tests/
git commit -m "feat(mchact-memory): DAG traversal for source attribution chains"
```

---

## Task 8: Memory Injection

**Files:**
- Create: `crates/mchact-memory/src/injection.rs`

- [ ] **Step 1: Write injection tests**

```rust
// crates/mchact-memory/src/injection.rs

use crate::types::*;

/// Build the memory context block for system prompt injection.
/// Returns formatted XML string with peer card + ranked observations.
pub fn build_memory_context(
    peer_card: Option<&PeerCard>,
    observations: &[Observation],
    omitted_count: usize,
) -> String {
    let mut output = String::new();

    if let Some(card) = peer_card {
        output.push_str(&format!("<peer_card peer=\"{}\">\n", card.peer_name));
        for fact in &card.facts {
            output.push_str(fact);
            output.push('\n');
        }
        output.push_str("</peer_card>\n\n");
    }

    if !observations.is_empty() {
        output.push_str("<observations>\n");
        for obs in observations {
            let level_tag = obs.level.as_str().to_uppercase();
            let mut line = format!("[{}] {}", level_tag, obs.content);

            if !obs.source_ids.is_empty() {
                let refs: Vec<String> = obs.source_ids.iter().map(|id| format!("#{id}")).collect();
                line.push_str(&format!(" (from: {})", refs.join(", ")));
            }

            output.push_str(&line);
            output.push('\n');
        }
        if omitted_count > 0 {
            output.push_str(&format!("(+{omitted_count} observations omitted)\n"));
        }
        output.push_str("</observations>\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context() {
        let result = build_memory_context(None, &[], 0);
        assert_eq!(result, "");
    }

    #[test]
    fn test_peer_card_only() {
        let card = PeerCard {
            peer_id: 1,
            peer_name: "alice".to_string(),
            facts: vec!["backend developer".to_string(), "prefers Rust".to_string()],
        };
        let result = build_memory_context(Some(&card), &[], 0);
        assert!(result.contains("<peer_card peer=\"alice\">"));
        assert!(result.contains("backend developer"));
        assert!(result.contains("</peer_card>"));
    }

    #[test]
    fn test_observations_with_levels() {
        let obs = vec![
            Observation {
                id: 1, workspace: "default".into(), observer_peer_id: 1,
                observed_peer_id: 2, chat_id: None,
                level: ObservationLevel::Explicit,
                content: "alice is a backend dev".into(),
                category: None, confidence: 0.85, source: "deriver".into(),
                source_ids: vec![], message_ids: vec![], times_derived: 0,
                is_archived: false, archived_at: None,
                created_at: String::new(), updated_at: String::new(),
            },
            Observation {
                id: 2, workspace: "default".into(), observer_peer_id: 1,
                observed_peer_id: 2, chat_id: None,
                level: ObservationLevel::Deductive,
                content: "likely familiar with tokio".into(),
                category: None, confidence: 0.70, source: "dreamer".into(),
                source_ids: vec![1], message_ids: vec![], times_derived: 0,
                is_archived: false, archived_at: None,
                created_at: String::new(), updated_at: String::new(),
            },
        ];
        let result = build_memory_context(None, &obs, 5);
        assert!(result.contains("[EXPLICIT] alice is a backend dev"));
        assert!(result.contains("[DEDUCTIVE] likely familiar with tokio (from: #1)"));
        assert!(result.contains("(+5 observations omitted)"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mchact-memory injection`
Expected: All 3 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-memory/src/injection.rs
git commit -m "feat(mchact-memory): memory injection formatter for system prompts"
```

---

## Task 9: Deriver Agent (LLM-backed observation extraction)

**Files:**
- Create: `crates/mchact-memory/src/deriver.rs`

This task creates the deriver module with the extraction logic. The actual LLM call is abstracted behind a trait so it can be mocked in tests and wired to mchact's provider at integration time.

- [ ] **Step 1: Define LLM abstraction and deriver types**

```rust
// crates/mchact-memory/src/deriver.rs

use crate::quality;
use crate::types::*;
use crate::{MemoryError, ObservationStore, Result};
use serde::{Deserialize, Serialize};

/// Abstraction for LLM calls used by deriver/dreamer.
/// Implemented by mchact's LLM provider at integration time.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> std::result::Result<String, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedObservation {
    pub content: String,
    pub level: String,
    pub category: Option<String>,
    #[serde(default)]
    pub source_message_ids: Vec<i64>,
    #[serde(default)]
    pub premises: Vec<String>,
}

const DERIVER_SYSTEM_PROMPT: &str = r#"Extract observations from this conversation segment.
Output a JSON array:
[
  {
    "content": "...",
    "level": "explicit|deductive",
    "category": "PROFILE|KNOWLEDGE|EVENT",
    "source_message_ids": [...],
    "premises": []
  }
]
Rules:
- explicit: facts directly stated by the user
- deductive: logical inferences from 2+ explicit facts
- max 100 chars per observation content
- high bar: specific, durable facts only
- skip: tool errors, broken behavior, temporary state
- Return ONLY the JSON array, no other text"#;

/// Run the deriver on a batch of messages.
/// Returns the number of observations created.
pub async fn derive_observations(
    store: &dyn ObservationStore,
    llm: &dyn LlmClient,
    observer_peer_id: i64,
    observed_peer_id: i64,
    chat_id: Option<i64>,
    workspace: &str,
    messages_text: &str,
) -> Result<(i64, i64)> {
    let started_at = chrono::Utc::now().to_rfc3339();

    let response = llm
        .complete(DERIVER_SYSTEM_PROMPT, messages_text)
        .await
        .map_err(|e| MemoryError::Database(format!("LLM error: {e}")))?;

    let extracted: Vec<ExtractedObservation> = parse_extractions(&response);

    let mut explicit_count = 0i64;
    let mut deductive_count = 0i64;
    let mut skipped_count = 0i64;

    for ext in &extracted {
        let normalized = match quality::normalize_content(&ext.content, 100) {
            Some(c) => c,
            None => {
                skipped_count += 1;
                continue;
            }
        };

        if quality::validate_observation(&normalized).is_err() {
            skipped_count += 1;
            continue;
        }

        let level = ObservationLevel::from_str(&ext.level).unwrap_or(ObservationLevel::Explicit);
        let confidence = match level {
            ObservationLevel::Explicit => 0.85,
            ObservationLevel::Deductive => 0.70,
            _ => 0.60,
        };

        store
            .store_observation(NewObservation {
                workspace: workspace.to_string(),
                observer_peer_id,
                observed_peer_id,
                chat_id,
                level,
                content: normalized,
                category: ext.category.clone(),
                confidence,
                source: "deriver".to_string(),
                source_ids: vec![],
                message_ids: ext.source_message_ids.clone(),
            })
            .await?;

        match level {
            ObservationLevel::Explicit => explicit_count += 1,
            ObservationLevel::Deductive => deductive_count += 1,
            _ => {}
        }
    }

    let finished_at = chrono::Utc::now().to_rfc3339();
    store
        .log_deriver_run(DeriverRun {
            chat_id,
            workspace: workspace.to_string(),
            started_at,
            finished_at,
            messages_processed: 0, // caller knows actual count
            explicit_count,
            deductive_count,
            skipped_count,
            error_text: None,
        })
        .await?;

    Ok((explicit_count, deductive_count))
}

fn parse_extractions(response: &str) -> Vec<ExtractedObservation> {
    // Try to find JSON array in response (may have markdown fences)
    let trimmed = response.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };

    serde_json::from_str(&json_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extractions_valid_json() {
        let input = r#"[{"content":"user likes Rust","level":"explicit","category":"PROFILE","source_message_ids":[1],"premises":[]}]"#;
        let result = parse_extractions(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "user likes Rust");
    }

    #[test]
    fn test_parse_extractions_with_markdown_fences() {
        let input = "```json\n[{\"content\":\"fact\",\"level\":\"explicit\"}]\n```";
        let result = parse_extractions(input);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_extractions_invalid() {
        let result = parse_extractions("not json at all");
        assert!(result.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mchact-memory deriver`
Expected: All 3 unit tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-memory/src/deriver.rs
git commit -m "feat(mchact-memory): deriver agent with LLM abstraction and extraction logic"
```

---

## Task 10: Dreamer Agent (Offline Consolidation)

**Files:**
- Create: `crates/mchact-memory/src/dreamer.rs`

- [ ] **Step 1: Implement dreamer with all 5 phases**

```rust
// crates/mchact-memory/src/dreamer.rs

use crate::deriver::LlmClient;
use crate::types::*;
use crate::{ObservationStore, Result};

const DEDUCTION_PROMPT: &str = r#"Given these observations about a person, what can you logically infer?
Output a JSON array of deductive observations:
[{"content": "...", "premise_ids": [1, 2]}]
Rules:
- Only logical inferences supported by 2+ premises
- max 100 chars per content
- Return ONLY the JSON array"#;

const INDUCTION_PROMPT: &str = r#"What patterns do you see across these observations?
Output a JSON array:
[{"content": "...", "pattern_type": "preference|behavior|personality|tendency", "confidence": "high|medium|low"}]
Rules:
- Patterns must be supported by 3+ observations
- max 100 chars per content
- Return ONLY the JSON array"#;

const CONTRADICTION_PROMPT: &str = r#"Do any of these observations contradict each other?
Output a JSON array:
[{"content": "...", "conflicting_ids": [1, 2]}]
Rules:
- Only genuine contradictions, not nuances
- max 100 chars per content
- Return ONLY the JSON array"#;

#[derive(Debug, serde::Deserialize)]
struct DeductionResult {
    content: String,
    #[serde(default)]
    premise_ids: Vec<i64>,
}

#[derive(Debug, serde::Deserialize)]
struct InductionResult {
    content: String,
    #[serde(default)]
    confidence: String,
}

#[derive(Debug, serde::Deserialize)]
struct ContradictionResult {
    content: String,
    #[serde(default)]
    conflicting_ids: Vec<i64>,
}

pub async fn run_dream_cycle(
    store: &dyn ObservationStore,
    llm: &dyn LlmClient,
    observer_peer_id: i64,
    observed_peer_id: i64,
    workspace: &str,
) -> Result<DreamerRun> {
    let scope = SearchScope {
        workspace: workspace.to_string(),
        observer_peer_id: Some(observer_peer_id),
        observed_peer_id: Some(observed_peer_id),
        min_confidence: 0.35,
        ..Default::default()
    };

    // Fetch all active observations for this peer pair
    let observations = store.search_hybrid("", scope, 200).await?;
    if observations.len() < 3 {
        return Ok(empty_run(workspace, observer_peer_id, observed_peer_id));
    }

    let obs_text = format_observations_for_llm(&observations);
    let mut deductions_created = 0i64;
    let mut inductions_created = 0i64;
    let mut contradictions_found = 0i64;

    // Phase 1: Deduction
    if let Ok(response) = llm.complete(DEDUCTION_PROMPT, &obs_text).await {
        let deductions: Vec<DeductionResult> =
            serde_json::from_str(response.trim()).unwrap_or_default();
        for d in deductions {
            if let Some(content) = crate::quality::normalize_content(&d.content, 100) {
                if crate::quality::validate_observation(&content).is_ok() {
                    store
                        .store_observation(NewObservation {
                            workspace: workspace.to_string(),
                            observer_peer_id,
                            observed_peer_id,
                            chat_id: None,
                            level: ObservationLevel::Deductive,
                            content,
                            category: None,
                            confidence: 0.70,
                            source: "dreamer".to_string(),
                            source_ids: d.premise_ids,
                            message_ids: vec![],
                        })
                        .await?;
                    deductions_created += 1;
                }
            }
        }
    }

    // Phase 2: Induction
    if let Ok(response) = llm.complete(INDUCTION_PROMPT, &obs_text).await {
        let inductions: Vec<InductionResult> =
            serde_json::from_str(response.trim()).unwrap_or_default();
        for ind in inductions {
            if let Some(content) = crate::quality::normalize_content(&ind.content, 100) {
                if crate::quality::validate_observation(&content).is_ok() {
                    let confidence = match ind.confidence.as_str() {
                        "high" => 0.75,
                        "medium" => 0.60,
                        _ => 0.50,
                    };
                    store
                        .store_observation(NewObservation {
                            workspace: workspace.to_string(),
                            observer_peer_id,
                            observed_peer_id,
                            chat_id: None,
                            level: ObservationLevel::Inductive,
                            content,
                            category: None,
                            confidence,
                            source: "dreamer".to_string(),
                            source_ids: vec![],
                            message_ids: vec![],
                        })
                        .await?;
                    inductions_created += 1;
                }
            }
        }
    }

    // Phase 3: Contradiction Detection
    if let Ok(response) = llm.complete(CONTRADICTION_PROMPT, &obs_text).await {
        let contradictions: Vec<ContradictionResult> =
            serde_json::from_str(response.trim()).unwrap_or_default();
        for c in contradictions {
            if let Some(content) = crate::quality::normalize_content(&c.content, 100) {
                store
                    .store_observation(NewObservation {
                        workspace: workspace.to_string(),
                        observer_peer_id,
                        observed_peer_id,
                        chat_id: None,
                        level: ObservationLevel::Contradiction,
                        content,
                        category: None,
                        confidence: 0.90,
                        source: "dreamer".to_string(),
                        source_ids: c.conflicting_ids,
                        message_ids: vec![],
                    })
                    .await?;
                contradictions_found += 1;
            }
        }
    }

    // Phase 4: Consolidation — skip for now, requires semantic search
    let consolidated = 0i64;

    // Phase 5: Peer Card Update
    let peer_card_updated = update_peer_card_from_observations(
        store,
        observed_peer_id,
        &observations,
    )
    .await? as i64;

    let run = DreamerRun {
        workspace: workspace.to_string(),
        observer_peer_id: Some(observer_peer_id),
        observed_peer_id: Some(observed_peer_id),
        deductions_created,
        inductions_created,
        contradictions_found,
        consolidated,
        peer_card_updated,
        run_at: chrono::Utc::now().to_rfc3339(),
    };

    store.log_dreamer_run(run.clone()).await?;
    Ok(run)
}

async fn update_peer_card_from_observations(
    store: &dyn ObservationStore,
    peer_id: i64,
    observations: &[Observation],
) -> Result<bool> {
    // Extract top stable facts: high confidence, explicit level preferred
    let mut candidates: Vec<&Observation> = observations
        .iter()
        .filter(|o| !o.is_archived && o.confidence >= 0.60)
        .collect();

    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap()
            .then_with(|| b.times_derived.cmp(&a.times_derived))
    });

    let facts: Vec<String> = candidates
        .iter()
        .take(PeerCard::MAX_FACTS)
        .map(|o| o.content.clone())
        .collect();

    if facts.is_empty() {
        return Ok(false);
    }

    store.update_peer_card(peer_id, facts).await?;
    Ok(true)
}

fn format_observations_for_llm(observations: &[Observation]) -> String {
    observations
        .iter()
        .map(|o| format!("[id={}] [{}] {}", o.id, o.level.as_str().to_uppercase(), o.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn empty_run(workspace: &str, observer: i64, observed: i64) -> DreamerRun {
    DreamerRun {
        workspace: workspace.to_string(),
        observer_peer_id: Some(observer),
        observed_peer_id: Some(observed),
        deductions_created: 0,
        inductions_created: 0,
        contradictions_found: 0,
        consolidated: 0,
        peer_card_updated: 0,
        run_at: chrono::Utc::now().to_rfc3339(),
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p mchact-memory`
Expected: Compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-memory/src/dreamer.rs
git commit -m "feat(mchact-memory): dreamer agent with deduction, induction, and contradiction phases"
```

---

## Task 11: Legacy Migration + Queue Processor Stub

**Files:**
- Create: `crates/mchact-memory/src/migration.rs`
- Create: `crates/mchact-memory/src/queue.rs`

- [ ] **Step 1: Implement migration module**

```rust
// crates/mchact-memory/src/migration.rs

use crate::types::*;
use crate::{ObservationStore, Result};
use tracing::info;

/// Represents a legacy flat memory record from mchact-storage.
#[derive(Debug, Clone)]
pub struct LegacyMemory {
    pub id: i64,
    pub chat_id: Option<i64>,
    pub content: String,
    pub category: String,
    pub confidence: f64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Migrate legacy memories into the observation model.
/// Creates a legacy peer for each unique chat_id.
/// All memories become explicit-level observations.
pub async fn migrate_legacy_memories(
    store: &dyn ObservationStore,
    memories: Vec<LegacyMemory>,
    bot_peer_id: i64,
    workspace: &str,
) -> Result<usize> {
    let mut migrated = 0usize;

    // Group by chat_id to create peers
    let mut chat_peers: std::collections::HashMap<Option<i64>, i64> = std::collections::HashMap::new();

    for memory in &memories {
        let peer_id = if let Some(&cached_id) = chat_peers.get(&memory.chat_id) {
            cached_id
        } else {
            let peer_name = match memory.chat_id {
                Some(cid) => format!("legacy_chat_{cid}"),
                None => "legacy_global".to_string(),
            };
            let peer = store
                .get_or_create_peer(workspace, &peer_name, PeerKind::User)
                .await?;
            chat_peers.insert(memory.chat_id, peer.id);
            peer.id
        };

        store
            .store_observation(NewObservation {
                workspace: workspace.to_string(),
                observer_peer_id: bot_peer_id,
                observed_peer_id: peer_id,
                chat_id: memory.chat_id,
                level: ObservationLevel::Explicit,
                content: memory.content.clone(),
                category: Some(memory.category.clone()),
                confidence: memory.confidence,
                source: format!("migration:{}", memory.source),
                source_ids: vec![],
                message_ids: vec![],
            })
            .await?;
        migrated += 1;
    }

    info!("Migrated {migrated} legacy memories to observations");
    Ok(migrated)
}
```

- [ ] **Step 2: Implement queue processor stub**

```rust
// crates/mchact-memory/src/queue.rs

use crate::deriver::{self, LlmClient};
use crate::dreamer;
use crate::types::*;
use crate::{ObservationStore, Result};
use tracing::{info, warn};

/// Process pending tasks from the observation queue.
/// Called by the scheduler on an interval.
pub async fn process_queue(
    store: &dyn ObservationStore,
    llm: &dyn LlmClient,
    batch_size: usize,
) -> Result<usize> {
    let mut processed = 0usize;

    // Process derive tasks
    let derive_tasks = store.dequeue("derive", batch_size).await?;
    for task in derive_tasks {
        let messages_text = task
            .payload
            .as_ref()
            .and_then(|p| p.get("messages_text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let observer_id = task.observer_peer_id.unwrap_or(0);
        let observed_id = task.observed_peer_id.unwrap_or(0);

        if observer_id == 0 || observed_id == 0 {
            warn!("Skipping derive task {} — missing peer IDs", task.id);
            store.mark_processed(task.id).await?;
            continue;
        }

        match deriver::derive_observations(
            store,
            llm,
            observer_id,
            observed_id,
            task.chat_id,
            &task.workspace,
            messages_text,
        )
        .await
        {
            Ok((explicit, deductive)) => {
                info!(
                    "Derived {explicit} explicit + {deductive} deductive observations for task {}",
                    task.id
                );
            }
            Err(e) => {
                warn!("Deriver failed for task {}: {e}", task.id);
            }
        }
        store.mark_processed(task.id).await?;
        processed += 1;
    }

    // Process dream tasks
    let dream_tasks = store.dequeue("dream", batch_size).await?;
    for task in dream_tasks {
        let observer_id = task.observer_peer_id.unwrap_or(0);
        let observed_id = task.observed_peer_id.unwrap_or(0);

        if observer_id == 0 || observed_id == 0 {
            store.mark_processed(task.id).await?;
            continue;
        }

        match dreamer::run_dream_cycle(store, llm, observer_id, observed_id, &task.workspace).await
        {
            Ok(run) => {
                info!(
                    "Dream cycle: +{}d +{}i +{}c for task {}",
                    run.deductions_created,
                    run.inductions_created,
                    run.contradictions_found,
                    task.id
                );
            }
            Err(e) => {
                warn!("Dreamer failed for task {}: {e}", task.id);
            }
        }
        store.mark_processed(task.id).await?;
        processed += 1;
    }

    Ok(processed)
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p mchact-memory`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/mchact-memory/src/migration.rs crates/mchact-memory/src/queue.rs
git commit -m "feat(mchact-memory): legacy migration and queue processor"
```

---

## Task 12: PostgreSQL Driver (Stub + Schema Init)

**Files:**
- Create: `crates/mchact-memory/src/driver/postgres.rs`

This task creates the PostgreSQL driver structure. The full implementation mirrors sqlite.rs but uses sqlx. Due to the size, we implement the struct + schema init + a few key methods, leaving the rest as stubs that follow the same pattern.

- [ ] **Step 1: Create PgDriver with schema initialization**

```rust
// crates/mchact-memory/src/driver/postgres.rs

use sqlx::{PgPool, Row};

use crate::types::*;
use crate::{MemoryError, ObservationStore, Result};

pub struct PgDriver {
    pool: PgPool,
}

impl PgDriver {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        let driver = Self { pool };
        driver.initialize().await?;
        Ok(driver)
    }

    async fn initialize(&self) -> Result<()> {
        let schema = include_str!("../schema/postgres.sql");
        sqlx::raw_sql(schema)
            .execute(&self.pool)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    fn now_iso() -> String {
        chrono::Utc::now().to_rfc3339()
    }
}

#[async_trait::async_trait]
impl ObservationStore for PgDriver {
    async fn store_observation(&self, obs: NewObservation) -> Result<i64> {
        let source_ids_json = serde_json::to_value(&obs.source_ids)?;
        let message_ids_json = serde_json::to_value(&obs.message_ids)?;

        let row = sqlx::query(
            "INSERT INTO observations (workspace, observer_peer_id, observed_peer_id, chat_id,
             level, content, category, confidence, source, source_ids, message_ids)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             RETURNING id",
        )
        .bind(&obs.workspace)
        .bind(obs.observer_peer_id)
        .bind(obs.observed_peer_id)
        .bind(obs.chat_id)
        .bind(obs.level.as_str())
        .bind(&obs.content)
        .bind(&obs.category)
        .bind(obs.confidence)
        .bind(&obs.source)
        .bind(&source_ids_json)
        .bind(&message_ids_json)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(row.get("id"))
    }

    async fn get_observation(&self, id: i64) -> Result<Option<Observation>> {
        let row = sqlx::query(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id,
                    level, content, category, confidence, source, source_ids,
                    message_ids, times_derived, is_archived, archived_at,
                    created_at::text, updated_at::text
             FROM observations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(row.map(|r| pg_row_to_observation(&r)))
    }

    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<()> {
        // Fetch current, merge, write back (same pattern as SQLite)
        let current = self.get_observation(id).await?
            .ok_or_else(|| MemoryError::NotFound(format!("observation {id}")))?;

        let content = update.content.unwrap_or(current.content);
        let category = update.category.or(current.category);
        let confidence = update.confidence.unwrap_or(current.confidence);
        let source_ids = update.source_ids.unwrap_or(current.source_ids);
        let times_derived = update.times_derived.unwrap_or(current.times_derived);

        sqlx::query(
            "UPDATE observations SET content = $1, category = $2, confidence = $3,
             source_ids = $4, times_derived = $5, updated_at = NOW()
             WHERE id = $6",
        )
        .bind(&content)
        .bind(&category)
        .bind(confidence)
        .bind(serde_json::to_value(&source_ids)?)
        .bind(times_derived)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn archive_observation(&self, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE observations SET is_archived = TRUE, archived_at = NOW(), updated_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn get_or_create_peer(&self, workspace: &str, name: &str, kind: PeerKind) -> Result<Peer> {
        // Try insert, on conflict return existing
        let row = sqlx::query(
            "INSERT INTO peers (workspace, name, kind)
             VALUES ($1, $2, $3)
             ON CONFLICT (workspace, name) DO UPDATE SET updated_at = NOW()
             RETURNING id, workspace, name, kind, peer_card, metadata, created_at::text, updated_at::text",
        )
        .bind(workspace)
        .bind(name)
        .bind(kind.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(pg_row_to_peer(&row))
    }

    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>> {
        let row = sqlx::query("SELECT id, name, peer_card FROM peers WHERE id = $1")
            .bind(peer_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(row.and_then(|r| {
            let name: String = r.get("name");
            let card: Option<serde_json::Value> = r.get("peer_card");
            let facts: Vec<String> = card
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            if facts.is_empty() { None } else {
                Some(PeerCard { peer_id, peer_name: name, facts })
            }
        }))
    }

    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<()> {
        let truncated: Vec<String> = facts.into_iter().take(PeerCard::MAX_FACTS).collect();
        let json = serde_json::to_value(&truncated)?;
        sqlx::query("UPDATE peers SET peer_card = $1, updated_at = NOW() WHERE id = $2")
            .bind(&json)
            .bind(peer_id)
            .execute(&self.pool)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    // --- Remaining methods follow the same pattern as SQLite ---
    // Implementing search_hybrid, queue, findings, DAG, observability with sqlx queries.
    // The logic is identical; only SQL dialect changes.

    async fn search_hybrid(&self, query: &str, scope: SearchScope, limit: usize) -> Result<Vec<Observation>> {
        // PostgreSQL: use tsvector for keyword search
        let rows = sqlx::query(
            "SELECT id, workspace, observer_peer_id, observed_peer_id, chat_id,
                    level, content, category, confidence, source, source_ids,
                    message_ids, times_derived, is_archived, archived_at,
                    created_at::text, updated_at::text,
                    ts_rank(tsv, plainto_tsquery('english', $1)) AS rank
             FROM observations
             WHERE ($1 = '' OR tsv @@ plainto_tsquery('english', $1))
               AND workspace = $2
               AND ($3 OR is_archived = FALSE)
               AND confidence >= $4
             ORDER BY rank DESC
             LIMIT $5",
        )
        .bind(query)
        .bind(&scope.workspace)
        .bind(scope.include_archived)
        .bind(scope.min_confidence)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(rows.iter().map(pg_row_to_observation).collect())
    }

    async fn get_for_context(&self, chat_id: i64, user_query: &str, token_budget: usize) -> Result<Vec<Observation>> {
        let scope = SearchScope {
            workspace: "default".to_string(),
            chat_id: Some(chat_id),
            min_confidence: 0.45,
            ..Default::default()
        };
        let candidates = self.search_hybrid(user_query, scope, 100).await?;

        let mut tokens_used = 0usize;
        let mut result = Vec::new();
        for obs in candidates {
            let est = obs.content.len() / 4 + 10;
            if tokens_used + est > token_budget { break; }
            tokens_used += est;
            result.push(obs);
        }
        Ok(result)
    }

    async fn trace_reasoning(&self, observation_id: i64) -> Result<Vec<Observation>> {
        crate::dag::trace_reasoning(self, observation_id).await
    }

    async fn enqueue(&self, task: QueueTask) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO observation_queue (task_type, workspace, chat_id, observer_peer_id, observed_peer_id, payload)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(&task.task_type)
        .bind(&task.workspace)
        .bind(task.chat_id)
        .bind(task.observer_peer_id)
        .bind(task.observed_peer_id)
        .bind(task.payload.as_ref().map(|v| serde_json::to_value(v).ok()).flatten())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(row.get("id"))
    }

    async fn dequeue(&self, task_type: &str, limit: usize) -> Result<Vec<QueueItem>> {
        let rows = sqlx::query(
            "SELECT id, task_type, workspace, chat_id, observer_peer_id, observed_peer_id, payload, created_at::text
             FROM observation_queue WHERE task_type = $1 AND processed = FALSE
             ORDER BY id ASC LIMIT $2",
        )
        .bind(task_type)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MemoryError::Database(e.to_string()))?;

        Ok(rows.iter().map(|r| QueueItem {
            id: r.get("id"),
            task_type: r.get("task_type"),
            workspace: r.get("workspace"),
            chat_id: r.get("chat_id"),
            observer_peer_id: r.get("observer_peer_id"),
            observed_peer_id: r.get("observed_peer_id"),
            payload: r.get::<Option<serde_json::Value>, _>("payload"),
            created_at: r.get("created_at"),
        }).collect())
    }

    async fn mark_processed(&self, queue_id: i64) -> Result<()> {
        sqlx::query("UPDATE observation_queue SET processed = TRUE, processed_at = NOW() WHERE id = $1")
            .bind(queue_id)
            .execute(&self.pool)
            .await
            .map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn insert_finding(&self, orchestration_id: &str, run_id: &str, finding: &str, category: &str) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO findings (orchestration_id, run_id, finding, category) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(orchestration_id).bind(run_id).bind(finding).bind(category)
        .fetch_one(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(row.get("id"))
    }

    async fn get_findings(&self, orchestration_id: &str) -> Result<Vec<Finding>> {
        let rows = sqlx::query(
            "SELECT id, orchestration_id, run_id, finding, category, created_at::text FROM findings WHERE orchestration_id = $1 ORDER BY id",
        )
        .bind(orchestration_id)
        .fetch_all(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(rows.iter().map(|r| Finding {
            id: r.get("id"), orchestration_id: r.get("orchestration_id"),
            run_id: r.get("run_id"), finding: r.get("finding"),
            category: r.get("category"), created_at: r.get("created_at"),
        }).collect())
    }

    async fn delete_findings(&self, orchestration_id: &str) -> Result<usize> {
        let result = sqlx::query("DELETE FROM findings WHERE orchestration_id = $1")
            .bind(orchestration_id)
            .execute(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(result.rows_affected() as usize)
    }

    async fn upsert_embedding(&self, observation_id: i64, embedding: &[f32]) -> Result<()> {
        let vec_str = format!("[{}]", embedding.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","));
        sqlx::query("UPDATE observations SET embedding = $1::vector WHERE id = $2")
            .bind(&vec_str)
            .bind(observation_id)
            .execute(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn log_deriver_run(&self, run: DeriverRun) -> Result<()> {
        sqlx::query(
            "INSERT INTO deriver_runs (chat_id, workspace, started_at, finished_at, messages_processed, explicit_count, deductive_count, skipped_count, error_text)
             VALUES ($1, $2, $3::timestamptz, $4::timestamptz, $5, $6, $7, $8, $9)",
        )
        .bind(run.chat_id).bind(&run.workspace).bind(&run.started_at).bind(&run.finished_at)
        .bind(run.messages_processed).bind(run.explicit_count).bind(run.deductive_count)
        .bind(run.skipped_count).bind(&run.error_text)
        .execute(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn log_dreamer_run(&self, run: DreamerRun) -> Result<()> {
        sqlx::query(
            "INSERT INTO dreamer_runs (workspace, observer_peer_id, observed_peer_id, deductions_created, inductions_created, contradictions_found, consolidated, peer_card_updated, run_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz)",
        )
        .bind(&run.workspace).bind(run.observer_peer_id).bind(run.observed_peer_id)
        .bind(run.deductions_created).bind(run.inductions_created).bind(run.contradictions_found)
        .bind(run.consolidated).bind(run.peer_card_updated).bind(&run.run_at)
        .execute(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }

    async fn log_injection(&self, log: InjectionLog) -> Result<()> {
        sqlx::query(
            "INSERT INTO injection_logs (chat_id, retrieval_method, candidate_count, selected_count, omitted_count, tokens_est)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(log.chat_id).bind(&log.retrieval_method).bind(log.candidate_count)
        .bind(log.selected_count).bind(log.omitted_count).bind(log.tokens_est)
        .execute(&self.pool).await.map_err(|e| MemoryError::Database(e.to_string()))?;
        Ok(())
    }
}

fn pg_row_to_observation(row: &sqlx::postgres::PgRow) -> Observation {
    let level_str: String = row.get("level");
    let source_ids_val: Option<serde_json::Value> = row.get("source_ids");
    let message_ids_val: Option<serde_json::Value> = row.get("message_ids");

    Observation {
        id: row.get("id"),
        workspace: row.get("workspace"),
        observer_peer_id: row.get("observer_peer_id"),
        observed_peer_id: row.get("observed_peer_id"),
        chat_id: row.get("chat_id"),
        level: ObservationLevel::from_str(&level_str).unwrap_or(ObservationLevel::Explicit),
        content: row.get("content"),
        category: row.get("category"),
        confidence: row.get("confidence"),
        source: row.get("source"),
        source_ids: source_ids_val
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        message_ids: message_ids_val
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        times_derived: row.get("times_derived"),
        is_archived: row.get("is_archived"),
        archived_at: row.get("archived_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn pg_row_to_peer(row: &sqlx::postgres::PgRow) -> Peer {
    let kind_str: String = row.get("kind");
    let card_val: Option<serde_json::Value> = row.get("peer_card");
    Peer {
        id: row.get("id"),
        workspace: row.get("workspace"),
        name: row.get("name"),
        kind: PeerKind::from_str(&kind_str).unwrap_or(PeerKind::User),
        peer_card: card_val.and_then(|v| serde_json::from_value(v).ok()),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
```

- [ ] **Step 2: Verify it compiles with postgres feature**

Run: `cargo check -p mchact-memory --features postgres --no-default-features`
Expected: Compiles (may have warnings about unused imports if sqlite feature not active).

- [ ] **Step 3: Commit**

```bash
git add crates/mchact-memory/src/driver/postgres.rs
git commit -m "feat(mchact-memory): PostgreSQL driver with pgvector and tsvector support"
```

---

## Task 13: Driver Selection + Config Integration

**Files:**
- Modify: `crates/mchact-memory/src/driver/mod.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Implement driver factory in driver/mod.rs**

```rust
// crates/mchact-memory/src/driver/mod.rs

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

use crate::{MemoryError, ObservationStore, Result};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

fn default_backend() -> String {
    "sqlite".to_string()
}

fn default_db_path() -> String {
    "./mchact.data/memory.db".to_string()
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            db_path: default_db_path(),
            database_url: None,
        }
    }
}

/// Create an ObservationStore from config.
/// Returns None if the backend is unavailable (fallback: no memory).
pub async fn create_store(config: &MemoryConfig) -> Option<Arc<dyn ObservationStore>> {
    match config.backend.as_str() {
        #[cfg(feature = "sqlite")]
        "sqlite" => {
            let path = std::path::Path::new(&config.db_path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match sqlite::SqliteDriver::open(path) {
                Ok(driver) => {
                    tracing::info!("mchact-memory: SQLite driver initialized at {}", config.db_path);
                    Some(Arc::new(driver))
                }
                Err(e) => {
                    tracing::warn!("mchact-memory: SQLite init failed: {e}. Running without memory.");
                    None
                }
            }
        }
        #[cfg(feature = "postgres")]
        "postgres" => {
            let url = config.database_url.as_deref().unwrap_or("postgres://localhost/mchact");
            match postgres::PgDriver::connect(url).await {
                Ok(driver) => {
                    tracing::info!("mchact-memory: PostgreSQL driver initialized");
                    Some(Arc::new(driver))
                }
                Err(e) => {
                    tracing::warn!("mchact-memory: PostgreSQL init failed: {e}. Running without memory.");
                    None
                }
            }
        }
        other => {
            tracing::warn!("mchact-memory: unknown backend '{other}'. Running without memory.");
            None
        }
    }
}
```

- [ ] **Step 2: Add MemoryConfig to src/config.rs**

In `src/config.rs`, add to the `Config` struct:

```rust
    #[serde(default)]
    pub memory: mchact_memory::driver::MemoryConfig,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/mchact-memory/src/driver/mod.rs src/config.rs
git commit -m "feat(mchact-memory): driver factory and config integration"
```

---

## Task 14: Runtime Integration (Wire into AppState)

**Files:**
- Modify: `src/runtime.rs`

This task wires the `ObservationStore` into `AppState` alongside the existing `MemoryBackend`. The existing `MemoryBackend` is not removed yet — both coexist during migration.

- [ ] **Step 1: Add observation_store to AppState**

In `src/runtime.rs`, add to the `AppState` struct:

```rust
    pub observation_store: Option<Arc<dyn mchact_memory::ObservationStore>>,
```

- [ ] **Step 2: Initialize in the run() function**

In `src/runtime.rs`, within the `run()` function where `MemoryBackend` is created, add after it:

```rust
    let observation_store = mchact_memory::driver::create_store(&config.memory).await;
    if observation_store.is_some() {
        info!("mchact-memory observation store initialized");
    }
```

And pass it to `AppState`:

```rust
    observation_store,
```

- [ ] **Step 3: Verify it compiles and runs**

Run: `cargo build`
Expected: Compiles. The observation store initializes at startup alongside existing memory.

- [ ] **Step 4: Commit**

```bash
git add src/runtime.rs
git commit -m "feat: wire ObservationStore into AppState alongside existing MemoryBackend"
```

---

## Summary

**Tasks 1-8**: Build the standalone `mchact-memory` crate with types, quality gates, schemas, SQLite driver (full CRUD, queue, findings, observability, FTS5 search, DAG traversal), and memory injection formatter.

**Tasks 9-11**: Build the deriver agent, dreamer agent, legacy migration, and queue processor.

**Task 12**: Build the PostgreSQL driver (mirrors SQLite driver with sqlx/pgvector/tsvector).

**Tasks 13-14**: Wire everything into mchact's config and runtime.

**Not covered in this plan (future tasks):**
- Replace `MemoryProvider` usage in `agent_engine.rs`, `memory_service.rs`, `structured_memory.rs`, `findings.rs` with `ObservationStore`
- Replace reflector loop in `scheduler.rs` with deriver + dreamer loops
- Deprecate file memory tools and add peer card tools
- sqlite-vec embedding integration in SQLite driver
- PostgreSQL integration tests (requires running PostgreSQL)
- Findings promotion from MoA orchestration

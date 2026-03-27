# mchact-memory: Honcho-Inspired Observation Engine

**Date:** 2026-03-27
**Status:** Draft
**Scope:** New crate `crates/mchact-memory/` + integration into existing microclaw runtime

## Problem

MicroClaw's memory system has three fundamental gaps compared to Honcho and Hermes:

1. **Flat memory model.** Three categories (PROFILE/KNOWLEDGE/EVENT) with no reasoning depth. No distinction between direct facts, logical inferences, discovered patterns, or contradictions. The reflector extracts facts but cannot reason about them.

2. **No identity model.** Memory is keyed by `chat_id` only. No concept of "who observed what about whom." Cannot model agent-to-agent knowledge or user profiles across channels.

3. **No offline consolidation.** The reflector extracts facts on a timer, but never revisits them. No pattern discovery, no contradiction detection, no pruning of stale inferences. Memory grows but never matures.

## Solution

A new Rust crate `mchact-memory` that implements Honcho's observation hierarchy, Hermes' production patterns, and microclaw's existing infrastructure — behind a single `ObservationStore` trait with two interchangeable drivers: SQLite (sqlite-vec + FTS5) and PostgreSQL (pgvector + tsvector).

Both drivers implement the **full feature set**. No degraded mode. Config picks the driver.

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│              mchact application layer                 │
│  agent_engine / deriver / dreamer / tools / scheduler │
└──────────────────────┬───────────────────────────────┘
                       │
            ┌──────────▼──────────┐
            │   ObservationStore  │  One trait, full features
            │   (async trait)     │  on BOTH backends
            └─────┬─────────┬────┘
                  │         │
        ┌─────────▼──┐  ┌──▼───────────┐
        │  SQLite     │  │  PostgreSQL   │
        │  Driver     │  │  Driver       │
        │             │  │               │
        │ rusqlite    │  │ sqlx          │
        │ sqlite-vec  │  │ pgvector      │
        │ FTS5        │  │ tsvector+GIN  │
        └─────────────┘  └───────────────┘
```

**Config:**
```yaml
memory:
  backend: "sqlite"           # or "postgres"
  # SQLite options
  db_path: "./mchact.data/memory.db"
  # PostgreSQL options
  # database_url: "postgres://user:pass@host/db"
```

**Fallback:** If the memory database is unavailable at startup, the agent runs with **no memory**. No silent degradation, no partial state.

---

## Data Model

### Peers

Unified identity for users and agents. Replaces chat_id-only keying.

```sql
CREATE TABLE peers (
    id            INTEGER PRIMARY KEY,  -- BIGSERIAL on PG
    workspace     TEXT NOT NULL DEFAULT 'default',
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL DEFAULT 'user',  -- 'user' | 'agent'
    peer_card     TEXT,       -- JSON array, max 40 durable biographical facts
    metadata      TEXT,       -- JSON, flexible config
    created_at    TEXT NOT NULL,  -- TIMESTAMPTZ on PG
    updated_at    TEXT NOT NULL,
    UNIQUE(workspace, name)
);
```

**Peer resolution:** On first message from a user, auto-create a peer from `(channel, external_user_id)` — not display name (display names change, external IDs are stable). The bot itself is a peer with `kind='agent'`.

**Peer card:** Max 40 durable facts distilled by the dreamer. Replaces AGENTS.md per-chat file sections. Injected into system prompt as `<peer_card>` block.

### Observations

Replaces the flat `memories` table. Four levels of reasoning depth.

```sql
CREATE TABLE observations (
    id                INTEGER PRIMARY KEY,  -- BIGSERIAL on PG
    workspace         TEXT NOT NULL DEFAULT 'default',
    observer_peer_id  INTEGER NOT NULL REFERENCES peers(id),
    observed_peer_id  INTEGER NOT NULL REFERENCES peers(id),
    chat_id           INTEGER,
    level             TEXT NOT NULL,  -- 'explicit' | 'deductive' | 'inductive' | 'contradiction'
    content           TEXT NOT NULL,
    category          TEXT,           -- PROFILE | KNOWLEDGE | EVENT (backward compat)
    confidence        REAL NOT NULL DEFAULT 0.8,
    source            TEXT NOT NULL DEFAULT 'deriver',
    source_ids        TEXT,           -- JSON array of premise observation IDs (DAG)
    message_ids       TEXT,           -- JSON array of source message IDs
    times_derived     INTEGER DEFAULT 0,
    is_archived       INTEGER DEFAULT 0,  -- BOOLEAN on PG
    archived_at       TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);
```

**Observation levels:**

| Level | What it captures | Source | Example |
|-------|-----------------|--------|---------|
| `explicit` | Direct facts stated by user | Deriver extraction or explicit remember | "I'm allergic to peanuts" |
| `deductive` | Logical inferences from premises | Deriver or dreamer deduction specialist | "Needs to check peanut content at Thai restaurants" (from: peanut allergy + loves Thai food) |
| `inductive` | Patterns across multiple observations | Dreamer induction specialist | "Health-conscious and detail-oriented about food" (from 5+ food-related observations) |
| `contradiction` | Conflicting statements detected | Dreamer contradiction detection | "Conflicting views on collaboration: 'prefers working alone' vs 'loves pair programming'" |

**Source attribution DAG:** `source_ids` is a JSON array of observation IDs that served as premises. Forms a directed acyclic graph. `trace_reasoning(obs_id)` traverses the DAG to show how a conclusion was reached.

**Confidence scoring (preserved from microclaw):**
- Explicit remember: 0.95
- Deriver extraction (explicit level): 0.85
- Deriver extraction (deductive level): 0.70
- Dreamer induction: 0.60
- Dreamer deduction: 0.70
- Tool update: 0.80
- Archive threshold: 0.35

### Vectors

**SQLite driver:**
```sql
CREATE VIRTUAL TABLE observation_vec USING vec0(embedding float[1536]);
```

**PostgreSQL driver:**
```sql
-- Column on observations table
embedding vector(1536),

-- HNSW index
CREATE INDEX idx_obs_embedding ON observations
    USING hnsw (embedding vector_cosine_ops);
```

Embedding dimension configurable via `memory.embedding_dim` (default 1536 for OpenAI text-embedding-3-small). Both drivers use the same configured dimension. Embedding provider reuses microclaw's existing `embedding_provider` / `embedding_api_key` / `embedding_model` config.

### Keyword Search Index

**SQLite driver:**
```sql
CREATE VIRTUAL TABLE observations_fts USING fts5(
    content, content='observations', content_rowid='id'
);
-- Auto-sync triggers on INSERT/UPDATE/DELETE
```

**PostgreSQL driver:**
```sql
-- Generated column on observations table
tsv tsvector GENERATED ALWAYS AS (to_tsvector('english', content)) STORED;

CREATE INDEX idx_obs_tsv ON observations USING GIN (tsv);
```

### Background Task Queue

```sql
CREATE TABLE observation_queue (
    id                INTEGER PRIMARY KEY,
    task_type         TEXT NOT NULL,   -- 'derive' | 'dream' | 'consolidate'
    workspace         TEXT NOT NULL DEFAULT 'default',
    chat_id           INTEGER,
    observer_peer_id  INTEGER,
    observed_peer_id  INTEGER,
    payload           TEXT,            -- JSON, task-specific data
    processed         INTEGER DEFAULT 0,
    created_at        TEXT NOT NULL,
    processed_at      TEXT
);
```

### MoA Findings

Moved from microclaw-storage into mchact-memory.

```sql
CREATE TABLE findings (
    id                INTEGER PRIMARY KEY,
    orchestration_id  TEXT NOT NULL,
    run_id            TEXT NOT NULL,
    finding           TEXT NOT NULL,
    category          TEXT DEFAULT 'general',
    created_at        TEXT NOT NULL
);

CREATE INDEX idx_findings_orch ON findings(orchestration_id);
```

### Observability

```sql
CREATE TABLE deriver_runs (
    id               INTEGER PRIMARY KEY,
    chat_id          INTEGER,
    workspace        TEXT,
    started_at       TEXT NOT NULL,
    finished_at      TEXT NOT NULL,
    messages_processed INTEGER DEFAULT 0,
    explicit_count   INTEGER DEFAULT 0,
    deductive_count  INTEGER DEFAULT 0,
    skipped_count    INTEGER DEFAULT 0,
    error_text       TEXT
);

CREATE TABLE dreamer_runs (
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

CREATE TABLE injection_logs (
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

---

## Core Trait

```rust
#[async_trait]
pub trait ObservationStore: Send + Sync {
    // --- Core CRUD ---
    async fn store_observation(&self, obs: NewObservation) -> Result<i64>;
    async fn get_observation(&self, id: i64) -> Result<Option<Observation>>;
    async fn update_observation(&self, id: i64, update: ObservationUpdate) -> Result<()>;
    async fn archive_observation(&self, id: i64) -> Result<()>;

    // --- Search ---
    async fn search_hybrid(&self, query: &str, scope: SearchScope, limit: usize) -> Result<Vec<Observation>>;
    async fn get_for_context(&self, chat_id: i64, user_query: &str, token_budget: usize) -> Result<Vec<Observation>>;

    // --- Peers ---
    async fn get_or_create_peer(&self, workspace: &str, name: &str, kind: PeerKind) -> Result<Peer>;
    async fn get_peer_card(&self, peer_id: i64) -> Result<Option<PeerCard>>;
    async fn update_peer_card(&self, peer_id: i64, facts: Vec<String>) -> Result<()>;

    // --- DAG ---
    async fn trace_reasoning(&self, observation_id: i64) -> Result<Vec<Observation>>;

    // --- Queue ---
    async fn enqueue(&self, task: QueueTask) -> Result<i64>;
    async fn dequeue(&self, task_type: &str, limit: usize) -> Result<Vec<QueueItem>>;
    async fn mark_processed(&self, queue_id: i64) -> Result<()>;

    // --- Findings ---
    async fn insert_finding(&self, orchestration_id: &str, run_id: &str, finding: &str, category: &str) -> Result<i64>;
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

Both `SqliteDriver` and `PgDriver` implement this trait fully.

---

## Hybrid Search (RRF)

Both drivers implement the same Reciprocal Rank Fusion merge in Rust. Only the SQL differs.

**Algorithm:**

1. **Keyword arm:** FTS5 `MATCH` (SQLite) or `tsv @@ plainto_tsquery()` (PostgreSQL). Returns top 20 ranked results.

2. **Semantic arm:** sqlite-vec distance query (SQLite) or `embedding <=> query_vec` (PostgreSQL). Returns top 20 by cosine similarity.

3. **RRF merge (shared Rust code):**
   ```rust
   let k = 60.0;
   for result in keyword_results.union(semantic_results) {
       let score = 1.0 / (k + keyword_rank.unwrap_or(1000.0))
                 + 1.0 / (k + semantic_rank.unwrap_or(1000.0));
       merged.push((result, score));
   }
   merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
   ```

4. Return top N merged results with full observation metadata.

**Fallback:** If embedding provider is unavailable, fall back to keyword-only search. Log degradation.

---

## Deriver Agent

Replaces the current reflector loop. Extracts multi-level observations from conversations.

**Trigger:** When new messages arrive, enqueue a `derive` task in `observation_queue`.

**Processing (deriver.rs):**

1. Dequeue batch of messages for same `(chat_id, observed_peer)`.
2. Resolve peers: auto-create from `(channel, external_user_id)` if new.
3. Call LLM (via microclaw's existing provider, using auxiliary/cheap model if configured) with deriver prompt:
   ```
   Extract observations from this conversation segment.
   Output JSON array:
   [
     {
       "content": "...",
       "level": "explicit|deductive",
       "category": "PROFILE|KNOWLEDGE|EVENT",
       "source_message_ids": [...],
       "premises": [...]  // for deductive: which extracted facts support this
     }
   ]
   Rules:
   - explicit: facts directly stated by the user
   - deductive: logical inferences from 2+ explicit facts
   - max 100 chars per observation
   - high bar: specific, durable facts only
   - skip: tool errors, broken behavior, temporary state
   ```
4. Quality gate each extracted observation (reuse + extend `memory_quality.rs`):
   - Length check (>= 8 chars)
   - No vague/uncertain language
   - PII/secrets scan (from Hermes pattern)
   - Poisoning guard (skip broken-behavior facts)
5. Dedup against existing observations:
   - Exact match → touch `last_seen_at`
   - Semantic near-match (cosine < 0.15) → update if incoming is more detailed
   - Topic conflict (same topic key, different content) → supersede via DAG
   - No match → insert new
6. Auto-embed new observations.
7. Update peer card if stable biographical fact extracted.
8. Log `deriver_runs` for observability.

**Confidence assignments:**
- Explicit extraction: 0.85
- Deductive extraction: 0.70

---

## Dreamer Agent

Offline consolidation and pattern discovery. Runs on schedule (configurable, default every 2 hours). Only processes peers with sufficient observation history.

**Processing (dreamer.rs):**

### Phase 1: Deduction Specialist
- Fetch recent explicit observations for a peer.
- LLM prompt: "Given these facts, what can you logically infer?"
- Output: deductive observations with `source_ids` linking to premises.
- Confidence: 0.70

### Phase 2: Induction Specialist
- Fetch all observations for peer (explicit + deductive).
- LLM prompt: "What patterns do you see across these observations?"
- Output: inductive observations with pattern type (preference/behavior/personality/tendency), confidence level (high/medium/low).
- Optional: surprisal sampling — use geometric surprisal scores to focus on "interesting" observations (from Honcho).
- Confidence: 0.60

### Phase 3: Contradiction Detection
- Compare new observations against existing for same peer.
- LLM prompt: "Do any of these contradict each other?"
- Output: contradiction observations with `source_ids` linking to the conflicting pair.
- Confidence: 0.90 (contradictions are high-signal)

### Phase 4: Consolidation
- Find near-duplicate observations (cosine similarity < 0.10).
- Keep most detailed version, archive others.
- Increment `times_derived` on consolidated observations.

### Phase 5: Peer Card Update
- Extract top stable facts from all active observations.
- Update `peers.peer_card` (max 40 facts).
- Prioritize: high confidence, frequently derived, explicit level.

**Log `dreamer_runs` for observability.**

---

## Memory Injection

Builds the memory context block for the system prompt.

**Flow (injection.rs):**

1. Fetch peer card for current user → format as `<peer_card>` block.
2. Fetch ranked observations via `get_for_context()`:
   - RRF hybrid search against user's current query.
   - Filter: `is_archived = false`, confidence >= 0.45.
   - Obey token budget (default from config `memory_token_budget`).
3. Format observations with level labels:
   ```xml
   <peer_card peer="alice">
   Backend developer, Rust specialist, allergic to peanuts, ...
   </peer_card>

   <observations>
   [EXPLICIT] alice is a backend dev who prefers Rust
   [DEDUCTIVE] likely familiar with tokio/async (from: #5)
   [INDUCTIVE] health-conscious, detail-oriented about food (confidence: medium)
   [CONTRADICTION] conflicting views on solo vs pair work (see: #15, #22)
   </observations>
   ```
4. Log injection event for observability.

**Frozen snapshot pattern (from Hermes):** Load observations once at session start. Mid-session observation writes go to disk immediately but don't re-inject until next session. This preserves prompt prefix cache across the agentic loop.

---

## Findings Promotion (mchact Innovation)

When MoA orchestration completes, valuable findings can be promoted to long-term observations.

**Flow:**
1. `subagents_orchestrate` completes → findings exist in `findings` table.
2. Promotion heuristic (in deriver or reflector):
   - Findings with high relevance to ongoing work.
   - Cross-worker agreements (same finding from 2+ workers).
3. Promoted findings stored as `explicit` observations with `source='findings_promotion'`.
4. Original findings cleaned up after orchestration.

---

## Migration: Legacy Memories to Observations

One-time migration when upgrading from microclaw flat memory.

**Script (migration.rs):**

1. Read all records from `memories` table.
2. For each memory:
   - Create peer from `chat_id` (auto-resolve or create `legacy_chat_{id}` peer).
   - Map to observation: `level='explicit'`, preserve `category`, `confidence`, `source`.
   - `source_ids = []` (no DAG for legacy data).
3. Migrate `memory_supersede_edges` → set `source_ids` on successor observations.
4. Migrate `subagent_findings` → `findings` table.
5. Migrate `memory_reflector_runs` → `deriver_runs`.
6. Migrate `memory_injection_logs` → `injection_logs`.
7. Mark legacy tables as migrated (don't delete — keep for rollback).

---

## Crate Structure

```
crates/mchact-memory/
├── Cargo.toml
│   [features]
│   default = ["sqlite"]
│   sqlite  = ["rusqlite/bundled", "sqlite-vec"]
│   postgres = ["sqlx/postgres", "pgvector"]
│
├── src/
│   ├── lib.rs              # ObservationStore trait + public API
│   ├── types.rs            # Observation, Peer, PeerCard, ObservationLevel,
│   │                       # NewObservation, SearchScope, QueueTask, Finding, etc.
│   │
│   ├── driver/
│   │   ├── mod.rs          # Driver selection from config
│   │   ├── sqlite.rs       # impl ObservationStore for SqliteDriver
│   │   └── postgres.rs     # impl ObservationStore for PgDriver
│   │
│   ├── search.rs           # RRF hybrid merge (shared Rust logic)
│   ├── deriver.rs          # Multi-level observation extraction
│   ├── dreamer.rs          # Offline consolidation + induction
│   ├── dag.rs              # Source attribution DAG traversal
│   ├── quality.rs          # Quality gates + normalization + PII scan
│   ├── injection.rs        # Build memory context for prompts
│   ├── queue.rs            # Background task queue processing
│   ├── migration.rs        # Legacy memories → observations
│   └── schema/
│       ├── sqlite.sql      # Full SQLite DDL
│       └── postgres.sql    # Full PostgreSQL DDL
```

---

## Integration Points

### Replaces

| Current code | Replaced by |
|-------------|-------------|
| `MemoryProvider` trait in `memory_backend.rs` | `ObservationStore` trait |
| `SqliteMemoryProvider` | `SqliteDriver` |
| `McpMemoryProvider` | Can wrap ObservationStore as MCP server (future) |
| `FallbackMemoryProvider` | Fallback = no memory (clean failure) |
| Reflector loop in `scheduler.rs` | Deriver loop in `mchact-memory::deriver` |
| `memory_reflector_runs` table | `deriver_runs` table |
| `memory_injection_logs` table | `injection_logs` table |
| `memories` table | `observations` table |
| `memory_supersede_edges` table | `observations.source_ids` (DAG) |
| `subagent_findings` in microclaw-storage | `findings` in mchact-memory |
| AGENTS.md file memory | Peer cards |

### Unchanged

- `messages` table and FTS5 index (microclaw-storage) — stays in SQLite
- `sessions` table — stays in SQLite
- Auth / API keys — stays in SQLite
- Scheduled tasks — stays in SQLite
- 5-phase context compressor — untouched
- `session_search` tool — untouched (searches messages, not observations)
- `mixture_of_agents` tool — untouched (uses findings via mchact-memory)
- Channel adapters — untouched
- LLM provider layer — untouched (deriver/dreamer proxy through it)

### Modified

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `crates/mchact-memory` to members |
| `src/runtime.rs` | Initialize `ObservationStore` from config, replace `MemoryBackend` |
| `src/agent_engine.rs` | Use `injection::build_memory_context()` instead of old memory injection |
| `src/scheduler.rs` | Replace reflector loop with deriver + dreamer loops |
| `src/tools/structured_memory.rs` | Query `ObservationStore` instead of `MemoryProvider` |
| `src/tools/memory.rs` | Deprecate file memory tools, add peer card tools |
| `src/tools/findings.rs` | Use `ObservationStore::insert_finding()` etc. |
| `src/memory_service.rs` | Explicit remember fast path uses `ObservationStore::store_observation()` |
| `src/config.rs` | Add `memory` config section |

---

## Testing Strategy

### Unit tests
- `quality.rs` — normalization, rejection criteria, PII scan
- `search.rs` — RRF merge logic with mock ranked lists
- `dag.rs` — DAG traversal with cycles/missing nodes
- `types.rs` — serialization roundtrips
- `migration.rs` — legacy memory → observation mapping

### Integration tests (per driver)
- Store + retrieve observations across all 4 levels
- Peer CRUD + peer card updates
- Hybrid search: keyword-only, semantic-only, RRF merged
- Queue: enqueue, dequeue, mark processed
- Findings: insert, read, delete by orchestration_id
- FTS triggers (SQLite) / tsvector (PostgreSQL)
- Vector upsert + KNN retrieval

### Agent tests
- Deriver: message batch → extracted observations (mock LLM)
- Dreamer: observation set → deductions + inductions + contradictions (mock LLM)
- Injection: observations → formatted prompt block within token budget
- Findings promotion: MoA findings → observations

---

## Risks

| Risk | Mitigation |
|------|------------|
| sqlite-vec not compiled | Feature flag `sqlite` includes it. Doctor check at startup. |
| PostgreSQL unavailable at startup | Fall back to no memory with warning. Agent works without context. |
| Deriver LLM extraction quality | Quality gates + poisoning guard. Deriver confidence (0.85/0.70) lower than explicit (0.95). |
| Dreamer produces low-quality inductions | Inductive observations get lowest confidence (0.60). Below injection threshold (0.45) if not validated. |
| Cross-DB: messages in SQLite, observations in PG | Deriver reads messages via existing methods, writes to PG. Queue makes this idempotent. |
| Legacy migration data loss | Migration preserves all fields. Legacy tables kept for rollback. |
| Dual driver maintenance burden | Same trait, same tests parameterized over both drivers. Schema SQL is the only divergence. |
| Peer auto-creation duplicates | Map by `(channel, external_user_id)` not display name. Stable IDs. |

---

## Future Enhancements (out of scope)

- Dialectic agent (NL query about a peer, agentic recall via tools)
- MCP server wrapping ObservationStore (expose memory as MCP tools for external agents)
- Full PostgreSQL migration (move messages, sessions, auth to PG)
- Multi-workspace support (for clawteam multi-tenant scenarios)
- Observation TTL / automatic archival of stale low-confidence observations

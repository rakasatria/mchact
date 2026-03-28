# Storage Unification Design

**Date:** 2026-03-28
**Status:** Draft
**Scope:** Wire all data paths through unified Database backend and ObjectStorage backend

## Problem

The codebase has three categories of broken wiring:

1. **Database backend**: `config.db_backend` and `config.db_database_url` are defined but never read. `AppState.db` is hardcoded to `Arc<Database>` (SQLite). The full PostgreSQL driver (`PgDriver`) with all 11 `DataStore` sub-trait implementations is dead code.

2. **ObjectStorage backend**: Three code paths create their own `LocalStorage` instances, bypassing the configured backend (S3/Azure/GCS):
   - ACP server (`src/acp.rs:63-72`)
   - Web upload endpoint (`src/web.rs:2016-2023`)
   - ToolRegistry constructor (`src/tools/mod.rs:127-131`)
   - Two channel adapters bypass `MediaManager.store_file()` for inbound files

3. **Direct filesystem operations**: 12 data paths use `std::fs` / `tokio::fs` directly instead of ObjectStorage, making them invisible when cloud storage is configured.

4. **Vector search**: PgDriver's `prepare_vector_index`, `upsert_memory_vec`, `knn_memories` are no-ops. Postgres users get no semantic memory search.

## Design

### Part 1: Unified vector feature gate

Rename Cargo feature `sqlite-vec` → `vector-search` across the workspace.

Behind `#[cfg(feature = "vector-search")]`:
- SQLite backend: uses `sqlite-vec` crate (existing behavior, unchanged)
- PostgreSQL backend: uses `pgvector` extension (new)

**Files changed:**
- `Cargo.toml` (root): rename feature `sqlite-vec` → `vector-search`, keep `sqlite-vec` crate as dependency behind this feature
- `crates/mchact-storage/Cargo.toml`: same rename
- All `#[cfg(feature = "sqlite-vec")]` annotations (~20 sites across `src/` and `crates/mchact-storage/`) → `#[cfg(feature = "vector-search")]`

### Part 2: pgvector in PgDriver

**Schema** (`crates/mchact-storage/src/schema/postgres.sql`):
- Add `CREATE EXTENSION IF NOT EXISTS vector;` at top
- Add column to `memories` table: `embedding vector` (no fixed dimension — set dynamically)
- Add HNSW index: `CREATE INDEX IF NOT EXISTS idx_memories_embedding ON memories USING hnsw (embedding vector_cosine_ops)`

**PgDriver methods** (`crates/mchact-storage/src/driver/postgres/memory_db.rs`):

- `prepare_vector_index(dimension)`:
  - `ALTER TABLE memories ADD COLUMN IF NOT EXISTS embedding vector($1)`
  - Create HNSW index if not exists
  - No-op if column already exists with correct dimension

- `upsert_memory_vec(memory_id, &[f32])`:
  - `UPDATE memories SET embedding = $1 WHERE id = $2`
  - Convert `&[f32]` to pgvector format

- `knn_memories(chat_id, query_vec, k)`:
  - `SELECT id, (embedding <=> $1) AS distance FROM memories WHERE chat_id = $2 AND is_archived = FALSE AND embedding IS NOT NULL ORDER BY embedding <=> $1 LIMIT $3`
  - Return `Vec<(i64, f32)>` — same interface as SQLite

**Dependency**: Add `pgvector` crate to `crates/mchact-storage/Cargo.toml` behind `postgres` + `vector-search` features.

### Part 3: Database backend wiring (`Arc<dyn DataStore>`)

**Type alias** (in `crates/mchact-storage/src/lib.rs`):
```rust
pub type DynDataStore = dyn DataStore + Send + Sync;
```

**Trait bounds**: Add `Send + Sync` supertrait to `DataStore` and all 11 sub-traits in `crates/mchact-storage/src/traits/`.

**`call_blocking` signature change** (`crates/mchact-storage/src/db/mod.rs`):
```rust
// Before
pub async fn call_blocking<T, F>(db: Arc<Database>, f: F) -> Result<T, MchactError>
where F: FnOnce(&Database) -> Result<T, MchactError> + Send + 'static

// After
pub async fn call_blocking<T, F>(db: Arc<DynDataStore>, f: F) -> Result<T, MchactError>
where F: FnOnce(&DynDataStore) -> Result<T, MchactError> + Send + 'static
```

**`AppState.db`** (`src/runtime.rs:54`):
```rust
// Before
pub db: Arc<Database>,
// After
pub db: Arc<DynDataStore>,
```

**Factory usage** (`src/main.rs`):
```rust
// Before
let db = db::Database::new(&runtime_data_dir)?;

// After
use mchact_storage::driver::{StorageDriverConfig, create_data_store};
let driver_config = StorageDriverConfig {
    backend: config.db_backend.clone(),
    db_path: runtime_data_dir.clone(),
    database_url: config.db_database_url.clone(),
};
let db = create_data_store(&driver_config)
    .await
    .unwrap_or_else(|| panic!(
        "Failed to initialize '{}' database backend", config.db_backend
    ));
```

**Bulk type change**: All ~102 occurrences of `Arc<Database>` across 22 files in `src/` → `Arc<DynDataStore>`. Includes:
- All tool structs (`src/tools/schedule.rs`, `subagents.rs`, `memory.rs`, `findings.rs`, etc.)
- `MediaManager` (`src/media_manager.rs`)
- `MemoryBackend` (`src/memory_backend.rs`)
- `KnowledgeManager` (`src/knowledge.rs`)
- `HookManager` (`src/hooks.rs`)
- Channel adapters (where they hold db references)
- Web handlers (`src/web/` modules)
- All `call_blocking` callsites (~304) — signature-compatible, no body changes needed

**Import change**: Files importing `mchact_storage::db::Database` switch to `mchact_storage::DynDataStore`.

### Part 4: ObjectStorage wiring (single instance)

**Fix ACP** (`src/acp.rs:63-72`):
```rust
// Before
let storage = LocalStorage::new(&config.data_dir).await...;

// After
let storage_config = crate::runtime::build_storage_backend_config(&config);
crate::runtime::apply_storage_env_overrides(&config);
let storage: Arc<dyn ObjectStorage> = Arc::from(
    mchact_storage_backend::create_storage(&storage_config).await...
);
```

**Fix web upload** (`src/web.rs:2016-2023`):
```rust
// Before
let local_storage = LocalStorage::new(...).await?;
let media_mgr = MediaManager::new(local_storage, state.app_state.db.clone());
let media_id = media_mgr.store_file(...).await?;

// After
let media_id = state.app_state.media_manager.store_file(...).await?;
```

**Fix ToolRegistry** (`src/tools/mod.rs`):
- Add `storage: Arc<dyn ObjectStorage>` and `media_manager: Arc<MediaManager>` parameters to `ToolRegistry::new()`
- Remove internal `LocalStorage` creation
- Pass through to tools that need them
- Callers (`runtime.rs`, `acp.rs`) pass the shared instances

### Part 5: Channel MediaManager bypass

**Telegram** (`src/channels/telegram.rs:832`):
```rust
// Before
state.media_manager.storage().put(&key, bytes.clone()).await

// After
state.media_manager.store_file(bytes, &filename, Some(&mime), chat_id, "channel_inbound").await
```

**Weixin** (`src/channels/weixin.rs:2098`):
```rust
// Before
storage.put(&key, bytes).await

// After
state.media_manager.store_file(bytes, &filename, Some(&mime), chat_id, "channel_inbound").await
```

### Part 6: Migrate all `std::fs` data operations to ObjectStorage

Every data file operation must go through the `ObjectStorage` trait. The shared storage instance is accessible via `AppState` (directly or through a helper).

#### 6a. SOUL.md reads

**File:** `src/agent_engine.rs` — `load_soul_content()`

Current: `std::fs::read_to_string()` with filesystem path resolution.

Change: Accept `&dyn ObjectStorage` parameter. Read SOUL.md files via `storage.get(key)`:
- Global: `storage.get("SOUL.md")`
- Per-chat: `storage.get(&format!("groups/{chat_id}/SOUL.md"))`
- Per-channel soul_path: `storage.get(&format!("souls/{filename}"))`

The function becomes async. Callers already in async context.

Fallback: `./SOUL.md` (project root) stays as `fs::read_to_string` — this is a developer convenience for the repo-shipped default soul, not user data.

#### 6b. Conversation archives

**File:** `src/agent_engine.rs:2323` — `archive_conversation()`

Current: `std::fs::write()` to `groups/{channel}/{chat_id}/conversations/{timestamp}.md`

Change: Accept `&dyn ObjectStorage`. Use `storage.put(&key, content.as_bytes())`.
Key: `groups/{channel}/{chat_id}/conversations/{timestamp}.md`

Function becomes async.

#### 6c. TODO.json

**File:** `crates/mchact-tools/src/todo_store.rs`

Current: `std::fs::read_to_string` / `std::fs::write` to `groups/{channel}/{chat_id}/TODO.json`

Change: Add `ObjectStorage`-based functions alongside or replacing filesystem functions:
- `read_todos(storage, channel, chat_id)` → `storage.get("groups/{channel}/{chat_id}/TODO.json")`
- `write_todos(storage, channel, chat_id, todos)` → `storage.put(...)`
- `clear_todos(storage, channel, chat_id)` → `storage.delete(...)`

Add `mchact-storage-backend` as a dependency to `crates/mchact-tools/Cargo.toml`. Add `Arc<dyn ObjectStorage>` to `TodoReadTool` and `TodoWriteTool` structs.

#### 6d. Skill files (SKILL.md creation)

**File:** `src/tools/create_skill.rs:253`

Current: `std::fs::write()` to `{skills_dir}/{skill_name}/SKILL.md`

Change: Use ObjectStorage. Key: `skills/{skill_name}/SKILL.md`. Pass storage into `CreateSkillTool`.

#### 6e. Skills state (skills_state.json)

**File:** `src/skills.rs`

Current: `std::fs::read_to_string` / `std::fs::write` to `{runtime_data_dir}/skills_state.json`

Change: Use ObjectStorage. Key: `state/skills_state.json`.

#### 6f. Hook files and state

**File:** `src/hooks.rs`

Current: `std::fs::write` for HOOK.md and hooks_state.json

Change: Use ObjectStorage. Keys: `hooks/{hook_name}/HOOK.md`, `state/hooks_state.json`.

#### 6g. Feishu uploads

**File:** `src/channels/feishu.rs:2627`

Current: `tokio::fs::write` to `uploads/{channel}/{chat_id}/{filename}`

Change: Use `media_manager.store_file()` — same pattern as other channel inbound files.

#### 6h. Weixin account state and sync buffer

**File:** `src/channels/weixin.rs:604,641`

Current: `std::fs::write` to `weixin/{account}.json` and `weixin/{account}_sync.json`

Change: Use ObjectStorage. Keys: `state/weixin/{account}.json`, `state/weixin/{account}_sync.json`.

#### 6i. Batch checkpoints and trajectories

**File:** `src/batch.rs:201,344`

Current: `std::fs::write` to `{output_dir}/checkpoint.json` and `{output_dir}/{trajectories_file}`

Change: Use ObjectStorage. Keys: `batch/{run_id}/checkpoint.json`, `batch/{run_id}/{trajectories_file}`.

#### 6j. Chat memory delete (/reset)

**File:** `src/chat_commands.rs:112`

Current: `std::fs::remove_file` on AGENTS.md path

Change: Use `storage.delete("groups/{channel}/{chat_id}/AGENTS.md")`.

#### 6k. Skill nudge

**File:** `src/agent_engine.rs:2470`

Current: `std::fs::write` to `runtime/skill_nudge_pending.txt`

Change: Use ObjectStorage. Key: `state/skill_nudge_pending.txt`.

#### 6l. Web UI soul file listing

**File:** `src/web/config.rs` — `list_available_soul_files()`

Current: `std::fs::read_dir()` to discover .md files in souls directory

Change: ObjectStorage doesn't support directory listing natively. Two options:
- Add `list_keys(prefix)` method to ObjectStorage trait
- Keep filesystem listing for local backend, return empty for cloud (souls are configured via config, not discovered at runtime on cloud)

Recommended: Add `list_keys(prefix: &str) -> Result<Vec<String>>` to ObjectStorage trait. LocalStorage implements via `read_dir`. Cloud backends implement via their list-objects API (S3 ListObjectsV2, Azure list_blobs, GCS list_objects).

### Part 7: Config example update

Update `mchact.config.example.yaml` to document:
- `db_backend: "sqlite"` / `"postgres"` with `db_database_url`
- Existing `storage_backend` fields (already documented but clarify they now apply to all data)

## Migration Notes

- Existing local data stays in place — `LocalStorage` reads from the same paths
- No data migration needed for existing SQLite users
- PostgreSQL users need to run schema init (handled by `PgDriver::connect`)
- Cloud storage users need to upload existing `groups/` data to their bucket

## Files Changed (estimated)

| Area | Files | Change Type |
|---|---|---|
| Feature rename (sqlite-vec → vector-search) | ~8 | Annotation change |
| pgvector in PgDriver | 3 | New implementation |
| `Arc<Database>` → `Arc<DynDataStore>` | ~25 | Type change |
| `call_blocking` signature | 1 def + ~304 callsites | Signature change (compatible) |
| ObjectStorage wiring (ACP, web, ToolRegistry) | 3 | Use shared instance |
| Channel MediaManager bypass | 2 | Route through store_file |
| fs → ObjectStorage migrations | ~12 | Rewrite to use storage trait |
| ObjectStorage `list_keys` | 5 (trait + 4 backends) | New method |
| Config/docs | 2 | Documentation |
| **Total** | ~60 files | |

## Testing Strategy

- Existing storage integration tests validate SQLite DataStore behavior
- Add PgDriver integration test for `knn_memories` with pgvector
- Add test verifying `create_data_store()` returns correct backend type
- Existing stability smoke tests (cross-chat permissions, scheduler recovery) validate end-to-end
- Manual verification: configure `db_backend: "postgres"` and confirm all operations work
- Manual verification: configure `storage_backend: "s3"` and confirm SOUL.md, archives, todos, skills all land in S3

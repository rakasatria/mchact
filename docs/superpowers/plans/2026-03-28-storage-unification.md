# Storage Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire all data paths through the unified Database backend (SQLite OR Postgres) and ObjectStorage backend (local/S3/Azure/GCS) so config choices are respected everywhere.

**Architecture:** Replace `Arc<Database>` with `Arc<dyn DataStore + Send + Sync>` via a type alias `DynDataStore`. Use the existing `create_data_store()` factory in main.rs. Pass the shared `Arc<dyn ObjectStorage>` into all components instead of letting them create their own. Migrate all `std::fs` data operations to ObjectStorage. Implement pgvector for Postgres vector search behind a unified `vector-search` feature gate.

**Tech Stack:** Rust, tokio, rusqlite, tokio-postgres/deadpool-postgres, pgvector crate, mchact-storage-backend (ObjectStorage trait)

**Spec:** `docs/superpowers/specs/2026-03-28-storage-unification-design.md`

---

### Task 1: Rename `sqlite-vec` feature to `vector-search`

**Files:**
- Modify: `Cargo.toml:26`
- Modify: `crates/mchact-storage/Cargo.toml:10`
- Modify: 34 `#[cfg(feature = "sqlite-vec")]` sites across 10 files

- [ ] **Step 1: Rename feature in root Cargo.toml**

In `Cargo.toml` line 26, change:

```toml
# Before
sqlite-vec = ["mchact-storage/sqlite-vec"]
# After
vector-search = ["mchact-storage/vector-search"]
```

- [ ] **Step 2: Rename feature in mchact-storage Cargo.toml**

In `crates/mchact-storage/Cargo.toml` line 10, change:

```toml
# Before
sqlite-vec = ["sqlite", "dep:sqlite-vec"]
# After
vector-search = ["sqlite", "dep:sqlite-vec"]
```

Keep the `sqlite-vec` crate dependency line unchanged (line 23) — it's the crate name, not the feature name.

- [ ] **Step 3: Replace all cfg annotations**

Find-and-replace `"sqlite-vec"` → `"vector-search"` in `#[cfg(feature = "...")]` across these files:

- `crates/mchact-storage/src/traits/memory.rs` — lines 103, 106, 115
- `crates/mchact-storage/src/db/memory_db.rs` — lines 465, 504, 529
- `crates/mchact-storage/src/db/mod.rs` — lines 30, 40, 43, 757, 2983
- `crates/mchact-storage/src/driver/postgres/memory_db.rs` — lines 512, 517, 540
- `src/embedding.rs` — lines 58, 151, 228
- `src/main.rs` — lines 690, 698
- `src/runtime.rs` — line 160
- `src/scheduler.rs` — lines 312, 412
- `src/memory_service.rs` — lines 151, 164, 278, 314, 323, 429, 486, 517, 530, 606, 625

- [ ] **Step 4: Verify compilation**

Run: `cargo check --features vector-search` — should compile.
Run: `cargo check` (without feature) — should compile.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor: rename sqlite-vec feature to vector-search"
```

---

### Task 2: Implement pgvector in PgDriver

**Files:**
- Modify: `crates/mchact-storage/Cargo.toml` — add pgvector dep + postgres-vector feature
- Modify: `crates/mchact-storage/src/schema/postgres.sql` — add vector extension + column
- Modify: `crates/mchact-storage/src/driver/postgres/memory_db.rs:512-548` — replace no-ops

- [ ] **Step 1: Add pgvector dependency and feature**

In `crates/mchact-storage/Cargo.toml`, add dependency:

```toml
pgvector = { version = "0.4", optional = true, features = ["postgres"] }
```

Add a combined feature:

```toml
postgres-vector = ["postgres", "vector-search", "dep:pgvector"]
```

Update root `Cargo.toml` to expose it:

```toml
postgres-vector = ["mchact-storage/postgres-vector"]
```

- [ ] **Step 2: Update postgres schema**

In `crates/mchact-storage/src/schema/postgres.sql`, add at the very top before any CREATE TABLE:

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

In the `memories` table, add `embedding vector` column after `external_chat_id`:

```sql
    external_chat_id TEXT,
    embedding vector
```

- [ ] **Step 3: Implement prepare_vector_index**

In `crates/mchact-storage/src/driver/postgres/memory_db.rs`, replace the no-op at line 512:

```rust
#[cfg(feature = "vector-search")]
fn prepare_vector_index(&self, dimension: usize) -> Result<(), MchactError> {
    let pool = self.pool.clone();
    let dim = dimension as i32;
    tokio::runtime::Handle::current().block_on(async move {
        let client = pool.get().await.map_err(pg_err)?;
        client.execute("CREATE EXTENSION IF NOT EXISTS vector", &[]).await.map_err(pg_err)?;
        let alter = format!(
            "DO $$ BEGIN \
                ALTER TABLE memories ADD COLUMN embedding vector({dim}); \
             EXCEPTION WHEN duplicate_column THEN NULL; END $$"
        );
        client.execute(&alter, &[]).await.map_err(pg_err)?;
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_memories_embedding \
                 ON memories USING hnsw (embedding vector_cosine_ops)",
                &[],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    })
}
```

- [ ] **Step 4: Implement upsert_memory_vec**

Replace the no-op at line 517:

```rust
#[cfg(feature = "vector-search")]
fn upsert_memory_vec(&self, memory_id: i64, embedding: &[f32]) -> Result<(), MchactError> {
    use pgvector::Vector;
    let pool = self.pool.clone();
    let vec = Vector::from(embedding.to_vec());
    tokio::runtime::Handle::current().block_on(async move {
        let client = pool.get().await.map_err(pg_err)?;
        client
            .execute("UPDATE memories SET embedding = $1 WHERE id = $2", &[&vec, &memory_id])
            .await
            .map_err(pg_err)?;
        Ok(())
    })
}
```

- [ ] **Step 5: Implement knn_memories**

Replace the no-op at line 540:

```rust
#[cfg(feature = "vector-search")]
fn knn_memories(
    &self,
    chat_id: i64,
    query_vec: &[f32],
    k: usize,
) -> Result<Vec<(i64, f32)>, MchactError> {
    use pgvector::Vector;
    let pool = self.pool.clone();
    let vec = Vector::from(query_vec.to_vec());
    let limit = k as i64;
    tokio::runtime::Handle::current().block_on(async move {
        let client = pool.get().await.map_err(pg_err)?;
        let rows = client
            .query(
                "SELECT id, (embedding <=> $1)::real AS distance \
                 FROM memories \
                 WHERE chat_id = $2 AND is_archived = FALSE AND embedding IS NOT NULL \
                 ORDER BY embedding <=> $1 \
                 LIMIT $3",
                &[&vec, &chat_id, &limit],
            )
            .await
            .map_err(pg_err)?;
        Ok(rows.iter().map(|r| (r.get::<_, i64>("id"), r.get::<_, f32>("distance"))).collect())
    })
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check --features postgres-vector -p mchact-storage`
Expected: Compiles.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: implement pgvector for Postgres memory vector search"
```

---

### Task 3: Add DynDataStore type alias and update call_blocking

**Files:**
- Modify: `crates/mchact-storage/src/traits/mod.rs:28-42` — Add Send + Sync bounds
- Modify: `crates/mchact-storage/src/lib.rs` — Add type alias
- Modify: `crates/mchact-storage/src/db/mod.rs:50-58` — Update call_blocking

- [ ] **Step 1: Add Send + Sync supertrait bounds**

In `crates/mchact-storage/src/traits/mod.rs`, add `+ Send + Sync` to the DataStore trait:

```rust
pub trait DataStore:
    ChatStore
    + MessageStore
    + SessionStore
    + TaskStore
    + MemoryDbStore
    + AuthStore
    + AuditStore
    + MetricsStore
    + SubagentStore
    + DocumentStore
    + MediaObjectStore
    + KnowledgeStore
    + Send
    + Sync
{
}
```

- [ ] **Step 2: Add type alias to lib.rs**

In `crates/mchact-storage/src/lib.rs`, add after `pub use traits::DataStore;`:

```rust
/// Thread-safe, dynamically-dispatched DataStore.
pub type DynDataStore = dyn DataStore + Send + Sync;
```

- [ ] **Step 3: Update call_blocking signature**

In `crates/mchact-storage/src/db/mod.rs`, change lines 50-58:

```rust
pub async fn call_blocking<T, F>(db: std::sync::Arc<crate::DynDataStore>, f: F) -> Result<T, MchactError>
where
    T: Send + 'static,
    F: FnOnce(&crate::DynDataStore) -> Result<T, MchactError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(db.as_ref()))
        .await
        .map_err(|e| MchactError::ToolExecution(format!("DB task join error: {e}")))?
}
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: add DynDataStore type alias and update call_blocking signature"
```

---

### Task 4: Wire AppState.db and main.rs factory

**Files:**
- Modify: `src/runtime.rs:47,54,73` — AppState.db type
- Modify: `src/main.rs:1397` — use create_data_store factory
- Modify: `src/acp.rs:36-42` — accept Arc<DynDataStore>

- [ ] **Step 1: Update AppState.db type in runtime.rs**

Change import and field:

```rust
// Replace import
use mchact_storage::db::Database;
// With
use mchact_storage::DynDataStore;

// Change field (line 54)
pub db: Arc<DynDataStore>,
```

Update `run()` signature to accept `Arc<DynDataStore>` and remove `let db = Arc::new(db);`.

- [ ] **Step 2: Update main.rs to use factory**

Replace line 1397:

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
    .unwrap_or_else(|| {
        panic!("Failed to initialize '{}' database backend", config.db_backend)
    });
info!(backend = %config.db_backend, "Database initialized");
```

Pass `db` directly to `runtime::run()` and `acp::serve()` (it's already `Arc<dyn DataStore>`).

- [ ] **Step 3: Update acp::serve() signature**

```rust
pub async fn serve(
    config: Config,
    db: Arc<DynDataStore>,
    skills: SkillManager,
    mcp_manager: crate::mcp::McpManager,
) -> anyhow::Result<()> {
    // Remove: let db = Arc::new(db);
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: wire AppState.db as Arc<DynDataStore> via create_data_store factory"
```

---

### Task 5: Bulk replace Arc<Database> → Arc<DynDataStore>

**Files:** ~25 files (mechanical find-and-replace)

- [ ] **Step 1: Replace in all tool structs**

For each file: replace `use mchact_storage::db::Database;` → `use mchact_storage::DynDataStore;` and `Arc<Database>` → `Arc<DynDataStore>` in struct fields, `::new()` params, and function signatures.

Files (struct name : file:line):
- `ScheduleTaskTool` + 7 siblings: `src/tools/schedule.rs:134,289,376,443,510,577,584,686`
- `SessionsSpawnTool` + 10 siblings: `src/tools/subagents.rs:618,1048,1133,1224,1348` (and more)
- `ReadMemoryTool`, `WriteMemoryTool`: `src/tools/memory.rs:17,197`
- `FindingsWriteTool`, `FindingsReadTool`: `src/tools/findings.rs:34,113`
- `SendMessageTool`: `src/tools/send_message.rs:20`
- `SessionSearchTool`: `src/tools/session_search.rs:13`
- `ExportChatTool`: `src/tools/export_chat.rs:13`
- `ReadDocumentTool`: `src/tools/read_document.rs:34`
- `KnowledgeQueryTool`: `src/tools/knowledge.rs:82`
- 3 structs in `src/tools/structured_memory.rs`
- `MixtureOfAgentsTool`: `src/tools/mixture_of_agents.rs:26`
- `ToolRegistry::new()`: `src/tools/mod.rs:105`

- [ ] **Step 2: Replace in managers**

- `MediaManager`: `src/media_manager.rs:10`
- `MemoryBackend` + `SqliteMemoryProvider`: `src/memory_backend.rs:181,551`
- `KnowledgeManager`: `src/knowledge.rs:159`
- `HookManager`: `src/hooks.rs:140`
- `memory_service.rs:166`

- [ ] **Step 3: Replace in other files**

- `src/acp_subagent.rs:31,116`
- `src/chat_commands.rs:270`

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo check && cargo test --workspace`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor: replace Arc<Database> with Arc<DynDataStore> across codebase"
```

---

### Task 6: ObjectStorage single instance (ACP, Web upload, ToolRegistry)

**Files:**
- Modify: `src/tools/mod.rs:102-135` — accept storage + media_manager params
- Modify: `src/runtime.rs` — pass storage to ToolRegistry
- Modify: `src/acp.rs:63-72` — use config-driven storage
- Modify: `src/web.rs:2016-2023` — use existing media_manager

- [ ] **Step 1: Add storage params to ToolRegistry::new()**

In `src/tools/mod.rs`, add two params and remove internal LocalStorage creation:

```rust
pub fn new(
    config: &Config,
    channel_registry: Arc<ChannelRegistry>,
    db: Arc<DynDataStore>,
    memory_backend: Arc<MemoryBackend>,
    storage: Arc<dyn mchact_storage_backend::ObjectStorage>,
    media_manager: Arc<crate::media_manager::MediaManager>,
) -> Self {
    // DELETE lines 127-135 (local_storage + media_manager creation)
    // Use the passed-in storage and media_manager instead
```

- [ ] **Step 2: Update callers**

In `src/runtime.rs`, pass `storage.clone()` and `media_manager.clone()` to `ToolRegistry::new()`.

In `src/acp.rs`, replace `LocalStorage::new()` with config-driven creation:

```rust
let storage_config = crate::runtime::build_storage_backend_config(&config);
crate::runtime::apply_storage_env_overrides(&config);
let storage: Arc<dyn mchact_storage_backend::ObjectStorage> = Arc::from(
    mchact_storage_backend::create_storage(&storage_config).await.unwrap_or_else(|e| {
        panic!("Cannot initialize '{}' storage backend: {e}", config.storage_backend)
    }),
);
```

Make `build_storage_backend_config` and `apply_storage_env_overrides` public in `src/runtime.rs`.

- [ ] **Step 3: Fix web upload**

In `src/web.rs:2016-2034`, replace with:

```rust
let media_id = state.app_state.media_manager
    .store_file(file_bytes, &filename_for_storage, Some(mime_type.as_str()), 0, "upload")
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to store file: {e}")))?;
```

Delete the `local_storage` and `media_mgr` creation.

- [ ] **Step 4: Verify and commit**

```bash
cargo check && cargo test --workspace
git add -A && git commit -m "fix: wire single ObjectStorage instance through ACP, web upload, and ToolRegistry"
```

---

### Task 7: Channel MediaManager bypass fix

**Files:**
- Modify: `src/channels/telegram.rs:822-840`
- Modify: `src/channels/weixin.rs:2088-2107`
- Modify: `src/channels/feishu.rs:2627-2646`

- [ ] **Step 1: Fix Telegram**

Replace direct `storage().put()` with `store_file()`:

```rust
// Before
match state.media_manager.storage().put(&key, bytes.clone()).await {
    Ok(()) => { document_saved_path = Some(key); }

// After
match state.media_manager.store_file(bytes.clone(), &safe_name, None, raw_chat_id, "channel_inbound").await {
    Ok(media_id) => { document_saved_path = Some(format!("media:{media_id}")); }
```

Check how `document_saved_path` is used downstream and adjust if it expects a storage key vs a media ID.

- [ ] **Step 2: Fix Weixin**

Same pattern — replace direct `storage.put()` with `media_manager.store_file()`.

- [ ] **Step 3: Fix Feishu**

Replace `tokio::fs::write()` with `media_manager.store_file()`.

- [ ] **Step 4: Verify and commit**

```bash
cargo check
git add -A && git commit -m "fix: route channel inbound files through MediaManager.store_file()"
```

---

### Task 8: Add list_keys to ObjectStorage trait

**Files:**
- Modify: `crates/mchact-storage-backend/src/lib.rs` — add trait method
- Modify: `crates/mchact-storage-backend/src/local.rs`
- Modify: `crates/mchact-storage-backend/src/s3.rs`
- Modify: `crates/mchact-storage-backend/src/azure.rs`
- Modify: `crates/mchact-storage-backend/src/gcs.rs`
- Modify: `crates/mchact-storage-backend/src/cache.rs`

- [ ] **Step 1: Add trait method**

In the `ObjectStorage` trait:

```rust
async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>>;
```

- [ ] **Step 2: Implement for LocalStorage**

Recursive `read_dir`, collect file paths relative to `base_dir`:

```rust
async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>> {
    let base = self.full_path(prefix);
    let mut result = Vec::new();
    if !base.exists() { return Ok(result); }
    let mut stack = vec![base];
    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(StorageError::Io)?;
        while let Some(entry) = entries.next_entry().await.map_err(StorageError::Io)? {
            let path = entry.path();
            if path.is_dir() { stack.push(path); }
            else if let Ok(rel) = path.strip_prefix(&self.base_dir) {
                result.push(rel.to_string_lossy().to_string());
            }
        }
    }
    result.sort();
    Ok(result)
}
```

- [ ] **Step 3: Implement for S3Storage**

Use `list_objects_v2` with pagination.

- [ ] **Step 4: Implement for Azure and GCS**

Use respective list APIs.

- [ ] **Step 5: CachedStorage delegates**

```rust
async fn list_keys(&self, prefix: &str) -> StorageResult<Vec<String>> {
    self.inner.list_keys(prefix).await
}
```

- [ ] **Step 6: Verify and commit**

```bash
cargo check -p mchact-storage-backend
git add -A && git commit -m "feat: add list_keys to ObjectStorage trait with all backend implementations"
```

---

### Task 9: Migrate SOUL.md, archives, TODO.json to ObjectStorage

**Files:**
- Modify: `src/agent_engine.rs` — `load_soul_content()` async + storage, `archive_conversation()` async + storage
- Modify: `crates/mchact-tools/Cargo.toml` — add mchact-storage-backend dep
- Modify: `crates/mchact-tools/src/todo_store.rs` — async ObjectStorage functions
- Modify: `src/tools/todo.rs` — pass storage
- Modify: `src/web/config.rs` — soul listing via list_keys

- [ ] **Step 1: Migrate load_soul_content to async + ObjectStorage**

Change signature to accept `&dyn ObjectStorage`, make async. Replace `fs::read_to_string` with `storage.get()` for data_dir paths. Keep `./SOUL.md` as `fs::read_to_string` (project root convenience).

- [ ] **Step 2: Migrate archive_conversation to async + ObjectStorage**

Change signature to accept `&dyn ObjectStorage`. Replace `fs::write` with `storage.put()`. Key: `groups/{channel}/{chat_id}/conversations/{timestamp}.md`.

- [ ] **Step 3: Add async ObjectStorage functions to todo_store.rs**

Add `mchact-storage-backend` dep to `crates/mchact-tools/Cargo.toml`. Add `read_todos_async`, `write_todos_async`, `clear_todos_async` functions that use `ObjectStorage`. Keep sync versions for existing tests.

- [ ] **Step 4: Update TodoReadTool and TodoWriteTool**

Add `storage: Arc<dyn ObjectStorage>` to both structs. Use async functions.

- [ ] **Step 5: Update soul listing in web/config.rs**

Replace `std::fs::read_dir()` with `storage.list_keys("souls/")`.

- [ ] **Step 6: Verify and commit**

```bash
cargo check && cargo test --workspace
git add -A && git commit -m "refactor: migrate SOUL.md, archives, and TODO.json to ObjectStorage"
```

---

### Task 10: Migrate remaining fs operations to ObjectStorage

**Files:**
- Modify: `src/chat_commands.rs:112` — memory delete
- Modify: `src/agent_engine.rs:2470` — skill nudge
- Modify: `src/tools/create_skill.rs:253` — skill creation
- Modify: `src/skills.rs:376-398` — skills state
- Modify: `src/hooks.rs:485-500` — hooks state
- Modify: `src/channels/weixin.rs:604,641` — account state
- Modify: `src/batch.rs:201,344` — batch output

- [ ] **Step 1: Fix chat memory delete** — replace `fs::remove_file` with `storage.delete()`
- [ ] **Step 2: Fix skill nudge** — replace `fs::write` with `storage.put("state/skill_nudge_pending.txt")`
- [ ] **Step 3: Fix create_skill** — add `storage` to tool, use `storage.put("skills/{name}/SKILL.md")`
- [ ] **Step 4: Fix skills state** — add `storage` to SkillManager, use `storage.get/put("state/skills_state.json")`
- [ ] **Step 5: Fix hooks state** — same pattern with `state/hooks_state.json`
- [ ] **Step 6: Fix Weixin account state** — use `state/weixin/{key}.json` and `state/weixin/{key}_sync.json`
- [ ] **Step 7: Fix batch output** — use `batch/{run_id}/checkpoint.json` and `batch/{run_id}/{file}`
- [ ] **Step 8: Verify and commit**

```bash
cargo check && cargo test --workspace
git add -A && git commit -m "refactor: migrate all remaining fs data operations to ObjectStorage"
```

---

### Task 11: Update config example and final verification

**Files:**
- Modify: `mchact.config.example.yaml`

- [ ] **Step 1: Document db_backend and db_database_url in config example**

```yaml
# Database backend: "sqlite" (default) or "postgres"
# db_backend: "sqlite"
# db_database_url: "postgres://user:pass@localhost:5432/mchact"
```

- [ ] **Step 2: Full build and test**

```bash
cargo build && cargo test --workspace
```

- [ ] **Step 3: Build with all features**

```bash
cargo build --features vector-search,postgres
```

- [ ] **Step 4: Grep verification**

Verify no remaining `LocalStorage::new` in production code (test code OK).
Verify no remaining `Arc<Database>` in production code (test helpers OK).
Verify no remaining `std::fs::write` / `std::fs::read_to_string` on user data paths.

- [ ] **Step 5: Commit config**

```bash
git add mchact.config.example.yaml
git commit -m "docs: document db_backend and storage_backend config options"
```

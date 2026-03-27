# FTS5 Session Search + 5-Phase Compressor + MoA Findings

**Date:** 2026-03-27
**Status:** Approved
**Scope:** ~1,050 lines across 8 files (4 new, 4 modified)

## Problem

MicroClaw has three capability gaps compared to hermes-agent:

1. **No cross-session search.** After compaction, old messages exist only as static markdown archives. The agent cannot search or recall details from past conversations. The `messages` table has no FTS index.

2. **Single-pass compaction loses detail.** The current `compact_messages()` sends all old messages through one LLM summarization call with a generic "summarize concisely" prompt. Tool results (often the most token-heavy content) are summarized alongside conversation text. Re-compaction compounds information loss (summary drift).

3. **Parallel sub-agents are blind to each other.** `subagents_orchestrate` spawns workers in parallel, but workers cannot see sibling outputs during execution. Results are merged only after all workers finish. No shared workspace for MoA-style collaboration.

## Solution

Three features delivered together:

### Feature 1: FTS5 Session Search

#### Schema (Migration v20)

```sql
CREATE VIRTUAL TABLE messages_fts USING fts5(
    sender_name,
    content,
    content='messages',
    content_rowid='rowid'
);

CREATE TRIGGER messages_fts_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, sender_name, content)
    VALUES (new.rowid, new.sender_name, new.content);
END;

CREATE TRIGGER messages_fts_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, sender_name, content)
    VALUES ('delete', old.rowid, old.sender_name, old.content);
END;

CREATE TRIGGER messages_fts_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, sender_name, content)
    VALUES ('delete', old.rowid, old.sender_name, old.content);
    INSERT INTO messages_fts(rowid, sender_name, content)
    VALUES (new.rowid, new.sender_name, new.content);
END;

INSERT INTO messages_fts(rowid, sender_name, content)
SELECT rowid, sender_name, content FROM messages;
```

No changes to `store_message` or `store_message_if_new` — triggers handle sync.

#### Query Sanitizer

**New file:** `crates/microclaw-storage/src/fts.rs`

```rust
pub fn sanitize_fts_query(raw: &str) -> Option<String>
```

- Strips FTS5 operators: `"`, `*`, `(`, `)`, `+`, `-`, `^`, `~`, `:`, `{`, `}`, `[`, `]`
- Collapses whitespace
- Quotes each remaining token: `hello world` becomes `"hello" "world"`
- Returns `None` if input is empty after sanitization

#### Database Methods

**File:** `crates/microclaw-storage/src/db.rs`

```rust
#[derive(Debug, Clone)]
pub struct FtsSearchResult {
    pub message_id: String,
    pub chat_id: i64,
    pub chat_title: Option<String>,
    pub sender_name: String,
    pub content_snippet: String,
    pub timestamp: String,
    pub rank: f64,
}

impl Database {
    pub fn search_messages_fts(
        &self,
        query: &str,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, MicroClawError>;

    pub fn get_message_context(
        &self,
        chat_id: i64,
        timestamp: &str,
        window: usize,
    ) -> Result<Vec<StoredMessage>, MicroClawError>;

    pub fn rebuild_fts_index(&self) -> Result<(), MicroClawError>;
}
```

**Search SQL:**
```sql
SELECT m.id, m.chat_id, c.chat_title, m.sender_name,
       snippet(messages_fts, 1, '**', '**', '...', 48) AS snippet,
       m.timestamp, messages_fts.rank
FROM messages_fts
JOIN messages m ON m.rowid = messages_fts.rowid
LEFT JOIN chats c ON c.chat_id = m.chat_id
WHERE messages_fts MATCH ?1
  AND (?2 IS NULL OR m.chat_id = ?2)
ORDER BY messages_fts.rank
LIMIT ?3
```

**Context SQL:**
```sql
(SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
 FROM messages WHERE chat_id = ?1 AND timestamp < ?2
 ORDER BY timestamp DESC LIMIT ?3)
UNION ALL
(SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
 FROM messages WHERE chat_id = ?1 AND timestamp >= ?2
 ORDER BY timestamp ASC LIMIT ?3)
ORDER BY timestamp ASC
```

#### SessionSearchTool

**New file:** `src/tools/session_search.rs`

```rust
pub struct SessionSearchTool {
    db: Arc<Database>,
    control_chat_ids: Vec<i64>,
}
```

**Tool schema input:**
- `query` (string, required) — search terms
- `limit` (integer, optional, default 10, max 30)
- `context_window` (integer, optional, default 2, max 5)
- `chat_id` (integer, optional) — restrict to specific chat

**Execute flow:**
1. Sanitize query via `fts::sanitize_fts_query()`
2. Enforce chat authorization: non-control chats forced to own `chat_id`
3. `db.search_messages_fts(query, chat_id_filter, limit)`
4. For each hit, `db.get_message_context(chat_id, timestamp, context_window)`
5. Group by `chat_id`, format as readable text
6. Return `ToolResult::success`

**Authorization:** Non-control chats can only search their own messages. Control chats can search globally or filter by `chat_id`.

**Registration:** Both `ToolRegistry::new()` (main) and `new_sub_agent()` (sub-agents). Read-only, safe for sub-agents.

**Output format:**
```
Found 3 results for "deployment pipeline":

-- Chat: DevOps Team (chat_id: -100123) --
[2026-03-15 14:32] alice: We need to fix the deployment pipeline
[2026-03-15 14:33] bob: **deployment pipeline** is broken  <- match
[2026-03-15 14:35] alice: Let me check the CI logs

-- Chat: 1-on-1 with Bob (chat_id: 456) --
[2026-03-20 09:10] bob: Remember the **deployment pipeline** issue?  <- match
```

---

### Feature 2: 5-Phase Context Compressor

Replaces the current `compact_messages()` in `agent_engine.rs`.

**New file:** `src/compressor.rs`

```rust
pub struct ContextCompressor {
    tail_token_budget: usize,          // default 20_000
    protect_first_n: usize,            // default 3
    tool_result_age_threshold: usize,  // messages older than tail window
    tool_result_max_chars: usize,      // 200
    previous_summary: Option<String>,
    compression_count: u32,
}

impl ContextCompressor {
    pub fn new(config: &CompressorConfig) -> Self;

    pub async fn compress(
        &mut self,
        messages: &[Message],
        provider: &dyn LlmProvider,
        model: &str,
    ) -> Vec<Message>;
}
```

**Phase 1 — Prune old tool results (zero LLM cost)**

Scan messages outside the tail window. Replace `ToolResult` content blocks longer than 200 chars with `"[Tool output cleared -- use session_search to recall details]"`. Reclaims 30-50% of context in tool-heavy sessions.

**Phase 2 — Protect head messages**

Keep first N messages (default 3) intact. Preserves the original user request and agent's initial understanding.

**Phase 3 — Token-budget tail protection**

Walk backward from end, accumulating estimated tokens (`content.len() / 4`). Stop at `tail_token_budget` (default 20K). Everything between protected head and protected tail is the compression zone. Scales automatically with conversation length.

**Phase 4 — Structured LLM summary**

Summarize the compression zone with a structured template:

```
Summarize this conversation segment. Organize into:
- **Goal**: What the user wants to accomplish
- **Progress**: Done / In Progress / Blocked items
- **Key Decisions**: Important choices made
- **Relevant Files/Commands**: Paths, commands, URLs mentioned
- **Critical Context**: Anything needed to continue
```

**Phase 5 — Iterative update on re-compression**

If `previous_summary` exists, prompt changes to:

```
Here is the existing summary:
{previous_summary}

Here are new conversation turns since that summary:
{new_turns}

PRESERVE all existing information that is still relevant.
ADD new progress. Move items between Done/In Progress/Blocked as needed.
```

Prevents summary drift across multiple compressions.

**Post-phase — Sanitize tool pairs**

Remove orphaned `ToolUse` blocks (no matching `ToolResult`) and orphaned `ToolResult` blocks (no matching `ToolUse`).

**Fallback:** If LLM times out or fails, fall back to truncation (tail-only). No behavior regression.

**Key constants:**
- `MIN_SUMMARY_TOKENS = 2000`
- `SUMMARY_RATIO = 0.20` (20% of compressed content)
- `SUMMARY_TOKENS_CEILING = 12_000`
- `CHARS_PER_TOKEN = 4` (rough estimate)

**Integration:** `agent_engine.rs` replaces inline `compact_messages()` body with `ContextCompressor::compress()`. Archive step unchanged.

---

### Feature 3: MoA Shared Findings Blackboard

Enables parallel sub-agents to share discoveries during execution.

#### Schema (Migration v20, same block)

```sql
CREATE TABLE subagent_findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    orchestration_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    finding TEXT NOT NULL,
    category TEXT DEFAULT 'general',
    created_at TEXT NOT NULL
);

CREATE INDEX idx_findings_orch ON subagent_findings(orchestration_id);
```

#### Database Methods

```rust
impl Database {
    pub fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64, MicroClawError>;

    pub fn get_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<Vec<Finding>, MicroClawError>;

    pub fn delete_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<usize, MicroClawError>;
}
```

#### Tools

**New file:** `src/tools/findings.rs`

Two tools, registered only in `new_sub_agent()`:

**`findings_write`**
- Input: `finding` (string, required), `category` (string, optional, default "general")
- Extracts `orchestration_id` from `__subagent_runtime` metadata
- Calls `db.insert_finding()`
- Returns confirmation with finding ID

**`findings_read`**
- Input: none required
- Extracts `orchestration_id` from `__subagent_runtime` metadata
- Calls `db.get_findings()`
- Returns all sibling findings formatted with run_id attribution

#### Orchestration Flow

```
Parent calls subagents_orchestrate(goal, work_packages, wait=true)
  |
  +-- Worker A: researches topic X
  |     Uses session_search to find past context
  |     Posts finding via findings_write
  |     Reads sibling findings via findings_read
  |
  +-- Worker B: researches topic Y
  |     Reads Worker A's finding, avoids duplicate work
  |     Posts own finding
  |
  +-- Worker C: synthesizes
        Reads all findings from A + B
        Returns aggregated result
  |
  v
Parent receives merged artifacts + shared findings
```

#### Cleanup

When `subagents_orchestrate` completes (all workers done or timeout), delete findings for that `orchestration_id`. Optionally, the parent agent can promote valuable findings to structured memory via the existing reflector.

---

## File Map

### New files (4)

| File | Purpose | ~Lines |
|------|---------|--------|
| `crates/microclaw-storage/src/fts.rs` | FTS5 query sanitizer | 60 |
| `src/tools/session_search.rs` | SessionSearchTool | 200 |
| `src/compressor.rs` | 5-phase ContextCompressor | 350 |
| `src/tools/findings.rs` | FindingsWriteTool + FindingsReadTool | 150 |

### Modified files (4)

| File | Change | ~Lines |
|------|--------|--------|
| `crates/microclaw-storage/src/lib.rs` | Add `pub mod fts;` | 1 |
| `crates/microclaw-storage/src/db.rs` | Migration v20, FtsSearchResult, search methods, findings CRUD, rebuild_fts_index | 250 |
| `src/tools/mod.rs` | Register session_search + findings tools | 15 |
| `src/agent_engine.rs` | Replace compact_messages body with ContextCompressor::compress() | 20 (net -80) |

### Unchanged

- `crates/microclaw-core/src/llm_types.rs` — no changes to Message/ContentBlock
- `store_message` / `store_message_if_new` — FTS5 triggers handle sync
- `archive_conversation` — still writes markdown before compaction
- Sub-agent spawn/orchestrate logic — unchanged, just gets new tools

---

## Testing Strategy

### Unit tests
- `fts::sanitize_fts_query` — empty, special chars, normal, CJK
- `ContextCompressor` — each phase independently, edge cases (short conversations, no tool results)

### Integration tests
- FTS5 search: insert messages, verify search returns correct results with snippets
- FTS5 triggers: verify new messages auto-indexed
- Migration v19->v20: verify FTS table + triggers + backfill
- Context window retrieval: verify surrounding messages

### Tool tests
- `session_search`: empty query, valid query, chat authorization
- `findings_write/read`: scoped to orchestration_id, cross-worker visibility
- Compressor: full compress cycle, iterative re-compression, fallback on timeout

---

## Risks

| Risk | Mitigation |
|------|------------|
| Large backfill during migration | Runs at startup before requests. Bulk INSERT completes in <1s for 100K messages. |
| FTS5 index corruption | `rebuild_fts_index()` available for admin recovery. Triggers are idempotent. |
| FTS5 query injection | `sanitize_fts_query()` strips all operators, quotes tokens. Parameterized queries. |
| Compressor changes break sessions | Fallback to truncation on any error. Archive still written pre-compaction. |
| Summary drift on re-compression | Phase 5 iterative updates preserve existing information. |
| Findings table grows unbounded | Cleanup on orchestration completion. TTL-based cleanup as fallback. |
| rusqlite bundled SQLite missing FTS5 | Bundled feature includes FTS5 by default. Verify with `pragma_compile_options`. |

---

## Future Enhancements (out of scope)

- Anthropic prompt caching (`cache_control` markers) — reduces cost 50-80%
- Smart model routing for simple messages — 200x cost reduction on trivial turns
- LLM summarization of FTS5 search results — structured output like Hermes
- Findings promotion to structured memory — auto-extract valuable findings via reflector

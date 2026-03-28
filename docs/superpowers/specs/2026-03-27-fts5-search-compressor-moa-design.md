# FTS5 Session Search + 5-Phase Compressor + MoA Findings

**Date:** 2026-03-27
**Status:** Approved
**Scope:** ~1,370 lines across 10 files (5 new, 5 modified)

## Problem

mchact has three capability gaps compared to hermes-agent:

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

**New file:** `crates/mchact-storage/src/fts.rs`

```rust
pub fn sanitize_fts_query(raw: &str) -> Option<String>
```

- Strips FTS5 operators: `"`, `*`, `(`, `)`, `+`, `-`, `^`, `~`, `:`, `{`, `}`, `[`, `]`
- Collapses whitespace
- Quotes each remaining token: `hello world` becomes `"hello" "world"`
- Returns `None` if input is empty after sanitization

#### Database Methods

**File:** `crates/mchact-storage/src/db.rs`

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
    ) -> Result<Vec<FtsSearchResult>, mchactError>;

    pub fn get_message_context(
        &self,
        chat_id: i64,
        timestamp: &str,
        window: usize,
    ) -> Result<Vec<StoredMessage>, mchactError>;

    pub fn rebuild_fts_index(&self) -> Result<(), mchactError>;
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
    ) -> Result<i64, mchactError>;

    pub fn get_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<Vec<Finding>, mchactError>;

    pub fn delete_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<usize, mchactError>;
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

### Feature 5: Mixture of Agents Tool

A dedicated `mixture_of_agents` tool that gives the **same question** to multiple independent agents and synthesizes a consensus answer. Combines Hermes' proven pattern (model diversity, resilience) with mchact's strengths (tool-equipped sub-agents, shared findings).

#### How It Differs From `subagents_orchestrate`

| | `subagents_orchestrate` | `mixture_of_agents` |
|---|---|---|
| Input | `goal` + `work_packages[]` (different tasks) | `user_prompt` (one shared task) |
| Workers | Each gets a different package | All get the same question |
| Diversity | Implicit (different tasks) | Model diversity + perspective prompts |
| Merge | Concatenate artifacts | LLM-synthesized consensus |
| Failure tolerance | All must complete for merge | Min 1 out of N must succeed |
| Output | Raw merged JSON | Single natural-language answer |

#### Two Modes

**Simple mode** (like Hermes): Just pass `user_prompt`. Workers use different models if `provider_presets` are configured, or different perspective prompts on the same model.

**Advanced mode**: Pass `user_prompt` + `approach_hints` + `model_overrides` for full control over which model tackles which perspective.

#### Tool Definition

**New file:** `src/tools/mixture_of_agents.rs`

```rust
pub struct MixtureOfAgentsTool {
    config: Config,
    db: Arc<Database>,
    channel_registry: Arc<ChannelRegistry>,
}
```

**Tool schema input:**
- `user_prompt` (string, required) — the question or task all agents tackle
- `perspectives` (integer, optional, default 3, max 5) — number of independent agents
- `approach_hints` (string[], optional) — explicit perspective labels, e.g. `["security expert", "performance engineer", "user experience"]`
- `model_overrides` (string[], optional) — per-worker model or provider_preset names, e.g. `["anthropic/claude-sonnet", "openai/gpt-5", "deepseek/deepseek-v3"]`. Uses configured default model when not specified.
- `wait_timeout_secs` (integer, optional, default 120, max 300)
- `token_budget_total` (integer, optional, default from config)
- `min_successful` (integer, optional, default 1) — minimum workers that must succeed before synthesis

**Execute flow:**

1. **Resolve diversity strategy.**
   - If `model_overrides` provided: each worker uses a different model (Hermes-style model diversity)
   - If `approach_hints` provided: each worker gets a different perspective prompt
   - If neither: auto-generate N perspective prompts (default diversity)
   - Both can be combined: different models AND different perspectives

2. **Auto-generate perspectives** (when no `approach_hints`):
   ```
   Perspective 1: "Analyze this as a pragmatic engineer focused on simplicity and correctness."
   Perspective 2: "Analyze this as a skeptic looking for edge cases, failure modes, and risks."
   Perspective 3: "Analyze this as an architect focused on long-term maintainability and scalability."
   ```

   | Count | Perspectives |
   |-------|-------------|
   | 2 | Pragmatic engineer, Skeptic/edge-cases |
   | 3 | Pragmatic engineer, Skeptic/edge-cases, Architect/long-term |
   | 4 | + Security/risk analyst |
   | 5 | + User/stakeholder advocate |

3. **Spawn workers via `subagents_orchestrate` internally.** Each work_package is the same `user_prompt` with a perspective prefix in the context. If `model_overrides` are specified, each worker's sub-agent gets a model override:
   ```json
   {
     "goal": "Answer this question from multiple perspectives and synthesize",
     "work_packages": [
       "PERSPECTIVE: pragmatic engineer\nQUESTION: {user_prompt}",
       "PERSPECTIVE: skeptic / edge cases\nQUESTION: {user_prompt}",
       "PERSPECTIVE: architect / long-term\nQUESTION: {user_prompt}"
     ],
     "wait": true,
     "wait_timeout_secs": timeout,
     "token_budget_total": budget
   }
   ```

4. **Workers execute independently.** Each sub-agent can:
   - Use `session_search` to find relevant past context
   - Use `findings_write` / `findings_read` to share discoveries with siblings
   - Use tools (bash, file I/O, web) if the question requires research
   - Return their perspective's answer as an artifact

5. **Check minimum success threshold.** If fewer than `min_successful` workers completed successfully, return an error with partial results rather than attempting synthesis. Default threshold is 1 (matches Hermes — up to N-1 can fail).

6. **Synthesize consensus.** Make one final LLM call with the aggregator prompt:
   ```
   You have been provided with responses from {N} independent perspectives
   to the same question. Your task is to synthesize these into a single,
   high-quality response. Critically evaluate the information — some may be
   biased or incorrect. Do not simply replicate the answers but offer a
   refined, accurate, and comprehensive reply.

   Responses:
   1. [Pragmatic Engineer]: {worker_1_result}
   2. [Skeptic]: {worker_2_result}
   3. [Architect]: {worker_3_result}

   Synthesize into a single answer that:
   - Identifies points of agreement (high confidence)
   - Notes disagreements and which perspective is most convincing
   - Provides a final recommended answer
   ```

7. **Return synthesized answer** as `ToolResult::success` with metadata:
   ```json
   {
     "success": true,
     "response": "synthesized answer text",
     "models_used": ["model_a", "model_b", "model_c"],
     "perspectives_used": ["pragmatic", "skeptic", "architect"],
     "successful_count": 3,
     "failed_count": 0
   }
   ```

#### Resilience (from Hermes)

- **Min success threshold**: Only `min_successful` (default 1) workers need to succeed. Failed workers are logged but don't block synthesis.
- **Retry handling**: Sub-agent infrastructure already handles retries at the tool execution level. No additional retry loop needed at MoA level.
- **Graceful degradation**: If only 1 out of 3 workers succeeds, synthesis still runs but notes limited perspectives in the output.
- **Timeout**: `wait_timeout_secs` (default 120) passed to `subagents_orchestrate`. Timed-out workers excluded from synthesis.

#### Model Diversity via `provider_presets`

mchact already supports `provider_presets` in config:
```yaml
provider_presets:
  claude:
    provider: "anthropic"
    api_key: "sk-..."
    default_model: "claude-sonnet-4-5"
  gpt:
    provider: "openai"
    api_key: "sk-..."
    default_model: "gpt-5.2"
  deepseek:
    provider: "deepseek"
    api_key: "sk-..."
    default_model: "deepseek-v3"
```

When `model_overrides: ["claude", "gpt", "deepseek"]` is passed, each sub-agent resolves the preset and uses that provider/model combination. This gives Hermes-style multi-model diversity without hardcoding model names.

#### Registration

Registered in `ToolRegistry::new()` (main agent only). Not available to sub-agents — MoA is a top-level orchestration pattern, not recursive.

#### Cost Control

- Each perspective runs within `token_budget_total / perspectives` budget
- Synthesis call is a single LLM call (~2-4K tokens input for 3 perspectives)
- Total cost: roughly `(perspectives + 1)` LLM calls per invocation
- For 3 perspectives with default model: ~4 LLM calls total
- Tool description warns "use sparingly for genuinely difficult problems" (same as Hermes)

---

## File Map

### New files (4)

| File | Purpose | ~Lines |
|------|---------|--------|
| `crates/mchact-storage/src/fts.rs` | FTS5 query sanitizer | 60 |
| `src/tools/session_search.rs` | SessionSearchTool | 200 |
| `src/compressor.rs` | 5-phase ContextCompressor | 350 |
| `src/tools/findings.rs` | FindingsWriteTool + FindingsReadTool | 150 |

### Modified files (4)

| File | Change | ~Lines |
|------|--------|--------|
| `crates/mchact-storage/src/lib.rs` | Add `pub mod fts;` | 1 |
| `crates/mchact-storage/src/db.rs` | Migration v20, FtsSearchResult, search methods, findings CRUD, rebuild_fts_index | 250 |
| `src/tools/mod.rs` | Register session_search + findings tools | 15 |
| `src/agent_engine.rs` | Replace compact_messages body with ContextCompressor::compress() | 20 (net -80) |

### Unchanged

- `crates/mchact-core/src/llm_types.rs` — no changes to Message/ContentBlock
- `store_message` / `store_message_if_new` — FTS5 triggers handle sync
- `archive_conversation` — still writes markdown before compaction
- Sub-agent spawn/orchestrate logic — unchanged, just gets new tools

---

### Feature 4: Anthropic Prompt Caching

Reduces input token costs ~75% for Anthropic provider by marking stable content with `cache_control` breakpoints.

#### Background

- **Anthropic**: Requires explicit `cache_control: {"type": "ephemeral"}` markers on content blocks. Up to 4 breakpoints per request. Cached tokens cost 90% less on cache hits.
- **OpenAI / DeepSeek**: Automatic prefix caching, no code changes needed.
- **Ollama / others**: No caching API, N/A.

Only the Anthropic code path needs changes.

#### Implementation

**Modified file:** `src/llm.rs` — Anthropic serialization path only (~70 lines)

```rust
pub fn apply_anthropic_cache_control(messages: &mut Vec<serde_json::Value>) {
    let marker = json!({"type": "ephemeral"});
    let mut breakpoints_used = 0;
    const MAX_BREAKPOINTS: usize = 4;

    // Breakpoint 1: System prompt (stable across all iterations)
    if let Some(first) = messages.first_mut() {
        if first.get("role").and_then(|r| r.as_str()) == Some("system") {
            apply_cache_marker(first, &marker);
            breakpoints_used += 1;
        }
    }

    // Breakpoints 2-4: Last 3 non-system messages (rolling window)
    let remaining = MAX_BREAKPOINTS - breakpoints_used;
    let non_system_indices: Vec<usize> = messages.iter().enumerate()
        .filter(|(_, m)| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .map(|(i, _)| i)
        .collect();

    for &idx in non_system_indices.iter().rev().take(remaining) {
        apply_cache_marker(&mut messages[idx], &marker);
    }
}

fn apply_cache_marker(msg: &mut serde_json::Value, marker: &serde_json::Value) {
    // If content is a string, convert to array-of-blocks format:
    //   "hello" -> [{"type": "text", "text": "hello", "cache_control": {...}}]
    // If content is already an array, append cache_control to last block.
    // Tool messages: add cache_control directly to message dict.
}
```

#### Integration Point

Applied in `src/llm.rs` **only** in the Anthropic provider's `send_request` implementation, after building the request body and before sending it. This is a serialization-time transformation — the shared `Message` / `ContentBlock` types in `llm_types.rs` are never modified.

```rust
// In Anthropic provider send_request():
let mut body = build_anthropic_request_body(request);
if self.enable_prompt_caching {
    apply_anthropic_cache_control(&mut body["messages"]);
}
// send body...
```

#### Safety

- **Non-Anthropic providers never see cache markers.** The function is only called in the Anthropic code path.
- **If caching fails**, Anthropic silently ignores invalid markers — no error, just no discount.
- **No changes to shared types.** `Message`, `ContentBlock`, `MessagesRequest` are untouched.
- **Configurable.** Respect existing config; can be disabled if needed.

#### Cost Impact

For a 10-iteration tool loop with a 3K-token system prompt:
- Without caching: ~30K input tokens for system prompt alone (3K x 10)
- With caching: ~3K on first call + ~2.7K x 9 cache hits = ~27K tokens at 90% discount
- **Effective savings: ~22K tokens per request (~75% reduction on repeated context)**

---

## File Map

### New files (5)

| File | Purpose | ~Lines |
|------|---------|--------|
| `crates/mchact-storage/src/fts.rs` | FTS5 query sanitizer | 60 |
| `src/tools/session_search.rs` | SessionSearchTool | 200 |
| `src/compressor.rs` | 5-phase ContextCompressor | 350 |
| `src/tools/findings.rs` | FindingsWriteTool + FindingsReadTool | 150 |
| `src/tools/mixture_of_agents.rs` | MixtureOfAgentsTool (simple + advanced modes) | 250 |

### Modified files (5)

| File | Change | ~Lines |
|------|--------|--------|
| `crates/mchact-storage/src/lib.rs` | Add `pub mod fts;` | 1 |
| `crates/mchact-storage/src/db.rs` | Migration v20, FtsSearchResult, search methods, findings CRUD, rebuild_fts_index | 250 |
| `src/tools/mod.rs` | Register session_search, findings, and mixture_of_agents tools | 20 |
| `src/agent_engine.rs` | Replace compact_messages body with ContextCompressor::compress() | 20 (net -80) |
| `src/llm.rs` | `apply_anthropic_cache_control()` + `apply_cache_marker()` in Anthropic provider path | 70 |

### Unchanged

- `crates/mchact-core/src/llm_types.rs` — no changes to Message/ContentBlock
- `store_message` / `store_message_if_new` — FTS5 triggers handle sync
- `archive_conversation` — still writes markdown before compaction
- Sub-agent spawn/orchestrate logic — unchanged, just gets new tools
- Non-Anthropic providers — untouched, automatic caching where supported

---

## Testing Strategy

### Unit tests
- `fts::sanitize_fts_query` — empty, special chars, normal, CJK
- `ContextCompressor` — each phase independently, edge cases (short conversations, no tool results)
- `apply_anthropic_cache_control` — string content conversion, array content, tool messages, max breakpoints

### Integration tests
- FTS5 search: insert messages, verify search returns correct results with snippets
- FTS5 triggers: verify new messages auto-indexed
- Migration v19->v20: verify FTS table + triggers + backfill
- Context window retrieval: verify surrounding messages

### Tool tests
- `session_search`: empty query, valid query, chat authorization
- `findings_write/read`: scoped to orchestration_id, cross-worker visibility
- `mixture_of_agents`: perspective generation (default and custom hints), synthesis prompt construction, timeout handling, cost budget enforcement
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
| Cache markers leak to non-Anthropic | Applied only in Anthropic serialization path. Shared types untouched. |
| Cache marker breaks message format | If invalid, Anthropic silently ignores. No error, just no discount. |
| MoA synthesis quality varies | Structured synthesis prompt with explicit instructions. Worst case: returns concatenated perspectives without consensus. |
| MoA cost multiplier (N+1 LLM calls) | Token budget enforced per perspective. Default 3 perspectives keeps cost at ~4x a single call. |

---

## Future Enhancements (out of scope)

- Smart model routing for simple messages — 200x cost reduction on trivial turns
- LLM summarization of FTS5 search results — structured output like Hermes
- Findings promotion to structured memory — auto-extract valuable findings via reflector

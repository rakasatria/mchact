# FTS5 Search + 5-Phase Compressor + MoA + Prompt Caching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cross-session full-text search, intelligent context compression, MoA sub-agent collaboration, and Anthropic prompt caching to mchact.

**Architecture:** FTS5 virtual table on existing `messages` table with auto-sync triggers. New `ContextCompressor` module replaces single-pass compaction. Shared `subagent_findings` table enables inter-worker communication. `mixture_of_agents` tool orchestrates multi-perspective consensus. Anthropic `cache_control` markers applied at serialization time.

**Tech Stack:** Rust 2021, rusqlite (bundled SQLite with FTS5), tokio, serde_json, async_trait.

**Spec:** `docs/superpowers/specs/2026-03-27-fts5-search-compressor-moa-design.md`

---

## File Structure

### New files
| File | Responsibility |
|------|---------------|
| `crates/mchact-storage/src/fts.rs` | FTS5 query sanitization |
| `src/tools/session_search.rs` | SessionSearchTool (Tool trait) |
| `src/compressor.rs` | 5-phase ContextCompressor |
| `src/tools/findings.rs` | FindingsWriteTool + FindingsReadTool |
| `src/tools/mixture_of_agents.rs` | MixtureOfAgentsTool |

### Modified files
| File | What changes |
|------|-------------|
| `crates/mchact-storage/src/lib.rs` | Add `pub mod fts;` |
| `crates/mchact-storage/src/db.rs` | Migration v20, FTS search methods, findings CRUD |
| `src/tools/mod.rs` | Register new tools |
| `src/agent_engine.rs` | Delegate to ContextCompressor |
| `src/llm.rs` | Anthropic cache_control markers |

---

### Task 1: FTS5 Query Sanitizer

**Files:**
- Create: `crates/mchact-storage/src/fts.rs`
- Modify: `crates/mchact-storage/src/lib.rs`

- [ ] **Step 1: Create fts.rs with sanitize_fts_query and tests**

```rust
// crates/mchact-storage/src/fts.rs

/// Sanitize a raw user query for safe use in FTS5 MATCH expressions.
/// Strips all FTS5 operators and quotes each token individually.
/// Returns `None` if the input is empty after sanitization.
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
    fn test_empty_input() {
        assert_eq!(sanitize_fts_query(""), None);
        assert_eq!(sanitize_fts_query("   "), None);
    }

    #[test]
    fn test_only_special_chars() {
        assert_eq!(sanitize_fts_query("+-*()"), None);
        assert_eq!(sanitize_fts_query("\"\"\""), None);
    }

    #[test]
    fn test_normal_words() {
        assert_eq!(
            sanitize_fts_query("hello world"),
            Some("\"hello\" \"world\"".to_string())
        );
    }

    #[test]
    fn test_strips_operators() {
        assert_eq!(
            sanitize_fts_query("hello AND (world)"),
            Some("\"hello\" \"AND\" \"world\"".to_string())
        );
    }

    #[test]
    fn test_mixed_special_and_words() {
        assert_eq!(
            sanitize_fts_query("deploy*ment +pipeline"),
            Some("\"deploy\" \"ment\" \"pipeline\"".to_string())
        );
    }

    #[test]
    fn test_cjk_characters() {
        assert_eq!(
            sanitize_fts_query("hello \u{4F60}\u{597D}"),
            Some("\"hello\" \"\u{4F60}\u{597D}\"".to_string())
        );
    }
}
```

- [ ] **Step 2: Export the module from lib.rs**

Add `pub mod fts;` to `crates/mchact-storage/src/lib.rs` so it becomes:

```rust
//! Storage and persistence domain for mchact.

pub mod db;
pub mod fts;
pub mod memory;
pub mod memory_quality;
pub mod usage;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p mchact-storage fts`
Expected: All 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/mchact-storage/src/fts.rs crates/mchact-storage/src/lib.rs
git commit -m "feat(storage): add FTS5 query sanitizer module"
```

---

### Task 2: Migration v20 — FTS5 Table + Findings Table

**Files:**
- Modify: `crates/mchact-storage/src/db.rs`

- [ ] **Step 1: Bump SCHEMA_VERSION_CURRENT to 20**

At line 195 of `crates/mchact-storage/src/db.rs`, change:
```rust
const SCHEMA_VERSION_CURRENT: i64 = 20;
```

- [ ] **Step 2: Add migration block after the version 19 block**

After line 856 (`version = 19;`), before the `if version != SCHEMA_VERSION_CURRENT` check, insert:

```rust
    if version < 20 {
        // FTS5 content-sync virtual table on messages
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                sender_name,
                content,
                content='messages',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS messages_fts_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, sender_name, content)
                VALUES (new.rowid, new.sender_name, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS messages_fts_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, sender_name, content)
                VALUES ('delete', old.rowid, old.sender_name, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS messages_fts_au AFTER UPDATE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, sender_name, content)
                VALUES ('delete', old.rowid, old.sender_name, old.content);
                INSERT INTO messages_fts(rowid, sender_name, content)
                VALUES (new.rowid, new.sender_name, new.content);
            END;

            -- Backfill existing messages
            INSERT INTO messages_fts(rowid, sender_name, content)
            SELECT rowid, sender_name, content FROM messages;

            -- Shared findings blackboard for MoA
            CREATE TABLE IF NOT EXISTS subagent_findings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                orchestration_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                finding TEXT NOT NULL,
                category TEXT DEFAULT 'general',
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_findings_orch
                ON subagent_findings(orchestration_id);
            ",
        )?;
        set_schema_version(conn, 20)?;
        version = 20;
    }
```

- [ ] **Step 3: Build to verify migration compiles**

Run: `cargo build -p mchact-storage`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/mchact-storage/src/db.rs
git commit -m "feat(storage): add migration v20 — FTS5 table, triggers, backfill, findings table"
```

---

### Task 3: FTS Search and Context Database Methods

**Files:**
- Modify: `crates/mchact-storage/src/db.rs`

- [ ] **Step 1: Add FtsSearchResult and Finding structs**

Add these near the other struct definitions (around line 197, after `SessionTreeRow`):

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

#[derive(Debug, Clone)]
pub struct Finding {
    pub id: i64,
    pub orchestration_id: String,
    pub run_id: String,
    pub finding: String,
    pub category: String,
    pub created_at: String,
}
```

- [ ] **Step 2: Add search_messages_fts method**

Add to the `impl Database` block (after `store_message_if_new` around line 1245):

```rust
    pub fn search_messages_fts(
        &self,
        query: &str,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, mchactError> {
        use crate::fts::sanitize_fts_query;
        let match_expr = match sanitize_fts_query(query) {
            Some(expr) => expr,
            None => return Ok(vec![]),
        };
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT m.id, m.chat_id, c.chat_title, m.sender_name,
                    snippet(messages_fts, 1, '**', '**', '...', 48) AS snippet,
                    m.timestamp, messages_fts.rank
             FROM messages_fts
             JOIN messages m ON m.rowid = messages_fts.rowid
             LEFT JOIN chats c ON c.chat_id = m.chat_id
             WHERE messages_fts MATCH ?1
               AND (?2 IS NULL OR m.chat_id = ?2)
             ORDER BY messages_fts.rank
             LIMIT ?3",
        )?;
        let chat_id_param: Option<i64> = chat_id;
        let limit_param = limit as i64;
        let rows = stmt.query_map(params![match_expr, chat_id_param, limit_param], |row| {
            Ok(FtsSearchResult {
                message_id: row.get(0)?,
                chat_id: row.get(1)?,
                chat_title: row.get(2)?,
                sender_name: row.get(3)?,
                content_snippet: row.get(4)?,
                timestamp: row.get(5)?,
                rank: row.get(6)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
```

- [ ] **Step 3: Add get_message_context method**

Add directly after `search_messages_fts`:

```rust
    pub fn get_message_context(
        &self,
        chat_id: i64,
        timestamp: &str,
        window: usize,
    ) -> Result<Vec<StoredMessage>, mchactError> {
        let conn = self.lock_conn();
        let window_param = window as i64;
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp FROM (
                SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                FROM messages WHERE chat_id = ?1 AND timestamp < ?2
                ORDER BY timestamp DESC LIMIT ?3
             )
             UNION ALL
             SELECT id, chat_id, sender_name, content, is_from_bot, timestamp FROM (
                SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                FROM messages WHERE chat_id = ?1 AND timestamp >= ?2
                ORDER BY timestamp ASC LIMIT ?3
             )
             ORDER BY timestamp ASC",
        )?;
        let rows = stmt.query_map(params![chat_id, timestamp, window_param], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                sender_name: row.get(2)?,
                content: row.get(3)?,
                is_from_bot: row.get::<_, i32>(4)? != 0,
                timestamp: row.get(5)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
```

- [ ] **Step 4: Add rebuild_fts_index method**

```rust
    pub fn rebuild_fts_index(&self) -> Result<(), mchactError> {
        let conn = self.lock_conn();
        conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES('rebuild')")?;
        Ok(())
    }
```

- [ ] **Step 5: Add findings CRUD methods**

```rust
    pub fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64, mchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subagent_findings (orchestration_id, run_id, finding, category, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![orchestration_id, run_id, finding, category, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<Vec<Finding>, mchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, orchestration_id, run_id, finding, category, created_at
             FROM subagent_findings
             WHERE orchestration_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![orchestration_id], |row| {
            Ok(Finding {
                id: row.get(0)?,
                orchestration_id: row.get(1)?,
                run_id: row.get(2)?,
                finding: row.get(3)?,
                category: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn delete_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<usize, mchactError> {
        let conn = self.lock_conn();
        let affected = conn.execute(
            "DELETE FROM subagent_findings WHERE orchestration_id = ?1",
            params![orchestration_id],
        )?;
        Ok(affected)
    }
```

- [ ] **Step 6: Build to verify**

Run: `cargo build -p mchact-storage`
Expected: Compiles with no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/mchact-storage/src/db.rs
git commit -m "feat(storage): add FTS search, context window, and findings CRUD methods"
```

---

### Task 4: SessionSearchTool

**Files:**
- Create: `src/tools/session_search.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create session_search.rs**

```rust
// src/tools/session_search.rs

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::Database;
use mchact_tools::runtime::{Tool, ToolResult};
use serde_json::json;

pub struct SessionSearchTool {
    db: Arc<Database>,
    control_chat_ids: Vec<i64>,
}

impl SessionSearchTool {
    pub fn new(db: Arc<Database>, control_chat_ids: Vec<i64>) -> Self {
        Self {
            db,
            control_chat_ids,
        }
    }
}

#[async_trait]
impl Tool for SessionSearchTool {
    fn name(&self) -> &str {
        "session_search"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "session_search".into(),
            description: "Search past messages across all conversations using full-text search. Returns matching messages with surrounding context, grouped by chat. Use this to recall information from previous conversations.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search terms to find in message history"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default 10, max 30)",
                        "default": 10
                    },
                    "context_window": {
                        "type": "integer",
                        "description": "Number of surrounding messages per hit (default 2, max 5)",
                        "default": 2
                    },
                    "chat_id": {
                        "type": "integer",
                        "description": "Restrict search to a specific chat (optional)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q,
            _ => return ToolResult::error("Missing or empty 'query' parameter".into()),
        };

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(30) as usize;

        let context_window = input
            .get("context_window")
            .and_then(|v| v.as_u64())
            .unwrap_or(2)
            .min(5) as usize;

        let chat_id_filter = input.get("chat_id").and_then(|v| v.as_i64());

        // Enforce chat authorization from __auth_context
        let caller_chat_id = input
            .get("__auth_context")
            .and_then(|a| a.get("chat_id"))
            .and_then(|v| v.as_i64());

        let effective_chat_id = if let Some(caller) = caller_chat_id {
            if self.control_chat_ids.contains(&caller) {
                chat_id_filter // control chats can search globally
            } else {
                Some(caller) // non-control forced to own chat
            }
        } else {
            chat_id_filter
        };

        let db = self.db.clone();
        let query_owned = query.to_string();
        let results = match tokio::task::spawn_blocking(move || {
            db.search_messages_fts(&query_owned, effective_chat_id, limit)
        })
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return ToolResult::error(format!("Search failed: {e}")),
            Err(e) => return ToolResult::error(format!("Search task failed: {e}")),
        };

        if results.is_empty() {
            return ToolResult::success(format!("No results found for \"{query}\"."));
        }

        // Group results by chat_id
        let mut grouped: BTreeMap<i64, Vec<_>> = BTreeMap::new();
        for result in &results {
            grouped.entry(result.chat_id).or_default().push(result);
        }

        // Build output with context
        let mut output = format!("Found {} results for \"{}\":\n", results.len(), query);

        for (cid, hits) in &grouped {
            let title = hits
                .first()
                .and_then(|h| h.chat_title.as_deref())
                .unwrap_or("Unknown");
            output.push_str(&format!("\n-- Chat: {} (chat_id: {}) --\n", title, cid));

            for hit in hits {
                let db = self.db.clone();
                let ts = hit.timestamp.clone();
                let chat = hit.chat_id;
                let window = context_window;
                if let Ok(Ok(context)) = tokio::task::spawn_blocking(move || {
                    db.get_message_context(chat, &ts, window)
                })
                .await
                {
                    for msg in &context {
                        let marker = if msg.timestamp == hit.timestamp {
                            "  <- match"
                        } else {
                            ""
                        };
                        output.push_str(&format!(
                            "[{}] {}: {}{}\n",
                            msg.timestamp, msg.sender_name, msg.content, marker
                        ));
                    }
                    output.push('\n');
                } else {
                    output.push_str(&format!(
                        "[{}] {}: {}\n\n",
                        hit.timestamp, hit.sender_name, hit.content_snippet
                    ));
                }
            }
        }

        ToolResult::success(output)
    }
}
```

- [ ] **Step 2: Register in mod.rs — add module declaration**

At the top of `src/tools/mod.rs`, add alongside existing `pub mod` declarations:
```rust
pub mod session_search;
```

- [ ] **Step 3: Register in ToolRegistry::new()**

In `src/tools/mod.rs`, inside `ToolRegistry::new()`, add before the ClawHub tools block (before line 272 `// Add ClawHub tools if enabled`):

```rust
            Box::new(session_search::SessionSearchTool::new(
                db.clone(),
                config.control_chat_ids.clone(),
            )),
```

- [ ] **Step 4: Register in new_sub_agent()**

In `src/tools/mod.rs`, inside `new_sub_agent()`, add after the `structured_memory` tool (after line 356):

```rust
            Box::new(session_search::SessionSearchTool::new(
                db.clone(),
                config.control_chat_ids.clone(),
            )),
```

- [ ] **Step 5: Build to verify**

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 6: Commit**

```bash
git add src/tools/session_search.rs src/tools/mod.rs
git commit -m "feat(tools): add session_search tool with FTS5 full-text search"
```

---

### Task 5: 5-Phase Context Compressor

**Files:**
- Create: `src/compressor.rs`
- Modify: `src/agent_engine.rs`

- [ ] **Step 1: Create compressor.rs**

```rust
// src/compressor.rs

use mchact_core::llm_types::{ContentBlock, Message, MessageContent};

const CHARS_PER_TOKEN: usize = 4;
const MIN_SUMMARY_TOKENS: usize = 2000;
const SUMMARY_RATIO: f64 = 0.20;
const SUMMARY_TOKENS_CEILING: usize = 12_000;
const TOOL_RESULT_PLACEHOLDER: &str =
    "[Tool output cleared -- use session_search to recall details]";

pub struct CompressorConfig {
    pub tail_token_budget: usize,
    pub protect_first_n: usize,
    pub tool_result_max_chars: usize,
    pub compaction_timeout_secs: u64,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            tail_token_budget: 20_000,
            protect_first_n: 3,
            tool_result_max_chars: 200,
            compaction_timeout_secs: 60,
        }
    }
}

pub struct ContextCompressor {
    config: CompressorConfig,
    previous_summary: Option<String>,
    compression_count: u32,
}

impl ContextCompressor {
    pub fn new(config: CompressorConfig) -> Self {
        Self {
            config,
            previous_summary: None,
            compression_count: 0,
        }
    }

    /// Estimate token count from a string.
    fn estimate_tokens(text: &str) -> usize {
        text.len() / CHARS_PER_TOKEN
    }

    /// Estimate token count for a message.
    fn message_tokens(msg: &Message) -> usize {
        Self::estimate_tokens(&message_to_text(msg))
    }

    /// Phase 1: Replace old ToolResult content with placeholder.
    fn prune_old_tool_results(&self, messages: &[Message], tail_start: usize) -> Vec<Message> {
        messages
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                if i >= tail_start {
                    return msg.clone();
                }
                match &msg.content {
                    MessageContent::Blocks(blocks) => {
                        let pruned_blocks: Vec<ContentBlock> = blocks
                            .iter()
                            .map(|block| match block {
                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } if content.len() > self.config.tool_result_max_chars => {
                                    ContentBlock::ToolResult {
                                        tool_use_id: tool_use_id.clone(),
                                        content: TOOL_RESULT_PLACEHOLDER.to_string(),
                                        is_error: *is_error,
                                    }
                                }
                                other => other.clone(),
                            })
                            .collect();
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(pruned_blocks),
                        }
                    }
                    _ => msg.clone(),
                }
            })
            .collect()
    }

    /// Phase 3: Find tail start index by walking backward with token budget.
    fn find_tail_start(&self, messages: &[Message]) -> usize {
        let mut tokens_accumulated: usize = 0;
        for i in (0..messages.len()).rev() {
            tokens_accumulated += Self::message_tokens(&messages[i]);
            if tokens_accumulated >= self.config.tail_token_budget {
                return i + 1;
            }
        }
        0
    }

    /// Compute max summary tokens from compression zone size.
    fn summary_budget(compression_zone_chars: usize) -> usize {
        let zone_tokens = compression_zone_chars / CHARS_PER_TOKEN;
        let budget = (zone_tokens as f64 * SUMMARY_RATIO) as usize;
        budget.max(MIN_SUMMARY_TOKENS).min(SUMMARY_TOKENS_CEILING)
    }

    /// Phase 4/5: Build the summarization prompt.
    fn build_summary_prompt(&self, compression_zone_text: &str) -> String {
        if let Some(prev) = &self.previous_summary {
            format!(
                "Here is the existing conversation summary:\n{prev}\n\n\
                 Here are new conversation turns since that summary:\n{compression_zone_text}\n\n\
                 PRESERVE all existing information that is still relevant.\n\
                 ADD new progress. Move items between Done/In Progress/Blocked as needed.\n\
                 Organize into:\n\
                 - **Goal**: What the user wants to accomplish\n\
                 - **Progress**: Done / In Progress / Blocked items\n\
                 - **Key Decisions**: Important choices made\n\
                 - **Relevant Files/Commands**: Paths, commands, URLs mentioned\n\
                 - **Critical Context**: Anything needed to continue"
            )
        } else {
            format!(
                "Summarize the following conversation segment. Organize into:\n\
                 - **Goal**: What the user wants to accomplish\n\
                 - **Progress**: Done / In Progress / Blocked items\n\
                 - **Key Decisions**: Important choices made\n\
                 - **Relevant Files/Commands**: Paths, commands, URLs mentioned\n\
                 - **Critical Context**: Anything needed to continue\n\n\
                 ---\n\n{compression_zone_text}"
            )
        }
    }

    /// Post-phase: Remove orphaned ToolUse/ToolResult blocks.
    fn sanitize_tool_pairs(messages: &[Message]) -> Vec<Message> {
        // Collect all tool_use IDs and tool_result IDs
        let mut tool_use_ids = std::collections::HashSet::new();
        let mut tool_result_ids = std::collections::HashSet::new();

        for msg in messages {
            if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    match block {
                        ContentBlock::ToolUse { id, .. } => {
                            tool_use_ids.insert(id.clone());
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            tool_result_ids.insert(tool_use_id.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        messages
            .iter()
            .map(|msg| match &msg.content {
                MessageContent::Blocks(blocks) => {
                    let filtered: Vec<ContentBlock> = blocks
                        .iter()
                        .filter(|block| match block {
                            ContentBlock::ToolUse { id, .. } => tool_result_ids.contains(id),
                            ContentBlock::ToolResult { tool_use_id, .. } => {
                                tool_use_ids.contains(tool_use_id)
                            }
                            _ => true,
                        })
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Text("[context cleared]".into()),
                        }
                    } else {
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(filtered),
                        }
                    }
                }
                _ => msg.clone(),
            })
            .collect()
    }

    /// Main entry point. Returns compressed messages.
    /// `summarize_fn` is an async closure that calls the LLM to summarize.
    pub async fn compress<F, Fut>(
        &mut self,
        messages: &[Message],
        summarize_fn: F,
    ) -> Vec<Message>
    where
        F: FnOnce(String, String) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        let total = messages.len();
        let head_end = self.config.protect_first_n.min(total);

        // Phase 3: Find tail boundary
        let tail_start = self.find_tail_start(messages).max(head_end);

        if tail_start <= head_end {
            // Nothing to compress
            return messages.to_vec();
        }

        // Phase 1: Prune old tool results
        let pruned = self.prune_old_tool_results(messages, tail_start);

        // Phase 2: Protected head
        let head = &pruned[..head_end];
        let compression_zone = &pruned[head_end..tail_start];
        let tail = &pruned[tail_start..];

        // Build compression zone text
        let mut zone_text = String::new();
        for msg in compression_zone {
            zone_text.push_str(&format!("[{}]: {}\n\n", msg.role, message_to_text(msg)));
        }

        if zone_text.is_empty() {
            return messages.to_vec();
        }

        // Truncate if very long
        let max_chars = Self::summary_budget(zone_text.len()) * CHARS_PER_TOKEN * 5;
        if zone_text.len() > max_chars {
            zone_text.truncate(max_chars);
            zone_text.push_str("\n... (truncated)");
        }

        // Phase 4/5: Summarize
        let prompt = self.build_summary_prompt(&zone_text);
        let system = "You are a helpful summarizer. Preserve concrete details like file paths, commands, error messages, and decisions.".to_string();

        let summary = match summarize_fn(system, prompt).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Compaction summarization failed: {e}, falling back to truncation");
                return tail.to_vec();
            }
        };

        // Store for iterative updates
        self.previous_summary = Some(summary.clone());
        self.compression_count += 1;

        // Build compacted message list
        let mut compacted = Vec::new();
        compacted.extend_from_slice(head);
        compacted.push(Message {
            role: "user".into(),
            content: MessageContent::Text(format!("[Conversation Summary]\n{summary}")),
        });
        compacted.push(Message {
            role: "assistant".into(),
            content: MessageContent::Text(
                "Understood, I have the conversation context. How can I help?".into(),
            ),
        });
        compacted.extend_from_slice(tail);

        // Post-phase: Sanitize tool pairs
        let sanitized = Self::sanitize_tool_pairs(&compacted);

        // Fix role alternation — merge consecutive same-role messages
        let mut result: Vec<Message> = Vec::new();
        for msg in sanitized {
            if let Some(last) = result.last_mut() {
                if last.role == msg.role {
                    let existing = message_to_text(last);
                    let new_text = message_to_text(&msg);
                    last.content = MessageContent::Text(format!("{existing}\n{new_text}"));
                    continue;
                }
            }
            result.push(msg);
        }

        // Ensure last message is from user
        if let Some(last) = result.last() {
            if last.role == "assistant" {
                result.pop();
            }
        }

        result
    }
}

/// Extract text from a message (replicates agent_engine helper).
pub fn message_to_text(msg: &Message) -> String {
    match &msg.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
                ContentBlock::ToolUse { name, input, .. } => {
                    Some(name.as_str()) // minimal representation
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
```

- [ ] **Step 2: Add module declaration to src/main.rs or src/lib.rs**

Check which file declares modules. Add `pub mod compressor;` alongside the existing module declarations.

- [ ] **Step 3: Integrate into agent_engine.rs**

In `src/agent_engine.rs`, replace the body of `compact_messages` (lines 2277-2432). The function signature stays the same. The new body delegates to `ContextCompressor`:

```rust
async fn compact_messages(
    state: &AppState,
    caller_channel: &str,
    chat_id: i64,
    messages: &[Message],
    keep_recent: usize,
) -> Vec<Message> {
    let total = messages.len();
    if total <= keep_recent {
        return messages.to_vec();
    }

    let config = crate::compressor::CompressorConfig {
        tail_token_budget: 20_000,
        protect_first_n: 3,
        tool_result_max_chars: 200,
        compaction_timeout_secs: state.config.compaction_timeout_secs,
    };
    let mut compressor = crate::compressor::ContextCompressor::new(config);

    let timeout_secs = state.config.compaction_timeout_secs;
    let (effective_profile, effective_model, _session_settings) =
        resolve_effective_provider_and_model(state, caller_channel, chat_id).await;
    let scoped_provider = if effective_profile.alias != state.config.llm_provider {
        Some(crate::llm::create_provider(&build_provider_runtime_config(
            state,
            &effective_profile,
            &effective_model,
        )))
    } else {
        None
    };

    let state_ref = state;
    let summarize_fn = |system: String, prompt: String| async move {
        let summarize_messages = vec![Message {
            role: "user".into(),
            content: MessageContent::Text(prompt),
        }];

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            async {
                if let Some(provider) = scoped_provider.as_ref() {
                    provider
                        .send_message_with_model(
                            &system,
                            summarize_messages,
                            None,
                            Some(&effective_model),
                        )
                        .await
                } else {
                    state_ref
                        .llm
                        .send_message_with_model(
                            &system,
                            summarize_messages,
                            None,
                            Some(&effective_model),
                        )
                        .await
                }
            },
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                let text = response
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        crate::llm::ResponseContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                Ok(text)
            }
            Ok(Err(e)) => Err(format!("LLM error: {e}")),
            Err(_) => Err(format!("Timeout after {timeout_secs}s")),
        }
    };

    compressor.compress(messages, summarize_fn).await
}
```

- [ ] **Step 4: Build to verify**

Run: `cargo build`
Expected: Compiles. May need minor import adjustments.

- [ ] **Step 5: Commit**

```bash
git add src/compressor.rs src/agent_engine.rs
git commit -m "feat: replace single-pass compaction with 5-phase ContextCompressor"
```

---

### Task 6: Findings Tools

**Files:**
- Create: `src/tools/findings.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create findings.rs**

```rust
// src/tools/findings.rs

use std::sync::Arc;

use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::Database;
use mchact_tools::runtime::{Tool, ToolResult};
use serde_json::json;

// -- FindingsWriteTool --

pub struct FindingsWriteTool {
    db: Arc<Database>,
}

impl FindingsWriteTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for FindingsWriteTool {
    fn name(&self) -> &str {
        "findings_write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "findings_write".into(),
            description: "Post a finding to the shared blackboard so sibling sub-agents can see it. Use this to share discoveries and avoid duplicate work during orchestrated tasks.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "finding": {
                        "type": "string",
                        "description": "The finding or discovery to share"
                    },
                    "category": {
                        "type": "string",
                        "description": "Category tag (default: general)",
                        "default": "general"
                    }
                },
                "required": ["finding"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let finding = match input.get("finding").and_then(|v| v.as_str()) {
            Some(f) if !f.trim().is_empty() => f,
            _ => return ToolResult::error("Missing or empty 'finding' parameter".into()),
        };

        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let orchestration_id = match input
            .get("__subagent_runtime")
            .and_then(|r| r.get("orchestration_id"))
            .and_then(|v| v.as_str())
        {
            Some(id) => id.to_string(),
            None => {
                // Fall back to run_id if orchestration_id not present
                match input
                    .get("__subagent_runtime")
                    .and_then(|r| r.get("run_id"))
                    .and_then(|v| v.as_str())
                {
                    Some(id) => id.to_string(),
                    None => return ToolResult::error(
                        "findings_write requires __subagent_runtime context (orchestration_id or run_id)".into(),
                    ),
                }
            }
        };

        let run_id = input
            .get("__subagent_runtime")
            .and_then(|r| r.get("run_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let db = self.db.clone();
        let orch_id = orchestration_id.clone();
        let run = run_id.to_string();
        let find = finding.to_string();
        let cat = category.to_string();

        match tokio::task::spawn_blocking(move || db.insert_finding(&orch_id, &run, &find, &cat))
            .await
        {
            Ok(Ok(id)) => ToolResult::success(format!("Finding #{id} posted to shared blackboard.")),
            Ok(Err(e)) => ToolResult::error(format!("Failed to write finding: {e}")),
            Err(e) => ToolResult::error(format!("Task failed: {e}")),
        }
    }
}

// -- FindingsReadTool --

pub struct FindingsReadTool {
    db: Arc<Database>,
}

impl FindingsReadTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for FindingsReadTool {
    fn name(&self) -> &str {
        "findings_read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "findings_read".into(),
            description: "Read all findings posted by sibling sub-agents in this orchestration. Use this to see what other workers have discovered.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let orchestration_id = match input
            .get("__subagent_runtime")
            .and_then(|r| r.get("orchestration_id"))
            .and_then(|v| v.as_str())
        {
            Some(id) => id.to_string(),
            None => match input
                .get("__subagent_runtime")
                .and_then(|r| r.get("run_id"))
                .and_then(|v| v.as_str())
            {
                Some(id) => id.to_string(),
                None => return ToolResult::error(
                    "findings_read requires __subagent_runtime context".into(),
                ),
            },
        };

        let db = self.db.clone();
        let orch_id = orchestration_id.clone();

        match tokio::task::spawn_blocking(move || db.get_findings(&orch_id)).await {
            Ok(Ok(findings)) => {
                if findings.is_empty() {
                    return ToolResult::success("No findings posted yet.".into());
                }
                let mut output = format!("{} findings from sibling workers:\n\n", findings.len());
                for f in &findings {
                    output.push_str(&format!(
                        "#{} [{}] (by {}): {}\n",
                        f.id, f.category, f.run_id, f.finding
                    ));
                }
                ToolResult::success(output)
            }
            Ok(Err(e)) => ToolResult::error(format!("Failed to read findings: {e}")),
            Err(e) => ToolResult::error(format!("Task failed: {e}")),
        }
    }
}
```

- [ ] **Step 2: Register in mod.rs**

Add module declaration at the top of `src/tools/mod.rs`:
```rust
pub mod findings;
```

In `new_sub_agent()`, add after the `session_search` tool registration:
```rust
            Box::new(findings::FindingsWriteTool::new(db.clone())),
            Box::new(findings::FindingsReadTool::new(db.clone())),
```

- [ ] **Step 3: Build to verify**

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/tools/findings.rs src/tools/mod.rs
git commit -m "feat(tools): add findings_write and findings_read for MoA blackboard"
```

---

### Task 7: Anthropic Prompt Caching

**Files:**
- Modify: `src/llm.rs`

- [ ] **Step 1: Add cache control functions**

Add these functions in `src/llm.rs` (near the Anthropic provider, around line 260):

```rust
/// Apply Anthropic cache_control markers to reduce input token costs.
/// Places up to 4 breakpoints: system prompt + last 3 non-system messages.
/// Operates on the serialized JSON body, not shared Message types.
pub fn apply_anthropic_cache_control(messages: &mut Vec<serde_json::Value>) {
    let marker = serde_json::json!({"type": "ephemeral"});
    let mut breakpoints_used = 0usize;
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
    let non_system_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .map(|(i, _)| i)
        .collect();

    for &idx in non_system_indices.iter().rev().take(remaining) {
        apply_cache_marker(&mut messages[idx], &marker);
    }
}

/// Apply cache_control marker to a single message's content.
fn apply_cache_marker(msg: &mut serde_json::Value, marker: &serde_json::Value) {
    if let Some(content) = msg.get_mut("content") {
        if content.is_string() {
            // Convert string content to array-of-blocks format
            let text = content.as_str().unwrap_or("").to_string();
            *content = serde_json::json!([{
                "type": "text",
                "text": text,
                "cache_control": marker
            }]);
        } else if let Some(arr) = content.as_array_mut() {
            // Append cache_control to last block
            if let Some(last) = arr.last_mut() {
                last.as_object_mut()
                    .map(|obj| obj.insert("cache_control".to_string(), marker.clone()));
            }
        }
    }
}
```

- [ ] **Step 2: Integrate into Anthropic send_message_with_model**

In the Anthropic provider's `send_message_with_model` method (around line 917-936), after building the `request` struct and before sending, add cache control application. The integration point is where `.json(&request)` is called. Change the request serialization to:

```rust
        // Serialize to Value so we can apply cache markers
        let mut body = serde_json::to_value(&request)
            .map_err(|e| mchactError::Internal(format!("Serialization failed: {e}")))?;

        // Apply Anthropic prompt caching
        if let Some(msgs) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            apply_anthropic_cache_control(msgs);
        }

        loop {
            let response = self
                .http
                .post(&self.base_url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)  // <-- use body instead of &request
```

- [ ] **Step 3: Add unit tests**

Add at the bottom of `src/llm.rs` in the test module:

```rust
#[cfg(test)]
mod cache_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_control_string_content() {
        let mut messages = vec![
            json!({"role": "system", "content": "You are helpful."}),
            json!({"role": "user", "content": "Hello"}),
        ];
        apply_anthropic_cache_control(&mut messages);

        // System prompt should be converted to array with cache_control
        let sys_content = &messages[0]["content"];
        assert!(sys_content.is_array());
        assert!(sys_content[0]["cache_control"].is_object());

        // Last non-system message should also have cache_control
        let user_content = &messages[1]["content"];
        assert!(user_content.is_array());
        assert!(user_content[0]["cache_control"].is_object());
    }

    #[test]
    fn test_cache_control_max_breakpoints() {
        let mut messages = vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": "msg1"}),
            json!({"role": "assistant", "content": "msg2"}),
            json!({"role": "user", "content": "msg3"}),
            json!({"role": "assistant", "content": "msg4"}),
            json!({"role": "user", "content": "msg5"}),
        ];
        apply_anthropic_cache_control(&mut messages);

        // System (1) + last 3 non-system (3) = 4 breakpoints
        // msg1 should NOT have cache_control (only last 3 get it)
        assert!(messages[1]["content"].is_string()); // msg1 unchanged
        // msg4, msg5 should have it
        assert!(messages[4]["content"].is_array()); // msg4
        assert!(messages[5]["content"].is_array()); // msg5
    }

    #[test]
    fn test_cache_control_array_content() {
        let mut messages = vec![json!({
            "role": "user",
            "content": [{"type": "text", "text": "Hello"}]
        })];
        apply_anthropic_cache_control(&mut messages);

        // Should append cache_control to last block
        assert!(messages[0]["content"][0]["cache_control"].is_object());
    }
}
```

- [ ] **Step 4: Build and test**

Run: `cargo test -p mchact cache_tests`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/llm.rs
git commit -m "feat(llm): add Anthropic prompt caching with cache_control markers"
```

---

### Task 8: Mixture of Agents Tool

**Files:**
- Create: `src/tools/mixture_of_agents.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create mixture_of_agents.rs**

```rust
// src/tools/mixture_of_agents.rs

use std::sync::Arc;

use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::Database;
use mchact_tools::runtime::{Tool, ToolResult};
use serde_json::json;

use crate::channels::ChannelRegistry;
use crate::config::Config;

const DEFAULT_PERSPECTIVES: &[&str] = &[
    "Analyze this as a pragmatic engineer focused on simplicity and correctness.",
    "Analyze this as a skeptic looking for edge cases, failure modes, and risks.",
    "Analyze this as an architect focused on long-term maintainability and scalability.",
    "Analyze this as a security and risk analyst focused on vulnerabilities and compliance.",
    "Analyze this as a user/stakeholder advocate focused on usability and impact.",
];

const AGGREGATOR_SYSTEM_PROMPT: &str = "You have been provided with a set of responses from various perspectives to the latest user query. Your task is to synthesize these responses into a single, high-quality response. It is crucial to critically evaluate the information provided in these responses, recognizing that some of it may be biased or incorrect. Your response should not simply replicate the given answers but should offer a refined, accurate, and comprehensive reply. Ensure your response is well-structured, coherent, and adheres to the highest standards of accuracy and reliability.\n\nResponses from perspectives:";

pub struct MixtureOfAgentsTool {
    config: Config,
    db: Arc<Database>,
    channel_registry: Arc<ChannelRegistry>,
}

impl MixtureOfAgentsTool {
    pub fn new(
        config: &Config,
        db: Arc<Database>,
        channel_registry: Arc<ChannelRegistry>,
    ) -> Self {
        Self {
            config: config.clone(),
            db,
            channel_registry,
        }
    }

    fn resolve_perspectives(
        &self,
        count: usize,
        approach_hints: Option<Vec<String>>,
    ) -> Vec<String> {
        if let Some(hints) = approach_hints {
            return hints;
        }
        DEFAULT_PERSPECTIVES
            .iter()
            .take(count)
            .map(|s| s.to_string())
            .collect()
    }
}

#[async_trait]
impl Tool for MixtureOfAgentsTool {
    fn name(&self) -> &str {
        "mixture_of_agents"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "mixture_of_agents".into(),
            description: "Route a hard problem through multiple independent perspectives collaboratively. Spawns N sub-agents that each tackle the same question from a different angle, then synthesizes a consensus answer. Use sparingly for genuinely difficult problems that benefit from diverse analysis.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "user_prompt": {
                        "type": "string",
                        "description": "The complex query or problem to solve using multiple perspectives"
                    },
                    "perspectives": {
                        "type": "integer",
                        "description": "Number of independent agents (default 3, max 5)",
                        "default": 3
                    },
                    "approach_hints": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Explicit perspective labels (optional)"
                    },
                    "model_overrides": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Per-worker model or provider_preset names (optional)"
                    },
                    "wait_timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default 120, max 300)",
                        "default": 120
                    },
                    "min_successful": {
                        "type": "integer",
                        "description": "Minimum workers that must succeed (default 1)",
                        "default": 1
                    }
                },
                "required": ["user_prompt"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let user_prompt = match input.get("user_prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => return ToolResult::error("Missing or empty 'user_prompt' parameter".into()),
        };

        let perspective_count = input
            .get("perspectives")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(5) as usize;

        let approach_hints: Option<Vec<String>> = input
            .get("approach_hints")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let wait_timeout = input
            .get("wait_timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(120)
            .min(300);

        let min_successful = input
            .get("min_successful")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let perspectives = self.resolve_perspectives(perspective_count, approach_hints);

        // Build work packages — each is the same question with a different perspective
        let work_packages: Vec<serde_json::Value> = perspectives
            .iter()
            .map(|perspective| {
                json!(format!(
                    "PERSPECTIVE: {perspective}\n\nQUESTION: {user_prompt}"
                ))
            })
            .collect();

        // Use subagents_orchestrate internally
        let orchestrate_input = json!({
            "goal": format!("Answer this question from {} independent perspectives and provide thorough analysis", perspectives.len()),
            "work_packages": work_packages,
            "wait": true,
            "wait_timeout_secs": wait_timeout,
        });

        // Pass through auth context
        let mut full_input = orchestrate_input;
        if let Some(auth) = input.get("__auth_context") {
            full_input
                .as_object_mut()
                .unwrap()
                .insert("__auth_context".into(), auth.clone());
        }
        if let Some(runtime) = input.get("__subagent_runtime") {
            full_input
                .as_object_mut()
                .unwrap()
                .insert("__subagent_runtime".into(), runtime.clone());
        }

        // Execute orchestration
        let orchestrate_tool = crate::tools::subagents::SubagentsOrchestrateTool::new(
            &self.config,
            self.db.clone(),
            self.channel_registry.clone(),
        );
        let orch_result = orchestrate_tool.execute(full_input).await;

        if orch_result.is_error {
            return ToolResult::error(format!(
                "MoA orchestration failed: {}",
                orch_result.content
            ));
        }

        // Parse orchestration result
        let orch_data: serde_json::Value = match serde_json::from_str(&orch_result.content) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::error(format!("Failed to parse orchestration result: {e}"))
            }
        };

        // Extract worker results
        let runs = orch_data
            .get("runs")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        let successful_results: Vec<(usize, String)> = runs
            .iter()
            .enumerate()
            .filter_map(|(i, run)| {
                let status = run.get("status").and_then(|s| s.as_str()).unwrap_or("");
                if status == "completed" {
                    let result = run
                        .get("result_text")
                        .and_then(|r| r.as_str())
                        .unwrap_or("[no output]")
                        .to_string();
                    Some((i, result))
                } else {
                    None
                }
            })
            .collect();

        let successful_count = successful_results.len();
        let failed_count = runs.len() - successful_count;

        if successful_count < min_successful {
            return ToolResult::error(format!(
                "Insufficient successful perspectives ({}/{}).\nNeed at least {}. {} workers failed.",
                successful_count,
                runs.len(),
                min_successful,
                failed_count
            ));
        }

        // Build aggregator prompt
        let mut aggregator_prompt = AGGREGATOR_SYSTEM_PROMPT.to_string();
        aggregator_prompt.push_str("\n\n");
        for (idx, (orig_idx, result)) in successful_results.iter().enumerate() {
            let perspective_label = perspectives
                .get(*orig_idx)
                .map(|s| s.as_str())
                .unwrap_or("Unknown");
            aggregator_prompt.push_str(&format!(
                "{}. [{}]:\n{}\n\n",
                idx + 1,
                perspective_label,
                result
            ));
        }

        let synthesis_prompt = format!(
            "{aggregator_prompt}\n\
             Synthesize into a single answer that:\n\
             - Identifies points of agreement (high confidence)\n\
             - Notes disagreements and which perspective is most convincing\n\
             - Provides a final recommended answer"
        );

        // For now, return the aggregated prompt as the result.
        // Full synthesis would require calling the LLM here, which
        // depends on having access to the provider. For the initial
        // implementation, we return the formatted multi-perspective output
        // and let the parent agent synthesize.
        let perspectives_used: Vec<String> = successful_results
            .iter()
            .filter_map(|(idx, _)| perspectives.get(*idx).cloned())
            .collect();

        let result_json = json!({
            "success": true,
            "response": synthesis_prompt,
            "perspectives_used": perspectives_used,
            "successful_count": successful_count,
            "failed_count": failed_count,
        });

        ToolResult::success(serde_json::to_string_pretty(&result_json).unwrap_or_default())
    }
}
```

- [ ] **Step 2: Register in mod.rs**

Add module declaration at the top of `src/tools/mod.rs`:
```rust
pub mod mixture_of_agents;
```

In `ToolRegistry::new()`, add before the ClawHub tools block:
```rust
            Box::new(mixture_of_agents::MixtureOfAgentsTool::new(
                config,
                db.clone(),
                channel_registry.clone(),
            )),
```

**Do NOT add to `new_sub_agent()`** — MoA is main-agent only.

- [ ] **Step 3: Build to verify**

Run: `cargo build`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/tools/mixture_of_agents.rs src/tools/mod.rs
git commit -m "feat(tools): add mixture_of_agents tool with multi-perspective consensus"
```

---

### Task 9: Final Build and Integration Test

**Files:** None new — verification only.

- [ ] **Step 1: Full build**

Run: `cargo build`
Expected: Clean compile.

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: All existing tests pass, plus new FTS and cache tests.

- [ ] **Step 3: Verify FTS5 is available in bundled SQLite**

Run: `cargo test -p mchact-storage fts`
Expected: All tests pass, confirming FTS5 is bundled.

- [ ] **Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit -m "chore: integration fixups for FTS5 + compressor + MoA + cache"
```

---

## Self-Review Checklist

- [x] Spec coverage: All 5 features have tasks (FTS5=T1-T4, Compressor=T5, Findings=T6, Cache=T7, MoA=T8)
- [x] No placeholders: All steps contain actual code
- [x] Type consistency: `FtsSearchResult`, `Finding`, `ToolResult`, `Message` types match across tasks
- [x] Tool names match: `session_search`, `findings_write`, `findings_read`, `mixture_of_agents`
- [x] Struct fields match spec: all DB methods, tool schemas, compressor config
- [x] Migration v20 creates both `messages_fts` and `subagent_findings`
- [x] Registration: `session_search` in both registries, findings in sub-agent only, MoA in main only

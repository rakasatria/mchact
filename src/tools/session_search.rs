use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::{auth_context_from_input, schema_object, Tool, ToolResult};
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::{call_blocking, FtsSearchResult, StoredMessage};
use mchact_storage::DynDataStore;
use mchact_storage::prelude::*;

pub struct SessionSearchTool {
    db: Arc<DynDataStore>,
    control_chat_ids: Vec<i64>,
}

impl SessionSearchTool {
    pub fn new(db: Arc<DynDataStore>, control_chat_ids: Vec<i64>) -> Self {
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
            description: "Full-text search across all past chat messages. Returns matching messages with surrounding context. Control chats can search across all chats; other chats are restricted to their own history.".into(),
            input_schema: schema_object(
                json!({
                    "query": {
                        "type": "string",
                        "description": "The search query to find in past messages"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of matches to return (default 10, max 30)"
                    },
                    "context_window": {
                        "type": "integer",
                        "description": "Number of messages before and after each match to include as context (default 2, max 5)"
                    },
                    "chat_id": {
                        "type": "integer",
                        "description": "Filter results to a specific chat ID. Control chats can search any chat; non-control chats are always restricted to their own chat."
                    }
                }),
                &["query"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.trim().to_string(),
            _ => return ToolResult::error("Missing or empty required parameter: query".into()),
        };

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n.min(30) as usize)
            .unwrap_or(10);

        let context_window = input
            .get("context_window")
            .and_then(|v| v.as_u64())
            .map(|n| n.min(5) as usize)
            .unwrap_or(2);

        let auth = auth_context_from_input(&input);
        let caller_chat_id = auth.as_ref().map(|a| a.caller_chat_id).unwrap_or(0);
        let is_control = auth
            .as_ref()
            .map(|a| self.control_chat_ids.contains(&a.caller_chat_id))
            .unwrap_or(false);

        // Determine chat_id filter: non-control chats are always restricted to their own chat
        let chat_id_filter: Option<i64> = if is_control {
            input.get("chat_id").and_then(|v| v.as_i64())
        } else {
            Some(caller_chat_id)
        };

        let db = self.db.clone();
        let query_clone = query.clone();
        let hits: Vec<FtsSearchResult> =
            match call_blocking(db.clone(), move |db| {
                db.search_messages_fts(&query_clone, chat_id_filter, limit)
            })
            .await
            {
                Ok(results) => results,
                Err(e) => return ToolResult::error(format!("Search failed: {e}")),
            };

        if hits.is_empty() {
            return ToolResult::success(format!("No messages found matching \"{}\".", query));
        }

        // Collect context for each hit
        let mut context_by_hit: Vec<Vec<StoredMessage>> = Vec::with_capacity(hits.len());
        for hit in &hits {
            let db = db.clone();
            let hit_chat_id = hit.chat_id;
            let hit_timestamp = hit.timestamp.clone();
            let window = context_window;
            let context = match call_blocking(db, move |db| {
                db.get_message_context(hit_chat_id, &hit_timestamp, window)
            })
            .await
            {
                Ok(ctx) => ctx,
                Err(e) => {
                    tracing::warn!("Failed to get context for message {}: {e}", hit.message_id);
                    vec![]
                }
            };
            context_by_hit.push(context);
        }

        // Group by chat_id
        // We preserve insertion order by tracking chat_id order
        let mut chat_order: Vec<i64> = Vec::new();
        let mut groups: HashMap<i64, Vec<(usize, &FtsSearchResult)>> = HashMap::new();
        for (idx, hit) in hits.iter().enumerate() {
            let entry = groups.entry(hit.chat_id).or_insert_with(|| {
                chat_order.push(hit.chat_id);
                Vec::new()
            });
            entry.push((idx, hit));
        }

        // Build formatted output
        let mut output = format!(
            "Found {} match(es) for \"{}\":\n\n",
            hits.len(),
            query
        );

        for chat_id in &chat_order {
            let group_hits = &groups[chat_id];
            let first_hit = group_hits[0].1;
            let chat_title = first_hit
                .chat_title
                .as_deref()
                .unwrap_or("(unknown chat)");
            output.push_str(&format!(
                "## Chat: {} (id: {})\n\n",
                chat_title, chat_id
            ));

            for (hit_idx, (ctx_idx, hit)) in group_hits.iter().enumerate() {
                let context = &context_by_hit[*ctx_idx];
                output.push_str(&format!(
                    "### Match {} — {} at {}\n",
                    hit_idx + 1,
                    hit.sender_name,
                    hit.timestamp,
                ));
                output.push_str(&format!("**Snippet:** {}\n\n", hit.content_snippet));

                if !context.is_empty() {
                    output.push_str("**Context:**\n");
                    for msg in context {
                        let sender = if msg.is_from_bot {
                            format!("[bot] {}", msg.sender_name)
                        } else {
                            msg.sender_name.clone()
                        };
                        // Mark the matching message
                        if msg.id == hit.message_id {
                            output.push_str(&format!(
                                "  > **{}** ({}): {}\n",
                                sender, msg.timestamp, msg.content
                            ));
                        } else {
                            output.push_str(&format!(
                                "    {} ({}): {}\n",
                                sender, msg.timestamp, msg.content
                            ));
                        }
                    }
                    output.push('\n');
                }
            }
        }

        ToolResult::success(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage::db::{Database, StoredMessage};
    use serde_json::json;

    fn make_db() -> Arc<Database> {
        let dir = std::env::temp_dir().join(format!(
            "mchact_session_search_{}",
            uuid::Uuid::new_v4()
        ));
        Arc::new(Database::new(dir.to_str().unwrap()).unwrap())
    }

    fn store_msg(db: &Database, chat_id: i64, id: &str, sender: &str, content: &str, ts: &str) {
        db.store_message(&StoredMessage {
            id: id.to_string(),
            chat_id,
            sender_name: sender.to_string(),
            content: content.to_string(),
            is_from_bot: false,
            timestamp: ts.to_string(),
        })
        .unwrap();
    }

    #[tokio::test]
    async fn test_empty_query_returns_error() {
        let db = make_db();
        let tool = SessionSearchTool::new(db, vec![]);
        let result = tool.execute(json!({"query": ""})).await;
        assert!(result.is_error);
        assert!(result.content.contains("empty"));
    }

    #[tokio::test]
    async fn test_no_results_returns_success_with_message() {
        let db = make_db();
        let tool = SessionSearchTool::new(db, vec![]);
        let result = tool
            .execute(json!({
                "query": "zzznomatch",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 1,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error);
        assert!(result.content.contains("No messages found"));
    }

    #[tokio::test]
    async fn test_finds_matching_message() {
        let db = make_db();
        store_msg(
            &db,
            100,
            "m1",
            "alice",
            "deployment pipeline test message",
            "2024-01-01T00:00:01Z",
        );

        let tool = SessionSearchTool::new(db, vec![100]);
        let result = tool
            .execute(json!({
                "query": "deployment pipeline",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 100,
                    "control_chat_ids": [100]
                }
            }))
            .await;
        assert!(!result.is_error, "Error: {}", result.content);
        assert!(result.content.contains("Match"));
        assert!(result.content.contains("alice"));
    }

    #[tokio::test]
    async fn test_non_control_restricted_to_own_chat() {
        let db = make_db();
        // Message in chat 200 - non-control caller from chat 100 should NOT find it
        store_msg(
            &db,
            200,
            "m_other",
            "bob",
            "secret project planning details",
            "2024-01-01T00:00:01Z",
        );

        let tool = SessionSearchTool::new(db, vec![999]); // 100 is not control
        let result = tool
            .execute(json!({
                "query": "secret project",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 100,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error);
        // Should find nothing because restricted to chat 100 which has no messages
        assert!(result.content.contains("No messages found"));
    }

    #[tokio::test]
    async fn test_control_chat_can_search_globally() {
        let db = make_db();
        store_msg(
            &db,
            200,
            "m_other",
            "bob",
            "cross chat search global content",
            "2024-01-01T00:00:01Z",
        );

        let tool = SessionSearchTool::new(db, vec![100]); // 100 is control
        let result = tool
            .execute(json!({
                "query": "cross chat search global",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 100,
                    "control_chat_ids": [100]
                }
            }))
            .await;
        assert!(!result.is_error, "Error: {}", result.content);
        assert!(result.content.contains("bob"));
    }

    #[tokio::test]
    async fn test_limit_respected() {
        let db = make_db();
        for i in 0..10 {
            store_msg(
                &db,
                300,
                &format!("msg{i}"),
                "user",
                &format!("unique repeatable keyword occurrence {i}"),
                &format!("2024-01-01T00:00:{:02}Z", i + 1),
            );
        }

        let tool = SessionSearchTool::new(db, vec![300]);
        let result = tool
            .execute(json!({
                "query": "unique repeatable keyword occurrence",
                "limit": 3,
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 300,
                    "control_chat_ids": [300]
                }
            }))
            .await;
        assert!(!result.is_error, "Error: {}", result.content);
        // Should contain at most 3 matches
        let match_count = result.content.matches("### Match").count();
        assert!(
            match_count <= 3,
            "Expected at most 3 matches, got {match_count}"
        );
    }
}

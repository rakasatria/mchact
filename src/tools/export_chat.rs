use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::{authorize_chat_access, schema_object, Tool, ToolResult};
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::call_blocking;
use mchact_storage::DynDataStore;
use mchact_storage_backend::ObjectStorage;

pub struct ExportChatTool {
    db: Arc<DynDataStore>,
    storage: Arc<dyn ObjectStorage>,
}

impl ExportChatTool {
    pub fn new(db: Arc<DynDataStore>, storage: Arc<dyn ObjectStorage>) -> Self {
        ExportChatTool { db, storage }
    }
}

#[async_trait]
impl Tool for ExportChatTool {
    fn name(&self) -> &str {
        "export_chat"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "export_chat".into(),
            description: "Export chat history to a markdown file. Returns the file path.".into(),
            input_schema: schema_object(
                json!({
                    "chat_id": {
                        "type": "integer",
                        "description": "The chat ID to export"
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional output file path. Defaults to data/exports/{chat_id}_{timestamp}.md"
                    }
                }),
                &["chat_id"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let chat_id = match input.get("chat_id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => return ToolResult::error("Missing required parameter: chat_id".into()),
        };
        if let Err(e) = authorize_chat_access(&input, chat_id) {
            return ToolResult::error(e);
        }

        let messages =
            match call_blocking(self.db.clone(), move |db| db.get_all_messages(chat_id)).await {
                Ok(msgs) => msgs,
                Err(e) => return ToolResult::error(format!("Failed to load messages: {e}")),
            };

        if messages.is_empty() {
            return ToolResult::error(format!("No messages found for chat {chat_id}."));
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let key = format!("exports/{}_{}.md", chat_id, timestamp);

        // Build markdown
        let mut md = format!("# Chat Export: {chat_id}\n\n");
        md.push_str(&format!(
            "Exported at: {}\n\n---\n\n",
            chrono::Utc::now().to_rfc3339()
        ));

        for msg in &messages {
            let sender = if msg.is_from_bot {
                "**Bot**"
            } else {
                &msg.sender_name
            };
            md.push_str(&format!(
                "**{}** ({})\n\n{}\n\n---\n\n",
                sender, msg.timestamp, msg.content
            ));
        }

        match self.storage.put(&key, md.as_bytes().to_vec()).await {
            Ok(()) => ToolResult::success(format!(
                "Exported {} messages to {}",
                messages.len(),
                key
            )),
            Err(e) => ToolResult::error(format!("Failed to write export: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage::db::{Database, StoredMessage};
    use mchact_storage::prelude::*;
    use mchact_storage_backend::local::LocalStorage;

    async fn test_setup() -> (Arc<Database>, Arc<dyn ObjectStorage>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mchact_export_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(dir.to_str().unwrap()).unwrap());
        let storage: Arc<dyn ObjectStorage> =
            Arc::new(LocalStorage::new(dir.to_str().unwrap()).await.unwrap());
        (db, storage, dir)
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_export_empty_chat() {
        let (db, storage, dir) = test_setup().await;
        let tool = ExportChatTool::new(db, storage);
        let result = tool.execute(json!({"chat_id": 999})).await;
        assert!(result.is_error);
        assert!(result.content.contains("No messages"));
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_export_chat_success() {
        let (db, storage, dir) = test_setup().await;
        db.store_message(&StoredMessage {
            id: "m1".into(),
            chat_id: 100,
            sender_name: "alice".into(),
            content: "hello".into(),
            is_from_bot: false,
            timestamp: "2024-01-01T00:00:01Z".into(),
        })
        .unwrap();
        db.store_message(&StoredMessage {
            id: "m2".into(),
            chat_id: 100,
            sender_name: "bot".into(),
            content: "hi there!".into(),
            is_from_bot: true,
            timestamp: "2024-01-01T00:00:02Z".into(),
        })
        .unwrap();

        let tool = ExportChatTool::new(db, storage.clone());
        let result = tool.execute(json!({"chat_id": 100})).await;
        assert!(!result.is_error, "Error: {}", result.content);
        assert!(result.content.contains("2 messages"));

        // Read back from storage to verify content
        let key = result
            .content
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_string();
        let bytes = storage.get(&key).await.unwrap();
        let content = String::from_utf8(bytes).unwrap();
        assert!(content.contains("alice"));
        assert!(content.contains("hello"));
        assert!(content.contains("**Bot**"));
        assert!(content.contains("hi there!"));
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_export_chat_permission_denied() {
        let (db, storage, dir) = test_setup().await;
        db.store_message(&StoredMessage {
            id: "m1".into(),
            chat_id: 200,
            sender_name: "alice".into(),
            content: "hello".into(),
            is_from_bot: false,
            timestamp: "2024-01-01T00:00:01Z".into(),
        })
        .unwrap();

        let tool = ExportChatTool::new(db, storage);
        let result = tool
            .execute(json!({
                "chat_id": 200,
                "__mchact_auth": {
                    "caller_chat_id": 100,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Permission denied"));
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_export_chat_allowed_for_control_chat_cross_chat() {
        let (db, storage, dir) = test_setup().await;
        db.store_message(&StoredMessage {
            id: "m1".into(),
            chat_id: 200,
            sender_name: "alice".into(),
            content: "hello".into(),
            is_from_bot: false,
            timestamp: "2024-01-01T00:00:01Z".into(),
        })
        .unwrap();
        let tool = ExportChatTool::new(db, storage.clone());
        let result = tool
            .execute(json!({
                "chat_id": 200,
                "__mchact_auth": {
                    "caller_chat_id": 100,
                    "control_chat_ids": [100]
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        // Read back from storage
        let key = result
            .content
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_string();
        let bytes = storage.get(&key).await.unwrap();
        let content = String::from_utf8(bytes).unwrap();
        assert!(content.contains("hello"));
        cleanup(&dir);
    }
}

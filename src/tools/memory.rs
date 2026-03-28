use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::memory_backend::MemoryBackend;
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::DynDataStore;
use mchact_storage::memory_quality;
use mchact_storage_backend::{ObjectStorage, StorageError};

use super::{auth_context_from_input, authorize_chat_access, schema_object, Tool, ToolResult};

pub struct ReadMemoryTool {
    storage: Arc<dyn ObjectStorage>,
    db: Arc<DynDataStore>,
}

impl ReadMemoryTool {
    pub fn new(storage: Arc<dyn ObjectStorage>, db: Arc<DynDataStore>) -> Self {
        ReadMemoryTool { storage, db }
    }
}

fn memory_channel_from_auth(input: &serde_json::Value) -> String {
    auth_context_from_input(input)
        .map(|a| a.caller_channel)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "web".to_string())
}

fn chat_id_from_input_or_auth(input: &serde_json::Value) -> Option<i64> {
    input
        .get("chat_id")
        .and_then(|v| v.as_i64())
        .or_else(|| auth_context_from_input(input).map(|a| a.caller_chat_id))
}

fn memory_channel_for_chat(
    db: &DynDataStore,
    input: &serde_json::Value,
    chat_id: i64,
) -> Result<String, String> {
    if let Some(channel) = db
        .get_chat_channel(chat_id)
        .map_err(|e| format!("Failed to resolve chat channel for {chat_id}: {e}"))?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Ok(channel)
    } else {
        Ok(memory_channel_from_auth(input))
    }
}

fn latest_sender_for_chat(db: &DynDataStore, chat_id: i64) -> Option<String> {
    db.get_recent_messages(chat_id, 20)
        .ok()?
        .into_iter()
        .rev()
        .find(|m| !m.is_from_bot && !m.content.trim_start().starts_with('/'))
        .map(|m| m.sender_name.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn find_person_section_range(lines: &[&str], header: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| line.trim() == header)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim_start().starts_with("## Person:"))
        .map(|(idx, _)| idx)
        .unwrap_or(lines.len());
    Some((start, end))
}

fn upsert_chat_person_memory(existing: &str, sender: &str, content: &str) -> String {
    let sender = sender.trim();
    let content = content.trim();
    if sender.is_empty() || content.is_empty() {
        return content.to_string();
    }

    let section_header = format!("## Person: {sender}");
    let section_block = format!("{section_header}\n{content}\n");
    let existing_trimmed = existing.trim();
    if existing_trimmed.is_empty() {
        return section_block;
    }

    let lines: Vec<&str> = existing_trimmed.lines().collect();
    if let Some((start, end)) = find_person_section_range(&lines, &section_header) {
        let mut out = String::new();
        for line in &lines[..start] {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&section_block);
        for line in &lines[end..] {
            out.push_str(line);
            out.push('\n');
        }
        return out;
    }

    let mut out = String::new();
    out.push_str(existing_trimmed);
    if !existing_trimmed.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&section_block);
    out
}

#[async_trait]
impl Tool for ReadMemoryTool {
    fn name(&self) -> &str {
        "read_memory"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_memory".into(),
            description: "Read internal AGENTS.md memory context for reasoning. Use scope 'global' for memories shared across all chats, 'bot' for the current bot/account, or 'chat' for chat-specific memories. Do not echo raw memory blocks/IDs directly to users; summarize in natural language.".into(),
            input_schema: schema_object(
                json!({
                    "scope": {
                        "type": "string",
                        "description": "Memory scope: 'global', 'bot', or 'chat'",
                        "enum": ["global", "bot", "chat"]
                    },
                    "chat_id": {
                        "type": "integer",
                        "description": "Chat ID (required for scope 'chat')"
                    }
                }),
                &["scope"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let scope = match input.get("scope").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error("Missing 'scope' parameter".into()),
        };

        let key = match scope {
            "global" => "groups/AGENTS.md".to_string(),
            "bot" => {
                let channel = memory_channel_from_auth(&input);
                let safe = channel.replace('/', "_");
                format!("groups/{safe}/AGENTS.md")
            }
            "chat" => {
                let chat_id = match chat_id_from_input_or_auth(&input) {
                    Some(id) => id,
                    None => return ToolResult::error("Missing 'chat_id' for chat scope".into()),
                };
                if let Err(e) = authorize_chat_access(&input, chat_id) {
                    return ToolResult::error(e);
                }
                let channel = match memory_channel_for_chat(&*self.db, &input, chat_id) {
                    Ok(v) => v,
                    Err(e) => return ToolResult::error(e),
                };
                let safe = channel.replace('/', "_");
                format!("groups/{safe}/{chat_id}/AGENTS.md")
            }
            _ => return ToolResult::error("scope must be 'global', 'bot', or 'chat'".into()),
        };

        info!("Reading memory: {}", key);

        match self.storage.get(&key).await {
            Ok(bytes) => {
                let content = String::from_utf8_lossy(&bytes).into_owned();
                if content.trim().is_empty() {
                    ToolResult::success("Memory file is empty.".into())
                } else {
                    ToolResult::success(content)
                }
            }
            Err(StorageError::NotFound(_)) => {
                ToolResult::success("No memory file found (not yet created).".into())
            }
            Err(e) => ToolResult::error(format!("Failed to read memory: {e}")),
        }
    }
}

pub struct WriteMemoryTool {
    storage: Arc<dyn ObjectStorage>,
    db: Arc<DynDataStore>,
    memory_backend: Arc<MemoryBackend>,
}

impl WriteMemoryTool {
    pub fn new(
        storage: Arc<dyn ObjectStorage>,
        db: Arc<DynDataStore>,
        memory_backend: Arc<MemoryBackend>,
    ) -> Self {
        WriteMemoryTool {
            storage,
            db,
            memory_backend,
        }
    }
}

#[async_trait]
impl Tool for WriteMemoryTool {
    fn name(&self) -> &str {
        "write_memory"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_memory".into(),
            description: "Write to the AGENTS.md memory file. Use this to remember important information about the user or conversation. Use scope 'global' for memories shared across all chats, 'bot' for the current bot/account, or 'chat' for chat-specific memories. In chat scope, memory is stored per person when sender identity is available.".into(),
            input_schema: schema_object(
                json!({
                    "scope": {
                        "type": "string",
                        "description": "Memory scope: 'global', 'bot', or 'chat'",
                        "enum": ["global", "bot", "chat"]
                    },
                    "chat_id": {
                        "type": "integer",
                        "description": "Chat ID (required for scope 'chat')"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the memory file (global/bot replace full file; chat updates the latest sender section when sender identity is available)"
                    }
                }),
                &["scope", "content"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let scope = match input.get("scope").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error("Missing 'scope' parameter".into()),
        };
        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing 'content' parameter".into()),
        };

        let (key, memory_chat_id) = match scope {
            "global" => {
                if let Some(auth) = auth_context_from_input(&input) {
                    if !auth.is_control_chat() {
                        return ToolResult::error(format!(
                            "Permission denied: chat {} cannot write global memory",
                            auth.caller_chat_id
                        ));
                    }
                }
                ("groups/AGENTS.md".to_string(), None)
            }
            "bot" => {
                let channel = memory_channel_from_auth(&input);
                let safe = channel.replace('/', "_");
                (format!("groups/{safe}/AGENTS.md"), None)
            }
            "chat" => {
                let chat_id = match chat_id_from_input_or_auth(&input) {
                    Some(id) => id,
                    None => return ToolResult::error("Missing 'chat_id' for chat scope".into()),
                };
                if let Err(e) = authorize_chat_access(&input, chat_id) {
                    return ToolResult::error(e);
                }
                let channel = match memory_channel_for_chat(&*self.db, &input, chat_id) {
                    Ok(v) => v,
                    Err(e) => return ToolResult::error(e),
                };
                let safe = channel.replace('/', "_");
                (format!("groups/{safe}/{chat_id}/AGENTS.md"), Some(chat_id))
            }
            _ => return ToolResult::error("scope must be 'global', 'bot', or 'chat'".into()),
        };

        info!("Writing memory: {}", key);

        let write_content = if scope == "chat" {
            let chat_id = memory_chat_id.unwrap_or_default();
            let sender = latest_sender_for_chat(&*self.db, chat_id);
            let existing = match self.storage.get(&key).await {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(StorageError::NotFound(_)) => String::new(),
                Err(e) => return ToolResult::error(format!("Failed to read existing memory: {e}")),
            };
            if let Some(sender) = sender {
                upsert_chat_person_memory(&existing, &sender, content)
            } else {
                content.to_string()
            }
        } else {
            content.to_string()
        };

        match self.storage.put(&key, write_content.as_bytes().to_vec()).await {
            Ok(()) => {
                let memory_content = content.trim().to_string();
                if !memory_content.is_empty() {
                    if let Some(normalized) =
                        memory_quality::normalize_memory_content(&memory_content, 180)
                    {
                        if memory_quality::memory_quality_ok(&normalized) {
                            let chat_id = memory_chat_id;
                            let _ = self
                                .memory_backend
                                .insert_memory_with_metadata(
                                    chat_id,
                                    &normalized,
                                    "KNOWLEDGE",
                                    "write_memory_tool",
                                    0.85,
                                )
                                .await;
                        }
                    }
                }

                ToolResult::success(format!("Memory saved to {} scope.", scope))
            }
            Err(e) => ToolResult::error(format!("Failed to write memory: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage::db::{Database, StoredMessage};
    use mchact_storage::prelude::*;
    use mchact_storage_backend::local::LocalStorage;
    use serde_json::json;

    fn test_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mchact_memtool_{}", uuid::Uuid::new_v4()))
    }

    fn test_db(dir: &std::path::Path) -> Arc<Database> {
        let runtime = dir.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        Arc::new(Database::new(runtime.to_str().unwrap()).unwrap())
    }

    async fn test_storage(dir: &std::path::Path) -> Arc<dyn ObjectStorage> {
        std::fs::create_dir_all(dir).unwrap();
        Arc::new(LocalStorage::new(dir.to_str().unwrap()).await.unwrap())
    }

    fn test_backend(db: Arc<DynDataStore>) -> Arc<MemoryBackend> {
        Arc::new(MemoryBackend::local_only(db))
    }

    fn store_user_message(db: &Database, chat_id: i64, sender_name: &str, content: &str) {
        let msg = StoredMessage {
            id: format!("{}-{}", sender_name, uuid::Uuid::new_v4()),
            chat_id,
            sender_name: sender_name.to_string(),
            content: content.to_string(),
            is_from_bot: false,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        db.store_message(&msg).unwrap();
    }

    #[tokio::test]
    async fn test_read_memory_global_not_exists() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let tool = ReadMemoryTool::new(storage, db);
        let result = tool.execute(json!({"scope": "global"})).await;
        assert!(!result.is_error);
        assert!(result.content.contains("No memory file found"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_and_read_memory_global() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let write_tool =
            WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let read_tool = ReadMemoryTool::new(storage, db.clone());

        let result = write_tool
            .execute(json!({"scope": "global", "content": "user prefers Rust"}))
            .await;
        assert!(!result.is_error);
        assert!(result.content.contains("Memory saved"));

        let result = read_tool.execute(json!({"scope": "global"})).await;
        assert!(!result.is_error);
        assert_eq!(result.content, "user prefers Rust");
        let mems = db.get_all_memories_for_chat(None).unwrap();
        assert_eq!(mems.len(), 1);
        assert_eq!(mems[0].content, "user prefers Rust");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_and_read_memory_chat() {
        let dir = test_dir();
        let db = test_db(&dir);
        db.resolve_or_create_chat_id("web", "42", Some("web-42"), "web")
            .unwrap();
        let storage = test_storage(&dir).await;
        let write_tool =
            WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let read_tool = ReadMemoryTool::new(storage, db.clone());

        let result = write_tool
            .execute(json!({"scope": "chat", "chat_id": 42, "content": "chat 42 notes"}))
            .await;
        assert!(!result.is_error);

        let result = read_tool
            .execute(json!({"scope": "chat", "chat_id": 42}))
            .await;
        assert!(!result.is_error);
        assert_eq!(result.content, "chat 42 notes");
        let mems = db.get_all_memories_for_chat(Some(42)).unwrap();
        assert_eq!(mems.len(), 1);
        assert_eq!(mems[0].content, "chat 42 notes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_read_memory_chat_missing_chat_id() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let tool = ReadMemoryTool::new(storage, db);
        let result = tool.execute(json!({"scope": "chat"})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing 'chat_id'"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_memory_chat_uses_auth_chat_id_when_missing() {
        let dir = test_dir();
        let db = test_db(&dir);
        db.resolve_or_create_chat_id("web", "42", Some("web-42"), "web")
            .unwrap();
        let storage = test_storage(&dir).await;
        let tool =
            WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let result = tool
            .execute(json!({
                "scope": "chat",
                "content": "from auth chat id",
                "__mchact_auth": {
                    "caller_channel": "web",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        // Verify via storage read
        let read_tool = ReadMemoryTool::new(storage, db.clone());
        let read = read_tool
            .execute(json!({
                "scope": "chat",
                "chat_id": 42,
            }))
            .await;
        assert_eq!(read.content, "from auth chat id");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_memory_chat_upserts_per_sender() {
        let dir = test_dir();
        let db = test_db(&dir);
        db.resolve_or_create_chat_id("web", "42", Some("web-42"), "web")
            .unwrap();
        let storage = test_storage(&dir).await;
        let tool =
            WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));

        store_user_message(&db, 42, "alice", "remember profile");
        let r1 = tool
            .execute(json!({"scope": "chat", "chat_id": 42, "content": "昵称: 老板"}))
            .await;
        assert!(!r1.is_error, "{}", r1.content);

        store_user_message(&db, 42, "bob", "remember profile");
        let r2 = tool
            .execute(json!({"scope": "chat", "chat_id": 42, "content": "昵称: Bob哥"}))
            .await;
        assert!(!r2.is_error, "{}", r2.content);

        store_user_message(&db, 42, "alice", "update profile");
        let r3 = tool
            .execute(json!({"scope": "chat", "chat_id": 42, "content": "昵称: 大老板"}))
            .await;
        assert!(!r3.is_error, "{}", r3.content);

        // Read back via storage to verify upsert
        let read_tool = ReadMemoryTool::new(storage, db.clone());
        let read = read_tool
            .execute(json!({"scope": "chat", "chat_id": 42}))
            .await;
        assert!(read.content.contains("## Person: alice"));
        assert!(read.content.contains("## Person: bob"));
        assert!(read.content.contains("昵称: 大老板"));
        assert!(read.content.contains("昵称: Bob哥"));
        assert!(!read.content.contains("昵称: 老板"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_memory_missing_scope() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let tool = WriteMemoryTool::new(storage, db.clone(), test_backend(db));
        let result = tool.execute(json!({"content": "data"})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing 'scope'"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_read_memory_invalid_scope() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let tool = ReadMemoryTool::new(storage, db);
        let result = tool.execute(json!({"scope": "invalid"})).await;
        assert!(result.is_error);
        assert!(result
            .content
            .contains("must be 'global', 'bot', or 'chat'"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_and_read_memory_bot_scope() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let write_tool =
            WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let read_tool = ReadMemoryTool::new(storage, db);

        let write = write_tool
            .execute(json!({
                "scope": "bot",
                "content": "bot identity",
                "__mchact_auth": {
                    "caller_channel": "feishu.ops",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!write.is_error, "{}", write.content);

        let read = read_tool
            .execute(json!({
                "scope": "bot",
                "__mchact_auth": {
                    "caller_channel": "feishu.ops",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!read.is_error, "{}", read.content);
        assert_eq!(read.content, "bot identity");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_read_memory_empty_file() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let write_tool =
            WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let read_tool = ReadMemoryTool::new(storage, db.clone());

        write_tool
            .execute(json!({"scope": "global", "content": "   "}))
            .await;

        let result = read_tool.execute(json!({"scope": "global"})).await;
        assert!(!result.is_error);
        assert!(result.content.contains("empty"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_memory_global_denied_for_non_control_chat() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let tool = WriteMemoryTool::new(storage, db.clone(), test_backend(db));
        let result = tool
            .execute(json!({
                "scope": "global",
                "content": "secret",
                "__mchact_auth": {
                    "caller_chat_id": 100,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Permission denied"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_write_memory_global_allowed_for_control_chat() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let write_tool = WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let read_tool = ReadMemoryTool::new(storage, db.clone());
        let result = write_tool
            .execute(json!({
                "scope": "global",
                "content": "global ok",
                "__mchact_auth": {
                    "caller_chat_id": 100,
                    "control_chat_ids": [100]
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        let read = read_tool.execute(json!({"scope": "global"})).await;
        assert_eq!(read.content, "global ok");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_read_memory_chat_permission_denied() {
        let dir = test_dir();
        let db = test_db(&dir);
        let storage = test_storage(&dir).await;
        let tool = ReadMemoryTool::new(storage, db);
        let result = tool
            .execute(json!({
                "scope": "chat",
                "chat_id": 200,
                "__mchact_auth": {
                    "caller_chat_id": 100,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Permission denied"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_read_memory_chat_allowed_for_control_chat_cross_chat() {
        let dir = test_dir();
        let db = test_db(&dir);
        db.resolve_or_create_chat_id("web", "200", Some("web-200"), "web")
            .unwrap();
        let storage = test_storage(&dir).await;
        let write_tool = WriteMemoryTool::new(storage.clone(), db.clone(), test_backend(db.clone()));
        let read_tool = ReadMemoryTool::new(storage, db.clone());
        write_tool
            .execute(json!({"scope": "chat", "chat_id": 200, "content": "chat200"}))
            .await;
        let result = read_tool
            .execute(json!({
                "scope": "chat",
                "chat_id": 200,
                "__mchact_auth": {
                    "caller_chat_id": 100,
                    "control_chat_ids": [100]
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "chat200");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

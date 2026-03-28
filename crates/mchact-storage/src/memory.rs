use mchact_storage_backend::{ObjectStorage, StorageError};
use std::sync::Arc;

pub struct MemoryManager {
    storage: Arc<dyn ObjectStorage>,
    prefix: String,
}

impl MemoryManager {
    /// Create a MemoryManager backed by the given ObjectStorage.
    ///
    /// `prefix` is prepended to every key, e.g. `"groups"`.
    pub fn new(storage: Arc<dyn ObjectStorage>, prefix: &str) -> Self {
        MemoryManager {
            storage,
            prefix: prefix.trim_end_matches('/').to_string(),
        }
    }

    fn global_key(&self) -> String {
        format!("{}/AGENTS.md", self.prefix)
    }

    fn chat_key(&self, channel: &str, chat_id: i64) -> String {
        let safe_channel = channel.trim().replace('/', "_");
        format!("{}/{}/{}/AGENTS.md", self.prefix, safe_channel, chat_id)
    }

    fn bot_key(&self, channel: &str) -> String {
        let safe_channel = channel.trim().replace('/', "_");
        format!("{}/{}/AGENTS.md", self.prefix, safe_channel)
    }

    pub async fn read_global_memory(&self) -> Option<String> {
        read_string(&self.storage, &self.global_key()).await
    }

    pub async fn read_chat_memory(&self, channel: &str, chat_id: i64) -> Option<String> {
        read_string(&self.storage, &self.chat_key(channel, chat_id)).await
    }

    pub async fn read_bot_memory(&self, channel: &str) -> Option<String> {
        read_string(&self.storage, &self.bot_key(channel)).await
    }

    #[allow(dead_code)]
    pub async fn write_global_memory(&self, content: &str) -> Result<(), StorageError> {
        self.storage
            .put(&self.global_key(), content.as_bytes().to_vec())
            .await
    }

    #[allow(dead_code)]
    pub async fn write_chat_memory(
        &self,
        channel: &str,
        chat_id: i64,
        content: &str,
    ) -> Result<(), StorageError> {
        self.storage
            .put(&self.chat_key(channel, chat_id), content.as_bytes().to_vec())
            .await
    }

    #[allow(dead_code)]
    pub async fn write_bot_memory(
        &self,
        channel: &str,
        content: &str,
    ) -> Result<(), StorageError> {
        self.storage
            .put(&self.bot_key(channel), content.as_bytes().to_vec())
            .await
    }

    pub async fn build_memory_context(&self, channel: &str, chat_id: i64) -> String {
        let mut context = String::new();

        if let Some(global) = self.read_global_memory().await {
            if !global.trim().is_empty() {
                context.push_str("<global_memory>\n");
                context.push_str(&global);
                context.push_str("\n</global_memory>\n\n");
            }
        }

        if let Some(bot) = self.read_bot_memory(channel).await {
            if !bot.trim().is_empty() {
                context.push_str("<bot_memory>\n");
                context.push_str(&bot);
                context.push_str("\n</bot_memory>\n\n");
            }
        }

        if let Some(chat) = self.read_chat_memory(channel, chat_id).await {
            if !chat.trim().is_empty() {
                context.push_str("<chat_memory>\n");
                context.push_str(&chat);
                context.push_str("\n</chat_memory>\n\n");
            }
        }

        context
    }
}

async fn read_string(storage: &Arc<dyn ObjectStorage>, key: &str) -> Option<String> {
    match storage.get(key).await {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(StorageError::NotFound(_)) => None,
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage_backend::local::LocalStorage;

    async fn test_memory_manager() -> (MemoryManager, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mchact_mem_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Arc::new(LocalStorage::new(dir.to_str().unwrap()).await.unwrap());
        let mm = MemoryManager::new(storage, "groups");
        (mm, dir)
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_read_nonexistent_memory() {
        let (mm, dir) = test_memory_manager().await;
        assert!(mm.read_global_memory().await.is_none());
        assert!(mm.read_chat_memory("telegram", 100).await.is_none());
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_write_and_read_global_memory() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_global_memory("global notes").await.unwrap();
        let content = mm.read_global_memory().await.unwrap();
        assert_eq!(content, "global notes");
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_write_and_read_chat_memory() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_chat_memory("telegram", 42, "chat 42 notes")
            .await
            .unwrap();
        let content = mm.read_chat_memory("telegram", 42).await.unwrap();
        assert_eq!(content, "chat 42 notes");

        // Different chat should be empty
        assert!(mm.read_chat_memory("telegram", 99).await.is_none());
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_write_and_read_bot_memory() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_bot_memory("feishu", "bot notes").await.unwrap();
        let content = mm.read_bot_memory("feishu").await.unwrap();
        assert_eq!(content, "bot notes");
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_build_memory_context_empty() {
        let (mm, dir) = test_memory_manager().await;
        let ctx = mm.build_memory_context("telegram", 100).await;
        assert!(ctx.is_empty());
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_build_memory_context_with_global_only() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_global_memory("I am global memory").await.unwrap();
        let ctx = mm.build_memory_context("telegram", 100).await;
        assert!(ctx.contains("<global_memory>"));
        assert!(ctx.contains("I am global memory"));
        assert!(ctx.contains("</global_memory>"));
        assert!(!ctx.contains("<chat_memory>"));
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_build_memory_context_with_both() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_global_memory("global stuff").await.unwrap();
        mm.write_bot_memory("telegram", "bot stuff").await.unwrap();
        mm.write_chat_memory("telegram", 100, "chat stuff")
            .await
            .unwrap();
        let ctx = mm.build_memory_context("telegram", 100).await;
        assert!(ctx.contains("<global_memory>"));
        assert!(ctx.contains("global stuff"));
        assert!(ctx.contains("<bot_memory>"));
        assert!(ctx.contains("bot stuff"));
        assert!(ctx.contains("<chat_memory>"));
        assert!(ctx.contains("chat stuff"));
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_build_memory_context_ignores_whitespace_only() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_global_memory("   \n  ").await.unwrap();
        let ctx = mm.build_memory_context("telegram", 100).await;
        // Whitespace-only content should be ignored
        assert!(ctx.is_empty());
        cleanup(&dir);
    }

    #[tokio::test]
    async fn test_channel_slash_sanitized() {
        let (mm, dir) = test_memory_manager().await;
        mm.write_bot_memory("feishu.ops", "bot notes").await.unwrap();
        let content = mm.read_bot_memory("feishu.ops").await.unwrap();
        assert_eq!(content, "bot notes");
        cleanup(&dir);
    }
}

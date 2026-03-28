use mchact_core::error::MchactError;

use crate::db::types::{FtsSearchResult, StoredMessage};
use crate::traits::MessageStore;

use super::PgDriver;

impl MessageStore for PgDriver {
    fn store_message(&self, msg: &StoredMessage) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let id = msg.id.clone();
        let chat_id = msg.chat_id;
        let sender_name = msg.sender_name.clone();
        let content = msg.content.clone();
        let is_from_bot = msg.is_from_bot;
        let timestamp = msg.timestamp.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO messages (id, chat_id, sender_name, content, is_from_bot, timestamp)
                         VALUES ($1, $2, $3, $4, $5, $6)
                         ON CONFLICT(id, chat_id) DO UPDATE SET
                             sender_name = $3,
                             content = $4,
                             is_from_bot = $5,
                             timestamp = $6",
                        &[&id, &chat_id, &sender_name, &content, &is_from_bot, &timestamp],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("store_message: {e}")))?;
                Ok(())
            })
        })
    }

    fn store_message_if_new(&self, msg: &StoredMessage) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let id = msg.id.clone();
        let chat_id = msg.chat_id;
        let sender_name = msg.sender_name.clone();
        let content = msg.content.clone();
        let is_from_bot = msg.is_from_bot;
        let timestamp = msg.timestamp.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let affected = client
                    .execute(
                        "INSERT INTO messages (id, chat_id, sender_name, content, is_from_bot, timestamp)
                         VALUES ($1, $2, $3, $4, $5, $6)
                         ON CONFLICT DO NOTHING",
                        &[&id, &chat_id, &sender_name, &content, &is_from_bot, &timestamp],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("store_message_if_new: {e}")))?;
                Ok(affected > 0)
            })
        })
    }

    fn message_exists(&self, chat_id: i64, message_id: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let message_id = message_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT 1 FROM messages WHERE chat_id = $1 AND id = $2 LIMIT 1",
                        &[&chat_id, &message_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("message_exists: {e}")))?;
                Ok(row.is_some())
            })
        })
    }

    fn search_messages_fts(
        &self,
        query: &str,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, MchactError> {
        use crate::fts::sanitize_fts_query;
        let query = match sanitize_fts_query(query) {
            Some(q) => q,
            None => return Ok(vec![]),
        };

        let pool = self.pool.clone();
        let limit = limit as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT m.id, m.chat_id, c.chat_title, m.sender_name,
                                m.content AS snippet,
                                m.timestamp,
                                ts_rank(m.search_vector, plainto_tsquery('english', $1)) AS rank
                         FROM messages m
                         LEFT JOIN chats c ON c.chat_id = m.chat_id
                         WHERE m.search_vector @@ plainto_tsquery('english', $1)
                           AND ($2::bigint IS NULL OR m.chat_id = $2)
                         ORDER BY rank DESC
                         LIMIT $3",
                        &[&query, &chat_id, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("search_messages_fts: {e}")))?;
                let results = rows
                    .iter()
                    .map(|row| FtsSearchResult {
                        message_id: row.get("id"),
                        chat_id: row.get("chat_id"),
                        chat_title: row.get("chat_title"),
                        sender_name: row.get("sender_name"),
                        content_snippet: row.get("snippet"),
                        timestamp: row.get("timestamp"),
                        rank: row.get::<_, f32>("rank") as f64,
                    })
                    .collect();
                Ok(results)
            })
        })
    }

    fn get_message_context(
        &self,
        chat_id: i64,
        timestamp: &str,
        window: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let pool = self.pool.clone();
        let timestamp = timestamp.to_string();
        let window = window as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp FROM (
                             SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                             FROM messages WHERE chat_id = $1 AND timestamp < $2
                             ORDER BY timestamp DESC LIMIT $3
                         ) before_rows
                         UNION ALL
                         SELECT id, chat_id, sender_name, content, is_from_bot, timestamp FROM (
                             SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                             FROM messages WHERE chat_id = $1 AND timestamp >= $2
                             ORDER BY timestamp ASC LIMIT $3
                         ) after_rows
                         ORDER BY timestamp ASC",
                        &[&chat_id, &timestamp, &window],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_message_context: {e}")))?;
                let results = rows
                    .iter()
                    .map(|row| StoredMessage {
                        id: row.get("id"),
                        chat_id: row.get("chat_id"),
                        sender_name: row.get("sender_name"),
                        content: row.get("content"),
                        is_from_bot: row.get("is_from_bot"),
                        timestamp: row.get("timestamp"),
                    })
                    .collect();
                Ok(results)
            })
        })
    }

    fn rebuild_fts_index(&self) -> Result<(), MchactError> {
        // No-op for PostgreSQL — tsvector indexes are maintained automatically
        Ok(())
    }

    fn get_recent_messages(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let pool = self.pool.clone();
        let limit = limit as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                         FROM messages
                         WHERE chat_id = $1
                         ORDER BY timestamp DESC
                         LIMIT $2",
                        &[&chat_id, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_recent_messages: {e}")))?;
                let mut messages: Vec<StoredMessage> = rows
                    .iter()
                    .map(|row| StoredMessage {
                        id: row.get("id"),
                        chat_id: row.get("chat_id"),
                        sender_name: row.get("sender_name"),
                        content: row.get("content"),
                        is_from_bot: row.get("is_from_bot"),
                        timestamp: row.get("timestamp"),
                    })
                    .collect();
                // Reverse so oldest first
                messages.reverse();
                Ok(messages)
            })
        })
    }

    fn get_all_messages(&self, chat_id: i64) -> Result<Vec<StoredMessage>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                         FROM messages
                         WHERE chat_id = $1
                         ORDER BY timestamp ASC",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_all_messages: {e}")))?;
                let messages = rows
                    .iter()
                    .map(|row| StoredMessage {
                        id: row.get("id"),
                        chat_id: row.get("chat_id"),
                        sender_name: row.get("sender_name"),
                        content: row.get("content"),
                        is_from_bot: row.get("is_from_bot"),
                        timestamp: row.get("timestamp"),
                    })
                    .collect();
                Ok(messages)
            })
        })
    }

    fn get_messages_since_last_bot_response(
        &self,
        chat_id: i64,
        max: usize,
        fallback: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let pool = self.pool.clone();
        let max = max as i64;
        let fallback = fallback as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;

                // Find timestamp of last bot message
                let last_bot_ts: Option<String> = client
                    .query_opt(
                        "SELECT timestamp FROM messages
                         WHERE chat_id = $1 AND is_from_bot = true
                         ORDER BY timestamp DESC LIMIT 1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_messages_since_last_bot_response ts: {e}")))?
                    .map(|r| r.get("timestamp"));

                let mut messages: Vec<StoredMessage> = if let Some(ts) = last_bot_ts {
                    let rows = client
                        .query(
                            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                             FROM messages
                             WHERE chat_id = $1 AND timestamp >= $2
                             ORDER BY timestamp DESC
                             LIMIT $3",
                            &[&chat_id, &ts, &max],
                        )
                        .await
                        .map_err(|e| MchactError::ToolExecution(format!("get_messages_since_last_bot_response rows: {e}")))?;
                    rows.iter()
                        .map(|row| StoredMessage {
                            id: row.get("id"),
                            chat_id: row.get("chat_id"),
                            sender_name: row.get("sender_name"),
                            content: row.get("content"),
                            is_from_bot: row.get("is_from_bot"),
                            timestamp: row.get("timestamp"),
                        })
                        .collect()
                } else {
                    let rows = client
                        .query(
                            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                             FROM messages
                             WHERE chat_id = $1
                             ORDER BY timestamp DESC
                             LIMIT $2",
                            &[&chat_id, &fallback],
                        )
                        .await
                        .map_err(|e| MchactError::ToolExecution(format!("get_messages_since_last_bot_response fallback: {e}")))?;
                    rows.iter()
                        .map(|row| StoredMessage {
                            id: row.get("id"),
                            chat_id: row.get("chat_id"),
                            sender_name: row.get("sender_name"),
                            content: row.get("content"),
                            is_from_bot: row.get("is_from_bot"),
                            timestamp: row.get("timestamp"),
                        })
                        .collect()
                };

                messages.reverse();
                Ok(messages)
            })
        })
    }

    fn get_new_user_messages_since(
        &self,
        chat_id: i64,
        since: &str,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let pool = self.pool.clone();
        let since = since.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                         FROM messages
                         WHERE chat_id = $1 AND timestamp > $2 AND is_from_bot = false
                         ORDER BY timestamp ASC",
                        &[&chat_id, &since],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_new_user_messages_since: {e}")))?;
                let messages = rows
                    .iter()
                    .map(|row| StoredMessage {
                        id: row.get("id"),
                        chat_id: row.get("chat_id"),
                        sender_name: row.get("sender_name"),
                        content: row.get("content"),
                        is_from_bot: row.get("is_from_bot"),
                        timestamp: row.get("timestamp"),
                    })
                    .collect();
                Ok(messages)
            })
        })
    }

    fn get_messages_since(
        &self,
        chat_id: i64,
        since: &str,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let pool = self.pool.clone();
        let since = since.to_string();
        let limit = limit as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                         FROM messages
                         WHERE chat_id = $1 AND timestamp > $2
                         ORDER BY timestamp ASC
                         LIMIT $3",
                        &[&chat_id, &since, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_messages_since: {e}")))?;
                let messages = rows
                    .iter()
                    .map(|row| StoredMessage {
                        id: row.get("id"),
                        chat_id: row.get("chat_id"),
                        sender_name: row.get("sender_name"),
                        content: row.get("content"),
                        is_from_bot: row.get("is_from_bot"),
                        timestamp: row.get("timestamp"),
                    })
                    .collect();
                Ok(messages)
            })
        })
    }
}

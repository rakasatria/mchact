use mchact_core::error::MchactError;

use crate::db::types::ChatSummary;
use crate::traits::ChatStore;

use super::PgDriver;

fn infer_channel_from_chat_type(chat_type: &str) -> &'static str {
    if chat_type.starts_with("telegram_")
        || matches!(chat_type, "private" | "group" | "supergroup" | "channel")
    {
        "telegram"
    } else if chat_type == "discord" {
        "discord"
    } else if chat_type == "web" {
        "web"
    } else {
        "unknown"
    }
}

impl ChatStore for PgDriver {
    fn upsert_chat(
        &self,
        chat_id: i64,
        chat_title: Option<&str>,
        chat_type: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let channel = infer_channel_from_chat_type(chat_type).to_string();
        let chat_type = chat_type.to_string();
        let chat_title = chat_title.map(|s| s.to_string());
        let external_chat_id = chat_id.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO chats (chat_id, chat_title, chat_type, last_message_time, channel, external_chat_id)
                         VALUES ($1, $2, $3, $4, $5, $6)
                         ON CONFLICT(chat_id) DO UPDATE SET
                             chat_title = COALESCE($2, chats.chat_title),
                             chat_type = $3,
                             last_message_time = $4,
                             channel = COALESCE(chats.channel, $5),
                             external_chat_id = COALESCE(chats.external_chat_id, $6)",
                        &[
                            &chat_id,
                            &chat_title,
                            &chat_type,
                            &now,
                            &channel,
                            &external_chat_id,
                        ],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("upsert_chat: {e}")))?;
                Ok(())
            })
        })
    }

    fn resolve_or_create_chat_id(
        &self,
        channel: &str,
        external_chat_id: &str,
        chat_title: Option<&str>,
        chat_type: &str,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let channel = channel.to_string();
        let external_chat_id = external_chat_id.to_string();
        let chat_title = chat_title.map(|s| s.to_string());
        let chat_type = chat_type.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;

                // Check if already exists
                if let Some(row) = client
                    .query_opt(
                        "SELECT chat_id FROM chats WHERE channel = $1 AND external_chat_id = $2 LIMIT 1",
                        &[&channel, &external_chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("resolve_or_create_chat_id select: {e}")))?
                {
                    let chat_id: i64 = row.get("chat_id");
                    client
                        .execute(
                            "UPDATE chats
                             SET chat_title = COALESCE($2, chat_title),
                                 chat_type = $3,
                                 last_message_time = $4
                             WHERE chat_id = $1",
                            &[&chat_id, &chat_title, &chat_type, &now],
                        )
                        .await
                        .map_err(|e| MchactError::ToolExecution(format!("resolve_or_create_chat_id update: {e}")))?;
                    return Ok(chat_id);
                }

                // Try to use numeric external_chat_id as the PK
                let preferred_chat_id = external_chat_id.parse::<i64>().ok();
                if let Some(cid) = preferred_chat_id {
                    let occupied = client
                        .query_opt(
                            "SELECT 1 FROM chats WHERE chat_id = $1 LIMIT 1",
                            &[&cid],
                        )
                        .await
                        .map_err(|e| MchactError::ToolExecution(format!("resolve_or_create_chat_id check: {e}")))?
                        .is_some();
                    if !occupied {
                        client
                            .execute(
                                "INSERT INTO chats(chat_id, chat_title, chat_type, last_message_time, channel, external_chat_id)
                                 VALUES($1, $2, $3, $4, $5, $6)",
                                &[&cid, &chat_title, &chat_type, &now, &channel, &external_chat_id],
                            )
                            .await
                            .map_err(|e| MchactError::ToolExecution(format!("resolve_or_create_chat_id insert preferred: {e}")))?;
                        return Ok(cid);
                    }
                }

                // Auto-generated id via RETURNING
                let row = client
                    .query_one(
                        "INSERT INTO chats(chat_title, chat_type, last_message_time, channel, external_chat_id)
                         VALUES($1, $2, $3, $4, $5)
                         RETURNING chat_id",
                        &[&chat_title, &chat_type, &now, &channel, &external_chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("resolve_or_create_chat_id insert auto: {e}")))?;
                let chat_id: i64 = row.get(0);
                Ok(chat_id)
            })
        })
    }

    fn get_chat_type(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT chat_type FROM chats WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_chat_type: {e}")))?;
                Ok(row.map(|r| r.get::<_, String>("chat_type")))
            })
        })
    }

    fn get_chat_id_by_channel_and_title(
        &self,
        channel: &str,
        chat_title: &str,
    ) -> Result<Option<i64>, MchactError> {
        let pool = self.pool.clone();
        let channel = channel.to_string();
        let chat_title = chat_title.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT chat_id
                         FROM chats
                         WHERE channel = $1 AND chat_title = $2
                         ORDER BY last_message_time DESC
                         LIMIT 1",
                        &[&channel, &chat_title],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_chat_id_by_channel_and_title: {e}")))?;
                Ok(row.map(|r| r.get::<_, i64>("chat_id")))
            })
        })
    }

    fn get_chat_channel(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT channel FROM chats WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_chat_channel: {e}")))?;
                Ok(row.and_then(|r| r.get::<_, Option<String>>("channel")))
            })
        })
    }

    fn get_chat_external_id(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT external_chat_id FROM chats WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_chat_external_id: {e}")))?;
                Ok(row.and_then(|r| r.get::<_, Option<String>>("external_chat_id")))
            })
        })
    }

    fn get_recent_chats(&self, limit: usize) -> Result<Vec<ChatSummary>, MchactError> {
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
                        "SELECT
                             c.chat_id,
                             c.chat_title,
                             s.label,
                             c.chat_type,
                             c.last_message_time,
                             (
                                 SELECT m.content
                                 FROM messages m
                                 WHERE m.chat_id = c.chat_id
                                 ORDER BY m.timestamp DESC
                                 LIMIT 1
                             ) AS last_message_preview
                         FROM chats c
                         LEFT JOIN sessions s ON s.chat_id = c.chat_id
                         ORDER BY c.last_message_time DESC
                         LIMIT $1",
                        &[&limit],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_recent_chats: {e}")))?;
                let chats = rows
                    .iter()
                    .map(|row| ChatSummary {
                        chat_id: row.get("chat_id"),
                        chat_title: row.get("chat_title"),
                        session_label: row.get("label"),
                        chat_type: row.get("chat_type"),
                        last_message_time: row.get("last_message_time"),
                        last_message_preview: row.get("last_message_preview"),
                    })
                    .collect();
                Ok(chats)
            })
        })
    }

    fn get_chats_by_type(
        &self,
        chat_type: &str,
        limit: usize,
    ) -> Result<Vec<ChatSummary>, MchactError> {
        let pool = self.pool.clone();
        let chat_type = chat_type.to_string();
        let limit = limit as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT
                             c.chat_id,
                             c.chat_title,
                             s.label,
                             c.chat_type,
                             c.last_message_time,
                             (
                                 SELECT m.content
                                 FROM messages m
                                 WHERE m.chat_id = c.chat_id
                                 ORDER BY m.timestamp DESC
                                 LIMIT 1
                             ) AS last_message_preview
                         FROM chats c
                         LEFT JOIN sessions s ON s.chat_id = c.chat_id
                         WHERE c.chat_type = $1
                         ORDER BY c.last_message_time DESC
                         LIMIT $2",
                        &[&chat_type, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("get_chats_by_type: {e}")))?;
                let chats = rows
                    .iter()
                    .map(|row| ChatSummary {
                        chat_id: row.get("chat_id"),
                        chat_title: row.get("chat_title"),
                        session_label: row.get("label"),
                        chat_type: row.get("chat_type"),
                        last_message_time: row.get("last_message_time"),
                        last_message_preview: row.get("last_message_preview"),
                    })
                    .collect();
                Ok(chats)
            })
        })
    }
}

use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::{FtsSearchResult, StoredMessage};
use crate::traits::MessageStore;

impl MessageStore for Database {
    fn store_message(&self, msg: &StoredMessage) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT OR REPLACE INTO messages (id, chat_id, sender_name, content, is_from_bot, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                msg.id,
                msg.chat_id,
                msg.sender_name,
                msg.content,
                msg.is_from_bot as i32,
                msg.timestamp,
            ],
        )?;
        Ok(())
    }

    fn store_message_if_new(&self, msg: &StoredMessage) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let affected = conn.execute(
            "INSERT OR IGNORE INTO messages (id, chat_id, sender_name, content, is_from_bot, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                msg.id,
                msg.chat_id,
                msg.sender_name,
                msg.content,
                msg.is_from_bot as i32,
                msg.timestamp,
            ],
        )?;
        Ok(affected > 0)
    }

    fn message_exists(&self, chat_id: i64, message_id: &str) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let exists = conn
            .query_row(
                "SELECT 1 FROM messages WHERE chat_id = ?1 AND id = ?2 LIMIT 1",
                params![chat_id, message_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(exists)
    }

    fn search_messages_fts(
        &self,
        query: &str,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, MchactError> {
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

    fn get_message_context(
        &self,
        chat_id: i64,
        timestamp: &str,
        window: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
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

    fn rebuild_fts_index(&self) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES('rebuild')")?;
        Ok(())
    }

    fn get_recent_messages(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
             FROM messages
             WHERE chat_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )?;

        let messages = stmt
            .query_map(params![chat_id, limit as i64], |row| {
                Ok(StoredMessage {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    sender_name: row.get(2)?,
                    content: row.get(3)?,
                    is_from_bot: row.get::<_, i32>(4)? != 0,
                    timestamp: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Reverse so oldest first
        let mut messages = messages;
        messages.reverse();
        Ok(messages)
    }

    fn get_all_messages(&self, chat_id: i64) -> Result<Vec<StoredMessage>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
             FROM messages
             WHERE chat_id = ?1
             ORDER BY timestamp ASC",
        )?;
        let messages = stmt
            .query_map(params![chat_id], |row| {
                Ok(StoredMessage {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    sender_name: row.get(2)?,
                    content: row.get(3)?,
                    is_from_bot: row.get::<_, i32>(4)? != 0,
                    timestamp: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    /// Get messages since the bot's last response in this chat.
    /// Falls back to `fallback_limit` most recent messages if bot never responded.
    fn get_messages_since_last_bot_response(
        &self,
        chat_id: i64,
        max: usize,
        fallback: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let conn = self.lock_conn();

        // Find timestamp of last bot message
        let last_bot_ts: Option<String> = conn
            .query_row(
                "SELECT timestamp FROM messages
                 WHERE chat_id = ?1 AND is_from_bot = 1
                 ORDER BY timestamp DESC LIMIT 1",
                params![chat_id],
                |row| row.get(0),
            )
            .ok();

        let mut messages = if let Some(ts) = last_bot_ts {
            let mut stmt = conn.prepare(
                "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                 FROM messages
                 WHERE chat_id = ?1 AND timestamp >= ?2
                 ORDER BY timestamp DESC
                 LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(params![chat_id, ts, max as i64], |row| {
                    Ok(StoredMessage {
                        id: row.get(0)?,
                        chat_id: row.get(1)?,
                        sender_name: row.get(2)?,
                        content: row.get(3)?,
                        is_from_bot: row.get::<_, i32>(4)? != 0,
                        timestamp: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
                 FROM messages
                 WHERE chat_id = ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![chat_id, fallback as i64], |row| {
                    Ok(StoredMessage {
                        id: row.get(0)?,
                        chat_id: row.get(1)?,
                        sender_name: row.get(2)?,
                        content: row.get(3)?,
                        is_from_bot: row.get::<_, i32>(4)? != 0,
                        timestamp: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };

        messages.reverse();
        Ok(messages)
    }

    fn get_new_user_messages_since(
        &self,
        chat_id: i64,
        since: &str,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
             FROM messages
             WHERE chat_id = ?1 AND timestamp > ?2 AND is_from_bot = 0
             ORDER BY timestamp ASC",
        )?;
        let messages = stmt
            .query_map(params![chat_id, since], |row| {
                Ok(StoredMessage {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    sender_name: row.get(2)?,
                    content: row.get(3)?,
                    is_from_bot: row.get::<_, i32>(4)? != 0,
                    timestamp: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    fn get_messages_since(
        &self,
        chat_id: i64,
        since: &str,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, sender_name, content, is_from_bot, timestamp
             FROM messages
             WHERE chat_id = ?1 AND timestamp > ?2
             ORDER BY timestamp ASC
             LIMIT ?3",
        )?;
        let messages = stmt
            .query_map(params![chat_id, since, limit as i64], |row| {
                Ok(StoredMessage {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    sender_name: row.get(2)?,
                    content: row.get(3)?,
                    is_from_bot: row.get::<_, i32>(4)? != 0,
                    timestamp: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }
}


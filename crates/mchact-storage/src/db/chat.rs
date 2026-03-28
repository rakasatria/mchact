use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::{ChatSummary};
use crate::traits::ChatStore;

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

impl ChatStore for Database {
    fn upsert_chat(
        &self,
        chat_id: i64,
        chat_title: Option<&str>,
        chat_type: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO chats (chat_id, chat_title, chat_type, last_message_time, channel, external_chat_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chat_id) DO UPDATE SET
                chat_title = COALESCE(?2, chat_title),
                chat_type = ?3,
                last_message_time = ?4,
                channel = COALESCE(channel, ?5),
                external_chat_id = COALESCE(external_chat_id, ?6)",
            params![
                chat_id,
                chat_title,
                chat_type,
                now,
                infer_channel_from_chat_type(chat_type),
                chat_id.to_string()
            ],
        )?;
        Ok(())
    }

    fn resolve_or_create_chat_id(
        &self,
        channel: &str,
        external_chat_id: &str,
        chat_title: Option<&str>,
        chat_type: &str,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(chat_id) = conn
            .query_row(
                "SELECT chat_id FROM chats WHERE channel = ?1 AND external_chat_id = ?2 LIMIT 1",
                params![channel, external_chat_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            conn.execute(
                "UPDATE chats
                 SET chat_title = COALESCE(?2, chat_title),
                     chat_type = ?3,
                     last_message_time = ?4
                 WHERE chat_id = ?1",
                params![chat_id, chat_title, chat_type, now],
            )?;
            return Ok(chat_id);
        }

        let preferred_chat_id = external_chat_id.parse::<i64>().ok();
        if let Some(cid) = preferred_chat_id {
            let occupied = conn
                .query_row(
                    "SELECT 1 FROM chats WHERE chat_id = ?1 LIMIT 1",
                    params![cid],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !occupied {
                conn.execute(
                    "INSERT INTO chats(chat_id, chat_title, chat_type, last_message_time, channel, external_chat_id)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                    params![cid, chat_title, chat_type, now, channel, external_chat_id],
                )?;
                return Ok(cid);
            }
        }

        conn.execute(
            "INSERT INTO chats(chat_title, chat_type, last_message_time, channel, external_chat_id)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![chat_title, chat_type, now, channel, external_chat_id],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_chat_type(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT chat_type FROM chats WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_chat_id_by_channel_and_title(
        &self,
        channel: &str,
        chat_title: &str,
    ) -> Result<Option<i64>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT chat_id
             FROM chats
             WHERE channel = ?1 AND chat_title = ?2
             ORDER BY last_message_time DESC
             LIMIT 1",
            params![channel, chat_title],
            |row| row.get::<_, i64>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_chat_channel(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT channel FROM chats WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get::<_, Option<String>>(0),
        );
        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_chat_external_id(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT external_chat_id FROM chats WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get::<_, Option<String>>(0),
        );
        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_recent_chats(&self, limit: usize) -> Result<Vec<ChatSummary>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
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
             LIMIT ?1",
        )?;
        let chats = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ChatSummary {
                    chat_id: row.get(0)?,
                    chat_title: row.get(1)?,
                    session_label: row.get(2)?,
                    chat_type: row.get(3)?,
                    last_message_time: row.get(4)?,
                    last_message_preview: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chats)
    }

    fn get_chats_by_type(
        &self,
        chat_type: &str,
        limit: usize,
    ) -> Result<Vec<ChatSummary>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
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
             WHERE c.chat_type = ?1
             ORDER BY c.last_message_time DESC
             LIMIT ?2",
        )?;
        let chats = stmt
            .query_map(params![chat_type, limit as i64], |row| {
                Ok(ChatSummary {
                    chat_id: row.get(0)?,
                    chat_title: row.get(1)?,
                    session_label: row.get(2)?,
                    chat_type: row.get(3)?,
                    last_message_time: row.get(4)?,
                    last_message_preview: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chats)
    }
}

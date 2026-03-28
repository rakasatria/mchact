use mchact_core::error::MchactError;
use rusqlite::params;

use super::Database;
use super::{SessionMetaRow, SessionSettings, SessionTreeRow};
use crate::traits::SessionStore;

impl SessionStore for Database {
    fn save_session(&self, chat_id: i64, messages_json: &str) -> Result<(), MchactError> {
        self.save_session_with_meta(chat_id, messages_json, None, None, None)
    }

    fn save_session_with_meta(
        &self,
        chat_id: i64,
        messages_json: &str,
        parent_session_key: Option<&str>,
        fork_point: Option<i64>,
        skill_envs_json: Option<&str>,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (chat_id, messages_json, updated_at, parent_session_key, fork_point, skill_envs_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chat_id) DO UPDATE SET
                messages_json = ?2,
                updated_at = ?3,
                parent_session_key = COALESCE(?4, parent_session_key),
                fork_point = COALESCE(?5, fork_point),
                skill_envs_json = COALESCE(?6, skill_envs_json)",
            params![chat_id, messages_json, now, parent_session_key, fork_point, skill_envs_json],
        )?;
        Ok(())
    }

    fn save_session_skill_envs(
        &self,
        chat_id: i64,
        skill_envs_json: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "UPDATE sessions SET skill_envs_json = ?2 WHERE chat_id = ?1",
            params![chat_id, skill_envs_json],
        )?;
        Ok(())
    }

    fn load_session(&self, chat_id: i64) -> Result<Option<(String, String)>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT messages_json, updated_at FROM sessions WHERE chat_id = ?1",
            params![chat_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok(pair) => Ok(Some(pair)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn load_session_skill_envs(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT skill_envs_json FROM sessions WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get::<_, Option<String>>(0),
        );
        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save_session_settings(
        &self,
        chat_id: i64,
        settings: &SessionSettings,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (
                chat_id,
                messages_json,
                updated_at,
                label,
                thinking_level,
                verbose_level,
                reasoning_level
             )
             VALUES (?1, '[]', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chat_id) DO UPDATE SET
                updated_at = excluded.updated_at,
                label = COALESCE(excluded.label, sessions.label),
                thinking_level = COALESCE(excluded.thinking_level, sessions.thinking_level),
                verbose_level = COALESCE(excluded.verbose_level, sessions.verbose_level),
                reasoning_level = COALESCE(excluded.reasoning_level, sessions.reasoning_level)",
            params![
                chat_id,
                now,
                settings.label,
                settings.thinking_level,
                settings.verbose_level,
                settings.reasoning_level
            ],
        )?;
        Ok(())
    }

    fn load_session_settings(
        &self,
        chat_id: i64,
    ) -> Result<Option<SessionSettings>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT label, thinking_level, verbose_level, reasoning_level
             FROM sessions
             WHERE chat_id = ?1",
            params![chat_id],
            |row| {
                Ok(SessionSettings {
                    label: row.get::<_, Option<String>>(0)?,
                    thinking_level: row.get::<_, Option<String>>(1)?,
                    verbose_level: row.get::<_, Option<String>>(2)?,
                    reasoning_level: row.get::<_, Option<String>>(3)?,
                })
            },
        );
        match result {
            Ok(settings) => Ok(Some(settings)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn load_session_meta(
        &self,
        chat_id: i64,
    ) -> Result<Option<SessionMetaRow>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT messages_json, updated_at, parent_session_key, fork_point
             FROM sessions WHERE chat_id = ?1",
            params![chat_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn list_session_meta(&self, limit: usize) -> Result<Vec<SessionTreeRow>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT chat_id, parent_session_key, fork_point, updated_at
             FROM sessions
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn delete_session(&self, chat_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute("DELETE FROM sessions WHERE chat_id = ?1", params![chat_id])?;
        Ok(rows > 0)
    }

    /// Clear all resettable chat state without deleting chat metadata or memories.
    /// This removes resumable session state, historical messages, and scheduled task state.
    fn clear_chat_context(&self, chat_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;
        let mut affected = 0usize;
        affected += tx.execute(
            "DELETE FROM task_run_logs WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM scheduled_task_dlq WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM scheduled_tasks WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute("DELETE FROM sessions WHERE chat_id = ?1", params![chat_id])?;
        affected += tx.execute("DELETE FROM messages WHERE chat_id = ?1", params![chat_id])?;
        tx.commit()?;
        Ok(affected > 0)
    }

    /// Clear conversational context for a chat while preserving scheduled task state.
    /// This removes resumable session state and historical messages only.
    fn clear_chat_conversation(&self, chat_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;
        let mut affected = 0usize;
        affected += tx.execute("DELETE FROM sessions WHERE chat_id = ?1", params![chat_id])?;
        affected += tx.execute("DELETE FROM messages WHERE chat_id = ?1", params![chat_id])?;
        tx.commit()?;
        Ok(affected > 0)
    }

    /// Clear memory state for a chat without deleting chat/session/message history.
    /// This removes structured memories and reflector bookkeeping for the chat.
    fn clear_chat_memory(&self, chat_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;
        let mut affected = 0usize;
        affected += tx.execute(
            "DELETE FROM memory_reflector_state WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_reflector_runs WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_injection_logs WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_supersede_edges
             WHERE from_memory_id IN (SELECT id FROM memories WHERE chat_id = ?1)
                OR to_memory_id IN (SELECT id FROM memories WHERE chat_id = ?1)",
            params![chat_id],
        )?;
        affected += tx.execute("DELETE FROM memories WHERE chat_id = ?1", params![chat_id])?;
        tx.commit()?;
        Ok(affected > 0)
    }

    fn delete_chat_data(&self, chat_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;
        let mut affected = 0usize;

        affected += tx.execute(
            "DELETE FROM llm_usage_logs WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute("DELETE FROM sessions WHERE chat_id = ?1", params![chat_id])?;
        affected += tx.execute("DELETE FROM messages WHERE chat_id = ?1", params![chat_id])?;
        affected += tx.execute(
            "DELETE FROM scheduled_tasks WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_reflector_state WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_reflector_runs WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_injection_logs WHERE chat_id = ?1",
            params![chat_id],
        )?;
        affected += tx.execute(
            "DELETE FROM memory_supersede_edges
             WHERE from_memory_id IN (SELECT id FROM memories WHERE chat_id = ?1)
                OR to_memory_id IN (SELECT id FROM memories WHERE chat_id = ?1)",
            params![chat_id],
        )?;
        affected += tx.execute("DELETE FROM memories WHERE chat_id = ?1", params![chat_id])?;
        affected += tx.execute("DELETE FROM chats WHERE chat_id = ?1", params![chat_id])?;

        tx.commit()?;
        Ok(affected > 0)
    }
}

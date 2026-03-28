use mchact_core::error::MchactError;

use crate::db::types::{SessionMetaRow, SessionSettings, SessionTreeRow};
use crate::traits::SessionStore;

use super::PgDriver;

impl SessionStore for PgDriver {
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
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let messages_json = messages_json.to_string();
        let parent_session_key = parent_session_key.map(|s| s.to_string());
        let skill_envs_json = skill_envs_json.map(|s| s.to_string());

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO sessions (chat_id, messages_json, updated_at, parent_session_key, fork_point, skill_envs_json)
                         VALUES ($1, $2, $3, $4, $5, $6)
                         ON CONFLICT(chat_id) DO UPDATE SET
                             messages_json = $2,
                             updated_at = $3,
                             parent_session_key = COALESCE($4, sessions.parent_session_key),
                             fork_point = COALESCE($5, sessions.fork_point),
                             skill_envs_json = COALESCE($6, sessions.skill_envs_json)",
                        &[
                            &chat_id,
                            &messages_json,
                            &now,
                            &parent_session_key,
                            &fork_point,
                            &skill_envs_json,
                        ],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("save_session_with_meta: {e}")))?;
                Ok(())
            })
        })
    }

    fn save_session_skill_envs(
        &self,
        chat_id: i64,
        skill_envs_json: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let skill_envs_json = skill_envs_json.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE sessions SET skill_envs_json = $2 WHERE chat_id = $1",
                        &[&chat_id, &skill_envs_json],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("save_session_skill_envs: {e}")))?;
                Ok(())
            })
        })
    }

    fn load_session(&self, chat_id: i64) -> Result<Option<(String, String)>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT messages_json, updated_at FROM sessions WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("load_session: {e}")))?;
                Ok(row.map(|r| {
                    let messages_json: String = r.get("messages_json");
                    let updated_at: String = r.get("updated_at");
                    (messages_json, updated_at)
                }))
            })
        })
    }

    fn load_session_skill_envs(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT skill_envs_json FROM sessions WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("load_session_skill_envs: {e}")))?;
                Ok(row.and_then(|r| r.get::<_, Option<String>>("skill_envs_json")))
            })
        })
    }

    fn save_session_settings(
        &self,
        chat_id: i64,
        settings: &SessionSettings,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let label = settings.label.clone();
        let thinking_level = settings.thinking_level.clone();
        let verbose_level = settings.verbose_level.clone();
        let reasoning_level = settings.reasoning_level.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO sessions (
                             chat_id,
                             messages_json,
                             updated_at,
                             label,
                             thinking_level,
                             verbose_level,
                             reasoning_level
                         )
                         VALUES ($1, '[]', $2, $3, $4, $5, $6)
                         ON CONFLICT(chat_id) DO UPDATE SET
                             updated_at = EXCLUDED.updated_at,
                             label = COALESCE(EXCLUDED.label, sessions.label),
                             thinking_level = COALESCE(EXCLUDED.thinking_level, sessions.thinking_level),
                             verbose_level = COALESCE(EXCLUDED.verbose_level, sessions.verbose_level),
                             reasoning_level = COALESCE(EXCLUDED.reasoning_level, sessions.reasoning_level)",
                        &[
                            &chat_id,
                            &now,
                            &label,
                            &thinking_level,
                            &verbose_level,
                            &reasoning_level,
                        ],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("save_session_settings: {e}")))?;
                Ok(())
            })
        })
    }

    fn load_session_settings(
        &self,
        chat_id: i64,
    ) -> Result<Option<SessionSettings>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT label, thinking_level, verbose_level, reasoning_level
                         FROM sessions
                         WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("load_session_settings: {e}")))?;
                Ok(row.map(|r| SessionSettings {
                    label: r.get("label"),
                    thinking_level: r.get("thinking_level"),
                    verbose_level: r.get("verbose_level"),
                    reasoning_level: r.get("reasoning_level"),
                }))
            })
        })
    }

    fn load_session_meta(
        &self,
        chat_id: i64,
    ) -> Result<Option<SessionMetaRow>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT messages_json, updated_at, parent_session_key, fork_point
                         FROM sessions WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("load_session_meta: {e}")))?;
                Ok(row.map(|r| {
                    let messages_json: String = r.get("messages_json");
                    let updated_at: String = r.get("updated_at");
                    let parent_session_key: Option<String> = r.get("parent_session_key");
                    let fork_point: Option<i64> = r.get("fork_point");
                    (messages_json, updated_at, parent_session_key, fork_point)
                }))
            })
        })
    }

    fn list_session_meta(&self, limit: usize) -> Result<Vec<SessionTreeRow>, MchactError> {
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
                        "SELECT chat_id, parent_session_key, fork_point, updated_at
                         FROM sessions
                         ORDER BY updated_at DESC
                         LIMIT $1",
                        &[&limit],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("list_session_meta: {e}")))?;
                let result = rows
                    .iter()
                    .map(|row| {
                        let chat_id: i64 = row.get("chat_id");
                        let parent_session_key: Option<String> = row.get("parent_session_key");
                        let fork_point: Option<i64> = row.get("fork_point");
                        let updated_at: String = row.get("updated_at");
                        (chat_id, parent_session_key, fork_point, updated_at)
                    })
                    .collect();
                Ok(result)
            })
        })
    }

    fn delete_session(&self, chat_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let affected = client
                    .execute(
                        "DELETE FROM sessions WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_session: {e}")))?;
                Ok(affected > 0)
            })
        })
    }

    fn clear_chat_context(&self, chat_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let mut client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let tx = client
                    .build_transaction()
                    .start()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context tx: {e}")))?;
                let mut affected: u64 = 0;
                affected += tx
                    .execute("DELETE FROM task_run_logs WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context task_run_logs: {e}")))?;
                affected += tx
                    .execute("DELETE FROM scheduled_task_dlq WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context scheduled_task_dlq: {e}")))?;
                affected += tx
                    .execute("DELETE FROM scheduled_tasks WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context scheduled_tasks: {e}")))?;
                affected += tx
                    .execute("DELETE FROM sessions WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context sessions: {e}")))?;
                affected += tx
                    .execute("DELETE FROM messages WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context messages: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_context commit: {e}")))?;
                Ok(affected > 0)
            })
        })
    }

    fn clear_chat_conversation(&self, chat_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let mut client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let tx = client
                    .build_transaction()
                    .start()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_conversation tx: {e}")))?;
                let mut affected: u64 = 0;
                affected += tx
                    .execute("DELETE FROM sessions WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_conversation sessions: {e}")))?;
                affected += tx
                    .execute("DELETE FROM messages WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_conversation messages: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_conversation commit: {e}")))?;
                Ok(affected > 0)
            })
        })
    }

    fn clear_chat_memory(&self, chat_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let mut client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let tx = client
                    .build_transaction()
                    .start()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory tx: {e}")))?;
                let mut affected: u64 = 0;
                affected += tx
                    .execute("DELETE FROM memory_reflector_state WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory reflector_state: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memory_reflector_runs WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory reflector_runs: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memory_injection_logs WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory injection_logs: {e}")))?;
                affected += tx
                    .execute(
                        "DELETE FROM memory_supersede_edges
                         WHERE from_memory_id IN (SELECT id FROM memories WHERE chat_id = $1)
                            OR to_memory_id IN (SELECT id FROM memories WHERE chat_id = $1)",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory supersede_edges: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memories WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory memories: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("clear_chat_memory commit: {e}")))?;
                Ok(affected > 0)
            })
        })
    }

    fn delete_chat_data(&self, chat_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let mut client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("pool: {e}")))?;
                let tx = client
                    .build_transaction()
                    .start()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data tx: {e}")))?;
                let mut affected: u64 = 0;
                affected += tx
                    .execute("DELETE FROM llm_usage_logs WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data llm_usage_logs: {e}")))?;
                affected += tx
                    .execute("DELETE FROM sessions WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data sessions: {e}")))?;
                affected += tx
                    .execute("DELETE FROM messages WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data messages: {e}")))?;
                affected += tx
                    .execute("DELETE FROM scheduled_tasks WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data scheduled_tasks: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memory_reflector_state WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data reflector_state: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memory_reflector_runs WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data reflector_runs: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memory_injection_logs WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data injection_logs: {e}")))?;
                affected += tx
                    .execute(
                        "DELETE FROM memory_supersede_edges
                         WHERE from_memory_id IN (SELECT id FROM memories WHERE chat_id = $1)
                            OR to_memory_id IN (SELECT id FROM memories WHERE chat_id = $1)",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data supersede_edges: {e}")))?;
                affected += tx
                    .execute("DELETE FROM memories WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data memories: {e}")))?;
                affected += tx
                    .execute("DELETE FROM chats WHERE chat_id = $1", &[&chat_id])
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data chats: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| MchactError::ToolExecution(format!("delete_chat_data commit: {e}")))?;
                Ok(affected > 0)
            })
        })
    }
}

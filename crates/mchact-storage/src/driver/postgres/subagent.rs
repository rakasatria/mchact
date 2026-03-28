use mchact_core::error::MchactError;

use crate::db::types::{
    CreateSubagentRunParams, Finding, FinishSubagentRunParams, SubagentAnnounceRecord,
    SubagentEventRecord, SubagentObservabilitySnapshot, SubagentRunRecord,
};
use crate::traits::SubagentStore;

use super::PgDriver;

fn map_run_row(row: &tokio_postgres::Row) -> SubagentRunRecord {
    SubagentRunRecord {
        run_id: row.get("run_id"),
        parent_run_id: row.get("parent_run_id"),
        depth: row.get("depth"),
        token_budget: row.get("token_budget"),
        chat_id: row.get("chat_id"),
        caller_channel: row.get("caller_channel"),
        task: row.get("task"),
        context: row.get("context"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        cancel_requested: row.get("cancel_requested"),
        error_text: row.get("error_text"),
        result_text: row.get("result_text"),
        input_tokens: row.get("input_tokens"),
        output_tokens: row.get("output_tokens"),
        total_tokens: row.get("total_tokens"),
        provider: row.get("provider"),
        model: row.get("model"),
        artifact_json: row.get("artifact_json"),
    }
}

const RUN_COLS: &str = "run_id, parent_run_id, depth, token_budget, chat_id, caller_channel, \
    task, context, status, created_at, started_at, finished_at, cancel_requested, \
    error_text, result_text, input_tokens, output_tokens, total_tokens, provider, model, artifact_json";

impl SubagentStore for PgDriver {
    fn create_subagent_run(
        &self,
        params: CreateSubagentRunParams<'_>,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let run_id = params.run_id.to_string();
        let parent_run_id = params.parent_run_id.map(|s| s.to_string());
        let depth = params.depth;
        let token_budget = params.token_budget;
        let chat_id = params.chat_id;
        let caller_channel = params.caller_channel.to_string();
        let task = params.task.to_string();
        let context = params.context.to_string();
        let provider = params.provider.to_string();
        let model = params.model.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO subagent_runs(
                             run_id, parent_run_id, depth, token_budget, chat_id, caller_channel,
                             task, context, status, created_at, provider, model
                         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'accepted', $9, $10, $11)",
                        &[
                            &run_id, &parent_run_id, &depth, &token_budget, &chat_id,
                            &caller_channel, &task, &context, &now, &provider, &model,
                        ],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("create_subagent_run: {e}")))?;
                Ok(())
            })
        })
    }

    fn mark_subagent_queued(&self, run_id: &str) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE subagent_runs
                         SET status = 'queued'
                         WHERE run_id = $1 AND status = 'accepted'",
                        &[&run_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("mark_subagent_queued: {e}")))?;
                Ok(())
            })
        })
    }

    fn mark_subagent_running(&self, run_id: &str) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE subagent_runs
                         SET status = 'running', started_at = COALESCE(started_at, $2)
                         WHERE run_id = $1",
                        &[&run_id, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("mark_subagent_running: {e}")))?;
                Ok(())
            })
        })
    }

    fn mark_subagent_finished(
        &self,
        params: FinishSubagentRunParams<'_>,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let run_id = params.run_id.to_string();
        let status = params.status.to_string();
        let error_text = params.error_text.map(|s| s.to_string());
        let result_text = params.result_text.map(|s| s.to_string());
        let artifact_json = params.artifact_json.map(|s| s.to_string());
        let input_tokens = params.input_tokens;
        let output_tokens = params.output_tokens;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE subagent_runs
                         SET status = $2,
                             finished_at = $3,
                             error_text = $4,
                             result_text = $5,
                             artifact_json = $6,
                             input_tokens = $7,
                             output_tokens = $8,
                             total_tokens = ($7 + $8)
                         WHERE run_id = $1",
                        &[
                            &run_id, &status, &now, &error_text, &result_text,
                            &artifact_json, &input_tokens, &output_tokens,
                        ],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("mark_subagent_finished: {e}")))?;
                Ok(())
            })
        })
    }

    fn is_subagent_cancel_requested(&self, run_id: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT cancel_requested FROM subagent_runs WHERE run_id = $1",
                        &[&run_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("is_subagent_cancel_requested: {e}")))?;
                Ok(row.map(|r| r.get::<_, bool>("cancel_requested")).unwrap_or(false))
            })
        })
    }

    fn request_subagent_cancel(
        &self,
        run_id: &str,
        chat_id: i64,
    ) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let affected = client
                    .execute(
                        "UPDATE subagent_runs
                         SET cancel_requested = true
                         WHERE run_id = $1 AND chat_id = $2
                           AND status IN ('accepted', 'queued', 'running')",
                        &[&run_id, &chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("request_subagent_cancel: {e}")))?;
                Ok(affected > 0)
            })
        })
    }

    fn list_subagent_runs(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<SubagentRunRecord>, MchactError> {
        let pool = self.pool.clone();
        let limit = limit.max(1) as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        &format!(
                            "SELECT {RUN_COLS}
                             FROM subagent_runs
                             WHERE chat_id = $1
                             ORDER BY created_at DESC
                             LIMIT $2"
                        ),
                        &[&chat_id, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_subagent_runs: {e}")))?;
                Ok(rows.iter().map(map_run_row).collect())
            })
        })
    }

    fn get_subagent_run(
        &self,
        run_id: &str,
        chat_id: i64,
    ) -> Result<Option<SubagentRunRecord>, MchactError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        &format!(
                            "SELECT {RUN_COLS}
                             FROM subagent_runs
                             WHERE run_id = $1 AND chat_id = $2"
                        ),
                        &[&run_id, &chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_subagent_run: {e}")))?;
                Ok(row.as_ref().map(map_run_row))
            })
        })
    }

    fn count_active_subagent_runs_for_chat(&self, chat_id: i64) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "SELECT COUNT(*) FROM subagent_runs
                         WHERE chat_id = $1
                           AND status IN ('accepted', 'queued', 'running')",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("count_active_subagent_runs_for_chat: {e}")))?;
                Ok(row.get::<_, i64>(0))
            })
        })
    }

    fn count_active_subagent_children(
        &self,
        parent_run_id: &str,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let parent_run_id = parent_run_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "SELECT COUNT(*) FROM subagent_runs
                         WHERE parent_run_id = $1
                           AND status IN ('accepted', 'queued', 'running')",
                        &[&parent_run_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("count_active_subagent_children: {e}")))?;
                Ok(row.get::<_, i64>(0))
            })
        })
    }

    fn enqueue_subagent_announce(
        &self,
        run_id: &str,
        chat_id: i64,
        caller_channel: &str,
        payload_text: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let run_id = run_id.to_string();
        let caller_channel = caller_channel.to_string();
        let payload_text = payload_text.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO subagent_announces(
                             run_id, chat_id, caller_channel, payload_text, status,
                             attempts, next_attempt_at, created_at, updated_at
                         ) VALUES($1, $2, $3, $4, 'pending', 0, $5, $6, $6)
                         ON CONFLICT(run_id) DO NOTHING",
                        &[&run_id, &chat_id, &caller_channel, &payload_text, &now, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("enqueue_subagent_announce: {e}")))?;
                Ok(())
            })
        })
    }

    fn list_due_subagent_announces(
        &self,
        now_iso: &str,
        limit: usize,
    ) -> Result<Vec<SubagentAnnounceRecord>, MchactError> {
        let pool = self.pool.clone();
        let now_iso = now_iso.to_string();
        let limit = limit.max(1) as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, run_id, chat_id, caller_channel, payload_text, status,
                                attempts, next_attempt_at, last_error
                         FROM subagent_announces
                         WHERE status IN ('pending', 'retry')
                           AND (next_attempt_at IS NULL
                                OR next_attempt_at::timestamptz <= $1::timestamptz)
                         ORDER BY id ASC
                         LIMIT $2",
                        &[&now_iso, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_due_subagent_announces: {e}")))?;
                Ok(rows
                    .iter()
                    .map(|row| SubagentAnnounceRecord {
                        id: row.get("id"),
                        run_id: row.get("run_id"),
                        chat_id: row.get("chat_id"),
                        caller_channel: row.get("caller_channel"),
                        payload_text: row.get("payload_text"),
                        status: row.get("status"),
                        attempts: row.get("attempts"),
                        next_attempt_at: row.get("next_attempt_at"),
                        last_error: row.get("last_error"),
                    })
                    .collect())
            })
        })
    }

    fn mark_subagent_announce_sent(&self, id: i64) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE subagent_announces SET status = 'sent', updated_at = $2 WHERE id = $1",
                        &[&id, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("mark_subagent_announce_sent: {e}")))?;
                Ok(())
            })
        })
    }

    fn mark_subagent_announce_retry(
        &self,
        id: i64,
        attempts: i64,
        next_attempt_at: Option<&str>,
        last_error: &str,
        terminal_fail: bool,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let status = if terminal_fail { "failed" } else { "retry" }.to_string();
        let next_attempt_at = next_attempt_at.map(|s| s.to_string());
        let last_error = last_error.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "UPDATE subagent_announces
                         SET status = $2, attempts = $3, next_attempt_at = $4,
                             last_error = $5, updated_at = $6
                         WHERE id = $1",
                        &[&id, &status, &attempts, &next_attempt_at, &last_error, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("mark_subagent_announce_retry: {e}")))?;
                Ok(())
            })
        })
    }

    fn append_subagent_event(
        &self,
        run_id: &str,
        event_type: &str,
        detail: Option<&str>,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let run_id = run_id.to_string();
        let event_type = event_type.to_string();
        let detail = detail.map(|s| s.to_string());
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO subagent_events(run_id, event_type, detail, created_at)
                         VALUES($1, $2, $3, $4)",
                        &[&run_id, &event_type, &detail, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("append_subagent_event: {e}")))?;
                Ok(())
            })
        })
    }

    fn list_subagent_events(
        &self,
        run_id: &str,
        limit: usize,
    ) -> Result<Vec<SubagentEventRecord>, MchactError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();
        let limit = limit.max(1) as i64;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, run_id, event_type, detail, created_at
                         FROM subagent_events
                         WHERE run_id = $1
                         ORDER BY created_at DESC
                         LIMIT $2",
                        &[&run_id, &limit],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("list_subagent_events: {e}")))?;
                Ok(rows
                    .iter()
                    .map(|row| SubagentEventRecord {
                        id: row.get("id"),
                        run_id: row.get("run_id"),
                        event_type: row.get("event_type"),
                        detail: row.get("detail"),
                        created_at: row.get("created_at"),
                    })
                    .collect())
            })
        })
    }

    fn set_subagent_focus(&self, chat_id: i64, run_id: &str) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let run_id = run_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "INSERT INTO subagent_focus_bindings(chat_id, run_id, updated_at)
                         VALUES($1, $2, $3)
                         ON CONFLICT(chat_id) DO UPDATE SET
                             run_id = EXCLUDED.run_id,
                             updated_at = EXCLUDED.updated_at",
                        &[&chat_id, &run_id, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("set_subagent_focus: {e}")))?;
                Ok(())
            })
        })
    }

    fn clear_subagent_focus(&self, chat_id: i64) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                client
                    .execute(
                        "DELETE FROM subagent_focus_bindings WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("clear_subagent_focus: {e}")))?;
                Ok(())
            })
        })
    }

    fn get_subagent_focus(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_opt(
                        "SELECT run_id FROM subagent_focus_bindings WHERE chat_id = $1",
                        &[&chat_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_subagent_focus: {e}")))?;
                Ok(row.map(|r| r.get::<_, String>("run_id")))
            })
        })
    }

    fn get_subagent_observability_snapshot(
        &self,
        chat_id: Option<i64>,
        recent_limit: usize,
    ) -> Result<SubagentObservabilitySnapshot, MchactError> {
        let pool = self.pool.clone();
        let recent_limit = recent_limit.max(1) as i64;
        let since = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;

                macro_rules! count_with_filter {
                    ($sql_no_filter:expr, $sql_with_filter:expr) => {
                        if let Some(cid) = chat_id {
                            client
                                .query_one($sql_with_filter, &[&cid])
                                .await
                                .map_err(|e| MchactError::Database(format!("obs snapshot count: {e}")))?
                                .get::<_, i64>(0)
                        } else {
                            client
                                .query_one($sql_no_filter, &[])
                                .await
                                .map_err(|e| MchactError::Database(format!("obs snapshot count: {e}")))?
                                .get::<_, i64>(0)
                        }
                    };
                }

                let active_runs = count_with_filter!(
                    "SELECT COUNT(*) FROM subagent_runs WHERE status IN ('accepted','queued','running')",
                    "SELECT COUNT(*) FROM subagent_runs WHERE status IN ('accepted','queued','running') AND chat_id = $1"
                );
                let queued_runs = count_with_filter!(
                    "SELECT COUNT(*) FROM subagent_runs WHERE status = 'queued'",
                    "SELECT COUNT(*) FROM subagent_runs WHERE status = 'queued' AND chat_id = $1"
                );
                let running_runs = count_with_filter!(
                    "SELECT COUNT(*) FROM subagent_runs WHERE status = 'running'",
                    "SELECT COUNT(*) FROM subagent_runs WHERE status = 'running' AND chat_id = $1"
                );
                let pending_announces = count_with_filter!(
                    "SELECT COUNT(*) FROM subagent_announces WHERE status = 'pending'",
                    "SELECT COUNT(*) FROM subagent_announces WHERE status = 'pending' AND chat_id = $1"
                );
                let retry_announces = count_with_filter!(
                    "SELECT COUNT(*) FROM subagent_announces WHERE status = 'retry'",
                    "SELECT COUNT(*) FROM subagent_announces WHERE status = 'retry' AND chat_id = $1"
                );
                let failed_announces = count_with_filter!(
                    "SELECT COUNT(*) FROM subagent_announces WHERE status = 'failed'",
                    "SELECT COUNT(*) FROM subagent_announces WHERE status = 'failed' AND chat_id = $1"
                );

                let completed_24h: i64 = if let Some(cid) = chat_id {
                    client
                        .query_one(
                            "SELECT COUNT(*) FROM subagent_runs
                             WHERE status = 'completed' AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz
                               AND chat_id = $2",
                            &[&since, &cid],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs completed_24h: {e}")))?
                        .get(0)
                } else {
                    client
                        .query_one(
                            "SELECT COUNT(*) FROM subagent_runs
                             WHERE status = 'completed' AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz",
                            &[&since],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs completed_24h: {e}")))?
                        .get(0)
                };

                let failed_24h: i64 = if let Some(cid) = chat_id {
                    client
                        .query_one(
                            "SELECT COUNT(*) FROM subagent_runs
                             WHERE status IN ('failed','timed_out','cancelled') AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz
                               AND chat_id = $2",
                            &[&since, &cid],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs failed_24h: {e}")))?
                        .get(0)
                } else {
                    client
                        .query_one(
                            "SELECT COUNT(*) FROM subagent_runs
                             WHERE status IN ('failed','timed_out','cancelled') AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz",
                            &[&since],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs failed_24h: {e}")))?
                        .get(0)
                };

                let budget_exceeded_24h: i64 = if let Some(cid) = chat_id {
                    client
                        .query_one(
                            "SELECT COUNT(*) FROM subagent_runs
                             WHERE status = 'budget_exceeded' AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz
                               AND chat_id = $2",
                            &[&since, &cid],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs budget_exceeded_24h: {e}")))?
                        .get(0)
                } else {
                    client
                        .query_one(
                            "SELECT COUNT(*) FROM subagent_runs
                             WHERE status = 'budget_exceeded' AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz",
                            &[&since],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs budget_exceeded_24h: {e}")))?
                        .get(0)
                };

                let avg_duration_ms_24h: i64 = if let Some(cid) = chat_id {
                    client
                        .query_one(
                            "SELECT COALESCE(AVG(
                                 EXTRACT(EPOCH FROM (finished_at::timestamptz - started_at::timestamptz)) * 1000.0
                             ), 0)
                             FROM subagent_runs
                             WHERE started_at IS NOT NULL AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz
                               AND chat_id = $2",
                            &[&since, &cid],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs avg_duration: {e}")))?
                        .get::<_, f64>(0) as i64
                } else {
                    client
                        .query_one(
                            "SELECT COALESCE(AVG(
                                 EXTRACT(EPOCH FROM (finished_at::timestamptz - started_at::timestamptz)) * 1000.0
                             ), 0)
                             FROM subagent_runs
                             WHERE started_at IS NOT NULL AND finished_at IS NOT NULL
                               AND finished_at::timestamptz >= $1::timestamptz",
                            &[&since],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs avg_duration: {e}")))?
                        .get::<_, f64>(0) as i64
                };

                let recent_rows = if let Some(cid) = chat_id {
                    client
                        .query(
                            &format!(
                                "SELECT {RUN_COLS}
                                 FROM subagent_runs
                                 WHERE chat_id = $1
                                 ORDER BY created_at DESC
                                 LIMIT $2"
                            ),
                            &[&cid, &recent_limit],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs recent_runs: {e}")))?
                } else {
                    client
                        .query(
                            &format!(
                                "SELECT {RUN_COLS}
                                 FROM subagent_runs
                                 ORDER BY created_at DESC
                                 LIMIT $1"
                            ),
                            &[&recent_limit],
                        )
                        .await
                        .map_err(|e| MchactError::Database(format!("obs recent_runs: {e}")))?
                };

                let recent_runs = recent_rows.iter().map(map_run_row).collect();

                Ok(SubagentObservabilitySnapshot {
                    active_runs,
                    queued_runs,
                    running_runs,
                    pending_announces,
                    retry_announces,
                    failed_announces,
                    completed_24h,
                    failed_24h,
                    budget_exceeded_24h,
                    avg_duration_ms_24h,
                    recent_runs,
                })
            })
        })
    }

    fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let orchestration_id = orchestration_id.to_string();
        let run_id = run_id.to_string();
        let finding = finding.to_string();
        let category = category.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let row = client
                    .query_one(
                        "INSERT INTO subagent_findings(orchestration_id, run_id, finding, category, created_at)
                         VALUES ($1, $2, $3, $4, $5)
                         RETURNING id",
                        &[&orchestration_id, &run_id, &finding, &category, &now],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("insert_finding: {e}")))?;
                Ok(row.get::<_, i64>(0))
            })
        })
    }

    fn get_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<Vec<Finding>, MchactError> {
        let pool = self.pool.clone();
        let orchestration_id = orchestration_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let rows = client
                    .query(
                        "SELECT id, orchestration_id, run_id, finding, category, created_at
                         FROM subagent_findings
                         WHERE orchestration_id = $1
                         ORDER BY created_at ASC",
                        &[&orchestration_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("get_findings: {e}")))?;
                Ok(rows
                    .iter()
                    .map(|row| Finding {
                        id: row.get("id"),
                        orchestration_id: row.get("orchestration_id"),
                        run_id: row.get("run_id"),
                        finding: row.get("finding"),
                        category: row.get("category"),
                        created_at: row.get("created_at"),
                    })
                    .collect())
            })
        })
    }

    fn delete_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<usize, MchactError> {
        let pool = self.pool.clone();
        let orchestration_id = orchestration_id.to_string();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let client = pool
                    .get()
                    .await
                    .map_err(|e| MchactError::Database(format!("pool: {e}")))?;
                let affected = client
                    .execute(
                        "DELETE FROM subagent_findings WHERE orchestration_id = $1",
                        &[&orchestration_id],
                    )
                    .await
                    .map_err(|e| MchactError::Database(format!("delete_findings: {e}")))?;
                Ok(affected as usize)
            })
        })
    }
}

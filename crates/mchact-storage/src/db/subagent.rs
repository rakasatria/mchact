use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::{
    CreateSubagentRunParams, Finding, FinishSubagentRunParams, SubagentAnnounceRecord,
    SubagentEventRecord, SubagentObservabilitySnapshot, SubagentRunRecord,
};

impl Database {
    pub fn create_subagent_run(
        &self,
        params: CreateSubagentRunParams<'_>,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subagent_runs(
                run_id, parent_run_id, depth, token_budget, chat_id, caller_channel, task, context, status, created_at, provider, model
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'accepted', ?9, ?10, ?11)",
            rusqlite::params![
                params.run_id,
                params.parent_run_id,
                params.depth,
                params.token_budget,
                params.chat_id,
                params.caller_channel,
                params.task,
                params.context,
                now,
                params.provider,
                params.model
            ],
        )?;
        Ok(())
    }

    pub fn mark_subagent_queued(&self, run_id: &str) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "UPDATE subagent_runs
             SET status = 'queued'
             WHERE run_id = ?1 AND status = 'accepted'",
            params![run_id],
        )?;
        Ok(())
    }

    pub fn mark_subagent_running(&self, run_id: &str) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE subagent_runs
             SET status = 'running', started_at = COALESCE(started_at, ?2)
             WHERE run_id = ?1",
            params![run_id, now],
        )?;
        Ok(())
    }

    pub fn mark_subagent_finished(
        &self,
        params: FinishSubagentRunParams<'_>,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE subagent_runs
             SET status = ?2,
                 finished_at = ?3,
                 error_text = ?4,
                 result_text = ?5,
                 artifact_json = ?6,
                 input_tokens = ?7,
                 output_tokens = ?8,
                 total_tokens = (?7 + ?8)
             WHERE run_id = ?1",
            rusqlite::params![
                params.run_id,
                params.status,
                now,
                params.error_text,
                params.result_text,
                params.artifact_json,
                params.input_tokens,
                params.output_tokens
            ],
        )?;
        Ok(())
    }

    pub fn is_subagent_cancel_requested(&self, run_id: &str) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let requested = conn
            .query_row(
                "SELECT cancel_requested FROM subagent_runs WHERE run_id = ?1",
                params![run_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        Ok(requested != 0)
    }

    pub fn request_subagent_cancel(
        &self,
        run_id: &str,
        chat_id: i64,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let affected = conn.execute(
            "UPDATE subagent_runs
             SET cancel_requested = 1
             WHERE run_id = ?1 AND chat_id = ?2
               AND status IN ('accepted', 'queued', 'running')",
            params![run_id, chat_id],
        )?;
        Ok(affected > 0)
    }

    pub fn list_subagent_runs(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<SubagentRunRecord>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT run_id, parent_run_id, depth, token_budget, chat_id, caller_channel, task, context, status, created_at,
                    started_at, finished_at, cancel_requested, error_text, result_text,
                    input_tokens, output_tokens, total_tokens, provider, model, artifact_json
             FROM subagent_runs
             WHERE chat_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![chat_id, limit.max(1) as i64], |row| {
            Ok(SubagentRunRecord {
                run_id: row.get(0)?,
                parent_run_id: row.get(1)?,
                depth: row.get(2)?,
                token_budget: row.get(3)?,
                chat_id: row.get(4)?,
                caller_channel: row.get(5)?,
                task: row.get(6)?,
                context: row.get(7)?,
                status: row.get(8)?,
                created_at: row.get(9)?,
                started_at: row.get(10)?,
                finished_at: row.get(11)?,
                cancel_requested: row.get::<_, i64>(12)? != 0,
                error_text: row.get(13)?,
                result_text: row.get(14)?,
                input_tokens: row.get(15)?,
                output_tokens: row.get(16)?,
                total_tokens: row.get(17)?,
                provider: row.get(18)?,
                model: row.get(19)?,
                artifact_json: row.get(20)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_subagent_run(
        &self,
        run_id: &str,
        chat_id: i64,
    ) -> Result<Option<SubagentRunRecord>, MchactError> {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT run_id, parent_run_id, depth, token_budget, chat_id, caller_channel, task, context, status, created_at,
                    started_at, finished_at, cancel_requested, error_text, result_text,
                    input_tokens, output_tokens, total_tokens, provider, model, artifact_json
             FROM subagent_runs
             WHERE run_id = ?1 AND chat_id = ?2",
            params![run_id, chat_id],
            |row| {
                Ok(SubagentRunRecord {
                    run_id: row.get(0)?,
                    parent_run_id: row.get(1)?,
                    depth: row.get(2)?,
                    token_budget: row.get(3)?,
                    chat_id: row.get(4)?,
                    caller_channel: row.get(5)?,
                    task: row.get(6)?,
                    context: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                    started_at: row.get(10)?,
                    finished_at: row.get(11)?,
                    cancel_requested: row.get::<_, i64>(12)? != 0,
                    error_text: row.get(13)?,
                    result_text: row.get(14)?,
                    input_tokens: row.get(15)?,
                    output_tokens: row.get(16)?,
                    total_tokens: row.get(17)?,
                    provider: row.get(18)?,
                    model: row.get(19)?,
                    artifact_json: row.get(20)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn count_active_subagent_runs_for_chat(&self, chat_id: i64) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT COUNT(*)
             FROM subagent_runs
             WHERE chat_id = ?1
               AND status IN ('accepted', 'queued', 'running')",
            params![chat_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn count_active_subagent_children(
        &self,
        parent_run_id: &str,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT COUNT(*)
             FROM subagent_runs
             WHERE parent_run_id = ?1
               AND status IN ('accepted', 'queued', 'running')",
            params![parent_run_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn enqueue_subagent_announce(
        &self,
        run_id: &str,
        chat_id: i64,
        caller_channel: &str,
        payload_text: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subagent_announces(
                run_id, chat_id, caller_channel, payload_text, status, attempts, next_attempt_at, created_at, updated_at
            ) VALUES(?1, ?2, ?3, ?4, 'pending', 0, ?5, ?6, ?6)
            ON CONFLICT(run_id) DO NOTHING",
            params![run_id, chat_id, caller_channel, payload_text, now, now],
        )?;
        Ok(())
    }

    pub fn list_due_subagent_announces(
        &self,
        now_iso: &str,
        limit: usize,
    ) -> Result<Vec<SubagentAnnounceRecord>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, chat_id, caller_channel, payload_text, status, attempts, next_attempt_at, last_error
             FROM subagent_announces
             WHERE status IN ('pending', 'retry')
               AND (next_attempt_at IS NULL OR unixepoch(next_attempt_at) <= unixepoch(?1))
             ORDER BY id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![now_iso, limit.max(1) as i64], |row| {
            Ok(SubagentAnnounceRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                chat_id: row.get(2)?,
                caller_channel: row.get(3)?,
                payload_text: row.get(4)?,
                status: row.get(5)?,
                attempts: row.get(6)?,
                next_attempt_at: row.get(7)?,
                last_error: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn mark_subagent_announce_sent(&self, id: i64) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE subagent_announces
             SET status='sent', updated_at=?2
             WHERE id=?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn mark_subagent_announce_retry(
        &self,
        id: i64,
        attempts: i64,
        next_attempt_at: Option<&str>,
        last_error: &str,
        terminal_fail: bool,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let status = if terminal_fail { "failed" } else { "retry" };
        conn.execute(
            "UPDATE subagent_announces
             SET status=?2, attempts=?3, next_attempt_at=?4, last_error=?5, updated_at=?6
             WHERE id=?1",
            params![id, status, attempts, next_attempt_at, last_error, now],
        )?;
        Ok(())
    }

    pub fn append_subagent_event(
        &self,
        run_id: &str,
        event_type: &str,
        detail: Option<&str>,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subagent_events(run_id, event_type, detail, created_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![run_id, event_type, detail, now],
        )?;
        Ok(())
    }

    pub fn list_subagent_events(
        &self,
        run_id: &str,
        limit: usize,
    ) -> Result<Vec<SubagentEventRecord>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, event_type, detail, created_at
             FROM subagent_events
             WHERE run_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![run_id, limit.max(1) as i64], |row| {
            Ok(SubagentEventRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                event_type: row.get(2)?,
                detail: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_subagent_focus(&self, chat_id: i64, run_id: &str) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subagent_focus_bindings(chat_id, run_id, updated_at)
             VALUES(?1, ?2, ?3)
             ON CONFLICT(chat_id) DO UPDATE SET
                run_id = excluded.run_id,
                updated_at = excluded.updated_at",
            params![chat_id, run_id, now],
        )?;
        Ok(())
    }

    pub fn clear_subagent_focus(&self, chat_id: i64) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "DELETE FROM subagent_focus_bindings WHERE chat_id = ?1",
            params![chat_id],
        )?;
        Ok(())
    }

    pub fn get_subagent_focus(&self, chat_id: i64) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        conn.query_row(
            "SELECT run_id FROM subagent_focus_bindings WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_subagent_observability_snapshot(
        &self,
        chat_id: Option<i64>,
        recent_limit: usize,
    ) -> Result<SubagentObservabilitySnapshot, MchactError> {
        let conn = self.lock_conn();
        let since = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let active_filter = if chat_id.is_some() {
            " AND chat_id = ?1"
        } else {
            ""
        };

        let active_runs: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status IN ('accepted','queued','running') AND chat_id = ?1",
                params![cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status IN ('accepted','queued','running')",
                [],
                |row| row.get(0),
            )?
        };
        let queued_runs: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs WHERE status = 'queued' AND chat_id = ?1",
                params![cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs WHERE status = 'queued'",
                [],
                |row| row.get(0),
            )?
        };
        let running_runs: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs WHERE status = 'running' AND chat_id = ?1",
                params![cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs WHERE status = 'running'",
                [],
                |row| row.get(0),
            )?
        };

        let pending_announces: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_announces WHERE status = 'pending' AND chat_id = ?1",
                params![cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_announces WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )?
        };
        let retry_announces: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_announces WHERE status = 'retry' AND chat_id = ?1",
                params![cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_announces WHERE status = 'retry'",
                [],
                |row| row.get(0),
            )?
        };
        let failed_announces: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_announces WHERE status = 'failed' AND chat_id = ?1",
                params![cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_announces WHERE status = 'failed'",
                [],
                |row| row.get(0),
            )?
        };

        let completed_24h: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status = 'completed' AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)
                   AND chat_id = ?2",
                params![since, cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status = 'completed' AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)",
                params![since],
                |row| row.get(0),
            )?
        };
        let failed_24h: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status IN ('failed','timed_out','cancelled') AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)
                   AND chat_id = ?2",
                params![since, cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status IN ('failed','timed_out','cancelled') AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)",
                params![since],
                |row| row.get(0),
            )?
        };
        let budget_exceeded_24h: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status = 'budget_exceeded' AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)
                   AND chat_id = ?2",
                params![since, cid],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM subagent_runs
                 WHERE status = 'budget_exceeded' AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)",
                params![since],
                |row| row.get(0),
            )?
        };

        let avg_duration_ms_24h: i64 = if let Some(cid) = chat_id {
            conn.query_row(
                "SELECT COALESCE(AVG((julianday(finished_at) - julianday(started_at)) * 86400000.0), 0)
                 FROM subagent_runs
                 WHERE started_at IS NOT NULL AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)
                   AND chat_id = ?2",
                params![since, cid],
                |row| row.get::<_, f64>(0).map(|v| v as i64),
            )?
        } else {
            conn.query_row(
                "SELECT COALESCE(AVG((julianday(finished_at) - julianday(started_at)) * 86400000.0), 0)
                 FROM subagent_runs
                 WHERE started_at IS NOT NULL AND finished_at IS NOT NULL
                   AND unixepoch(finished_at) >= unixepoch(?1)",
                params![since],
                |row| row.get::<_, f64>(0).map(|v| v as i64),
            )?
        };

        let mut stmt = conn.prepare(&format!(
            "SELECT run_id, parent_run_id, depth, token_budget, chat_id, caller_channel, task, context, status, created_at,
                    started_at, finished_at, cancel_requested, error_text, result_text,
                    input_tokens, output_tokens, total_tokens, provider, model, artifact_json
             FROM subagent_runs
             WHERE 1=1 {active_filter}
             ORDER BY created_at DESC
             LIMIT ?{}",
            if chat_id.is_some() { 2 } else { 1 }
        ))?;
        let recent_runs = if let Some(cid) = chat_id {
            let rows = stmt.query_map(params![cid, recent_limit.max(1) as i64], |row| {
                Ok(SubagentRunRecord {
                    run_id: row.get(0)?,
                    parent_run_id: row.get(1)?,
                    depth: row.get(2)?,
                    token_budget: row.get(3)?,
                    chat_id: row.get(4)?,
                    caller_channel: row.get(5)?,
                    task: row.get(6)?,
                    context: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                    started_at: row.get(10)?,
                    finished_at: row.get(11)?,
                    cancel_requested: row.get::<_, i64>(12)? != 0,
                    error_text: row.get(13)?,
                    result_text: row.get(14)?,
                    input_tokens: row.get(15)?,
                    output_tokens: row.get(16)?,
                    total_tokens: row.get(17)?,
                    provider: row.get(18)?,
                    model: row.get(19)?,
                    artifact_json: row.get(20)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let rows = stmt.query_map(params![recent_limit.max(1) as i64], |row| {
                Ok(SubagentRunRecord {
                    run_id: row.get(0)?,
                    parent_run_id: row.get(1)?,
                    depth: row.get(2)?,
                    token_budget: row.get(3)?,
                    chat_id: row.get(4)?,
                    caller_channel: row.get(5)?,
                    task: row.get(6)?,
                    context: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                    started_at: row.get(10)?,
                    finished_at: row.get(11)?,
                    cancel_requested: row.get::<_, i64>(12)? != 0,
                    error_text: row.get(13)?,
                    result_text: row.get(14)?,
                    input_tokens: row.get(15)?,
                    output_tokens: row.get(16)?,
                    total_tokens: row.get(17)?,
                    provider: row.get(18)?,
                    model: row.get(19)?,
                    artifact_json: row.get(20)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

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
    }

    pub fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subagent_findings (orchestration_id, run_id, finding, category, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![orchestration_id, run_id, finding, category, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<Vec<Finding>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, orchestration_id, run_id, finding, category, created_at
             FROM subagent_findings
             WHERE orchestration_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![orchestration_id], |row| {
            Ok(Finding {
                id: row.get(0)?,
                orchestration_id: row.get(1)?,
                run_id: row.get(2)?,
                finding: row.get(3)?,
                category: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn delete_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<usize, MchactError> {
        let conn = self.lock_conn();
        let affected = conn.execute(
            "DELETE FROM subagent_findings WHERE orchestration_id = ?1",
            params![orchestration_id],
        )?;
        Ok(affected)
    }
}

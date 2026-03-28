use mchact_core::error::MchactError;
use rusqlite::params;

use super::Database;
use super::{ScheduledTask, ScheduledTaskDlqEntry, TaskRunLog};
use crate::traits::TaskStore;

impl TaskStore for Database {
    fn create_scheduled_task(
        &self,
        chat_id: i64,
        prompt: &str,
        schedule_type: &str,
        schedule_value: &str,
        next_run: &str,
    ) -> Result<i64, MchactError> {
        self.create_scheduled_task_with_timezone(
            chat_id,
            prompt,
            schedule_type,
            schedule_value,
            "",
            next_run,
        )
    }

    fn create_scheduled_task_with_timezone(
        &self,
        chat_id: i64,
        prompt: &str,
        schedule_type: &str,
        schedule_value: &str,
        timezone: &str,
        next_run: &str,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scheduled_tasks (chat_id, prompt, schedule_type, schedule_value, timezone, next_run, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7)",
            params![
                chat_id,
                prompt,
                schedule_type,
                schedule_value,
                timezone,
                next_run,
                now
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_due_tasks(&self, now: &str) -> Result<Vec<ScheduledTask>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone, next_run, last_run, status, created_at
             FROM scheduled_tasks
             WHERE status = 'active' AND next_run <= ?1",
        )?;
        let tasks = stmt
            .query_map(params![now], |row| {
                Ok(ScheduledTask {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_type: row.get(3)?,
                    schedule_value: row.get(4)?,
                    timezone: row.get(5)?,
                    next_run: row.get(6)?,
                    last_run: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    fn claim_due_tasks(
        &self,
        now: &str,
        limit: usize,
    ) -> Result<Vec<ScheduledTask>, MchactError> {
        let conn = self.lock_conn();
        let tx = conn.unchecked_transaction()?;

        let mut stmt = tx.prepare(
            "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone, next_run, last_run, status, created_at
             FROM scheduled_tasks
             WHERE status = 'active' AND next_run <= ?1
             ORDER BY next_run ASC, id ASC
             LIMIT ?2",
        )?;
        let candidates = stmt
            .query_map(params![now, limit as i64], |row| {
                Ok(ScheduledTask {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_type: row.get(3)?,
                    schedule_value: row.get(4)?,
                    timezone: row.get(5)?,
                    next_run: row.get(6)?,
                    last_run: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut claimed = Vec::new();
        for task in candidates {
            let rows = tx.execute(
                "UPDATE scheduled_tasks
                 SET status = 'running'
                 WHERE id = ?1 AND status = 'active' AND next_run <= ?2",
                params![task.id, now],
            )?;
            if rows > 0 {
                let mut claimed_task = task;
                claimed_task.status = "running".to_string();
                claimed.push(claimed_task);
            }
        }

        tx.commit()?;
        Ok(claimed)
    }

    fn get_tasks_for_chat(&self, chat_id: i64) -> Result<Vec<ScheduledTask>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone, next_run, last_run, status, created_at
             FROM scheduled_tasks
             WHERE chat_id = ?1 AND status IN ('active', 'paused')
             ORDER BY id",
        )?;
        let tasks = stmt
            .query_map(params![chat_id], |row| {
                Ok(ScheduledTask {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_type: row.get(3)?,
                    schedule_value: row.get(4)?,
                    timezone: row.get(5)?,
                    next_run: row.get(6)?,
                    last_run: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    fn get_task_by_id(&self, task_id: i64) -> Result<Option<ScheduledTask>, MchactError> {
        let conn = self.lock_conn();
        let result = conn.query_row(
            "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone, next_run, last_run, status, created_at
             FROM scheduled_tasks
             WHERE id = ?1",
            params![task_id],
            |row| {
                Ok(ScheduledTask {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_type: row.get(3)?,
                    schedule_value: row.get(4)?,
                    timezone: row.get(5)?,
                    next_run: row.get(6)?,
                    last_run: row.get(7)?,
                    status: row.get(8)?,
                    created_at: row.get(9)?,
                })
            },
        );
        match result {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn update_task_status(&self, task_id: i64, status: &str) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute(
            "UPDATE scheduled_tasks SET status = ?1 WHERE id = ?2",
            params![status, task_id],
        )?;
        Ok(rows > 0)
    }

    fn requeue_scheduled_task(
        &self,
        task_id: i64,
        next_run: &str,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute(
            "UPDATE scheduled_tasks
             SET status = 'active', next_run = ?1
             WHERE id = ?2",
            params![next_run, task_id],
        )?;
        Ok(rows > 0)
    }

    fn update_task_after_run(
        &self,
        task_id: i64,
        last_run: &str,
        next_run: Option<&str>,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        match next_run {
            Some(next) => {
                conn.execute(
                    "UPDATE scheduled_tasks
                     SET last_run = ?1, next_run = ?2, status = 'active'
                     WHERE id = ?3",
                    params![last_run, next, task_id],
                )?;
            }
            None => {
                // One-shot task, mark completed
                conn.execute(
                    "UPDATE scheduled_tasks SET last_run = ?1, status = 'completed' WHERE id = ?2",
                    params![last_run, task_id],
                )?;
            }
        }
        Ok(())
    }

    fn recover_running_tasks(&self) -> Result<usize, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute(
            "UPDATE scheduled_tasks
             SET status = 'active'
             WHERE status = 'running'",
            [],
        )?;
        Ok(rows)
    }

    #[allow(dead_code)]
    fn delete_task(&self, task_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute(
            "DELETE FROM scheduled_tasks WHERE id = ?1",
            params![task_id],
        )?;
        Ok(rows > 0)
    }

    // --- Task run logs ---

    #[allow(clippy::too_many_arguments)]
    fn log_task_run(
        &self,
        task_id: i64,
        chat_id: i64,
        started_at: &str,
        finished_at: &str,
        duration_ms: i64,
        success: bool,
        result_summary: Option<&str>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO task_run_logs (task_id, chat_id, started_at, finished_at, duration_ms, success, result_summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                task_id,
                chat_id,
                started_at,
                finished_at,
                duration_ms,
                success as i32,
                result_summary,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_task_run_logs(
        &self,
        task_id: i64,
        limit: usize,
    ) -> Result<Vec<TaskRunLog>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, task_id, chat_id, started_at, finished_at, duration_ms, success, result_summary
             FROM task_run_logs
             WHERE task_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let logs = stmt
            .query_map(params![task_id, limit as i64], |row| {
                Ok(TaskRunLog {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    started_at: row.get(3)?,
                    finished_at: row.get(4)?,
                    duration_ms: row.get(5)?,
                    success: row.get::<_, i32>(6)? != 0,
                    result_summary: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(logs)
    }

    fn get_task_run_summary_since(
        &self,
        since: Option<&str>,
    ) -> Result<(i64, i64), MchactError> {
        let conn = self.lock_conn();
        if let Some(since) = since {
            let (total, success): (i64, i64) = conn.query_row(
                "SELECT
                    COUNT(*) AS total_runs,
                    COALESCE(SUM(CASE WHEN success != 0 THEN 1 ELSE 0 END), 0) AS success_runs
                 FROM task_run_logs
                 WHERE started_at >= ?1",
                params![since],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            Ok((total, success))
        } else {
            let (total, success): (i64, i64) = conn.query_row(
                "SELECT
                    COUNT(*) AS total_runs,
                    COALESCE(SUM(CASE WHEN success != 0 THEN 1 ELSE 0 END), 0) AS success_runs
                 FROM task_run_logs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            Ok((total, success))
        }
    }

    fn insert_scheduled_task_dlq(
        &self,
        task_id: i64,
        chat_id: i64,
        started_at: &str,
        finished_at: &str,
        duration_ms: i64,
        error_summary: Option<&str>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let failed_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scheduled_task_dlq (
                task_id, chat_id, failed_at, started_at, finished_at, duration_ms, error_summary
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                task_id,
                chat_id,
                failed_at,
                started_at,
                finished_at,
                duration_ms,
                error_summary
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn list_scheduled_task_dlq(
        &self,
        chat_id: Option<i64>,
        task_id: Option<i64>,
        include_replayed: bool,
        limit: usize,
    ) -> Result<Vec<ScheduledTaskDlqEntry>, MchactError> {
        let conn = self.lock_conn();
        let replay_filter = if include_replayed {
            ""
        } else {
            " AND replayed_at IS NULL"
        };
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(ScheduledTaskDlqEntry {
                id: row.get(0)?,
                task_id: row.get(1)?,
                chat_id: row.get(2)?,
                failed_at: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                duration_ms: row.get(6)?,
                error_summary: row.get(7)?,
                replayed_at: row.get(8)?,
                replay_note: row.get(9)?,
            })
        };
        let query = match (chat_id, task_id) {
            (Some(_), Some(_)) => format!(
                "SELECT id, task_id, chat_id, failed_at, started_at, finished_at, duration_ms,
                        error_summary, replayed_at, replay_note
                 FROM scheduled_task_dlq
                 WHERE chat_id = ?1 AND task_id = ?2{replay_filter}
                 ORDER BY failed_at DESC LIMIT ?3"
            ),
            (Some(_), None) => format!(
                "SELECT id, task_id, chat_id, failed_at, started_at, finished_at, duration_ms,
                        error_summary, replayed_at, replay_note
                 FROM scheduled_task_dlq
                 WHERE chat_id = ?1{replay_filter}
                 ORDER BY failed_at DESC LIMIT ?2"
            ),
            (None, Some(_)) => format!(
                "SELECT id, task_id, chat_id, failed_at, started_at, finished_at, duration_ms,
                        error_summary, replayed_at, replay_note
                 FROM scheduled_task_dlq
                 WHERE task_id = ?1{replay_filter}
                 ORDER BY failed_at DESC LIMIT ?2"
            ),
            (None, None) => format!(
                "SELECT id, task_id, chat_id, failed_at, started_at, finished_at, duration_ms,
                        error_summary, replayed_at, replay_note
                 FROM scheduled_task_dlq
                 WHERE 1=1{replay_filter}
                 ORDER BY failed_at DESC LIMIT ?1"
            ),
        };
        let mut stmt = conn.prepare(&query)?;
        match (chat_id, task_id) {
            (Some(c), Some(t)) => stmt
                .query_map(params![c, t, limit as i64], mapper)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into),
            (Some(c), None) => stmt
                .query_map(params![c, limit as i64], mapper)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into),
            (None, Some(t)) => stmt
                .query_map(params![t, limit as i64], mapper)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into),
            (None, None) => stmt
                .query_map(params![limit as i64], mapper)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into),
        }
    }

    fn mark_scheduled_task_dlq_replayed(
        &self,
        dlq_id: i64,
        note: Option<&str>,
    ) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let replayed_at = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE scheduled_task_dlq
             SET replayed_at = ?1, replay_note = ?2
             WHERE id = ?3",
            params![replayed_at, note, dlq_id],
        )?;
        Ok(rows > 0)
    }
}

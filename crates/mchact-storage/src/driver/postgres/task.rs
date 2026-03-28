use mchact_core::error::MchactError;

use crate::db::types::{ScheduledTask, ScheduledTaskDlqEntry, TaskRunLog};
use crate::traits::TaskStore;

use super::PgDriver;

fn pg_err(e: tokio_postgres::Error) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn pool_err(e: deadpool_postgres::PoolError) -> MchactError {
    MchactError::ToolExecution(format!("pool: {e}"))
}

fn row_to_task(row: &tokio_postgres::Row) -> ScheduledTask {
    ScheduledTask {
        id: row.get("id"),
        chat_id: row.get("chat_id"),
        prompt: row.get("prompt"),
        schedule_type: row.get("schedule_type"),
        schedule_value: row.get("schedule_value"),
        timezone: row.get("timezone"),
        next_run: row.get("next_run"),
        last_run: row.get("last_run"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    }
}

impl TaskStore for PgDriver {
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
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let chat_id = chat_id;
        let prompt = prompt.to_string();
        let schedule_type = schedule_type.to_string();
        let schedule_value = schedule_value.to_string();
        let timezone = timezone.to_string();
        let next_run = next_run.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_one(
                    "INSERT INTO scheduled_tasks
                        (chat_id, prompt, schedule_type, schedule_value, timezone, next_run, status, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, 'active', $7)
                     RETURNING id",
                    &[
                        &chat_id,
                        &prompt,
                        &schedule_type,
                        &schedule_value,
                        &timezone,
                        &next_run,
                        &now,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_due_tasks(&self, now: &str) -> Result<Vec<ScheduledTask>, MchactError> {
        let pool = self.pool.clone();
        let now = now.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let rows = client
                .query(
                    "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone,
                            next_run, last_run, status, created_at
                     FROM scheduled_tasks
                     WHERE status = 'active' AND next_run <= $1",
                    &[&now],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_task).collect())
        })
    }

    fn claim_due_tasks(&self, now: &str, limit: usize) -> Result<Vec<ScheduledTask>, MchactError> {
        let pool = self.pool.clone();
        let now = now.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let mut client = pool.get().await.map_err(pool_err)?;
            let tx = client.transaction().await.map_err(pg_err)?;
            let rows = tx
                .query(
                    "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone,
                            next_run, last_run, status, created_at
                     FROM scheduled_tasks
                     WHERE status = 'active' AND next_run <= $1
                     ORDER BY next_run ASC, id ASC
                     LIMIT $2",
                    &[&now, &(limit as i64)],
                )
                .await
                .map_err(pg_err)?;
            let candidates: Vec<ScheduledTask> = rows.iter().map(row_to_task).collect();
            let mut claimed = Vec::new();
            for task in candidates {
                let n = tx
                    .execute(
                        "UPDATE scheduled_tasks
                         SET status = 'running'
                         WHERE id = $1 AND status = 'active' AND next_run <= $2",
                        &[&task.id, &now],
                    )
                    .await
                    .map_err(pg_err)?;
                if n > 0 {
                    let mut claimed_task = task;
                    claimed_task.status = "running".to_string();
                    claimed.push(claimed_task);
                }
            }
            tx.commit().await.map_err(pg_err)?;
            Ok(claimed)
        })
    }

    fn get_tasks_for_chat(&self, chat_id: i64) -> Result<Vec<ScheduledTask>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let rows = client
                .query(
                    "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone,
                            next_run, last_run, status, created_at
                     FROM scheduled_tasks
                     WHERE chat_id = $1 AND status IN ('active', 'paused')
                     ORDER BY id",
                    &[&chat_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(rows.iter().map(row_to_task).collect())
        })
    }

    fn get_task_by_id(&self, task_id: i64) -> Result<Option<ScheduledTask>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_opt(
                    "SELECT id, chat_id, prompt, schedule_type, schedule_value, timezone,
                            next_run, last_run, status, created_at
                     FROM scheduled_tasks
                     WHERE id = $1",
                    &[&task_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.as_ref().map(row_to_task))
        })
    }

    fn update_task_status(&self, task_id: i64, status: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let status = status.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE scheduled_tasks SET status = $1 WHERE id = $2",
                    &[&status, &task_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn requeue_scheduled_task(&self, task_id: i64, next_run: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let next_run = next_run.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE scheduled_tasks
                     SET status = 'active', next_run = $1
                     WHERE id = $2",
                    &[&next_run, &task_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn update_task_after_run(
        &self,
        task_id: i64,
        last_run: &str,
        next_run: Option<&str>,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let last_run = last_run.to_string();
        let next_run = next_run.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            match next_run {
                Some(next) => {
                    client
                        .execute(
                            "UPDATE scheduled_tasks
                             SET last_run = $1, next_run = $2, status = 'active'
                             WHERE id = $3",
                            &[&last_run, &next, &task_id],
                        )
                        .await
                        .map_err(pg_err)?;
                }
                None => {
                    client
                        .execute(
                            "UPDATE scheduled_tasks
                             SET last_run = $1, status = 'completed'
                             WHERE id = $2",
                            &[&last_run, &task_id],
                        )
                        .await
                        .map_err(pg_err)?;
                }
            }
            Ok(())
        })
    }

    fn recover_running_tasks(&self) -> Result<usize, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE scheduled_tasks SET status = 'active' WHERE status = 'running'",
                    &[],
                )
                .await
                .map_err(pg_err)?;
            Ok(n as usize)
        })
    }

    #[allow(dead_code)]
    fn delete_task(&self, task_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute("DELETE FROM scheduled_tasks WHERE id = $1", &[&task_id])
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
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
        let pool = self.pool.clone();
        let started_at = started_at.to_string();
        let finished_at = finished_at.to_string();
        let result_summary = result_summary.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_one(
                    "INSERT INTO task_run_logs
                        (task_id, chat_id, started_at, finished_at, duration_ms, success, result_summary)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     RETURNING id",
                    &[
                        &task_id,
                        &chat_id,
                        &started_at,
                        &finished_at,
                        &duration_ms,
                        &success,
                        &result_summary,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_task_run_logs(
        &self,
        task_id: i64,
        limit: usize,
    ) -> Result<Vec<TaskRunLog>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let rows = client
                .query(
                    "SELECT id, task_id, chat_id, started_at, finished_at, duration_ms, success, result_summary
                     FROM task_run_logs
                     WHERE task_id = $1
                     ORDER BY id DESC
                     LIMIT $2",
                    &[&task_id, &(limit as i64)],
                )
                .await
                .map_err(pg_err)?;
            let logs = rows
                .iter()
                .map(|row| TaskRunLog {
                    id: row.get("id"),
                    task_id: row.get("task_id"),
                    chat_id: row.get("chat_id"),
                    started_at: row.get("started_at"),
                    finished_at: row.get("finished_at"),
                    duration_ms: row.get("duration_ms"),
                    success: row.get("success"),
                    result_summary: row.get("result_summary"),
                })
                .collect();
            Ok(logs)
        })
    }

    fn get_task_run_summary_since(
        &self,
        since: Option<&str>,
    ) -> Result<(i64, i64), MchactError> {
        let pool = self.pool.clone();
        let since = since.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = if let Some(since_ts) = since {
                client
                    .query_one(
                        "SELECT
                            COUNT(*) AS total_runs,
                            COALESCE(SUM(CASE WHEN success THEN 1 ELSE 0 END), 0) AS success_runs
                         FROM task_run_logs
                         WHERE started_at >= $1",
                        &[&since_ts],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .query_one(
                        "SELECT
                            COUNT(*) AS total_runs,
                            COALESCE(SUM(CASE WHEN success THEN 1 ELSE 0 END), 0) AS success_runs
                         FROM task_run_logs",
                        &[],
                    )
                    .await
                    .map_err(pg_err)?
            };
            let total: i64 = row.get("total_runs");
            let success: i64 = row.get("success_runs");
            Ok((total, success))
        })
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
        let pool = self.pool.clone();
        let failed_at = chrono::Utc::now().to_rfc3339();
        let started_at = started_at.to_string();
        let finished_at = finished_at.to_string();
        let error_summary = error_summary.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_one(
                    "INSERT INTO scheduled_task_dlq
                        (task_id, chat_id, failed_at, started_at, finished_at, duration_ms, error_summary)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     RETURNING id",
                    &[
                        &task_id,
                        &chat_id,
                        &failed_at,
                        &started_at,
                        &finished_at,
                        &duration_ms,
                        &error_summary,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn list_scheduled_task_dlq(
        &self,
        chat_id: Option<i64>,
        task_id: Option<i64>,
        include_replayed: bool,
        limit: usize,
    ) -> Result<Vec<ScheduledTaskDlqEntry>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let replay_filter = if include_replayed {
                ""
            } else {
                " AND replayed_at IS NULL"
            };
            let select = "SELECT id, task_id, chat_id, failed_at, started_at, finished_at,
                                 duration_ms, error_summary, replayed_at, replay_note
                          FROM scheduled_task_dlq";
            let rows = match (chat_id, task_id) {
                (Some(c), Some(t)) => {
                    let q = format!(
                        "{select} WHERE chat_id = $1 AND task_id = $2{replay_filter}
                         ORDER BY failed_at DESC LIMIT $3"
                    );
                    client.query(&q as &str, &[&c, &t, &(limit as i64)]).await.map_err(pg_err)?
                }
                (Some(c), None) => {
                    let q = format!(
                        "{select} WHERE chat_id = $1{replay_filter}
                         ORDER BY failed_at DESC LIMIT $2"
                    );
                    client.query(&q as &str, &[&c, &(limit as i64)]).await.map_err(pg_err)?
                }
                (None, Some(t)) => {
                    let q = format!(
                        "{select} WHERE task_id = $1{replay_filter}
                         ORDER BY failed_at DESC LIMIT $2"
                    );
                    client.query(&q as &str, &[&t, &(limit as i64)]).await.map_err(pg_err)?
                }
                (None, None) => {
                    let q = format!(
                        "{select} WHERE 1=1{replay_filter}
                         ORDER BY failed_at DESC LIMIT $1"
                    );
                    client.query(&q as &str, &[&(limit as i64)]).await.map_err(pg_err)?
                }
            };
            let entries = rows
                .iter()
                .map(|row| ScheduledTaskDlqEntry {
                    id: row.get("id"),
                    task_id: row.get("task_id"),
                    chat_id: row.get("chat_id"),
                    failed_at: row.get("failed_at"),
                    started_at: row.get("started_at"),
                    finished_at: row.get("finished_at"),
                    duration_ms: row.get("duration_ms"),
                    error_summary: row.get("error_summary"),
                    replayed_at: row.get("replayed_at"),
                    replay_note: row.get("replay_note"),
                })
                .collect();
            Ok(entries)
        })
    }

    fn mark_scheduled_task_dlq_replayed(
        &self,
        dlq_id: i64,
        note: Option<&str>,
    ) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let replayed_at = chrono::Utc::now().to_rfc3339();
        let note = note.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE scheduled_task_dlq
                     SET replayed_at = $1, replay_note = $2
                     WHERE id = $3",
                    &[&replayed_at, &note, &dlq_id],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }
}

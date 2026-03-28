use mchact_core::error::MchactError;

use crate::db::types::{ScheduledTask, ScheduledTaskDlqEntry, TaskRunLog};
use crate::traits::TaskStore;

use super::{not_impl, PgDriver};

impl TaskStore for PgDriver {
    fn create_scheduled_task(
        &self,
        _chat_id: i64,
        _prompt: &str,
        _schedule_type: &str,
        _schedule_value: &str,
        _next_run: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn create_scheduled_task_with_timezone(
        &self,
        _chat_id: i64,
        _prompt: &str,
        _schedule_type: &str,
        _schedule_value: &str,
        _timezone: &str,
        _next_run: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_due_tasks(&self, _now: &str) -> Result<Vec<ScheduledTask>, MchactError> {
        Err(not_impl())
    }

    fn claim_due_tasks(&self, _now: &str, _limit: usize) -> Result<Vec<ScheduledTask>, MchactError> {
        Err(not_impl())
    }

    fn get_tasks_for_chat(&self, _chat_id: i64) -> Result<Vec<ScheduledTask>, MchactError> {
        Err(not_impl())
    }

    fn get_task_by_id(&self, _task_id: i64) -> Result<Option<ScheduledTask>, MchactError> {
        Err(not_impl())
    }

    fn update_task_status(&self, _task_id: i64, _status: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn requeue_scheduled_task(&self, _task_id: i64, _next_run: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn update_task_after_run(
        &self,
        _task_id: i64,
        _last_run: &str,
        _next_run: Option<&str>,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn recover_running_tasks(&self) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn delete_task(&self, _task_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn log_task_run(
        &self,
        _task_id: i64,
        _chat_id: i64,
        _started_at: &str,
        _finished_at: &str,
        _duration_ms: i64,
        _success: bool,
        _result_summary: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_task_run_logs(
        &self,
        _task_id: i64,
        _limit: usize,
    ) -> Result<Vec<TaskRunLog>, MchactError> {
        Err(not_impl())
    }

    fn get_task_run_summary_since(
        &self,
        _since: Option<&str>,
    ) -> Result<(i64, i64), MchactError> {
        Err(not_impl())
    }

    fn insert_scheduled_task_dlq(
        &self,
        _task_id: i64,
        _chat_id: i64,
        _started_at: &str,
        _finished_at: &str,
        _duration_ms: i64,
        _error_summary: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn list_scheduled_task_dlq(
        &self,
        _chat_id: Option<i64>,
        _task_id: Option<i64>,
        _include_replayed: bool,
        _limit: usize,
    ) -> Result<Vec<ScheduledTaskDlqEntry>, MchactError> {
        Err(not_impl())
    }

    fn mark_scheduled_task_dlq_replayed(
        &self,
        _dlq_id: i64,
        _note: Option<&str>,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }
}

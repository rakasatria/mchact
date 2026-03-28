use mchact_core::error::MchactError;

use crate::db::types::{ScheduledTask, ScheduledTaskDlqEntry, TaskRunLog};

pub trait TaskStore {
    fn create_scheduled_task(
        &self,
        chat_id: i64,
        prompt: &str,
        schedule_type: &str,
        schedule_value: &str,
        next_run: &str,
    ) -> Result<i64, MchactError>;

    fn create_scheduled_task_with_timezone(
        &self,
        chat_id: i64,
        prompt: &str,
        schedule_type: &str,
        schedule_value: &str,
        timezone: &str,
        next_run: &str,
    ) -> Result<i64, MchactError>;

    fn get_due_tasks(&self, now: &str) -> Result<Vec<ScheduledTask>, MchactError>;

    fn claim_due_tasks(
        &self,
        now: &str,
        limit: usize,
    ) -> Result<Vec<ScheduledTask>, MchactError>;

    fn get_tasks_for_chat(&self, chat_id: i64) -> Result<Vec<ScheduledTask>, MchactError>;

    fn get_task_by_id(&self, task_id: i64) -> Result<Option<ScheduledTask>, MchactError>;

    fn update_task_status(&self, task_id: i64, status: &str) -> Result<bool, MchactError>;

    fn requeue_scheduled_task(
        &self,
        task_id: i64,
        next_run: &str,
    ) -> Result<bool, MchactError>;

    fn update_task_after_run(
        &self,
        task_id: i64,
        last_run: &str,
        next_run: Option<&str>,
    ) -> Result<(), MchactError>;

    fn recover_running_tasks(&self) -> Result<usize, MchactError>;

    fn delete_task(&self, task_id: i64) -> Result<bool, MchactError>;

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
    ) -> Result<i64, MchactError>;

    fn get_task_run_logs(
        &self,
        task_id: i64,
        limit: usize,
    ) -> Result<Vec<TaskRunLog>, MchactError>;

    fn get_task_run_summary_since(
        &self,
        since: Option<&str>,
    ) -> Result<(i64, i64), MchactError>;

    fn insert_scheduled_task_dlq(
        &self,
        task_id: i64,
        chat_id: i64,
        started_at: &str,
        finished_at: &str,
        duration_ms: i64,
        error_summary: Option<&str>,
    ) -> Result<i64, MchactError>;

    fn list_scheduled_task_dlq(
        &self,
        chat_id: Option<i64>,
        task_id: Option<i64>,
        include_replayed: bool,
        limit: usize,
    ) -> Result<Vec<ScheduledTaskDlqEntry>, MchactError>;

    fn mark_scheduled_task_dlq_replayed(
        &self,
        dlq_id: i64,
        note: Option<&str>,
    ) -> Result<bool, MchactError>;
}

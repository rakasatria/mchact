use mchact_core::error::MchactError;

use crate::db::types::{
    CreateSubagentRunParams, Finding, FinishSubagentRunParams, SubagentAnnounceRecord,
    SubagentEventRecord, SubagentObservabilitySnapshot, SubagentRunRecord,
};
use crate::traits::SubagentStore;

use super::{not_impl, PgDriver};

impl SubagentStore for PgDriver {
    fn create_subagent_run(
        &self,
        _params: CreateSubagentRunParams<'_>,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn mark_subagent_queued(&self, _run_id: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn mark_subagent_running(&self, _run_id: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn mark_subagent_finished(
        &self,
        _params: FinishSubagentRunParams<'_>,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn is_subagent_cancel_requested(&self, _run_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn request_subagent_cancel(
        &self,
        _run_id: &str,
        _chat_id: i64,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn list_subagent_runs(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<SubagentRunRecord>, MchactError> {
        Err(not_impl())
    }

    fn get_subagent_run(
        &self,
        _run_id: &str,
        _chat_id: i64,
    ) -> Result<Option<SubagentRunRecord>, MchactError> {
        Err(not_impl())
    }

    fn count_active_subagent_runs_for_chat(&self, _chat_id: i64) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn count_active_subagent_children(&self, _parent_run_id: &str) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn enqueue_subagent_announce(
        &self,
        _run_id: &str,
        _chat_id: i64,
        _caller_channel: &str,
        _payload_text: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn list_due_subagent_announces(
        &self,
        _now_iso: &str,
        _limit: usize,
    ) -> Result<Vec<SubagentAnnounceRecord>, MchactError> {
        Err(not_impl())
    }

    fn mark_subagent_announce_sent(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn mark_subagent_announce_retry(
        &self,
        _id: i64,
        _attempts: i64,
        _next_attempt_at: Option<&str>,
        _last_error: &str,
        _terminal_fail: bool,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn append_subagent_event(
        &self,
        _run_id: &str,
        _event_type: &str,
        _detail: Option<&str>,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn list_subagent_events(
        &self,
        _run_id: &str,
        _limit: usize,
    ) -> Result<Vec<SubagentEventRecord>, MchactError> {
        Err(not_impl())
    }

    fn set_subagent_focus(&self, _chat_id: i64, _run_id: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn clear_subagent_focus(&self, _chat_id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_subagent_focus(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_subagent_observability_snapshot(
        &self,
        _chat_id: Option<i64>,
        _recent_limit: usize,
    ) -> Result<SubagentObservabilitySnapshot, MchactError> {
        Err(not_impl())
    }

    fn insert_finding(
        &self,
        _orchestration_id: &str,
        _run_id: &str,
        _finding: &str,
        _category: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_findings(&self, _orchestration_id: &str) -> Result<Vec<Finding>, MchactError> {
        Err(not_impl())
    }

    fn delete_findings(&self, _orchestration_id: &str) -> Result<usize, MchactError> {
        Err(not_impl())
    }
}

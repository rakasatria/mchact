use mchact_core::error::MchactError;

use crate::db::types::{
    CreateSubagentRunParams, Finding, FinishSubagentRunParams, SubagentAnnounceRecord,
    SubagentEventRecord, SubagentObservabilitySnapshot, SubagentRunRecord,
};

pub trait SubagentStore {
    fn create_subagent_run(
        &self,
        params: CreateSubagentRunParams<'_>,
    ) -> Result<(), MchactError>;

    fn mark_subagent_queued(&self, run_id: &str) -> Result<(), MchactError>;

    fn mark_subagent_running(&self, run_id: &str) -> Result<(), MchactError>;

    fn mark_subagent_finished(
        &self,
        params: FinishSubagentRunParams<'_>,
    ) -> Result<(), MchactError>;

    fn is_subagent_cancel_requested(&self, run_id: &str) -> Result<bool, MchactError>;

    fn request_subagent_cancel(
        &self,
        run_id: &str,
        chat_id: i64,
    ) -> Result<bool, MchactError>;

    fn list_subagent_runs(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<SubagentRunRecord>, MchactError>;

    fn get_subagent_run(
        &self,
        run_id: &str,
        chat_id: i64,
    ) -> Result<Option<SubagentRunRecord>, MchactError>;

    fn count_active_subagent_runs_for_chat(&self, chat_id: i64) -> Result<i64, MchactError>;

    fn count_active_subagent_children(
        &self,
        parent_run_id: &str,
    ) -> Result<i64, MchactError>;

    fn enqueue_subagent_announce(
        &self,
        run_id: &str,
        chat_id: i64,
        caller_channel: &str,
        payload_text: &str,
    ) -> Result<(), MchactError>;

    fn list_due_subagent_announces(
        &self,
        now_iso: &str,
        limit: usize,
    ) -> Result<Vec<SubagentAnnounceRecord>, MchactError>;

    fn mark_subagent_announce_sent(&self, id: i64) -> Result<(), MchactError>;

    fn mark_subagent_announce_retry(
        &self,
        id: i64,
        attempts: i64,
        next_attempt_at: Option<&str>,
        last_error: &str,
        terminal_fail: bool,
    ) -> Result<(), MchactError>;

    fn append_subagent_event(
        &self,
        run_id: &str,
        event_type: &str,
        detail: Option<&str>,
    ) -> Result<(), MchactError>;

    fn list_subagent_events(
        &self,
        run_id: &str,
        limit: usize,
    ) -> Result<Vec<SubagentEventRecord>, MchactError>;

    fn set_subagent_focus(&self, chat_id: i64, run_id: &str) -> Result<(), MchactError>;

    fn clear_subagent_focus(&self, chat_id: i64) -> Result<(), MchactError>;

    fn get_subagent_focus(&self, chat_id: i64) -> Result<Option<String>, MchactError>;

    fn get_subagent_observability_snapshot(
        &self,
        chat_id: Option<i64>,
        recent_limit: usize,
    ) -> Result<SubagentObservabilitySnapshot, MchactError>;

    fn insert_finding(
        &self,
        orchestration_id: &str,
        run_id: &str,
        finding: &str,
        category: &str,
    ) -> Result<i64, MchactError>;

    fn get_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<Vec<Finding>, MchactError>;

    fn delete_findings(
        &self,
        orchestration_id: &str,
    ) -> Result<usize, MchactError>;
}

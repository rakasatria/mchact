use deadpool_postgres::{Config as PoolConfig, Pool, Runtime};
use mchact_core::error::MchactError;
use tokio_postgres::NoTls;

use crate::db::types::{
    AuthApiKeyRecord, AuditLogRecord, ChatSummary, CreateSubagentRunParams, DocumentChunk,
    DocumentExtraction, Finding, FinishSubagentRunParams, FtsSearchResult, Knowledge,
    LlmModelUsageSummary, LlmUsageSummary, MediaObject, Memory, MemoryInjectionLog,
    MemoryObservabilitySummary, MemoryReflectorRun, MetricsHistoryPoint, ScheduledTask,
    ScheduledTaskDlqEntry, SessionMetaRow, SessionSettings, SessionTreeRow, StoredMessage,
    SubagentAnnounceRecord, SubagentEventRecord, SubagentObservabilitySnapshot, SubagentRunRecord,
    TaskRunLog,
};
use crate::traits::{
    AuthStore, AuditStore, ChatStore, DataStore, DocumentStore, KnowledgeStore, MediaObjectStore,
    MemoryDbStore, MessageStore, MetricsStore, SessionStore, SubagentStore, TaskStore,
};

const SCHEMA_SQL: &str = include_str!("../schema/postgres.sql");

fn not_impl() -> MchactError {
    MchactError::ToolExecution("postgres: not yet implemented".into())
}

pub struct PgDriver {
    #[allow(dead_code)]
    pool: Pool,
}

impl PgDriver {
    pub async fn connect(database_url: &str) -> Result<Self, MchactError> {
        let mut cfg = PoolConfig::new();
        cfg.url = Some(database_url.to_string());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| MchactError::Config(format!("pool creation failed: {e}")))?;

        // Run schema initialization
        let client = pool
            .get()
            .await
            .map_err(|e| MchactError::Config(format!("connect failed: {e}")))?;
        for stmt in SCHEMA_SQL.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            client
                .execute(trimmed, &[])
                .await
                .map_err(|e| MchactError::Config(format!("schema init failed: {e}")))?;
        }

        Ok(Self { pool })
    }
}

impl DataStore for PgDriver {}

// ---------------------------------------------------------------------------
// ChatStore
// ---------------------------------------------------------------------------

impl ChatStore for PgDriver {
    fn upsert_chat(
        &self,
        _chat_id: i64,
        _chat_title: Option<&str>,
        _chat_type: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn resolve_or_create_chat_id(
        &self,
        _channel: &str,
        _external_chat_id: &str,
        _chat_title: Option<&str>,
        _chat_type: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_chat_type(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_chat_id_by_channel_and_title(
        &self,
        _channel: &str,
        _chat_title: &str,
    ) -> Result<Option<i64>, MchactError> {
        Err(not_impl())
    }

    fn get_chat_channel(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_chat_external_id(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_recent_chats(&self, _limit: usize) -> Result<Vec<ChatSummary>, MchactError> {
        Err(not_impl())
    }

    fn get_chats_by_type(
        &self,
        _chat_type: &str,
        _limit: usize,
    ) -> Result<Vec<ChatSummary>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// MessageStore
// ---------------------------------------------------------------------------

impl MessageStore for PgDriver {
    fn store_message(&self, _msg: &StoredMessage) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn store_message_if_new(&self, _msg: &StoredMessage) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn message_exists(&self, _chat_id: i64, _message_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn search_messages_fts(
        &self,
        _query: &str,
        _chat_id: Option<i64>,
        _limit: usize,
    ) -> Result<Vec<FtsSearchResult>, MchactError> {
        Err(not_impl())
    }

    fn get_message_context(
        &self,
        _chat_id: i64,
        _timestamp: &str,
        _window: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn rebuild_fts_index(&self) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_recent_messages(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_all_messages(&self, _chat_id: i64) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_messages_since_last_bot_response(
        &self,
        _chat_id: i64,
        _max: usize,
        _fallback: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_new_user_messages_since(
        &self,
        _chat_id: i64,
        _since: &str,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_messages_since(
        &self,
        _chat_id: i64,
        _since: &str,
        _limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// SessionStore
// ---------------------------------------------------------------------------

impl SessionStore for PgDriver {
    fn save_session(&self, _chat_id: i64, _messages_json: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn save_session_with_meta(
        &self,
        _chat_id: i64,
        _messages_json: &str,
        _parent_session_key: Option<&str>,
        _fork_point: Option<i64>,
        _skill_envs_json: Option<&str>,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn save_session_skill_envs(
        &self,
        _chat_id: i64,
        _skill_envs_json: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn load_session(&self, _chat_id: i64) -> Result<Option<(String, String)>, MchactError> {
        Err(not_impl())
    }

    fn load_session_skill_envs(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn save_session_settings(
        &self,
        _chat_id: i64,
        _settings: &SessionSettings,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn load_session_settings(
        &self,
        _chat_id: i64,
    ) -> Result<Option<SessionSettings>, MchactError> {
        Err(not_impl())
    }

    fn load_session_meta(&self, _chat_id: i64) -> Result<Option<SessionMetaRow>, MchactError> {
        Err(not_impl())
    }

    fn list_session_meta(&self, _limit: usize) -> Result<Vec<SessionTreeRow>, MchactError> {
        Err(not_impl())
    }

    fn delete_session(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn clear_chat_context(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn clear_chat_conversation(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn clear_chat_memory(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn delete_chat_data(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// TaskStore
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// MemoryDbStore
// ---------------------------------------------------------------------------

impl MemoryDbStore for PgDriver {
    fn insert_memory(
        &self,
        _chat_id: Option<i64>,
        _content: &str,
        _category: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn insert_memory_with_metadata(
        &self,
        _chat_id: Option<i64>,
        _content: &str,
        _category: &str,
        _source: &str,
        _confidence: f64,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_memory_by_id(&self, _id: i64) -> Result<Option<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_memories_for_context(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_all_memories_for_chat(
        &self,
        _chat_id: Option<i64>,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_active_chat_ids_since(&self, _since: &str) -> Result<Vec<i64>, MchactError> {
        Err(not_impl())
    }

    fn delete_memory(&self, _id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn search_memories(
        &self,
        _chat_id: i64,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn search_memories_with_options(
        &self,
        _chat_id: i64,
        _query: &str,
        _limit: usize,
        _include_archived: bool,
        _broad_recall: bool,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn update_memory_content(
        &self,
        _id: i64,
        _content: &str,
        _category: &str,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn update_memory_with_metadata(
        &self,
        _id: i64,
        _content: &str,
        _category: &str,
        _confidence: f64,
        _source: &str,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn update_memory_embedding_model(&self, _id: i64, _model: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn touch_memory_last_seen(
        &self,
        _id: i64,
        _confidence_floor: Option<f64>,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn archive_memory(&self, _id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn archive_stale_memories(&self, _stale_days: i64) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn supersede_memory(
        &self,
        _from_memory_id: i64,
        _new_content: &str,
        _category: &str,
        _source: &str,
        _confidence: f64,
        _reason: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_memories_without_embedding(
        &self,
        _chat_id: Option<i64>,
        _limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_all_active_memories(&self) -> Result<Vec<(i64, String)>, MchactError> {
        Err(not_impl())
    }

    fn get_reflector_cursor(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn set_reflector_cursor(&self, _chat_id: i64, _last_reflected_ts: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn log_reflector_run(
        &self,
        _chat_id: i64,
        _started_at: &str,
        _finished_at: &str,
        _extracted_count: usize,
        _inserted_count: usize,
        _updated_count: usize,
        _skipped_count: usize,
        _dedup_method: &str,
        _parse_ok: bool,
        _error_text: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn log_memory_injection(
        &self,
        _chat_id: i64,
        _retrieval_method: &str,
        _candidate_count: usize,
        _selected_count: usize,
        _omitted_count: usize,
        _tokens_est: usize,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_memory_observability_summary(
        &self,
        _chat_id: Option<i64>,
    ) -> Result<MemoryObservabilitySummary, MchactError> {
        Err(not_impl())
    }

    fn get_memory_reflector_runs(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<MemoryReflectorRun>, MchactError> {
        Err(not_impl())
    }

    fn get_memory_injection_logs(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<MemoryInjectionLog>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// AuthStore
// ---------------------------------------------------------------------------

impl AuthStore for PgDriver {
    fn upsert_auth_password_hash(&self, _password_hash: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_auth_password_hash(&self) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn clear_auth_password_hash(&self) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn create_auth_session(
        &self,
        _session_id: &str,
        _label: Option<&str>,
        _expires_at: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn validate_auth_session(&self, _session_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn revoke_auth_session(&self, _session_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn revoke_all_auth_sessions(&self) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn create_api_key(
        &self,
        _label: &str,
        _key_hash: &str,
        _prefix: &str,
        _scopes: &[String],
        _expires_at: Option<&str>,
        _rotated_from_key_id: Option<i64>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn list_api_keys(&self) -> Result<Vec<AuthApiKeyRecord>, MchactError> {
        Err(not_impl())
    }

    fn rotate_api_key_revoke_old(&self, _old_key_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn revoke_api_key(&self, _key_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn validate_api_key_hash(
        &self,
        _key_hash: &str,
    ) -> Result<Option<(i64, Vec<String>)>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// AuditStore
// ---------------------------------------------------------------------------

impl AuditStore for PgDriver {
    fn log_audit_event(
        &self,
        _kind: &str,
        _actor: &str,
        _action: &str,
        _target: Option<&str>,
        _status: &str,
        _detail: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn list_audit_logs(
        &self,
        _kind: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<AuditLogRecord>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// MetricsStore
// ---------------------------------------------------------------------------

impl MetricsStore for PgDriver {
    fn upsert_metrics_history(&self, _point: &MetricsHistoryPoint) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_metrics_history(
        &self,
        _since_ts_ms: i64,
        _limit: usize,
    ) -> Result<Vec<MetricsHistoryPoint>, MchactError> {
        Err(not_impl())
    }

    fn cleanup_metrics_history_before(&self, _before_ts_ms: i64) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn log_llm_usage(
        &self,
        _chat_id: i64,
        _caller_channel: &str,
        _provider: &str,
        _model: &str,
        _input_tokens: i64,
        _output_tokens: i64,
        _request_kind: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_llm_usage_summary(
        &self,
        _chat_id: Option<i64>,
    ) -> Result<LlmUsageSummary, MchactError> {
        Err(not_impl())
    }

    fn get_llm_usage_summary_since(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
    ) -> Result<LlmUsageSummary, MchactError> {
        Err(not_impl())
    }

    fn get_llm_usage_by_model(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
        _limit: Option<usize>,
    ) -> Result<Vec<LlmModelUsageSummary>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// SubagentStore
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// DocumentStore
// ---------------------------------------------------------------------------

impl DocumentStore for PgDriver {
    fn insert_document_extraction(
        &self,
        _chat_id: i64,
        _file_hash: &str,
        _filename: &str,
        _mime_type: Option<&str>,
        _file_size: i64,
        _extracted_text: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_document_extraction(
        &self,
        _chat_id: i64,
        _file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn search_document_extractions(
        &self,
        _chat_id: Option<i64>,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn list_document_extractions(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn get_document_extraction_by_id(
        &self,
        _id: i64,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn set_document_extraction_media_id(
        &self,
        _extraction_id: i64,
        _media_object_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_document_extraction_id_by_media_object_id(
        &self,
        _media_object_id: i64,
    ) -> Result<Option<i64>, MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// MediaObjectStore
// ---------------------------------------------------------------------------

impl MediaObjectStore for PgDriver {
    fn insert_media_object(
        &self,
        _key: &str,
        _backend: &str,
        _chat_id: i64,
        _mime_type: Option<&str>,
        _size_bytes: Option<i64>,
        _hash: Option<&str>,
        _source: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_media_object(&self, _id: i64) -> Result<Option<MediaObject>, MchactError> {
        Err(not_impl())
    }

    fn get_media_object_by_hash(&self, _hash: &str) -> Result<Option<MediaObject>, MchactError> {
        Err(not_impl())
    }

    fn list_media_objects_for_chat(
        &self,
        _chat_id: i64,
    ) -> Result<Vec<MediaObject>, MchactError> {
        Err(not_impl())
    }

    fn delete_media_object(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }
}

// ---------------------------------------------------------------------------
// KnowledgeStore
// ---------------------------------------------------------------------------

impl KnowledgeStore for PgDriver {
    fn create_knowledge(
        &self,
        _name: &str,
        _description: &str,
        _owner_chat_id: i64,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_knowledge_by_name(&self, _name: &str) -> Result<Option<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn list_knowledge(&self) -> Result<Vec<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn delete_knowledge(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn update_knowledge_timestamp(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn update_knowledge_grouping_check(
        &self,
        _knowledge_id: i64,
        _doc_count: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_knowledge_needing_grouping(
        &self,
        _min_docs: i64,
    ) -> Result<Vec<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn add_document_to_knowledge(
        &self,
        _knowledge_id: i64,
        _doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn remove_document_from_knowledge(
        &self,
        _knowledge_id: i64,
        _doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn list_knowledge_documents(&self, _knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        Err(not_impl())
    }

    fn count_knowledge_documents(&self, _knowledge_id: i64) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn add_knowledge_chat_access(
        &self,
        _knowledge_id: i64,
        _chat_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn has_knowledge_chat_access(
        &self,
        _knowledge_id: i64,
        _chat_id: i64,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn list_knowledge_for_chat(&self, _chat_id: i64) -> Result<Vec<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn list_knowledge_chat_ids(&self, _knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        Err(not_impl())
    }

    fn insert_document_chunk(
        &self,
        _doc_extraction_id: i64,
        _page_number: i64,
        _text: &str,
        _token_count: Option<i64>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_chunks_by_status(
        &self,
        _embedding_status: &str,
        _limit: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        Err(not_impl())
    }

    fn get_chunks_for_observation(&self, _limit: i64) -> Result<Vec<DocumentChunk>, MchactError> {
        Err(not_impl())
    }

    fn update_chunk_embedding(
        &self,
        _chunk_id: i64,
        _embedding_bytes: &[u8],
        _status: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn update_chunk_observation_status(
        &self,
        _chunk_id: i64,
        _status: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_chunks_for_document(
        &self,
        _doc_extraction_id: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        Err(not_impl())
    }

    fn reset_failed_chunks(&self, _older_than_mins: i64) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_knowledge_chunk_stats(
        &self,
        _knowledge_id: i64,
    ) -> Result<(i64, i64, i64, i64, i64, i64), MchactError> {
        Err(not_impl())
    }
}

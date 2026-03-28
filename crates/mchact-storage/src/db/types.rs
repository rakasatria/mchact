#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub chat_id: i64,
    pub sender_name: String,
    pub content: String,
    pub is_from_bot: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct ChatSummary {
    pub chat_id: i64,
    pub chat_title: Option<String>,
    pub session_label: Option<String>,
    pub chat_type: String,
    pub last_message_time: String,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionSettings {
    pub label: Option<String>,
    pub thinking_level: Option<String>,
    pub verbose_level: Option<String>,
    pub reasoning_level: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TaskRunLog {
    pub id: i64,
    pub task_id: i64,
    pub chat_id: i64,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
    pub success: bool,
    pub result_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LlmUsageSummary {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub last_request_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LlmModelUsageSummary {
    pub model: String,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone)]
pub struct Memory {
    pub id: i64,
    pub chat_id: Option<i64>,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
    pub embedding_model: Option<String>,
    pub confidence: f64,
    pub source: String,
    pub last_seen_at: String,
    pub is_archived: bool,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryObservabilitySummary {
    pub total: i64,
    pub active: i64,
    pub archived: i64,
    pub low_confidence: i64,
    pub avg_confidence: f64,
    pub reflector_runs_24h: i64,
    pub reflector_inserted_24h: i64,
    pub reflector_updated_24h: i64,
    pub reflector_skipped_24h: i64,
    pub injection_events_24h: i64,
    pub injection_selected_24h: i64,
    pub injection_candidates_24h: i64,
}

#[derive(Debug, Clone)]
pub struct MemoryReflectorRun {
    pub id: i64,
    pub chat_id: i64,
    pub started_at: String,
    pub finished_at: String,
    pub extracted_count: i64,
    pub inserted_count: i64,
    pub updated_count: i64,
    pub skipped_count: i64,
    pub dedup_method: String,
    pub parse_ok: bool,
    pub error_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryInjectionLog {
    pub id: i64,
    pub chat_id: i64,
    pub created_at: String,
    pub retrieval_method: String,
    pub candidate_count: i64,
    pub selected_count: i64,
    pub omitted_count: i64,
    pub tokens_est: i64,
}

#[derive(Debug, Clone)]
pub struct AuthApiKeyRecord {
    pub id: i64,
    pub label: String,
    pub prefix: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub rotated_from_key_id: Option<i64>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MetricsHistoryPoint {
    pub timestamp_ms: i64,
    pub llm_completions: i64,
    pub llm_input_tokens: i64,
    pub llm_output_tokens: i64,
    pub http_requests: i64,
    pub tool_executions: i64,
    pub mcp_calls: i64,
    pub mcp_rate_limited_rejections: i64,
    pub mcp_bulkhead_rejections: i64,
    pub mcp_circuit_open_rejections: i64,
    pub active_sessions: i64,
}

#[derive(Debug, Clone)]
pub struct AuditLogRecord {
    pub id: i64,
    pub kind: String,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub status: String,
    pub detail: Option<String>,
    pub created_at: String,
}

pub type SessionMetaRow = (String, String, Option<String>, Option<i64>);
pub type SessionTreeRow = (i64, Option<String>, Option<i64>, String);

#[derive(Debug, Clone)]
pub struct FtsSearchResult {
    pub message_id: String,
    pub chat_id: i64,
    pub chat_title: Option<String>,
    pub sender_name: String,
    pub content_snippet: String,
    pub timestamp: String,
    pub rank: f64,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub id: i64,
    pub orchestration_id: String,
    pub run_id: String,
    pub finding: String,
    pub category: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DocumentExtraction {
    pub id: i64,
    pub chat_id: i64,
    pub file_hash: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub file_size: i64,
    pub extracted_text: String,
    pub char_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Knowledge {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub owner_chat_id: i64,
    pub last_grouping_check_at: Option<String>,
    pub document_count_at_last_check: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentChunk {
    pub id: i64,
    pub document_extraction_id: i64,
    pub page_number: i64,
    pub text: String,
    pub token_count: Option<i64>,
    pub embedding: Option<Vec<u8>>,
    pub embedding_status: String,
    pub observation_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaObject {
    pub id: i64,
    pub object_key: String,
    pub storage_backend: String,
    pub original_chat_id: i64,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub sha256_hash: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScheduledTask {
    pub id: i64,
    pub chat_id: i64,
    pub prompt: String,
    pub schedule_type: String,  // "cron" or "once"
    pub schedule_value: String, // cron expression or ISO timestamp
    pub timezone: String,       // IANA timezone; empty means "use app default"
    pub next_run: String,       // ISO timestamp
    pub last_run: Option<String>,
    pub status: String, // "active", "paused", "completed", "cancelled"
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ScheduledTaskDlqEntry {
    pub id: i64,
    pub task_id: i64,
    pub chat_id: i64,
    pub failed_at: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
    pub error_summary: Option<String>,
    pub replayed_at: Option<String>,
    pub replay_note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubagentRunRecord {
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub depth: i64,
    pub chat_id: i64,
    pub caller_channel: String,
    pub task: String,
    pub context: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub cancel_requested: bool,
    pub error_text: Option<String>,
    pub result_text: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub provider: String,
    pub model: String,
    pub token_budget: i64,
    pub artifact_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubagentAnnounceRecord {
    pub id: i64,
    pub run_id: String,
    pub chat_id: i64,
    pub caller_channel: String,
    pub payload_text: String,
    pub status: String,
    pub attempts: i64,
    pub next_attempt_at: Option<String>,
    pub last_error: Option<String>,
}

pub struct CreateSubagentRunParams<'a> {
    pub run_id: &'a str,
    pub parent_run_id: Option<&'a str>,
    pub depth: i64,
    pub token_budget: i64,
    pub chat_id: i64,
    pub caller_channel: &'a str,
    pub task: &'a str,
    pub context: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
}

pub struct FinishSubagentRunParams<'a> {
    pub run_id: &'a str,
    pub status: &'a str,
    pub error_text: Option<&'a str>,
    pub result_text: Option<&'a str>,
    pub artifact_json: Option<&'a str>,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

#[derive(Debug, Clone)]
pub struct SubagentObservabilitySnapshot {
    pub active_runs: i64,
    pub queued_runs: i64,
    pub running_runs: i64,
    pub pending_announces: i64,
    pub retry_announces: i64,
    pub failed_announces: i64,
    pub completed_24h: i64,
    pub failed_24h: i64,
    pub budget_exceeded_24h: i64,
    pub avg_duration_ms_24h: i64,
    pub recent_runs: Vec<SubagentRunRecord>,
}

#[derive(Debug, Clone)]
pub struct SubagentEventRecord {
    pub id: i64,
    pub run_id: String,
    pub event_type: String,
    pub detail: Option<String>,
    pub created_at: String,
}

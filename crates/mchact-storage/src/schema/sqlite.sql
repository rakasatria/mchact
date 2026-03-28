-- mchact SQLite target schema (schema version 23)
-- This represents the fully-migrated state after all migrations have been applied.
-- Generated from crates/mchact-storage/src/db/mod.rs

PRAGMA journal_mode=WAL;

CREATE TABLE IF NOT EXISTS chats (
    chat_id INTEGER PRIMARY KEY,
    chat_title TEXT,
    chat_type TEXT NOT NULL DEFAULT 'private',
    last_message_time TEXT NOT NULL,
    channel TEXT,
    external_chat_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_chats_channel_external
    ON chats(channel, external_chat_id);

CREATE INDEX IF NOT EXISTS idx_chats_channel_title
    ON chats(channel, chat_title);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT NOT NULL,
    chat_id INTEGER NOT NULL,
    sender_name TEXT NOT NULL,
    content TEXT NOT NULL,
    is_from_bot INTEGER NOT NULL DEFAULT 0,
    timestamp TEXT NOT NULL,
    PRIMARY KEY (id, chat_id)
);

CREATE INDEX IF NOT EXISTS idx_messages_chat_timestamp
    ON messages(chat_id, timestamp);

-- FTS5 virtual table for full-text search on messages
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    sender_name,
    content,
    content='messages',
    content_rowid='rowid'
);

CREATE TRIGGER IF NOT EXISTS messages_fts_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, sender_name, content)
    VALUES (new.rowid, new.sender_name, new.content);
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, sender_name, content)
    VALUES ('delete', old.rowid, old.sender_name, old.content);
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, sender_name, content)
    VALUES ('delete', old.rowid, old.sender_name, old.content);
    INSERT INTO messages_fts(rowid, sender_name, content)
    VALUES (new.rowid, new.sender_name, new.content);
END;

CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    prompt TEXT NOT NULL,
    schedule_type TEXT NOT NULL DEFAULT 'cron',
    schedule_value TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT '',
    next_run TEXT NOT NULL,
    last_run TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_status_next
    ON scheduled_tasks(status, next_run);

CREATE TABLE IF NOT EXISTS task_run_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    chat_id INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    success INTEGER NOT NULL DEFAULT 1,
    result_summary TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_run_logs_task_id
    ON task_run_logs(task_id);

CREATE TABLE IF NOT EXISTS scheduled_task_dlq (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    chat_id INTEGER NOT NULL,
    failed_at TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    error_summary TEXT,
    replayed_at TEXT,
    replay_note TEXT
);

CREATE INDEX IF NOT EXISTS idx_scheduled_task_dlq_task_failed
    ON scheduled_task_dlq(task_id, failed_at DESC);

CREATE INDEX IF NOT EXISTS idx_scheduled_task_dlq_chat_failed
    ON scheduled_task_dlq(chat_id, failed_at DESC);

CREATE TABLE IF NOT EXISTS sessions (
    chat_id INTEGER PRIMARY KEY,
    messages_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    label TEXT,
    thinking_level TEXT,
    verbose_level TEXT,
    reasoning_level TEXT,
    skill_envs_json TEXT,
    parent_session_key TEXT,
    fork_point INTEGER
);

CREATE INDEX IF NOT EXISTS idx_sessions_parent_session_key
    ON sessions(parent_session_key);

CREATE TABLE IF NOT EXISTS llm_usage_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    caller_channel TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    request_kind TEXT NOT NULL DEFAULT 'agent_loop',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_llm_usage_chat_created
    ON llm_usage_logs(chat_id, created_at);

CREATE INDEX IF NOT EXISTS idx_llm_usage_created
    ON llm_usage_logs(created_at);

CREATE TABLE IF NOT EXISTS memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER,
    content TEXT NOT NULL,
    category TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    embedding_model TEXT,
    confidence REAL NOT NULL DEFAULT 0.70,
    source TEXT NOT NULL DEFAULT 'legacy',
    last_seen_at TEXT NOT NULL,
    is_archived INTEGER NOT NULL DEFAULT 0,
    archived_at TEXT,
    chat_channel TEXT,
    external_chat_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_memories_chat ON memories(chat_id);
CREATE INDEX IF NOT EXISTS idx_memories_active_updated ON memories(is_archived, updated_at);
CREATE INDEX IF NOT EXISTS idx_memories_confidence ON memories(confidence);

CREATE TABLE IF NOT EXISTS memory_reflector_state (
    chat_id INTEGER PRIMARY KEY,
    last_reflected_ts TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_reflector_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    extracted_count INTEGER NOT NULL DEFAULT 0,
    inserted_count INTEGER NOT NULL DEFAULT 0,
    updated_count INTEGER NOT NULL DEFAULT 0,
    skipped_count INTEGER NOT NULL DEFAULT 0,
    dedup_method TEXT NOT NULL,
    parse_ok INTEGER NOT NULL DEFAULT 1,
    error_text TEXT
);

CREATE INDEX IF NOT EXISTS idx_memory_reflector_runs_chat_started
    ON memory_reflector_runs(chat_id, started_at);

CREATE TABLE IF NOT EXISTS memory_injection_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    retrieval_method TEXT NOT NULL,
    candidate_count INTEGER NOT NULL DEFAULT 0,
    selected_count INTEGER NOT NULL DEFAULT 0,
    omitted_count INTEGER NOT NULL DEFAULT 0,
    tokens_est INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_memory_injection_logs_chat_created
    ON memory_injection_logs(chat_id, created_at);

CREATE TABLE IF NOT EXISTS memory_supersede_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_memory_id INTEGER NOT NULL,
    to_memory_id INTEGER NOT NULL,
    reason TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_supersede_from
    ON memory_supersede_edges(from_memory_id, created_at);

CREATE INDEX IF NOT EXISTS idx_memory_supersede_to
    ON memory_supersede_edges(to_memory_id, created_at);

CREATE TABLE IF NOT EXISTS auth_passwords (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS auth_sessions (
    session_id TEXT PRIMARY KEY,
    label TEXT,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires ON auth_sessions(expires_at);

CREATE TABLE IF NOT EXISTS api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    prefix TEXT NOT NULL,
    created_at TEXT NOT NULL,
    revoked_at TEXT,
    last_used_at TEXT,
    expires_at TEXT,
    rotated_from_key_id INTEGER
);

CREATE TABLE IF NOT EXISTS api_key_scopes (
    api_key_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    PRIMARY KEY (api_key_id, scope)
);

CREATE INDEX IF NOT EXISTS idx_api_key_scopes_scope ON api_key_scopes(scope);

CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT,
    status TEXT NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_kind_created
    ON audit_logs(kind, created_at DESC);

CREATE TABLE IF NOT EXISTS metrics_history (
    timestamp_ms INTEGER PRIMARY KEY,
    llm_completions INTEGER NOT NULL DEFAULT 0,
    llm_input_tokens INTEGER NOT NULL DEFAULT 0,
    llm_output_tokens INTEGER NOT NULL DEFAULT 0,
    http_requests INTEGER NOT NULL DEFAULT 0,
    tool_executions INTEGER NOT NULL DEFAULT 0,
    mcp_calls INTEGER NOT NULL DEFAULT 0,
    mcp_rate_limited_rejections INTEGER NOT NULL DEFAULT 0,
    mcp_bulkhead_rejections INTEGER NOT NULL DEFAULT 0,
    mcp_circuit_open_rejections INTEGER NOT NULL DEFAULT 0,
    active_sessions INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_metrics_history_ts ON metrics_history(timestamp_ms);

CREATE TABLE IF NOT EXISTS subagent_runs (
    run_id TEXT PRIMARY KEY,
    parent_run_id TEXT,
    depth INTEGER NOT NULL DEFAULT 1,
    chat_id INTEGER NOT NULL,
    caller_channel TEXT NOT NULL,
    task TEXT NOT NULL,
    context TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    cancel_requested INTEGER NOT NULL DEFAULT 0,
    error_text TEXT,
    result_text TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    token_budget INTEGER NOT NULL DEFAULT 0,
    artifact_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_subagent_runs_chat_created
    ON subagent_runs(chat_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_subagent_runs_chat_status
    ON subagent_runs(chat_id, status);

CREATE INDEX IF NOT EXISTS idx_subagent_runs_parent_status
    ON subagent_runs(parent_run_id, status);

CREATE TABLE IF NOT EXISTS subagent_announces (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    chat_id INTEGER NOT NULL,
    caller_channel TEXT NOT NULL,
    payload_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_subagent_announces_status_next
    ON subagent_announces(status, next_attempt_at);

CREATE TABLE IF NOT EXISTS subagent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_subagent_events_run_created
    ON subagent_events(run_id, created_at ASC);

CREATE TABLE IF NOT EXISTS subagent_focus_bindings (
    chat_id INTEGER PRIMARY KEY,
    run_id TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subagent_findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    orchestration_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    finding TEXT NOT NULL,
    category TEXT DEFAULT 'general',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_findings_orch
    ON subagent_findings(orchestration_id);

CREATE TABLE IF NOT EXISTS document_extractions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    file_hash TEXT NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT,
    file_size INTEGER,
    extracted_text TEXT NOT NULL,
    extraction_method TEXT DEFAULT 'kreuzberg',
    char_count INTEGER,
    created_at TEXT NOT NULL,
    media_object_id INTEGER,
    UNIQUE(chat_id, file_hash)
);

CREATE INDEX IF NOT EXISTS idx_doc_extractions_chat
    ON document_extractions(chat_id);

CREATE INDEX IF NOT EXISTS idx_doc_extractions_media
    ON document_extractions(media_object_id);

CREATE TABLE IF NOT EXISTS media_objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    object_key TEXT NOT NULL UNIQUE,
    storage_backend TEXT NOT NULL DEFAULT 'local',
    original_chat_id INTEGER NOT NULL,
    mime_type TEXT,
    size_bytes INTEGER,
    sha256_hash TEXT,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_objects_chat ON media_objects(original_chat_id);
CREATE INDEX IF NOT EXISTS idx_media_objects_hash ON media_objects(sha256_hash);

CREATE TABLE IF NOT EXISTS knowledge (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    owner_chat_id INTEGER NOT NULL,
    last_grouping_check_at TEXT,
    document_count_at_last_check INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_owner ON knowledge(owner_chat_id);

CREATE TABLE IF NOT EXISTS knowledge_documents (
    knowledge_id INTEGER NOT NULL REFERENCES knowledge(id) ON DELETE CASCADE,
    document_extraction_id INTEGER NOT NULL REFERENCES document_extractions(id),
    added_at TEXT NOT NULL,
    PRIMARY KEY (knowledge_id, document_extraction_id)
);

CREATE TABLE IF NOT EXISTS knowledge_chat_access (
    knowledge_id INTEGER NOT NULL REFERENCES knowledge(id) ON DELETE CASCADE,
    chat_id INTEGER NOT NULL,
    attached_at TEXT NOT NULL,
    PRIMARY KEY (knowledge_id, chat_id)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_access_chat ON knowledge_chat_access(chat_id);

CREATE TABLE IF NOT EXISTS document_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_extraction_id INTEGER NOT NULL REFERENCES document_extractions(id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL,
    text TEXT NOT NULL,
    token_count INTEGER,
    embedding BLOB,
    embedding_status TEXT NOT NULL DEFAULT 'pending',
    observation_status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_extraction ON document_chunks(document_extraction_id);
CREATE INDEX IF NOT EXISTS idx_chunks_embedding_status ON document_chunks(embedding_status);
CREATE INDEX IF NOT EXISTS idx_chunks_observation_status ON document_chunks(observation_status);

CREATE TABLE IF NOT EXISTS db_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL,
    note TEXT
);

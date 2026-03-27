-- PostgreSQL Schema for mchact-memory

-- Extensions
CREATE EXTENSION IF NOT EXISTS vector;

-- Peers table
CREATE TABLE IF NOT EXISTS peers (
    id BIGSERIAL PRIMARY KEY,
    workspace TEXT DEFAULT 'default',
    name TEXT,
    kind TEXT DEFAULT 'user',
    peer_card JSONB,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(workspace, name)
);

-- Observations table
CREATE TABLE IF NOT EXISTS observations (
    id BIGSERIAL PRIMARY KEY,
    workspace TEXT DEFAULT 'default',
    observer_peer_id BIGINT REFERENCES peers(id),
    observed_peer_id BIGINT REFERENCES peers(id),
    chat_id BIGINT,
    level TEXT,
    content TEXT,
    category TEXT,
    confidence REAL DEFAULT 0.8,
    source TEXT DEFAULT 'deriver',
    source_ids JSONB DEFAULT '[]'::jsonb,
    message_ids JSONB DEFAULT '[]'::jsonb,
    times_derived INTEGER DEFAULT 0,
    is_archived BOOLEAN DEFAULT false,
    archived_at TIMESTAMPTZ,
    embedding vector(1536),
    tsv tsvector GENERATED ALWAYS AS (to_tsvector('english', content)) STORED,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Observation indexes
CREATE INDEX IF NOT EXISTS idx_obs_observer ON observations(observer_peer_id);
CREATE INDEX IF NOT EXISTS idx_obs_observed ON observations(observed_peer_id);
CREATE INDEX IF NOT EXISTS idx_obs_level ON observations(level);
CREATE INDEX IF NOT EXISTS idx_obs_chat ON observations(chat_id);
CREATE INDEX IF NOT EXISTS idx_obs_confidence ON observations(confidence);
CREATE INDEX IF NOT EXISTS idx_obs_embedding ON observations USING hnsw (embedding vector_cosine_ops);
CREATE INDEX IF NOT EXISTS idx_obs_tsv ON observations USING GIN (tsv);

-- Observation queue table
CREATE TABLE IF NOT EXISTS observation_queue (
    id BIGSERIAL PRIMARY KEY,
    task_type TEXT,
    workspace TEXT,
    chat_id BIGINT,
    observer_peer_id BIGINT,
    observed_peer_id BIGINT,
    payload JSONB,
    processed BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);

-- Findings table
CREATE TABLE IF NOT EXISTS findings (
    id BIGSERIAL PRIMARY KEY,
    orchestration_id TEXT,
    run_id TEXT,
    finding TEXT,
    category TEXT DEFAULT 'general',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Findings index
CREATE INDEX IF NOT EXISTS idx_findings_orch ON findings(orchestration_id);

-- Deriver runs table
CREATE TABLE IF NOT EXISTS deriver_runs (
    id BIGSERIAL PRIMARY KEY,
    chat_id BIGINT,
    workspace TEXT,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    messages_processed INTEGER,
    explicit_count INTEGER,
    deductive_count INTEGER,
    skipped_count INTEGER,
    error_text TEXT
);

-- Dreamer runs table
CREATE TABLE IF NOT EXISTS dreamer_runs (
    id BIGSERIAL PRIMARY KEY,
    workspace TEXT,
    observer_peer_id BIGINT,
    observed_peer_id BIGINT,
    deductions_created INTEGER,
    inductions_created INTEGER,
    contradictions_found INTEGER,
    consolidated INTEGER,
    peer_card_updated INTEGER,
    run_at TIMESTAMPTZ
);

-- Injection logs table
CREATE TABLE IF NOT EXISTS injection_logs (
    id BIGSERIAL PRIMARY KEY,
    chat_id BIGINT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    retrieval_method TEXT,
    candidate_count INTEGER,
    selected_count INTEGER,
    omitted_count INTEGER,
    tokens_est INTEGER
);

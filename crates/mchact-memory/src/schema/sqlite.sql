-- SQLite Schema for mchact-memory

-- Peers table
CREATE TABLE IF NOT EXISTS peers (
    id INTEGER PRIMARY KEY,
    workspace TEXT DEFAULT 'default',
    name TEXT,
    kind TEXT DEFAULT 'user',
    peer_card TEXT,
    metadata TEXT,
    created_at TEXT,
    updated_at TEXT,
    UNIQUE(workspace, name)
);

-- Observations table
CREATE TABLE IF NOT EXISTS observations (
    id INTEGER PRIMARY KEY,
    workspace TEXT DEFAULT 'default',
    observer_peer_id INTEGER REFERENCES peers(id),
    observed_peer_id INTEGER REFERENCES peers(id),
    chat_id TEXT,
    level TEXT,
    content TEXT,
    category TEXT,
    confidence REAL DEFAULT 0.8,
    source TEXT DEFAULT 'deriver',
    source_ids TEXT DEFAULT '[]',
    message_ids TEXT DEFAULT '[]',
    times_derived INTEGER DEFAULT 0,
    is_archived INTEGER DEFAULT 0,
    archived_at TEXT,
    created_at TEXT,
    updated_at TEXT
);

-- Observation indexes
CREATE INDEX IF NOT EXISTS idx_obs_observer ON observations(observer_peer_id);
CREATE INDEX IF NOT EXISTS idx_obs_observed ON observations(observed_peer_id);
CREATE INDEX IF NOT EXISTS idx_obs_level ON observations(level);
CREATE INDEX IF NOT EXISTS idx_obs_chat ON observations(chat_id);
CREATE INDEX IF NOT EXISTS idx_obs_confidence ON observations(confidence);

-- FTS5 virtual table for observations
CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
    content,
    content='observations',
    content_rowid='id'
);

-- FTS5 trigger: INSERT
CREATE TRIGGER IF NOT EXISTS observations_ai AFTER INSERT ON observations BEGIN
    INSERT INTO observations_fts(rowid, content) VALUES (new.id, new.content);
END;

-- FTS5 trigger: DELETE
CREATE TRIGGER IF NOT EXISTS observations_ad AFTER DELETE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, content) VALUES('delete', old.id, old.content);
END;

-- FTS5 trigger: UPDATE
CREATE TRIGGER IF NOT EXISTS observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, content) VALUES('delete', old.id, old.content);
    INSERT INTO observations_fts(rowid, content) VALUES (new.id, new.content);
END;

-- Observation queue table
CREATE TABLE IF NOT EXISTS observation_queue (
    id INTEGER PRIMARY KEY,
    task_type TEXT,
    workspace TEXT,
    chat_id TEXT,
    observer_peer_id INTEGER,
    observed_peer_id INTEGER,
    payload TEXT,
    processed INTEGER DEFAULT 0,
    created_at TEXT,
    processed_at TEXT
);

-- Findings table
CREATE TABLE IF NOT EXISTS findings (
    id INTEGER PRIMARY KEY,
    orchestration_id TEXT,
    run_id TEXT,
    finding TEXT,
    category TEXT DEFAULT 'general',
    created_at TEXT
);

-- Findings index
CREATE INDEX IF NOT EXISTS idx_findings_orch ON findings(orchestration_id);

-- Deriver runs table
CREATE TABLE IF NOT EXISTS deriver_runs (
    id INTEGER PRIMARY KEY,
    orchestration_id TEXT,
    workspace TEXT,
    observer_peer_id INTEGER,
    observed_peer_id INTEGER,
    chat_id TEXT,
    observations_in INTEGER DEFAULT 0,
    observations_out INTEGER DEFAULT 0,
    duration_ms INTEGER DEFAULT 0,
    created_at TEXT
);

-- Dreamer runs table
CREATE TABLE IF NOT EXISTS dreamer_runs (
    id INTEGER PRIMARY KEY,
    orchestration_id TEXT,
    workspace TEXT,
    observer_peer_id INTEGER,
    observed_peer_id INTEGER,
    observations_in INTEGER DEFAULT 0,
    findings_out INTEGER DEFAULT 0,
    duration_ms INTEGER DEFAULT 0,
    created_at TEXT
);

-- Injection logs table
CREATE TABLE IF NOT EXISTS injection_logs (
    id INTEGER PRIMARY KEY,
    orchestration_id TEXT,
    workspace TEXT,
    chat_id TEXT,
    observer_peer_id INTEGER,
    observed_peer_id INTEGER,
    observations_injected INTEGER DEFAULT 0,
    token_estimate INTEGER DEFAULT 0,
    created_at TEXT
);

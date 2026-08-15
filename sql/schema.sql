CREATE TABLE IF NOT EXISTS agent_sessions (
    session_id TEXT PRIMARY KEY NOT NULL,
    objective TEXT NOT NULL,
    state TEXT NOT NULL,
    supervisor TEXT NOT NULL,
    authorized_agents_json TEXT NOT NULL DEFAULT '[]',
    authorized_tools_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    max_sources INTEGER NOT NULL,
    max_candidates INTEGER NOT NULL,
    max_agent_runs INTEGER NOT NULL,
    max_runtime_seconds INTEGER NOT NULL,
    max_retries INTEGER NOT NULL,
    max_artifact_bytes INTEGER NOT NULL,
    sources_used INTEGER NOT NULL DEFAULT 0,
    candidates_used INTEGER NOT NULL DEFAULT 0,
    agent_runs_used INTEGER NOT NULL DEFAULT 0,
    publication_allowed INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS agent_runs (
    run_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    agent_version TEXT NOT NULL,
    state TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    input_json TEXT NOT NULL DEFAULT '{}',
    output_json TEXT NOT NULL DEFAULT '{}',
    artifact_id TEXT NOT NULL DEFAULT '',
    queued_at TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT '',
    completed_at TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (session_id) REFERENCES agent_sessions(session_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_runs_session ON agent_runs(session_id, queued_at);

CREATE TABLE IF NOT EXISTS agent_messages (
    message_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    from_agent TEXT NOT NULL,
    to_agent TEXT NOT NULL,
    type TEXT NOT NULL,
    artifact_id TEXT NOT NULL DEFAULT '',
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES agent_sessions(session_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_messages_session ON agent_messages(session_id, created_at);

CREATE TABLE IF NOT EXISTS agent_events (
    event_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL,
    meta_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_events_session ON agent_events(session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agent_events_kind ON agent_events(kind, created_at);

CREATE TABLE IF NOT EXISTS agent_artifacts (
    artifact_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    creator TEXT NOT NULL,
    agent_id TEXT NOT NULL DEFAULT '',
    agent_version TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    path TEXT NOT NULL,
    provenance_json TEXT NOT NULL DEFAULT '[]',
    FOREIGN KEY (session_id) REFERENCES agent_sessions(session_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_artifacts_session ON agent_artifacts(session_id, created_at);

CREATE TABLE IF NOT EXISTS agent_receipts (
    receipt_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL UNIQUE,
    body_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES agent_sessions(session_id)
);

CREATE TABLE IF NOT EXISTS agent_id_counters (
    kind TEXT PRIMARY KEY NOT NULL,
    next_value INTEGER NOT NULL
);

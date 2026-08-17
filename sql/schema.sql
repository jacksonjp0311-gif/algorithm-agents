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

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS archive_publication_receipts (
    receipt_id TEXT PRIMARY KEY NOT NULL,
    candidate_id TEXT NOT NULL DEFAULT '',
    session_id TEXT NOT NULL DEFAULT '',
    revision_id TEXT NOT NULL,
    action TEXT NOT NULL,
    reviewer TEXT NOT NULL,
    reason TEXT NOT NULL,
    candidate_hash TEXT NOT NULL DEFAULT '',
    canonical_hash TEXT NOT NULL,
    body_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_archive_receipts_revision
ON archive_publication_receipts(revision_id, created_at);

CREATE TABLE IF NOT EXISTS knowledge_nodes (
    node_id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    canonical INTEGER NOT NULL DEFAULT 0,
    label TEXT NOT NULL,
    body_json TEXT NOT NULL,
    provenance_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_edges (
    edge_id TEXT PRIMARY KEY NOT NULL,
    from_node TEXT NOT NULL,
    to_node TEXT NOT NULL,
    relation TEXT NOT NULL,
    canonical INTEGER NOT NULL DEFAULT 0,
    evidence_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    FOREIGN KEY (from_node) REFERENCES knowledge_nodes(node_id),
    FOREIGN KEY (to_node) REFERENCES knowledge_nodes(node_id)
);

CREATE TABLE IF NOT EXISTS relationship_proposals (
    proposal_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    from_node TEXT NOT NULL,
    to_node TEXT NOT NULL,
    relation TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'PENDING',
    evidence_json TEXT NOT NULL DEFAULT '[]',
    proposed_by TEXT NOT NULL,
    reviewed_by TEXT NOT NULL DEFAULT '',
    review_reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_relationship_proposals_state
ON relationship_proposals(state, created_at);

CREATE TABLE IF NOT EXISTS supervisor_invocations (
    invocation_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    operation TEXT NOT NULL,
    request_json TEXT NOT NULL,
    response_json TEXT NOT NULL DEFAULT '{}',
    state TEXT NOT NULL,
    created_at TEXT NOT NULL,
    completed_at TEXT NOT NULL DEFAULT ''
);

-- Alchetron v2 evidence, model, evaluation, and emergence tables.
CREATE TABLE IF NOT EXISTS source_snapshots (
    snapshot_id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL, artifact_id TEXT NOT NULL,
    locator TEXT NOT NULL, resolved TEXT NOT NULL DEFAULT '', content_hash TEXT NOT NULL,
    bytes INTEGER NOT NULL, path TEXT NOT NULL, captured_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS extraction_claims (
    claim_id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL, candidate_id TEXT NOT NULL DEFAULT '',
    field TEXT NOT NULL, claim_text TEXT NOT NULL, epistemic_status TEXT NOT NULL,
    confidence REAL NOT NULL, evidence_json TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_extraction_claims_candidate ON extraction_claims(candidate_id, field);
CREATE TABLE IF NOT EXISTS run_manifests (
    manifest_id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL, snapshot_id TEXT NOT NULL,
    content_hash TEXT NOT NULL, body_json TEXT NOT NULL, path TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS model_invocations (
    invocation_id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL DEFAULT '', provider TEXT NOT NULL,
    model TEXT NOT NULL, endpoint TEXT NOT NULL, prompt_hash TEXT NOT NULL,
    response_hash TEXT NOT NULL DEFAULT '', state TEXT NOT NULL, input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER NOT NULL DEFAULT 0,
    error TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, completed_at TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS evaluation_runs (
    evaluation_id TEXT PRIMARY KEY NOT NULL, corpus_version TEXT NOT NULL, runtime_version TEXT NOT NULL,
    metrics_json TEXT NOT NULL, cases_json TEXT NOT NULL, passed INTEGER NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS emergent_hypotheses (
    hypothesis_id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL DEFAULT '', title TEXT NOT NULL,
    thesis TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'PRIVATE', confidence REAL NOT NULL DEFAULT 0,
    evidence_json TEXT NOT NULL DEFAULT '[]', node_refs_json TEXT NOT NULL DEFAULT '[]',
    proposed_by TEXT NOT NULL, model_json TEXT NOT NULL DEFAULT '{}', reviewed_by TEXT NOT NULL DEFAULT '',
    review_reason TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS emergent_challenges (
    challenge_id TEXT PRIMARY KEY NOT NULL, hypothesis_id TEXT NOT NULL, challenger TEXT NOT NULL,
    verdict TEXT NOT NULL, rationale TEXT NOT NULL, evidence_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL, FOREIGN KEY (hypothesis_id) REFERENCES emergent_hypotheses(hypothesis_id)
);
CREATE TABLE IF NOT EXISTS research_logs (
    log_id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL DEFAULT '', title TEXT NOT NULL,
    body TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'PRIVATE', author_type TEXT NOT NULL,
    author_name TEXT NOT NULL, model_json TEXT NOT NULL DEFAULT '{}', sources_json TEXT NOT NULL DEFAULT '[]',
    node_refs_json TEXT NOT NULL DEFAULT '[]', reviewed_by TEXT NOT NULL DEFAULT '',
    review_reason TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS knowledge_disputes (
    dispute_id TEXT PRIMARY KEY NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'OPEN', claim TEXT NOT NULL, evidence_json TEXT NOT NULL DEFAULT '[]',
    opened_by TEXT NOT NULL, resolved_by TEXT NOT NULL DEFAULT '', resolution TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS imported_bundles (
    bundle_id TEXT PRIMARY KEY NOT NULL, content_hash TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'PRIVATE_REVIEW', source TEXT NOT NULL,
    body_json TEXT NOT NULL, created_at TEXT NOT NULL
);

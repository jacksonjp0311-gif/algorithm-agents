CREATE TABLE IF NOT EXISTS source_snapshots (
    snapshot_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    locator TEXT NOT NULL,
    resolved TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL,
    bytes INTEGER NOT NULL,
    path TEXT NOT NULL,
    captured_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS extraction_claims (
    claim_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL DEFAULT '',
    field TEXT NOT NULL,
    claim_text TEXT NOT NULL,
    epistemic_status TEXT NOT NULL,
    confidence REAL NOT NULL,
    evidence_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_extraction_claims_candidate
ON extraction_claims(candidate_id, field);

CREATE TABLE IF NOT EXISTS run_manifests (
    manifest_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    body_json TEXT NOT NULL,
    path TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_invocations (
    invocation_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    prompt_hash TEXT NOT NULL,
    response_hash TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    error TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    completed_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS evaluation_runs (
    evaluation_id TEXT PRIMARY KEY NOT NULL,
    corpus_version TEXT NOT NULL,
    runtime_version TEXT NOT NULL,
    metrics_json TEXT NOT NULL,
    cases_json TEXT NOT NULL,
    passed INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS emergent_hypotheses (
    hypothesis_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL,
    thesis TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'PRIVATE',
    confidence REAL NOT NULL DEFAULT 0,
    evidence_json TEXT NOT NULL DEFAULT '[]',
    node_refs_json TEXT NOT NULL DEFAULT '[]',
    proposed_by TEXT NOT NULL,
    model_json TEXT NOT NULL DEFAULT '{}',
    reviewed_by TEXT NOT NULL DEFAULT '',
    review_reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS emergent_challenges (
    challenge_id TEXT PRIMARY KEY NOT NULL,
    hypothesis_id TEXT NOT NULL,
    challenger TEXT NOT NULL,
    verdict TEXT NOT NULL,
    rationale TEXT NOT NULL,
    evidence_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    FOREIGN KEY (hypothesis_id) REFERENCES emergent_hypotheses(hypothesis_id)
);

CREATE TABLE IF NOT EXISTS research_logs (
    log_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'PRIVATE',
    author_type TEXT NOT NULL,
    author_name TEXT NOT NULL,
    model_json TEXT NOT NULL DEFAULT '{}',
    sources_json TEXT NOT NULL DEFAULT '[]',
    node_refs_json TEXT NOT NULL DEFAULT '[]',
    reviewed_by TEXT NOT NULL DEFAULT '',
    review_reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_disputes (
    dispute_id TEXT PRIMARY KEY NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'OPEN',
    claim TEXT NOT NULL,
    evidence_json TEXT NOT NULL DEFAULT '[]',
    opened_by TEXT NOT NULL,
    resolved_by TEXT NOT NULL DEFAULT '',
    resolution TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS imported_bundles (
    bundle_id TEXT PRIMARY KEY NOT NULL,
    content_hash TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'PRIVATE_REVIEW',
    source TEXT NOT NULL,
    body_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

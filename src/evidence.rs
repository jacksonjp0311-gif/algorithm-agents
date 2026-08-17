use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::error::AgentError;
use crate::persist;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSnapshot {
    pub snapshot_id: String,
    pub session_id: String,
    pub artifact_id: String,
    pub locator: String,
    pub resolved: String,
    pub content_hash: String,
    pub bytes: usize,
    pub captured_at: String,
    pub path: String,
    #[serde(skip)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSpan {
    pub span_id: String,
    pub snapshot_id: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub quote: String,
    pub match_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRecord {
    pub claim_id: String,
    pub field: String,
    pub text: String,
    pub epistemic_status: String,
    pub confidence: f64,
    pub evidence: Vec<EvidenceSpan>,
}

pub async fn capture_source_snapshot(
    pool: &SqlitePool,
    data_dir: &Path,
    session_id: &str,
    artifact_id: &str,
    source: &Value,
) -> Result<SourceSnapshot, AgentError> {
    let text = source
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if text.trim().is_empty() {
        return Err(AgentError::Invalid(
            "source snapshot requires retrieved source text".into(),
        ));
    }
    let content_hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    let snapshot_id = format!("SRC-{}", &content_hash[..24]);
    let dir = data_dir.join("snapshots");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{snapshot_id}.txt"));
    if tokio::fs::metadata(&path).await.is_err() {
        let temporary = path.with_extension("tmp");
        tokio::fs::write(&temporary, text.as_bytes()).await?;
        tokio::fs::rename(&temporary, &path).await?;
    }
    let snapshot = SourceSnapshot {
        snapshot_id,
        session_id: session_id.to_owned(),
        artifact_id: artifact_id.to_owned(),
        locator: source
            .get("locator")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        resolved: source
            .get("resolved")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        content_hash,
        bytes: text.len(),
        captured_at: persist::now_rfc3339()?,
        path: path.to_string_lossy().into_owned(),
        text,
    };
    sqlx::query(
        "INSERT INTO source_snapshots
         (snapshot_id, session_id, artifact_id, locator, resolved, content_hash, bytes, path, captured_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(snapshot_id) DO UPDATE SET session_id=excluded.session_id,
         artifact_id=excluded.artifact_id, locator=excluded.locator, resolved=excluded.resolved",
    )
    .bind(&snapshot.snapshot_id)
    .bind(&snapshot.session_id)
    .bind(&snapshot.artifact_id)
    .bind(&snapshot.locator)
    .bind(&snapshot.resolved)
    .bind(&snapshot.content_hash)
    .bind(snapshot.bytes as i64)
    .bind(&snapshot.path)
    .bind(&snapshot.captured_at)
    .execute(pool)
    .await?;
    Ok(snapshot)
}

pub fn build_claim_records(extraction: &Value, snapshot: &SourceSnapshot) -> Vec<ClaimRecord> {
    let mut claims = Vec::new();
    for field in ["title", "summary", "core_idea", "math"] {
        if let Some(text) = extraction.get(field).and_then(Value::as_str) {
            push_claim(&mut claims, field, text, snapshot);
        }
    }
    for field in ["assumptions", "constraints", "known_uses"] {
        if let Some(values) = extraction.get(field).and_then(Value::as_array) {
            for text in values.iter().filter_map(Value::as_str) {
                push_claim(&mut claims, field, text, snapshot);
            }
        }
    }
    claims
}

fn push_claim(claims: &mut Vec<ClaimRecord>, field: &str, raw: &str, snapshot: &SourceSnapshot) {
    let text = raw.trim();
    if text.is_empty() {
        return;
    }
    let claim_id = format!(
        "CLM-{}",
        short_hash(&format!("{}|{}|{}", snapshot.snapshot_id, field, text))
    );
    let evidence = find_span(&snapshot.text, text, &snapshot.snapshot_id, &claim_id)
        .into_iter()
        .collect::<Vec<_>>();
    claims.push(ClaimRecord {
        claim_id,
        field: field.to_owned(),
        text: text.to_owned(),
        epistemic_status: if evidence.is_empty() {
            "MODEL_INFERRED".into()
        } else {
            "SOURCE_STATED".into()
        },
        confidence: if evidence.is_empty() { 0.45 } else { 1.0 },
        evidence,
    });
}

fn find_span(source: &str, claim: &str, snapshot_id: &str, claim_id: &str) -> Option<EvidenceSpan> {
    let source_lower = source.to_lowercase();
    let claim_lower = claim.to_lowercase();
    let start = source_lower.find(&claim_lower)?;
    let end = start + claim.len();
    if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return None;
    }
    let start_line = source[..start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let end_line = start_line
        + source[start..end]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
    Some(EvidenceSpan {
        span_id: format!("SPN-{}", short_hash(&format!("{claim_id}|{start}|{end}"))),
        snapshot_id: snapshot_id.to_owned(),
        start_byte: start,
        end_byte: end,
        start_line,
        end_line,
        quote: source[start..end].to_owned(),
        match_kind: "EXACT_CASE_INSENSITIVE".into(),
    })
}

pub async fn persist_claims(
    pool: &SqlitePool,
    session_id: &str,
    candidate_id: &str,
    claims: &[ClaimRecord],
) -> Result<(), AgentError> {
    let mut transaction = pool.begin().await?;
    for claim in claims {
        sqlx::query(
            "INSERT INTO extraction_claims
             (claim_id, session_id, candidate_id, field, claim_text, epistemic_status,
              confidence, evidence_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(claim_id) DO UPDATE SET candidate_id=excluded.candidate_id,
             epistemic_status=excluded.epistemic_status, confidence=excluded.confidence,
             evidence_json=excluded.evidence_json",
        )
        .bind(&claim.claim_id)
        .bind(session_id)
        .bind(candidate_id)
        .bind(&claim.field)
        .bind(&claim.text)
        .bind(&claim.epistemic_status)
        .bind(claim.confidence)
        .bind(serde_json::to_string(&claim.evidence)?)
        .bind(persist::now_rfc3339()?)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn write_run_manifest(
    pool: &SqlitePool,
    data_dir: &Path,
    session_id: &str,
    snapshot: &SourceSnapshot,
) -> Result<Value, AgentError> {
    let session = persist::get_session(pool, session_id).await?;
    let runs = persist::list_runs(pool, session_id).await?;
    let manifest_id = format!("MAN-{}", uuid::Uuid::new_v4().simple());
    let body = json!({
        "schema_version": "1.0",
        "manifest_id": manifest_id,
        "session_id": session_id,
        "objective": session.objective,
        "supervisor": session.supervisor,
        "source_snapshot": {
            "snapshot_id": snapshot.snapshot_id,
            "content_hash": snapshot.content_hash,
            "locator": snapshot.locator,
            "resolved": snapshot.resolved,
            "bytes": snapshot.bytes
        },
        "agents": runs.iter().map(|run| json!({
            "run_id": run.run_id,
            "agent": run.agent_id,
            "version": run.agent_version,
            "state": run.state,
            "input_hash": short_hash(&serde_json::to_string(&run.input).unwrap_or_default()),
            "output_hash": short_hash(&serde_json::to_string(&run.output).unwrap_or_default())
        })).collect::<Vec<_>>(),
        "budgets": {
            "max_sources": session.max_sources,
            "max_candidates": session.max_candidates,
            "max_agent_runs": session.max_agent_runs,
            "max_runtime_seconds": session.max_runtime_seconds,
            "max_artifact_bytes": session.max_artifact_bytes
        },
        "created_at": persist::now_rfc3339()?
    });
    let bytes = serde_json::to_vec_pretty(&body)?;
    let content_hash = format!("{:x}", Sha256::digest(&bytes));
    let dir = data_dir.join("manifests");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{manifest_id}.json"));
    tokio::fs::write(&path, bytes).await?;
    sqlx::query(
        "INSERT INTO run_manifests
         (manifest_id, session_id, snapshot_id, content_hash, body_json, path, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&manifest_id)
    .bind(session_id)
    .bind(&snapshot.snapshot_id)
    .bind(content_hash)
    .bind(serde_json::to_string(&body)?)
    .bind(path.to_string_lossy().as_ref())
    .bind(persist::now_rfc3339()?)
    .execute(pool)
    .await?;
    Ok(body)
}

pub async fn list_claims(pool: &SqlitePool, candidate_id: &str) -> Result<Vec<Value>, AgentError> {
    let rows = sqlx::query(
        "SELECT * FROM extraction_claims WHERE candidate_id=? ORDER BY field, claim_id",
    )
    .bind(candidate_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let evidence: String = row.get("evidence_json");
            json!({
                "claim_id": row.get::<String, _>("claim_id"),
                "field": row.get::<String, _>("field"),
                "text": row.get::<String, _>("claim_text"),
                "epistemic_status": row.get::<String, _>("epistemic_status"),
                "confidence": row.get::<f64, _>("confidence"),
                "evidence": serde_json::from_str::<Value>(&evidence).unwrap_or_else(|_| json!([]))
            })
        })
        .collect())
}

fn short_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
        .chars()
        .take(24)
        .collect()
}

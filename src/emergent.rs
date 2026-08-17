use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};

use crate::error::AgentError;
use crate::persist;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisInput {
    #[serde(default)]
    pub session_id: String,
    pub title: String,
    pub thesis: String,
    pub confidence: f64,
    pub evidence: Value,
    #[serde(default)]
    pub node_refs: Vec<String>,
    pub proposed_by: String,
    #[serde(default)]
    pub model: Value,
    #[serde(default)]
    pub experiment: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchLogInput {
    #[serde(default)]
    pub session_id: String,
    pub title: String,
    pub body: String,
    pub author_type: String,
    pub author_name: String,
    #[serde(default)]
    pub model: Value,
    #[serde(default)]
    pub sources: Value,
    #[serde(default)]
    pub node_refs: Vec<String>,
}

pub async fn propose_hypothesis(
    pool: &SqlitePool,
    input: HypothesisInput,
) -> Result<Value, AgentError> {
    validate_hypothesis(pool, &input).await?;
    let hypothesis_id = format!("HYP-{}", uuid::Uuid::new_v4().simple());
    let now = persist::now_rfc3339()?;
    sqlx::query(
        "INSERT INTO emergent_hypotheses
         (hypothesis_id, session_id, title, thesis, state, confidence, evidence_json,
          node_refs_json, proposed_by, model_json, reviewed_by, review_reason,
          created_at, updated_at)
         VALUES (?, ?, ?, ?, 'PRIVATE', ?, ?, ?, ?, ?, '', '', ?, ?)",
    )
    .bind(&hypothesis_id)
    .bind(&input.session_id)
    .bind(&input.title)
    .bind(&input.thesis)
    .bind(input.confidence.clamp(0.0, 1.0))
    .bind(serde_json::to_string(&input.evidence)?)
    .bind(serde_json::to_string(&input.node_refs)?)
    .bind(&input.proposed_by)
    .bind(serde_json::to_string(&json!({
        "identity": input.model,
        "experiment": input.experiment
    }))?)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    persist::insert_event(
        pool,
        &input.session_id,
        "PRIVATE_HYPOTHESIS_PROPOSED",
        json!({ "hypothesis_id": hypothesis_id, "proposed_by": input.proposed_by }),
    )
    .await?;
    get_hypothesis(pool, &hypothesis_id).await
}

pub async fn challenge_hypothesis(
    pool: &SqlitePool,
    hypothesis_id: &str,
    challenger: &str,
    verdict: &str,
    rationale: &str,
    evidence: Value,
) -> Result<Value, AgentError> {
    get_hypothesis(pool, hypothesis_id).await?;
    if challenger.trim().is_empty()
        || !matches!(verdict, "SUPPORTED" | "CONTESTED" | "REFUTED" | "UNKNOWN")
        || rationale.trim().len() < 8
    {
        return Err(AgentError::Invalid(
            "challenge requires identity, supported verdict, and rationale".into(),
        ));
    }
    let challenge_id = format!("CHL-{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO emergent_challenges
         (challenge_id, hypothesis_id, challenger, verdict, rationale, evidence_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&challenge_id)
    .bind(hypothesis_id)
    .bind(challenger)
    .bind(verdict)
    .bind(rationale)
    .bind(serde_json::to_string(&evidence)?)
    .bind(persist::now_rfc3339()?)
    .execute(pool)
    .await?;
    Ok(json!({
        "challenge_id": challenge_id,
        "hypothesis_id": hypothesis_id,
        "verdict": verdict,
        "canonical_publication": false
    }))
}

pub async fn review_hypothesis(
    pool: &SqlitePool,
    hypothesis_id: &str,
    accept: bool,
    reviewer: &str,
    reason: &str,
) -> Result<Value, AgentError> {
    if reviewer.trim().is_empty() || reason.trim().len() < 8 {
        return Err(AgentError::Invalid(
            "hypothesis review requires reviewer and meaningful reason".into(),
        ));
    }
    let state = if accept { "REVIEWED" } else { "REJECTED" };
    let changed = sqlx::query(
        "UPDATE emergent_hypotheses SET state=?, reviewed_by=?, review_reason=?, updated_at=?
         WHERE hypothesis_id=? AND state='PRIVATE'",
    )
    .bind(state)
    .bind(reviewer)
    .bind(reason)
    .bind(persist::now_rfc3339()?)
    .bind(hypothesis_id)
    .execute(pool)
    .await?;
    if changed.rows_affected() != 1 {
        return Err(AgentError::State(
            "hypothesis is missing or already reviewed".into(),
        ));
    }
    let mut result = get_hypothesis(pool, hypothesis_id).await?;
    result["canonical_publication"] = Value::Bool(false);
    result["next_boundary"] = json!(if accept {
        "operator may create a separate relationship proposal"
    } else {
        "terminal"
    });
    Ok(result)
}

pub async fn list_hypotheses(
    pool: &SqlitePool,
    include_private: bool,
) -> Result<Vec<Value>, AgentError> {
    let sql = if include_private {
        "SELECT * FROM emergent_hypotheses ORDER BY created_at DESC"
    } else {
        "SELECT * FROM emergent_hypotheses WHERE state='REVIEWED' ORDER BY created_at DESC"
    };
    let rows = sqlx::query(sql).fetch_all(pool).await?;
    let mut hypotheses = Vec::new();
    for row in rows {
        let id: String = row.get("hypothesis_id");
        let challenges = list_challenges(pool, &id).await?;
        hypotheses.push(hypothesis_from_row(row, challenges));
    }
    Ok(hypotheses)
}

pub async fn create_research_log(
    pool: &SqlitePool,
    input: ResearchLogInput,
) -> Result<Value, AgentError> {
    if input.title.trim().len() < 3
        || input.body.trim().len() < 40
        || !matches!(input.author_type.as_str(), "HUMAN" | "MODEL" | "HYBRID")
        || input.author_name.trim().is_empty()
    {
        return Err(AgentError::Invalid(
            "research log requires title, substantive body, author type, and identity".into(),
        ));
    }
    validate_node_refs(pool, &input.node_refs).await?;
    let log_id = format!("LOG-{}", uuid::Uuid::new_v4().simple());
    let now = persist::now_rfc3339()?;
    sqlx::query(
        "INSERT INTO research_logs
         (log_id, session_id, title, body, state, author_type, author_name, model_json,
          sources_json, node_refs_json, reviewed_by, review_reason, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'PRIVATE', ?, ?, ?, ?, ?, '', '', ?, ?)",
    )
    .bind(&log_id)
    .bind(&input.session_id)
    .bind(&input.title)
    .bind(&input.body)
    .bind(&input.author_type)
    .bind(&input.author_name)
    .bind(serde_json::to_string(&input.model)?)
    .bind(serde_json::to_string(&input.sources)?)
    .bind(serde_json::to_string(&input.node_refs)?)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_research_log(pool, &log_id).await
}

pub async fn review_research_log(
    pool: &SqlitePool,
    log_id: &str,
    publish: bool,
    reviewer: &str,
    reason: &str,
) -> Result<Value, AgentError> {
    if reviewer.trim().is_empty() || reason.trim().len() < 8 {
        return Err(AgentError::Invalid(
            "log review requires reviewer and meaningful reason".into(),
        ));
    }
    let state = if publish { "PUBLIC" } else { "REJECTED" };
    let changed = sqlx::query(
        "UPDATE research_logs SET state=?, reviewed_by=?, review_reason=?, updated_at=?
         WHERE log_id=? AND state='PRIVATE'",
    )
    .bind(state)
    .bind(reviewer)
    .bind(reason)
    .bind(persist::now_rfc3339()?)
    .bind(log_id)
    .execute(pool)
    .await?;
    if changed.rows_affected() != 1 {
        return Err(AgentError::State(
            "research log is missing or already reviewed".into(),
        ));
    }
    get_research_log(pool, log_id).await
}

pub async fn list_research_logs(
    pool: &SqlitePool,
    include_private: bool,
) -> Result<Vec<Value>, AgentError> {
    let sql = if include_private {
        "SELECT * FROM research_logs ORDER BY created_at DESC"
    } else {
        "SELECT * FROM research_logs WHERE state='PUBLIC' ORDER BY created_at DESC"
    };
    Ok(sqlx::query(sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(research_log_from_row)
        .collect())
}

pub async fn scan_for_missing_links(
    pool: &SqlitePool,
    proposed_by: &str,
) -> Result<Vec<Value>, AgentError> {
    let rows = sqlx::query(
        "SELECT DISTINCT a.node_id AS a_id, a.label AS a_label, b.node_id AS b_id,
         b.label AS b_label, d.node_id AS domain_id, d.label AS domain_label
         FROM knowledge_edges ea
         JOIN knowledge_edges eb ON ea.to_node=eb.to_node
         JOIN knowledge_nodes a ON a.node_id=ea.from_node AND a.kind='ALGORITHM'
         JOIN knowledge_nodes b ON b.node_id=eb.from_node AND b.kind='ALGORITHM'
         JOIN knowledge_nodes d ON d.node_id=ea.to_node AND d.kind='DOMAIN'
         WHERE a.node_id < b.node_id AND ea.canonical=1 AND eb.canonical=1
         AND NOT EXISTS (
           SELECT 1 FROM knowledge_edges direct
           WHERE (direct.from_node=a.node_id AND direct.to_node=b.node_id)
              OR (direct.from_node=b.node_id AND direct.to_node=a.node_id)
         ) LIMIT 20",
    )
    .fetch_all(pool)
    .await?;
    let mut proposed = Vec::new();
    for row in rows {
        let a_id: String = row.get("a_id");
        let b_id: String = row.get("b_id");
        let domain_id: String = row.get("domain_id");
        let input = HypothesisInput {
            session_id: String::new(),
            title: format!(
                "Possible structural relation: {} ↔ {}",
                row.get::<String, _>("a_label"),
                row.get::<String, _>("b_label")
            ),
            thesis: format!(
                "These canonical algorithms share the {} domain but have no reviewed direct relationship. Test for a reusable structural analogy.",
                row.get::<String, _>("domain_label")
            ),
            confidence: 0.25,
            evidence: json!([{
                "kind": "GRAPH_PATTERN",
                "domain_node": domain_id,
                "algorithm_nodes": [a_id, b_id]
            }]),
            node_refs: vec![a_id, b_id],
            proposed_by: proposed_by.into(),
            model: json!({ "provider": "deterministic", "model": "missing-link-scanner@2.0" }),
            experiment: json!({
                "question": "Do the state transitions or invariants admit a structure-preserving mapping?",
                "required_result": "mapping, counterexample, or UNKNOWN"
            }),
        };
        proposed.push(propose_hypothesis(pool, input).await?);
    }
    Ok(proposed)
}

async fn validate_hypothesis(pool: &SqlitePool, input: &HypothesisInput) -> Result<(), AgentError> {
    if input.title.trim().len() < 5
        || input.thesis.trim().len() < 20
        || input.proposed_by.trim().is_empty()
        || input.evidence.as_array().is_none_or(Vec::is_empty)
    {
        return Err(AgentError::Invalid(
            "hypothesis requires title, thesis, evidence, and proposer identity".into(),
        ));
    }
    validate_node_refs(pool, &input.node_refs).await
}

async fn validate_node_refs(pool: &SqlitePool, refs: &[String]) -> Result<(), AgentError> {
    for node_id in refs {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM knowledge_nodes WHERE node_id=? AND canonical=1",
        )
        .bind(node_id)
        .fetch_one(pool)
        .await?;
        if exists != 1 {
            return Err(AgentError::Denied(format!(
                "emergent work may reference only canonical node `{node_id}`"
            )));
        }
    }
    Ok(())
}

async fn get_hypothesis(pool: &SqlitePool, hypothesis_id: &str) -> Result<Value, AgentError> {
    let row = sqlx::query("SELECT * FROM emergent_hypotheses WHERE hypothesis_id=?")
        .bind(hypothesis_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("hypothesis `{hypothesis_id}` not found")))?;
    let challenges = list_challenges(pool, hypothesis_id).await?;
    Ok(hypothesis_from_row(row, challenges))
}

async fn list_challenges(pool: &SqlitePool, hypothesis_id: &str) -> Result<Vec<Value>, AgentError> {
    let rows =
        sqlx::query("SELECT * FROM emergent_challenges WHERE hypothesis_id=? ORDER BY created_at")
            .bind(hypothesis_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let evidence: String = row.get("evidence_json");
            json!({
                "challenge_id": row.get::<String, _>("challenge_id"),
                "challenger": row.get::<String, _>("challenger"),
                "verdict": row.get::<String, _>("verdict"),
                "rationale": row.get::<String, _>("rationale"),
                "evidence": serde_json::from_str::<Value>(&evidence).unwrap_or_else(|_| json!([])),
                "created_at": row.get::<String, _>("created_at")
            })
        })
        .collect())
}

fn hypothesis_from_row(row: sqlx::sqlite::SqliteRow, challenges: Vec<Value>) -> Value {
    let evidence: String = row.get("evidence_json");
    let refs: String = row.get("node_refs_json");
    let model: String = row.get("model_json");
    json!({
        "hypothesis_id": row.get::<String, _>("hypothesis_id"),
        "session_id": row.get::<String, _>("session_id"),
        "title": row.get::<String, _>("title"),
        "thesis": row.get::<String, _>("thesis"),
        "state": row.get::<String, _>("state"),
        "confidence": row.get::<f64, _>("confidence"),
        "evidence": serde_json::from_str::<Value>(&evidence).unwrap_or_else(|_| json!([])),
        "node_refs": serde_json::from_str::<Value>(&refs).unwrap_or_else(|_| json!([])),
        "proposed_by": row.get::<String, _>("proposed_by"),
        "model": serde_json::from_str::<Value>(&model).unwrap_or_else(|_| json!({})),
        "reviewed_by": row.get::<String, _>("reviewed_by"),
        "review_reason": row.get::<String, _>("review_reason"),
        "challenges": challenges,
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at"),
        "canonical_publication": false
    })
}

async fn get_research_log(pool: &SqlitePool, log_id: &str) -> Result<Value, AgentError> {
    let row = sqlx::query("SELECT * FROM research_logs WHERE log_id=?")
        .bind(log_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("research log `{log_id}` not found")))?;
    Ok(research_log_from_row(row))
}

fn research_log_from_row(row: sqlx::sqlite::SqliteRow) -> Value {
    let model: String = row.get("model_json");
    let sources: String = row.get("sources_json");
    let refs: String = row.get("node_refs_json");
    json!({
        "log_id": row.get::<String, _>("log_id"),
        "session_id": row.get::<String, _>("session_id"),
        "title": row.get::<String, _>("title"),
        "body": row.get::<String, _>("body"),
        "state": row.get::<String, _>("state"),
        "author_type": row.get::<String, _>("author_type"),
        "author_name": row.get::<String, _>("author_name"),
        "model": serde_json::from_str::<Value>(&model).unwrap_or_else(|_| json!({})),
        "sources": serde_json::from_str::<Value>(&sources).unwrap_or_else(|_| json!([])),
        "node_refs": serde_json::from_str::<Value>(&refs).unwrap_or_else(|_| json!([])),
        "reviewed_by": row.get::<String, _>("reviewed_by"),
        "review_reason": row.get::<String, _>("review_reason"),
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at")
    })
}

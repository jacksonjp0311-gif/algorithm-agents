use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::error::AgentError;
use crate::host::PublishedAlgorithm;

pub const RELATIONS: [&str; 16] = [
    "DERIVED_FROM",
    "GENERALIZES",
    "SPECIALIZES",
    "APPROXIMATES",
    "COMPOSES_WITH",
    "USES_STRUCTURE",
    "SHARES_MOTIF",
    "CONTRADICTS",
    "VALIDATED_BY",
    "EXTRACTED_DURING",
    "IMPLEMENTS",
    "EVALUATES",
    "DEPENDS_ON",
    "ANALOGOUS_TO",
    "HAS_ASSUMPTION",
    "HAS_EXPERIMENT",
];

pub const NODE_KINDS: [&str; 10] = [
    "ALGORITHM",
    "THEORY",
    "IMPLEMENTATION",
    "DOMAIN",
    "SOURCE",
    "ASSUMPTION",
    "EXPERIMENT",
    "PERSON",
    "DATASET",
    "HYPOTHESIS",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipProposal {
    pub proposal_id: String,
    pub session_id: String,
    pub from_node: String,
    pub to_node: String,
    pub relation: String,
    pub state: String,
    pub evidence: Value,
    pub proposed_by: String,
    pub reviewed_by: String,
    pub review_reason: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeDispute {
    pub dispute_id: String,
    pub target_type: String,
    pub target_id: String,
    pub state: String,
    pub claim: String,
    pub evidence: Value,
    pub opened_by: String,
    pub resolved_by: String,
    pub resolution: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn ontology() -> Value {
    json!({
        "version": "2.0",
        "node_kinds": NODE_KINDS,
        "relations": RELATIONS,
        "rules": {
            "canonical_edges_require_canonical_endpoints": true,
            "proposals_require_evidence": true,
            "model_approval_allowed": false,
            "disputes_mutate_canon": false
        }
    })
}

pub async fn upsert_canonical_algorithm(
    pool: &SqlitePool,
    algorithm: &PublishedAlgorithm,
    extraction: &Value,
    revision_id: &str,
) -> Result<(), AgentError> {
    let now = crate::persist::now_rfc3339()?;
    let mut transaction = pool.begin().await?;
    upsert_node(
        &mut transaction,
        &format!("algorithm:{}", algorithm.id),
        "ALGORITHM",
        true,
        &algorithm.title,
        extraction,
        &json!([{ "revision_id": revision_id }]),
        &now,
    )
    .await?;
    if !algorithm.domain.trim().is_empty() {
        let domain_id = format!("domain:{}", slug(&algorithm.domain));
        upsert_node(
            &mut transaction,
            &domain_id,
            "DOMAIN",
            true,
            &algorithm.domain,
            &json!({ "name": algorithm.domain }),
            &json!([{ "revision_id": revision_id }]),
            &now,
        )
        .await?;
        insert_edge(
            &mut transaction,
            &format!("algorithm:{}", algorithm.id),
            &domain_id,
            "USES_STRUCTURE",
            true,
            &json!([{ "field": "domain", "revision_id": revision_id }]),
            &now,
        )
        .await?;
    }
    if let Some(url) = extraction
        .pointer("/source/url")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
    {
        let source_id = format!("source:{}", short_hash(url));
        upsert_node(
            &mut transaction,
            &source_id,
            "SOURCE",
            true,
            extraction
                .pointer("/source/title")
                .and_then(Value::as_str)
                .unwrap_or(url),
            &json!({ "url": url, "source": extraction.get("source") }),
            &json!([{ "revision_id": revision_id }]),
            &now,
        )
        .await?;
        insert_edge(
            &mut transaction,
            &format!("algorithm:{}", algorithm.id),
            &source_id,
            "DERIVED_FROM",
            true,
            &json!([{ "field": "source.url", "revision_id": revision_id }]),
            &now,
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn propose_relationship(
    pool: &SqlitePool,
    session_id: &str,
    from_node: &str,
    to_node: &str,
    relation: &str,
    evidence: Value,
    proposed_by: &str,
) -> Result<RelationshipProposal, AgentError> {
    validate_relation(relation)?;
    if session_id.is_empty() || from_node.is_empty() || to_node.is_empty() || proposed_by.is_empty()
    {
        return Err(AgentError::Invalid(
            "session, endpoints, and proposer are required".into(),
        ));
    }
    if evidence.as_array().is_none_or(Vec::is_empty) {
        return Err(AgentError::Invalid(
            "relationship proposals require evidence".into(),
        ));
    }
    let now = crate::persist::now_rfc3339()?;
    let proposal = RelationshipProposal {
        proposal_id: format!("REL-{}", uuid::Uuid::new_v4().simple()),
        session_id: session_id.into(),
        from_node: from_node.into(),
        to_node: to_node.into(),
        relation: relation.into(),
        state: "PENDING".into(),
        evidence,
        proposed_by: proposed_by.into(),
        reviewed_by: String::new(),
        review_reason: String::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    sqlx::query(
        "INSERT INTO relationship_proposals
         (proposal_id, session_id, from_node, to_node, relation, state, evidence_json,
          proposed_by, reviewed_by, review_reason, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&proposal.proposal_id)
    .bind(&proposal.session_id)
    .bind(&proposal.from_node)
    .bind(&proposal.to_node)
    .bind(&proposal.relation)
    .bind(&proposal.state)
    .bind(serde_json::to_string(&proposal.evidence)?)
    .bind(&proposal.proposed_by)
    .bind(&proposal.reviewed_by)
    .bind(&proposal.review_reason)
    .bind(&proposal.created_at)
    .bind(&proposal.updated_at)
    .execute(pool)
    .await?;
    Ok(proposal)
}

pub async fn review_relationship(
    pool: &SqlitePool,
    proposal_id: &str,
    accept: bool,
    reviewer: &str,
    reason: &str,
) -> Result<RelationshipProposal, AgentError> {
    if reviewer.trim().is_empty() || reason.trim().len() < 4 {
        return Err(AgentError::Invalid(
            "reviewer and meaningful reason are required".into(),
        ));
    }
    let mut proposal = get_proposal(pool, proposal_id).await?;
    if proposal.state != "PENDING" {
        return Err(AgentError::State(format!(
            "relationship proposal is already {}",
            proposal.state
        )));
    }
    proposal.state = if accept { "ACCEPTED" } else { "REJECTED" }.into();
    proposal.reviewed_by = reviewer.into();
    proposal.review_reason = reason.into();
    proposal.updated_at = crate::persist::now_rfc3339()?;
    let mut transaction = pool.begin().await?;
    if accept {
        let canonical: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM knowledge_nodes WHERE node_id IN (?, ?) AND canonical = 1",
        )
        .bind(&proposal.from_node)
        .bind(&proposal.to_node)
        .fetch_one(&mut *transaction)
        .await?;
        if canonical != 2 {
            return Err(AgentError::Denied(
                "both relationship endpoints must already be canonical".into(),
            ));
        }
        insert_edge(
            &mut transaction,
            &proposal.from_node,
            &proposal.to_node,
            &proposal.relation,
            true,
            &proposal.evidence,
            &proposal.updated_at,
        )
        .await?;
    }
    sqlx::query(
        "UPDATE relationship_proposals
         SET state=?, reviewed_by=?, review_reason=?, updated_at=? WHERE proposal_id=?",
    )
    .bind(&proposal.state)
    .bind(&proposal.reviewed_by)
    .bind(&proposal.review_reason)
    .bind(&proposal.updated_at)
    .bind(&proposal.proposal_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(proposal)
}

pub async fn graph_snapshot(pool: &SqlitePool, include_private: bool) -> Result<Value, AgentError> {
    let node_sql = if include_private {
        "SELECT * FROM knowledge_nodes ORDER BY kind, label"
    } else {
        "SELECT * FROM knowledge_nodes WHERE canonical=1 ORDER BY kind, label"
    };
    let edge_sql = if include_private {
        "SELECT * FROM knowledge_edges ORDER BY created_at"
    } else {
        "SELECT * FROM knowledge_edges WHERE canonical=1 ORDER BY created_at"
    };
    let nodes = sqlx::query(node_sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            json!({
                "id": row.get::<String, _>("node_id"),
                "kind": row.get::<String, _>("kind"),
                "canonical": row.get::<i64, _>("canonical") != 0,
                "label": row.get::<String, _>("label"),
                "body": parse_json(row.get::<String, _>("body_json")),
                "provenance": parse_json(row.get::<String, _>("provenance_json"))
            })
        })
        .collect::<Vec<_>>();
    let edges = sqlx::query(edge_sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            json!({
                "id": row.get::<String, _>("edge_id"),
                "from": row.get::<String, _>("from_node"),
                "to": row.get::<String, _>("to_node"),
                "relation": row.get::<String, _>("relation"),
                "canonical": row.get::<i64, _>("canonical") != 0,
                "evidence": parse_json(row.get::<String, _>("evidence_json"))
            })
        })
        .collect::<Vec<_>>();
    let disputes = list_disputes(pool).await?;
    Ok(json!({
        "ontology": ontology(),
        "nodes": nodes,
        "edges": edges,
        "disputes": disputes.into_iter().filter(|item| include_private || item.state == "OPEN").collect::<Vec<_>>()
    }))
}

pub async fn open_dispute(
    pool: &SqlitePool,
    target_type: &str,
    target_id: &str,
    claim: &str,
    evidence: Value,
    opened_by: &str,
) -> Result<KnowledgeDispute, AgentError> {
    if !matches!(target_type, "NODE" | "EDGE")
        || target_id.trim().is_empty()
        || claim.trim().len() < 8
        || opened_by.trim().is_empty()
        || evidence.as_array().is_none_or(Vec::is_empty)
    {
        return Err(AgentError::Invalid(
            "disputes require NODE/EDGE target, claim, evidence, and opener".into(),
        ));
    }
    let now = crate::persist::now_rfc3339()?;
    let dispute = KnowledgeDispute {
        dispute_id: format!("DSP-{}", uuid::Uuid::new_v4().simple()),
        target_type: target_type.into(),
        target_id: target_id.into(),
        state: "OPEN".into(),
        claim: claim.into(),
        evidence,
        opened_by: opened_by.into(),
        resolved_by: String::new(),
        resolution: String::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    sqlx::query(
        "INSERT INTO knowledge_disputes
         (dispute_id, target_type, target_id, state, claim, evidence_json, opened_by,
          resolved_by, resolution, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&dispute.dispute_id)
    .bind(&dispute.target_type)
    .bind(&dispute.target_id)
    .bind(&dispute.state)
    .bind(&dispute.claim)
    .bind(serde_json::to_string(&dispute.evidence)?)
    .bind(&dispute.opened_by)
    .bind(&dispute.resolved_by)
    .bind(&dispute.resolution)
    .bind(&dispute.created_at)
    .bind(&dispute.updated_at)
    .execute(pool)
    .await?;
    Ok(dispute)
}

pub async fn resolve_dispute(
    pool: &SqlitePool,
    dispute_id: &str,
    reviewer: &str,
    resolution: &str,
) -> Result<KnowledgeDispute, AgentError> {
    if reviewer.trim().is_empty() || resolution.trim().len() < 8 {
        return Err(AgentError::Invalid(
            "dispute resolution requires reviewer and meaningful resolution".into(),
        ));
    }
    let now = crate::persist::now_rfc3339()?;
    let changed = sqlx::query(
        "UPDATE knowledge_disputes SET state='RESOLVED', resolved_by=?, resolution=?, updated_at=?
         WHERE dispute_id=? AND state='OPEN'",
    )
    .bind(reviewer)
    .bind(resolution)
    .bind(&now)
    .bind(dispute_id)
    .execute(pool)
    .await?;
    if changed.rows_affected() != 1 {
        return Err(AgentError::State(format!(
            "dispute `{dispute_id}` is missing or not open"
        )));
    }
    get_dispute(pool, dispute_id).await
}

pub async fn list_disputes(pool: &SqlitePool) -> Result<Vec<KnowledgeDispute>, AgentError> {
    let rows = sqlx::query("SELECT * FROM knowledge_disputes ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(dispute_from_row).collect())
}

async fn get_dispute(pool: &SqlitePool, dispute_id: &str) -> Result<KnowledgeDispute, AgentError> {
    let row = sqlx::query("SELECT * FROM knowledge_disputes WHERE dispute_id=?")
        .bind(dispute_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("dispute `{dispute_id}` not found")))?;
    Ok(dispute_from_row(row))
}

fn dispute_from_row(row: sqlx::sqlite::SqliteRow) -> KnowledgeDispute {
    KnowledgeDispute {
        dispute_id: row.get("dispute_id"),
        target_type: row.get("target_type"),
        target_id: row.get("target_id"),
        state: row.get("state"),
        claim: row.get("claim"),
        evidence: parse_json(row.get("evidence_json")),
        opened_by: row.get("opened_by"),
        resolved_by: row.get("resolved_by"),
        resolution: row.get("resolution"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub async fn integrity_report(pool: &SqlitePool) -> Result<Value, AgentError> {
    let missing_endpoints: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM knowledge_edges e
         LEFT JOIN knowledge_nodes f ON f.node_id=e.from_node
         LEFT JOIN knowledge_nodes t ON t.node_id=e.to_node
         WHERE f.node_id IS NULL OR t.node_id IS NULL",
    )
    .fetch_one(pool)
    .await?;
    let noncanonical_endpoints: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM knowledge_edges e
         JOIN knowledge_nodes f ON f.node_id=e.from_node
         JOIN knowledge_nodes t ON t.node_id=e.to_node
         WHERE e.canonical=1 AND (f.canonical=0 OR t.canonical=0)",
    )
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query("SELECT relation FROM knowledge_edges")
        .fetch_all(pool)
        .await?;
    let invalid_relations = rows
        .into_iter()
        .map(|row| row.get::<String, _>("relation"))
        .filter(|relation| !RELATIONS.contains(&relation.as_str()))
        .collect::<Vec<_>>();
    Ok(json!({
        "valid": missing_endpoints == 0 && noncanonical_endpoints == 0 && invalid_relations.is_empty(),
        "missing_endpoints": missing_endpoints,
        "canonical_edges_with_private_endpoints": noncanonical_endpoints,
        "invalid_relations": invalid_relations,
        "ontology_version": "2.0"
    }))
}

pub async fn list_proposals(pool: &SqlitePool) -> Result<Vec<RelationshipProposal>, AgentError> {
    let rows = sqlx::query("SELECT * FROM relationship_proposals ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(proposal_from_row).collect())
}

async fn get_proposal(
    pool: &SqlitePool,
    proposal_id: &str,
) -> Result<RelationshipProposal, AgentError> {
    let row = sqlx::query("SELECT * FROM relationship_proposals WHERE proposal_id=?")
        .bind(proposal_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("proposal `{proposal_id}` not found")))?;
    Ok(proposal_from_row(row))
}

fn proposal_from_row(row: sqlx::sqlite::SqliteRow) -> RelationshipProposal {
    RelationshipProposal {
        proposal_id: row.get("proposal_id"),
        session_id: row.get("session_id"),
        from_node: row.get("from_node"),
        to_node: row.get("to_node"),
        relation: row.get("relation"),
        state: row.get("state"),
        evidence: parse_json(row.get("evidence_json")),
        proposed_by: row.get("proposed_by"),
        reviewed_by: row.get("reviewed_by"),
        review_reason: row.get("review_reason"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn upsert_node(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    node_id: &str,
    kind: &str,
    canonical: bool,
    label: &str,
    body: &Value,
    provenance: &Value,
    now: &str,
) -> Result<(), AgentError> {
    sqlx::query(
        "INSERT INTO knowledge_nodes
         (node_id, kind, canonical, label, body_json, provenance_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(node_id) DO UPDATE SET kind=excluded.kind, canonical=excluded.canonical,
         label=excluded.label, body_json=excluded.body_json,
         provenance_json=excluded.provenance_json, updated_at=excluded.updated_at",
    )
    .bind(node_id)
    .bind(kind)
    .bind(i64::from(canonical))
    .bind(label)
    .bind(serde_json::to_string(body)?)
    .bind(serde_json::to_string(provenance)?)
    .bind(now)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_edge(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    from_node: &str,
    to_node: &str,
    relation: &str,
    canonical: bool,
    evidence: &Value,
    now: &str,
) -> Result<(), AgentError> {
    validate_relation(relation)?;
    let edge_id = format!(
        "edge:{}",
        short_hash(&format!("{from_node}|{relation}|{to_node}"))
    );
    sqlx::query(
        "INSERT INTO knowledge_edges
         (edge_id, from_node, to_node, relation, canonical, evidence_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(edge_id) DO UPDATE SET canonical=excluded.canonical,
         evidence_json=excluded.evidence_json",
    )
    .bind(edge_id)
    .bind(from_node)
    .bind(to_node)
    .bind(relation)
    .bind(i64::from(canonical))
    .bind(serde_json::to_string(evidence)?)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn validate_relation(relation: &str) -> Result<(), AgentError> {
    if RELATIONS.contains(&relation) {
        Ok(())
    } else {
        Err(AgentError::Invalid(format!(
            "unsupported graph relation `{relation}`"
        )))
    }
}

fn parse_json(raw: String) -> Value {
    serde_json::from_str(&raw).unwrap_or(Value::Null)
}

fn short_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
        .chars()
        .take(24)
        .collect()
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

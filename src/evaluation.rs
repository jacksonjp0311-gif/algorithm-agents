use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use sqlx_sqlite::SqlitePool;

use crate::error::AgentError;
use crate::parse::{contains_claim, parse_all_documents};
use crate::persist;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Corpus {
    version: String,
    cases: Vec<GoldCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoldCase {
    id: String,
    fixture: String,
    title_contains: String,
    domain: String,
    algorithm_present: bool,
    expected_claims: Vec<String>,
}

pub async fn run_corpus(pool: &SqlitePool, root: &Path) -> Result<Value, AgentError> {
    let raw = tokio::fs::read_to_string(root.join("eval").join("corpus.json")).await?;
    let corpus: Corpus = serde_json::from_str(&raw)?;
    if corpus.cases.is_empty() {
        return Err(AgentError::Invalid("evaluation corpus is empty".into()));
    }
    let mut results = Vec::new();
    let mut title_hits = 0usize;
    let mut domain_hits = 0usize;
    let mut detection_hits = 0usize;
    let mut claim_hits = 0usize;
    let mut claim_total = 0usize;
    for case in &corpus.cases {
        let fixture_path = root.join(&case.fixture);
        let source = tokio::fs::read_to_string(&fixture_path).await?;
        let doc = parse_all_documents(&source)
            .into_iter()
            .next()
            .ok_or_else(|| {
                AgentError::Schema(format!("{} has no primary document", case.fixture))
            })?;
        let title_ok = doc
            .title
            .to_ascii_lowercase()
            .contains(&case.title_contains.to_ascii_lowercase());
        let domain_ok = doc.domain.eq_ignore_ascii_case(&case.domain);
        let detection_ok = doc.algorithm_present.unwrap_or(false) == case.algorithm_present;
        let evidenced = case
            .expected_claims
            .iter()
            .filter(|claim| contains_claim(&source, claim))
            .count();
        title_hits += usize::from(title_ok);
        domain_hits += usize::from(domain_ok);
        detection_hits += usize::from(detection_ok);
        claim_hits += evidenced;
        claim_total += case.expected_claims.len();
        results.push(json!({
            "id": case.id,
            "fixture": case.fixture,
            "title": doc.title,
            "domain": doc.domain,
            "title_ok": title_ok,
            "domain_ok": domain_ok,
            "detection_ok": detection_ok,
            "evidenced_claims": evidenced,
            "expected_claims": case.expected_claims.len(),
            "passed": title_ok && domain_ok && detection_ok && evidenced == case.expected_claims.len()
        }));
    }
    let total = corpus.cases.len() as f64;
    let metrics = json!({
        "case_count": corpus.cases.len(),
        "title_accuracy": title_hits as f64 / total,
        "domain_accuracy": domain_hits as f64 / total,
        "algorithm_detection_accuracy": detection_hits as f64 / total,
        "evidence_coverage": if claim_total == 0 { 0.0 } else { claim_hits as f64 / claim_total as f64 },
        "passed_cases": results.iter().filter(|case| case.get("passed") == Some(&Value::Bool(true))).count()
    });
    let passed = metrics
        .get("title_accuracy")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        >= 1.0
        && metrics
            .get("domain_accuracy")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            >= 1.0
        && metrics
            .get("algorithm_detection_accuracy")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            >= 1.0
        && metrics
            .get("evidence_coverage")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            >= 1.0;
    let evaluation_id = format!("EVAL-{}", uuid::Uuid::new_v4().simple());
    let report = json!({
        "schema_version": "1.0",
        "evaluation_id": evaluation_id,
        "corpus_version": corpus.version,
        "runtime_version": env!("CARGO_PKG_VERSION"),
        "metrics": metrics,
        "cases": results,
        "passed": passed,
        "created_at": persist::now_rfc3339()?
    });
    sqlx::query(
        "INSERT INTO evaluation_runs
         (evaluation_id, corpus_version, runtime_version, metrics_json, cases_json, passed, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(report.get("evaluation_id").and_then(Value::as_str).unwrap_or(""))
    .bind(report.get("corpus_version").and_then(Value::as_str).unwrap_or(""))
    .bind(env!("CARGO_PKG_VERSION"))
    .bind(serde_json::to_string(&metrics)?)
    .bind(serde_json::to_string(report.get("cases").unwrap_or(&Value::Null))?)
    .bind(i64::from(passed))
    .bind(report.get("created_at").and_then(Value::as_str).unwrap_or(""))
    .execute(pool)
    .await?;
    Ok(report)
}

pub async fn history(pool: &SqlitePool) -> Result<Vec<Value>, AgentError> {
    let rows = sqlx::query("SELECT * FROM evaluation_runs ORDER BY created_at DESC LIMIT 50")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let metrics: String = row.get("metrics_json");
            json!({
                "evaluation_id": row.get::<String, _>("evaluation_id"),
                "corpus_version": row.get::<String, _>("corpus_version"),
                "runtime_version": row.get::<String, _>("runtime_version"),
                "metrics": serde_json::from_str::<Value>(&metrics).unwrap_or(Value::Null),
                "passed": row.get::<i64, _>("passed") != 0,
                "created_at": row.get::<String, _>("created_at")
            })
        })
        .collect())
}

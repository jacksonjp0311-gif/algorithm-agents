use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::archive::FileArchive;
use crate::error::AgentError;
use crate::persist;

pub async fn export(
    archive: &FileArchive,
    pool: &SqlitePool,
    output: &Path,
) -> Result<Value, AgentError> {
    if output.exists() {
        return Err(AgentError::Denied(format!(
            "refusing to overwrite existing bundle {}",
            output.display()
        )));
    }
    let payload = json!({
        "format": "alchetron.bundle",
        "version": "2.0",
        "created_at": persist::now_rfc3339()?,
        "catalog": archive.catalog().await?,
        "graph": crate::graph::graph_snapshot(pool, false).await?,
        "research_logs": crate::emergent::list_research_logs(pool, false).await?,
        "publication_receipts": persist::list_publication_receipts(pool).await?,
        "archive_integrity": archive.integrity_report().await?,
        "graph_integrity": crate::graph::integrity_report(pool).await?
    });
    let payload_hash = hash_json(&payload)?;
    let bundle = json!({
        "payload_hash": payload_hash,
        "payload": payload
    });
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temporary = output.with_extension("tmp");
    tokio::fs::write(&temporary, serde_json::to_vec_pretty(&bundle)?).await?;
    tokio::fs::rename(&temporary, output).await?;
    Ok(json!({
        "output": output,
        "payload_hash": payload_hash,
        "canonical": false,
        "portable": true
    }))
}

pub async fn inspect(input: &Path) -> Result<Value, AgentError> {
    let raw = tokio::fs::read_to_string(input).await?;
    let bundle: Value = serde_json::from_str(&raw)?;
    let payload = bundle
        .get("payload")
        .ok_or_else(|| AgentError::Schema("bundle payload missing".into()))?;
    let expected = bundle
        .get("payload_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    let actual = hash_json(payload)?;
    Ok(json!({
        "valid": !expected.is_empty() && expected == actual,
        "expected_hash": expected,
        "actual_hash": actual,
        "format": payload.get("format"),
        "version": payload.get("version"),
        "created_at": payload.get("created_at")
    }))
}

pub async fn import_private(
    pool: &SqlitePool,
    input: &Path,
    source: &str,
) -> Result<Value, AgentError> {
    let inspection = inspect(input).await?;
    if inspection.get("valid") != Some(&Value::Bool(true)) {
        return Err(AgentError::Schema("bundle hash verification failed".into()));
    }
    let raw = tokio::fs::read_to_string(input).await?;
    let body: Value = serde_json::from_str(&raw)?;
    let bundle_id = format!("BND-{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO imported_bundles
         (bundle_id, content_hash, state, source, body_json, created_at)
         VALUES (?, ?, 'PRIVATE_REVIEW', ?, ?, ?)",
    )
    .bind(&bundle_id)
    .bind(
        inspection
            .get("actual_hash")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )
    .bind(source)
    .bind(serde_json::to_string(&body)?)
    .bind(persist::now_rfc3339()?)
    .execute(pool)
    .await?;
    Ok(json!({
        "bundle_id": bundle_id,
        "state": "PRIVATE_REVIEW",
        "canonical_changes": 0,
        "message": "imported data requires separate candidate and relationship review"
    }))
}

fn hash_json(value: &Value) -> Result<String, AgentError> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

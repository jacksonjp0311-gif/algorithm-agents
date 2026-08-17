use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::AgentError;

pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (
        1,
        "initial runtime schema",
        include_str!("../sql/migrations/0001-initial.sql"),
    ),
    (
        2,
        "governed archive graph and supervisor audit",
        include_str!("../sql/migrations/0002-governance.sql"),
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub objective: String,
    pub state: String,
    pub supervisor: String,
    pub authorized_agents: Vec<String>,
    pub authorized_tools: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub max_sources: i64,
    pub max_candidates: i64,
    pub max_agent_runs: i64,
    pub max_runtime_seconds: i64,
    pub max_retries: i64,
    pub max_artifact_bytes: i64,
    pub sources_used: i64,
    pub candidates_used: i64,
    pub agent_runs_used: i64,
    pub publication_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub run_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub agent_version: String,
    pub state: String,
    pub reason: String,
    pub input: Value,
    pub output: Value,
    pub artifact_id: String,
    pub queued_at: String,
    pub started_at: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub message_id: String,
    pub session_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub r#type: String,
    pub artifact_id: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event_id: String,
    pub session_id: String,
    pub kind: String,
    pub meta: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMeta {
    pub artifact_id: String,
    pub session_id: String,
    pub creator: String,
    pub agent_id: String,
    pub agent_version: String,
    pub r#type: String,
    pub created_at: String,
    pub content_hash: String,
    pub schema_version: String,
    pub path: String,
    pub provenance_refs: Vec<String>,
}

pub fn now_rfc3339() -> Result<String, AgentError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AgentError::Internal(error.to_string()))
}

pub async fn connect_sqlite(database_url: &str) -> Result<SqlitePool, AgentError> {
    if let Some(path) = database_url.strip_prefix("sqlite://") {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    let options = database_url
        .parse::<SqliteConnectOptions>()
        .map_err(|error| AgentError::Invalid(error.to_string()))?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePool::connect_with(options).await?;
    apply_schema(&pool).await?;
    Ok(pool)
}

pub async fn apply_schema(pool: &SqlitePool) -> Result<(), AgentError> {
    sqlx::raw_sql(SCHEMA_SQL).execute(pool).await?;
    for (version, name, sql) in MIGRATIONS {
        let applied: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations WHERE version = ?")
                .bind(version)
                .fetch_one(pool)
                .await?;
        if applied == 0 {
            let mut transaction = pool.begin().await?;
            sqlx::raw_sql(sql).execute(&mut *transaction).await?;
            sqlx::query(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, ?)",
            )
            .bind(version)
            .bind(name)
            .bind(now_rfc3339()?)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
        }
    }
    Ok(())
}

pub async fn next_id(pool: &SqlitePool, kind: &str, prefix: &str) -> Result<String, AgentError> {
    let row = sqlx::query(
        "INSERT INTO agent_id_counters (kind, next_value) VALUES (?, 2)
         ON CONFLICT(kind) DO UPDATE SET next_value = agent_id_counters.next_value + 1
         RETURNING next_value - 1 AS assigned",
    )
    .bind(kind)
    .fetch_one(pool)
    .await?;
    let next: i64 = row.get("assigned");
    Ok(format!("{prefix}{next:04}"))
}

pub async fn insert_session(pool: &SqlitePool, session: &Session) -> Result<(), AgentError> {
    sqlx::query(
        "INSERT INTO agent_sessions (
            session_id, objective, state, supervisor, authorized_agents_json, authorized_tools_json,
            created_at, updated_at, max_sources, max_candidates, max_agent_runs, max_runtime_seconds,
            max_retries, max_artifact_bytes, sources_used, candidates_used, agent_runs_used, publication_allowed
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session.session_id)
    .bind(&session.objective)
    .bind(&session.state)
    .bind(&session.supervisor)
    .bind(serde_json::to_string(&session.authorized_agents)?)
    .bind(serde_json::to_string(&session.authorized_tools)?)
    .bind(&session.created_at)
    .bind(&session.updated_at)
    .bind(session.max_sources)
    .bind(session.max_candidates)
    .bind(session.max_agent_runs)
    .bind(session.max_runtime_seconds)
    .bind(session.max_retries)
    .bind(session.max_artifact_bytes)
    .bind(session.sources_used)
    .bind(session.candidates_used)
    .bind(session.agent_runs_used)
    .bind(i64::from(session.publication_allowed))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_session(pool: &SqlitePool, session: &Session) -> Result<(), AgentError> {
    sqlx::query(
        "UPDATE agent_sessions SET objective=?, state=?, supervisor=?, authorized_agents_json=?,
         authorized_tools_json=?, updated_at=?, max_sources=?, max_candidates=?, max_agent_runs=?,
         max_runtime_seconds=?, max_retries=?, max_artifact_bytes=?, sources_used=?, candidates_used=?,
         agent_runs_used=?, publication_allowed=? WHERE session_id=?",
    )
    .bind(&session.objective)
    .bind(&session.state)
    .bind(&session.supervisor)
    .bind(serde_json::to_string(&session.authorized_agents)?)
    .bind(serde_json::to_string(&session.authorized_tools)?)
    .bind(&session.updated_at)
    .bind(session.max_sources)
    .bind(session.max_candidates)
    .bind(session.max_agent_runs)
    .bind(session.max_runtime_seconds)
    .bind(session.max_retries)
    .bind(session.max_artifact_bytes)
    .bind(session.sources_used)
    .bind(session.candidates_used)
    .bind(session.agent_runs_used)
    .bind(i64::from(session.publication_allowed))
    .bind(&session.session_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_session(pool: &SqlitePool, session_id: &str) -> Result<Session, AgentError> {
    let row = sqlx::query("SELECT * FROM agent_sessions WHERE session_id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("session `{session_id}` not found")))?;
    Ok(session_from_row(&row))
}

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<Session>, AgentError> {
    let rows = sqlx::query("SELECT * FROM agent_sessions ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(session_from_row).collect())
}

fn session_from_row(row: &sqlx::sqlite::SqliteRow) -> Session {
    let agents: String = row.get("authorized_agents_json");
    let tools: String = row.get("authorized_tools_json");
    Session {
        session_id: row.get("session_id"),
        objective: row.get("objective"),
        state: row.get("state"),
        supervisor: row.get("supervisor"),
        authorized_agents: serde_json::from_str(&agents).unwrap_or_default(),
        authorized_tools: serde_json::from_str(&tools).unwrap_or_default(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        max_sources: row.get("max_sources"),
        max_candidates: row.get("max_candidates"),
        max_agent_runs: row.get("max_agent_runs"),
        max_runtime_seconds: row.get("max_runtime_seconds"),
        max_retries: row.get("max_retries"),
        max_artifact_bytes: row.get("max_artifact_bytes"),
        sources_used: row.get("sources_used"),
        candidates_used: row.get("candidates_used"),
        agent_runs_used: row.get("agent_runs_used"),
        publication_allowed: {
            let flag: i64 = row.get("publication_allowed");
            flag != 0
        },
    }
}

pub async fn insert_run(pool: &SqlitePool, run: &AgentRun) -> Result<(), AgentError> {
    sqlx::query(
        "INSERT INTO agent_runs (run_id, session_id, agent_id, agent_version, state, reason, input_json, output_json, artifact_id, queued_at, started_at, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run.run_id)
    .bind(&run.session_id)
    .bind(&run.agent_id)
    .bind(&run.agent_version)
    .bind(&run.state)
    .bind(&run.reason)
    .bind(serde_json::to_string(&run.input)?)
    .bind(serde_json::to_string(&run.output)?)
    .bind(&run.artifact_id)
    .bind(&run.queued_at)
    .bind(&run.started_at)
    .bind(&run.completed_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_run(pool: &SqlitePool, run: &AgentRun) -> Result<(), AgentError> {
    sqlx::query(
        "UPDATE agent_runs SET state=?, reason=?, input_json=?, output_json=?, artifact_id=?, started_at=?, completed_at=? WHERE run_id=?",
    )
    .bind(&run.state)
    .bind(&run.reason)
    .bind(serde_json::to_string(&run.input)?)
    .bind(serde_json::to_string(&run.output)?)
    .bind(&run.artifact_id)
    .bind(&run.started_at)
    .bind(&run.completed_at)
    .bind(&run.run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_run(pool: &SqlitePool, run_id: &str) -> Result<AgentRun, AgentError> {
    let row = sqlx::query("SELECT * FROM agent_runs WHERE run_id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("run `{run_id}` not found")))?;
    Ok(run_from_row(&row))
}

pub async fn list_runs(pool: &SqlitePool, session_id: &str) -> Result<Vec<AgentRun>, AgentError> {
    let rows = sqlx::query("SELECT * FROM agent_runs WHERE session_id = ? ORDER BY queued_at")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(run_from_row).collect())
}

fn run_from_row(row: &sqlx::sqlite::SqliteRow) -> AgentRun {
    let input: String = row.get("input_json");
    let output: String = row.get("output_json");
    AgentRun {
        run_id: row.get("run_id"),
        session_id: row.get("session_id"),
        agent_id: row.get("agent_id"),
        agent_version: row.get("agent_version"),
        state: row.get("state"),
        reason: row.get("reason"),
        input: serde_json::from_str(&input).unwrap_or_else(|_| json!({})),
        output: serde_json::from_str(&output).unwrap_or_else(|_| json!({})),
        artifact_id: row.get("artifact_id"),
        queued_at: row.get("queued_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
    }
}

pub async fn insert_message(pool: &SqlitePool, message: &AgentMessage) -> Result<(), AgentError> {
    sqlx::query(
        "INSERT INTO agent_messages (message_id, session_id, from_agent, to_agent, type, artifact_id, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&message.message_id)
    .bind(&message.session_id)
    .bind(&message.from_agent)
    .bind(&message.to_agent)
    .bind(&message.r#type)
    .bind(&message.artifact_id)
    .bind(serde_json::to_string(&message.payload)?)
    .bind(&message.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_messages(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<AgentMessage>, AgentError> {
    let rows = sqlx::query("SELECT * FROM agent_messages WHERE session_id = ? ORDER BY created_at")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let payload: String = row.get("payload_json");
            AgentMessage {
                message_id: row.get("message_id"),
                session_id: row.get("session_id"),
                from_agent: row.get("from_agent"),
                to_agent: row.get("to_agent"),
                r#type: row.get("type"),
                artifact_id: row.get("artifact_id"),
                payload: serde_json::from_str(&payload).unwrap_or_else(|_| json!({})),
                created_at: row.get("created_at"),
            }
        })
        .collect())
}

pub async fn insert_event(
    pool: &SqlitePool,
    session_id: &str,
    kind: &str,
    meta: Value,
) -> Result<AgentEvent, AgentError> {
    let event = AgentEvent {
        event_id: format!("EVT-{}", uuid::Uuid::new_v4().simple()),
        session_id: session_id.to_owned(),
        kind: kind.to_owned(),
        meta,
        created_at: now_rfc3339()?,
    };
    sqlx::query(
        "INSERT INTO agent_events (event_id, session_id, kind, meta_json, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&event.event_id)
    .bind(&event.session_id)
    .bind(&event.kind)
    .bind(serde_json::to_string(&event.meta)?)
    .bind(&event.created_at)
    .execute(pool)
    .await?;
    Ok(event)
}

pub async fn list_events(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<AgentEvent>, AgentError> {
    let rows = if session_id.is_empty() {
        sqlx::query("SELECT * FROM agent_events ORDER BY created_at")
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM agent_events WHERE session_id = ? ORDER BY created_at")
            .bind(session_id)
            .fetch_all(pool)
            .await?
    };
    Ok(rows
        .iter()
        .map(|row| {
            let meta: String = row.get("meta_json");
            AgentEvent {
                event_id: row.get("event_id"),
                session_id: row.get("session_id"),
                kind: row.get("kind"),
                meta: serde_json::from_str(&meta).unwrap_or_else(|_| json!({})),
                created_at: row.get("created_at"),
            }
        })
        .collect())
}

pub async fn insert_artifact(
    pool: &SqlitePool,
    data_dir: &Path,
    meta: &mut ArtifactMeta,
    content: &Value,
    max_bytes: i64,
) -> Result<ArtifactMeta, AgentError> {
    let bytes = serde_json::to_vec_pretty(content)?;
    let effective_limit = max_bytes.clamp(1, 8_000_000);
    if bytes.len() as i64 > effective_limit {
        return Err(AgentError::Budget("artifact exceeds hard size cap".into()));
    }
    meta.content_hash = format!("{:x}", Sha256::digest(&bytes));
    let dir = data_dir.join("artifacts").join(&meta.session_id);
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!(
        "{}.json",
        meta.artifact_id.replace([' ', '/'], "_")
    ));
    let tmp = path.with_extension("tmp.json");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &path).await?;
    meta.path = path.to_string_lossy().into_owned();
    let inserted = sqlx::query(
        "INSERT INTO agent_artifacts (artifact_id, session_id, creator, agent_id, agent_version, type, created_at, content_hash, schema_version, path, provenance_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&meta.artifact_id)
    .bind(&meta.session_id)
    .bind(&meta.creator)
    .bind(&meta.agent_id)
    .bind(&meta.agent_version)
    .bind(&meta.r#type)
    .bind(&meta.created_at)
    .bind(&meta.content_hash)
    .bind(&meta.schema_version)
    .bind(&meta.path)
    .bind(serde_json::to_string(&meta.provenance_refs)?)
    .execute(pool)
    .await;
    if let Err(error) = inserted {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(AgentError::from(error));
    }
    Ok(meta.clone())
}

pub async fn get_artifact(
    pool: &SqlitePool,
    artifact_id: &str,
) -> Result<(ArtifactMeta, Value), AgentError> {
    let row = sqlx::query("SELECT * FROM agent_artifacts WHERE artifact_id = ?")
        .bind(artifact_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("artifact `{artifact_id}` not found")))?;
    let provenance: String = row.get("provenance_json");
    let meta = ArtifactMeta {
        artifact_id: row.get("artifact_id"),
        session_id: row.get("session_id"),
        creator: row.get("creator"),
        agent_id: row.get("agent_id"),
        agent_version: row.get("agent_version"),
        r#type: row.get("type"),
        created_at: row.get("created_at"),
        content_hash: row.get("content_hash"),
        schema_version: row.get("schema_version"),
        path: row.get("path"),
        provenance_refs: serde_json::from_str(&provenance).unwrap_or_default(),
    };
    let raw = tokio::fs::read_to_string(&meta.path).await?;
    let content = serde_json::from_str(&raw)?;
    Ok((meta, content))
}

pub async fn list_artifacts(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<ArtifactMeta>, AgentError> {
    let rows =
        sqlx::query("SELECT * FROM agent_artifacts WHERE session_id = ? ORDER BY created_at")
            .bind(session_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let provenance: String = row.get("provenance_json");
            ArtifactMeta {
                artifact_id: row.get("artifact_id"),
                session_id: row.get("session_id"),
                creator: row.get("creator"),
                agent_id: row.get("agent_id"),
                agent_version: row.get("agent_version"),
                r#type: row.get("type"),
                created_at: row.get("created_at"),
                content_hash: row.get("content_hash"),
                schema_version: row.get("schema_version"),
                path: row.get("path"),
                provenance_refs: serde_json::from_str(&provenance).unwrap_or_default(),
            }
        })
        .collect())
}

pub async fn save_receipt(
    pool: &SqlitePool,
    session_id: &str,
    body: &Value,
) -> Result<String, AgentError> {
    let receipt_id = format!("REC-{session_id}");
    let created_at = now_rfc3339()?;
    sqlx::query(
        "INSERT OR REPLACE INTO agent_receipts (receipt_id, session_id, body_json, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&receipt_id)
    .bind(session_id)
    .bind(serde_json::to_string(body)?)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(receipt_id)
}

pub async fn get_receipt(pool: &SqlitePool, session_id: &str) -> Result<Value, AgentError> {
    let row = sqlx::query("SELECT body_json FROM agent_receipts WHERE session_id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AgentError::NotFound(format!("receipt for `{session_id}` not found")))?;
    let body: String = row.get("body_json");
    Ok(serde_json::from_str(&body)?)
}

pub async fn save_publication_receipt(pool: &SqlitePool, body: &Value) -> Result<(), AgentError> {
    sqlx::query(
        "INSERT INTO archive_publication_receipts
         (receipt_id, candidate_id, session_id, revision_id, action, reviewer, reason,
          candidate_hash, canonical_hash, body_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(required_json_str(body, "receipt_id")?)
    .bind(json_str(body, "candidate_id"))
    .bind(json_str(body, "session_id"))
    .bind(required_json_str(body, "revision_id")?)
    .bind(required_json_str(body, "action")?)
    .bind(required_json_str(body, "reviewer")?)
    .bind(required_json_str(body, "reason")?)
    .bind(json_str(body, "candidate_hash"))
    .bind(required_json_str(body, "canonical_hash")?)
    .bind(serde_json::to_string(body)?)
    .bind(required_json_str(body, "created_at")?)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_publication_receipts(pool: &SqlitePool) -> Result<Vec<Value>, AgentError> {
    let rows =
        sqlx::query("SELECT body_json FROM archive_publication_receipts ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?;
    rows.into_iter()
        .map(|row| {
            let raw: String = row.get("body_json");
            serde_json::from_str(&raw).map_err(AgentError::from)
        })
        .collect()
}

fn json_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn required_json_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, AgentError> {
    let field = json_str(value, key);
    if field.is_empty() {
        Err(AgentError::Invalid(format!(
            "receipt field `{key}` is required"
        )))
    } else {
        Ok(field)
    }
}

pub fn default_data_dir(cwd: &Path) -> PathBuf {
    cwd.join("data")
}

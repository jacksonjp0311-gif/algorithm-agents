use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::agents::{self, ExecContext, HandlerResult};
use crate::error::AgentError;
use crate::host::ArchiveHost;
use crate::permissions::Permissions;
use crate::persist::{self, AgentMessage, AgentRun, ArtifactMeta, Session};
use crate::registry::Registry;
use crate::schema::validate_against_schema;

pub const MESSAGE_TYPES: [&str; 9] = [
    "ANALYSIS_REQUEST",
    "ANALYSIS_RESULT",
    "VERIFICATION_REQUEST",
    "VERIFICATION_RESULT",
    "CLARIFICATION_REQUEST",
    "CLARIFICATION_RESULT",
    "ARTIFACT_REFERENCE",
    "BLOCKED_NOTICE",
    "LOW_CONFIDENCE_NOTICE",
];

pub const SESSION_STATES: [&str; 7] = [
    "CREATED", "ACTIVE", "PAUSED", "BLOCKED", "COMPLETE", "FAILED", "CANCELLED",
];

pub struct AgentRuntime {
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub pool: SqlitePool,
    pub registry: Registry,
    pub permissions: Permissions,
    pub host: Arc<dyn ArchiveHost>,
}

impl AgentRuntime {
    pub async fn open(
        root: PathBuf,
        data_dir: PathBuf,
        pool: SqlitePool,
        host: Arc<dyn ArchiveHost>,
    ) -> Result<Self, AgentError> {
        persist::apply_schema(&pool).await?;
        tokio::fs::create_dir_all(&data_dir).await?;
        let registry = Registry::load(&root)?;
        let permissions = Permissions::load(&root)?;
        persist::insert_event(
            &pool,
            "",
            "RUNTIME_STARTED",
            json!({
                "valid_agents": registry.agents.len(),
                "invalid_agents": registry.invalid.len()
            }),
        )
        .await?;
        Ok(Self {
            root,
            data_dir,
            pool,
            registry,
            permissions,
            host,
        })
    }

    pub fn discover_root(hint: Option<&Path>) -> PathBuf {
        if let Ok(value) = std::env::var("ALG_AGENT_ROOT")
            .or_else(|_| std::env::var("ALGORITHM_AGENTS_ROOT"))
            .or_else(|_| std::env::var("JACKSON_AGENT_SYSTEM_ROOT"))
        {
            return PathBuf::from(value);
        }
        if let Some(path) = hint {
            if path.join("registry/agents.json").exists() {
                return path.to_path_buf();
            }
            if path.join("agent_system/registry/agents.json").exists() {
                return path.join("agent_system");
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            if cwd.join("registry/agents.json").exists() {
                return cwd;
            }
            if cwd.join("agent_system/registry/agents.json").exists() {
                return cwd.join("agent_system");
            }
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    pub async fn start_session(&self, args: Value) -> Result<Session, AgentError> {
        let now = persist::now_rfc3339()?;
        let session = Session {
            session_id: persist::next_id(&self.pool, "session", "RS-").await?,
            objective: args
                .get("objective")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_owned(),
            state: "ACTIVE".into(),
            supervisor: args
                .get("supervisor")
                .and_then(|v| v.as_str())
                .unwrap_or("manual")
                .to_owned(),
            authorized_agents: string_list(&args, "authorized_agents"),
            authorized_tools: string_list(&args, "authorized_tools"),
            created_at: now.clone(),
            updated_at: now,
            max_sources: int_or(&args, "max_sources", 8),
            max_candidates: int_or(&args, "max_candidates", 4),
            max_agent_runs: int_or(&args, "max_agent_runs", 40),
            max_runtime_seconds: int_or(&args, "max_runtime_seconds", 1800),
            max_retries: int_or(&args, "max_retries", 2),
            max_artifact_bytes: int_or(&args, "max_artifact_bytes", 250_000),
            sources_used: 0,
            candidates_used: 0,
            agent_runs_used: 0,
            publication_allowed: false,
        };
        if session.objective.is_empty() {
            return Err(AgentError::Invalid("session objective is required".into()));
        }
        persist::insert_session(&self.pool, &session).await?;
        persist::insert_event(
            &self.pool,
            &session.session_id,
            "SESSION_CREATED",
            json!({ "objective": session.objective, "supervisor": session.supervisor }),
        )
        .await?;
        persist::insert_event(
            &self.pool,
            &session.session_id,
            "SESSION_STARTED",
            json!({ "state": session.state }),
        )
        .await?;
        Ok(session)
    }

    pub async fn set_session_state(&self, session_id: &str, state: &str) -> Result<Session, AgentError> {
        if !SESSION_STATES.contains(&state) {
            return Err(AgentError::Invalid(format!("unknown session state {state}")));
        }
        let mut session = persist::get_session(&self.pool, session_id).await?;
        session.state = state.to_owned();
        session.updated_at = persist::now_rfc3339()?;
        persist::update_session(&self.pool, &session).await?;
        let kind = match state {
            "PAUSED" => "SESSION_PAUSED",
            "COMPLETE" => "SESSION_COMPLETED",
            "FAILED" => "SESSION_FAILED",
            "CANCELLED" => "SESSION_CANCELLED",
            "ACTIVE" => "SESSION_STARTED",
            _ => "SESSION_STATE",
        };
        persist::insert_event(&self.pool, session_id, kind, json!({ "state": state })).await?;
        if matches!(state, "COMPLETE" | "FAILED" | "CANCELLED") {
            let _ = self.write_receipt(session_id).await;
        }
        Ok(session)
    }

    pub async fn run_agent(&self, session_id: &str, agent_id: &str, input: Value) -> Result<AgentRun, AgentError> {
        let mut session = persist::get_session(&self.pool, session_id).await?;
        if session.state != "ACTIVE" {
            return Err(AgentError::State(format!(
                "session is {} — not accepting runs",
                session.state
            )));
        }
        if session.agent_runs_used >= session.max_agent_runs {
            return Err(AgentError::Budget("max_agent_runs exhausted".into()));
        }
        let loaded = self.registry.get_agent(agent_id)?;
        if !loaded.manifest.enabled {
            return Err(AgentError::Denied(format!("agent `{agent_id}` is disabled")));
        }
        if !session.authorized_agents.is_empty()
            && !session.authorized_agents.iter().any(|item| item == agent_id)
        {
            return Err(AgentError::Denied(format!(
                "agent `{agent_id}` is not authorized for this session"
            )));
        }
        validate_against_schema(&loaded.input_schema, &input)?;

        let now = persist::now_rfc3339()?;
        let mut run = AgentRun {
            run_id: persist::next_id(&self.pool, "run", "RUN-").await?,
            session_id: session_id.to_owned(),
            agent_id: agent_id.to_owned(),
            agent_version: loaded.manifest.version.clone(),
            state: "QUEUED".into(),
            reason: String::new(),
            input: input.clone(),
            output: json!({}),
            artifact_id: String::new(),
            queued_at: now.clone(),
            started_at: String::new(),
            completed_at: String::new(),
        };
        persist::insert_run(&self.pool, &run).await?;
        persist::insert_event(
            &self.pool,
            session_id,
            "AGENT_QUEUED",
            json!({ "run_id": run.run_id, "agent_id": agent_id }),
        )
        .await?;

        run.state = "RUNNING".into();
        run.started_at = persist::now_rfc3339()?;
        persist::update_run(&self.pool, &run).await?;
        persist::insert_event(
            &self.pool,
            session_id,
            "AGENT_STARTED",
            json!({ "run_id": run.run_id, "agent_id": agent_id, "version": run.agent_version }),
        )
        .await?;

        let ctx = ExecContext {
            runtime: self,
            session: &session,
            agent_id,
            agent_version: &loaded.manifest.version,
        };
        let handled = match agents::execute(&ctx, &input).await {
            Ok(result) => result,
            Err(error) => HandlerResult {
                state: "FAILED".into(),
                reason: error.to_string(),
                output: error.to_json(),
                artifact_type: "VERIFICATION_REPORT".into(),
                messages: Vec::new(),
            },
        };

        if handled.state != "FAILED" {
            if let Err(error) = validate_against_schema(&loaded.output_schema, &handled.output) {
                run.state = "FAILED".into();
                run.reason = format!("output schema: {error}");
                run.output = json!({ "error": run.reason });
                run.completed_at = persist::now_rfc3339()?;
                persist::update_run(&self.pool, &run).await?;
                persist::insert_event(
                    &self.pool,
                    session_id,
                    "AGENT_FAILED",
                    json!({ "run_id": run.run_id, "reason": run.reason }),
                )
                .await?;
                return Ok(run);
            }
        }

        let mut meta = ArtifactMeta {
            artifact_id: persist::next_id(&self.pool, "artifact", "ART-").await?,
            session_id: session_id.to_owned(),
            creator: agent_id.to_owned(),
            agent_id: agent_id.to_owned(),
            agent_version: loaded.manifest.version.clone(),
            r#type: handled.artifact_type.clone(),
            created_at: persist::now_rfc3339()?,
            content_hash: String::new(),
            schema_version: "1.0.0".into(),
            path: String::new(),
            provenance_refs: input
                .get("artifact_id")
                .and_then(|v| v.as_str())
                .map(|id| vec![id.to_owned()])
                .unwrap_or_default(),
        };
        persist::insert_artifact(&self.pool, &self.data_dir, &mut meta, &handled.output).await?;
        persist::insert_event(
            &self.pool,
            session_id,
            "ARTIFACT_CREATED",
            json!({ "artifact_id": meta.artifact_id, "type": meta.r#type, "agent_id": agent_id }),
        )
        .await?;

        for pending in handled.messages {
            let _ = self
                .send_message(
                    session_id,
                    agent_id,
                    &pending.to_agent,
                    &pending.r#type,
                    if pending.artifact_id.is_empty() {
                        &meta.artifact_id
                    } else {
                        &pending.artifact_id
                    },
                    pending.payload,
                )
                .await;
        }

        run.artifact_id = meta.artifact_id.clone();
        run.output = handled.output;
        run.reason = handled.reason;
        run.state = handled.state;
        run.completed_at = persist::now_rfc3339()?;
        persist::update_run(&self.pool, &run).await?;

        session.agent_runs_used += 1;
        if agent_id == "source_scout" || agent_id == "web_collector" {
            session.sources_used += 1;
        }
        session.updated_at = persist::now_rfc3339()?;
        persist::update_session(&self.pool, &session).await?;

        let event_kind = match run.state.as_str() {
            "FAILED" => "AGENT_FAILED",
            "LOW_CONFIDENCE" => "AGENT_LOW_CONFIDENCE",
            "NEEDS_HUMAN" => "AGENT_NEEDS_HUMAN",
            "BLOCKED" => "AGENT_BLOCKED",
            _ => "AGENT_COMPLETED",
        };
        persist::insert_event(
            &self.pool,
            session_id,
            event_kind,
            json!({
                "run_id": run.run_id,
                "agent_id": agent_id,
                "state": run.state,
                "artifact_id": run.artifact_id,
                "reason": run.reason
            }),
        )
        .await?;
        Ok(run)
    }

    pub async fn send_message(
        &self,
        session_id: &str,
        from_agent: &str,
        to_agent: &str,
        message_type: &str,
        artifact_id: &str,
        payload: Value,
    ) -> Result<AgentMessage, AgentError> {
        if !MESSAGE_TYPES.contains(&message_type) {
            return Err(AgentError::Invalid(format!(
                "unsupported message type `{message_type}`"
            )));
        }
        self.registry.get_agent(from_agent)?;
        self.registry.get_agent(to_agent)?;
        let from = self.registry.get_agent(from_agent)?;
        if !from.manifest.can_message.is_empty()
            && !from.manifest.can_message.iter().any(|item| item == to_agent)
        {
            return Err(AgentError::Denied(format!(
                "`{from_agent}` is not allowed to message `{to_agent}`"
            )));
        }
        persist::get_session(&self.pool, session_id).await?;
        let message = AgentMessage {
            message_id: persist::next_id(&self.pool, "message", "MSG-").await?,
            session_id: session_id.to_owned(),
            from_agent: from_agent.to_owned(),
            to_agent: to_agent.to_owned(),
            r#type: message_type.to_owned(),
            artifact_id: artifact_id.to_owned(),
            payload,
            created_at: persist::now_rfc3339()?,
        };
        persist::insert_message(&self.pool, &message).await?;
        persist::insert_event(
            &self.pool,
            session_id,
            "AGENT_MESSAGE_SENT",
            json!({
                "message_id": message.message_id,
                "from": from_agent,
                "to": to_agent,
                "type": message_type
            }),
        )
        .await?;
        Ok(message)
    }

    pub async fn submit_review_candidate(
        &self,
        session_id: &str,
        normalized: Value,
        raw: &str,
    ) -> Result<Value, AgentError> {
        let mut session = persist::get_session(&self.pool, session_id).await?;
        if session.candidates_used >= session.max_candidates {
            return Err(AgentError::Budget("max_candidates exhausted".into()));
        }
        if session.publication_allowed {
            return Err(AgentError::Denied(
                "publication_allowed must remain false; candidates go to private review only".into(),
            ));
        }
        let published = self.host.list_published_algorithms().await?;
        let title = normalized
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let short_name = normalized
            .get("short_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let core = normalized
            .get("core_idea")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (verdict, matches) =
            crate::archive_match::verdict_against_archive(title, short_name, core, &published);
        if verdict == "DUPLICATE" {
            persist::insert_event(
                &self.pool,
                session_id,
                "CANDIDATE_BLOCKED_DUPLICATE",
                json!({ "title": title, "matches": matches, "verdict": verdict }),
            )
            .await?;
            return Ok(json!({
                "blocked": true,
                "verdict": verdict,
                "matches": matches,
                "publication_count": 0,
                "reason": "archive already contains this procedure"
            }));
        }
        let submitted = self
            .host
            .submit_extraction_candidate(crate::host::ReviewCandidate {
                raw_extraction: raw.to_owned(),
                normalized: normalized.clone(),
            })
            .await?;
        session.candidates_used += 1;
        session.updated_at = persist::now_rfc3339()?;
        persist::update_session(&self.pool, &session).await?;
        persist::insert_event(
            &self.pool,
            session_id,
            "CANDIDATE_SUBMITTED",
            json!({
                "candidate_id": submitted.candidate_id,
                "review_state": submitted.review_state,
                "canonical_publications": 0
            }),
        )
        .await?;
        Ok(json!({
            "candidate_id": submitted.candidate_id,
            "review_state": submitted.review_state,
            "publication_count": 0
        }))
    }

    pub async fn write_receipt(&self, session_id: &str) -> Result<Value, AgentError> {
        let session = persist::get_session(&self.pool, session_id).await?;
        let runs = persist::list_runs(&self.pool, session_id).await?;
        let messages = persist::list_messages(&self.pool, session_id).await?;
        let artifacts = persist::list_artifacts(&self.pool, session_id).await?;
        let mut versions = Vec::new();
        for run in &runs {
            let stamp = format!("{}@{}", run.agent_id, run.agent_version);
            if !versions.contains(&stamp) {
                versions.push(stamp);
            }
        }
        let body = json!({
            "receipt_id": format!("REC-{session_id}"),
            "session_id": session_id,
            "state": session.state,
            "directive": session.objective,
            "supervisor": session.supervisor,
            "sources_used": session.sources_used,
            "candidates_used": session.candidates_used,
            "agent_runs": runs.len(),
            "failures": runs.iter().filter(|run| run.state == "FAILED").count(),
            "low_confidence": runs.iter().filter(|run| run.state == "LOW_CONFIDENCE").count(),
            "needs_human": runs.iter().filter(|run| run.state == "NEEDS_HUMAN").count(),
            "messages": messages.len(),
            "artifacts": artifacts.len(),
            "canonical_publications": 0,
            "agent_versions": versions
        });
        persist::save_receipt(&self.pool, session_id, &body).await?;
        persist::insert_event(
            &self.pool,
            session_id,
            "RECEIPT_WRITTEN",
            json!({ "receipt_id": format!("REC-{session_id}") }),
        )
        .await?;
        Ok(body)
    }

    pub async fn overview(&self) -> Result<Value, AgentError> {
        let sessions = persist::list_sessions(&self.pool).await?;
        let mut running = 0;
        let mut queued = 0;
        let mut blocked = 0;
        let mut needs_human = 0;
        let mut candidates = 0;
        for session in &sessions {
            candidates += session.candidates_used;
            for run in persist::list_runs(&self.pool, &session.session_id).await? {
                match run.state.as_str() {
                    "RUNNING" => running += 1,
                    "QUEUED" => queued += 1,
                    "BLOCKED" => blocked += 1,
                    "NEEDS_HUMAN" => needs_human += 1,
                    _ => {}
                }
            }
        }
        Ok(json!({
            "runtime": "ONLINE",
            "registered": self.registry.agents.len() + self.registry.invalid.len(),
            "valid": self.registry.agents.len(),
            "invalid": self.registry.invalid.len(),
            "invalid_details": self.registry.invalid,
            "active_sessions": sessions.iter().filter(|s| s.state == "ACTIVE").count(),
            "running_agents": running,
            "queued": queued,
            "blocked": blocked,
            "candidates": candidates,
            "needs_human": needs_human
        }))
    }
}

fn string_list(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn int_or(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

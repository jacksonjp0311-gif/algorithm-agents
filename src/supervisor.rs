use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::AgentError;
use crate::runtime::AgentRuntime;
use crate::tools;

#[async_trait]
pub trait SupervisorAdapter: Send + Sync {
    async fn receive_directive(
        &self,
        runtime: &AgentRuntime,
        directive: &str,
    ) -> Result<Value, AgentError>;
    async fn inspect_agents(&self, runtime: &AgentRuntime) -> Result<Value, AgentError>;
    async fn inspect_tools(&self, runtime: &AgentRuntime) -> Result<Value, AgentError>;
    async fn request_tool(
        &self,
        runtime: &AgentRuntime,
        tool: &str,
        args: Value,
    ) -> Result<Value, AgentError>;
    async fn stop(&self, runtime: &AgentRuntime, session_id: &str) -> Result<Value, AgentError>;
}

pub struct ManualSupervisor;

#[async_trait]
impl SupervisorAdapter for ManualSupervisor {
    async fn receive_directive(
        &self,
        runtime: &AgentRuntime,
        directive: &str,
    ) -> Result<Value, AgentError> {
        self.request_tool(
            runtime,
            "start_session",
            serde_json::json!({ "objective": directive, "supervisor": "manual" }),
        )
        .await
    }

    async fn inspect_agents(&self, runtime: &AgentRuntime) -> Result<Value, AgentError> {
        self.request_tool(runtime, "list_agents", serde_json::json!({}))
            .await
    }

    async fn inspect_tools(&self, runtime: &AgentRuntime) -> Result<Value, AgentError> {
        self.request_tool(runtime, "list_tools", serde_json::json!({}))
            .await
    }

    async fn request_tool(
        &self,
        runtime: &AgentRuntime,
        tool: &str,
        args: Value,
    ) -> Result<Value, AgentError> {
        tools::dispatch(runtime, tool, args).await
    }

    async fn stop(&self, runtime: &AgentRuntime, session_id: &str) -> Result<Value, AgentError> {
        self.request_tool(
            runtime,
            "stop_session",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
    }
}

pub struct ScriptedSupervisor {
    pub label: String,
}

impl ScriptedSupervisor {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
        }
    }
}

#[async_trait]
impl SupervisorAdapter for ScriptedSupervisor {
    async fn receive_directive(
        &self,
        runtime: &AgentRuntime,
        directive: &str,
    ) -> Result<Value, AgentError> {
        self.request_tool(
            runtime,
            "start_session",
            serde_json::json!({
                "objective": directive,
                "supervisor": self.label,
                "max_agent_runs": 160,
                "max_sources": 16,
                "max_candidates": 12,
                "max_runtime_seconds": 3600
            }),
        )
        .await
    }

    async fn inspect_agents(&self, runtime: &AgentRuntime) -> Result<Value, AgentError> {
        tools::dispatch(runtime, "list_agents", serde_json::json!({})).await
    }

    async fn inspect_tools(&self, runtime: &AgentRuntime) -> Result<Value, AgentError> {
        tools::dispatch(runtime, "list_tools", serde_json::json!({})).await
    }

    async fn request_tool(
        &self,
        runtime: &AgentRuntime,
        tool: &str,
        args: Value,
    ) -> Result<Value, AgentError> {
        tools::dispatch(runtime, tool, args).await
    }

    async fn stop(&self, runtime: &AgentRuntime, session_id: &str) -> Result<Value, AgentError> {
        tools::dispatch(
            runtime,
            "stop_session",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
    }
}

/// Attach contract for Grok / Codex. The runtime does not special-case either provider.
pub fn attach_contract() -> Value {
    serde_json::json!({
        "protocol_version": "1.0",
        "interface": "SupervisorEnvelope(provider, model, tool, args) → audited ToolResult",
        "surfaces": ["rust", "cli: algo supervisor --request <json>", "future MCP"],
        "providers": {
            "grok": "Call the same tools.json schemas. Do not embed provider-specific orchestration in the runtime.",
            "codex": "Call the same tools.json schemas. Do not embed provider-specific orchestration in the runtime."
        },
        "publication": false,
        "archive_authority": "human operator only",
        "required_fields": ["protocol_version", "provider", "model", "tool", "args"]
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorEnvelope {
    #[serde(default = "protocol_version")]
    pub protocol_version: String,
    #[serde(default)]
    pub invocation_id: String,
    #[serde(default)]
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub tool: String,
    #[serde(default = "empty_object")]
    pub args: Value,
}

fn protocol_version() -> String {
    "1.0".into()
}

fn empty_object() -> Value {
    json!({})
}

pub async fn execute_envelope(
    runtime: &AgentRuntime,
    mut envelope: SupervisorEnvelope,
) -> Result<Value, AgentError> {
    if envelope.protocol_version != "1.0" {
        return Err(AgentError::Invalid(format!(
            "unsupported supervisor protocol {}",
            envelope.protocol_version
        )));
    }
    if envelope.provider.trim().is_empty() || envelope.model.trim().is_empty() {
        return Err(AgentError::Invalid(
            "provider and model identity are required".into(),
        ));
    }
    if matches!(
        envelope.tool.as_str(),
        "publish_algorithm" | "approve_extraction" | "modify_canonical_archive"
    ) {
        return Err(AgentError::Denied(
            "model supervisors never receive archive authority".into(),
        ));
    }
    if envelope.invocation_id.is_empty() {
        envelope.invocation_id = format!("SUP-{}", uuid::Uuid::new_v4().simple());
    }
    if !envelope.session_id.is_empty() && envelope.args.get("session_id").is_none() {
        envelope.args["session_id"] = json!(envelope.session_id);
    }
    let created_at = crate::persist::now_rfc3339()?;
    sqlx::query(
        "INSERT INTO supervisor_invocations
         (invocation_id, session_id, provider, model, operation, request_json,
          response_json, state, created_at, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, '{}', 'RUNNING', ?, '')",
    )
    .bind(&envelope.invocation_id)
    .bind(&envelope.session_id)
    .bind(&envelope.provider)
    .bind(&envelope.model)
    .bind(&envelope.tool)
    .bind(serde_json::to_string(&envelope)?)
    .bind(&created_at)
    .execute(&runtime.pool)
    .await?;
    let result = tools::dispatch(runtime, &envelope.tool, envelope.args.clone()).await;
    let completed_at = crate::persist::now_rfc3339()?;
    let (state, response) = match &result {
        Ok(value) => ("COMPLETE", json!({ "ok": true, "result": value })),
        Err(error) => ("FAILED", error.to_json()),
    };
    sqlx::query(
        "UPDATE supervisor_invocations
         SET response_json=?, state=?, completed_at=? WHERE invocation_id=?",
    )
    .bind(serde_json::to_string(&response)?)
    .bind(state)
    .bind(&completed_at)
    .bind(&envelope.invocation_id)
    .execute(&runtime.pool)
    .await?;
    match result {
        Ok(value) => Ok(json!({
            "protocol_version": "1.0",
            "invocation_id": envelope.invocation_id,
            "provider": envelope.provider,
            "model": envelope.model,
            "archive_authority": false,
            "result": value
        })),
        Err(error) => Err(error),
    }
}

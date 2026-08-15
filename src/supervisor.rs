use async_trait::async_trait;
use serde_json::Value;

use crate::error::AgentError;
use crate::runtime::AgentRuntime;
use crate::tools;

#[async_trait]
pub trait SupervisorAdapter: Send + Sync {
    async fn receive_directive(&self, runtime: &AgentRuntime, directive: &str) -> Result<Value, AgentError>;
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
    async fn receive_directive(&self, runtime: &AgentRuntime, directive: &str) -> Result<Value, AgentError> {
        self.request_tool(
            runtime,
            "start_session",
            serde_json::json!({ "objective": directive, "supervisor": "manual" }),
        )
        .await
    }

    async fn inspect_agents(&self, runtime: &AgentRuntime) -> Result<Value, AgentError> {
        self.request_tool(runtime, "list_agents", serde_json::json!({})).await
    }

    async fn inspect_tools(&self, runtime: &AgentRuntime) -> Result<Value, AgentError> {
        self.request_tool(runtime, "list_tools", serde_json::json!({})).await
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
    async fn receive_directive(&self, runtime: &AgentRuntime, directive: &str) -> Result<Value, AgentError> {
        self.request_tool(
            runtime,
            "start_session",
            serde_json::json!({ "objective": directive, "supervisor": self.label }),
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
        "interface": "SupervisorAdapter.request_tool(tool, args) → ToolResult",
        "surfaces": ["rust", "cli", "http /api/dev/agents/tools/{name}", "future MCP"],
        "providers": {
            "grok": "Call the same tools.json schemas. Do not embed provider-specific orchestration in the runtime.",
            "codex": "Call the same tools.json schemas. Do not embed provider-specific orchestration in the runtime."
        },
        "publication": false
    })
}

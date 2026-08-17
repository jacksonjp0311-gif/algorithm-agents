use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::AgentError;
use crate::runtime::AgentRuntime;

pub async fn serve(runtime: Arc<AgentRuntime>) -> Result<(), AgentError> {
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": { "code": -32700, "message": error.to_string() }
                    }),
                )
                .await?;
                continue;
            }
        };
        if request.get("id").is_none() {
            continue;
        }
        let response = handle(&runtime, &request).await;
        write_message(&mut stdout, &response).await?;
    }
    Ok(())
}

async fn handle(runtime: &AgentRuntime, request: &Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "alchetron", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Agent output is private proposal. No MCP tool can publish canonical truth."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": runtime.registry.tools.values().map(|tool| json!({
                "name": tool.id,
                "description": tool.description,
                "inputSchema": tool.args
            })).collect::<Vec<_>>()
        })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            crate::tools::dispatch(runtime, name, arguments).await.map(|value| {
                json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                    "structuredContent": value,
                    "isError": false
                })
            })
        }
        _ => Err(AgentError::NotFound(format!(
            "MCP method `{method}` not found"
        ))),
    };
    match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err(error) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": error.to_string() }
        }),
    }
}

async fn write_message(stdout: &mut tokio::io::Stdout, value: &Value) -> Result<(), AgentError> {
    stdout
        .write_all(serde_json::to_string(value)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

pub fn descriptor() -> Value {
    json!({
        "name": "alchetron",
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "stdio",
        "command": "algo mcp",
        "protocol": "2025-06-18",
        "publication_tools_exposed": false
    })
}

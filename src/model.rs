use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::error::AgentError;
use crate::persist;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    #[serde(default)]
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub system: String,
    #[serde(default = "default_output_tokens")]
    pub max_output_tokens: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub invocation_id: String,
    pub provider: String,
    pub model: String,
    pub text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_ms: u128,
    pub audited: bool,
    pub archive_authority: bool,
}

struct ProviderProfile {
    endpoint: String,
    api_key: Option<String>,
    response_kind: &'static str,
}

pub async fn invoke(pool: &SqlitePool, request: ModelRequest) -> Result<ModelResponse, AgentError> {
    validate_request(&request)?;
    let profile = provider_profile(&request.provider)?;
    let invocation_id = format!("MOD-{}", uuid::Uuid::new_v4().simple());
    let created_at = persist::now_rfc3339()?;
    let prompt_hash = hash(&format!("{}\n{}", request.system, request.prompt));
    sqlx::query(
        "INSERT INTO model_invocations
         (invocation_id, session_id, provider, model, endpoint, prompt_hash, state, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'RUNNING', ?)",
    )
    .bind(&invocation_id)
    .bind(&request.session_id)
    .bind(&request.provider)
    .bind(&request.model)
    .bind(&profile.endpoint)
    .bind(&prompt_hash)
    .bind(&created_at)
    .execute(pool)
    .await?;

    let started = Instant::now();
    let result = invoke_profile(&profile, &request).await;
    let duration_ms = started.elapsed().as_millis();
    let completed_at = persist::now_rfc3339()?;
    match result {
        Ok((text, input_tokens, output_tokens)) => {
            let response_hash = hash(&text);
            sqlx::query(
                "UPDATE model_invocations SET response_hash=?, state='COMPLETE', input_tokens=?,
                 output_tokens=?, duration_ms=?, completed_at=? WHERE invocation_id=?",
            )
            .bind(response_hash)
            .bind(input_tokens as i64)
            .bind(output_tokens as i64)
            .bind(duration_ms as i64)
            .bind(completed_at)
            .bind(&invocation_id)
            .execute(pool)
            .await?;
            Ok(ModelResponse {
                invocation_id,
                provider: request.provider,
                model: request.model,
                text,
                input_tokens,
                output_tokens,
                duration_ms,
                audited: true,
                archive_authority: false,
            })
        }
        Err(error) => {
            sqlx::query(
                "UPDATE model_invocations SET state='FAILED', duration_ms=?, error=?, completed_at=?
                 WHERE invocation_id=?",
            )
            .bind(duration_ms as i64)
            .bind(error.to_string().chars().take(1000).collect::<String>())
            .bind(completed_at)
            .bind(&invocation_id)
            .execute(pool)
            .await?;
            Err(error)
        }
    }
}

async fn invoke_profile(
    profile: &ProviderProfile,
    request: &ModelRequest,
) -> Result<(String, u64, u64), AgentError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(request.timeout_seconds.clamp(1, 180)))
        .build()
        .map_err(|error| AgentError::Internal(error.to_string()))?;
    let body = if profile.response_kind == "responses" {
        json!({
            "model": request.model,
            "instructions": request.system,
            "input": request.prompt,
            "max_output_tokens": request.max_output_tokens.clamp(1, 8192)
        })
    } else {
        let mut messages = Vec::new();
        if !request.system.trim().is_empty() {
            messages.push(json!({ "role": "system", "content": request.system }));
        }
        messages.push(json!({ "role": "user", "content": request.prompt }));
        json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_output_tokens.clamp(1, 8192),
            "stream": false
        })
    };
    let mut last_error = None;
    for attempt in 0..=request.max_retries.min(2) {
        let mut call = client.post(&profile.endpoint).json(&body);
        if let Some(api_key) = &profile.api_key {
            call = call.bearer_auth(api_key);
        }
        match call.send().await {
            Ok(response) => {
                let status = response.status();
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|error| AgentError::Internal(error.to_string()))?;
                if bytes.len() > 2_000_000 {
                    return Err(AgentError::Budget(
                        "model response exceeded the 2 MB hard cap".into(),
                    ));
                }
                if !status.is_success() {
                    let message = String::from_utf8_lossy(&bytes);
                    last_error = Some(AgentError::Internal(format!(
                        "model provider returned {status}: {}",
                        message.chars().take(400).collect::<String>()
                    )));
                } else {
                    let payload: Value = serde_json::from_slice(&bytes)?;
                    return parse_response(profile.response_kind, &payload);
                }
            }
            Err(error) => last_error = Some(AgentError::Internal(error.to_string())),
        }
        if attempt < request.max_retries.min(2) {
            tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
        }
    }
    Err(last_error.unwrap_or_else(|| AgentError::Internal("model invocation failed".into())))
}

fn parse_response(kind: &str, payload: &Value) -> Result<(String, u64, u64), AgentError> {
    let text = if kind == "responses" {
        payload
            .get("output_text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                payload
                    .get("output")?
                    .as_array()?
                    .iter()
                    .flat_map(|item| {
                        item.get("content")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                    })
                    .find_map(|item| item.get("text").and_then(Value::as_str).map(str::to_owned))
            })
    } else {
        payload
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
    .ok_or_else(|| AgentError::Schema("model response did not contain text".into()))?;
    let input_tokens = payload
        .pointer("/usage/input_tokens")
        .or_else(|| payload.pointer("/usage/prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = payload
        .pointer("/usage/output_tokens")
        .or_else(|| payload.pointer("/usage/completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok((text, input_tokens, output_tokens))
}

fn provider_profile(provider: &str) -> Result<ProviderProfile, AgentError> {
    match provider.to_ascii_lowercase().as_str() {
        "openai" => Ok(ProviderProfile {
            endpoint: "https://api.openai.com/v1/responses".into(),
            api_key: Some(required_env("OPENAI_API_KEY")?),
            response_kind: "responses",
        }),
        "grok" | "xai" => Ok(ProviderProfile {
            endpoint: "https://api.x.ai/v1/chat/completions".into(),
            api_key: Some(required_env("XAI_API_KEY")?),
            response_kind: "chat",
        }),
        "local" => {
            let endpoint = std::env::var("ALCHETRON_LOCAL_MODEL_ENDPOINT")
                .unwrap_or_else(|_| "http://127.0.0.1:11434/v1/chat/completions".into());
            if !(endpoint.starts_with("http://127.0.0.1:")
                || endpoint.starts_with("http://localhost:"))
            {
                return Err(AgentError::Denied(
                    "local model endpoint must use loopback".into(),
                ));
            }
            Ok(ProviderProfile {
                endpoint,
                api_key: std::env::var("ALCHETRON_LOCAL_MODEL_KEY").ok(),
                response_kind: "chat",
            })
        }
        "codex" => Err(AgentError::Invalid(
            "Codex enters through `algo supervisor` or `algo mcp`; it is not a direct API-key adapter"
                .into(),
        )),
        other => Err(AgentError::Invalid(format!(
            "unsupported model provider `{other}`"
        ))),
    }
}

pub async fn list_invocations(pool: &SqlitePool) -> Result<Vec<Value>, AgentError> {
    let rows = sqlx::query(
        "SELECT invocation_id, session_id, provider, model, endpoint, state, input_tokens,
         output_tokens, duration_ms, error, created_at, completed_at
         FROM model_invocations ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "invocation_id": row.get::<String, _>("invocation_id"),
                "session_id": row.get::<String, _>("session_id"),
                "provider": row.get::<String, _>("provider"),
                "model": row.get::<String, _>("model"),
                "endpoint": row.get::<String, _>("endpoint"),
                "state": row.get::<String, _>("state"),
                "input_tokens": row.get::<i64, _>("input_tokens"),
                "output_tokens": row.get::<i64, _>("output_tokens"),
                "duration_ms": row.get::<i64, _>("duration_ms"),
                "error": row.get::<String, _>("error"),
                "created_at": row.get::<String, _>("created_at"),
                "completed_at": row.get::<String, _>("completed_at"),
                "archive_authority": false
            })
        })
        .collect())
}

pub fn capabilities() -> Value {
    json!({
        "protocol_version": "2.0",
        "providers": {
            "openai": { "transport": "Responses API", "credential": "OPENAI_API_KEY" },
            "grok": { "transport": "xAI chat completions", "credential": "XAI_API_KEY" },
            "local": { "transport": "OpenAI-compatible loopback", "credential": "optional" },
            "codex": { "transport": "supervisor envelope or MCP", "credential": "host managed" }
        },
        "limits": { "max_output_tokens": 8192, "max_timeout_seconds": 180, "max_retries": 2 },
        "secrets_persisted": false,
        "archive_authority": false
    })
}

fn validate_request(request: &ModelRequest) -> Result<(), AgentError> {
    if request.provider.trim().is_empty()
        || request.model.trim().is_empty()
        || request.prompt.trim().is_empty()
    {
        return Err(AgentError::Invalid(
            "provider, model, and prompt are required".into(),
        ));
    }
    if request.prompt.len() > 500_000 || request.system.len() > 100_000 {
        return Err(AgentError::Budget("model prompt exceeds hard cap".into()));
    }
    Ok(())
}

fn required_env(name: &str) -> Result<String, AgentError> {
    std::env::var(name).map_err(|_| AgentError::Denied(format!("{name} is not configured")))
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn default_output_tokens() -> u32 {
    2048
}

fn default_timeout() -> u64 {
    60
}

fn default_retries() -> u32 {
    1
}

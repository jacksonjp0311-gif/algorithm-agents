use serde_json::{Value, json};

use crate::error::AgentError;
use crate::persist;
use crate::registry::AGENT_IDS;
use crate::runtime::AgentRuntime;
use crate::schema::validate_against_schema;
use crate::tools;

#[derive(Debug, Clone)]
pub struct SmokeResult {
    pub agent_id: String,
    pub ok: bool,
    pub reason: String,
}

pub async fn smoke_all(runtime: &AgentRuntime, only: Option<&str>) -> Result<Value, AgentError> {
    let mut results = Vec::new();
    let ids: Vec<String> = if let Some(id) = only {
        vec![id.to_owned()]
    } else {
        AGENT_IDS.iter().map(|id| (*id).to_owned()).collect()
    };

    let session = runtime
        .start_session(json!({
            "objective": "Smoke compile every registered agent contract",
            "supervisor": "scripted",
            "max_agent_runs": 200,
            "max_sources": 20,
            "max_candidates": 10
        }))
        .await?;

    let scout = runtime
        .run_agent(
            &session.session_id,
            "source_scout",
            json!({ "objective": "shortest path weighted graphs" }),
        )
        .await?;
    let collect = runtime
        .run_agent(
            &session.session_id,
            "web_collector",
            json!({ "locator": "fixture://shortest-path/dijkstra.md" }),
        )
        .await?;
    let source_artifact = collect.artifact_id.clone();

    for id in ids {
        let result = smoke_one(runtime, &session.session_id, &source_artifact, &id).await;
        results.push(result);
    }

    // Failure path: invalid input should not silently pass.
    let invalid = runtime
        .run_agent(&session.session_id, "math_analyst", json!({ "nope": true }))
        .await;
    let failure_path = match invalid {
        Err(_) => true,
        Ok(run) => run.state == "FAILED",
    };

    let pass = results.iter().filter(|item| item.ok).count();
    let fail = results.len() - pass;
    let verdict = if fail == 0 && failure_path { "PASS" } else { "FAIL" };
    let _ = runtime.set_session_state(&session.session_id, "COMPLETE").await;
    Ok(json!({
        "banner": "ALGORITHM AGENTS / SMOKE COMPILER",
        "session_id": session.session_id,
        "seed_run": scout.run_id,
        "results": results.iter().map(|item| json!({
            "agent": item.agent_id,
            "ok": item.ok,
            "reason": item.reason
        })).collect::<Vec<_>>(),
        "agents_tested": results.len(),
        "pass": pass,
        "fail": fail,
        "failure_path": failure_path,
        "verdict": verdict
    }))
}

async fn smoke_one(
    runtime: &AgentRuntime,
    session_id: &str,
    source_artifact: &str,
    agent_id: &str,
) -> SmokeResult {
    let loaded = match runtime.registry.get_agent(agent_id) {
        Ok(agent) => agent.clone(),
        Err(error) => {
            return SmokeResult {
                agent_id: agent_id.into(),
                ok: false,
                reason: error.to_string(),
            };
        }
    };
    if loaded.manifest.timeout_seconds == 0 {
        return SmokeResult {
            agent_id: agent_id.into(),
            ok: false,
            reason: "timeout is zero".into(),
        };
    }
    if let Err(error) = validate_against_schema(&loaded.input_schema, &loaded.smoke_input) {
        return SmokeResult {
            agent_id: agent_id.into(),
            ok: false,
            reason: format!("smoke input: {error}"),
        };
    }
    let mut input = loaded.smoke_input.clone();
    if input.get("artifact_id").and_then(|v| v.as_str()) == Some("$SOURCE") {
        input["artifact_id"] = json!(source_artifact);
    }
    match runtime.run_agent(session_id, agent_id, input).await {
        Ok(run) => {
            if run.artifact_id.is_empty() {
                return SmokeResult {
                    agent_id: agent_id.into(),
                    ok: false,
                    reason: "no artifact created".into(),
                };
            }
            if let Ok(events) = persist::list_events(&runtime.pool, session_id).await {
                if !events.iter().any(|event| event.kind == "AGENT_COMPLETED" || event.kind == "AGENT_FAILED" || event.kind == "AGENT_LOW_CONFIDENCE" || event.kind == "AGENT_NEEDS_HUMAN" || event.kind == "AGENT_BLOCKED") {
                    return SmokeResult {
                        agent_id: agent_id.into(),
                        ok: false,
                        reason: "no completion event".into(),
                    };
                }
            }
            SmokeResult {
                agent_id: agent_id.into(),
                ok: true,
                reason: run.state,
            }
        }
        Err(error) => SmokeResult {
            agent_id: agent_id.into(),
            ok: false,
            reason: error.to_string(),
        },
    }
}

pub fn format_smoke_report(report: &Value) -> String {
    let mut out = String::from("JACKSON AGENT SYSTEM / SMOKE COMPILER\n=====================================\n\n");
    if let Some(results) = report.get("results").and_then(|v| v.as_array()) {
        for item in results {
            let id = item.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let ok = item.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            let reason = item.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            if ok {
                out.push_str(&format!("[OK] {id}\n"));
            } else {
                out.push_str(&format!("[FAIL] {id}\nREASON\n{reason}\n"));
            }
        }
    }
    out.push('\n');
    out.push_str(&format!(
        "AGENTS TESTED: {}\nPASS:          {}\nFAIL:           {}\n\nVERDICT: {}\n",
        report.get("agents_tested").and_then(|v| v.as_u64()).unwrap_or(0),
        report.get("pass").and_then(|v| v.as_u64()).unwrap_or(0),
        report.get("fail").and_then(|v| v.as_u64()).unwrap_or(0),
        report.get("verdict").and_then(|v| v.as_str()).unwrap_or("FAIL")
    ));
    out
}

pub async fn smoke_via_tool(runtime: &AgentRuntime, agent: Option<&str>) -> Result<Value, AgentError> {
    let report = smoke_all(runtime, agent).await?;
    let _ = tools::dispatch(runtime, "get_overview", json!({})).await;
    Ok(report)
}

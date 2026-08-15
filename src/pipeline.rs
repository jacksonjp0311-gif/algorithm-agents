use serde_json::{Value, json};

use crate::error::AgentError;
use crate::persist;
use crate::runtime::AgentRuntime;
use crate::supervisor::{ScriptedSupervisor, SupervisorAdapter};

pub async fn find_sources(runtime: &AgentRuntime, query: &str) -> Result<Value, AgentError> {
    let session = runtime
        .start_session(json!({
            "objective": format!("Find algorithms matching: {query}"),
            "supervisor": "find",
            "max_agent_runs": 8,
            "max_sources": 12
        }))
        .await?;
    let scout = runtime
        .run_agent(
            &session.session_id,
            "source_scout",
            json!({ "objective": query }),
        )
        .await?;
    let accepted = runtime.host.list_published_algorithms().await?;
    let q = query.to_ascii_lowercase();
    let archive: Vec<Value> = accepted
        .into_iter()
        .filter(|item| {
            format!(
                "{} {} {} {}",
                item.title, item.short_name, item.domain, item.summary
            )
            .to_ascii_lowercase()
            .contains(&q)
                || item
                    .tags
                    .iter()
                    .any(|tag| tag.to_ascii_lowercase().contains(&q))
        })
        .map(|item| {
            json!({
                "id": item.id,
                "archive_id": item.archive_id,
                "title": item.title,
                "domain": item.domain,
                "summary": item.summary
            })
        })
        .collect();
    let _ = runtime.set_session_state(&session.session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session.session_id,
        "query": query,
        "sources": scout.output.get("sources").cloned().unwrap_or(json!([])),
        "archive_hits": archive
    }))
}

pub async fn harvest(runtime: &AgentRuntime, objective: &str, limit: usize) -> Result<Value, AgentError> {
    let supervisor = ScriptedSupervisor::new("harvest");
    let session = supervisor
        .receive_directive(
            runtime,
            &format!("{objective} Extract well-supported algorithm candidates. Do not publish."),
        )
        .await?;
    let session_id = session
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_owned();
    let scout = supervisor
        .request_tool(
            runtime,
            "search_sources",
            json!({ "session_id": session_id, "input": { "objective": objective } }),
        )
        .await?;
    let sources = scout
        .get("output")
        .and_then(|v| v.get("sources"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut harvested = Vec::new();
    for source in sources.into_iter().take(limit.max(1)) {
        let locator = source
            .get("locator")
            .or_else(|| source.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if locator.is_empty() {
            continue;
        }
        harvested.push(extract_and_save(runtime, &supervisor, &session_id, locator).await?);
    }
    let _ = runtime.set_session_state(&session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session_id,
        "objective": objective,
        "harvested": harvested
    }))
}

pub async fn scrape(runtime: &AgentRuntime, locator: &str, objective: &str) -> Result<Value, AgentError> {
    let supervisor = ScriptedSupervisor::new("scrape");
    let session = supervisor
        .receive_directive(
            runtime,
            &format!("{objective} Retrieve {locator} and extract any reconstructable algorithm. Do not publish."),
        )
        .await?;
    let session_id = session
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_owned();
    let saved = extract_and_save(runtime, &supervisor, &session_id, locator).await?;
    let _ = runtime.set_session_state(&session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session_id,
        "locator": locator,
        "result": saved
    }))
}

async fn extract_and_save(
    runtime: &AgentRuntime,
    supervisor: &ScriptedSupervisor,
    session_id: &str,
    locator: &str,
) -> Result<Value, AgentError> {
    let collect = supervisor
        .request_tool(
            runtime,
            "retrieve_source",
            json!({ "session_id": session_id, "locator": locator }),
        )
        .await?;
    if collect.get("output").and_then(|v| v.get("blocked")).and_then(|v| v.as_bool()) == Some(true) {
        return Ok(json!({
            "locator": locator,
            "blocked": true,
            "reason": collect.get("output").and_then(|v| v.get("reason")).cloned().unwrap_or(json!("blocked"))
        }));
    }
    let artifact = collect
        .get("artifact_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let detect = supervisor
        .request_tool(
            runtime,
            "detect_algorithm",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let extract = supervisor
        .request_tool(
            runtime,
            "extract_algorithm",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _ = supervisor
        .request_tool(
            runtime,
            "analyze_math",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _ = supervisor
        .request_tool(
            runtime,
            "verify_provenance",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _ = supervisor
        .request_tool(
            runtime,
            "normalize_candidate",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let extracted = extract
        .get("output")
        .and_then(|v| v.get("extracted"))
        .and_then(|v| v.as_bool());
    if extracted == Some(false) {
        return Ok(json!({
            "locator": locator,
            "detect": detect.get("output"),
            "extracted": false
        }));
    }
    let normalized = extract
        .get("output")
        .and_then(|v| v.get("extraction"))
        .cloned()
        .unwrap_or_else(|| json!({ "title": "Unknown" }));
    let submitted = supervisor
        .request_tool(
            runtime,
            "submit_review_candidate",
            json!({
                "session_id": session_id,
                "normalized": normalized,
                "raw_extraction": format!("pipeline extract from {locator}")
            }),
        )
        .await?;
    Ok(json!({
        "locator": locator,
        "artifact_id": artifact,
        "detect": detect.get("output"),
        "title": normalized.get("title"),
        "candidate": submitted
    }))
}

pub async fn session_summary(runtime: &AgentRuntime, session_id: &str) -> Result<Value, AgentError> {
    let session = persist::get_session(&runtime.pool, session_id).await?;
    let runs = persist::list_runs(&runtime.pool, session_id).await?;
    Ok(json!({
        "session": session,
        "runs": runs.len()
    }))
}

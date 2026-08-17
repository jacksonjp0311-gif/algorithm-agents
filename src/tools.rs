use serde_json::{Value, json};

use crate::error::AgentError;
use crate::persist;
use crate::runtime::AgentRuntime;

pub async fn dispatch(
    runtime: &AgentRuntime,
    tool: &str,
    args: Value,
) -> Result<Value, AgentError> {
    if runtime.permissions.tool_globally_denied(tool) {
        return Err(AgentError::Denied(format!(
            "tool `{tool}` is globally denied"
        )));
    }
    if tool == "publish_algorithm"
        || tool == "approve_extraction"
        || tool == "modify_canonical_archive"
    {
        return Err(AgentError::Denied(
            "canonical publication tools are not available to the runtime".into(),
        ));
    }
    let manifest = runtime.registry.get_tool(tool)?;
    crate::schema::validate_against_schema(&manifest.args, &args)?;
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    if manifest.session_required && session_id.is_empty() {
        return Err(AgentError::Invalid(format!(
            "tool `{tool}` requires session_id"
        )));
    }
    if !session_id.is_empty() {
        let session = persist::get_session(&runtime.pool, &session_id).await?;
        if !session.authorized_tools.is_empty()
            && !session.authorized_tools.iter().any(|item| item == tool)
        {
            return Err(AgentError::Denied(format!(
                "tool `{tool}` is not authorized for {session_id}"
            )));
        }
    }
    if let Some(actor) = args.get("actor_agent").and_then(|v| v.as_str()) {
        {
            let agent = runtime.registry.get_agent(actor)?;
            if agent.manifest.denied_tools.iter().any(|item| item == tool) {
                return Err(AgentError::Denied(format!(
                    "agent `{actor}` is denied tool `{tool}`"
                )));
            }
            if !agent.manifest.allowed_tools.is_empty()
                && !agent.manifest.allowed_tools.iter().any(|item| item == tool)
            {
                return Err(AgentError::Denied(format!(
                    "agent `{actor}` is not allowed to call `{tool}`"
                )));
            }
        }
    }

    match tool {
        "list_agents" => Ok(json!({
            "agents": runtime.registry.agents.values().map(|a| json!({
                "id": a.manifest.id,
                "name": a.manifest.name,
                "version": a.manifest.version,
                "family": a.manifest.family,
                "purpose": a.manifest.purpose,
                "enabled": a.manifest.enabled
            })).collect::<Vec<_>>(),
            "invalid": runtime.registry.invalid
        })),
        "describe_agent" => {
            let id = required_str(&args, "agent_id")?;
            let agent = runtime.registry.get_agent(id)?;
            Ok(json!(agent.manifest))
        }
        "list_tools" => Ok(json!({
            "tools": runtime.registry.tools.values().map(|t| json!({
                "id": t.id,
                "description": t.description,
                "session_required": t.session_required
            })).collect::<Vec<_>>()
        })),
        "describe_tool" => {
            let id = required_str(&args, "tool_id")?;
            let tool = runtime.registry.get_tool(id)?;
            Ok(json!(tool))
        }
        "start_session" => {
            let session = runtime.start_session(args).await?;
            Ok(serde_json::to_value(session)?)
        }
        "get_session" => {
            let session =
                persist::get_session(&runtime.pool, required_str(&args, "session_id")?).await?;
            Ok(serde_json::to_value(session)?)
        }
        "list_sessions" => {
            let sessions = persist::list_sessions(&runtime.pool).await?;
            Ok(json!({ "sessions": sessions }))
        }
        "run_agent" => {
            let session_id = required_str(&args, "session_id")?;
            let agent_id = required_str(&args, "agent_id")?;
            let input = args.get("input").cloned().unwrap_or_else(|| json!({}));
            let run = runtime.run_agent(session_id, agent_id, input).await?;
            Ok(serde_json::to_value(run)?)
        }
        "get_agent_status" | "get_agent_result" => {
            let run = persist::get_run(&runtime.pool, required_str(&args, "run_id")?).await?;
            Ok(serde_json::to_value(run)?)
        }
        "send_agent_message" => {
            let message = runtime
                .send_message(
                    required_str(&args, "session_id")?,
                    required_str(&args, "from_agent")?,
                    required_str(&args, "to_agent")?,
                    required_str(&args, "type")?,
                    args.get("artifact_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    args.get("payload").cloned().unwrap_or_else(|| json!({})),
                )
                .await?;
            Ok(serde_json::to_value(message)?)
        }
        "get_agent_messages" => {
            let messages =
                persist::list_messages(&runtime.pool, required_str(&args, "session_id")?).await?;
            Ok(json!({ "messages": messages }))
        }
        "search_sources" => alias_run(runtime, &session_id, "source_scout", args).await,
        "retrieve_source" => alias_run(runtime, &session_id, "web_collector", args).await,
        "inspect_document" => alias_run(runtime, &session_id, "paper_analyst", args).await,
        "inspect_repository" => alias_run(runtime, &session_id, "repository_scout", args).await,
        "detect_algorithm" => alias_run(runtime, &session_id, "algorithm_detector", args).await,
        "extract_algorithm" => alias_run(runtime, &session_id, "algorithm_extractor", args).await,
        "analyze_math" => alias_run(runtime, &session_id, "math_analyst", args).await,
        "analyze_code" => alias_run(runtime, &session_id, "code_analyst", args).await,
        "analyze_complexity" => alias_run(runtime, &session_id, "complexity_analyst", args).await,
        "analyze_assumptions" => alias_run(runtime, &session_id, "assumption_analyst", args).await,
        "analyze_failures" => alias_run(runtime, &session_id, "failure_mode_analyst", args).await,
        "verify_provenance" => alias_run(runtime, &session_id, "provenance_checker", args).await,
        "verify_math" => alias_run(runtime, &session_id, "math_checker", args).await,
        "verify_code" => alias_run(runtime, &session_id, "code_verifier", args).await,
        "challenge_claims" => {
            alias_run(runtime, &session_id, "hallucination_challenger", args).await
        }
        "find_duplicates" => alias_run(runtime, &session_id, "deduplicator", args).await,
        "find_relationships" => alias_run(runtime, &session_id, "relationship_mapper", args).await,
        "find_structural_matches" => {
            alias_run(runtime, &session_id, "structural_matcher", args).await
        }
        "classify_domain" => alias_run(runtime, &session_id, "domain_classifier", args).await,
        "map_use_cases" => alias_run(runtime, &session_id, "use_case_mapper", args).await,
        "normalize_candidate" => alias_run(runtime, &session_id, "normalizer", args).await,
        "summarize_candidate" => alias_run(runtime, &session_id, "summarizer", args).await,
        "generate_reference_code" => alias_run(runtime, &session_id, "code_translator", args).await,
        "design_experiment" => alias_run(runtime, &session_id, "experiment_designer", args).await,
        "propose_relationship" => Ok(serde_json::to_value(
            crate::graph::propose_relationship(
                &runtime.pool,
                required_str(&args, "session_id")?,
                required_str(&args, "from_node")?,
                required_str(&args, "to_node")?,
                required_str(&args, "relation")?,
                args.get("evidence").cloned().unwrap_or_else(|| json!([])),
                required_str(&args, "proposed_by")?,
            )
            .await?,
        )?),
        "list_relationship_proposals" => Ok(json!({
            "proposals": crate::graph::list_proposals(&runtime.pool).await?
        })),
        "get_canonical_graph" => crate::graph::graph_snapshot(&runtime.pool, false).await,
        "submit_review_candidate" => {
            let normalized = args
                .get("normalized")
                .cloned()
                .ok_or_else(|| AgentError::Invalid("normalized candidate is required".into()))?;
            let raw = args
                .get("raw_extraction")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            runtime
                .submit_review_candidate(required_str(&args, "session_id")?, normalized, raw)
                .await
        }
        "pause_session" => {
            let session = runtime
                .set_session_state(required_str(&args, "session_id")?, "PAUSED")
                .await?;
            Ok(serde_json::to_value(session)?)
        }
        "resume_session" => {
            let session = runtime
                .set_session_state(required_str(&args, "session_id")?, "ACTIVE")
                .await?;
            Ok(serde_json::to_value(session)?)
        }
        "stop_session" => {
            let session = runtime
                .set_session_state(required_str(&args, "session_id")?, "CANCELLED")
                .await?;
            Ok(serde_json::to_value(session)?)
        }
        "get_session_events" => {
            let events =
                persist::list_events(&runtime.pool, required_str(&args, "session_id")?).await?;
            Ok(json!({ "events": events }))
        }
        "get_session_receipt" => {
            runtime
                .write_receipt(required_str(&args, "session_id")?)
                .await
        }
        "get_overview" => runtime.overview().await,
        "list_artifacts" => {
            let artifacts =
                persist::list_artifacts(&runtime.pool, required_str(&args, "session_id")?).await?;
            Ok(json!({ "artifacts": artifacts }))
        }
        "get_artifact" => {
            let (meta, content) =
                persist::get_artifact(&runtime.pool, required_str(&args, "artifact_id")?).await?;
            Ok(json!({ "meta": meta, "content": content }))
        }
        "list_runs" => {
            let runs =
                persist::list_runs(&runtime.pool, required_str(&args, "session_id")?).await?;
            Ok(json!({ "runs": runs }))
        }
        other => Err(AgentError::NotFound(format!("unknown tool `{other}`"))),
    }
}

async fn alias_run(
    runtime: &AgentRuntime,
    session_id: &str,
    agent_id: &str,
    args: Value,
) -> Result<Value, AgentError> {
    if session_id.is_empty() {
        return Err(AgentError::Invalid("session_id is required".into()));
    }
    let input = args.get("input").cloned().unwrap_or(args);
    let run = runtime.run_agent(session_id, agent_id, input).await?;
    Ok(serde_json::to_value(run)?)
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, AgentError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AgentError::Invalid(format!("`{key}` is required")))
}

use serde_json::{Value, json};

use crate::error::AgentError;
use crate::persist;
use crate::runtime::AgentRuntime;
use crate::supervisor::{ScriptedSupervisor, SupervisorAdapter};

pub async fn demo_shortest_path(runtime: &AgentRuntime) -> Result<Value, AgentError> {
    let supervisor = ScriptedSupervisor::new("scripted");
    let session = supervisor
        .receive_directive(
            runtime,
            "Find and analyze established shortest-path algorithms suitable for weighted graphs. Extract one well-supported algorithm candidate. Do not publish.",
        )
        .await?;
    let session_id = session.get("session_id").and_then(|v| v.as_str()).unwrap().to_owned();
    let scout = supervisor
        .request_tool(
            runtime,
            "search_sources",
            json!({ "session_id": session_id, "input": { "objective": "weighted shortest path dijkstra bellman" } }),
        )
        .await?;
    let collect = supervisor
        .request_tool(
            runtime,
            "retrieve_source",
            json!({ "session_id": session_id, "locator": "fixture://shortest-path/dijkstra.md" }),
        )
        .await?;
    let artifact = collect.get("artifact_id").and_then(|v| v.as_str()).unwrap_or("");
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
    let _math = supervisor
        .request_tool(
            runtime,
            "analyze_math",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _complexity = supervisor
        .request_tool(
            runtime,
            "analyze_complexity",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _prov = supervisor
        .request_tool(
            runtime,
            "verify_provenance",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
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
                "raw_extraction": "demo session 1 — shortest path fixture"
            }),
        )
        .await?;
    let _ = runtime.set_session_state(&session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session_id,
        "sources": scout.get("output"),
        "detect": detect.get("output"),
        "candidate": submitted,
        "publication_count": 0,
        "result": if submitted.get("review_state").and_then(|v| v.as_str()) == Some("PENDING") { "PASS" } else { "FAIL" }
    }))
}

pub async fn demo_inter_agent(runtime: &AgentRuntime) -> Result<Value, AgentError> {
    let supervisor = ScriptedSupervisor::new("scripted");
    let session = supervisor
        .receive_directive(
            runtime,
            "Find an established recursive state-estimation method. Require extractor to request math analysis through the typed message bus.",
        )
        .await?;
    let session_id = session.get("session_id").and_then(|v| v.as_str()).unwrap().to_owned();
    let collect = supervisor
        .request_tool(
            runtime,
            "retrieve_source",
            json!({ "session_id": session_id, "locator": "fixture://state-estimation/kalman.md" }),
        )
        .await?;
    let artifact = collect.get("artifact_id").and_then(|v| v.as_str()).unwrap_or("");
    let _detect = supervisor
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
    let _math = supervisor
        .request_tool(
            runtime,
            "analyze_math",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _prov = supervisor
        .request_tool(
            runtime,
            "verify_provenance",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let _challenge = supervisor
        .request_tool(
            runtime,
            "challenge_claims",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact, "extraction": extract.get("output") } }),
        )
        .await?;
    let _norm = supervisor
        .request_tool(
            runtime,
            "normalize_candidate",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let messages = persist::list_messages(&runtime.pool, &session_id).await?;
    let events = persist::list_events(&runtime.pool, &session_id).await?;
    let message_ok = messages.iter().any(|msg| {
        msg.from_agent == "algorithm_extractor"
            && msg.to_agent == "math_analyst"
            && msg.r#type == "ANALYSIS_REQUEST"
    });
    let event_ok = events.iter().any(|event| event.kind == "AGENT_MESSAGE_SENT");
    let _ = runtime.set_session_state(&session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session_id,
        "inter_agent_communication": if message_ok && event_ok { "PASS" } else { "FAIL" },
        "messages": messages.len(),
        "publication_count": 0
    }))
}

pub async fn demo_escalation(runtime: &AgentRuntime) -> Result<Value, AgentError> {
    let supervisor = ScriptedSupervisor::new("scripted");
    let session = supervisor
        .receive_directive(
            runtime,
            "Analyze an ambiguous correction-term source and preserve uncertainty.",
        )
        .await?;
    let session_id = session.get("session_id").and_then(|v| v.as_str()).unwrap().to_owned();
    let collect = supervisor
        .request_tool(
            runtime,
            "retrieve_source",
            json!({ "session_id": session_id, "locator": "fixture://ambiguous/correction.md" }),
        )
        .await?;
    let artifact = collect.get("artifact_id").and_then(|v| v.as_str()).unwrap_or("");
    let math = supervisor
        .request_tool(
            runtime,
            "analyze_math",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let prov = supervisor
        .request_tool(
            runtime,
            "verify_provenance",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let math_state = math.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let prov_state = prov.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let preserved = ["NEEDS_HUMAN", "LOW_CONFIDENCE"].contains(&math_state)
        || ["NEEDS_HUMAN", "LOW_CONFIDENCE"].contains(&prov_state);
    let _ = runtime.set_session_state(&session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session_id,
        "math_state": math_state,
        "provenance_state": prov_state,
        "human_escalation": if preserved { "PASS" } else { "FAIL" },
        "publication_count": 0
    }))
}

pub async fn demo_harvest(runtime: &AgentRuntime) -> Result<Value, AgentError> {
    let locators = [
        "fixture://set-union/union-find.md",
        "fixture://sampling/metropolis-hastings.md",
        "fixture://filtering/sir-particle.md",
        "fixture://decoding/viterbi.md",
        "fixture://shortest-path/bellman-ford.md",
    ];
    let supervisor = ScriptedSupervisor::new("scripted");
    let session = supervisor
        .receive_directive(
            runtime,
            "Harvest established computational procedures and any latent companion algorithms sitting inside the same sources. Prefer quality over volume. Do not publish.",
        )
        .await?;
    let session_id = session.get("session_id").and_then(|v| v.as_str()).unwrap().to_owned();
    let mut harvested = Vec::new();
    for locator in locators {
        let collect = supervisor
            .request_tool(
                runtime,
                "retrieve_source",
                json!({ "session_id": session_id, "locator": locator }),
            )
            .await?;
        let artifact = collect.get("artifact_id").and_then(|v| v.as_str()).unwrap_or("");
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
        for specialist in [
            "complexity_analyst",
            "structural_matcher",
            "domain_classifier",
            "deduplicator",
        ] {
            let _ = supervisor
                .request_tool(
                    runtime,
                    "run_agent",
                    json!({
                        "session_id": session_id,
                        "agent_id": specialist,
                        "input": { "artifact_id": artifact }
                    }),
                )
                .await?;
        }
        let primary = extract
            .get("output")
            .and_then(|v| v.get("extraction"))
            .cloned()
            .unwrap_or(json!({}));
        let emergent = extract
            .get("output")
            .and_then(|v| v.get("emergent"))
            .cloned()
            .unwrap_or(json!([]));
        harvested.push(json!({
            "locator": locator,
            "detect": detect.get("output"),
            "primary": primary,
            "emergent": emergent
        }));
    }
    let _ = runtime.set_session_state(&session_id, "COMPLETE").await;
    Ok(json!({
        "session_id": session_id,
        "harvested": harvested,
        "publication_count": 0
    }))
}

pub async fn demo_restart(runtime: &AgentRuntime) -> Result<Value, AgentError> {
    let session = runtime
        .start_session(json!({
            "objective": "Restart persistence probe",
            "supervisor": "scripted"
        }))
        .await?;
    let _ = runtime
        .run_agent(
            &session.session_id,
            "source_scout",
            json!({ "objective": "shortest path" }),
        )
        .await?;
    let reloaded = persist::get_session(&runtime.pool, &session.session_id).await?;
    let runs = persist::list_runs(&runtime.pool, &session.session_id).await?;
    Ok(json!({
        "session_id": reloaded.session_id,
        "runs": runs.len(),
        "restart": if !runs.is_empty() { "PASS" } else { "FAIL" }
    }))
}

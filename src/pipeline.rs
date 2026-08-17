use serde_json::{Value, json};

use crate::error::AgentError;
use crate::evidence;
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
    let _ = runtime
        .set_session_state(&session.session_id, "COMPLETE")
        .await;
    Ok(json!({
        "session_id": session.session_id,
        "query": query,
        "sources": scout.output.get("sources").cloned().unwrap_or(json!([])),
        "archive_hits": archive
    }))
}

pub async fn harvest(
    runtime: &AgentRuntime,
    objective: &str,
    limit: usize,
) -> Result<Value, AgentError> {
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

pub async fn scrape(
    runtime: &AgentRuntime,
    locator: &str,
    objective: &str,
) -> Result<Value, AgentError> {
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
    if collect
        .get("output")
        .and_then(|v| v.get("blocked"))
        .and_then(|v| v.as_bool())
        == Some(true)
    {
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
    let (_, source_body) = persist::get_artifact(&runtime.pool, artifact).await?;
    let snapshot = evidence::capture_source_snapshot(
        &runtime.pool,
        &runtime.data_dir,
        session_id,
        artifact,
        &source_body,
    )
    .await?;
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
    let math_analysis = supervisor
        .request_tool(
            runtime,
            "analyze_math",
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
    let extraction = extract
        .get("output")
        .and_then(|v| v.get("extraction"))
        .cloned()
        .unwrap_or_else(|| json!({ "title": "Unknown" }));
    let code_analysis = supervisor
        .request_tool(
            runtime,
            "analyze_code",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let complexity = supervisor
        .request_tool(
            runtime,
            "analyze_complexity",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let assumptions = supervisor
        .request_tool(
            runtime,
            "analyze_assumptions",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let failures = supervisor
        .request_tool(
            runtime,
            "analyze_failures",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let provenance = supervisor
        .request_tool(
            runtime,
            "verify_provenance",
            json!({
                "session_id": session_id,
                "input": { "artifact_id": artifact, "claims": claim_strings(&extraction) }
            }),
        )
        .await?;
    let math_verification = supervisor
        .request_tool(
            runtime,
            "verify_math",
            json!({ "session_id": session_id, "input": { "artifact_id": artifact } }),
        )
        .await?;
    let cross_source = supervisor
        .request_tool(
            runtime,
            "run_agent",
            json!({
                "session_id": session_id,
                "agent_id": "cross_source_verifier",
                "input": { "artifact_id": artifact }
            }),
        )
        .await?;
    let challenge = supervisor
        .request_tool(
            runtime,
            "challenge_claims",
            json!({
                "session_id": session_id,
                "input": { "artifact_id": artifact, "extraction": extraction }
            }),
        )
        .await?;
    let code = extraction
        .get("reference_code")
        .and_then(Value::as_str)
        .filter(|code| !code.trim().is_empty())
        .map(|source| {
            json!({
                "session_id": session_id,
                "input": {
                    "language": extraction
                        .get("reference_language")
                        .and_then(Value::as_str)
                        .unwrap_or("python"),
                    "source": source
                }
            })
        });
    let code_verification = if let Some(args) = code {
        Some(
            supervisor
                .request_tool(runtime, "verify_code", args)
                .await?,
        )
    } else {
        None
    };
    let normalizer = supervisor
        .request_tool(
            runtime,
            "normalize_candidate",
            json!({ "session_id": session_id, "input": { "extraction": extraction } }),
        )
        .await?;
    let mut normalized = normalizer
        .get("output")
        .and_then(|v| v.get("normalized"))
        .cloned()
        .unwrap_or_else(|| json!({ "title": "Unknown" }));
    let mut verification = verification_report(
        &detect,
        &math_verification,
        &provenance,
        &challenge,
        code_verification.as_ref(),
        &cross_source,
    );
    if let Some(object) = normalized.as_object_mut() {
        object.insert("validation".into(), verification.clone());
        object.insert(
            "analysis".into(),
            json!({
                "math": math_analysis.get("output"),
                "code": code_analysis.get("output"),
                "complexity": complexity.get("output"),
                "assumptions": assumptions.get("output"),
                "failure_modes": failures.get("output")
            }),
        );
        object.insert(
            "agent_artifacts".into(),
            json!({
                "source": artifact,
                "detector": detect.get("artifact_id"),
                "extractor": extract.get("artifact_id"),
                "math_analysis": math_analysis.get("artifact_id"),
                "code_analysis": code_analysis.get("artifact_id"),
                "complexity": complexity.get("artifact_id"),
                "assumptions": assumptions.get("artifact_id"),
                "failure_modes": failures.get("artifact_id"),
                "math_verification": math_verification.get("artifact_id"),
                "provenance": provenance.get("artifact_id"),
                "challenge": challenge.get("artifact_id"),
                "cross_source": cross_source.get("artifact_id"),
                "code": code_verification.as_ref().and_then(|run| run.get("artifact_id")),
                "normalizer": normalizer.get("artifact_id")
            }),
        );
    }
    let claims = evidence::build_claim_records(&normalized, &snapshot);
    let evidenced = claims
        .iter()
        .filter(|claim| !claim.evidence.is_empty())
        .count();
    let evidence_coverage = if claims.is_empty() {
        0.0
    } else {
        evidenced as f64 / claims.len() as f64
    };
    if let Some(gates) = verification.get_mut("gates").and_then(Value::as_object_mut) {
        gates.insert("evidence_coverage".into(), json!(evidence_coverage));
        gates.insert("evidenced_claims".into(), json!(evidenced));
        gates.insert("total_claims".into(), json!(claims.len()));
    }
    let manifest =
        evidence::write_run_manifest(&runtime.pool, &runtime.data_dir, session_id, &snapshot)
            .await?;
    if let Some(object) = normalized.as_object_mut() {
        object.insert("validation".into(), verification.clone());
        object.insert(
            "source_snapshot".into(),
            json!({
                "snapshot_id": snapshot.snapshot_id,
                "content_hash": snapshot.content_hash,
                "locator": snapshot.locator,
                "resolved": snapshot.resolved,
                "bytes": snapshot.bytes,
                "captured_at": snapshot.captured_at
            }),
        );
        object.insert("claims".into(), serde_json::to_value(&claims)?);
        object.insert(
            "run_manifest".into(),
            json!({
                "manifest_id": manifest.get("manifest_id"),
                "schema_version": manifest.get("schema_version"),
                "created_at": manifest.get("created_at")
            }),
        );
    }
    let submitted = supervisor
        .request_tool(
            runtime,
            "submit_review_candidate",
            json!({
                "session_id": session_id,
                "normalized": normalized,
                "raw_extraction": serde_json::to_string_pretty(
                    extract.get("output").unwrap_or(&Value::Null)
                ).unwrap_or_else(|_| format!("pipeline extract from {locator}"))
            }),
        )
        .await?;
    if let Some(candidate_id) = submitted.get("candidate_id").and_then(Value::as_str) {
        evidence::persist_claims(&runtime.pool, session_id, candidate_id, &claims).await?;
    }
    Ok(json!({
        "locator": locator,
        "artifact_id": artifact,
        "detect": detect.get("output"),
        "verification": verification,
        "title": normalized.get("title"),
        "candidate": submitted
    }))
}

fn verification_report(
    detect: &Value,
    math: &Value,
    provenance: &Value,
    challenge: &Value,
    code: Option<&Value>,
    cross_source: &Value,
) -> Value {
    let detector = detect.pointer("/output/verdict").and_then(Value::as_str);
    let unsupported = provenance
        .pointer("/output/unsupported")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let math_result = math.pointer("/output/result").and_then(Value::as_str);
    let challenged = challenge
        .pointer("/output/challenged")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let code_result = code
        .and_then(|run| run.pointer("/output/result"))
        .and_then(Value::as_str)
        .unwrap_or("NOT_TESTED");
    let corroboration = cross_source
        .pointer("/output/certainty")
        .and_then(Value::as_str)
        .unwrap_or("NOT_INCREASED");
    let mandatory_pass = detector == Some("YES")
        && unsupported == 0
        && math_result != Some("FAIL")
        && !challenged
        && code_result != "FAIL";
    let contested = detector != Some("YES")
        || unsupported > 0
        || challenged
        || math_result == Some("FAIL")
        || code_result == "FAIL";
    let status = if mandatory_pass {
        "VERIFIED"
    } else if contested {
        "NEEDS_HUMAN"
    } else {
        "UNVERIFIED"
    };
    json!({
        "status": status,
        "lifecycle": if mandatory_pass { "REVIEW_READY" } else { "CONTESTED" },
        "mandatory_pass": mandatory_pass,
        "gates": {
            "detector": detector.unwrap_or("UNKNOWN"),
            "provenance_unsupported_claims": unsupported,
            "math": math_result.unwrap_or("NOT_TESTED"),
            "hallucination_challenged": challenged,
            "code": code_result,
            "cross_source": corroboration
        },
        "claims": claim_verdicts(provenance),
        "epistemic_classes": [
            "SOURCE_STATED",
            "MECHANICALLY_VERIFIED",
            "MODEL_INFERRED",
            "HUMAN_CONFIRMED",
            "UNKNOWN"
        ]
    })
}

fn claim_strings(extraction: &Value) -> Vec<String> {
    ["title", "summary", "core_idea", "math"]
        .iter()
        .filter_map(|key| extraction.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|claim| !claim.is_empty())
        .map(str::to_owned)
        .collect()
}

fn claim_verdicts(provenance: &Value) -> Vec<Value> {
    provenance
        .pointer("/output/claims")
        .and_then(Value::as_array)
        .map(|claims| {
            claims
                .iter()
                .map(|claim| {
                    json!({
                        "claim": claim.get("claim"),
                        "supported": claim.get("supported"),
                        "epistemic_status": if claim
                            .get("supported")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            "SOURCE_STATED"
                        } else {
                            "UNKNOWN"
                        }
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn session_summary(
    runtime: &AgentRuntime,
    session_id: &str,
) -> Result<Value, AgentError> {
    let session = persist::get_session(&runtime.pool, session_id).await?;
    let runs = persist::list_runs(&runtime.pool, session_id).await?;
    Ok(json!({
        "session": session,
        "runs": runs.len()
    }))
}

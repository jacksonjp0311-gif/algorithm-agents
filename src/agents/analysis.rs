use serde_json::json;

use super::{ExecContext, HandlerResult, PendingMessage, extraction_skeleton, load_doc};
use crate::error::AgentError;
use crate::parse::parse_all_documents;

pub async fn algorithm_detector(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let (verdict, reason) = if doc.ambiguous {
        ("UNCERTAIN", doc.ambiguity_reason.clone())
    } else if doc.algorithm_present == Some(false) {
        ("NO", "source labels ALGORITHM as no".into())
    } else if doc.algorithm_present == Some(true)
        || (!doc.pseudocode.is_empty() && !doc.equations.is_empty())
    {
        ("YES", "source contains an explicit computational procedure".into())
    } else if !doc.pseudocode.is_empty() || !doc.equations.is_empty() {
        ("UNCERTAIN", "partial procedure cues without a clear algorithm label".into())
    } else {
        ("NO", "no reconstructable procedure found".into())
    };
    Ok(HandlerResult::complete(
        "EXTRACTION",
        json!({
            "verdict": verdict,
            "reason": reason,
            "title": doc.title
        }),
        &reason,
    ))
}

pub async fn algorithm_extractor(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (origin, doc, _text) = load_doc(ctx, input).await?;
    if doc.algorithm_present == Some(false) {
        return Ok(HandlerResult::complete(
            "EXTRACTION",
            json!({ "extracted": false, "reason": "source is not an algorithm" }),
            "source is not an algorithm",
        )
        .with_state("BLOCKED"));
    }
    let docs = parse_all_documents(&_text);
    let primary = docs.first().cloned().unwrap_or_else(|| doc.clone());
    let emergent: Vec<_> = docs
        .iter()
        .skip(1)
        .map(|item| {
            json!({
                "role": "emergent",
                "note": "Latent or secondary procedure in the source — not the named subject",
                "extraction": extraction_skeleton(item)
            })
        })
        .collect();
    let mut result = HandlerResult::complete(
        "EXTRACTION",
        json!({
            "extraction": extraction_skeleton(&primary),
            "emergent": emergent,
            "origin": origin,
            "ambiguous": primary.ambiguous,
            "role": primary.role
        }),
        if emergent.is_empty() {
            "normalized labeled procedure fields into an extraction candidate"
        } else {
            "extracted the named procedure and latent companion procedures from the same source"
        },
    );
    result.messages.push(PendingMessage {
        to_agent: "math_analyst".into(),
        r#type: "ANALYSIS_REQUEST".into(),
        artifact_id: origin.clone(),
        payload: json!({
            "reason": "extractor requests independent math analysis",
            "title": doc.title
        }),
    });
    if doc.ambiguous {
        result.state = "LOW_CONFIDENCE".into();
        result.reason = primary.ambiguity_reason.clone();
    }
    Ok(result)
}

pub async fn math_analyst(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let mut result = HandlerResult::complete(
        "MATH_ANALYSIS",
        json!({
            "equations": doc.equations,
            "variables": doc.variables,
            "operators": infer_operators(&doc.equations),
            "recurrences": doc.equations.iter().filter(|eq| eq.contains("t+") || eq.contains("k+1")).cloned().collect::<Vec<_>>(),
            "constraints": doc.constraints,
            "assumptions": doc.assumptions,
            "objective_functions": doc.equations.iter().filter(|eq| eq.contains("min") || eq.contains("max") || eq.contains("argmin")).cloned().collect::<Vec<_>>(),
            "update_rules": doc.equations.clone(),
            "ambiguous": doc.ambiguous
        }),
        "extracted labeled mathematical structure",
    );
    if doc.ambiguous {
        result = result.with_state("NEEDS_HUMAN");
        result.reason = if doc.ambiguity_reason.is_empty() {
            "mathematical reading is ambiguous".into()
        } else {
            doc.ambiguity_reason
        };
    }
    Ok(result)
}

pub async fn code_analyst(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "CODE_ANALYSIS",
        json!({
            "algorithm_structure": if doc.pseudocode.is_empty() { "UNKNOWN" } else { "explicit_pseudocode" },
            "state": doc.variables,
            "data_flow": doc.equations,
            "important_functions": function_names(&doc.reference_code),
            "implementation_constraints": doc.constraints,
            "complexity_clues": [doc.complexity_time.clone(), doc.complexity_space.clone()],
            "language": doc.reference_language
        }),
        "analyzed supplied implementation material only",
    ))
}

pub async fn complexity_analyst(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let origin = if doc.complexity_origin.is_empty() {
        "UNKNOWN"
    } else {
        doc.complexity_origin.as_str()
    };
    Ok(HandlerResult::complete(
        "EXTRACTION",
        json!({
            "time_complexity": empty_unknown(&doc.complexity_time),
            "space_complexity": empty_unknown(&doc.complexity_space),
            "scaling": empty_unknown(&doc.complexity_time),
            "origin": origin
        }),
        &format!("complexity origin marked {origin}"),
    ))
}

pub async fn assumption_analyst(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "EXTRACTION",
        json!({ "assumptions": doc.assumptions }),
        "listed assumptions stated in the source",
    ))
}

pub async fn failure_mode_analyst(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "EXTRACTION",
        json!({
            "edge_cases": doc.failures,
            "numerical_instability": doc.failures.iter().filter(|item| item.to_ascii_lowercase().contains("numeric") || item.to_ascii_lowercase().contains("overflow")).cloned().collect::<Vec<_>>(),
            "invalid_input_conditions": doc.constraints,
            "convergence_risks": doc.failures.iter().filter(|item| item.to_ascii_lowercase().contains("converg")).cloned().collect::<Vec<_>>(),
            "known_limitations": doc.failures,
            "failure_conditions": doc.failures
        }),
        "collected labeled failure conditions",
    ))
}

fn infer_operators(equations: &[String]) -> Vec<String> {
    let mut ops = Vec::new();
    for token in ["min", "max", "sum", "+", "-", "*", "/", "="] {
        if equations.iter().any(|eq| eq.contains(token)) {
            ops.push(token.to_owned());
        }
    }
    ops
}

fn function_names(code: &str) -> Vec<String> {
    code.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("def ")
                .or_else(|| line.strip_prefix("fn "))
                .map(|rest| rest.split(['(', ' ']).next().unwrap_or(rest).to_owned())
        })
        .collect()
}

fn empty_unknown(value: &str) -> &str {
    if value.trim().is_empty() { "UNKNOWN" } else { value }
}

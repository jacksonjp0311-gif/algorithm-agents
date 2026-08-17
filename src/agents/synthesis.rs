use serde_json::{Value, json};

use super::{ExecContext, HandlerResult, extraction_skeleton, load_doc};
use crate::error::AgentError;

pub async fn normalizer(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let extraction = if let Some(value) = input.get("extraction") {
        value.clone()
    } else {
        let (_origin, doc, _text) = load_doc(ctx, input).await?;
        extraction_skeleton(&doc)
    };
    let mut normalized = extraction;
    if let Some(obj) = normalized.as_object_mut() {
        obj.entry("publication")
            .or_insert(json!({ "state": "draft" }));
        obj.entry("validation")
            .or_insert(json!({ "status": "UNVERIFIED" }));
        obj.entry("provenance").or_insert(json!({
            "class": "SOURCE_RECONSTRUCTED",
            "extraction_method": "agent_system",
            "extractor_name": "normalizer",
            "extractor_version": ctx.agent_version,
            "confidence": 0.6
        }));
    }
    Ok(HandlerResult::complete(
        "FINAL_CANDIDATE",
        json!({ "normalized": normalized }),
        "normalized heterogeneous agent output into the archive candidate shape",
    ))
}

pub async fn summarizer(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "FINAL_CANDIDATE",
        json!({
            "card": doc.abstract_text,
            "detail": format!(
                "{}\n\nCore idea: {}\n\nAssumptions: {}",
                doc.abstract_text,
                doc.equations.first().cloned().unwrap_or_default(),
                doc.assumptions.join("; ")
            ),
            "title": doc.title
        }),
        "produced human-facing summary text from source fields",
    ))
}

pub async fn code_translator(
    ctx: &ExecContext<'_>,
    input: &Value,
) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let (code, origin_label) = if !doc.reference_code.trim().is_empty() {
        (
            doc.reference_code.clone(),
            "permissibly supplied source implementation",
        )
    } else if !doc.pseudocode.trim().is_empty() {
        (
            format!(
                "# GENERATED REFERENCE IMPLEMENTATION\n# Translated from labeled pseudocode. Not a verbatim source extract.\n\n{}",
                comment_block(&doc.pseudocode)
            ),
            "GENERATED REFERENCE IMPLEMENTATION",
        )
    } else {
        (
            "# GENERATED REFERENCE IMPLEMENTATION\n# Insufficient source material; placeholder only.\nraise NotImplementedError\n".into(),
            "GENERATED REFERENCE IMPLEMENTATION",
        )
    };
    Ok(HandlerResult::complete(
        "REFERENCE_IMPLEMENTATION",
        json!({
            "language": if doc.reference_language.is_empty() { "python" } else { doc.reference_language.as_str() },
            "code": code,
            "label": origin_label
        }),
        origin_label,
    ))
}

pub async fn experiment_designer(
    ctx: &ExecContext<'_>,
    input: &Value,
) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "EXPERIMENT_PLAN",
        json!({
            "proposed_tests": [
                format!("Execute a tiny {} instance and compare against a known hand result", doc.title),
                "Perturb inputs that violate stated assumptions and record failure",
                "If a reference implementation exists, run the bounded code verifier"
            ],
            "validation_status": "NOT_PROMOTED",
            "note": "this agent proposes tests only"
        }),
        "proposed bounded experiments without promoting validation status",
    ))
}

fn comment_block(pseudocode: &str) -> String {
    pseudocode
        .lines()
        .map(|line| format!("# {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

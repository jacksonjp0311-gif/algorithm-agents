use serde_json::json;

use super::{ExecContext, HandlerResult, load_doc};
use crate::error::AgentError;
use crate::parse::contains_claim;
use crate::sandbox;

pub async fn provenance_checker(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, text) = load_doc(ctx, input).await?;
    let claims = collect_claims(input, &doc);
    let mut report = Vec::new();
    let mut unsupported = 0;
    for claim in claims {
        let supported = contains_claim(&text, &claim) || contains_claim(&doc.body, &claim);
        if !supported {
            unsupported += 1;
        }
        report.push(json!({
            "claim": claim,
            "supported": supported
        }));
    }
    let (state, reason) = if doc.ambiguous {
        (
            "LOW_CONFIDENCE",
            if doc.ambiguity_reason.is_empty() {
                "source admits more than one reading".into()
            } else {
                doc.ambiguity_reason.clone()
            },
        )
    } else if unsupported > 0 {
        ("LOW_CONFIDENCE", format!("{unsupported} claim(s) lack source support"))
    } else {
        ("COMPLETE", "checked claims against the supplied source".into())
    };
    Ok(HandlerResult::complete(
        "VERIFICATION_REPORT",
        json!({
            "claims": report,
            "ambiguous": doc.ambiguous,
            "unsupported": unsupported
        }),
        &reason,
    )
    .with_state(state))
}

pub async fn cross_source_verifier(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let others = crate::collect::list_fixture_files(&ctx.runtime.root)?;
    let mut independent = Vec::new();
    for path in others {
        let other = std::fs::read_to_string(&path)?;
        if other == doc.body {
            continue;
        }
        if !doc.title.is_empty() && crate::parse::contains_claim(&other, &doc.title) {
            independent.push(crate::collect::fixture_locator(&ctx.runtime.root, &path));
        }
    }
    Ok(HandlerResult::complete(
        "VERIFICATION_REPORT",
        json!({
            "independent_sources": independent,
            "certainty": if independent.is_empty() { "NOT_INCREASED" } else { "CORROBORATED_TITLE_ONLY" },
            "note": "title overlap is not treated as full confirmation"
        }),
        "looked for independent fixture corroboration without manufacturing certainty",
    ))
}

pub async fn math_checker(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    if doc.equations.is_empty() {
        return Ok(HandlerResult::complete(
            "VERIFICATION_REPORT",
            json!({ "result": "NOT_TESTED", "reason": "no equations to check" }),
            "no equations to check",
        ));
    }
    if doc.ambiguous {
        return Ok(HandlerResult::complete(
            "VERIFICATION_REPORT",
            json!({
                "result": "UNCERTAIN",
                "reason": doc.ambiguity_reason,
                "equations": doc.equations
            }),
            "ambiguous equations were not forced into a single reading",
        )
        .with_state("NEEDS_HUMAN"));
    }
    let mut failed = Vec::new();
    for eq in &doc.equations {
        if !balanced(eq) {
            failed.push(eq.clone());
        }
    }
    let result = if failed.is_empty() { "PASS" } else { "FAIL" };
    Ok(HandlerResult::complete(
        "VERIFICATION_REPORT",
        json!({
            "result": result,
            "failed": failed,
            "checked": "balanced delimiters and explicit source equations only"
        }),
        &format!("bounded math check: {result}"),
    ))
}

pub async fn code_verifier(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let language = input
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("python");
    let code = if let Some(code) = input.get("source").and_then(|v| v.as_str()) {
        code.to_owned()
    } else {
        let (_origin, doc, _text) = load_doc(ctx, input).await?;
        doc.reference_code
    };
    let report = sandbox::verify_reference_code(language, &code, &ctx.runtime.permissions).await;
    let result = report
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or("NOT_TESTED")
        .to_owned();
    let state = match result.as_str() {
        "FAIL" => "FAILED",
        _ => "COMPLETE",
    };
    Ok(HandlerResult::complete("VERIFICATION_REPORT", report, &format!("code verifier: {result}"))
        .with_state(state))
}

pub async fn hallucination_challenger(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, text) = load_doc(ctx, input).await?;
    let mut challenges = Vec::new();
    if let Some(extraction) = input.get("extraction").or_else(|| input.get("claims")) {
        if let Some(obj) = extraction.as_object() {
            for (key, value) in obj {
                let claim = value.as_str().unwrap_or(&value.to_string()).to_owned();
                if claim.trim().is_empty() {
                    continue;
                }
                if !contains_claim(&text, &claim) && !contains_claim(&doc.body, &claim) {
                    challenges.push(json!({
                        "field": key,
                        "claim": claim,
                        "challenge": "unsupported attribution or invented content"
                    }));
                }
            }
        }
    }
    if doc.ambiguous {
        challenges.push(json!({
            "field": "ambiguity",
            "claim": doc.ambiguity_reason,
            "challenge": "source does not uniquely support a single reading"
        }));
    }
    let state = if challenges.is_empty() {
        "COMPLETE"
    } else {
        "LOW_CONFIDENCE"
    };
    Ok(HandlerResult::complete(
        "VERIFICATION_REPORT",
        json!({
            "challenges": challenges,
            "challenged": !challenges.is_empty()
        }),
        if challenges.is_empty() {
            "no unsupported claims were found in the supplied extraction"
        } else {
            "weakened claims that are not grounded in the source"
        },
    )
    .with_state(state))
}

fn collect_claims(input: &serde_json::Value, doc: &crate::parse::SourceDoc) -> Vec<String> {
    let mut claims = Vec::new();
    if let Some(list) = input.get("claims").and_then(|v| v.as_array()) {
        for item in list {
            if let Some(text) = item.as_str() {
                claims.push(text.to_owned());
            } else if let Some(text) = item.get("claim").and_then(|v| v.as_str()) {
                claims.push(text.to_owned());
            }
        }
    }
    if claims.is_empty() {
        claims.extend(doc.equations.clone());
        claims.extend(doc.assumptions.clone());
        if !doc.title.is_empty() {
            claims.push(doc.title.clone());
        }
    }
    claims
}

fn balanced(text: &str) -> bool {
    let mut paren = 0i32;
    let mut bracket = 0i32;
    for ch in text.chars() {
        match ch {
            '(' => paren += 1,
            ')' => paren -= 1,
            '[' => bracket += 1,
            ']' => bracket -= 1,
            _ => {}
        }
        if paren < 0 || bracket < 0 {
            return false;
        }
    }
    paren == 0 && bracket == 0
}

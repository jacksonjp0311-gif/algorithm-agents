use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::{ExecContext, HandlerResult, load_doc};
use crate::collect::{self, fixture_locator, list_local_sources, live_fetch_blocked};
use crate::error::AgentError;
use crate::parse::{keyword_score, parse_labeled_document};

pub async fn source_scout(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let objective = input
        .get("objective")
        .and_then(|v| v.as_str())
        .unwrap_or(&ctx.session.objective);
    let mut scored = Vec::new();
    for path in list_local_sources(&ctx.runtime.root)? {
        let text = std::fs::read_to_string(&path)?;
        let doc = parse_labeled_document(&text);
        let score = keyword_score(objective, &doc);
        if score == 0 && !objective.is_empty() {
            continue;
        }
        scored.push((score, fixture_locator(&ctx.runtime.root, &path), doc));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let limit = ctx.session.max_sources.max(1) as usize;
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".into());
    let mut sources: Vec<Value> = scored
        .into_iter()
        .take(limit)
        .map(|(score, locator, doc)| {
            json!({
                "url": if doc.url.is_empty() { locator.clone() } else { doc.url },
                "locator": locator,
                "title": doc.title,
                "source_type": doc.source_type,
                "discovery_reason": format!("keyword score {score} against session objective"),
                "retrieved_at": now
            })
        })
        .collect();
    if let Some(urls) = input.get("urls").and_then(|v| v.as_array()) {
        for url in urls.iter().filter_map(|v| v.as_str()) {
            sources.push(json!({
                "url": url,
                "locator": url,
                "title": url,
                "source_type": "url",
                "discovery_reason": "operator-supplied URL",
                "retrieved_at": now
            }));
        }
    }
    Ok(HandlerResult::complete(
        "SOURCE",
        json!({ "sources": sources, "objective": objective }),
        &format!("discovered {} sources", sources.len()),
    ))
}

pub async fn web_collector(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let locator = input
        .get("locator")
        .or_else(|| input.get("url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Invalid("web_collector needs locator or url".into()))?;
    if let Some(reason) = live_fetch_blocked(&ctx.runtime.permissions, locator) {
        return Ok(HandlerResult::complete(
            "SOURCE_TEXT",
            json!({ "blocked": true, "reason": reason, "locator": locator }),
            &reason,
        )
        .with_state("BLOCKED"));
    }
    let (resolved, raw) = if locator.starts_with("http://") || locator.starts_with("https://") {
        let fetch_url = collect::canonical_fetch_url(locator);
        crate::fetch::fetch_url(&ctx.runtime.permissions, &fetch_url).await?
    } else {
        collect::resolve_source(&ctx.runtime.root, locator)?
    };
    let text = collect::normalize_source_text(locator, &raw);
    if text.len() > ctx.runtime.permissions.max_fetch_bytes {
        return Err(AgentError::Budget("retrieved source exceeds size limit".into()));
    }
    Ok(HandlerResult::complete(
        "SOURCE_TEXT",
        json!({
            "locator": locator,
            "resolved": resolved,
            "text": text,
            "bytes": text.len()
        }),
        "loaded permitted source",
    ))
}

pub async fn paper_analyst(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let (origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "PAPER_ANALYSIS",
        json!({
            "title": doc.title,
            "authors": doc.authors,
            "abstract": doc.abstract_text,
            "sections": doc.sections.iter().map(|(n, b)| json!({"name": n, "excerpt": excerpt(b)})).collect::<Vec<_>>(),
            "equations": doc.equations,
            "algorithms": if doc.algorithm_present.unwrap_or(!doc.pseudocode.is_empty()) { vec![doc.title.clone()] } else { Vec::<String>::new() },
            "references": doc.citations,
            "methodology_clues": doc.assumptions,
            "origin": origin
        }),
        "extracted labeled paper fields; full document was not copied into the archive",
    ))
}

pub async fn repository_scout(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let (origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "SOURCE",
        json!({
            "repository": doc.repository,
            "path": origin,
            "commit_ref": doc.fields.get("COMMIT").cloned().unwrap_or_default(),
            "language": doc.reference_language,
            "license": doc.license,
            "title": doc.title
        }),
        "recorded repository metadata from the supplied source",
    ))
}

pub async fn citation_walker(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let max_depth = input
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .min(2) as usize;
    let max_sources = ctx.session.max_sources.max(1) as usize;
    let mut walked = Vec::new();
    walk_citations(
        &ctx.runtime.root,
        &doc.citations,
        1,
        max_depth,
        max_sources,
        &mut walked,
    )?;
    Ok(HandlerResult::complete(
        "SOURCE",
        json!({
            "citations": walked,
            "max_depth": max_depth,
            "bounded": true
        }),
        "followed labeled citations within session budget",
    ))
}

fn walk_citations(
    root: &std::path::Path,
    citations: &[String],
    depth: usize,
    max_depth: usize,
    max_sources: usize,
    walked: &mut Vec<Value>,
) -> Result<(), AgentError> {
    if depth > max_depth {
        return Ok(());
    }
    for citation in citations {
        if walked.len() >= max_sources {
            break;
        }
        if let Some(locator) = citation.strip_prefix("fixture://") {
            if let Ok((_, text)) = collect::resolve_source(root, &format!("fixture://{locator}")) {
                let child = parse_labeled_document(&text);
                walked.push(json!({
                    "locator": format!("fixture://{locator}"),
                    "title": child.title,
                    "depth": depth
                }));
                if depth < max_depth {
                    walk_citations(root, &child.citations, depth + 1, max_depth, max_sources, walked)?;
                }
            }
        } else {
            walked.push(json!({
                "locator": citation,
                "title": citation,
                "depth": depth,
                "followed": false,
                "reason": "non-fixture citations are recorded but not crawled"
            }));
        }
    }
    Ok(())
}

fn excerpt(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 280 {
        trimmed.to_owned()
    } else {
        format!("{}…", &trimmed[..280])
    }
}

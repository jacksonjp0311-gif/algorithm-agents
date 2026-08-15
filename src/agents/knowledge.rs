use serde_json::json;

use super::{ExecContext, HandlerResult, extraction_skeleton, load_doc};
use crate::error::AgentError;

pub async fn deduplicator(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let published = ctx.runtime.host.list_published_algorithms().await?;
    let core = doc.equations.first().cloned().unwrap_or_default();
    let (verdict, matches) =
        crate::archive_match::verdict_against_archive(&doc.title, &doc.title, &core, &published);
    Ok(HandlerResult::complete(
        "RELATIONSHIP_REPORT",
        json!({
            "verdict": verdict,
            "matches": matches,
            "candidate": extraction_skeleton(&doc)
        }),
        &format!("archive comparison: {verdict}"),
    ))
}

pub async fn relationship_mapper(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let published = ctx.runtime.host.list_published_algorithms().await?;
    let related_algorithms: Vec<_> = published
        .iter()
        .filter(|item| {
            (!doc.domain.is_empty() && item.domain.eq_ignore_ascii_case(&doc.domain))
                || item.tags.iter().any(|tag| doc.tags.iter().any(|mine| mine.eq_ignore_ascii_case(tag)))
        })
        .map(|item| json!({ "id": item.id, "title": item.title, "kind": "algorithm" }))
        .collect();
    Ok(HandlerResult::complete(
        "RELATIONSHIP_REPORT",
        json!({
            "algorithms": related_algorithms,
            "research": [],
            "projects": [],
            "notes": [],
            "labs": []
        }),
        "mapped related published algorithms by domain/tags only",
    ))
}

pub async fn structural_matcher(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let motifs = if doc.motifs.is_empty() {
        infer_motifs(&doc)
    } else {
        doc.motifs.clone()
    };
    Ok(HandlerResult::complete(
        "RELATIONSHIP_REPORT",
        json!({
            "motifs": motifs,
            "equivalence_claimed": false,
            "note": "motifs are conservative labels, not equivalence proofs"
        }),
        "assigned conservative structural motifs",
    ))
}

pub async fn domain_classifier(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    let mut tags = doc.tags.clone();
    if tags.is_empty() && !doc.domain.is_empty() {
        tags.push(doc.domain.clone());
    }
    Ok(HandlerResult::complete(
        "EXTRACTION",
        json!({
            "domain": doc.domain,
            "tags": tags
        }),
        "classified from labeled domain and tags",
    ))
}

pub async fn use_case_mapper(ctx: &ExecContext<'_>, input: &serde_json::Value) -> Result<HandlerResult, AgentError> {
    let (_origin, doc, _text) = load_doc(ctx, input).await?;
    Ok(HandlerResult::complete(
        "EXTRACTION",
        json!({
            "known_uses": doc.known_uses,
            "potential_uses": doc.potential_uses,
            "separated": true
        }),
        "kept known uses separate from potential uses",
    ))
}

fn infer_motifs(doc: &crate::parse::SourceDoc) -> Vec<String> {
    let blob = format!("{} {} {}", doc.abstract_text, doc.pseudocode, doc.equations.join(" ")).to_ascii_lowercase();
    let mut motifs = Vec::new();
    if blob.contains("estimate") && blob.contains("correct") {
        motifs.push("observe → estimate → correct".into());
    }
    if blob.contains("weight") && blob.contains("resample") {
        motifs.push("sample → weight → resample".into());
    }
    if blob.contains("relax") || blob.contains("priority") || blob.contains("shortest") {
        motifs.push("select → route → aggregate".into());
    }
    motifs
}

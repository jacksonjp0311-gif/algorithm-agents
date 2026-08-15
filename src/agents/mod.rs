mod acquisition;
mod analysis;
mod knowledge;
mod synthesis;
mod verification;

use serde_json::{Value, json};

use crate::error::AgentError;
use crate::parse::{SourceDoc, parse_source_document};
use crate::persist::{self, Session};
use crate::runtime::AgentRuntime;

pub struct ExecContext<'a> {
    pub runtime: &'a AgentRuntime,
    pub session: &'a Session,
    pub agent_id: &'a str,
    pub agent_version: &'a str,
}

#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub to_agent: String,
    pub r#type: String,
    pub artifact_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct HandlerResult {
    pub state: String,
    pub reason: String,
    pub output: Value,
    pub artifact_type: String,
    pub messages: Vec<PendingMessage>,
}

impl HandlerResult {
    pub fn complete(artifact_type: &str, output: Value, reason: &str) -> Self {
        Self {
            state: "COMPLETE".into(),
            reason: reason.into(),
            output,
            artifact_type: artifact_type.into(),
            messages: Vec::new(),
        }
    }

    pub fn with_state(mut self, state: &str) -> Self {
        self.state = state.into();
        self
    }
}

pub async fn execute(ctx: &ExecContext<'_>, input: &Value) -> Result<HandlerResult, AgentError> {
    match ctx.agent_id {
        "source_scout" => acquisition::source_scout(ctx, input).await,
        "web_collector" => acquisition::web_collector(ctx, input).await,
        "paper_analyst" => acquisition::paper_analyst(ctx, input).await,
        "repository_scout" => acquisition::repository_scout(ctx, input).await,
        "citation_walker" => acquisition::citation_walker(ctx, input).await,
        "algorithm_detector" => analysis::algorithm_detector(ctx, input).await,
        "algorithm_extractor" => analysis::algorithm_extractor(ctx, input).await,
        "math_analyst" => analysis::math_analyst(ctx, input).await,
        "code_analyst" => analysis::code_analyst(ctx, input).await,
        "complexity_analyst" => analysis::complexity_analyst(ctx, input).await,
        "assumption_analyst" => analysis::assumption_analyst(ctx, input).await,
        "failure_mode_analyst" => analysis::failure_mode_analyst(ctx, input).await,
        "provenance_checker" => verification::provenance_checker(ctx, input).await,
        "cross_source_verifier" => verification::cross_source_verifier(ctx, input).await,
        "math_checker" => verification::math_checker(ctx, input).await,
        "code_verifier" => verification::code_verifier(ctx, input).await,
        "hallucination_challenger" => verification::hallucination_challenger(ctx, input).await,
        "deduplicator" => knowledge::deduplicator(ctx, input).await,
        "relationship_mapper" => knowledge::relationship_mapper(ctx, input).await,
        "structural_matcher" => knowledge::structural_matcher(ctx, input).await,
        "domain_classifier" => knowledge::domain_classifier(ctx, input).await,
        "use_case_mapper" => knowledge::use_case_mapper(ctx, input).await,
        "normalizer" => synthesis::normalizer(ctx, input).await,
        "summarizer" => synthesis::summarizer(ctx, input).await,
        "code_translator" => synthesis::code_translator(ctx, input).await,
        "experiment_designer" => synthesis::experiment_designer(ctx, input).await,
        other => Err(AgentError::NotFound(format!(
            "no executor registered for `{other}`"
        ))),
    }
}

pub async fn load_doc(ctx: &ExecContext<'_>, input: &Value) -> Result<(String, SourceDoc, String), AgentError> {
    if let Some(text) = input.get("source_text").and_then(|v| v.as_str()) {
        return Ok((String::new(), parse_source_document(text), text.to_owned()));
    }
    if let Some(locator) = input.get("locator").and_then(|v| v.as_str()) {
        if locator.starts_with("fixture://")
            || locator.starts_with("sources://")
            || locator.starts_with("file://")
        {
            let (_path, text) = crate::collect::resolve_source(&ctx.runtime.root, locator)?;
            return Ok((locator.to_owned(), parse_source_document(&text), text));
        }
    }
    if let Some(artifact_id) = input.get("artifact_id").and_then(|v| v.as_str()) {
        let (_meta, content) = persist::get_artifact(&ctx.runtime.pool, artifact_id).await?;
        let text = content
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| content.get("body").and_then(|v| v.as_str()))
            .or_else(|| content.get("source_text").and_then(|v| v.as_str()))
            .map(|s| s.to_owned())
            .unwrap_or_else(|| content.to_string());
        return Ok((artifact_id.to_owned(), parse_source_document(&text), text));
    }
    Err(AgentError::Invalid(
        "agent input needs artifact_id, locator, or source_text".into(),
    ))
}

pub fn extraction_skeleton(doc: &SourceDoc) -> Value {
    json!({
        "title": doc.title,
        "short_name": if doc.role == "emergent" {
            format!("{} (latent)", doc.title)
        } else {
            doc.title.clone()
        },
        "domain": doc.domain,
        "summary": doc.abstract_text,
        "plain_language_explanation": doc.abstract_text,
        "core_idea": doc.equations.first().cloned().unwrap_or_default(),
        "why_it_matters": if doc.role == "emergent" {
            "Latent companion procedure in the source; not the named subject of the document.".to_owned()
        } else {
            doc.abstract_text.clone()
        },
        "math": doc.equations.join("\n"),
        "variables": doc.variables,
        "assumptions": doc.assumptions,
        "constraints": doc.constraints,
        "complexity": format!(
            "time={} space={} origin={}",
            empty_unknown(&doc.complexity_time),
            empty_unknown(&doc.complexity_space),
            doc.complexity_origin
        ),
        "pseudocode": doc.pseudocode,
        "reference_code": doc.reference_code,
        "reference_language": doc.reference_language,
        "known_uses": doc.known_uses,
        "potential_uses": doc.potential_uses,
        "source": {
            "type": doc.source_type,
            "title": doc.title,
            "authors": doc.authors,
            "publication_date": doc.year,
            "url": doc.url,
            "repository_url": doc.repository,
            "notes": doc.license
        },
        "provenance": {
            "class": if doc.role == "emergent" { "LATENT_STRUCTURE" } else { "SOURCE_RECONSTRUCTED" },
            "extraction_method": "agent_system",
            "extractor_name": "algorithm_extractor",
            "extractor_version": "1.1.0",
            "confidence": if doc.role == "emergent" { 0.62 } else { 0.84 },
            "evidence_notes": if doc.role == "emergent" {
                "Companion procedure reconstructed from the same source. Not the named subject of the document."
            } else {
                "Normalized from labeled source fields. Private until human review."
            }
        },
        "publication": { "state": "draft" },
        "tags": doc.tags
    })
}

fn empty_unknown(value: &str) -> &str {
    if value.trim().is_empty() { "UNKNOWN" } else { value }
}

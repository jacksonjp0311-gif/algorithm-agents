use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AgentError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PublishedAlgorithm {
    pub id: String,
    pub archive_id: String,
    pub title: String,
    pub short_name: String,
    pub slug: String,
    pub domain: String,
    pub summary: String,
    pub core_idea: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewCandidate {
    pub session_id: String,
    pub raw_extraction: String,
    pub normalized: Value,
    pub review_state: String,
    pub verification: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubmittedCandidate {
    pub candidate_id: String,
    pub review_state: String,
}

#[async_trait]
pub trait ArchiveHost: Send + Sync {
    async fn list_published_algorithms(&self) -> Result<Vec<PublishedAlgorithm>, AgentError>;
    async fn submit_extraction_candidate(
        &self,
        candidate: ReviewCandidate,
    ) -> Result<SubmittedCandidate, AgentError>;
}

#[derive(Default)]
pub struct MemoryHost {
    pub published: Vec<PublishedAlgorithm>,
    pub submitted: tokio::sync::Mutex<Vec<ReviewCandidate>>,
}

#[async_trait]
impl ArchiveHost for MemoryHost {
    async fn list_published_algorithms(&self) -> Result<Vec<PublishedAlgorithm>, AgentError> {
        Ok(self.published.clone())
    }

    async fn submit_extraction_candidate(
        &self,
        candidate: ReviewCandidate,
    ) -> Result<SubmittedCandidate, AgentError> {
        let mut submitted = self.submitted.lock().await;
        submitted.push(candidate);
        let review_state = submitted
            .last()
            .map(|candidate| candidate.review_state.clone())
            .unwrap_or_else(|| "NEEDS_HUMAN".into());
        Ok(SubmittedCandidate {
            candidate_id: format!("EXT / {:04}", submitted.len()),
            review_state,
        })
    }
}

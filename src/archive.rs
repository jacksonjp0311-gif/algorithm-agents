use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::error::AgentError;
use crate::host::{ArchiveHost, PublishedAlgorithm, ReviewCandidate, SubmittedCandidate};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCandidate {
    pub candidate_id: String,
    pub review_state: String,
    pub review_notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub raw_extraction: String,
    pub normalized: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct QueueFile {
    next: u64,
    items: Vec<QueuedCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ArchiveFile {
    items: Vec<PublishedAlgorithm>,
}

pub struct FileArchive {
    dir: PathBuf,
    queue: Mutex<QueueFile>,
    published: Mutex<ArchiveFile>,
}

impl FileArchive {
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self, AgentError> {
        let dir = data_dir.as_ref().join("archive");
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::create_dir_all(dir.join("accepted")).await?;
        let queue = read_json(&dir.join("queue.json"))
            .await?
            .unwrap_or(QueueFile {
                next: 1,
                items: Vec::new(),
            });
        let published = read_json(&dir.join("algorithms.json"))
            .await?
            .unwrap_or_default();
        Ok(Self {
            dir,
            queue: Mutex::new(queue),
            published: Mutex::new(published),
        })
    }

    pub async fn list_queue(&self) -> Vec<QueuedCandidate> {
        self.queue.lock().await.items.clone()
    }

    pub async fn list_accepted(&self) -> Vec<PublishedAlgorithm> {
        self.published.lock().await.items.clone()
    }

    pub async fn get_candidate(&self, id: &str) -> Result<QueuedCandidate, AgentError> {
        self.queue
            .lock()
            .await
            .items
            .iter()
            .find(|item| item.candidate_id.eq_ignore_ascii_case(id))
            .cloned()
            .ok_or_else(|| AgentError::NotFound(format!("candidate `{id}` not found")))
    }

    pub async fn accept(&self, id: &str) -> Result<PublishedAlgorithm, AgentError> {
        let mut queue = self.queue.lock().await;
        let index = queue
            .items
            .iter()
            .position(|item| item.candidate_id.eq_ignore_ascii_case(id))
            .ok_or_else(|| AgentError::NotFound(format!("candidate `{id}` not found")))?;
        let mut item = queue.items.remove(index);
        item.review_state = "ACCEPTED".into();
        item.updated_at = now()?;
        let published = published_from_candidate(&item);
        {
            let mut archive = self.published.lock().await;
            if archive
                .items
                .iter()
                .any(|existing| existing.id == published.id || existing.slug == published.slug)
            {
                queue.items.insert(index, item);
                return Err(AgentError::Denied(
                    "an accepted algorithm with this identity already exists".into(),
                ));
            }
            archive.items.insert(0, published.clone());
            write_json(&self.dir.join("algorithms.json"), &*archive).await?;
        }
        write_json(&self.dir.join("queue.json"), &*queue).await?;
        write_json(
            &self.dir.join("accepted").join(format!("{}.json", published.id)),
            &item.normalized,
        )
        .await?;
        Ok(published)
    }

    pub async fn reject(&self, id: &str, reason: &str) -> Result<QueuedCandidate, AgentError> {
        let mut queue = self.queue.lock().await;
        let item = queue
            .items
            .iter_mut()
            .find(|item| item.candidate_id.eq_ignore_ascii_case(id))
            .ok_or_else(|| AgentError::NotFound(format!("candidate `{id}` not found")))?;
        item.review_state = "REJECTED".into();
        item.review_notes = reason.to_owned();
        item.updated_at = now()?;
        let cloned = item.clone();
        write_json(&self.dir.join("queue.json"), &*queue).await?;
        Ok(cloned)
    }

    async fn persist_queue(&self, queue: &QueueFile) -> Result<(), AgentError> {
        write_json(&self.dir.join("queue.json"), queue).await
    }
}

#[async_trait]
impl ArchiveHost for FileArchive {
    async fn list_published_algorithms(&self) -> Result<Vec<PublishedAlgorithm>, AgentError> {
        Ok(self.published.lock().await.items.clone())
    }

    async fn submit_extraction_candidate(
        &self,
        candidate: ReviewCandidate,
    ) -> Result<SubmittedCandidate, AgentError> {
        let mut queue = self.queue.lock().await;
        let number = queue.next.max(1);
        let now = now()?;
        let item = QueuedCandidate {
            candidate_id: format!("CAND-{number:04}"),
            review_state: "PENDING".into(),
            review_notes: String::new(),
            created_at: now.clone(),
            updated_at: now,
            raw_extraction: candidate.raw_extraction,
            normalized: candidate.normalized,
        };
        queue.next = number + 1;
        queue.items.insert(0, item.clone());
        self.persist_queue(&queue).await?;
        Ok(SubmittedCandidate {
            candidate_id: item.candidate_id,
            review_state: item.review_state,
        })
    }
}

fn published_from_candidate(item: &QueuedCandidate) -> PublishedAlgorithm {
    let n = &item.normalized;
    let title = string_field(n, "title").unwrap_or_else(|| item.candidate_id.clone());
    let slug = slugify(&title);
    PublishedAlgorithm {
        id: slug.clone(),
        archive_id: item.candidate_id.clone(),
        title,
        short_name: string_field(n, "short_name").unwrap_or_default(),
        slug,
        domain: string_field(n, "domain").unwrap_or_default(),
        summary: string_field(n, "summary")
            .or_else(|| string_field(n, "plain_language_explanation"))
            .unwrap_or_default(),
        core_idea: string_field(n, "core_idea").unwrap_or_default(),
        tags: n
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "algorithm".into()
    } else {
        slug.chars().take(80).collect()
    }
}

fn now() -> Result<String, AgentError> {
    crate::persist::now_rfc3339()
}

async fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, AgentError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = tokio::fs::read_to_string(path).await?;
    if raw.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&raw)?))
}

async fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AgentError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    let tmp = path.with_extension("tmp.json");
    tokio::fs::write(&tmp, body).await?;
    if tokio::fs::metadata(path).await.is_ok() {
        let _ = tokio::fs::remove_file(path).await;
    }
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|error| AgentError::Io(error.to_string()))?;
    Ok(())
}

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::error::AgentError;
use crate::host::{ArchiveHost, PublishedAlgorithm, ReviewCandidate, SubmittedCandidate};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedCandidate {
    pub candidate_id: String,
    #[serde(default)]
    pub session_id: String,
    pub review_state: String,
    pub review_notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub raw_extraction: String,
    pub normalized: Value,
    #[serde(default)]
    pub verification: Value,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecision {
    pub reviewer: String,
    pub reason: String,
    #[serde(default)]
    pub expected_candidate_hash: String,
    #[serde(default)]
    pub override_contested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurrentRevision {
    revision_id: String,
    previous_revision_id: String,
    canonical_hash: String,
    committed_at: String,
}

pub struct FileArchive {
    dir: PathBuf,
    queue: Mutex<QueueFile>,
    published: Mutex<ArchiveFile>,
    operation: Mutex<()>,
    audit_pool: Option<SqlitePool>,
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
        let published = read_current_archive(&dir).await?;
        Ok(Self {
            dir,
            queue: Mutex::new(queue),
            published: Mutex::new(published),
            operation: Mutex::new(()),
            audit_pool: None,
        })
    }

    pub fn with_audit_pool(mut self, pool: SqlitePool) -> Self {
        self.audit_pool = Some(pool);
        self
    }

    pub async fn list_queue(&self) -> Vec<QueuedCandidate> {
        self.queue.lock().await.items.clone()
    }

    pub async fn list_accepted(&self) -> Vec<PublishedAlgorithm> {
        self.published.lock().await.items.clone()
    }

    pub async fn reload(&self) -> Result<(), AgentError> {
        let queue = read_json(&self.dir.join("queue.json"))
            .await?
            .unwrap_or(QueueFile {
                next: 1,
                items: Vec::new(),
            });
        let published = read_current_archive(&self.dir).await?;
        *self.queue.lock().await = queue;
        *self.published.lock().await = published;
        Ok(())
    }

    pub async fn catalog(&self) -> Result<Catalog, AgentError> {
        self.reload().await?;
        let published = self.published.lock().await.items.clone();
        let mut accepted = Vec::new();
        for item in published {
            let extraction = read_accepted_detail(&self.dir, &item.id)
                .await?
                .unwrap_or_else(|| serde_json::to_value(&item).unwrap_or(Value::Null));
            accepted.push(catalog_from_published(item, extraction));
        }
        let queue = self
            .queue
            .lock()
            .await
            .items
            .iter()
            .map(catalog_from_queued)
            .collect();
        Ok(Catalog { accepted, queue })
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
        self.accept_with_review(
            id,
            ReviewDecision {
                reviewer: "cli-operator".into(),
                reason: "explicit CLI acceptance".into(),
                expected_candidate_hash: String::new(),
                override_contested: false,
            },
        )
        .await
    }

    pub async fn accept_with_review(
        &self,
        id: &str,
        decision: ReviewDecision,
    ) -> Result<PublishedAlgorithm, AgentError> {
        validate_decision(&decision)?;
        let _operation = self.operation.lock().await;
        let mut queue = read_json(&self.dir.join("queue.json"))
            .await?
            .unwrap_or(QueueFile {
                next: 1,
                items: Vec::new(),
            });
        let mut archive = read_current_archive(&self.dir).await?;
        let index = queue
            .items
            .iter()
            .position(|item| item.candidate_id.eq_ignore_ascii_case(id))
            .ok_or_else(|| AgentError::NotFound(format!("candidate `{id}` not found")))?;
        let mut item = queue.items[index].clone();
        if item.review_state != "PENDING" && !decision.override_contested {
            return Err(AgentError::Denied(format!(
                "candidate `{id}` is {} and needs an explicit contested override",
                item.review_state
            )));
        }
        let candidate_hash = candidate_hash(&item)?;
        if !decision.expected_candidate_hash.is_empty()
            && decision.expected_candidate_hash != candidate_hash
        {
            return Err(AgentError::State(
                "candidate changed after it was opened; inspect the current version before accepting"
                    .into(),
            ));
        }
        item.review_state = "ACCEPTED".into();
        item.review_notes = decision.reason.clone();
        item.updated_at = now()?;
        let published = published_from_candidate(&item);
        if archive
            .items
            .iter()
            .any(|existing| existing.id == published.id || existing.slug == published.slug)
        {
            return Err(AgentError::Denied(
                "an accepted algorithm with this identity already exists".into(),
            ));
        }
        queue.items.remove(index);
        archive.items.insert(0, published.clone());

        let previous = current_revision(&self.dir)
            .await?
            .map(|pointer| pointer.revision_id)
            .unwrap_or_default();
        let revision_id = format!("REV-{}", uuid::Uuid::new_v4().simple());
        let canonical_hash = hash_json(&archive)?;
        write_revision(
            &self.dir,
            &revision_id,
            &previous,
            &canonical_hash,
            &archive,
            Some((&published.id, &item.normalized)),
        )
        .await?;
        let pointer = CurrentRevision {
            revision_id: revision_id.clone(),
            previous_revision_id: previous.clone(),
            canonical_hash: canonical_hash.clone(),
            committed_at: now()?,
        };
        write_json(&self.dir.join("CURRENT.json"), &pointer).await?;
        write_json(&self.dir.join("queue.json"), &queue).await?;
        // Compatibility exports are downstream views; CURRENT.json is authoritative.
        write_json(&self.dir.join("algorithms.json"), &archive).await?;
        write_json(
            &self
                .dir
                .join("accepted")
                .join(format!("{}.json", published.id)),
            &item.normalized,
        )
        .await?;

        *self.queue.lock().await = queue;
        *self.published.lock().await = archive;
        let receipt = json!({
            "receipt_id": format!("PUB-{}", uuid::Uuid::new_v4().simple()),
            "candidate_id": item.candidate_id,
            "session_id": item.session_id,
            "revision_id": revision_id,
            "previous_revision_id": previous,
            "action": "ACCEPT",
            "reviewer": decision.reviewer,
            "reason": decision.reason,
            "candidate_hash": candidate_hash,
            "canonical_hash": canonical_hash,
            "created_at": now()?
        });
        self.persist_archive_receipt(&receipt).await?;
        if let Some(pool) = &self.audit_pool {
            if let Err(error) = crate::graph::upsert_canonical_algorithm(
                pool,
                &published,
                &item.normalized,
                receipt
                    .get("revision_id")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            )
            .await
            {
                self.append_archive_event(json!({
                    "event": "GRAPH_REPLICATION_FAILED",
                    "candidate_id": item.candidate_id,
                    "error": error.to_string(),
                    "created_at": now()?
                }))
                .await?;
            }
        }
        Ok(published)
    }

    pub async fn reject(&self, id: &str, reason: &str) -> Result<QueuedCandidate, AgentError> {
        let _operation = self.operation.lock().await;
        self.reload().await?;
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
        self.append_archive_event(json!({
            "event": "CANDIDATE_REJECTED",
            "candidate_id": cloned.candidate_id,
            "session_id": cloned.session_id,
            "reason": reason,
            "created_at": now()?
        }))
        .await?;
        Ok(cloned)
    }

    pub async fn candidate_hash(&self, id: &str) -> Result<String, AgentError> {
        candidate_hash(&self.get_candidate(id).await?)
    }

    pub async fn edit_candidate(
        &self,
        id: &str,
        expected_hash: &str,
        editor: &str,
        reason: &str,
        normalized: Value,
    ) -> Result<QueuedCandidate, AgentError> {
        if editor.trim().is_empty() || reason.trim().len() < 4 {
            return Err(AgentError::Invalid(
                "candidate edit requires editor and meaningful reason".into(),
            ));
        }
        if !normalized.is_object()
            || normalized
                .get("title")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(AgentError::Schema(
                "edited candidate must remain an object with title".into(),
            ));
        }
        let _operation = self.operation.lock().await;
        let mut queue = read_json(&self.dir.join("queue.json"))
            .await?
            .unwrap_or(QueueFile {
                next: 1,
                items: Vec::new(),
            });
        let item = queue
            .items
            .iter_mut()
            .find(|item| item.candidate_id.eq_ignore_ascii_case(id))
            .ok_or_else(|| AgentError::NotFound(format!("candidate `{id}` not found")))?;
        if matches!(item.review_state.as_str(), "ACCEPTED" | "REJECTED") {
            return Err(AgentError::State(
                "terminal candidates cannot be edited".into(),
            ));
        }
        let current_hash = candidate_hash(item)?;
        if expected_hash != current_hash {
            return Err(AgentError::State(
                "candidate changed after it was opened; reload before editing".into(),
            ));
        }
        item.normalized = normalized;
        item.review_state = "NEEDS_HUMAN".into();
        item.review_notes = format!("Edited by {editor}: {reason}");
        item.updated_at = now()?;
        let edited = item.clone();
        write_json(&self.dir.join("queue.json"), &queue).await?;
        *self.queue.lock().await = queue;
        self.append_archive_event(json!({
            "event": "CANDIDATE_EDITED",
            "candidate_id": edited.candidate_id,
            "session_id": edited.session_id,
            "editor": editor,
            "reason": reason,
            "new_candidate_hash": candidate_hash(&edited)?,
            "canonical_changes": 0,
            "created_at": now()?
        }))
        .await?;
        Ok(edited)
    }

    pub async fn revision_history(&self) -> Result<Vec<Value>, AgentError> {
        let dir = self.dir.join("revisions");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut revisions = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                if let Some(manifest) = read_json::<Value>(&path.join("manifest.json")).await? {
                    revisions.push(manifest);
                }
            }
        }
        revisions.sort_by(|a, b| {
            b.get("committed_at")
                .and_then(Value::as_str)
                .cmp(&a.get("committed_at").and_then(Value::as_str))
        });
        Ok(revisions)
    }

    pub async fn integrity_report(&self) -> Result<Value, AgentError> {
        let pointer = current_revision(&self.dir).await?;
        let Some(pointer) = pointer else {
            return Ok(json!({
                "valid": true,
                "state": "EMPTY",
                "algorithm_count": self.published.lock().await.items.len()
            }));
        };
        let revision_dir = self.dir.join("revisions").join(&pointer.revision_id);
        let archive: ArchiveFile = read_json(&revision_dir.join("algorithms.json"))
            .await?
            .ok_or_else(|| AgentError::State("CURRENT revision payload is missing".into()))?;
        let manifest: Value = read_json(&revision_dir.join("manifest.json"))
            .await?
            .ok_or_else(|| AgentError::State("CURRENT revision manifest is missing".into()))?;
        let actual_hash = hash_json(&archive)?;
        let manifest_hash = manifest
            .get("canonical_hash")
            .and_then(Value::as_str)
            .unwrap_or("");
        Ok(json!({
            "valid": actual_hash == pointer.canonical_hash && actual_hash == manifest_hash,
            "state": "CANONICAL",
            "revision_id": pointer.revision_id,
            "expected_hash": pointer.canonical_hash,
            "manifest_hash": manifest_hash,
            "actual_hash": actual_hash,
            "algorithm_count": archive.items.len()
        }))
    }

    pub async fn rollback(
        &self,
        revision_id: &str,
        reviewer: &str,
        reason: &str,
    ) -> Result<Value, AgentError> {
        if reviewer.trim().is_empty() || reason.trim().is_empty() {
            return Err(AgentError::Invalid(
                "rollback reviewer and reason are required".into(),
            ));
        }
        let _operation = self.operation.lock().await;
        let revision_dir = self.dir.join("revisions").join(revision_id);
        let archive: ArchiveFile = read_json(&revision_dir.join("algorithms.json"))
            .await?
            .ok_or_else(|| AgentError::NotFound(format!("revision `{revision_id}` not found")))?;
        let previous = current_revision(&self.dir)
            .await?
            .map(|pointer| pointer.revision_id)
            .unwrap_or_default();
        let canonical_hash = hash_json(&archive)?;
        let pointer = CurrentRevision {
            revision_id: revision_id.to_owned(),
            previous_revision_id: previous.clone(),
            canonical_hash: canonical_hash.clone(),
            committed_at: now()?,
        };
        write_json(&self.dir.join("CURRENT.json"), &pointer).await?;
        write_json(&self.dir.join("algorithms.json"), &archive).await?;
        *self.published.lock().await = archive;
        let receipt = json!({
            "receipt_id": format!("PUB-{}", uuid::Uuid::new_v4().simple()),
            "candidate_id": "",
            "session_id": "",
            "revision_id": revision_id,
            "previous_revision_id": previous,
            "action": "ROLLBACK",
            "reviewer": reviewer,
            "reason": reason,
            "candidate_hash": "",
            "canonical_hash": canonical_hash,
            "created_at": now()?
        });
        self.persist_archive_receipt(&receipt).await?;
        Ok(receipt)
    }

    async fn persist_archive_receipt(&self, receipt: &Value) -> Result<(), AgentError> {
        write_json(
            &self.dir.join("receipts").join(format!(
                "{}.json",
                receipt
                    .get("receipt_id")
                    .and_then(Value::as_str)
                    .unwrap_or("receipt")
            )),
            receipt,
        )
        .await?;
        self.append_archive_event(receipt.clone()).await?;
        if let Some(pool) = &self.audit_pool {
            let replicated = async {
                crate::persist::save_publication_receipt(pool, receipt).await?;
                crate::persist::insert_event(
                    pool,
                    receipt
                        .get("session_id")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    "CANONICAL_ARCHIVE_CHANGED",
                    receipt.clone(),
                )
                .await?;
                Ok::<(), AgentError>(())
            }
            .await;
            if let Err(error) = replicated {
                self.append_archive_event(json!({
                    "event": "AUDIT_REPLICATION_FAILED",
                    "receipt_id": receipt.get("receipt_id"),
                    "error": error.to_string(),
                    "created_at": now()?
                }))
                .await?;
            }
        }
        Ok(())
    }

    async fn append_archive_event(&self, event: Value) -> Result<(), AgentError> {
        use tokio::io::AsyncWriteExt;
        let path = self.dir.join("events.jsonl");
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        file.write_all(serde_json::to_string(&event)?.as_bytes())
            .await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        Ok(())
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
            session_id: candidate.session_id,
            review_state: candidate.review_state,
            review_notes: String::new(),
            created_at: now.clone(),
            updated_at: now,
            raw_extraction: candidate.raw_extraction,
            normalized: candidate.normalized,
            verification: candidate.verification,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub archive_id: String,
    pub status: String,
    pub title: String,
    pub short_name: String,
    pub slug: String,
    pub domain: String,
    pub summary: String,
    pub core_idea: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub extraction: Value,
    pub candidate_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Catalog {
    pub accepted: Vec<CatalogItem>,
    pub queue: Vec<CatalogItem>,
}

fn catalog_from_published(item: PublishedAlgorithm, extraction: Value) -> CatalogItem {
    CatalogItem {
        id: item.id,
        archive_id: item.archive_id,
        status: "accepted".into(),
        title: item.title,
        short_name: item.short_name,
        slug: item.slug,
        domain: item.domain,
        summary: item.summary,
        core_idea: item.core_idea,
        tags: item.tags,
        created_at: String::new(),
        extraction,
        candidate_hash: String::new(),
    }
}

fn catalog_from_queued(item: &QueuedCandidate) -> CatalogItem {
    let n = &item.normalized;
    CatalogItem {
        id: item.candidate_id.clone(),
        archive_id: item.candidate_id.clone(),
        status: item.review_state.to_ascii_lowercase(),
        title: string_field(n, "title").unwrap_or_else(|| item.candidate_id.clone()),
        short_name: string_field(n, "short_name").unwrap_or_default(),
        slug: slugify(&string_field(n, "title").unwrap_or_else(|| item.candidate_id.clone())),
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
        created_at: item.created_at.clone(),
        extraction: n.clone(),
        candidate_hash: candidate_hash(item).unwrap_or_default(),
    }
}

fn validate_decision(decision: &ReviewDecision) -> Result<(), AgentError> {
    if decision.reviewer.trim().is_empty() {
        return Err(AgentError::Invalid("reviewer is required".into()));
    }
    if decision.reason.trim().len() < 4 {
        return Err(AgentError::Invalid(
            "a meaningful review reason is required".into(),
        ));
    }
    Ok(())
}

fn candidate_hash(item: &QueuedCandidate) -> Result<String, AgentError> {
    hash_json(&json!({
        "candidate_id": item.candidate_id,
        "session_id": item.session_id,
        "raw_extraction": item.raw_extraction,
        "normalized": item.normalized,
        "verification": item.verification
    }))
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, AgentError> {
    let bytes = serde_json::to_vec(value)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

async fn current_revision(dir: &Path) -> Result<Option<CurrentRevision>, AgentError> {
    read_json(&dir.join("CURRENT.json")).await
}

async fn read_current_archive(dir: &Path) -> Result<ArchiveFile, AgentError> {
    if let Some(pointer) = current_revision(dir).await? {
        let path = dir
            .join("revisions")
            .join(pointer.revision_id)
            .join("algorithms.json");
        if let Some(archive) = read_json(&path).await? {
            return Ok(archive);
        }
    }
    Ok(read_json(&dir.join("algorithms.json"))
        .await?
        .unwrap_or_default())
}

async fn read_accepted_detail(dir: &Path, id: &str) -> Result<Option<Value>, AgentError> {
    if let Some(pointer) = current_revision(dir).await? {
        let path = dir
            .join("revisions")
            .join(pointer.revision_id)
            .join("accepted")
            .join(format!("{id}.json"));
        if let Some(detail) = read_json(&path).await? {
            return Ok(Some(detail));
        }
    }
    read_json(&dir.join("accepted").join(format!("{id}.json"))).await
}

async fn write_revision(
    dir: &Path,
    revision_id: &str,
    previous_revision_id: &str,
    canonical_hash: &str,
    archive: &ArchiveFile,
    new_detail: Option<(&str, &Value)>,
) -> Result<(), AgentError> {
    let revisions = dir.join("revisions");
    tokio::fs::create_dir_all(&revisions).await?;
    let staging = revisions.join(format!(".staging-{revision_id}"));
    let accepted = staging.join("accepted");
    tokio::fs::create_dir_all(&accepted).await?;
    for item in &archive.items {
        let detail = if new_detail.is_some_and(|(id, _)| id == item.id) {
            new_detail.map(|(_, value)| value.clone())
        } else {
            read_accepted_detail(dir, &item.id).await?
        }
        .unwrap_or_else(|| serde_json::to_value(item).unwrap_or(Value::Null));
        write_json(&accepted.join(format!("{}.json", item.id)), &detail).await?;
    }
    write_json(&staging.join("algorithms.json"), archive).await?;
    write_json(
        &staging.join("manifest.json"),
        &json!({
            "revision_id": revision_id,
            "previous_revision_id": previous_revision_id,
            "canonical_hash": canonical_hash,
            "algorithm_count": archive.items.len(),
            "committed_at": now()?
        }),
    )
    .await?;
    tokio::fs::rename(&staging, revisions.join(revision_id))
        .await
        .map_err(|error| AgentError::Io(format!("commit archive revision: {error}")))?;
    Ok(())
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

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx_sqlite::SqlitePool;
use tower_http::services::{ServeDir, ServeFile};

use crate::archive::{FileArchive, ReviewDecision};
use crate::error::AgentError;

#[derive(Clone)]
struct UiState {
    archive: Arc<FileArchive>,
    operator_token: Arc<String>,
    reviewer_token: Arc<String>,
    researcher_token: Arc<String>,
    expected_origin: Arc<String>,
    pool: SqlitePool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AccessRole {
    Reader,
    Researcher,
    Reviewer,
    Operator,
}

pub async fn serve(
    root: PathBuf,
    archive: Arc<FileArchive>,
    pool: SqlitePool,
    bind: SocketAddr,
    operator_token: String,
    allow_remote: bool,
) -> Result<(), AgentError> {
    if !bind.ip().is_loopback() && !allow_remote {
        return Err(AgentError::Denied(
            "operator UI must bind to loopback; pass --allow-remote only behind an authenticated proxy"
                .into(),
        ));
    }
    if operator_token.len() < 32 {
        return Err(AgentError::Denied(
            "operator UI token must contain at least 32 characters".into(),
        ));
    }
    let web = root.join("web");
    if !web.join("index.html").exists() {
        return Err(AgentError::NotFound(format!(
            "UI files missing at {}",
            web.display()
        )));
    }
    let state = UiState {
        archive,
        operator_token: Arc::new(operator_token),
        reviewer_token: Arc::new(std::env::var("ALCHETRON_REVIEWER_TOKEN").unwrap_or_default()),
        researcher_token: Arc::new(std::env::var("ALCHETRON_RESEARCHER_TOKEN").unwrap_or_default()),
        expected_origin: Arc::new(format!("http://{bind}")),
        pool,
    };
    let app = Router::new()
        .route("/api/catalog", get(catalog))
        .route("/api/dashboard", get(dashboard))
        .route("/api/graph", get(graph))
        .route("/api/relationships", get(relationships))
        .route("/api/relationships/{id}/review", post(review_relationship))
        .route("/api/archive/rollback", post(rollback))
        .route("/api/queue/{id}/accept", post(accept))
        .route("/api/queue/{id}/reject", post(reject))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/catalog", get(catalog))
        .route("/api/v1/graph", get(graph))
        .route("/api/v1/emergent/logs", get(public_logs))
        .route("/api/v1/emergent/hypotheses", get(public_hypotheses))
        .route("/api/v1/operator/dashboard", get(operator_dashboard))
        .route("/api/v1/operator/integrity", get(integrity))
        .route("/api/v1/operator/emergent", get(operator_emergent))
        .route("/api/v1/operator/models", get(model_invocations))
        .route("/api/v1/operator/queue/{id}/claims", get(candidate_claims))
        .route("/api/v1/operator/queue/{id}/edit", post(edit_candidate))
        .route("/api/v1/research/hypotheses", post(propose_hypothesis))
        .route("/api/v1/research/logs", post(create_research_log))
        .route(
            "/api/v1/research/hypotheses/{id}/challenge",
            post(challenge_hypothesis),
        )
        .route(
            "/api/v1/operator/hypotheses/{id}/review",
            post(review_hypothesis),
        )
        .route(
            "/api/v1/operator/logs/{id}/review",
            post(review_research_log),
        )
        .route("/api/v1/operator/emergent/scan", post(scan_emergent))
        .route("/api/v1/research/disputes", post(open_dispute))
        .route(
            "/api/v1/operator/disputes/{id}/resolve",
            post(resolve_dispute),
        )
        .route(
            "/api/v1/operator/relationships/{id}/review",
            post(review_relationship),
        )
        .route("/api/v1/operator/archive/rollback", post(rollback))
        .route("/api/v1/operator/queue/{id}/accept", post(accept))
        .route("/api/v1/operator/queue/{id}/reject", post(reject))
        .route_service("/", ServeFile::new(web.join("index.html")))
        .nest_service("/css", ServeDir::new(web.join("css")))
        .nest_service("/js", ServeDir::new(web.join("js")))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| AgentError::Io(error.to_string()))?;
    axum::serve(listener, app)
        .await
        .map_err(|error| AgentError::Internal(error.to_string()))
}

async fn catalog(State(state): State<UiState>) -> Result<Json<Value>, UiError> {
    let catalog = state.archive.catalog().await?;
    Ok(Json(serde_json::to_value(catalog)?))
}

async fn dashboard(State(state): State<UiState>) -> Result<Json<Value>, UiError> {
    let sessions = crate::persist::list_sessions(&state.pool).await?;
    let events = crate::persist::list_events(&state.pool, "").await?;
    let receipts = crate::persist::list_publication_receipts(&state.pool).await?;
    let revisions = state.archive.revision_history().await?;
    let proposals = crate::graph::list_proposals(&state.pool).await?;
    let hypotheses = crate::emergent::list_hypotheses(&state.pool, false).await?;
    let research_logs = crate::emergent::list_research_logs(&state.pool, false).await?;
    let evaluations = crate::evaluation::history(&state.pool).await?;
    Ok(Json(json!({
        "sessions": sessions.into_iter().take(30).collect::<Vec<_>>(),
        "events": events.into_iter().rev().take(80).collect::<Vec<_>>(),
        "publication_receipts": receipts,
        "revisions": revisions,
        "relationship_proposals": proposals,
        "hypotheses": hypotheses,
        "research_logs": research_logs,
        "evaluations": evaluations
    })))
}

async fn capabilities() -> Json<Value> {
    Json(json!({
        "name": "Alchetron",
        "version": env!("CARGO_PKG_VERSION"),
        "api_version": "v1",
        "mcp": crate::mcp::descriptor(),
        "models": crate::model::capabilities(),
        "roles": {
            "reader": ["catalog:read", "graph:read", "public_logs:read"],
            "researcher": ["hypothesis:propose", "hypothesis:challenge", "log:create", "dispute:open"],
            "reviewer": ["candidate:review", "relationship:review", "hypothesis:review", "log:review"],
            "operator": ["archive:rollback", "integrity:read", "dashboard:read", "all reviewer actions"]
        },
        "governance": {
            "agent_can_publish": false,
            "model_can_publish": false,
            "imports_are_private": true
        }
    }))
}

async fn public_logs(State(state): State<UiState>) -> Result<Json<Value>, UiError> {
    Ok(Json(json!({
        "logs": crate::emergent::list_research_logs(&state.pool, false).await?
    })))
}

async fn public_hypotheses(State(state): State<UiState>) -> Result<Json<Value>, UiError> {
    Ok(Json(json!({
        "hypotheses": crate::emergent::list_hypotheses(&state.pool, false).await?
    })))
}

async fn operator_dashboard(
    State(state): State<UiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Operator)?;
    dashboard(State(state)).await
}

async fn integrity(
    State(state): State<UiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Operator)?;
    Ok(Json(json!({
        "archive": state.archive.integrity_report().await?,
        "graph": crate::graph::integrity_report(&state.pool).await?
    })))
}

async fn operator_emergent(
    State(state): State<UiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Reviewer)?;
    Ok(Json(json!({
        "hypotheses": crate::emergent::list_hypotheses(&state.pool, true).await?,
        "logs": crate::emergent::list_research_logs(&state.pool, true).await?,
        "disputes": crate::graph::list_disputes(&state.pool).await?
    })))
}

async fn model_invocations(
    State(state): State<UiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Operator)?;
    Ok(Json(json!({
        "invocations": crate::model::list_invocations(&state.pool).await?
    })))
}

async fn candidate_claims(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Reviewer)?;
    Ok(Json(
        json!({ "claims": crate::evidence::list_claims(&state.pool, &id).await? }),
    ))
}

#[derive(Deserialize)]
struct EditCandidateBody {
    confirmation: String,
    editor: String,
    reason: String,
    candidate_hash: String,
    normalized: Value,
}

async fn edit_candidate(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<EditCandidateBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Reviewer)?;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must match candidate id".into(),
        )));
    }
    Ok(Json(serde_json::to_value(
        state
            .archive
            .edit_candidate(
                &id,
                &body.candidate_hash,
                &body.editor,
                &body.reason,
                body.normalized,
            )
            .await?,
    )?))
}

async fn propose_hypothesis(
    State(state): State<UiState>,
    headers: HeaderMap,
    Json(input): Json<crate::emergent::HypothesisInput>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Researcher)?;
    Ok(Json(
        crate::emergent::propose_hypothesis(&state.pool, input).await?,
    ))
}

async fn create_research_log(
    State(state): State<UiState>,
    headers: HeaderMap,
    Json(input): Json<crate::emergent::ResearchLogInput>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Researcher)?;
    Ok(Json(
        crate::emergent::create_research_log(&state.pool, input).await?,
    ))
}

#[derive(Deserialize)]
struct ChallengeBody {
    challenger: String,
    verdict: String,
    rationale: String,
    #[serde(default)]
    evidence: Value,
}

async fn challenge_hypothesis(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ChallengeBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Researcher)?;
    Ok(Json(
        crate::emergent::challenge_hypothesis(
            &state.pool,
            &id,
            &body.challenger,
            &body.verdict,
            &body.rationale,
            body.evidence,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
struct ReviewBody {
    confirmation: String,
    reviewer: String,
    reason: String,
    accept: bool,
}

async fn review_hypothesis(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ReviewBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Reviewer)?;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must match hypothesis id".into(),
        )));
    }
    Ok(Json(
        crate::emergent::review_hypothesis(
            &state.pool,
            &id,
            body.accept,
            &body.reviewer,
            &body.reason,
        )
        .await?,
    ))
}

async fn review_research_log(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ReviewBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Reviewer)?;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must match log id".into(),
        )));
    }
    Ok(Json(
        crate::emergent::review_research_log(
            &state.pool,
            &id,
            body.accept,
            &body.reviewer,
            &body.reason,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
struct ScanBody {
    proposed_by: String,
}

async fn scan_emergent(
    State(state): State<UiState>,
    headers: HeaderMap,
    Json(body): Json<ScanBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Researcher)?;
    Ok(Json(json!({
        "hypotheses": crate::emergent::scan_for_missing_links(&state.pool, &body.proposed_by).await?,
        "canonical_publications": 0
    })))
}

#[derive(Deserialize)]
struct DisputeBody {
    target_type: String,
    target_id: String,
    claim: String,
    #[serde(default)]
    evidence: Value,
    opened_by: String,
}

async fn open_dispute(
    State(state): State<UiState>,
    headers: HeaderMap,
    Json(body): Json<DisputeBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Researcher)?;
    Ok(Json(serde_json::to_value(
        crate::graph::open_dispute(
            &state.pool,
            &body.target_type,
            &body.target_id,
            &body.claim,
            body.evidence,
            &body.opened_by,
        )
        .await?,
    )?))
}

#[derive(Deserialize)]
struct ResolveDisputeBody {
    confirmation: String,
    reviewer: String,
    resolution: String,
}

async fn resolve_dispute(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ResolveDisputeBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Reviewer)?;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must match dispute id".into(),
        )));
    }
    Ok(Json(serde_json::to_value(
        crate::graph::resolve_dispute(&state.pool, &id, &body.reviewer, &body.resolution).await?,
    )?))
}

async fn graph(State(state): State<UiState>) -> Result<Json<Value>, UiError> {
    Ok(Json(
        crate::graph::graph_snapshot(&state.pool, false).await?,
    ))
}

async fn relationships(State(state): State<UiState>) -> Result<Json<Value>, UiError> {
    Ok(Json(json!({
        "proposals": crate::graph::list_proposals(&state.pool).await?
    })))
}

#[derive(Deserialize)]
struct RelationshipReviewBody {
    confirmation: String,
    reviewer: String,
    reason: String,
    accept: bool,
}

async fn review_relationship(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RelationshipReviewBody>,
) -> Result<Json<Value>, UiError> {
    authorize(&state, &headers)?;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must exactly match the proposal id".into(),
        )));
    }
    Ok(Json(serde_json::to_value(
        crate::graph::review_relationship(
            &state.pool,
            &id,
            body.accept,
            &body.reviewer,
            &body.reason,
        )
        .await?,
    )?))
}

#[derive(Deserialize)]
struct RollbackBody {
    revision_id: String,
    confirmation: String,
    reviewer: String,
    reason: String,
}

async fn rollback(
    State(state): State<UiState>,
    headers: HeaderMap,
    Json(body): Json<RollbackBody>,
) -> Result<Json<Value>, UiError> {
    authorize_role(&state, &headers, AccessRole::Operator)?;
    if body.confirmation != body.revision_id {
        return Err(UiError(AgentError::Denied(
            "confirmation must exactly match the revision id".into(),
        )));
    }
    Ok(Json(
        state
            .archive
            .rollback(&body.revision_id, &body.reviewer, &body.reason)
            .await?,
    ))
}

#[derive(Deserialize)]
struct AcceptBody {
    confirmation: String,
    reviewer: String,
    reason: String,
    candidate_hash: String,
    #[serde(default)]
    override_contested: bool,
}

async fn accept(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<AcceptBody>,
) -> Result<Json<Value>, UiError> {
    authorize(&state, &headers)?;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must exactly match the candidate id".into(),
        )));
    }
    let item = state
        .archive
        .accept_with_review(
            &id,
            ReviewDecision {
                reviewer: body.reviewer,
                reason: body.reason,
                expected_candidate_hash: body.candidate_hash,
                override_contested: body.override_contested,
            },
        )
        .await?;
    Ok(Json(serde_json::to_value(item)?))
}

#[derive(Deserialize)]
struct RejectBody {
    #[serde(default)]
    reason: String,
    #[serde(default)]
    confirmation: String,
}

async fn reject(
    State(state): State<UiState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Json<RejectBody>,
) -> Result<Json<Value>, UiError> {
    authorize(&state, &headers)?;
    let Json(body) = body;
    if body.confirmation != id {
        return Err(UiError(AgentError::Denied(
            "confirmation must exactly match the candidate id".into(),
        )));
    }
    let reason = body.reason;
    let reason = if reason.trim().is_empty() {
        "rejected by operator"
    } else {
        reason.trim()
    };
    let item = state.archive.reject(&id, reason).await?;
    Ok(Json(serde_json::to_value(item)?))
}

fn authorize(state: &UiState, headers: &HeaderMap) -> Result<(), UiError> {
    authorize_role(state, headers, AccessRole::Reviewer)
}

fn authorize_role(
    state: &UiState,
    headers: &HeaderMap,
    required: AccessRole,
) -> Result<(), UiError> {
    let supplied = headers
        .get("x-alchetron-operator-token")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
        })
        .unwrap_or("");
    let role = if supplied == state.operator_token.as_str() {
        AccessRole::Operator
    } else if !state.reviewer_token.is_empty() && supplied == state.reviewer_token.as_str() {
        AccessRole::Reviewer
    } else if !state.researcher_token.is_empty() && supplied == state.researcher_token.as_str() {
        AccessRole::Researcher
    } else {
        AccessRole::Reader
    };
    if role < required {
        return Err(UiError(AgentError::Denied(format!(
            "{} role required",
            format!("{required:?}").to_ascii_lowercase()
        ))));
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "cross-site")
    {
        return Err(UiError(AgentError::Denied(
            "cross-site archive mutation denied".into(),
        )));
    }
    if let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) {
        let request_host = headers
            .get("host")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let same_request_origin = origin_matches_request_host(origin, request_host);
        if origin != state.expected_origin.as_str() && !same_request_origin {
            return Err(UiError(AgentError::Denied(format!(
                "origin `{origin}` is not authorized"
            ))));
        }
    }
    Ok(())
}

fn origin_matches_request_host(origin: &str, request_host: &str) -> bool {
    if request_host.is_empty() {
        return false;
    }
    origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .is_some_and(|origin_host| origin_host == request_host)
}

struct UiError(AgentError);

impl From<AgentError> for UiError {
    fn from(value: AgentError) -> Self {
        Self(value)
    }
}

impl From<serde_json::Error> for UiError {
    fn from(value: serde_json::Error) -> Self {
        Self(AgentError::Schema(value.to_string()))
    }
}

impl IntoResponse for UiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.0 {
            AgentError::NotFound(_) => StatusCode::NOT_FOUND,
            AgentError::Denied(_) => StatusCode::FORBIDDEN,
            AgentError::Invalid(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}

pub fn open_browser(url: &str) {
    let _ = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
}

#[cfg(test)]
mod tests {
    use super::origin_matches_request_host;

    #[test]
    fn same_request_host_allows_container_and_proxy_origins() {
        assert!(origin_matches_request_host(
            "http://127.0.0.1:8791",
            "127.0.0.1:8791"
        ));
        assert!(origin_matches_request_host(
            "https://alchetron.example",
            "alchetron.example"
        ));
        assert!(!origin_matches_request_host(
            "https://evil.example",
            "127.0.0.1:8791"
        ));
        assert!(!origin_matches_request_host(
            "https://alchetron.example.evil.test",
            "alchetron.example"
        ));
    }
}

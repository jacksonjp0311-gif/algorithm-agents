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
use sqlx::SqlitePool;
use tower_http::services::{ServeDir, ServeFile};

use crate::archive::{FileArchive, ReviewDecision};
use crate::error::AgentError;

#[derive(Clone)]
struct UiState {
    archive: Arc<FileArchive>,
    operator_token: Arc<String>,
    expected_origin: Arc<String>,
    pool: SqlitePool,
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
    Ok(Json(json!({
        "sessions": sessions.into_iter().take(30).collect::<Vec<_>>(),
        "events": events.into_iter().rev().take(80).collect::<Vec<_>>(),
        "publication_receipts": receipts,
        "revisions": revisions,
        "relationship_proposals": proposals
    })))
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
    authorize(&state, &headers)?;
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
    let supplied = headers
        .get("x-alchetron-operator-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if supplied != state.operator_token.as_str() {
        return Err(UiError(AgentError::Denied(
            "valid operator token required".into(),
        )));
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
        if origin != state.expected_origin.as_str() {
            return Err(UiError(AgentError::Denied(format!(
                "origin `{origin}` is not authorized"
            ))));
        }
    }
    Ok(())
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

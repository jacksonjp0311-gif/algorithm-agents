use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::services::{ServeDir, ServeFile};

use crate::archive::FileArchive;
use crate::error::AgentError;

#[derive(Clone)]
struct UiState {
    archive: Arc<FileArchive>,
}

pub async fn serve(root: PathBuf, archive: Arc<FileArchive>, bind: SocketAddr) -> Result<(), AgentError> {
    let web = root.join("web");
    if !web.join("index.html").exists() {
        return Err(AgentError::NotFound(format!(
            "UI files missing at {}",
            web.display()
        )));
    }
    let state = UiState { archive };
    let app = Router::new()
        .route("/api/catalog", get(catalog))
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

async fn accept(State(state): State<UiState>, Path(id): Path<String>) -> Result<Json<Value>, UiError> {
    let item = state.archive.accept(&id).await?;
    Ok(Json(serde_json::to_value(item)?))
}

#[derive(Deserialize)]
struct RejectBody {
    #[serde(default)]
    reason: String,
}

async fn reject(
    State(state): State<UiState>,
    Path(id): Path<String>,
    body: Option<Json<RejectBody>>,
) -> Result<Json<Value>, UiError> {
    let reason = body
        .map(|Json(body)| body.reason)
        .unwrap_or_default();
    let reason = if reason.trim().is_empty() {
        "rejected by operator"
    } else {
        reason.trim()
    };
    let item = state.archive.reject(&id, reason).await?;
    Ok(Json(serde_json::to_value(item)?))
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
            AgentError::Denied(_) | AgentError::Invalid(_) => StatusCode::BAD_REQUEST,
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

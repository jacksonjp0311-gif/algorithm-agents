use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Denied(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Budget(String),
    #[error("{0}")]
    State(String),
    #[error("{0}")]
    Schema(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Internal(String),
}

impl AgentError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid",
            Self::Denied(_) => "denied",
            Self::NotFound(_) => "not_found",
            Self::Budget(_) => "budget",
            Self::State(_) => "state",
            Self::Schema(_) => "schema",
            Self::Io(_) => "io",
            Self::Internal(_) => "internal",
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "ok": false,
            "error": self.to_string(),
            "kind": self.kind(),
        })
    }
}

impl From<sqlx::Error> for AgentError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<std::io::Error> for AgentError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<serde_json::Error> for AgentError {
    fn from(error: serde_json::Error) -> Self {
        Self::Schema(error.to_string())
    }
}

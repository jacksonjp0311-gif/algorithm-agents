use serde::{Deserialize, Serialize};

use crate::error::AgentError;
use crate::schema::load_json_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub publication_allowed: bool,
    #[serde(default)]
    pub live_fetch_enabled: bool,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub denied_hosts: Vec<String>,
    #[serde(default = "default_fetch_bytes")]
    pub max_fetch_bytes: usize,
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_seconds: u64,
    #[serde(default = "default_redirects")]
    pub max_redirects: usize,
    #[serde(default = "default_require_https")]
    pub require_https: bool,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default = "default_languages")]
    pub code_languages: Vec<String>,
    #[serde(default = "default_code_timeout")]
    pub code_timeout_seconds: u64,
    #[serde(default = "default_code_output")]
    pub code_output_limit: usize,
    #[serde(default)]
    pub globally_denied_tools: Vec<String>,
}

fn default_fetch_bytes() -> usize {
    250_000
}
fn default_fetch_timeout() -> u64 {
    8
}
fn default_user_agent() -> String {
    "Algorithm-Agents/1.0 (local research extractor; +https://github.com)".into()
}
fn default_redirects() -> usize {
    3
}
fn default_require_https() -> bool {
    true
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}
fn default_languages() -> Vec<String> {
    vec!["python".into()]
}
fn default_code_timeout() -> u64 {
    3
}
fn default_code_output() -> usize {
    4096
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            publication_allowed: false,
            live_fetch_enabled: false,
            allowed_hosts: Vec::new(),
            denied_hosts: Vec::new(),
            max_fetch_bytes: default_fetch_bytes(),
            fetch_timeout_seconds: default_fetch_timeout(),
            max_redirects: default_redirects(),
            require_https: default_require_https(),
            user_agent: default_user_agent(),
            code_languages: default_languages(),
            code_timeout_seconds: default_code_timeout(),
            code_output_limit: default_code_output(),
            globally_denied_tools: vec![
                "publish_algorithm".into(),
                "modify_canonical_archive".into(),
                "unrestricted_shell".into(),
                "write_file".into(),
                "run_shell".into(),
                "approve_extraction".into(),
            ],
        }
    }
}

impl Permissions {
    pub fn load(root: &std::path::Path) -> Result<Self, AgentError> {
        let path = root.join("registry/permissions.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let value = load_json_file(&path)?;
        serde_json::from_value(value).map_err(|error| AgentError::Schema(error.to_string()))
    }

    pub fn tool_globally_denied(&self, tool: &str) -> bool {
        self.globally_denied_tools.iter().any(|item| item == tool)
    }

    pub fn apply_runtime_overrides(&mut self, live: bool) {
        if live || env_flag("ALGO_LIVE_FETCH") || env_flag("ALGORITHM_AGENTS_LIVE_FETCH") {
            self.live_fetch_enabled = true;
        }
        if let Ok(ua) = std::env::var("ALGO_USER_AGENT") {
            if !ua.trim().is_empty() {
                self.user_agent = ua;
            }
        }
    }
}

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AgentError;
use crate::schema::{load_json_file, validate_against_schema};

pub const AGENT_IDS: [&str; 26] = [
    "source_scout",
    "web_collector",
    "paper_analyst",
    "repository_scout",
    "citation_walker",
    "algorithm_detector",
    "algorithm_extractor",
    "math_analyst",
    "code_analyst",
    "complexity_analyst",
    "assumption_analyst",
    "failure_mode_analyst",
    "provenance_checker",
    "cross_source_verifier",
    "math_checker",
    "code_verifier",
    "hallucination_challenger",
    "deduplicator",
    "relationship_mapper",
    "structural_matcher",
    "domain_classifier",
    "use_case_mapper",
    "normalizer",
    "summarizer",
    "code_translator",
    "experiment_designer",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub purpose: String,
    pub family: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    #[serde(default)]
    pub can_message: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_timeout() -> u64 {
    120
}
fn default_retries() -> u32 {
    2
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub session_required: bool,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone)]
pub struct LoadedAgent {
    pub manifest: AgentManifest,
    pub dir: PathBuf,
    pub input_schema: Value,
    pub output_schema: Value,
    pub smoke_input: Value,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub root: PathBuf,
    pub agents: BTreeMap<String, LoadedAgent>,
    pub tools: BTreeMap<String, ToolManifest>,
    pub invalid: Vec<(String, String)>,
}

impl Registry {
    pub fn load(root: &Path) -> Result<Self, AgentError> {
        let agent_schema = load_optional_schema(root, "schemas/agent.schema.json")?;
        let tool_schema = load_optional_schema(root, "schemas/tool.schema.json")?;

        let index_path = root.join("registry/agents.json");
        let index = load_json_file(&index_path)?;
        let listed = index
            .get("agents")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut agents = BTreeMap::new();
        let mut invalid = Vec::new();
        let mut seen = BTreeMap::new();

        for entry in listed {
            let Some(id) = entry.as_str() else {
                invalid.push(("<index>".into(), "non-string agent id".into()));
                continue;
            };
            match load_agent(root, id, agent_schema.as_ref()) {
                Ok(loaded) => {
                    if seen.insert(loaded.manifest.id.clone(), ()).is_some() {
                        invalid.push((id.into(), "duplicate agent id".into()));
                        continue;
                    }
                    if !semver_ok(&loaded.manifest.version) {
                        invalid.push((id.into(), "version is not major.minor.patch".into()));
                        continue;
                    }
                    if loaded.manifest.purpose.trim().is_empty() {
                        invalid.push((id.into(), "purpose missing".into()));
                        continue;
                    }
                    agents.insert(loaded.manifest.id.clone(), loaded);
                }
                Err(error) => invalid.push((id.into(), error.to_string())),
            }
        }

        let tools_path = root.join("registry/tools.json");
        let tools_doc = load_json_file(&tools_path)?;
        let mut tools = BTreeMap::new();
        if let Some(list) = tools_doc.get("tools").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(schema) = &tool_schema {
                    if let Err(error) = validate_against_schema(schema, item) {
                        return Err(error);
                    }
                }
                let tool: ToolManifest = serde_json::from_value(item.clone())?;
                tools.insert(tool.id.clone(), tool);
            }
        }

        for agent in agents.values() {
            for tool in &agent.manifest.allowed_tools {
                if !tools.contains_key(tool) {
                    invalid.push((
                        agent.manifest.id.clone(),
                        format!("allowed tool `{tool}` is not registered"),
                    ));
                }
            }
            let overlap: Vec<_> = agent
                .manifest
                .allowed_tools
                .iter()
                .filter(|tool| agent.manifest.denied_tools.iter().any(|denied| denied == *tool))
                .cloned()
                .collect();
            if !overlap.is_empty() {
                invalid.push((
                    agent.manifest.id.clone(),
                    format!("tools both allowed and denied: {}", overlap.join(", ")),
                ));
            }
            for denied in ["publish_algorithm", "modify_canonical_archive", "unrestricted_shell"] {
                if !agent.manifest.denied_tools.iter().any(|item| item == denied) {
                    invalid.push((
                        agent.manifest.id.clone(),
                        format!("must deny `{denied}`"),
                    ));
                }
            }
        }

        // Drop agents that were marked invalid after the tool checks.
        let bad: Vec<String> = invalid.iter().map(|(id, _)| id.clone()).collect();
        for id in bad {
            agents.remove(&id);
        }

        Ok(Self {
            root: root.to_path_buf(),
            agents,
            tools,
            invalid,
        })
    }

    pub fn get_agent(&self, id: &str) -> Result<&LoadedAgent, AgentError> {
        self.agents
            .get(id)
            .ok_or_else(|| AgentError::NotFound(format!("agent `{id}` is not registered")))
    }

    pub fn get_tool(&self, id: &str) -> Result<&ToolManifest, AgentError> {
        self.tools
            .get(id)
            .ok_or_else(|| AgentError::NotFound(format!("tool `{id}` is not registered")))
    }
}

fn load_optional_schema(root: &Path, rel: &str) -> Result<Option<Value>, AgentError> {
    let path = root.join(rel);
    if path.exists() {
        Ok(Some(load_json_file(&path)?))
    } else {
        Ok(None)
    }
}

fn load_agent(root: &Path, id: &str, agent_schema: Option<&Value>) -> Result<LoadedAgent, AgentError> {
    let dir = root.join("agents").join(id);
    let manifest_path = dir.join("agent.json");
    if !manifest_path.exists() {
        return Err(AgentError::NotFound(format!(
            "manifest missing at {}",
            manifest_path.display()
        )));
    }
    let manifest_value = load_json_file(&manifest_path)?;
    if let Some(schema) = agent_schema {
        validate_against_schema(schema, &manifest_value)?;
    }
    let manifest: AgentManifest = serde_json::from_value(manifest_value)?;
    if manifest.id != id {
        return Err(AgentError::Invalid(format!(
            "manifest id `{}` does not match directory `{id}`",
            manifest.id
        )));
    }
    let input_schema = load_json_file(&dir.join("input.schema.json"))?;
    let output_schema = load_json_file(&dir.join("output.schema.json"))?;
    let smoke_input = load_json_file(&dir.join("smoke_input.json"))?;
    Ok(LoadedAgent {
        manifest,
        dir,
        input_schema,
        output_schema,
        smoke_input,
    })
}

fn semver_ok(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| part.parse::<u32>().is_ok())
}

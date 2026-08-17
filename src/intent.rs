use serde_json::{Value, json};

use crate::error::AgentError;
use crate::hunt::{self, HuntSpec};
use crate::pipeline;
use crate::runtime::AgentRuntime;
use crate::smoke;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentKind {
    Scrape,
    Hunt,
    Harvest,
    Find,
    Smoke,
    ArchiveList,
}

#[derive(Debug, Clone)]
pub struct Intent {
    pub kind: IntentKind,
    pub live: bool,
    pub query: String,
    pub locator: Option<String>,
}

pub fn parse_intent(raw: &str) -> Intent {
    let text = raw.trim();
    if let Some(url) = first_url(text) {
        return Intent {
            kind: IntentKind::Scrape,
            live: true,
            query: text.to_owned(),
            locator: Some(url),
        };
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("smoke") || lower.contains("compile agent") || lower == "check agents" {
        return Intent {
            kind: IntentKind::Smoke,
            live: false,
            query: text.to_owned(),
            locator: None,
        };
    }
    if lower.contains("archive list")
        || lower.contains("show queue")
        || lower.contains("list candidates")
    {
        return Intent {
            kind: IntentKind::ArchiveList,
            live: false,
            query: text.to_owned(),
            locator: None,
        };
    }
    let wants_web = lower.contains("arxiv")
        || lower.contains("wikipedia")
        || lower.contains("wiki")
        || lower.contains("hunt")
        || lower.contains("search the web")
        || lower.contains("on the web")
        || lower.contains("online")
        || lower.contains("internet")
        || lower.contains("papers")
        || lower.contains("collective");
    if wants_web {
        return Intent {
            kind: IntentKind::Hunt,
            live: true,
            query: strip_command_words(text),
            locator: None,
        };
    }
    if lower.starts_with("find ") || lower.contains("in the archive") || lower.contains("look up") {
        return Intent {
            kind: IntentKind::Find,
            live: false,
            query: strip_command_words(text),
            locator: None,
        };
    }
    Intent {
        kind: IntentKind::Harvest,
        live: false,
        query: strip_command_words(text),
        locator: None,
    }
}

pub async fn execute_intent(
    runtime: &mut AgentRuntime,
    raw: &str,
    limit: usize,
) -> Result<Value, AgentError> {
    let intent = parse_intent(raw);
    if intent.live {
        runtime.permissions.live_fetch_enabled = true;
        runtime.permissions.fetch_timeout_seconds =
            runtime.permissions.fetch_timeout_seconds.max(20);
        runtime.permissions.max_fetch_bytes = runtime.permissions.max_fetch_bytes.max(1_200_000);
    }
    let action = match intent.kind {
        IntentKind::Scrape => "scrape",
        IntentKind::Hunt => "hunt",
        IntentKind::Harvest => "harvest",
        IntentKind::Find => "find",
        IntentKind::Smoke => "smoke",
        IntentKind::ArchiveList => "archive_list",
    };
    let payload = match intent.kind {
        IntentKind::Scrape => {
            let locator = intent.locator.clone().unwrap_or_default();
            pipeline::scrape(runtime, &locator, &intent.query).await?
        }
        IntentKind::Hunt => {
            hunt::hunt(
                runtime,
                HuntSpec {
                    query: intent.query.clone(),
                    arxiv: true,
                    web: true,
                    limit,
                },
            )
            .await?
        }
        IntentKind::Harvest => pipeline::harvest(runtime, &intent.query, limit).await?,
        IntentKind::Find => pipeline::find_sources(runtime, &intent.query).await?,
        IntentKind::Smoke => smoke::smoke_all(runtime, None).await?,
        IntentKind::ArchiveList => {
            let accepted = runtime.host.list_published_algorithms().await?;
            json!({ "accepted": accepted })
        }
    };
    Ok(json!({
        "intent": raw,
        "action": action,
        "live": intent.live,
        "query": intent.query,
        "locator": intent.locator,
        "result": payload
    }))
}

pub fn command_card() -> Value {
    json!({
        "role": "AI operator",
        "rule": "Run these as shell commands. Do not invent algorithms. Do not accept unless the human says so.",
        "preferred": "algo do \"<human intent>\"",
        "commands": [
            {"cmd": "algo do \"scrape this: https://arxiv.org/abs/1234.5678\"", "does": "Direct scrape of a named URL"},
            {"cmd": "algo do \"find monte carlo algorithms on arxiv\"", "does": "Live hunt arXiv + Wikipedia from intent"},
            {"cmd": "algo do \"extract shortest path from fixtures\"", "does": "Local harvest, no network"},
            {"cmd": "algo --live scrape <url>", "does": "Direct scrape when you already have a locator"},
            {"cmd": "algo --live hunt \"physics\" --limit 4", "does": "Topic hunt"},
            {"cmd": "algo harvest \"shortest path\" --limit 1", "does": "Offline fixture/source harvest"},
            {"cmd": "algo find \"kalman\"", "does": "Search fixtures and archive"},
            {"cmd": "algo archive list", "does": "Show PENDING and accepted"},
            {"cmd": "algo smoke", "does": "Compile every agent"},
            {"cmd": "algo smoke --process", "does": "Compile agents and the harvest path"}
        ]
    })
}

fn first_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|word| word.starts_with("http://") || word.starts_with("https://"))
        .map(|word| word.trim_end_matches([',', '.', ';', ')']).to_owned())
}

fn strip_command_words(text: &str) -> String {
    let drop = [
        "please",
        "algo",
        "do",
        "find",
        "hunt",
        "search",
        "scrape",
        "extract",
        "get",
        "pull",
        "from",
        "the",
        "web",
        "online",
        "arxiv",
        "wikipedia",
        "wiki",
        "papers",
        "algorithms",
        "algorithm",
        "for",
        "me",
        "and",
    ];
    let kept: Vec<&str> = text
        .split_whitespace()
        .filter(|word| {
            let lower = word
                .trim_matches(|ch: char| !ch.is_alphanumeric())
                .to_ascii_lowercase();
            !drop.contains(&lower.as_str()) && lower.len() > 1
        })
        .collect();
    if kept.is_empty() {
        text.to_owned()
    } else {
        kept.join(" ")
    }
}

use serde_json::{Value, json};

use crate::error::AgentError;
use crate::fetch;
use crate::permissions::Permissions;
use crate::pipeline;
use crate::runtime::AgentRuntime;

#[derive(Debug, Clone)]
pub struct HuntSpec {
    pub query: String,
    pub arxiv: bool,
    pub web: bool,
    pub limit: usize,
}

#[derive(Debug, Clone)]
struct Hit {
    title: String,
    url: String,
    origin: String,
    blurb: String,
}

pub async fn hunt(runtime: &AgentRuntime, spec: HuntSpec) -> Result<Value, AgentError> {
    if !runtime.permissions.live_fetch_enabled {
        return Err(AgentError::Denied(
            "hunt needs live fetch; pass --live or set ALGO_LIVE_FETCH=1".into(),
        ));
    }
    let limit = spec.limit.clamp(1, 12);
    let mut hits = Vec::new();
    let web_budget = if spec.arxiv && spec.web {
        (limit / 3).clamp(2, 4)
    } else if spec.web {
        limit
    } else {
        0
    };
    let arxiv_budget = if spec.web { limit.saturating_sub(web_budget).max(1) } else { limit };
    if spec.arxiv {
        hits.extend(search_arxiv(&runtime.permissions, &spec.query, arxiv_budget).await?);
        hits.truncate(arxiv_budget);
    }
    if spec.web {
        hits.extend(search_wikipedia(&runtime.permissions, &spec.query, web_budget.max(1)).await?);
    }
    hits = dedupe(hits);
    hits.truncate(limit);

    let mut harvested = Vec::new();
    for (index, hit) in hits.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
        let objective = format!(
            "Extract reconstructable algorithms from {} ({})",
            hit.title, hit.origin
        );
        match pipeline::scrape(runtime, &hit.url, &objective).await {
            Ok(result) => harvested.push(json!({
                "origin": hit.origin,
                "title": hit.title,
                "url": hit.url,
                "blurb": hit.blurb,
                "result": result
            })),
            Err(error) => harvested.push(json!({
                "origin": hit.origin,
                "title": hit.title,
                "url": hit.url,
                "error": error.to_string()
            })),
        }
    }
    Ok(json!({
        "query": spec.query,
        "arxiv": spec.arxiv,
        "web": spec.web,
        "hits": hits.iter().map(|hit| json!({
            "title": hit.title,
            "url": hit.url,
            "origin": hit.origin
        })).collect::<Vec<_>>(),
        "harvested": harvested
    }))
}

async fn search_arxiv(permissions: &Permissions, query: &str, limit: usize) -> Result<Vec<Hit>, AgentError> {
    let terms = arxiv_terms(query);
    let url = format!(
        "https://export.arxiv.org/api/query?search_query={}&start=0&max_results={}&sortBy=relevance&sortOrder=descending",
        urlencoding(&terms),
        limit.max(3)
    );
    let (_resolved, xml) = fetch::fetch_url(permissions, &url).await?;
    Ok(parse_atom(&xml))
}

fn arxiv_terms(query: &str) -> String {
    let cleaned = query
        .split_whitespace()
        .filter(|word| word.len() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    let focus = if cleaned.is_empty() {
        "algorithm".to_owned()
    } else {
        cleaned
    };
    let quoted = if focus.contains(' ') {
        format!("all:\"{focus}\"")
    } else {
        format!("all:{focus}")
    };
    format!(
        "(all:algorithm OR all:\"numerical method\" OR all:\"monte carlo\") AND ({quoted}) AND (cat:physics.comp-ph OR cat:physics.data-an OR cat:physics.chem-ph OR cat:quant-ph OR cat:cond-mat.stat-mech OR cat:hep-lat)"
    )
}

fn abs_url(id: &str) -> String {
    let mut url = id.replace("http://", "https://");
    url = url.replace("arxiv.org/pdf/", "arxiv.org/abs/");
    if let Some(pos) = url.rfind("/abs/") {
        let (head, tail) = url.split_at(pos + 5);
        let cleaned = if let Some(vpos) = tail.rfind('v') {
            if vpos > 0 && tail[vpos + 1..].chars().all(|ch| ch.is_ascii_digit()) {
                &tail[..vpos]
            } else {
                tail
            }
        } else {
            tail
        };
        format!("{head}{cleaned}")
    } else {
        url
    }
}

fn parse_atom(xml: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for chunk in xml.split("<entry>").skip(1) {
        let block = chunk.split("</entry>").next().unwrap_or("");
        let title = decode(tag_text(block, "title"));
        let id = tag_text(block, "id");
        let summary = decode(tag_text(block, "summary"));
        if title.is_empty() || id.is_empty() {
            continue;
        }
        let url = abs_url(&id);
        hits.push(Hit {
            title,
            url,
            origin: "arxiv".into(),
            blurb: summary.chars().take(280).collect(),
        });
    }
    hits
}

async fn search_wikipedia(
    permissions: &Permissions,
    query: &str,
    limit: usize,
) -> Result<Vec<Hit>, AgentError> {
    let searches = [
        format!("{query} algorithm"),
        format!("computational physics {query}"),
        "quantum algorithm".to_owned(),
    ];
    let mut hits = Vec::new();
    for (index, term) in searches.iter().enumerate() {
        if hits.len() >= limit {
            break;
        }
        if index > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        let url = format!(
            "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&srlimit=3&format=json&utf8=1",
            urlencoding(term)
        );
        let (_resolved, body) = fetch::fetch_url(permissions, &url).await?;
        let parsed: Value = serde_json::from_str(&body).unwrap_or(json!({}));
        if let Some(rows) = parsed
            .pointer("/query/search")
            .and_then(|v| v.as_array())
        {
            for row in rows {
                let title = row.get("title").and_then(|v| v.as_str()).unwrap_or("");
                if title.is_empty() {
                    continue;
                }
                let page = title.replace(' ', "_");
                hits.push(Hit {
                    title: title.to_owned(),
                    url: format!(
                        "https://en.wikipedia.org/w/index.php?title={page}&action=raw"
                    ),
                    origin: "wikipedia".into(),
                    blurb: decode_html(row.get("snippet").and_then(|v| v.as_str()).unwrap_or("")),
                });
            }
        }
    }
    Ok(dedupe(hits))
}

fn tag_text(block: &str, name: &str) -> String {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let Some(start) = block.find(&open) else {
        return String::new();
    };
    let after = &block[start..];
    let Some(gt) = after.find('>') else {
        return String::new();
    };
    let rest = &after[gt + 1..];
    rest.split(&close)
        .next()
        .unwrap_or("")
        .trim()
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode(value: String) -> String {
    decode_html(&value)
}

fn decode_html(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("<span class=\"searchmatch\">", "")
        .replace("</span>", "")
}

fn urlencoding(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            ' ' => out.push('+'),
            _ => {
                for byte in ch.encode_utf8(&mut [0; 4]).as_bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    out
}

fn dedupe(hits: Vec<Hit>) -> Vec<Hit> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for hit in hits {
        let key = hit.url.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(hit);
        }
    }
    out
}

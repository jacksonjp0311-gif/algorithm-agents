use std::path::{Path, PathBuf};

use crate::error::AgentError;
use crate::permissions::Permissions;

pub fn resolve_local(root: &Path, locator: &str) -> Result<(String, String), AgentError> {
    if let Some(rest) = locator.strip_prefix("fixture://") {
        let path = contained_path(&root.join("fixtures"), Path::new(rest))?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            AgentError::NotFound(format!("fixture `{}`: {error}", path.display()))
        })?;
        return Ok((path.to_string_lossy().into_owned(), text));
    }
    if let Some(rest) = locator.strip_prefix("sources://") {
        let path = contained_path(&root.join("sources"), Path::new(rest))?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            AgentError::NotFound(format!("source `{}`: {error}", path.display()))
        })?;
        return Ok((path.to_string_lossy().into_owned(), text));
    }
    if let Some(rest) = locator.strip_prefix("file://") {
        let requested = PathBuf::from(rest);
        let path = [root.join("sources"), root.join("fixtures")]
            .iter()
            .find_map(|base| contained_path(base, &requested).ok())
            .ok_or_else(|| {
                AgentError::Denied(
                    "file:// access is restricted to the configured sources and fixtures roots"
                        .into(),
                )
            })?;
        let text = std::fs::read_to_string(&path)
            .map_err(|error| AgentError::NotFound(format!("file `{}`: {error}", path.display())))?;
        return Ok((path.to_string_lossy().into_owned(), text));
    }
    Err(AgentError::Denied(
        "live URL retrieval needs --live (or ALGO_LIVE_FETCH=1); use fixture://, sources://, or file:// locators otherwise".into(),
    ))
}

fn contained_path(base: &Path, requested: &Path) -> Result<PathBuf, AgentError> {
    let base = std::fs::canonicalize(base).map_err(|error| {
        AgentError::NotFound(format!("source root `{}`: {error}", base.display()))
    })?;
    let joined = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base.join(requested)
    };
    let resolved = std::fs::canonicalize(&joined)
        .map_err(|error| AgentError::NotFound(format!("source `{}`: {error}", joined.display())))?;
    if !resolved.starts_with(&base) {
        return Err(AgentError::Denied(format!(
            "source path escapes configured root `{}`",
            base.display()
        )));
    }
    if !resolved.is_file() {
        return Err(AgentError::Denied(
            "source locator must resolve to a file".into(),
        ));
    }
    Ok(resolved)
}

pub fn resolve_source(root: &Path, locator: &str) -> Result<(String, String), AgentError> {
    resolve_local(root, locator)
}

pub fn live_fetch_blocked(permissions: &Permissions, url: &str) -> Option<String> {
    if url.starts_with("fixture://") || url.starts_with("sources://") || url.starts_with("file://")
    {
        return None;
    }
    if !permissions.live_fetch_enabled {
        return Some("live fetch is disabled; pass --live or set ALGO_LIVE_FETCH=1".into());
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Some("only http(s) URLs are considered".into());
    }
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("172.16.")
        || host.starts_with("172.17.")
        || host.starts_with("172.18.")
        || host.starts_with("172.19.")
        || host.starts_with("172.2")
        || host.starts_with("172.30.")
        || host.starts_with("172.31.")
        || host.ends_with(".local")
        || host == "169.254.169.254"
        || host == "[::1]"
        || host == "::1"
    {
        return Some("private or local hosts are denied".into());
    }
    if permissions
        .denied_hosts
        .iter()
        .any(|item| host == item.to_ascii_lowercase())
    {
        return Some(format!("host `{host}` is denied"));
    }
    if !permissions.allowed_hosts.is_empty()
        && !permissions.allowed_hosts.iter().any(|item| {
            let item = item.to_ascii_lowercase();
            host == item || host.ends_with(&format!(".{item}"))
        })
    {
        return Some(format!("host `{host}` is not on the allowlist"));
    }
    None
}

pub fn list_local_sources(root: &Path) -> Result<Vec<PathBuf>, AgentError> {
    let mut out = Vec::new();
    visit(&root.join("fixtures"), &mut out)?;
    visit(&root.join("sources"), &mut out)?;
    out.sort();
    Ok(out)
}

pub fn list_fixture_files(root: &Path) -> Result<Vec<PathBuf>, AgentError> {
    list_local_sources(root)
}

fn visit(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), AgentError> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if path.is_dir() {
            visit(&path, out)?;
        } else if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("md" | "txt" | "rst")
        ) && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.eq_ignore_ascii_case("readme.md"))
        {
            out.push(path);
        }
    }
    Ok(())
}

pub fn canonical_fetch_url(locator: &str) -> String {
    if let Some(id) = arxiv_id(locator) {
        return format!("https://export.arxiv.org/api/query?id_list={id}");
    }
    if let Some(title) = wikipedia_title(locator) {
        return format!(
            "https://en.wikipedia.org/w/api.php?action=query&prop=extracts&explaintext=1&redirects=1&format=json&utf8=1&titles={}",
            urlencoding(&title)
        );
    }
    locator.to_owned()
}

pub fn normalize_source_text(locator: &str, raw: &str) -> String {
    if raw.contains("<feed") && raw.contains("<entry>") {
        return atom_to_labeled(raw, locator);
    }
    if raw.contains("\"query\"") && raw.contains("\"extract\"") {
        return wikipedia_json_to_labeled(raw, locator);
    }
    if looks_like_html(raw) {
        return crate::parse::clean_unstructured(&crate::parse::strip_markup(raw));
    }
    if raw.contains("{{") || raw.contains("'''") {
        return crate::parse::clean_wikitext(raw);
    }
    crate::parse::clean_unstructured(raw)
}

pub fn arxiv_id(locator: &str) -> Option<String> {
    let lower = locator.to_ascii_lowercase();
    let marker = if let Some(rest) = lower.split("/abs/").nth(1) {
        rest
    } else if let Some(rest) = lower.split("/pdf/").nth(1) {
        rest
    } else if let Some(rest) = lower.split("id_list=").nth(1) {
        rest
    } else {
        return None;
    };
    if !locator.to_ascii_lowercase().contains("arxiv.org") && !locator.contains("id_list=") {
        return None;
    }
    let id = marker
        .split(['?', '#', '&'])
        .next()
        .unwrap_or("")
        .trim_end_matches(".pdf");
    let id = id
        .rsplit_once('v')
        .filter(|(_, ver)| ver.chars().all(|ch| ch.is_ascii_digit()))
        .map(|(head, _)| head)
        .unwrap_or(id);
    if id.is_empty() {
        None
    } else {
        Some(id.to_owned())
    }
}

pub fn wikipedia_title(locator: &str) -> Option<String> {
    let lower = locator.to_ascii_lowercase();
    if !lower.contains("wikipedia.org") {
        return None;
    }
    if let Some(rest) = locator.split("title=").nth(1) {
        let title = rest.split('&').next().unwrap_or(rest);
        return Some(decode_url(title).replace('_', " "));
    }
    if let Some(rest) = locator.split("/wiki/").nth(1) {
        let title = rest.split(['?', '#']).next().unwrap_or(rest);
        return Some(decode_url(title).replace('_', " "));
    }
    None
}

fn atom_to_labeled(xml: &str, locator: &str) -> String {
    let block = xml
        .split("<entry>")
        .nth(1)
        .and_then(|chunk| chunk.split("</entry>").next())
        .unwrap_or(xml);
    let title = xml_tag(block, "title");
    let summary = xml_tag(block, "summary");
    let published = xml_tag(block, "published");
    let year = published.get(..4).unwrap_or("").to_owned();
    let authors: Vec<String> = block
        .split("<author>")
        .skip(1)
        .filter_map(|chunk| {
            let name = xml_tag(chunk, "name");
            if name.is_empty() { None } else { Some(name) }
        })
        .collect();
    let abs = arxiv_id(locator)
        .or_else(|| arxiv_id(&xml_tag(block, "id")))
        .map(|id| format!("https://arxiv.org/abs/{id}"))
        .unwrap_or_else(|| locator.to_owned());
    let hay = format!("{title} {summary}").to_ascii_lowercase();
    let algorithm = if hay.contains("algorithm")
        || hay.contains("monte carlo")
        || hay.contains("method")
        || hay.contains("procedure")
    {
        "yes"
    } else {
        "uncertain"
    };
    format!(
        "TITLE: {title}\nAUTHORS:\n{}\nYEAR: {year}\nTYPE: paper\nURL: {abs}\nDOMAIN: Computational Science\nABSTRACT: {summary}\nALGORITHM: {algorithm}\nAMBIGUOUS: {}\n",
        authors
            .iter()
            .map(|name| format!("- {name}"))
            .collect::<Vec<_>>()
            .join("\n"),
        if algorithm == "yes" { "no" } else { "yes" }
    )
}

fn wikipedia_json_to_labeled(raw: &str, locator: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::json!({}));
    let pages = parsed
        .pointer("/query/pages")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let page = pages.values().next();
    let fallback_title = wikipedia_title(locator).unwrap_or_else(|| "Untitled".into());
    let title = page
        .and_then(|v| v.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_title.as_str())
        .to_owned();
    let extract = page
        .and_then(|v| v.get("extract"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_owned();
    let url = wikipedia_title(locator)
        .map(|name| format!("https://en.wikipedia.org/wiki/{}", name.replace(' ', "_")))
        .unwrap_or_else(|| locator.to_owned());
    let hay = format!("{title} {extract}").to_ascii_lowercase();
    let algorithm =
        if hay.contains("algorithm") || hay.contains("procedure") || hay.contains("monte carlo") {
            "yes"
        } else {
            "uncertain"
        };
    format!(
        "TITLE: {title}\nTYPE: encyclopedia\nURL: {url}\nABSTRACT: {extract}\nALGORITHM: {algorithm}\nAMBIGUOUS: {}\n",
        if algorithm == "yes" { "no" } else { "yes" }
    )
}

fn xml_tag(block: &str, name: &str) -> String {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let Some(start) = block.find(&open) else {
        return String::new();
    };
    let after = &block[start..];
    let Some(gt) = after.find('>') else {
        return String::new();
    };
    after[gt + 1..]
        .split(&close)
        .next()
        .unwrap_or("")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_html(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("<html") || lower.contains("<article") || lower.contains("<p>")
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

fn decode_url(value: &str) -> String {
    let mut out = String::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &value[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

pub fn fixture_locator(root: &Path, path: &Path) -> String {
    let fixtures = root.join("fixtures");
    if let Ok(rel) = path.strip_prefix(&fixtures) {
        return format!("fixture://{}", rel.to_string_lossy().replace('\\', "/"));
    }
    let sources = root.join("sources");
    if let Ok(rel) = path.strip_prefix(&sources) {
        return format!("sources://{}", rel.to_string_lossy().replace('\\', "/"));
    }
    format!("file://{}", path.to_string_lossy())
}

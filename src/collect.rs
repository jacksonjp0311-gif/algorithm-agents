use std::path::{Path, PathBuf};

use crate::error::AgentError;
use crate::permissions::Permissions;

pub fn resolve_local(root: &Path, locator: &str) -> Result<(String, String), AgentError> {
    if let Some(rest) = locator.strip_prefix("fixture://") {
        let path = root.join("fixtures").join(rest);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            AgentError::NotFound(format!("fixture `{}`: {error}", path.display()))
        })?;
        return Ok((path.to_string_lossy().into_owned(), text));
    }
    if let Some(rest) = locator.strip_prefix("sources://") {
        let path = root.join("sources").join(rest);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            AgentError::NotFound(format!("source `{}`: {error}", path.display()))
        })?;
        return Ok((path.to_string_lossy().into_owned(), text));
    }
    if let Some(rest) = locator.strip_prefix("file://") {
        let path = PathBuf::from(rest);
        let text = std::fs::read_to_string(&path).map_err(|error| {
            AgentError::NotFound(format!("file `{}`: {error}", path.display()))
        })?;
        return Ok((path.to_string_lossy().into_owned(), text));
    }
    Err(AgentError::Denied(
        "live URL retrieval needs --live (or ALGO_LIVE_FETCH=1); use fixture://, sources://, or file:// locators otherwise".into(),
    ))
}

pub fn resolve_source(root: &Path, locator: &str) -> Result<(String, String), AgentError> {
    resolve_local(root, locator)
}

pub fn live_fetch_blocked(permissions: &Permissions, url: &str) -> Option<String> {
    if url.starts_with("fixture://") || url.starts_with("sources://") || url.starts_with("file://") {
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

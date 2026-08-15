use std::time::Duration;

use crate::error::AgentError;
use crate::permissions::Permissions;

pub async fn fetch_url(permissions: &Permissions, url: &str) -> Result<(String, String), AgentError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(permissions.fetch_timeout_seconds.max(1)))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(&permissions.user_agent)
        .build()
        .map_err(|error| AgentError::Internal(error.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| AgentError::Io(format!("fetch failed: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AgentError::Io(format!(
            "fetch {url} returned HTTP {status}"
        )));
    }
    let final_url = response.url().to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AgentError::Io(format!("read body failed: {error}")))?;
    if bytes.len() > permissions.max_fetch_bytes {
        return Err(AgentError::Budget("retrieved source exceeds size limit".into()));
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok((final_url, text))
}

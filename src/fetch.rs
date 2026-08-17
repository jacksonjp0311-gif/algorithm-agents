use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use reqwest::header::LOCATION;

use crate::error::AgentError;
use crate::permissions::Permissions;

pub async fn fetch_url(
    permissions: &Permissions,
    url: &str,
) -> Result<(String, String), AgentError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(
            permissions.fetch_timeout_seconds.max(1),
        ))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(&permissions.user_agent)
        .build()
        .map_err(|error| AgentError::Internal(error.to_string()))?;
    let mut current = reqwest::Url::parse(url)
        .map_err(|error| AgentError::Invalid(format!("invalid URL: {error}")))?;
    let mut redirect_chain = Vec::new();

    for hop in 0..=permissions.max_redirects {
        validate_network_target(permissions, &current).await?;
        let mut response = client
            .get(current.clone())
            .send()
            .await
            .map_err(|error| AgentError::Io(format!("fetch failed: {error}")))?;
        if response.status().is_redirection() {
            if hop == permissions.max_redirects {
                return Err(AgentError::Denied("redirect limit exceeded".into()));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| AgentError::Io("redirect omitted Location header".into()))?;
            let next = current
                .join(location)
                .map_err(|error| AgentError::Invalid(format!("invalid redirect: {error}")))?;
            redirect_chain.push(current.to_string());
            current = next;
            continue;
        }
        if !response.status().is_success() {
            return Err(AgentError::Io(format!(
                "fetch {current} returned HTTP {}",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|size| size > permissions.max_fetch_bytes as u64)
        {
            return Err(AgentError::Budget(
                "retrieved source exceeds size limit".into(),
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| AgentError::Io(format!("read body failed: {error}")))?
        {
            if bytes.len().saturating_add(chunk.len()) > permissions.max_fetch_bytes {
                return Err(AgentError::Budget(
                    "retrieved source exceeds size limit".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        return Ok((current.to_string(), text));
    }
    Err(AgentError::Denied(format!(
        "redirect chain was not resolved: {redirect_chain:?}"
    )))
}

pub async fn validate_network_target(
    permissions: &Permissions,
    url: &reqwest::Url,
) -> Result<(), AgentError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AgentError::Denied("only http(s) URLs are allowed".into()));
    }
    if permissions.require_https && url.scheme() != "https" {
        return Err(AgentError::Denied(
            "HTTPS is required for live retrieval".into(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AgentError::Denied("URL credentials are not allowed".into()));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AgentError::Denied("URL host is required".into()))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return Err(AgentError::Denied(
            "private or local hosts are denied".into(),
        ));
    }
    if permissions
        .denied_hosts
        .iter()
        .any(|item| host == item.trim_end_matches('.').to_ascii_lowercase())
    {
        return Err(AgentError::Denied(format!("host `{host}` is denied")));
    }
    if !permissions.allowed_hosts.is_empty()
        && !permissions.allowed_hosts.iter().any(|item| {
            let item = item.trim_end_matches('.').to_ascii_lowercase();
            host == item || host.ends_with(&format!(".{item}"))
        })
    {
        return Err(AgentError::Denied(format!(
            "host `{host}` is not on the allowlist"
        )));
    }

    let port = url
        .port_or_known_default()
        .ok_or_else(|| AgentError::Denied("URL port is unsupported".into()))?;
    let resolved = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|error| AgentError::Io(format!("DNS resolution failed: {error}")))?;
    let addresses: Vec<IpAddr> = resolved.map(|addr| addr.ip()).collect();
    if addresses.is_empty() {
        return Err(AgentError::Io(
            "DNS resolution returned no addresses".into(),
        ));
    }
    if addresses.iter().any(|address| unsafe_address(*address)) {
        return Err(AgentError::Denied(format!(
            "host `{host}` resolves to a private, local, or reserved address"
        )));
    }
    Ok(())
}

pub fn unsafe_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => unsafe_ipv4(ip),
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return unsafe_ipv4(mapped);
            }
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
        }
    }
}

fn unsafe_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.octets()[0] == 0
        || ip.octets()[0] >= 224
        || ip.octets() == [169, 254, 169, 254]
}

use crate::error::{CoreError, Result};
use crate::model::Provider;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest::{Method, StatusCode};
use serde_json::Value;
use std::time::Duration;

/// How the credential is attached to a provider request.
#[derive(Clone)]
pub enum Auth {
    Bearer(String),
    Header(&'static str, String),
    None,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the secret itself.
        match self {
            Auth::Bearer(_) => f.write_str("Auth::Bearer(<redacted>)"),
            Auth::Header(name, _) => write!(f, "Auth::Header({name}, <redacted>)"),
            Auth::None => f.write_str("Auth::None"),
        }
    }
}

/// Thin JSON client shared by the REST providers. Centralises timeouts,
/// retries, status handling and message extraction so each provider module only
/// deals with its own request/response shapes.
#[derive(Debug, Clone)]
pub struct ApiClient {
    provider: Provider,
    base: String,
    auth: Auth,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(provider: Provider, base: impl Into<String>, auth: Auth) -> Result<Self> {
        let base = base.into();
        let base = base.trim_end_matches('/').to_string();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(CoreError::invalid(format!(
                "{} base URL must start with http:// or https://",
                provider.label()
            )));
        }
        let client = reqwest::Client::builder()
            .user_agent(concat!("BTFarm/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| CoreError::Transport {
                provider: provider.label().to_string(),
                message: e.to_string(),
            })?;
        Ok(Self {
            provider,
            base,
            auth,
            client,
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn provider(&self) -> Provider {
        self.provider
    }

    fn label(&self) -> String {
        self.provider.label().to_string()
    }

    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("BTFarm/", env!("CARGO_PKG_VERSION"))),
        );
        match &self.auth {
            Auth::Bearer(token) => {
                let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|_| CoreError::invalid("API key contains characters that cannot be sent in a header"))?;
                value.set_sensitive(true);
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
            Auth::Header(name, token) => {
                let header = HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| CoreError::invalid(format!("invalid auth header name {name}")))?;
                let mut value = HeaderValue::from_str(token)
                    .map_err(|_| CoreError::invalid("API key contains characters that cannot be sent in a header"))?;
                value.set_sensitive(true);
                headers.insert(header, value);
            }
            Auth::None => {}
        }
        Ok(headers)
    }

    /// Sends a request and decodes a JSON body. `204 No Content` yields `Null`.
    pub async fn request(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        self.request_with_headers(method, path, body, &[]).await
    }

    /// Same, plus extra headers (providers such as Boat require a confirmation
    /// header for destructive calls).
    pub async fn request_with_headers(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        extra: &[(&str, &str)],
    ) -> Result<Value> {
        let mut attempt = 0u32;
        let extra: Vec<(String, String)> = extra
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        loop {
            attempt += 1;
            match self.send_once(method.clone(), path, body.clone(), &extra).await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    let retryable = is_retryable(&err);
                    if retryable && attempt < 3 {
                        let wait = backoff(attempt);
                        tracing::debug!(
                            provider = self.provider.slug(),
                            attempt,
                            wait_ms = wait.as_millis() as u64,
                            "retrying provider request after transient failure"
                        );
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }

    async fn send_once(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        extra: &[(String, String)],
    ) -> Result<Value> {
        let url = self.url(path);
        let mut headers = self.headers()?;
        for (key, value) in extra {
            let name = HeaderName::from_bytes(key.as_bytes())
                .map_err(|_| CoreError::invalid(format!("invalid header name {key}")))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|_| CoreError::invalid(format!("invalid value for header {key}")))?;
            headers.insert(name, header_value);
        }
        let mut builder = self.client.request(method.clone(), &url).headers(headers);
        if let Some(payload) = body {
            builder = builder.json(&payload);
        }
        let response = builder.send().await.map_err(|e| CoreError::Transport {
            provider: self.label(),
            message: transport_message(&e),
        })?;
        let status = response.status();
        let bytes = response.bytes().await.unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes).to_string();

        if status == StatusCode::NO_CONTENT || (status.is_success() && text.trim().is_empty()) {
            return Ok(Value::Null);
        }
        if status.is_success() {
            return serde_json::from_str(&text).map_err(|e| CoreError::Api {
                provider: self.label(),
                status: status.as_u16(),
                message: format!("provider returned a body that is not JSON ({e}); first 200 bytes: {}", truncate(&text, 200)),
            });
        }
        Err(CoreError::Api {
            provider: self.label(),
            status: status.as_u16(),
            message: extract_error_message(&text),
        })
    }

    pub fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_string();
        }
        format!("{}{}", self.base, path)
    }
}

fn transport_message(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "request timed out".to_string()
    } else if err.is_connect() {
        "connection failed (check network access and the base URL)".to_string()
    } else {
        err.to_string()
    }
}

fn is_retryable(err: &CoreError) -> bool {
    match err {
        CoreError::Api { status, .. } => {
            *status == 429 || *status == 502 || *status == 503 || *status == 504
        }
        CoreError::Transport { .. } => true,
        _ => false,
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(250 * 2u64.pow(attempt.min(4)))
}

pub fn truncate(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

/// Pulls the human-readable message out of the several error envelopes the
/// providers use (`message`, `error`, `detail`, `errors[].message`) and never
/// echoes a bearer token.
pub fn extract_error_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "provider returned an empty error body".to_string();
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        for key in ["message", "error", "detail", "error_description", "reason"] {
            if let Some(text) = value.get(key).and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    return redact(text);
                }
            }
        }
        if let Some(first) = value.get("errors").and_then(|v| v.as_array()).and_then(|a| a.first()) {
            if let Some(text) = first.get("message").and_then(|v| v.as_str()) {
                return redact(text);
            }
            if let Some(text) = first.as_str() {
                return redact(text);
            }
        }
        if let Some(text) = value.get("title").and_then(|v| v.as_str()) {
            return redact(text);
        }
        return redact(&truncate(trimmed, 300));
    }
    // HTML error pages are common at the edge; strip tags so the log stays readable.
    let stripped = strip_tags(trimmed);
    redact(&truncate(stripped.trim(), 300))
}

fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Removes anything that looks like a credential from provider text.
pub fn redact(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for token in input.split_inclusive(|c: char| c.is_whitespace()) {
        let word = token.trim_end();
        let looks_secret = word.starts_with("e2b_")
            || word.starts_with("boat_")
            || word.starts_with("bxd_")
            || word.starts_with("dtn_")
            || word.starts_with("sk-")
            || (word.len() > 32 && word.chars().all(|c| c.is_ascii_alphanumeric()));
        if looks_secret {
            out.push_str("[redacted]");
            if token.ends_with(char::is_whitespace) {
                out.push(' ');
            }
        } else {
            out.push_str(token);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_messages_from_common_envelopes() {
        assert_eq!(extract_error_message(r#"{"message":"bad key"}"#), "bad key");
        assert_eq!(extract_error_message(r#"{"error":"nope"}"#), "nope");
        assert_eq!(
            extract_error_message(r#"{"errors":[{"message":"invalid template"}]}"#),
            "invalid template"
        );
    }

    #[test]
    fn strips_html_error_pages() {
        let msg = extract_error_message("<html><body><h1>502 Bad Gateway</h1></body></html>");
        assert!(msg.contains("502 Bad Gateway"));
        assert!(!msg.contains('<'));
    }

    #[test]
    fn redacts_secret_shaped_tokens() {
        let text = "auth failed for boat_abc123 and AKIAABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
        let out = redact(text);
        assert!(!out.contains("boat_abc123"));
        assert!(!out.contains("AKIAABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
        assert!(out.contains("[redacted]"));
    }

    #[test]
    fn url_join_is_predictable() {
        let client = ApiClient::new(Provider::E2b, "https://api.e2b.app/", Auth::None).unwrap();
        assert_eq!(client.url("/sandboxes"), "https://api.e2b.app/sandboxes");
        assert_eq!(client.url("https://x.dev/y"), "https://x.dev/y");
    }

    #[test]
    fn rejects_non_http_base() {
        assert!(ApiClient::new(Provider::Boat, "boat.dev", Auth::None).is_err());
    }

    #[test]
    fn auth_debug_is_redacted() {
        let auth = Auth::Bearer("super-secret".into());
        assert!(!format!("{auth:?}").contains("super-secret"));
    }
}

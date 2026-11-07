use crate::error::{CoreError, Result};
use crate::model::PublisherKind;
use serde_json::{json, Value};
use std::time::Duration;

/// Publishing goes only through platforms' official APIs.
///
/// * `Mastodon` — the public Mastodon REST API with a user access token.
/// * `Webhook` — a request template the user fills in with the official endpoint
///   of any other platform (Buffer, X, Instagram Graph, TikTok, a custom service).
///
/// Nothing here automates account creation, bypasses a CAPTCHA or evades a
/// platform's rate limits.
#[derive(Clone)]
pub struct Publisher {
    http: reqwest::Client,
}

pub struct PublishOutcome {
    pub remote_id: Option<String>,
    pub detail: String,
}

impl Publisher {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(45))
            .user_agent(concat!("BTFarm/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self { http }
    }

    /// Confirms the credentials work against the real API.
    pub async fn verify(
        &self,
        kind: PublisherKind,
        config: &Value,
        secret: Option<&str>,
    ) -> Result<String> {
        match kind {
            PublisherKind::Mastodon => {
                let instance = mastodon_instance(config)?;
                let token = require_secret(secret, "a Mastodon access token")?;
                let response = self
                    .http
                    .get(format!("{instance}/api/v1/accounts/verify_credentials"))
                    .bearer_auth(token)
                    .header("Accept", "application/json")
                    .send()
                    .await
                    .map_err(|e| CoreError::Transport {
                        provider: "Mastodon".into(),
                        message: e.to_string(),
                    })?;
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                if !status.is_success() {
                    return Err(CoreError::Api {
                        provider: "Mastodon".into(),
                        status: status.as_u16(),
                        message: crate::http::extract_error_message(&text),
                    });
                }
                let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                let acct = value.get("acct").and_then(|v| v.as_str()).unwrap_or("unknown");
                Ok(format!("@{acct}"))
            }
            PublisherKind::Webhook => {
                let url = webhook_url(config)?;
                let method = webhook_method(config);
                let mut request = self.http.request(reqwest::Method::GET, &url);
                if let Some(token) = secret.filter(|s| !s.trim().is_empty()) {
                    request = request.bearer_auth(token);
                }
                for (key, value) in header_pairs(config) {
                    request = request.header(key, value);
                }
                match tokio::time::timeout(Duration::from_secs(20), request.send()).await {
                    Ok(Ok(response)) => {
                        let status = response.status().as_u16();
                        if status >= 500 {
                            return Err(CoreError::Automation(format!(
                                "{url} answered {status} to a reachability probe"
                            )));
                        }
                        Ok(format!(
                            "endpoint reachable (probe → {status}); publishing will use {method}"
                        ))
                    }
                    Ok(Err(e)) => Err(CoreError::Transport {
                        provider: "HTTP publisher".into(),
                        message: e.to_string(),
                    }),
                    Err(_) => Err(CoreError::Transport {
                        provider: "HTTP publisher".into(),
                        message: "the reachability probe timed out".into(),
                    }),
                }
            }
        }
    }

    pub async fn publish(
        &self,
        kind: PublisherKind,
        config: &Value,
        secret: Option<&str>,
        body: &str,
    ) -> Result<PublishOutcome> {
        match kind {
            PublisherKind::Mastodon => {
                let instance = mastodon_instance(config)?;
                let token = require_secret(secret, "a Mastodon access token")?;
                let response = self
                    .http
                    .post(format!("{instance}/api/v1/statuses"))
                    .bearer_auth(token)
                    .json(&json!({ "status": body }))
                    .send()
                    .await
                    .map_err(|e| CoreError::Transport {
                        provider: "Mastodon".into(),
                        message: e.to_string(),
                    })?;
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                if !status.is_success() {
                    return Err(CoreError::Api {
                        provider: "Mastodon".into(),
                        status: status.as_u16(),
                        message: crate::http::extract_error_message(&text),
                    });
                }
                let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                let id = value.get("id").and_then(|v| v.as_str()).map(str::to_string);
                let url = value
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(PublishOutcome {
                    remote_id: id,
                    detail: if url.is_empty() {
                        "published through the Mastodon API".to_string()
                    } else {
                        format!("published: {url}")
                    },
                })
            }
            PublisherKind::Webhook => {
                let url = webhook_url(config)?;
                let method = reqwest::Method::from_bytes(webhook_method(config).as_bytes())
                    .map_err(|_| CoreError::invalid("the configured HTTP method is not valid"))?;
                let template = config
                    .get("body_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{\"content\":\"{{content}}\"}");
                let rendered = template.replace("{{content}}", &json_escape(body));
                let mut request = self.http.request(method, &url);
                let mut has_content_type = false;
                for (key, value) in header_pairs(config) {
                    if key.eq_ignore_ascii_case("content-type") {
                        has_content_type = true;
                    }
                    request = request.header(key, value);
                }
                if !has_content_type {
                    request = request.header("Content-Type", "application/json");
                }
                if let Some(token) = secret.filter(|s| !s.trim().is_empty()) {
                    request = request.bearer_auth(token);
                }
                let response = request.body(rendered).send().await.map_err(|e| CoreError::Transport {
                    provider: "HTTP publisher".into(),
                    message: e.to_string(),
                })?;
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                if !status.is_success() {
                    return Err(CoreError::Api {
                        provider: "HTTP publisher".into(),
                        status: status.as_u16(),
                        message: crate::http::extract_error_message(&text),
                    });
                }
                let remote_id = serde_json::from_str::<Value>(&text).ok().and_then(|v| {
                    v.get("id")
                        .or_else(|| v.get("post_id"))
                        .and_then(|x| match x {
                            Value::String(s) => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                });
                Ok(PublishOutcome {
                    remote_id,
                    detail: format!("endpoint answered {status}"),
                })
            }
        }
    }
}

impl Default for Publisher {
    fn default() -> Self {
        Self::new()
    }
}

fn mastodon_instance(config: &Value) -> Result<String> {
    let raw = config
        .get("instance_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string();
    if raw.is_empty() {
        return Err(CoreError::invalid("add the Mastodon instance URL, e.g. https://mastodon.social"));
    }
    if !(raw.starts_with("http://") || raw.starts_with("https://")) {
        return Err(CoreError::invalid("the Mastodon instance URL must start with https://"));
    }
    Ok(raw)
}

fn webhook_url(config: &Value) -> Result<String> {
    let raw = config
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if !(raw.starts_with("http://") || raw.starts_with("https://")) {
        return Err(CoreError::invalid("add the official API endpoint URL (https://…)"));
    }
    Ok(raw)
}

fn webhook_method(config: &Value) -> String {
    config
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("POST")
        .to_ascii_uppercase()
}

fn header_pairs(config: &Value) -> Vec<(String, String)> {
    config
        .get("headers")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn require_secret<'a>(secret: Option<&'a str>, what: &str) -> Result<&'a str> {
    match secret {
        Some(value) if !value.trim().is_empty() => Ok(value.trim()),
        _ => Err(CoreError::NotConfigured {
            provider: "publisher".into(),
            message: format!("add {what} for this account"),
        }),
    }
}

fn json_escape(value: &str) -> String {
    let quoted = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string());
    quoted[1..quoted.len() - 1].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_content_for_json_bodies() {
        assert_eq!(json_escape("a \"quote\"\nline"), "a \\\"quote\\\"\\nline");
    }

    #[test]
    fn mastodon_instance_must_be_a_url() {
        assert!(mastodon_instance(&json!({"instance_url": "mastodon.social"})).is_err());
        assert_eq!(
            mastodon_instance(&json!({"instance_url": "https://mastodon.social/"})).unwrap(),
            "https://mastodon.social"
        );
    }

    #[test]
    fn method_defaults_to_post() {
        assert_eq!(webhook_method(&json!({})), "POST");
        assert_eq!(webhook_method(&json!({"method": "put"})), "PUT");
    }
}

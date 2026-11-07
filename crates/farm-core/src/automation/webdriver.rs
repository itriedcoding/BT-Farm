use crate::error::{CoreError, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// W3C WebDriver client. This speaks the standard protocol that Selenium Grid
/// and chromedriver/geckodriver expose, so any Selenium endpoint works —
/// including a Selenium container running inside one of the provisioned VMs.
#[derive(Clone)]
pub struct WebDriver {
    base: String,
    http: reqwest::Client,
}

impl std::fmt::Debug for WebDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebDriver").field("base", &self.base).finish()
    }
}

impl WebDriver {
    pub fn new(base_url: &str) -> Result<Self> {
        let base = base_url.trim().trim_end_matches('/').to_string();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(CoreError::invalid(
                "the Selenium / WebDriver URL must start with http:// or https://",
            ));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("BTFarm/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| CoreError::Automation(format!("webdriver client: {e}")))?;
        Ok(Self { base, http })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    async fn send(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let mut builder = self
            .http
            .request(method.clone(), &url)
            .header("Content-Type", "application/json");
        if let Some(payload) = body {
            builder = builder.json(&payload);
        }
        let response = builder.send().await.map_err(|e| {
            CoreError::Automation(format!(
                "WebDriver request {method} {path} failed: {}",
                if e.is_connect() {
                    "connection refused (is the Selenium endpoint running and reachable?)".to_string()
                } else {
                    e.to_string()
                }
            ))
        })?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if !status.is_success() {
            let inner = value.get("value").unwrap_or(&value);
            let message = inner
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| if text.trim().is_empty() { status.canonical_reason().unwrap_or("error") } else { &text });
            let error_kind = inner.get("error").and_then(|v| v.as_str()).unwrap_or("webdriver error");
            return Err(CoreError::Automation(format!(
                "WebDriver {error_kind} ({}): {}",
                status.as_u16(),
                crate::http::truncate(message.lines().next().unwrap_or(message), 300)
            )));
        }
        Ok(value.get("value").cloned().unwrap_or(value))
    }

    /// GET /status — used by "test connection" before a browser is launched.
    pub async fn verify(&self) -> Result<String> {
        let value = self.send(reqwest::Method::GET, "/status", None).await?;
        let ready = value.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);
        let message = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if !ready && !value.is_null() {
            return Err(CoreError::Automation(format!(
                "Selenium is reachable but reports it is not ready: {message}"
            )));
        }
        Ok(if message.is_empty() {
            "WebDriver endpoint is ready".to_string()
        } else {
            message.to_string()
        })
    }

    pub async fn create_session(&self, browser: &str) -> Result<String> {
        let body = json!({
            "capabilities": {
                "alwaysMatch": { "browserName": browser }
            }
        });
        let value = self.send(reqwest::Method::POST, "/session", Some(body)).await?;
        value
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| CoreError::Automation("WebDriver did not return a sessionId".into()))
    }

    pub async fn delete_session(&self, session: &str) -> Result<()> {
        self.send(reqwest::Method::DELETE, &format!("/session/{session}"), None)
            .await
            .map(|_| ())
    }

    pub async fn navigate(&self, session: &str, url: &str) -> Result<()> {
        self.send(
            reqwest::Method::POST,
            &format!("/session/{session}/url"),
            Some(json!({ "url": url })),
        )
        .await
        .map(|_| ())
    }

    pub async fn current_url(&self, session: &str) -> Result<String> {
        let value = self
            .send(reqwest::Method::GET, &format!("/session/{session}/url"), None)
            .await?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    pub async fn find(&self, session: &str, selector: &str) -> Result<String> {
        let value = self
            .send(
                reqwest::Method::POST,
                &format!("/session/{session}/element"),
                Some(json!({ "using": "css selector", "value": selector })),
            )
            .await?;
        element_id(&value).ok_or_else(|| {
            CoreError::Automation(format!("no element matched the selector {selector:?}"))
        })
    }

    pub async fn click(&self, session: &str, selector: &str) -> Result<()> {
        let element = self.find(session, selector).await?;
        self.send(
            reqwest::Method::POST,
            &format!("/session/{session}/element/{element}/click"),
            Some(json!({})),
        )
        .await
        .map(|_| ())
    }

    pub async fn type_text(&self, session: &str, selector: &str, text: &str) -> Result<()> {
        let element = self.find(session, selector).await?;
        let chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
        self.send(
            reqwest::Method::POST,
            &format!("/session/{session}/element/{element}/value"),
            Some(json!({ "text": text, "value": chars })),
        )
        .await
        .map(|_| ())
    }

    pub async fn element_text(&self, session: &str, selector: &str) -> Result<String> {
        let element = self.find(session, selector).await?;
        let value = self
            .send(
                reqwest::Method::GET,
                &format!("/session/{session}/element/{element}/text"),
                None,
            )
            .await?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    pub async fn wait_for(&self, session: &str, selector: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            match self.find(session, selector).await {
                Ok(_) => return Ok(()),
                Err(err) => last_error = Some(err),
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        Err(CoreError::Automation(format!(
            "selector {selector:?} did not appear within {}s: {}",
            timeout.as_secs(),
            last_error.map(|e| e.to_string()).unwrap_or_default()
        )))
    }

    pub async fn screenshot(&self, session: &str) -> Result<Vec<u8>> {
        let value = self
            .send(
                reqwest::Method::GET,
                &format!("/session/{session}/screenshot"),
                None,
            )
            .await?;
        let encoded = value.as_str().unwrap_or_default();
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| CoreError::Automation(format!("screenshot was not valid base64: {e}")))
    }
}

fn element_id(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    const KEY: &str = "element-6066-11e4-a52e-4f735466cecf";
    value
        .get(KEY)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| value.get("ELEMENT").and_then(|v| v.as_str()).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_ids_are_recognised() {
        let w3c = json!({"element-6066-11e4-a52e-4f735466cecf": "abc"});
        assert_eq!(element_id(&w3c).as_deref(), Some("abc"));
        assert_eq!(element_id(&json!("xyz")).as_deref(), Some("xyz"));
        assert_eq!(element_id(&json!({"nope": 1})), None);
    }

    #[test]
    fn rejects_relative_urls() {
        assert!(WebDriver::new("127.0.0.1:4444").is_err());
        assert!(WebDriver::new("http://127.0.0.1:4444").is_ok());
    }
}

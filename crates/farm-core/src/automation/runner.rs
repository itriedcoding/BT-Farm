use crate::android::adb::Adb;
use crate::automation::webdriver::WebDriver;
use crate::error::{CoreError, Result};
use crate::model::{is_valid_package, Step, StepAction};
use base64::Engine;
use chrono::Utc;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Everything a workflow step needs. ADB and the WebDriver session are created
/// lazily so a workflow that never touches a browser does not need Selenium.
pub struct StepContext {
    pub adb: Option<Adb>,
    pub serial: Option<String>,
    pub webdriver_url: String,
    pub artifacts: PathBuf,
    http: reqwest::Client,
    wd: Mutex<Option<(WebDriver, String)>>,
}

impl StepContext {
    pub fn new(
        adb: Option<Adb>,
        serial: Option<String>,
        webdriver_url: String,
        artifacts: PathBuf,
    ) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("BTFarm/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self {
            adb,
            serial,
            webdriver_url,
            artifacts,
            http,
            wd: Mutex::new(None),
        }
    }

    fn device(&self) -> Result<(&Adb, &str)> {
        match (&self.adb, &self.serial) {
            (Some(adb), Some(serial)) => Ok((adb, serial.as_str())),
            _ => Err(CoreError::Automation(
                "this step needs an Android device, but the run has none attached".into(),
            )),
        }
    }

    async fn session(&self) -> Result<(WebDriver, String)> {
        let mut guard = self.wd.lock().await;
        if let Some((client, session)) = guard.as_ref() {
            return Ok((client.clone(), session.clone()));
        }
        let client = WebDriver::new(&self.webdriver_url)?;
        let session = client.create_session("chrome").await?;
        *guard = Some((client.clone(), session.clone()));
        Ok((client, session))
    }

    pub async fn close(&self) {
        let mut guard = self.wd.lock().await;
        if let Some((client, session)) = guard.take() {
            let _ = client.delete_session(&session).await;
        }
    }

    fn save_png(&self, bytes: &[u8], label: &str) -> Result<PathBuf> {
        let dir = self.artifacts.join("screenshots");
        std::fs::create_dir_all(&dir)?;
        let safe = label
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect::<String>();
        let path = dir.join(format!(
            "{}-{}.png",
            safe.trim_matches('-'),
            Utc::now().format("%Y%m%d-%H%M%S%.3f")
        ));
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}

/// Executes one step, returning human-readable log lines. Retries are handled by
/// the caller so that every attempt is logged.
pub async fn run_step(ctx: &StepContext, step: &Step) -> Result<Vec<String>> {
    let timeout = match step.timeout_ms {
        Some(ms) => Duration::from_millis(ms.clamp(500, 900_000)),
        None => match &step.action {
            StepAction::Wait { ms } => Duration::from_millis(*ms + 5_000),
            StepAction::WaitForActivity { timeout_ms, .. } => {
                Duration::from_millis(*timeout_ms + 10_000)
            }
            StepAction::WdWaitFor { timeout_ms, .. } => Duration::from_millis(*timeout_ms + 10_000),
            StepAction::InstallApk { .. } => Duration::from_secs(360),
            _ => Duration::from_secs(120),
        },
    };

    match tokio::time::timeout(timeout, execute(ctx, &step.action)).await {
        Ok(result) => result,
        Err(_) => Err(CoreError::Automation(format!(
            "step timed out after {}s",
            timeout.as_secs()
        ))),
    }
}

async fn execute(ctx: &StepContext, action: &StepAction) -> Result<Vec<String>> {
    match action {
        StepAction::Tap { x, y } => {
            let (adb, serial) = ctx.device()?;
            adb.tap(serial, *x, *y).await?;
            Ok(vec![format!("tapped ({x}, {y}) on {serial}")])
        }
        StepAction::Swipe { x1, y1, x2, y2, duration_ms } => {
            let (adb, serial) = ctx.device()?;
            adb.swipe(serial, *x1, *y1, *x2, *y2, *duration_ms).await?;
            Ok(vec![format!("swiped ({x1},{y1}) → ({x2},{y2}) in {duration_ms} ms")])
        }
        StepAction::Text { text } => {
            let (adb, serial) = ctx.device()?;
            adb.input_text(serial, text).await?;
            Ok(vec![format!("typed {} characters", text.chars().count())])
        }
        StepAction::Key { code } => {
            let (adb, serial) = ctx.device()?;
            adb.keyevent(serial, code).await?;
            Ok(vec![format!("sent keyevent {code}")])
        }
        StepAction::Shell { command } => {
            let (adb, serial) = ctx.device()?;
            let output = adb.shell(serial, command).await?;
            let mut lines = vec![format!("$ {command}")];
            for line in output.combined().lines().take(40) {
                lines.push(format!("  {line}"));
            }
            if output.code != 0 {
                return Err(CoreError::Automation(format!(
                    "shell command exited with {}",
                    output.code
                )));
            }
            Ok(lines)
        }
        StepAction::LaunchApp { package } => {
            if !is_valid_package(package) {
                return Err(CoreError::Automation(format!("invalid package {package}")));
            }
            let (adb, serial) = ctx.device()?;
            adb.start_app(serial, package).await?;
            Ok(vec![format!("launched {package}")])
        }
        StepAction::StopApp { package } => {
            let (adb, serial) = ctx.device()?;
            adb.stop_app(serial, package).await?;
            Ok(vec![format!("force-stopped {package}")])
        }
        StepAction::InstallApk { path } => {
            let (adb, serial) = ctx.device()?;
            if !std::path::Path::new(path).is_file() {
                return Err(CoreError::Automation(format!("APK not found: {path}")));
            }
            let message = adb.install(serial, path).await?;
            Ok(vec![format!("installed {path}: {message}")])
        }
        StepAction::Wait { ms } => {
            tokio::time::sleep(Duration::from_millis(*ms)).await;
            Ok(vec![format!("waited {ms} ms")])
        }
        StepAction::WaitForActivity { package, timeout_ms } => {
            let (adb, serial) = ctx.device()?;
            let deadline = Instant::now() + Duration::from_millis(*timeout_ms);
            let mut last = String::new();
            while Instant::now() < deadline {
                if let Ok(focus) = adb.current_focus(serial).await {
                    last = focus.clone();
                    if focus.contains(package) {
                        return Ok(vec![format!("{package} is in the foreground ({focus})")]);
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(CoreError::Automation(format!(
                "{package} did not reach the foreground within {}s (last focus: {})",
                timeout_ms / 1000,
                if last.is_empty() { "unknown".to_string() } else { last }
            )))
        }
        StepAction::Screenshot { label } => {
            let (adb, serial) = ctx.device()?;
            let bytes = adb.screencap(serial).await?;
            let path = ctx.save_png(&bytes, label)?;
            Ok(vec![format!("saved screenshot to {}", path.display())])
        }
        StepAction::WdNavigate { url } => {
            let (client, session) = ctx.session().await?;
            client.navigate(&session, url).await?;
            let current = client.current_url(&session).await.unwrap_or_else(|_| url.clone());
            Ok(vec![format!("browser at {current}")])
        }
        StepAction::WdClick { selector } => {
            let (client, session) = ctx.session().await?;
            client.click(&session, selector).await?;
            Ok(vec![format!("clicked {selector}")])
        }
        StepAction::WdType { selector, text } => {
            let (client, session) = ctx.session().await?;
            client.type_text(&session, selector, text).await?;
            Ok(vec![format!("typed into {selector}")])
        }
        StepAction::WdWaitFor { selector, timeout_ms } => {
            let (client, session) = ctx.session().await?;
            client
                .wait_for(&session, selector, Duration::from_millis(*timeout_ms))
                .await?;
            Ok(vec![format!("{selector} appeared")])
        }
        StepAction::WdAssertText { selector, contains } => {
            let (client, session) = ctx.session().await?;
            let text = client.element_text(&session, selector).await?;
            if !text.contains(contains) {
                return Err(CoreError::Automation(format!(
                    "expected {selector} to contain {contains:?}, but it read {:?}",
                    crate::http::truncate(text.trim(), 160)
                )));
            }
            Ok(vec![format!("{selector} contains {contains:?}")])
        }
        StepAction::WdScreenshot { label } => {
            let (client, session) = ctx.session().await?;
            let bytes = client.screenshot(&session).await?;
            let path = ctx.save_png(&bytes, label)?;
            Ok(vec![format!("saved browser screenshot to {}", path.display())])
        }
        StepAction::HttpRequest { method, url, headers, body, expect_status } => {
            let verb = reqwest::Method::from_bytes(method.to_ascii_uppercase().as_bytes())
                .map_err(|_| CoreError::Automation(format!("unsupported HTTP method {method}")))?;
            let mut request = ctx.http.request(verb.clone(), url);
            for (key, value) in headers {
                request = request.header(key, value);
            }
            if let Some(payload) = body {
                request = request.body(payload.clone());
            }
            let response = request
                .send()
                .await
                .map_err(|e| CoreError::Automation(format!("{verb} {url} failed: {e}")))?;
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            let mut lines = vec![format!("{verb} {url} → {status}")];
            for line in text.lines().take(20) {
                lines.push(format!("  {line}"));
            }
            if let Some(expected) = expect_status {
                if status != *expected {
                    return Err(CoreError::Automation(format!(
                        "expected HTTP {expected}, got {status}"
                    )));
                }
            } else if status >= 400 {
                return Err(CoreError::Automation(format!("HTTP request returned {status}")));
            }
            Ok(lines)
        }
        StepAction::Notify { message } => Ok(vec![format!("note: {message}")]),
    }
}

/// Encodes a PNG for inline display in the UI.
pub fn png_data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_url_is_well_formed() {
        let url = png_data_url(&[1, 2, 3]);
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(url.ends_with("AQID"));
    }
}

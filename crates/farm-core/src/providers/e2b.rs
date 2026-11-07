use super::{array_from, pick_dt, pick_f64, pick_str, pick_u64, ProviderClient};
use crate::error::{CoreError, Result};
use crate::http::{ApiClient, Auth};
use crate::model::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

/// E2B platform API (https://api.e2b.app). Auth: `X-API-Key`.
/// Docs: https://docs.e2b.dev/api-reference
pub struct E2bClient {
    api: ApiClient,
}

impl E2bClient {
    pub fn new(api_key: &str, base_url: Option<String>) -> Result<Self> {
        let base = base_url.unwrap_or_else(|| Provider::E2b.default_base_url().to_string());
        Ok(Self {
            api: ApiClient::new(
                Provider::E2b,
                base,
                Auth::Header("X-API-Key", api_key.trim().to_string()),
            )?,
        })
    }

    fn to_instance(&self, value: &Value) -> Option<RemoteInstance> {
        let id = pick_str(value, &["sandboxID", "sandboxId", "id"])?;
        let raw_state = pick_str(value, &["state", "status"]).unwrap_or_else(|| "running".to_string());
        let memory_mb = pick_u64(value, &["memoryMB", "memoryMb"]);
        let disk_mb = pick_u64(value, &["diskSizeMB", "diskSizeMb"]);
        let name = value
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let template = pick_str(value, &["templateID", "templateId", "alias"]);
        Some(RemoteInstance {
            id,
            provider: Provider::E2b,
            name,
            state: InstanceState::parse(&raw_state),
            raw_state,
            template,
            cpu: pick_u64(value, &["cpuCount"]).map(|v| v as u32),
            memory_mb,
            disk_gb: disk_mb.map(|mb| (mb as f64) / 1024.0),
            region: None,
            endpoint: pick_str(value, &["domain", "envdHost"]),
            created_at: pick_dt(value, &["startedAt", "createdAt"]),
            detail: None,
        })
    }
}

#[async_trait]
impl ProviderClient for E2bClient {
    fn provider(&self) -> Provider {
        Provider::E2b
    }

    async fn verify(&self) -> Result<AccountInfo> {
        let value = self.api.request(reqwest::Method::GET, "/v2/sandboxes?limit=1", None).await?;
        let count = array_from(&value, &["sandboxes"]).len();
        Ok(AccountInfo {
            account: "E2B API key accepted".to_string(),
            plan: None,
            detail: Some(format!(
                "{count} sandbox{} visible with this key",
                if count == 1 { "" } else { "s" }
            )),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteInstance>> {
        let value = self
            .api
            .request(reqwest::Method::GET, "/v2/sandboxes?limit=100", None)
            .await?;
        Ok(array_from(&value, &["sandboxes"])
            .into_iter()
            .filter_map(|v| self.to_instance(v))
            .collect())
    }

    async fn get(&self, id: &str) -> Result<RemoteInstance> {
        let value = self
            .api
            .request(reqwest::Method::GET, &format!("/sandboxes/{id}"), None)
            .await?;
        self.to_instance(&value)
            .ok_or_else(|| CoreError::Api {
                provider: "E2B".into(),
                status: 200,
                message: "sandbox response did not contain a sandboxID".into(),
            })
    }

    async fn create(&self, req: &CreateInstanceRequest) -> Result<RemoteInstance> {
        let template = req
            .template
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "base".to_string());
        let timeout = req.ttl_seconds.unwrap_or(1800).clamp(60, 86_400);
        let mut metadata = serde_json::Map::new();
        if let Some(name) = req.sanitized_name(Provider::E2b) {
            metadata.insert("name".into(), Value::String(name));
        }
        let env: serde_json::Map<String, Value> = req
            .env
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        let body = json!({
            "templateID": template,
            "timeout": timeout,
            "metadata": metadata,
            "envVars": env,
            "autoPause": false,
        });
        let value = self.api.request(reqwest::Method::POST, "/sandboxes", Some(body)).await?;
        let id = pick_str(&value, &["sandboxID", "sandboxId", "id"]).ok_or_else(|| CoreError::Api {
            provider: "E2B".into(),
            status: 201,
            message: "E2B accepted the request but returned no sandboxID".into(),
        })?;
        let mut instance = self.to_instance(&value).unwrap_or(RemoteInstance {
            id: id.clone(),
            provider: Provider::E2b,
            name: req.sanitized_name(Provider::E2b),
            state: InstanceState::Starting,
            raw_state: "starting".into(),
            template: Some(template),
            cpu: None,
            memory_mb: None,
            disk_gb: None,
            region: None,
            endpoint: None,
            created_at: Some(chrono::Utc::now()),
            detail: None,
        });
        instance.state = InstanceState::Running;
        instance.raw_state = "running".into();
        Ok(instance)
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::POST, &format!("/sandboxes/{id}/resume"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn stop(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::POST, &format!("/sandboxes/{id}/pause"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn destroy(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::DELETE, &format!("/sandboxes/{id}"), None)
            .await?;
        Ok(())
    }

    async fn metrics(&self, id: &str) -> Result<Option<InstanceMetrics>> {
        let value = self
            .api
            .request(
                reqwest::Method::GET,
                &format!("/sandboxes/metrics?sandbox_ids={id}"),
                None,
            )
            .await?;
        // The endpoint answers either an array of samples or a map of sandbox ->
        // samples depending on API version; take the newest sample either way.
        let samples = if let Some(array) = value.as_array() {
            array.clone()
        } else if let Some(obj) = value.as_object() {
            obj.get(id)
                .and_then(|v| v.as_array())
                .cloned()
                .or_else(|| {
                    obj.values()
                        .find_map(|v| if v.is_array() { v.as_array().cloned() } else { None })
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let Some(sample) = samples.last() else { return Ok(None) };
        Ok(Some(InstanceMetrics {
            cpu_percent: pick_f64(sample, &["cpuUsedPct", "cpuUsedPercent"]),
            memory_used_mb: pick_f64(sample, &["memUsed", "memUsedMb"]).map(bytes_to_mb),
            memory_total_mb: pick_f64(sample, &["memTotal", "memTotalMb"]).map(bytes_to_mb),
            disk_used_mb: pick_f64(sample, &["diskUsed"]).map(bytes_to_mb),
            disk_total_mb: pick_f64(sample, &["diskTotal"]).map(bytes_to_mb),
            sampled_at: chrono::Utc::now(),
            note: None,
        }))
    }
}

fn bytes_to_mb(value: f64) -> f64 {
    // E2B reports memory and disk in bytes.
    (value / (1024.0 * 1024.0) * 10.0).round() / 10.0
}

#[allow(dead_code)]
fn timeout_from(value: &Value) -> Duration {
    Duration::from_secs(pick_u64(value, &["timeout"]).unwrap_or(300))
}

use super::{array_from, pick_dt, pick_f64, pick_str, pick_u64, ProviderClient};
use crate::error::{CoreError, Result};
use crate::http::{ApiClient, Auth};
use crate::model::*;
use async_trait::async_trait;
use serde_json::{json, Value};

/// Daytona API (https://app.daytona.io/api). Auth: `Authorization: Bearer`.
/// Docs: https://www.daytona.io/docs/en/tools/api
pub struct DaytonaClient {
    api: ApiClient,
}

impl DaytonaClient {
    pub fn new(api_key: &str, base_url: Option<String>) -> Result<Self> {
        let base = base_url.unwrap_or_else(|| Provider::Daytona.default_base_url().to_string());
        Ok(Self {
            api: ApiClient::new(
                Provider::Daytona,
                base,
                Auth::Bearer(api_key.trim().to_string()),
            )?,
        })
    }

    fn to_instance(&self, value: &Value) -> Option<RemoteInstance> {
        let id = pick_str(value, &["id", "sandboxId", "name"])?;
        let raw_state = pick_str(value, &["state", "status"]).unwrap_or_else(|| "unknown".into());
        let snapshot = pick_str(value, &["snapshot", "image"]);
        Some(RemoteInstance {
            id,
            provider: Provider::Daytona,
            name: pick_str(value, &["name"]),
            state: InstanceState::parse(&raw_state),
            raw_state,
            template: snapshot,
            cpu: pick_u64(value, &["cpu"]).map(|v| v as u32),
            // Daytona expresses memory in GiB.
            memory_mb: pick_f64(value, &["memory"]).map(|g| (g * 1024.0).round() as u64),
            disk_gb: pick_f64(value, &["disk"]),
            region: pick_str(value, &["target"]),
            endpoint: pick_str(value, &["toolboxProxyUrl"]),
            created_at: pick_dt(value, &["createdAt", "created_at"]),
            detail: pick_str(value, &["errorReason"]),
        })
    }
}

#[async_trait]
impl ProviderClient for DaytonaClient {
    fn provider(&self) -> Provider {
        Provider::Daytona
    }

    async fn verify(&self) -> Result<AccountInfo> {
        let value = self
            .api
            .request(reqwest::Method::GET, "/sandbox?limit=1", None)
            .await?;
        let total = pick_u64(&value, &["total"]).unwrap_or_default();
        let key = self.api.request(reqwest::Method::GET, "/api-keys/current", None).await.ok();
        let account = key
            .as_ref()
            .and_then(|v| pick_str(v, &["name", "userId", "user"]))
            .unwrap_or_else(|| "Daytona API key accepted".to_string());
        Ok(AccountInfo {
            account,
            plan: None,
            detail: Some(format!("{total} sandbox(es) in this organisation")),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteInstance>> {
        let value = self
            .api
            .request(reqwest::Method::GET, "/sandbox?limit=100", None)
            .await?;
        Ok(array_from(&value, &["items", "sandboxes"])
            .into_iter()
            .filter_map(|v| self.to_instance(v))
            .collect())
    }

    async fn get(&self, id: &str) -> Result<RemoteInstance> {
        let value = self
            .api
            .request(reqwest::Method::GET, &format!("/sandbox/{id}"), None)
            .await?;
        self.to_instance(&value).ok_or_else(|| CoreError::Api {
            provider: "Daytona".into(),
            status: 200,
            message: "sandbox response did not contain an id".into(),
        })
    }

    async fn create(&self, req: &CreateInstanceRequest) -> Result<RemoteInstance> {
        let mut body = json!({
            "autoStopInterval": 30,
            "labels": { "managed-by": "bt-farm" },
        });
        if let Some(name) = req.sanitized_name(Provider::Daytona) {
            body["name"] = Value::String(name);
        }
        let gpu_units = req.gpu.unwrap_or(0);
        if let Some(template) = req.template.as_ref().filter(|t| !t.trim().is_empty()) {
            // GPU sandboxes are built from an image, plain ones from a snapshot.
            if gpu_units > 0 {
                body["image"] = Value::String(template.clone());
            } else {
                body["snapshot"] = Value::String(template.clone());
            }
        }
        if gpu_units > 0 {
            body["gpu"] = json!(gpu_units);
            if let Some(kind) = req.gpu_type.as_ref().filter(|t| !t.trim().is_empty()) {
                body["gpuType"] = json!([kind.trim()]);
            }
        }
        if let Some(cpu) = req.cpu {
            body["cpu"] = json!(cpu);
        }
        if let Some(memory) = req.memory_mb {
            body["memory"] = json!((memory as f64 / 1024.0).round() as u64);
        }
        if let Some(disk) = req.disk_gb {
            body["disk"] = json!(disk);
        }
        if !req.env.is_empty() {
            body["env"] = json!(req.env);
        }
        let path = if gpu_units > 0 { "/sandboxes/gpu" } else { "/sandbox" };
        let value = self.api.request(reqwest::Method::POST, path, Some(body)).await?;
        let mut instance = self.to_instance(&value).ok_or_else(|| CoreError::Api {
            provider: "Daytona".into(),
            status: 201,
            message: "Daytona accepted the request but returned no sandbox id".into(),
        })?;
        if instance.state == InstanceState::Unknown {
            instance.state = InstanceState::Starting;
        }
        Ok(instance)
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::POST, &format!("/sandbox/{id}/start"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn stop(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::POST, &format!("/sandbox/{id}/stop"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn destroy(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::DELETE, &format!("/sandbox/{id}"), None)
            .await?;
        Ok(())
    }
}

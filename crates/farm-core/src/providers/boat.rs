use super::{array_from, pick_dt, pick_str, ProviderClient};
use crate::error::{CoreError, Result};
use crate::http::{ApiClient, Auth};
use crate::model::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

/// Boat public API v1 (https://boat.dev/api/v1). Auth: `Authorization: Bearer`.
/// Docs: https://docs.boat.dev/api/v1
pub struct BoatClient {
    api: ApiClient,
}

impl BoatClient {
    pub fn new(api_key: &str, base_url: Option<String>) -> Result<Self> {
        let base = base_url.unwrap_or_else(|| Provider::Boat.default_base_url().to_string());
        Ok(Self {
            api: ApiClient::new(Provider::Boat, base, Auth::Bearer(api_key.trim().to_string()))?,
        })
    }

    fn to_instance(&self, value: &Value) -> Option<RemoteInstance> {
        let sandbox = value.get("sandbox").unwrap_or(value);
        let id = pick_str(sandbox, &["id"])?;
        let raw_state = pick_str(sandbox, &["state", "status"]).unwrap_or_else(|| "unknown".into());
        let size = pick_str(sandbox, &["type"]);
        let (cpu, memory_mb) = match size.as_deref() {
            Some("small") => (Some(2), Some(4096)),
            Some("large") => (Some(8), Some(16_384)),
            Some("xlarge") => (Some(16), Some(32_768)),
            Some("default") => (Some(4), Some(8192)),
            _ => (None, None),
        };
        let name = pick_str(sandbox, &["name"]).or_else(|| {
            sandbox
                .get("labels")
                .and_then(|labels| labels.get("name"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
        Some(RemoteInstance {
            id,
            provider: Provider::Boat,
            name,
            state: InstanceState::parse(&raw_state),
            raw_state,
            template: size,
            cpu,
            memory_mb,
            disk_gb: None,
            region: None,
            endpoint: pick_str(sandbox, &["url", "httpsUrl", "domain"]),
            created_at: pick_dt(sandbox, &["createdAt", "created_at"]),
            detail: pick_str(sandbox, &["error"]),
        })
    }
}

#[async_trait]
impl ProviderClient for BoatClient {
    fn provider(&self) -> Provider {
        Provider::Boat
    }

    async fn verify(&self) -> Result<AccountInfo> {
        let value = self.api.request(reqwest::Method::GET, "/me", None).await?;
        let user = value.get("user").unwrap_or(&value);
        let account = pick_str(user, &["email", "name", "login"])
            .or_else(|| pick_str(&value, &["org", "organization"]))
            .unwrap_or_else(|| "Boat authentication accepted".to_string());
        let limits = self.api.request(reqwest::Method::GET, "/limits", None).await.ok();
        let plan = limits
            .as_ref()
            .and_then(|v| pick_str(v, &["plan", "planName"]));
        Ok(AccountInfo {
            account,
            plan,
            detail: Some("Boat API key accepted".into()),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteInstance>> {
        let value = self
            .api
            .request(reqwest::Method::GET, "/sandboxes?limit=100", None)
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
        self.to_instance(&value).ok_or_else(|| CoreError::Api {
            provider: "Boat".into(),
            status: 200,
            message: "sandbox response did not contain an id".into(),
        })
    }

    async fn create(&self, req: &CreateInstanceRequest) -> Result<RemoteInstance> {
        let size = match req.size.as_deref() {
            Some("small") | Some("large") | Some("xlarge") | Some("default") => {
                req.size.clone().unwrap()
            }
            _ => {
                let cpu = req.cpu.unwrap_or(4);
                match cpu {
                    0..=2 => "small".to_string(),
                    3..=4 => "default".to_string(),
                    5..=8 => "large".to_string(),
                    _ => "xlarge".to_string(),
                }
            }
        };
        let mut labels = serde_json::Map::new();
        if let Some(name) = req.sanitized_name(Provider::Boat) {
            labels.insert("name".into(), Value::String(name));
        }
        let env: serde_json::Map<String, Value> = req
            .env
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        // noEnv keeps account-level secrets out of user sandboxes.
        let body = json!({
            "type": size,
            "ttlSeconds": req.ttl_seconds.unwrap_or(7200),
            "noEnv": true,
            "labels": labels,
            "env": env,
        });
        let value = self.api.request(reqwest::Method::POST, "/sandboxes", Some(body)).await?;
        let mut instance = self.to_instance(&value).ok_or_else(|| CoreError::Api {
            provider: "Boat".into(),
            status: 201,
            message: "Boat accepted the request but returned no sandbox id".into(),
        })?;
        instance.state = InstanceState::Starting;
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
            .request(reqwest::Method::POST, &format!("/sandboxes/{id}/stop"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn destroy(&self, id: &str) -> Result<()> {
        // Boat requires the sandbox id echoed back in this header before it will
        // accept an irreversible delete. The response is a 202 operation.
        self.api
            .request_with_headers(
                reqwest::Method::DELETE,
                &format!("/sandboxes/{id}"),
                None,
                &[("X-Ascii-Confirm-Delete", id)],
            )
            .await?;
        Ok(())
    }

    async fn exec(&self, id: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let seconds = timeout.as_secs().clamp(1, 600);
        let body = json!({
            "command": command,
            "timeoutSeconds": seconds,
        });
        let value = self
            .api
            .request(reqwest::Method::POST, &format!("/sandboxes/{id}/commands"), Some(body))
            .await?;
        let exit_code = value
            .get("exitCode")
            .or_else(|| value.get("exit_code"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let stdout = pick_str(&value, &["stdout", "output"]).unwrap_or_default();
        let stderr = pick_str(&value, &["stderr"]).unwrap_or_default();
        Ok(ExecOutput {
            stdout,
            stderr,
            exit_code,
            duration_ms: 0,
        })
    }
}

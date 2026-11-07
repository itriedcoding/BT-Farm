use super::{array_from, pick_dt, pick_f64, pick_str, pick_u64, ProviderClient};
use crate::error::{CoreError, Result};
use crate::http::{ApiClient, Auth};
use crate::model::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

/// Freestyle VMs API v5 (https://api.freestyle.sh). Auth: `Authorization: Bearer`.
/// Docs: https://www.freestyle.sh/docs/vms
pub struct FreestyleClient {
    api: ApiClient,
}

impl FreestyleClient {
    pub fn new(api_key: &str, base_url: Option<String>) -> Result<Self> {
        let base = base_url.unwrap_or_else(|| Provider::Freestyle.default_base_url().to_string());
        Ok(Self {
            api: ApiClient::new(
                Provider::Freestyle,
                base,
                Auth::Bearer(api_key.trim().to_string()),
            )?,
        })
    }

    fn to_instance(&self, value: &Value) -> Option<RemoteInstance> {
        let vm = if value.get("vm").is_some() { value.get("vm")? } else { value };
        let id = pick_str(vm, &["id", "vmId"])?;
        let raw_state = pick_str(vm, &["state", "status"]).unwrap_or_else(|| "unknown".into());
        let resources = vm.get("resources").cloned().unwrap_or(Value::Null);
        // Freestyle's VmResources is { cpu, memory (MiB), storage (MiB) }.
        let cpu = pick_u64(&resources, &["cpu", "vcpu", "cpus"]).map(|v| v as u32);
        let memory_mb = pick_u64(&resources, &["memory", "memoryMb"]);
        let disk_gb = pick_u64(&resources, &["storage", "diskMb"])
            .map(|mib| ((mib as f64 / 1024.0) * 10.0).round() / 10.0);
        let cpu_seconds = pick_f64(vm, &["cpuTimeSeconds"]);
        Some(RemoteInstance {
            id,
            provider: Provider::Freestyle,
            name: pick_str(vm, &["slug", "displayName", "name"]),
            state: InstanceState::parse(&raw_state),
            raw_state,
            template: pick_str(vm, &["snapshotId", "snapshot"]),
            cpu,
            memory_mb,
            disk_gb,
            region: pick_str(vm, &["region"]),
            endpoint: pick_str(vm, &["publicIpv6", "url", "publicUrl"]),
            created_at: pick_dt(vm, &["createdAt", "created_at"]),
            detail: cpu_seconds.map(|s| format!("{s:.0}s cumulative CPU time")),
        })
    }
}

#[async_trait]
impl ProviderClient for FreestyleClient {
    fn provider(&self) -> Provider {
        Provider::Freestyle
    }

    async fn verify(&self) -> Result<AccountInfo> {
        let value = self.api.request(reqwest::Method::GET, "/v5/vms", None).await?;
        let total = pick_u64(&value, &["totalCount"])
            .unwrap_or_else(|| array_from(&value, &["vms"]).len() as u64);
        Ok(AccountInfo {
            account: "Freestyle API key accepted".to_string(),
            plan: None,
            detail: Some(format!("{total} VM(s) in this account")),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteInstance>> {
        let value = self.api.request(reqwest::Method::GET, "/v5/vms", None).await?;
        Ok(array_from(&value, &["vms"])
            .into_iter()
            .filter_map(|v| self.to_instance(v))
            .collect())
    }

    async fn get(&self, id: &str) -> Result<RemoteInstance> {
        let value = self
            .api
            .request(reqwest::Method::GET, &format!("/v5/vms/{id}"), None)
            .await?;
        self.to_instance(&value).ok_or_else(|| CoreError::Api {
            provider: "Freestyle".into(),
            status: 200,
            message: "VM response did not contain an id".into(),
        })
    }

    async fn create(&self, req: &CreateInstanceRequest) -> Result<RemoteInstance> {
        // Freestyle requires an explicit firewall; allow outbound only, which is
        // what an automation worker needs.
        let mut body = json!({
            "firewall": {
                "rules": [
                    { "action": "allow", "source": {}, "destination": { "public": true } }
                ]
            }
        });
        if let Some(name) = req.sanitized_name(Provider::Freestyle) {
            body["slug"] = Value::String(name);
            body["displayName"] = Value::String(req.name.clone().unwrap_or_default());
        }
        if let Some(template) = req.template.as_ref().filter(|t| !t.trim().is_empty()) {
            body["snapshotId"] = Value::String(template.clone());
        }
        if let Some(ttl) = req.ttl_seconds {
            body["ttlSeconds"] = json!(ttl);
        }
        let value = self.api.request(reqwest::Method::POST, "/v5/vms", Some(body)).await?;
        let id = pick_str(&value, &["vmId", "id"])
            .or_else(|| {
                value
                    .get("vm")
                    .and_then(|v| pick_str(v, &["id"]))
            })
            .ok_or_else(|| CoreError::Api {
                provider: "Freestyle".into(),
                status: 201,
                message: "Freestyle accepted the request but returned no VM id".into(),
            })?;
        let mut instance = self.to_instance(&value).unwrap_or(RemoteInstance {
            id,
            provider: Provider::Freestyle,
            name: req.sanitized_name(Provider::Freestyle),
            state: InstanceState::Starting,
            raw_state: "starting".into(),
            template: req.template.clone(),
            cpu: req.cpu,
            memory_mb: req.memory_mb,
            disk_gb: req.disk_gb.map(|g| g as f64),
            region: None,
            endpoint: None,
            created_at: Some(chrono::Utc::now()),
            detail: None,
        });
        if instance.state == InstanceState::Unknown {
            instance.state = InstanceState::Starting;
        }
        Ok(instance)
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::POST, &format!("/v5/vms/{id}/start"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn stop(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::POST, &format!("/v5/vms/{id}/pause"), Some(json!({})))
            .await?;
        Ok(())
    }

    async fn destroy(&self, id: &str) -> Result<()> {
        self.api
            .request(reqwest::Method::DELETE, &format!("/v5/vms/{id}"), None)
            .await?;
        Ok(())
    }

    async fn exec(&self, id: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let body = json!({
            "command": command,
            "timeoutMs": timeout.as_millis().min(600_000) as u64,
        });
        let value = self
            .api
            .request(
                reqwest::Method::POST,
                &format!("/v5/vms/{id}/exec-await"),
                Some(body),
            )
            .await?;
        let exit_code = value
            .get("statusCode")
            .or_else(|| value.get("exitCode"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        Ok(ExecOutput {
            stdout: pick_str(&value, &["stdout", "output"]).unwrap_or_default(),
            stderr: pick_str(&value, &["stderr"]).unwrap_or_default(),
            exit_code,
            duration_ms: 0,
        })
    }
}

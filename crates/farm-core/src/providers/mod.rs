use crate::error::{CoreError, Result};
use crate::model::*;
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

pub mod boat;
pub mod boxd;
pub mod daytona;
pub mod e2b;
pub mod freestyle;

/// Uniform surface every infrastructure backend implements. Methods a provider
/// genuinely cannot support return `CoreError::Unsupported` with an explanation
/// instead of silently pretending.
#[async_trait]
pub trait ProviderClient: Send + Sync {
    fn provider(&self) -> Provider;

    fn capabilities(&self) -> Capabilities {
        self.provider().capabilities()
    }

    async fn verify(&self) -> Result<AccountInfo>;

    async fn list(&self) -> Result<Vec<RemoteInstance>>;

    async fn get(&self, id: &str) -> Result<RemoteInstance>;

    async fn create(&self, req: &CreateInstanceRequest) -> Result<RemoteInstance>;

    async fn start(&self, _id: &str) -> Result<()> {
        Err(not_supported(self.provider(), "starting instances"))
    }

    async fn stop(&self, _id: &str) -> Result<()> {
        Err(not_supported(self.provider(), "stopping instances"))
    }

    async fn destroy(&self, _id: &str) -> Result<()> {
        Err(not_supported(self.provider(), "deleting instances"))
    }

    async fn exec(&self, id: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let _ = (id, command, timeout);
        Err(not_supported(self.provider(), "running commands inside instances"))
    }

    async fn metrics(&self, id: &str) -> Result<Option<InstanceMetrics>> {
        let _ = id;
        Ok(None)
    }
}

pub fn not_supported(provider: Provider, capability: &str) -> CoreError {
    let caps = provider.capabilities();
    CoreError::unsupported(provider.label(), capability, caps.note)
}

pub fn require_credentials<'a>(provider: Provider, api_key: Option<&'a String>) -> Result<&'a str> {
    match api_key {
        Some(key) if !key.trim().is_empty() => Ok(key.trim()),
        _ => Err(CoreError::NotConfigured {
            provider: provider.label().to_string(),
            message: format!("add an API key for {} in Settings", provider.label()),
        }),
    }
}

/// Reads the first present key from a JSON object.
pub fn pick_str(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = value.get(*key) {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

pub fn pick_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(v) = value.get(*key) {
            if let Some(n) = v.as_u64() {
                return Some(n);
            }
            if let Some(n) = v.as_f64() {
                if n >= 0.0 {
                    return Some(n as u64);
                }
            }
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.parse::<u64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

pub fn pick_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(v) = value.get(*key) {
            if let Some(n) = v.as_f64() {
                return Some(n);
            }
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

pub fn pick_dt(value: &Value, keys: &[&str]) -> Option<chrono::DateTime<chrono::Utc>> {
    for key in keys {
        if let Some(s) = value.get(*key).and_then(|v| v.as_str()) {
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(s) {
                return Some(parsed.with_timezone(&chrono::Utc));
            }
        }
    }
    None
}

/// Many provider list endpoints return either a bare array or `{ items: [...] }`.
pub fn array_from<'a>(value: &'a Value, preferred_keys: &[&str]) -> Vec<&'a Value> {
    if let Some(items) = value.as_array() {
        return items.iter().collect();
    }
    for key in preferred_keys {
        if let Some(items) = value.get(*key).and_then(|v| v.as_array()) {
            return items.iter().collect();
        }
    }
    // Last resort: the first array-valued field.
    if let Some(obj) = value.as_object() {
        for (_, v) in obj {
            if let Some(items) = v.as_array() {
                return items.iter().collect();
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_values_tolerantly() {
        let v = json!({"name":"vm-1","cpu":4,"createdAt":"2026-01-01T00:00:00Z"});
        assert_eq!(pick_str(&v, &["id", "name"]).as_deref(), Some("vm-1"));
        assert_eq!(pick_u64(&v, &["vcpu", "cpu"]), Some(4));
        assert!(pick_dt(&v, &["createdAt"]).is_some());
    }

    #[test]
    fn arrays_from_various_envelopes() {
        assert_eq!(array_from(&json!([1, 2]), &[]).len(), 2);
        assert_eq!(array_from(&json!({"vms": [1, 2, 3]}), &["vms"]).len(), 3);
        assert_eq!(array_from(&json!({"items": [1]}), &["data"]).len(), 1);
        assert!(array_from(&json!({"total": 0}), &["items"]).is_empty());
    }
}

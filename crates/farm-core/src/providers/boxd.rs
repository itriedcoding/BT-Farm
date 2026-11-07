use super::{array_from, pick_dt, pick_str, pick_u64, ProviderClient};
use crate::error::{CoreError, Result};
use crate::model::*;
use crate::process;
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

/// boxd does not publish a stable third-party HTTP API: its console API rejects
/// clients it does not ship, and the documented automation surface is the
/// official `boxd` CLI with `--json` plus `BOXD_TOKEN`.
/// Docs: https://docs.boxd.sh/cli/commands
///
/// This adapter drives that CLI. If the CLI is missing the app says so, it never
/// fabricates a machine list.
pub struct BoxdClient {
    binary: Option<String>,
    token: Option<String>,
    base_url: Option<String>,
}

impl BoxdClient {
    pub fn new(binary: Option<String>, token: Option<String>, base_url: Option<String>) -> Self {
        Self {
            binary,
            token,
            base_url,
        }
    }

    fn binary(&self) -> Result<String> {
        self.binary.clone().ok_or_else(|| CoreError::NotConfigured {
            provider: "boxd".into(),
            message: "install the boxd CLI (https://boxd.sh/downloads/install.ps1) and set its path in Settings, or put `boxd` on PATH".into(),
        })
    }

    fn envs(&self) -> Vec<(String, String)> {
        let mut envs = Vec::new();
        if let Some(token) = &self.token {
            if !token.trim().is_empty() {
                envs.push(("BOXD_TOKEN".to_string(), token.trim().to_string()));
            }
        }
        if let Some(base) = &self.base_url {
            if !base.trim().is_empty() {
                envs.push(("BOXD_API_URL".to_string(), base.trim().to_string()));
            }
        }
        envs
    }

    async fn cli(&self, args: &[&str], timeout: Duration) -> Result<process::ProcessOutput> {
        let binary = self.binary()?;
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let output = process::run(&binary, &owned, &self.envs(), timeout).await?;
        if output.timed_out {
            return Err(CoreError::Transport {
                provider: "boxd".into(),
                message: format!("boxd {} timed out after {}s", args.join(" "), timeout.as_secs()),
            });
        }
        if !output.success() {
            let message = output.combined();
            let hint = if self.token.is_none() {
                " (no API key is configured, so the CLI cannot authenticate)"
            } else {
                ""
            };
            return Err(CoreError::Api {
                provider: "boxd".into(),
                status: output.code.max(0) as u16,
                message: format!("{}{hint}", crate::http::redact(&message.trim().to_string())),
            });
        }
        Ok(output)
    }

    fn parse_json(output: &process::ProcessOutput) -> Result<Value> {
        let stdout = output.stdout.trim();
        serde_json::from_str(stdout).map_err(|e| CoreError::Api {
            provider: "boxd".into(),
            status: 200,
            message: format!(
                "boxd CLI did not return JSON ({e}); first 200 bytes: {}",
                crate::http::truncate(stdout, 200)
            ),
        })
    }

    fn to_instance(&self, value: &Value) -> Option<RemoteInstance> {
        let id = pick_str(value, &["name", "id", "machine"])?;
        let raw_state = pick_str(value, &["state", "status", "power_state"])
            .unwrap_or_else(|| "unknown".into());
        Some(RemoteInstance {
            id: id.clone(),
            provider: Provider::Boxd,
            name: pick_str(value, &["displayName", "name", "id"]),
            state: InstanceState::parse(&raw_state),
            raw_state,
            template: pick_str(value, &["snapshot", "image"]),
            cpu: pick_u64(value, &["vcpu", "cpu", "cpus"]).map(|v| v as u32),
            memory_mb: pick_u64(value, &["memoryMiB", "memoryMb", "memory"]).map(|v| {
                if v < 512 { v * 1024 } else { v }
            }),
            disk_gb: pick_u64(value, &["diskGb", "disk"]).map(|v| v as f64),
            region: pick_str(value, &["region"]),
            endpoint: pick_str(value, &["url", "httpsUrl"]),
            created_at: pick_dt(value, &["createdAt", "created_at"]),
            detail: None,
        })
    }

    async fn machines(&self) -> Result<Vec<RemoteInstance>> {
        let output = self
            .cli(&["machine", "list", "--json"], Duration::from_secs(60))
            .await?;
        let value = Self::parse_json(&output)?;
        let list = if value.is_array() {
            array_from(&value, &[])
        } else {
            array_from(&value, &["machines", "items", "data"])
        };
        Ok(list.into_iter().filter_map(|v| self.to_instance(v)).collect())
    }
}

#[async_trait]
impl ProviderClient for BoxdClient {
    fn provider(&self) -> Provider {
        Provider::Boxd
    }

    async fn verify(&self) -> Result<AccountInfo> {
        let machines = self.machines().await?;
        Ok(AccountInfo {
            account: "boxd CLI authenticated".to_string(),
            plan: None,
            detail: Some(format!("{} machine(s) visible", machines.len())),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteInstance>> {
        self.machines().await
    }

    async fn get(&self, id: &str) -> Result<RemoteInstance> {
        let output = self
            .cli(&["machine", "get", id, "--json"], Duration::from_secs(45))
            .await?;
        let value = Self::parse_json(&output)?;
        self.to_instance(&value).ok_or_else(|| CoreError::Api {
            provider: "boxd".into(),
            status: 200,
            message: "boxd machine get returned an unexpected shape".into(),
        })
    }

    async fn create(&self, req: &CreateInstanceRequest) -> Result<RemoteInstance> {
        let name = req
            .sanitized_name(Provider::Boxd)
            .unwrap_or_else(|| format!("farm-{}", uuid::Uuid::new_v4().simple()));
        let mut args: Vec<String> = vec!["machine".into(), "new".into(), name.clone(), "--json".into()];
        if let Some(cpu) = req.cpu {
            let vcpu = match cpu {
                0..=1 => 1,
                2..=3 => 2,
                4..=7 => 4,
                _ => 8,
            };
            args.push("--vcpu".into());
            args.push(vcpu.to_string());
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.cli(&arg_refs, Duration::from_secs(180)).await?;
        let value = Self::parse_json(&output).unwrap_or(Value::Null);
        let mut instance = self.to_instance(&value).unwrap_or(RemoteInstance {
            id: name.clone(),
            provider: Provider::Boxd,
            name: Some(name),
            state: InstanceState::Creating,
            raw_state: "creating".into(),
            template: req.template.clone(),
            cpu: req.cpu,
            memory_mb: req.memory_mb,
            disk_gb: req.disk_gb.map(|v| v as f64),
            region: None,
            endpoint: None,
            created_at: Some(chrono::Utc::now()),
            detail: None,
        });
        if instance.state == InstanceState::Unknown {
            instance.state = InstanceState::Creating;
        }
        Ok(instance)
    }

    async fn start(&self, id: &str) -> Result<()> {
        // boxd machines hibernate on their own and wake on the next connection.
        self.cli(
            &["machine", "exec", id, "--json", "--", "true"],
            Duration::from_secs(90),
        )
        .await?;
        Ok(())
    }

    async fn stop(&self, id: &str) -> Result<()> {
        let _ = id;
        Err(CoreError::unsupported(
            "boxd",
            "explicit stop",
            "boxd machines hibernate on their idle timer (auto-hibernate) and wake on connection; the CLI exposes no explicit stop command.",
        ))
    }

    async fn destroy(&self, id: &str) -> Result<()> {
        self.cli(
            &["machine", "remove", id, "-y", "--json"],
            Duration::from_secs(120),
        )
        .await?;
        Ok(())
    }

    async fn exec(&self, id: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let seconds = timeout.as_secs().clamp(1, 600).to_string();
        let output = self
            .cli(
                &[
                    "machine", "exec", id, "--json", "--timeout", &seconds, "--", command,
                ],
                timeout + Duration::from_secs(15),
            )
            .await?;
        match Self::parse_json(&output) {
            Ok(value) => {
                let exit_code = value
                    .get("exit_code")
                    .or_else(|| value.get("exitCode"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                Ok(ExecOutput {
                    stdout: pick_str(&value, &["output", "stdout"]).unwrap_or_default(),
                    stderr: pick_str(&value, &["stderr"]).unwrap_or_default(),
                    exit_code,
                    duration_ms: output.duration_ms,
                })
            }
            Err(_) => Ok(ExecOutput {
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.code,
                duration_ms: output.duration_ms,
            }),
        }
    }
}

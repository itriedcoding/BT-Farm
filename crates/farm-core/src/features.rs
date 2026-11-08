//! Feature surface added on top of the core engine: device health, wireless
//! pairing, batch ADB operations, instance presets, workflow import/export,
//! recordings, diagnostics and the GitHub-release updater.
//!
//! This module is a child of the crate root, so it can reach `FarmCore`'s
//! private state without widening the public API. Everything here talks to real
//! systems: adb, the filesystem, SQLite and the GitHub API.

use crate::error::{CoreError, Result};
use crate::model::*;
use crate::model::{CustomTemplate, CustomTemplateDraft, TemplateProvision};
use uuid::Uuid;
use crate::templates::{self, TemplateView};
use crate::updates::{self, UpdateDownload, UpdateInfo};
use crate::FarmCore;
use chrono::Utc;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use tokio::task::JoinSet;

/// The installer downloaded by the last successful `download_update`, so the
/// shell can launch it without trusting a path that came from the frontend.
static DOWNLOADED_INSTALLER: Mutex<Option<PathBuf>> = Mutex::new(None);

impl FarmCore {
    // ------------------------------------------------------------------
    // Instance templates and sizing
    // ------------------------------------------------------------------

    /// The shipped catalog plus every template the operator saved, so the UI
    /// treats both kinds the same way.
    pub fn templates(&self) -> Result<Vec<TemplateView>> {
        let mut out: Vec<TemplateView> = templates::catalog().iter().map(|t| t.view()).collect();
        for custom in self.store.list_custom_templates()? {
            out.push(custom.view());
        }
        Ok(out)
    }

    pub fn save_custom_template(&self, draft: CustomTemplateDraft) -> Result<CustomTemplate> {
        let name = draft.name.trim();
        if name.is_empty() {
            return Err(CoreError::invalid("give the template a name"));
        }
        if draft.setup.len() > 8_000 {
            return Err(CoreError::invalid(
                "the setup command is limited to 8000 characters",
            ));
        }
        let id = if draft.id.trim().is_empty() {
            format!("custom-{}", Uuid::new_v4())
        } else {
            draft.id.trim().to_string()
        };
        if templates::find(&id).is_some() {
            return Err(CoreError::invalid(
                "that id belongs to a built-in template; pick another one",
            ));
        }
        let created_at = self
            .store
            .find_custom_template(&id)?
            .map(|existing| existing.created_at)
            .unwrap_or_else(Utc::now);
        let template = CustomTemplate {
            id,
            name: name.to_string(),
            description: draft.description.trim().to_string(),
            provider: draft.provider,
            workload: if draft.workload.trim().is_empty() {
                "Custom".to_string()
            } else {
                draft.workload.trim().to_string()
            },
            request: draft.request.normalized_for(draft.provider),
            setup: draft.setup.trim().to_string(),
            created_at,
        };
        self.store.save_custom_template(&template)?;
        self.record_log(
            "info",
            "templates",
            &format!("saved custom template '{}'", template.name),
        );
        Ok(template)
    }

    pub fn delete_custom_template(&self, id: &str) -> Result<()> {
        if templates::find(id).is_some() {
            return Err(CoreError::invalid("built-in templates cannot be deleted"));
        }
        self.store.delete_custom_template(id)?;
        self.record_log(
            "info",
            "templates",
            &format!("deleted custom template {id}"),
        );
        Ok(())
    }

    /// Provisions one instance from a template and, when the template carries a
    /// setup command and the provider can execute commands, runs it and returns
    /// its real output.
    pub async fn create_from_template(
        &self,
        template_id: &str,
        name: Option<String>,
    ) -> Result<TemplateProvision> {
        let (provider, request, setup) = if let Some(builtin) = templates::find(template_id) {
            (
                builtin.provider,
                builtin.to_request(name),
                templates::setup_script(template_id).map(str::to_string),
            )
        } else if let Some(custom) = self.store.find_custom_template(template_id)? {
            let mut request = custom.request.clone();
            if name.is_some() {
                request.name = name;
            }
            (
                custom.provider,
                request.normalized_for(custom.provider),
                Some(custom.setup).filter(|script| !script.trim().is_empty()),
            )
        } else {
            return Err(CoreError::invalid(format!(
                "no template called '{template_id}'"
            )));
        };

        let instance = self.create_instance(provider, request).await?;
        let mut provision = TemplateProvision {
            template_id: template_id.to_string(),
            instance,
            setup_ran: false,
            setup_exit_code: None,
            setup_output: String::new(),
            setup_note: "This template provisions the machine without a setup command.".into(),
        };

        let Some(script) = setup else {
            return Ok(provision);
        };
        let caps = provider.capabilities();
        if !caps.exec {
            provision.setup_note = format!(
                "{} cannot run commands from this app ({}), so the setup command was not run. Run it manually once the machine is reachable.",
                provider.label(),
                caps.terminal
            );
            self.record_log(
                "warn",
                provider.slug(),
                &format!("setup for {template_id} was not run: {}", caps.terminal),
            );
            return Ok(provision);
        }

        // The machine may still be booting, so retry the first command a few times.
        let mut last_error = None;
        for attempt in 1..=6_u32 {
            match self
                .instance_exec(provider, &provision.instance.id, &script, 600)
                .await
            {
                Ok(output) => {
                    provision.setup_ran = true;
                    provision.setup_exit_code = Some(output.exit_code);
                    provision.setup_output = output.combined();
                    provision.setup_note = if output.exit_code == 0 {
                        format!(
                            "Setup command finished with exit code 0 in {} ms.",
                            output.duration_ms
                        )
                    } else {
                        format!(
                            "Setup command exited {} after {} ms; the output above is the provider's real output.",
                            output.exit_code, output.duration_ms
                        )
                    };
                    self.record_log(
                        if output.exit_code == 0 { "info" } else { "warn" },
                        provider.slug(),
                        &format!("setup for {template_id} exited {}", output.exit_code),
                    );
                    return Ok(provision);
                }
                Err(err) => {
                    last_error = Some(err);
                    if attempt < 6 {
                        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                    }
                }
            }
        }
        let message = last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "unknown error".to_string());
        provision.setup_note = format!(
            "The instance was created, but the setup command could not be executed: {message}"
        );
        self.record_log("error", provider.slug(), &provision.setup_note);
        Ok(provision)
    }

    /// Runs an action on several instances, reporting each provider's answer.
    pub async fn instance_bulk(&self, targets: Vec<BulkTarget>, action: &str) -> Vec<BatchOutcome> {
        let mut outcomes = Vec::new();
        for target in targets {
            let label = format!("{}:{}", target.provider.slug(), target.id);
            match self
                .instance_action(target.provider, &target.id, action)
                .await
            {
                Ok(_) => outcomes.push(BatchOutcome {
                    serial: label,
                    ok: true,
                    output: format!("{action} accepted by {}", target.provider.label()),
                }),
                Err(err) => outcomes.push(BatchOutcome {
                    serial: label,
                    ok: false,
                    output: err.to_string(),
                }),
            }
        }
        let failed = outcomes.iter().filter(|outcome| !outcome.ok).count();
        if !outcomes.is_empty() {
            self.record_log(
                if failed == 0 { "info" } else { "warn" },
                "instances",
                &format!(
                    "bulk {action}: {} of {} instance(s) succeeded",
                    outcomes.len() - failed,
                    outcomes.len()
                ),
            );
        }
        outcomes
    }

    // ------------------------------------------------------------------
    // Local storage usage
    // ------------------------------------------------------------------

    fn storage_dirs(&self) -> Vec<(&'static str, &'static str, PathBuf)> {
        vec![
            ("screenshots", "Device screenshots", self.data_dir.join("screenshots")),
            ("recordings", "Screen recordings", self.recordings_dir()),
            ("logs", "Log files", crate::logging::log_dir(&self.data_dir)),
            ("updates", "Downloaded updates", self.data_dir.join("updates")),
            ("diagnostics", "Diagnostics reports", self.data_dir.join("diagnostics")),
        ]
    }

    pub fn storage_report(&self) -> Result<StorageReport> {
        let mut entries = Vec::new();
        let mut total = 0_u64;
        for (key, label, path) in self.storage_dirs() {
            let (files, bytes) = dir_usage(&path);
            total += bytes;
            entries.push(StorageEntry {
                key: key.to_string(),
                label: label.to_string(),
                path: path.to_string_lossy().to_string(),
                files,
                bytes,
            });
        }
        Ok(StorageReport {
            data_dir: self.data_dir.to_string_lossy().to_string(),
            total_bytes: total,
            entries,
        })
    }

    /// Deletes files older than `older_than_days` (0 = everything) from one of
    /// the app's own directories. Only known keys are accepted, so this cannot
    /// be pointed at an arbitrary path.
    pub fn clean_storage(&self, key: &str, older_than_days: u64) -> Result<StorageCleanup> {
        let Some((_, _, dir)) = self
            .storage_dirs()
            .into_iter()
            .find(|(candidate, _, _)| *candidate == key)
        else {
            return Err(CoreError::invalid(format!(
                "unknown storage area '{key}'"
            )));
        };
        let cutoff = Utc::now() - chrono::Duration::days(older_than_days as i64);
        let mut deleted_files = 0_usize;
        let mut freed_bytes = 0_u64;
        let mut stack = vec![dir];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                if meta.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !meta.is_file() {
                    continue;
                }
                let modified = meta
                    .modified()
                    .map(chrono::DateTime::<Utc>::from)
                    .unwrap_or_else(|_| Utc::now());
                if modified > cutoff {
                    continue;
                }
                if std::fs::remove_file(&path).is_ok() {
                    deleted_files += 1;
                    freed_bytes += meta.len();
                }
            }
        }
        self.record_log(
            "info",
            "storage",
            &format!(
                "cleaned {key}: {deleted_files} file(s), {freed_bytes} bytes freed (older than {older_than_days} day(s))"
            ),
        );
        Ok(StorageCleanup {
            key: key.to_string(),
            deleted_files,
            freed_bytes,
        })
    }

    // ------------------------------------------------------------------
    // Device health
    // ------------------------------------------------------------------

    pub async fn device_health(&self, serial: &str) -> Result<DeviceHealth> {
        let adb = self.adb()?;
        let commands = [
            "dumpsys battery",
            "df -k /data",
            "cat /proc/meminfo",
            "cat /proc/uptime",
            "wm size",
            "getprop ro.product.cpu.abi",
            "getprop ro.build.version.sdk",
        ];
        let mut set: JoinSet<(usize, Result<String>)> = JoinSet::new();
        for (index, command) in commands.iter().enumerate() {
            let adb = adb.clone();
            let serial = serial.to_string();
            let command = command.to_string();
            set.spawn(async move { (index, adb.shell_checked(&serial, &command).await) });
        }
        let mut outputs = vec![String::new(); commands.len()];
        let mut errors = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((index, Ok(text))) => outputs[index] = text,
                Ok((index, Err(err))) => errors.push(format!("{}: {err}", commands[index])),
                Err(err) => errors.push(err.to_string()),
            }
        }
        let mut health = DeviceHealth::from_outputs(
            serial,
            &outputs[0],
            &outputs[1],
            &outputs[2],
            &outputs[3],
            outputs[4].lines().last().unwrap_or_default(),
            &outputs[5],
            &outputs[6],
        );
        health.errors = errors;
        Ok(health)
    }

    // ------------------------------------------------------------------
    // Wireless debugging
    // ------------------------------------------------------------------

    pub async fn pair_device(&self, address: &str, code: &str) -> Result<String> {
        let address = address.trim();
        let code = code.trim();
        if !address.contains(':') {
            return Err(CoreError::invalid(
                "use the host:port shown on the device's wireless debugging screen",
            ));
        }
        if code.len() < 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(CoreError::invalid(
                "the pairing code is the 6-digit number shown on the device",
            ));
        }
        let adb = self.adb()?;
        let message = adb.pair(address, code).await?;
        self.record_log("info", "adb", &format!("paired with {address}: {message}"));
        Ok(message)
    }

    // ------------------------------------------------------------------
    // File transfer and batch operations
    // ------------------------------------------------------------------

    pub async fn device_push(&self, serial: &str, local: &str, remote: &str) -> Result<String> {
        if !Path::new(local).is_file() {
            return Err(CoreError::invalid(format!("no file at {local}")));
        }
        if remote.trim().is_empty() {
            return Err(CoreError::invalid("the destination path is required"));
        }
        let adb = self.adb()?;
        adb.push(serial, local, remote).await?;
        let detail = format!("pushed {local} → {remote}");
        self.record_log("info", "adb", &format!("{detail} on {serial}"));
        Ok(detail)
    }

    pub async fn device_pull(&self, serial: &str, remote: &str, local: &str) -> Result<String> {
        if remote.trim().is_empty() {
            return Err(CoreError::invalid("the device path is required"));
        }
        if local.trim().is_empty() {
            return Err(CoreError::invalid("the destination path is required"));
        }
        if let Some(parent) = Path::new(local).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let adb = self.adb()?;
        adb.pull(serial, remote, local).await?;
        let detail = format!("pulled {remote} → {local}");
        self.record_log("info", "adb", &format!("{detail} from {serial}"));
        Ok(detail)
    }

    pub async fn device_batch_shell(
        &self,
        serials: Vec<String>,
        command: String,
    ) -> Result<Vec<BatchOutcome>> {
        if serials.is_empty() {
            return Err(CoreError::invalid("select at least one device"));
        }
        if command.trim().is_empty() {
            return Err(CoreError::invalid("the shell command cannot be empty"));
        }
        let adb = self.adb()?;
        let mut set: JoinSet<BatchOutcome> = JoinSet::new();
        for serial in serials {
            let adb = adb.clone();
            let command = command.clone();
            set.spawn(async move {
                match adb.shell_result(&serial, &command).await {
                    Ok(output) => BatchOutcome {
                        serial,
                        ok: !output.timed_out && output.code == 0,
                        output: if output.timed_out {
                            format!("timed out after 60s\n{}", output.combined().trim())
                        } else {
                            output.combined().trim().to_string()
                        },
                    },
                    Err(err) => BatchOutcome {
                        serial,
                        ok: false,
                        output: err.to_string(),
                    },
                }
            });
        }
        let mut outcomes = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(outcome) => outcomes.push(outcome),
                Err(err) => outcomes.push(BatchOutcome {
                    serial: "unknown".into(),
                    ok: false,
                    output: err.to_string(),
                }),
            }
        }
        outcomes.sort_by(|a, b| a.serial.cmp(&b.serial));
        let failed = outcomes.iter().filter(|o| !o.ok).count();
        self.record_log(
            if failed == 0 { "info" } else { "warn" },
            "adb",
            &format!(
                "batch shell `{}` on {} device(s): {} succeeded, {failed} failed",
                command.trim(),
                outcomes.len(),
                outcomes.len() - failed
            ),
        );
        Ok(outcomes)
    }

    pub async fn device_batch_screenshot(
        &self,
        serials: Vec<String>,
    ) -> Result<Vec<BatchOutcome>> {
        if serials.is_empty() {
            return Err(CoreError::invalid("select at least one device"));
        }
        let adb = self.adb()?;
        let dir = self.data_dir.join("screenshots");
        std::fs::create_dir_all(&dir)?;
        let mut set: JoinSet<BatchOutcome> = JoinSet::new();
        for serial in serials {
            let adb = adb.clone();
            let dir = dir.clone();
            set.spawn(async move {
                match adb.screencap(&serial).await {
                    Ok(bytes) => {
                        let path = dir.join(format!(
                            "{}-{}.png",
                            serial.replace([':', '/', '\\'], "-"),
                            Utc::now().format("%Y%m%d-%H%M%S%.3f")
                        ));
                        match std::fs::write(&path, &bytes) {
                            Ok(()) => BatchOutcome {
                                serial,
                                ok: true,
                                output: path.to_string_lossy().to_string(),
                            },
                            Err(err) => BatchOutcome {
                                serial,
                                ok: false,
                                output: err.to_string(),
                            },
                        }
                    }
                    Err(err) => BatchOutcome {
                        serial,
                        ok: false,
                        output: err.to_string(),
                    },
                }
            });
        }
        let mut outcomes = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(outcome) => outcomes.push(outcome),
                Err(err) => outcomes.push(BatchOutcome {
                    serial: "unknown".into(),
                    ok: false,
                    output: err.to_string(),
                }),
            }
        }
        outcomes.sort_by(|a, b| a.serial.cmp(&b.serial));
        self.record_log(
            "info",
            "adb",
            &format!(
                "batch screenshot on {} device(s): {} captured",
                outcomes.len(),
                outcomes.iter().filter(|o| o.ok).count()
            ),
        );
        Ok(outcomes)
    }

    // ------------------------------------------------------------------
    // Logs and diagnostics
    // ------------------------------------------------------------------

    pub fn export_logs(
        &self,
        path: &str,
        limit: usize,
        level: Option<&str>,
        search: Option<&str>,
    ) -> Result<String> {
        let path = Path::new(path);
        if path.as_os_str().is_empty() {
            return Err(CoreError::invalid("choose where to save the log export"));
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let lines = self.store.list_logs(limit.clamp(1, 2000), level, search)?;
        let mut body = String::from("# BT Farm log export\n");
        body.push_str(&format!(
            "# created {} · {} line(s){}{}\n",
            Utc::now().to_rfc3339(),
            lines.len(),
            level.map(|l| format!(" · level {l}")).unwrap_or_default(),
            search
                .map(|s| format!(" · search \"{s}\""))
                .unwrap_or_default()
        ));
        for line in lines.iter().rev() {
            body.push_str(&format!(
                "{} [{}] {} — {}\n",
                line.ts.to_rfc3339(),
                line.level,
                line.target,
                line.message
            ));
        }
        std::fs::write(path, body)?;
        self.record_log(
            "info",
            "logs",
            &format!("exported {} log line(s) to {}", lines.len(), path.display()),
        );
        Ok(path.to_string_lossy().to_string())
    }

    pub async fn diagnostics_report(&self) -> Result<String> {
        let mut body = String::new();
        body.push_str("BT Farm diagnostics report\n");
        body.push_str("=========================\n");
        body.push_str(&format!("generated: {}\n", Utc::now().to_rfc3339()));
        body.push_str(&format!("app version: {}\n", env!("CARGO_PKG_VERSION")));
        body.push_str(&format!("platform: {}\n", std::env::consts::OS));
        body.push_str(&format!("data directory: {}\n", self.data_dir.display()));
        body.push_str(&format!("secret storage: {}\n", self.secrets.method()));
        let settings = self.settings();
        body.push_str("\nSettings\n--------\n");
        body.push_str(&format!(
            "stream_fps: {}\nboxd_path: {}\nffmpeg: {}\nwebdriver: {}\nrecordings_dir: {}\ncheck_updates_on_start: {}\n",
            settings.stream_fps,
            settings.boxd_path.as_deref().unwrap_or("(auto)"),
            settings.ffmpeg_path.as_deref().unwrap_or("(auto)"),
            settings.webdriver_url,
            settings.recordings_dir.as_deref().unwrap_or("(default)"),
            settings.check_updates_on_start
        ));

        body.push_str("\nadb\n---\n");
        let status = self.adb_status().await;
        body.push_str(&format!(
            "available: {}\npath: {}\nversion: {}\nmessage: {}\n",
            status.available,
            status.adb_path.as_deref().unwrap_or("(none)"),
            status.version.as_deref().unwrap_or("(unknown)"),
            status.message
        ));

        body.push_str("\nProviders\n---------\n");
        for status in self.provider_statuses()? {
            let caps = status.provider.capabilities();
            body.push_str(&format!(
                "{} [{}] enabled={} configured={} runtime={} transport={} exec={} instances={} error={}\n",
                status.label,
                status.provider.slug(),
                status.enabled,
                status.configured,
                caps.runtime,
                caps.transport.label(),
                caps.exec,
                status.instance_count,
                status.last_error.unwrap_or_else(|| "(none)".into())
            ));
        }

        let cache = self.cache.read();
        body.push_str("\nCached devices\n--------------\n");
        if cache.devices.is_empty() {
            body.push_str("(no scan has run yet)\n");
        }
        for device in &cache.devices {
            body.push_str(&format!(
                "{} · {} · state={} · android={} · transport={:?}\n",
                device.serial,
                device.label,
                device.state,
                device.android.as_deref().unwrap_or("?"),
                device.transport
            ));
        }
        if let Some(err) = &cache.devices_error {
            body.push_str(&format!("scan error: {err}\n"));
        }
        let runs = self.store.list_workflows()?.len();
        body.push_str(&format!(
            "\nworkflows: {runs}\nschedules: {}\nlog directory: {}\n",
            self.store.list_schedules()?.len(),
            self.log_file_path()
        ));
        body.push_str("\nNo credentials or API keys are included in this report.\n");

        let dir = self.data_dir.join("diagnostics");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("diagnostics-{}.txt", Utc::now().format("%Y%m%d-%H%M%S")));
        std::fs::write(&path, body)?;
        self.record_log(
            "info",
            "diagnostics",
            &format!("wrote diagnostics report to {}", path.display()),
        );
        Ok(path.to_string_lossy().to_string())
    }

    // ------------------------------------------------------------------
    // Workflows
    // ------------------------------------------------------------------

    pub fn duplicate_workflow(&self, id: &str) -> Result<Workflow> {
        let source = self
            .store
            .get_workflow(id)?
            .ok_or_else(|| CoreError::invalid("workflow not found"))?;
        let mut copy = source.clone();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.name = format!("{} (copy)", source.name);
        copy.created_at = Utc::now();
        copy.updated_at = copy.created_at;
        self.save_workflow(copy)
    }

    pub fn export_workflow(&self, id: &str, path: &str) -> Result<String> {
        let workflow = self
            .store
            .get_workflow(id)?
            .ok_or_else(|| CoreError::invalid("workflow not found"))?;
        let path = Path::new(path);
        if path.as_os_str().is_empty() {
            return Err(CoreError::invalid("choose where to save the workflow"));
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(&workflow)?;
        std::fs::write(path, json)?;
        self.record_log(
            "info",
            "workflow",
            &format!("exported '{}' to {}", workflow.name, path.display()),
        );
        Ok(path.to_string_lossy().to_string())
    }

    pub fn import_workflow(&self, path: &str) -> Result<Workflow> {
        let raw = std::fs::read_to_string(path)?;
        let mut workflow: Workflow = serde_json::from_str(&raw)
            .map_err(|e| CoreError::invalid(format!("that file is not a BT Farm workflow: {e}")))?;
        workflow.id = uuid::Uuid::new_v4().to_string();
        workflow.created_at = Utc::now();
        workflow.updated_at = workflow.created_at;
        let saved = self.save_workflow(workflow)?;
        self.record_log(
            "info",
            "workflow",
            &format!("imported '{}' from {path}", saved.name),
        );
        Ok(saved)
    }

    pub fn runs(
        &self,
        limit: usize,
        status: Option<&str>,
        workflow_id: Option<&str>,
    ) -> Result<Vec<WorkflowRun>> {
        let mut runs = self.store.query_runs(limit.clamp(1, 500), status, workflow_id)?;
        for run in &mut runs {
            run.lines.clear();
        }
        Ok(runs)
    }

    pub fn run_detail(&self, id: &str) -> Result<Option<WorkflowRun>> {
        self.store.get_run(id)
    }

    // ------------------------------------------------------------------
    // Recordings
    // ------------------------------------------------------------------

    pub fn recordings(&self) -> Result<Vec<RecordingFile>> {
        let dir = self.recordings_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };
            if !name.to_ascii_lowercase().ends_with(".webm") {
                continue;
            }
            let meta = entry.metadata().ok();
            files.push(RecordingFile {
                name,
                path: path.to_string_lossy().to_string(),
                bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified: meta
                    .and_then(|m| m.modified().ok())
                    .map(|t| chrono::DateTime::<Utc>::from(t)),
            });
        }
        files.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(files)
    }

    pub fn recordings_dir(&self) -> PathBuf {
        self.settings
            .read()
            .recordings_dir
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| self.data_dir.join("recordings"))
    }

    // ------------------------------------------------------------------
    // Instance presets
    // ------------------------------------------------------------------

    pub fn presets(&self) -> Result<Vec<InstancePreset>> {
        self.store.list_presets()
    }

    pub fn save_preset(&self, mut preset: InstancePreset) -> Result<InstancePreset> {
        if preset.name.trim().is_empty() {
            return Err(CoreError::invalid("give the preset a name"));
        }
        preset.name = preset.name.trim().to_string();
        if preset.id.trim().is_empty() {
            preset.id = uuid::Uuid::new_v4().to_string();
            preset.created_at = Utc::now();
        }
        self.store.save_preset(&preset)?;
        self.record_log(
            "info",
            "presets",
            &format!("saved preset '{}' for {}", preset.name, preset.provider.slug()),
        );
        Ok(preset)
    }

    pub fn delete_preset(&self, id: &str) -> Result<()> {
        self.store.delete_preset(id)
    }

    // ------------------------------------------------------------------
    // Updater
    // ------------------------------------------------------------------

    pub async fn check_update(&self) -> Result<UpdateInfo> {
        let info = updates::check(updates::UPDATE_REPO, env!("CARGO_PKG_VERSION")).await?;
        self.record_log(
            "info",
            "updater",
            &format!(
                "checked {}: current {}, latest {}, available {}",
                updates::UPDATE_REPO, info.current_version, info.latest_version, info.available
            ),
        );
        Ok(info)
    }

    pub async fn download_update(&self, info: &UpdateInfo) -> Result<UpdateDownload> {
        let dir = self.data_dir.join("updates");
        let downloaded = updates::download(info, &dir).await?;
        *DOWNLOADED_INSTALLER.lock() = Some(PathBuf::from(&downloaded.path));
        self.record_log(
            "info",
            "updater",
            &format!("downloaded {} bytes to {}", downloaded.bytes, downloaded.path),
        );
        Ok(downloaded)
    }

    pub fn install_update(&self) -> Result<String> {
        let path = DOWNLOADED_INSTALLER
            .lock()
            .clone()
            .ok_or_else(|| CoreError::invalid("download the update before installing it"))?;
        let dir = self.data_dir.join("updates");
        let launched = updates::launch_installer(&path, &dir)?;
        self.record_log(
            "info",
            "updater",
            &format!("launched installer {}", launched.display()),
        );
        Ok(launched.to_string_lossy().to_string())
    }
}

/// Recursive file count and byte total for one directory tree.
fn dir_usage(path: &std::path::Path) -> (usize, u64) {
    let mut files = 0_usize;
    let mut bytes = 0_u64;
    let Ok(entries) = std::fs::read_dir(path) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            let (nested_files, nested_bytes) = dir_usage(&entry.path());
            files += nested_files;
            bytes += nested_bytes;
        } else if meta.is_file() {
            files += 1;
            bytes += meta.len();
        }
    }
    (files, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Recursive size walk used by `storage_report`.
    fn dir_usage_test(path: &std::path::Path) -> (usize, u64) {
        dir_usage(path)
    }

    #[test]
    fn storage_report_measures_real_files() {
        let dir = tempfile::tempdir().unwrap();
        let core = FarmCore::new(dir.path()).unwrap();
        let shots = dir.path().join("screenshots");
        std::fs::create_dir_all(&shots).unwrap();
        std::fs::write(shots.join("a.png"), vec![0_u8; 1024]).unwrap();
        std::fs::write(shots.join("b.png"), vec![0_u8; 2048]).unwrap();

        let report = core.storage_report().unwrap();
        let entry = report
            .entries
            .iter()
            .find(|entry| entry.key == "screenshots")
            .unwrap();
        assert_eq!(entry.files, 2);
        assert_eq!(entry.bytes, 3072);
        assert_eq!(dir_usage_test(&shots), (2, 3072));
        assert!(report.total_bytes >= 3072);

        let cleaned = core.clean_storage("screenshots", 0).unwrap();
        assert_eq!(cleaned.deleted_files, 2);
        assert_eq!(cleaned.freed_bytes, 3072);
        assert!(!shots.join("a.png").exists());
    }

    #[test]
    fn custom_templates_round_trip_and_are_normalised() {
        let dir = tempfile::tempdir().unwrap();
        let core = FarmCore::new(dir.path()).unwrap();
        let saved = core
            .save_custom_template(CustomTemplateDraft {
                id: String::new(),
                name: "  Panel on Boat  ".into(),
                description: "panel test".into(),
                provider: Provider::Boat,
                workload: "Game panel".into(),
                request: CreateInstanceRequest {
                    cpu: Some(8),
                    memory_mb: Some(8192),
                    ..Default::default()
                },
                setup: "apt-get update -y".into(),
            })
            .unwrap();
        assert_eq!(saved.name, "Panel on Boat");
        assert!(saved.request.memory_mb.is_none(), "Boat cannot size memory");
        assert_eq!(saved.setup, "apt-get update -y");

        let listed = core.templates().unwrap();
        let entry = listed
            .iter()
            .find(|entry| entry.id == saved.id)
            .expect("custom template is listed");
        assert!(!entry.builtin);
        assert_eq!(entry.setup.as_deref(), Some("apt-get update -y"));
        assert!(listed.iter().any(|entry| entry.id == "boat-pterodactyl-panel" && entry.builtin));

        core.delete_custom_template(&saved.id).unwrap();
        assert!(!core.templates().unwrap().iter().any(|entry| entry.id == saved.id));
        assert!(
            core.delete_custom_template("boat-pterodactyl-panel").is_err(),
            "built-in templates cannot be deleted"
        );
    }

    #[test]
    fn cleaning_an_unknown_area_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let core = FarmCore::new(dir.path()).unwrap();
        assert!(core.clean_storage("..", 0).is_err());
        assert!(core.clean_storage("/etc", 0).is_err());
    }

    #[test]
    fn health_parses_real_device_output() {
        let battery = "Current Battery Service state:\n  AC powered: false\n  status: 2\n  health: 2\n  level: 87\n  temperature: 305\n";
        let storage = "Filesystem     1K-blocks    Used Available Use% Mounted on\n/dev/block/dm-4 115343360 4026532 111316828   4% /data\n";
        let memory = "MemTotal:        7643212 kB\nMemFree:          512000 kB\nMemAvailable:    3021000 kB\n";
        let uptime = "12345.67 98765.43\n";

        let health = DeviceHealth::from_outputs(
            "emulator-5554",
            battery,
            storage,
            memory,
            uptime,
            "Physical size: 1080x2400",
            "arm64-v8a",
            "34",
        );

        assert_eq!(health.battery_level, Some(87));
        assert_eq!(health.battery_status.as_deref(), Some("2"));
        assert_eq!(health.battery_temp_c, Some(30.5));
        assert_eq!(health.storage_total_kb, Some(115_343_360));
        assert_eq!(health.storage_free_kb, Some(111_316_828));
        assert_eq!(health.ram_total_kb, Some(7_643_212));
        assert_eq!(health.ram_free_kb, Some(3_021_000));
        assert_eq!(health.uptime_seconds, Some(12_345));
        assert_eq!(health.screen.as_deref(), Some("Physical size: 1080x2400"));
        assert_eq!(health.abi.as_deref(), Some("arm64-v8a"));
        assert_eq!(health.sdk.as_deref(), Some("34"));
    }

    #[test]
    fn presets_round_trip_through_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let core = FarmCore::new(dir.path()).unwrap();
        let draft = InstancePreset {
            id: String::new(),
            name: "  worker-4vcpu  ".into(),
            provider: Provider::E2b,
            request: CreateInstanceRequest {
                name: None,
                template: Some("base".into()),
                size: None,
                cpu: Some(4),
                memory_mb: Some(8192),
                disk_gb: None,
                ttl_seconds: Some(3600),
                gpu: None,
                gpu_type: None,
                env: BTreeMap::from([("ROLE".to_string(), "worker".to_string())]),
            },
            created_at: Utc::now(),
        };
        let saved = core.save_preset(draft).unwrap();
        assert_eq!(saved.name, "worker-4vcpu");
        assert!(!saved.id.is_empty());

        let listed = core.presets().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].provider, Provider::E2b);
        assert_eq!(listed[0].request.cpu, Some(4));
        assert_eq!(listed[0].request.memory_mb, Some(8192));
        assert_eq!(listed[0].request.env.get("ROLE").map(String::as_str), Some("worker"));

        core.delete_preset(&saved.id).unwrap();
        assert!(core.presets().unwrap().is_empty());
    }

    #[test]
    fn unnamed_presets_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let core = FarmCore::new(dir.path()).unwrap();
        let draft = InstancePreset {
            id: String::new(),
            name: "   ".into(),
            provider: Provider::Boat,
            request: CreateInstanceRequest {
                name: None,
                template: None,
                size: None,
                cpu: None,
                memory_mb: None,
                disk_gb: None,
                ttl_seconds: None,
                gpu: None,
                gpu_type: None,
                env: BTreeMap::new(),
            },
            created_at: Utc::now(),
        };
        assert!(core.save_preset(draft).is_err());
    }

    #[test]
    fn pair_rejects_bad_input_before_touching_adb() {
        let dir = tempfile::tempdir().unwrap();
        let core = FarmCore::new(dir.path()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(runtime.block_on(core.pair_device("192.168.1.4", "123456")).is_err());
        assert!(runtime.block_on(core.pair_device("192.168.1.4:5555", "abc")).is_err());
    }
}

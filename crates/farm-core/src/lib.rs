//! `farm-core` — the engine behind the desktop app.
//!
//! Everything that talks to the outside world lives here: provider REST clients,
//! the ADB transport, the real screencap streamer, the workflow runner, the
//! scheduler and publisher loops, and SQLite persistence. The Tauri shell is a
//! thin command surface over this crate.

pub mod android;
pub mod automation;
pub mod error;
pub mod events;
pub mod features;
pub mod http;
pub mod logging;
pub mod model;
pub mod process;
pub mod providers;
pub mod secrets;
pub mod social;
pub mod store;
pub mod updates;

use android::adb::Adb;
use android::stream::StreamManager;
use automation::{run_step, StepContext};
use base64::Engine;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use error::{CoreError, Result};
use events::CoreEvent;
use model::*;
use parking_lot::{Mutex, RwLock};
use providers::boxd::BoxdClient;
use providers::boat::BoatClient;
use providers::daytona::DaytonaClient;
use providers::e2b::E2bClient;
use providers::freestyle::FreestyleClient;
use providers::ProviderClient;
use serde::Serialize;
use social::Publisher;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use store::{ProviderMeta, Store};
use tokio::sync::broadcast;
use tokio::task::JoinSet;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderError {
    pub provider: Provider,
    pub message: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheState {
    pub instances: Vec<RemoteInstance>,
    pub errors: Vec<ProviderError>,
    pub devices: Vec<Device>,
    pub devices_error: Option<String>,
    pub last_refresh: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstancesSnapshot {
    pub instances: Vec<RemoteInstance>,
    pub errors: Vec<ProviderError>,
    pub refreshed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenshotResult {
    pub path: String,
    pub data_url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub struct FarmCore {
    data_dir: PathBuf,
    store: Arc<Store>,
    secrets: secrets::SecretStore,
    events: broadcast::Sender<CoreEvent>,
    settings: RwLock<AppSettings>,
    cache: RwLock<CacheState>,
    streams: StreamManager,
    runs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    props_cache: Mutex<HashMap<String, (Instant, android::adb::DeviceProps)>>,
    publisher: Publisher,
}

impl FarmCore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Arc<Self>> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir)?;
        let store = Arc::new(Store::open(&data_dir.join("farm.db"))?);
        let secrets = secrets::SecretStore::new(&data_dir)?;
        let settings = store
            .get_json_setting::<AppSettings>("app_settings")?
            .unwrap_or_default();
        let (events, _) = broadcast::channel(1024);
        // Any run left "running" is from a previous session and cannot still be alive.
        let _ = store.fail_stale_runs("the app closed while this run was in progress");
        let streams = StreamManager::new(events.clone(), data_dir.join("recordings"));
        Ok(Arc::new(Self {
            data_dir,
            store,
            secrets,
            events,
            settings: RwLock::new(settings),
            cache: RwLock::new(CacheState::default()),
            streams,
            runs: Mutex::new(HashMap::new()),
            props_cache: Mutex::new(HashMap::new()),
            publisher: Publisher::new(),
        }))
    }

    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }

    pub fn secret_backend(&self) -> &'static str {
        self.secrets.method()
    }

    fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }

    fn record_log(&self, level: &str, target: &str, message: &str) {
        match level {
            "error" => tracing::error!(target: "farm", "{target}: {message}"),
            "warn" => tracing::warn!(target: "farm", "{target}: {message}"),
            _ => tracing::info!(target: "farm", "{target}: {message}"),
        }
        if let Ok(()) = self.store.log(level, target, message) {
            if let Ok(lines) = self.store.list_logs(1, None, None) {
                if let Some(line) = lines.into_iter().next() {
                    self.emit(CoreEvent::Log { line });
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Provider configuration
    // ------------------------------------------------------------------

    fn secret_key(provider: Provider) -> String {
        format!("provider:{}:api_key", provider.slug())
    }

    fn base_url(&self, provider: Provider) -> Option<String> {
        self.store
            .provider_meta()
            .ok()
            .and_then(|meta| meta.into_iter().find(|m| m.provider == provider))
            .and_then(|m| m.base_url)
            .filter(|b| !b.trim().is_empty())
    }

    fn meta(&self, provider: Provider) -> Option<ProviderMeta> {
        self.store
            .provider_meta()
            .ok()
            .and_then(|meta| meta.into_iter().find(|m| m.provider == provider))
    }

    pub fn is_configured(&self, provider: Provider) -> bool {
        if provider == Provider::Boxd {
            let binary = process::resolve_binary(self.settings.read().boxd_path.as_deref(), "boxd");
            let token = self.secrets.has(&Self::secret_key(provider));
            binary.is_some() && token
        } else {
            self.secrets.has(&Self::secret_key(provider))
        }
    }

    fn is_enabled(&self, provider: Provider) -> bool {
        match self.meta(provider) {
            Some(meta) => meta.enabled,
            None => false,
        }
    }

    fn client(&self, provider: Provider) -> Result<Box<dyn ProviderClient>> {
        let base = self.base_url(provider);
        let key = self.secrets.get(&Self::secret_key(provider));
        let key_ref = providers::require_credentials(provider, key.as_ref())?;
        match provider {
            Provider::E2b => Ok(Box::new(E2bClient::new(key_ref, base)?)),
            Provider::Boat => Ok(Box::new(BoatClient::new(key_ref, base)?)),
            Provider::Daytona => Ok(Box::new(DaytonaClient::new(key_ref, base)?)),
            Provider::Freestyle => Ok(Box::new(FreestyleClient::new(key_ref, base)?)),
            Provider::Boxd => {
                let binary = process::resolve_binary(self.settings.read().boxd_path.as_deref(), "boxd");
                Ok(Box::new(BoxdClient::new(binary, Some(key_ref.to_string()), base)))
            }
        }
    }

    /// Stores credentials. `api_key = None` keeps the existing key, `Some("")`
    /// removes it.
    pub fn save_provider_settings(
        &self,
        provider: Provider,
        api_key: Option<String>,
        base_url: Option<String>,
        enabled: bool,
    ) -> Result<()> {
        if let Some(ref key) = api_key {
            self.secrets.set(&Self::secret_key(provider), key)?;
        }
        let base = base_url
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty())
            .filter(|b| b != provider.default_base_url());
        if let Some(base) = &base {
            if !(base.starts_with("http://") || base.starts_with("https://")) {
                return Err(CoreError::invalid("the base URL must start with http:// or https://"));
            }
        }
        self.store.set_provider_meta(provider, enabled, base.as_deref())?;
        if api_key.is_some() {
            self.store.set_provider_check(provider, None, None)?;
        }
        self.record_log(
            "info",
            "providers",
            &format!("{} settings updated (enabled: {enabled})", provider.label()),
        );
        Ok(())
    }

    pub fn provider_statuses(&self) -> Result<Vec<ProviderStatus>> {
        let cache = self.cache.read();
        Ok(Provider::ALL
            .into_iter()
            .map(|provider| {
                let meta = self.meta(provider);
                let instances: Vec<&RemoteInstance> = cache
                    .instances
                    .iter()
                    .filter(|i| i.provider == provider)
                    .collect();
                let capabilities = provider.capabilities();
                ProviderStatus {
                    provider,
                    label: provider.label().to_string(),
                    enabled: meta.as_ref().map(|m| m.enabled).unwrap_or(false),
                    configured: self.is_configured(provider),
                    base_url: meta
                        .as_ref()
                        .and_then(|m| m.base_url.clone())
                        .or_else(|| Some(provider.default_base_url().to_string())),
                    account: meta.as_ref().and_then(|m| m.account.clone()),
                    last_error: meta.as_ref().and_then(|m| m.last_error.clone()),
                    last_verified_at: meta.as_ref().and_then(|m| m.verified_at),
                    capabilities,
                    docs_url: provider.docs_url().to_string(),
                    console_url: provider.console_url().to_string(),
                    credential_hint: provider.credential_hint().to_string(),
                    instance_count: instances.len(),
                    active_instance_count: instances.iter().filter(|i| i.state.is_active()).count(),
                }
            })
            .collect())
    }

    pub async fn test_provider(&self, provider: Provider) -> Result<AccountInfo> {
        let client = self.client(provider)?;
        self.record_log("info", provider.slug(), "verifying credentials");
        match client.verify().await {
            Ok(info) => {
                self.store
                    .set_provider_check(provider, Some(&info.account), None)?;
                self.record_log(
                    "info",
                    provider.slug(),
                    &format!("credentials verified as {} ", info.account),
                );
                Ok(info)
            }
            Err(err) => {
                self.store
                    .set_provider_check(provider, None, Some(&err.to_string()))?;
                self.record_log("error", provider.slug(), &err.to_string());
                Err(err)
            }
        }
    }

    // ------------------------------------------------------------------
    // Instances
    // ------------------------------------------------------------------

    pub async fn refresh_instances(&self) -> InstancesSnapshot {
        let mut set: JoinSet<(Provider, Result<Vec<RemoteInstance>>)> = JoinSet::new();
        for provider in Provider::ALL {
            if !self.is_enabled(provider) {
                continue;
            }
            match self.client(provider) {
                Ok(client) => {
                    set.spawn(async move {
                        let result = client.list().await;
                        (provider, result)
                    });
                }
                Err(err) => {
                    set.spawn(async move { (provider, Err(err)) });
                }
            }
        }

        let mut instances = Vec::new();
        let mut errors = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((_provider, Ok(mut list))) => instances.append(&mut list),
                Ok((provider, Err(err))) => errors.push(ProviderError {
                    provider,
                    kind: err.kind().to_string(),
                    message: err.to_string(),
                }),
                Err(join_err) => errors.push(ProviderError {
                    provider: Provider::E2b,
                    kind: "internal".to_string(),
                    message: join_err.to_string(),
                }),
            }
        }
        instances.sort_by(|a, b| {
            b.state
                .is_active()
                .cmp(&a.state.is_active())
                .then_with(|| a.provider.slug().cmp(b.provider.slug()))
                .then_with(|| a.id.cmp(&b.id))
        });
        for err in &errors {
            self.record_log("warn", err.provider.slug(), &err.message);
        }
        let refreshed_at = Utc::now();
        {
            let mut cache = self.cache.write();
            cache.instances = instances.clone();
            cache.errors = errors.clone();
            cache.last_refresh = Some(refreshed_at);
        }
        InstancesSnapshot {
            instances,
            errors,
            refreshed_at: Some(refreshed_at),
        }
    }

    pub fn cached_instances(&self) -> InstancesSnapshot {
        let cache = self.cache.read();
        InstancesSnapshot {
            instances: cache.instances.clone(),
            errors: cache.errors.clone(),
            refreshed_at: cache.last_refresh,
        }
    }

    pub async fn create_instance(
        &self,
        provider: Provider,
        req: CreateInstanceRequest,
    ) -> Result<RemoteInstance> {
        let client = self.client(provider)?;
        self.record_log(
            "info",
            provider.slug(),
            &format!("creating instance (name: {:?})", req.sanitized_name(provider)),
        );
        match client.create(&req).await {
            Ok(instance) => {
                self.record_log(
                    "info",
                    provider.slug(),
                    &format!("instance {} created in state {}", instance.id, instance.raw_state),
                );
                self.emit(CoreEvent::InstanceChanged {
                    provider,
                    instance_id: instance.id.clone(),
                    message: format!("created ({})", instance.state.label()),
                });
                Ok(instance)
            }
            Err(err) => {
                self.record_log("error", provider.slug(), &err.to_string());
                Err(err)
            }
        }
    }

    pub async fn instance_action(
        &self,
        provider: Provider,
        id: &str,
        action: &str,
    ) -> Result<RemoteInstance> {
        let client = self.client(provider)?;
        match action {
            "start" => client.start(id).await?,
            "stop" => client.stop(id).await?,
            "delete" => {
                client.destroy(id).await?;
                self.record_log(
                    "info",
                    provider.slug(),
                    &format!("instance {id} deleted"),
                );
                self.emit(CoreEvent::InstanceChanged {
                    provider,
                    instance_id: id.to_string(),
                    message: "deleted".into(),
                });
                return Ok(RemoteInstance {
                    id: id.to_string(),
                    provider,
                    name: None,
                    state: InstanceState::Archived,
                    raw_state: "deleted".into(),
                    template: None,
                    cpu: None,
                    memory_mb: None,
                    disk_gb: None,
                    region: None,
                    endpoint: None,
                    created_at: None,
                    detail: None,
                });
            }
            other => return Err(CoreError::invalid(format!("unknown instance action '{other}'"))),
        }
        self.record_log(
            "info",
            provider.slug(),
            &format!("instance {id}: {action} requested"),
        );
        self.emit(CoreEvent::InstanceChanged {
            provider,
            instance_id: id.to_string(),
            message: format!("{action} requested"),
        });
        client.get(id).await
    }

    pub async fn instance_exec(
        &self,
        provider: Provider,
        id: &str,
        command: &str,
        timeout_secs: u64,
    ) -> Result<ExecOutput> {
        let client = self.client(provider)?;
        let output = client
            .exec(id, command, Duration::from_secs(timeout_secs.clamp(1, 600)))
            .await?;
        self.record_log(
            "info",
            provider.slug(),
            &format!("ran command on {id}: {command} (exit {})", output.exit_code),
        );
        Ok(output)
    }

    pub async fn instance_metrics(
        &self,
        provider: Provider,
        id: &str,
    ) -> Result<Option<InstanceMetrics>> {
        let client = self.client(provider)?;
        client.metrics(id).await
    }

    // ------------------------------------------------------------------
    // Android devices
    // ------------------------------------------------------------------

    pub fn adb_path(&self) -> Option<String> {
        process::resolve_binary(self.settings.read().adb_path.as_deref(), "adb")
    }

    fn adb(&self) -> Result<Adb> {
        self.adb_path().map(Adb::new).ok_or_else(|| {
            CoreError::Adb(
                "adb was not found. Install Android platform-tools, then set the full path to adb.exe in Settings."
                    .to_string(),
            )
        })
    }

    pub async fn adb_status(&self) -> AdbStatus {
        let Some(path) = self.adb_path() else {
            return AdbStatus {
                available: false,
                adb_path: None,
                version: None,
                server_running: false,
                message: "adb was not found on PATH and no path is configured.".into(),
            };
        };
        let adb = Adb::new(path.clone());
        match adb.version().await {
            Ok(version) => AdbStatus {
                available: true,
                adb_path: Some(path),
                version: Some(version),
                server_running: true,
                message: "adb is ready".into(),
            },
            Err(err) => AdbStatus {
                available: false,
                adb_path: Some(path),
                version: None,
                server_running: false,
                message: err.to_string(),
            },
        }
    }

    pub async fn list_devices(&self, refresh_props: bool) -> Result<Vec<Device>> {
        let adb = self.adb()?;
        let raw = adb.devices().await?;
        let meta = self.store.device_meta()?;
        let labels: HashMap<String, (String, Option<String>)> = meta
            .into_iter()
            .map(|(serial, label, notes)| (serial, (label, notes)))
            .collect();
        let online: Vec<String> = raw
            .iter()
            .filter(|d| d.state == "device")
            .map(|d| d.serial.clone())
            .collect();

        let mut props: HashMap<String, android::adb::DeviceProps> = HashMap::new();
        {
            let cache = self.props_cache.lock();
            for serial in &online {
                if let Some((at, cached)) = cache.get(serial) {
                    if !refresh_props || at.elapsed() < Duration::from_secs(120) {
                        props.insert(serial.clone(), cached.clone());
                    }
                }
            }
        }
        let missing: Vec<String> = online
            .into_iter()
            .filter(|serial| !props.contains_key(serial))
            .collect();
        if !missing.is_empty() {
            let mut set: JoinSet<(String, Option<android::adb::DeviceProps>)> = JoinSet::new();
            for serial in missing {
                let adb = adb.clone();
                set.spawn(async move {
                    let fetched = adb.props(&serial).await.ok();
                    (serial, fetched)
                });
            }
            while let Some(joined) = set.join_next().await {
                if let Ok((serial, Some(fetched))) = joined {
                    self.props_cache
                        .lock()
                        .insert(serial.clone(), (Instant::now(), fetched.clone()));
                    props.insert(serial, fetched);
                }
            }
        }

        let devices: Vec<Device> = raw
            .into_iter()
            .map(|device| {
                let fetched = props.get(&device.serial);
                let model = fetched
                    .and_then(|p| p.model.clone())
                    .or_else(|| device.model.clone())
                    .map(|m| m.replace('_', " "));
                let (label, notes) = match labels.get(&device.serial) {
                    Some((label, notes)) => (label.clone(), notes.clone()),
                    None => (
                        model.clone().unwrap_or_else(|| device.serial.clone()),
                        None,
                    ),
                };
                Device {
                    id: device.serial.clone(),
                    serial: device.serial.clone(),
                    label,
                    transport: if device.serial.contains(':') || device.serial.starts_with("adb-") {
                        DeviceTransport::Tcp
                    } else {
                        DeviceTransport::Usb
                    },
                    address: if device.serial.contains(':') {
                        Some(device.serial.clone())
                    } else {
                        None
                    },
                    state: device.state,
                    model,
                    android: fetched.and_then(|p| p.android.clone()),
                    product: device.product,
                    notes,
                    last_seen: Some(Utc::now()),
                }
            })
            .collect();

        {
            let mut cache = self.cache.write();
            cache.devices = devices.clone();
            cache.devices_error = None;
        }
        self.emit(CoreEvent::DeviceListChanged);
        Ok(devices)
    }

    pub async fn connect_device(&self, address: &str) -> Result<String> {
        let address = address.trim();
        if !address.contains(':') {
            return Err(CoreError::invalid(
                "use host:port, for example 192.168.1.50:5555 or a cloud phone endpoint",
            ));
        }
        let adb = self.adb()?;
        let message = adb.connect(address).await?;
        self.record_log("info", "adb", &format!("connect {address}: {message}"));
        Ok(message)
    }

    pub async fn disconnect_device(&self, address: &str) -> Result<String> {
        let adb = self.adb()?;
        let message = adb.disconnect(address).await?;
        self.record_log("info", "adb", &format!("disconnect {address}: {message}"));
        Ok(message)
    }

    pub fn set_device_label(&self, serial: &str, label: &str, notes: Option<&str>) -> Result<()> {
        if label.trim().is_empty() {
            return Err(CoreError::invalid("the device label cannot be empty"));
        }
        self.store.set_device_meta(serial, label.trim(), notes)
    }

    pub async fn device_shell(&self, serial: &str, command: &str) -> Result<String> {
        let adb = self.adb()?;
        let output = adb.shell(serial, command).await?;
        if output.timed_out {
            return Err(CoreError::Adb("the shell command timed out".into()));
        }
        Ok(output.combined())
    }

    pub async fn device_input(
        &self,
        serial: &str,
        kind: &str,
        a: Option<u32>,
        b: Option<u32>,
        c: Option<u32>,
        d: Option<u32>,
        text: Option<String>,
    ) -> Result<()> {
        let adb = self.adb()?;
        match kind {
            "tap" => adb
                .tap(serial, a.unwrap_or(0), b.unwrap_or(0))
                .await
                .map_err(|e| CoreError::Adb(e.to_string())),
            "swipe" => adb
                .swipe(
                    serial,
                    a.unwrap_or(0),
                    b.unwrap_or(0),
                    c.unwrap_or(0),
                    d.unwrap_or(0),
                    300,
                )
                .await
                .map_err(|e| CoreError::Adb(e.to_string())),
            "text" => adb
                .input_text(serial, &text.unwrap_or_default())
                .await
                .map_err(|e| CoreError::Adb(e.to_string())),
            "key" => adb
                .keyevent(serial, &text.unwrap_or_default())
                .await
                .map_err(|e| CoreError::Adb(e.to_string())),
            "back" => adb.keyevent(serial, "KEYCODE_BACK").await.map_err(|e| CoreError::Adb(e.to_string())),
            "home" => adb.keyevent(serial, "KEYCODE_HOME").await.map_err(|e| CoreError::Adb(e.to_string())),
            "app_switch" => adb
                .keyevent(serial, "KEYCODE_APP_SWITCH")
                .await
                .map_err(|e| CoreError::Adb(e.to_string())),
            "enter" => adb.keyevent(serial, "KEYCODE_ENTER").await.map_err(|e| CoreError::Adb(e.to_string())),
            other => Err(CoreError::invalid(format!("unknown input kind '{other}'"))),
        }
    }

    pub async fn device_screenshot(&self, serial: &str) -> Result<ScreenshotResult> {
        let adb = self.adb()?;
        let bytes = adb.screencap(serial).await?;
        let dir = self.data_dir.join("screenshots");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!(
            "{}-{}.png",
            serial.replace([':', '/', '\\'], "-"),
            Utc::now().format("%Y%m%d-%H%M%S%.3f")
        ));
        std::fs::write(&path, &bytes)?;
        let dims = android::adb::png_dimensions(&bytes);
        self.record_log("info", "adb", &format!("screenshot saved to {}", path.display()));
        Ok(ScreenshotResult {
            path: path.to_string_lossy().to_string(),
            data_url: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            ),
            width: dims.map(|d| d.0),
            height: dims.map(|d| d.1),
        })
    }

    pub async fn device_logcat(&self, serial: &str, lines: u32) -> Result<String> {
        self.adb()?.logcat(serial, lines.clamp(10, 5000)).await
    }

    pub async fn device_packages(&self, serial: &str) -> Result<Vec<String>> {
        self.adb()?.list_packages(serial).await
    }

    pub async fn device_install(&self, serial: &str, apk_path: &str) -> Result<String> {
        if !std::path::Path::new(apk_path).is_file() {
            return Err(CoreError::invalid(format!("no file at {apk_path}")));
        }
        let message = self.adb()?.install(serial, apk_path).await?;
        self.record_log("info", "adb", &format!("installed {apk_path} on {serial}"));
        Ok(message)
    }

    pub async fn device_uninstall(&self, serial: &str, package: &str) -> Result<()> {
        self.adb()?.uninstall(serial, package).await?;
        self.record_log("info", "adb", &format!("uninstalled {package} from {serial}"));
        Ok(())
    }

    pub async fn device_launch(&self, serial: &str, package: &str) -> Result<()> {
        self.adb()?.start_app(serial, package).await
    }

    pub async fn device_stop_app(&self, serial: &str, package: &str) -> Result<()> {
        self.adb()?.stop_app(serial, package).await
    }

    // ------------------------------------------------------------------
    // Live screen streaming
    // ------------------------------------------------------------------

    pub fn start_stream(&self, device_id: &str, fps: Option<u8>) -> Result<StreamStatus> {
        let adb = self.adb()?;
        let fps = fps.unwrap_or(self.settings.read().stream_fps).clamp(1, 15);
        self.streams.start(device_id, adb, fps)?;
        self.record_log("info", "stream", &format!("stream started for {device_id} at {fps} fps"));
        self.streams
            .status(device_id)
            .ok_or_else(|| CoreError::Adb("the stream could not be started".into()))
    }

    pub fn stop_all_streams(&self) {
        self.streams.stop_all();
    }

    pub fn stop_stream(&self, device_id: &str) -> bool {
        let stopped = self.streams.stop(device_id);
        if stopped {
            self.record_log("info", "stream", &format!("stream stopped for {device_id}"));
        }
        stopped
    }

    pub fn stream_statuses(&self) -> Vec<StreamStatus> {
        self.streams.statuses()
    }

    pub fn stream_frame(&self, device_id: &str) -> Option<String> {
        self.streams.latest_frame(device_id).map(|bytes| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes.as_slice())
            )
        })
    }

    pub fn set_recording(&self, device_id: &str, recording: bool) -> Result<String> {
        let dir = self.streams.set_recording(device_id, recording)?;
        Ok(dir.to_string_lossy().to_string())
    }

    pub async fn finish_recording(&self, device_id: &str) -> Result<(String, String)> {
        let ffmpeg = process::resolve_binary(self.settings.read().ffmpeg_path.as_deref(), "ffmpeg");
        let fps = self.settings.read().stream_fps;
        let (path, note) = self.streams.finish_recording(device_id, ffmpeg, fps).await?;
        self.record_log("info", "stream", &format!("recording finished: {}", path.display()));
        Ok((path.to_string_lossy().to_string(), note))
    }

    // ------------------------------------------------------------------
    // Workflows
    // ------------------------------------------------------------------

    pub fn list_workflows(&self) -> Result<Vec<Workflow>> {
        self.store.list_workflows()
    }

    pub fn get_workflow(&self, id: &str) -> Result<Option<Workflow>> {
        self.store.get_workflow(id)
    }

    pub fn save_workflow(&self, workflow: Workflow) -> Result<Workflow> {
        workflow.validate().map_err(CoreError::Automation)?;
        let mut workflow = workflow;
        if workflow.id.trim().is_empty() {
            workflow.id = uuid::Uuid::new_v4().to_string();
            workflow.created_at = Utc::now();
        }
        workflow.updated_at = Utc::now();
        for step in &mut workflow.steps {
            if step.id.trim().is_empty() {
                step.id = uuid::Uuid::new_v4().to_string();
            }
        }
        self.store.save_workflow(&workflow)?;
        self.record_log(
            "info",
            "workflow",
            &format!("saved '{}' with {} steps", workflow.name, workflow.steps.len()),
        );
        Ok(workflow)
    }

    pub fn delete_workflow(&self, id: &str) -> Result<()> {
        self.store.delete_workflow(id)?;
        self.record_log("info", "workflow", &format!("deleted workflow {id}"));
        Ok(())
    }

    pub async fn run_workflow(
        self: &Arc<Self>,
        workflow_id: &str,
        device_id: Option<String>,
        trigger: &str,
    ) -> Result<String> {
        let workflow = self
            .store
            .get_workflow(workflow_id)?
            .ok_or_else(|| CoreError::invalid("workflow not found"))?;
        workflow.validate().map_err(CoreError::Automation)?;
        let device = device_id.or_else(|| workflow.target_device.clone());
        let needs_device = workflow.steps.iter().any(|s| s.action.needs_device());
        if needs_device && device.is_none() {
            return Err(CoreError::Automation(
                "this workflow has device steps but no device was selected".into(),
            ));
        }
        let adb = if needs_device {
            Some(self.adb()?)
        } else {
            None
        };
        let webdriver_url = self.settings.read().webdriver_url.clone();
        let run_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let run = WorkflowRun {
            id: run_id.clone(),
            workflow_id: workflow.id.clone(),
            workflow_name: workflow.name.clone(),
            device_id: device.clone(),
            status: RunStatus::Queued,
            trigger: trigger.to_string(),
            started_at: now,
            finished_at: None,
            error: None,
            lines: Vec::new(),
        };
        self.store.insert_run(&run)?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.runs.lock().insert(run_id.clone(), cancel.clone());

        let ctx = StepContext::new(
            adb,
            device.clone(),
            webdriver_url,
            self.data_dir.clone(),
        );
        let store = self.store.clone();
        let events = self.events.clone();
        let run_id_task = run_id.clone();
        tokio::spawn(async move {
            execute_run(store, events, ctx, workflow, run_id_task, cancel, device).await;
        });
        self.emit(CoreEvent::RunUpdate {
            run_id: run_id.clone(),
            workflow_id: workflow_id.to_string(),
            status: RunStatus::Queued,
            line: Some(format!("queued ({trigger})")),
            error: None,
        });
        self.record_log(
            "info",
            "workflow",
            &format!("run {run_id} queued for '{}'", self.store.get_workflow(workflow_id)?.map(|w| w.name).unwrap_or_default()),
        );
        Ok(run_id)
    }

    pub fn cancel_run(&self, run_id: &str) -> bool {
        if let Some(flag) = self.runs.lock().get(run_id) {
            flag.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<WorkflowRun>> {
        self.store.get_run(run_id)
    }

    pub fn recent_runs(&self, limit: usize) -> Result<Vec<WorkflowRun>> {
        self.store.recent_runs(limit.min(200))
    }

    // ------------------------------------------------------------------
    // Schedules
    // ------------------------------------------------------------------

    pub fn list_schedules(&self) -> Result<Vec<Schedule>> {
        self.store.list_schedules()
    }

    pub fn save_schedule(&self, mut schedule: Schedule) -> Result<Schedule> {
        if schedule.id.trim().is_empty() {
            schedule.id = uuid::Uuid::new_v4().to_string();
        }
        if schedule.every_minutes == 0 && schedule.daily_at.is_none() {
            return Err(CoreError::invalid("set an interval in minutes or a daily time"));
        }
        if let Some(daily) = &schedule.daily_at {
            if parse_hhmm(daily).is_none() {
                return Err(CoreError::invalid("the daily time must be HH:MM"));
            }
        }
        if schedule.every_minutes > 0 && schedule.every_minutes < 1 {
            return Err(CoreError::invalid("the interval must be at least one minute"));
        }
        if self.store.get_workflow(&schedule.workflow_id)?.is_none() {
            return Err(CoreError::invalid("the selected workflow no longer exists"));
        }
        schedule.next_run_at = schedule.compute_next(Utc::now());
        self.store.save_schedule(&schedule)?;
        Ok(schedule)
    }

    pub fn delete_schedule(&self, id: &str) -> Result<()> {
        self.store.delete_schedule(id)
    }

    // ------------------------------------------------------------------
    // Publishing accounts
    // ------------------------------------------------------------------

    fn account_secret_key(id: &str) -> String {
        format!("account:{id}:token")
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut accounts = self.store.list_accounts()?;
        for account in &mut accounts {
            account.secret_set = self.secrets.has(&Self::account_secret_key(&account.id));
        }
        Ok(accounts)
    }

    pub fn save_account(
        &self,
        id: Option<String>,
        platform: String,
        label: String,
        kind: PublisherKind,
        config: serde_json::Value,
        token: Option<String>,
    ) -> Result<Account> {
        if label.trim().is_empty() {
            return Err(CoreError::invalid("give the account a label"));
        }
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let existing = self.store.get_account(&id)?;
        let account = Account {
            id: id.clone(),
            platform: platform.trim().to_string(),
            label: label.trim().to_string(),
            kind,
            config,
            secret_set: false,
            verified_at: existing.as_ref().and_then(|a| a.verified_at),
            verified_as: existing.as_ref().and_then(|a| a.verified_as.clone()),
            last_error: existing.as_ref().and_then(|a| a.last_error.clone()),
            created_at: existing.map(|a| a.created_at).unwrap_or_else(Utc::now),
        };
        if let Some(token) = token {
            self.secrets.set(&Self::account_secret_key(&id), &token)?;
        }
        self.store.save_account(&account)?;
        self.record_log("info", "publisher", &format!("saved account '{}'", account.label));
        self.list_accounts()?
            .into_iter()
            .find(|a| a.id == account.id)
            .ok_or_else(|| CoreError::invalid("account could not be reloaded"))
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        self.secrets.delete(&Self::account_secret_key(id))?;
        self.store.delete_account(id)
    }

    pub async fn verify_account(&self, id: &str) -> Result<Account> {
        let mut account = self
            .store
            .get_account(id)?
            .ok_or_else(|| CoreError::invalid("account not found"))?;
        let secret = self.secrets.get(&Self::account_secret_key(id));
        match self
            .publisher
            .verify(account.kind, &account.config, secret.as_deref())
            .await
        {
            Ok(identity) => {
                account.verified_at = Some(Utc::now());
                account.verified_as = Some(identity.clone());
                account.last_error = None;
                self.store.save_account(&account)?;
                self.record_log(
                    "info",
                    "publisher",
                    &format!("account '{}' verified as {identity}", account.label),
                );
                Ok(account)
            }
            Err(err) => {
                account.last_error = Some(err.to_string());
                self.store.save_account(&account)?;
                self.record_log("error", "publisher", &err.to_string());
                Err(err)
            }
        }
    }

    // ------------------------------------------------------------------
    // Posts
    // ------------------------------------------------------------------

    pub fn list_posts(&self, limit: usize) -> Result<Vec<Post>> {
        self.store.list_posts(limit.min(500))
    }

    pub fn save_post(
        &self,
        id: Option<String>,
        account_id: String,
        body: String,
        scheduled_at: Option<DateTime<Utc>>,
        publish_now: bool,
    ) -> Result<Post> {
        if body.trim().is_empty() {
            return Err(CoreError::invalid("the post body cannot be empty"));
        }
        let account = self
            .store
            .get_account(&account_id)?
            .ok_or_else(|| CoreError::invalid("the selected account no longer exists"))?;
        let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let existing = self.store.get_post(&id)?;
        let status = if publish_now {
            PostStatus::Scheduled
        } else if scheduled_at.is_some() {
            PostStatus::Scheduled
        } else {
            PostStatus::Draft
        };
        let post = Post {
            id: id.clone(),
            account_id: account.id,
            account_label: account.label,
            body,
            scheduled_at: if publish_now { Some(Utc::now()) } else { scheduled_at },
            status,
            remote_id: existing.as_ref().and_then(|p| p.remote_id.clone()),
            published_at: existing.as_ref().and_then(|p| p.published_at),
            error: None,
            created_at: existing.map(|p| p.created_at).unwrap_or_else(Utc::now),
        };
        self.store.save_post(&post)?;
        Ok(post)
    }

    pub fn delete_post(&self, id: &str) -> Result<()> {
        self.store.delete_post(id)
    }

    pub async fn publish_post(&self, id: &str) -> Result<Post> {
        let mut post = self
            .store
            .get_post(id)?
            .ok_or_else(|| CoreError::invalid("post not found"))?;
        let account = self
            .store
            .get_account(&post.account_id)?
            .ok_or_else(|| CoreError::invalid("the publishing account was removed"))?;
        post.status = PostStatus::Publishing;
        self.store.save_post(&post)?;
        let secret = self.secrets.get(&Self::account_secret_key(&account.id));
        match self
            .publisher
            .publish(account.kind, &account.config, secret.as_deref(), &post.body)
            .await
        {
            Ok(outcome) => {
                post.status = PostStatus::Published;
                post.published_at = Some(Utc::now());
                post.remote_id = outcome.remote_id;
                post.error = None;
                self.store.save_post(&post)?;
                self.record_log(
                    "info",
                    "publisher",
                    &format!("published to '{}': {}", account.label, outcome.detail),
                );
                self.emit(CoreEvent::PostUpdate {
                    post_id: post.id.clone(),
                    status: post.status,
                    error: None,
                });
                Ok(post)
            }
            Err(err) => {
                post.status = PostStatus::Failed;
                post.error = Some(err.to_string());
                self.store.save_post(&post)?;
                self.record_log("error", "publisher", &err.to_string());
                self.emit(CoreEvent::PostUpdate {
                    post_id: post.id.clone(),
                    status: post.status,
                    error: Some(err.to_string()),
                });
                Err(err)
            }
        }
    }

    // ------------------------------------------------------------------
    // Settings, logs, overview
    // ------------------------------------------------------------------

    pub fn settings(&self) -> AppSettings {
        self.settings.read().clone()
    }

    pub fn save_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        if !(settings.webdriver_url.starts_with("http://") || settings.webdriver_url.starts_with("https://")) {
            return Err(CoreError::invalid("the WebDriver URL must start with http:// or https://"));
        }
        let settings = AppSettings {
            stream_fps: settings.stream_fps.clamp(1, 15),
            ..settings
        };
        self.store.set_json_setting("app_settings", &settings)?;
        *self.settings.write() = settings.clone();
        self.record_log("info", "settings", "settings updated");
        Ok(settings)
    }

    pub async fn test_webdriver(&self) -> Result<String> {
        let client = automation::WebDriver::new(&self.settings.read().webdriver_url)?;
        client.verify().await
    }

    pub fn logs(&self, limit: usize, level: Option<&str>, search: Option<&str>) -> Result<Vec<LogLine>> {
        self.store.list_logs(limit.min(2000), level, search)
    }

    pub fn log_file_path(&self) -> String {
        logging::log_dir(&self.data_dir).to_string_lossy().to_string()
    }

    pub async fn overview(&self) -> Result<Overview> {
        let statuses = self.provider_statuses()?;
        let cache = self.cache.read().clone();
        let since = Utc::now() - ChronoDuration::hours(24);
        let runs_today = self.store.count_runs_since(since, false)?;
        let runs_failed_today = self.store.count_runs_since(since, true)?;
        let schedules = self.store.list_schedules()?;
        let posts = self.store.list_posts(200)?;
        let workflows = self.store.list_workflows()?;
        let provider_breakdown = statuses
            .iter()
            .map(|status| ProviderSummary {
                provider: status.provider,
                label: status.label.clone(),
                configured: status.configured,
                running: status.active_instance_count,
                total: status.instance_count,
                error: status.last_error.clone(),
            })
            .collect();
        let mut recent_runs = self.store.recent_runs(5)?;
        for run in &mut recent_runs {
            run.lines.clear();
        }
        Ok(Overview {
            providers_configured: statuses.iter().filter(|s| s.configured).count(),
            providers_total: Provider::ALL.len(),
            instances_running: cache.instances.iter().filter(|i| i.state.is_active()).count(),
            instances_total: cache.instances.len(),
            devices_online: cache.devices.iter().filter(|d| d.is_online()).count(),
            devices_total: cache.devices.len(),
            workflows: workflows.len(),
            runs_today,
            runs_failed_today,
            schedules_enabled: schedules.iter().filter(|s| s.enabled).count(),
            posts_pending: posts
                .iter()
                .filter(|p| matches!(p.status, PostStatus::Scheduled | PostStatus::Publishing))
                .count(),
            provider_breakdown,
            recent_runs,
            recent_logs: self.store.list_logs(8, None, None)?,
        })
    }

    // ------------------------------------------------------------------
    // Background loops
    // ------------------------------------------------------------------

    pub fn start_background(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(15));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let Some(core) = weak.upgrade() else { break };
                if let Err(err) = core.tick_schedules().await {
                    core.record_log("error", "scheduler", &err.to_string());
                }
                if let Err(err) = core.tick_publishes().await {
                    core.record_log("error", "publisher", &err.to_string());
                }
            }
        });
    }

    async fn tick_schedules(self: &Arc<Self>) -> Result<()> {
        let now = Utc::now();
        for mut schedule in self.store.due_schedules(now)? {
            if self.store.get_workflow(&schedule.workflow_id)?.is_none() {
                schedule.enabled = false;
                schedule.last_status = Some("workflow missing".into());
                self.store.save_schedule(&schedule)?;
                continue;
            }
            match self
                .run_workflow(&schedule.workflow_id, schedule.device_id.clone(), "schedule")
                .await
            {
                Ok(_) => {
                    schedule.last_run_at = Some(now);
                    schedule.last_status = Some("started".into());
                }
                Err(err) => {
                    schedule.last_run_at = Some(now);
                    schedule.last_status = Some(err.to_string());
                    self.record_log("error", "scheduler", &err.to_string());
                }
            }
            schedule.next_run_at = schedule.compute_next(now);
            self.store.save_schedule(&schedule)?;
        }
        Ok(())
    }

    async fn tick_publishes(&self) -> Result<()> {
        for post in self.store.due_posts(Utc::now())? {
            if let Err(err) = self.publish_post(&post.id).await {
                self.record_log("warn", "publisher", &format!("post {}: {err}", post.id));
            }
        }
        Ok(())
    }
}

async fn log_line(
    store: &Store,
    events: &broadcast::Sender<CoreEvent>,
    run_id: &str,
    workflow_id: &str,
    message: &str,
) {
    let _ = store.append_run_line(run_id, message);
    let _ = events.send(CoreEvent::RunUpdate {
        run_id: run_id.to_string(),
        workflow_id: workflow_id.to_string(),
        status: RunStatus::Running,
        line: Some(message.to_string()),
        error: None,
    });
}

async fn execute_run(
    store: Arc<Store>,
    events: broadcast::Sender<CoreEvent>,
    ctx: StepContext,
    workflow: Workflow,
    run_id: String,
    cancel: Arc<AtomicBool>,
    device_id: Option<String>,
) {
    let _ = store.update_run(&run_id, RunStatus::Running, None);
    let _ = events.send(CoreEvent::RunUpdate {
        run_id: run_id.clone(),
        workflow_id: workflow.id.clone(),
        status: RunStatus::Running,
        line: Some(format!(
            "starting '{}' ({} steps{})",
            workflow.name,
            workflow.steps.len(),
            device_id
                .as_ref()
                .map(|d| format!(" on {d}"))
                .unwrap_or_default()
        )),
        error: None,
    });

    let mut failure: Option<String> = None;
    let mut cancelled = false;
    let total = workflow.steps.len();

    for (index, step) in workflow.steps.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        log_line(
            &store,
            &events,
            &run_id,
            &workflow.id,
            &format!("step {}/{}: {} [{}]", index + 1, total, step.name, step.action.kind()),
        )
        .await;

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match run_step(&ctx, step).await {
                Ok(lines) => {
                    for line in lines {
                        log_line(&store, &events, &run_id, &workflow.id, &format!("  {line}")).await;
                    }
                    break;
                }
                Err(err) => {
                    let message = err.to_string();
                    if attempt <= step.retries && !cancel.load(Ordering::SeqCst) {
                        log_line(
                            &store,
                            &events,
                            &run_id,
                            &workflow.id,
                            &format!("  attempt {attempt} failed: {message}; retrying"),
                        )
                        .await;
                        tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                        continue;
                    }
                    if step.continue_on_error {
                        log_line(
                            &store,
                            &events,
                            &run_id,
                            &workflow.id,
                            &format!("  failed but continuing: {message}"),
                        )
                        .await;
                        break;
                    }
                    failure = Some(format!("step '{}': {message}", step.name));
                    log_line(
                        &store,
                        &events,
                        &run_id,
                        &workflow.id,
                        &format!("  failed: {message}"),
                    )
                    .await;
                    break;
                }
            }
        }
        if failure.is_some() {
            break;
        }
    }

    ctx.close().await;

    let (status, error) = if cancelled || cancel.load(Ordering::SeqCst) {
        (RunStatus::Cancelled, Some("cancelled by the user".to_string()))
    } else if let Some(message) = failure {
        (RunStatus::Failed, Some(message))
    } else {
        (RunStatus::Succeeded, None)
    };

    let _ = store.update_run(&run_id, status, error.as_deref());
    let _ = events.send(CoreEvent::RunUpdate {
        run_id: run_id.clone(),
        workflow_id: workflow.id.clone(),
        status,
        line: Some(format!("run finished: {}", status.label())),
        error,
    });
}

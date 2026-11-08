use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Infrastructure backends this app can drive. Each one is a *Linux* compute
/// provider: none of them exposes Android virtualisation, which is why cloud
/// phones are attached over ADB instead of being faked inside these VMs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    E2b,
    Boat,
    Boxd,
    Daytona,
    Freestyle,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::E2b,
        Provider::Boat,
        Provider::Boxd,
        Provider::Daytona,
        Provider::Freestyle,
    ];

    pub fn slug(&self) -> &'static str {
        match self {
            Provider::E2b => "e2b",
            Provider::Boat => "boat",
            Provider::Boxd => "boxd",
            Provider::Daytona => "daytona",
            Provider::Freestyle => "freestyle",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Provider::E2b => "E2B",
            Provider::Boat => "Boat",
            Provider::Boxd => "boxd",
            Provider::Daytona => "Daytona",
            Provider::Freestyle => "Freestyle",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.slug() == slug)
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            Provider::E2b => "https://api.e2b.app",
            Provider::Boat => "https://boat.dev/api/v1",
            // boxd is driven through its official CLI, which talks to boxd.sh.
            Provider::Boxd => "https://boxd.sh",
            Provider::Daytona => "https://app.daytona.io/api",
            Provider::Freestyle => "https://api.freestyle.sh",
        }
    }

    pub fn docs_url(&self) -> &'static str {
        match self {
            Provider::E2b => "https://docs.e2b.dev/api-reference",
            Provider::Boat => "https://docs.boat.dev/api/v1",
            Provider::Boxd => "https://docs.boxd.sh/cli/commands",
            Provider::Daytona => "https://www.daytona.io/docs/en/tools/api",
            Provider::Freestyle => "https://www.freestyle.sh/docs/llms-full.txt",
        }
    }

    pub fn console_url(&self) -> &'static str {
        match self {
            Provider::E2b => "https://e2b.dev/dashboard",
            Provider::Boat => "https://boat.dev/dashboard",
            Provider::Boxd => "https://app.boxd.sh",
            Provider::Daytona => "https://app.daytona.io/dashboard",
            Provider::Freestyle => "https://dash.freestyle.sh",
        }
    }

    /// How a credential reaches the provider. Shown verbatim in Settings so the
    /// user knows what kind of token to paste.
    pub fn credential_hint(&self) -> &'static str {
        match self {
            Provider::E2b => "E2B API key (sent as the X-API-Key header)",
            Provider::Boat => "Boat API key starting with boat_ (sent as a bearer token)",
            Provider::Boxd => "boxd API key starting with bxd_ (passed to the boxd CLI as BOXD_TOKEN)",
            Provider::Daytona => "Daytona API key (sent as a bearer token)",
            Provider::Freestyle => "Freestyle API key (sent as a bearer token)",
        }
    }

    pub fn capabilities(&self) -> Capabilities {
        match self {
            Provider::E2b => Capabilities {
                transport: Transport::Rest,
                runtime: "Firecracker microVM",
                terminal: "No in-app terminal (exec needs envd)",
                sizing: Sizing::FixedByTemplate,
                sizing_note: "E2B derives CPU and RAM from the template; the create call accepts no sizing fields.",
                gpu: GpuSupport::Unsupported,
                gpu_note: "E2B documents CPU-only sandboxes: no GPU in the SDKs, CLI, API or template definition.",
                gpu_types: &[],
                provision: true,
                lifecycle: true,
                exec: false,
                metrics: true,
                android: false,
                note: "Linux microVMs. Command execution requires E2B's envd/Connect channel, which is not part of the platform REST API authenticated here, so exec is reported unsupported. Metrics come from GET /sandboxes/metrics.",
                android_note: "E2B sandboxes are Linux microVMs with no KVM passthrough or GPU, so an Android emulator cannot run inside them.",
            },
            Provider::Boat => Capabilities {
                transport: Transport::Rest,
                runtime: "Ubuntu 24.04 virtual machine",
                terminal: "In-app terminal (REST exec)",
                sizing: Sizing::CpuOnly,
                sizing_note: "Boat machines come in small/default/large/xlarge classes; the requested vCPU count selects the class and RAM is fixed per class.",
                gpu: GpuSupport::Unsupported,
                gpu_note: "Boat's documented machine classes carry no GPU option.",
                gpu_types: &[],
                provision: true,
                lifecycle: true,
                exec: true,
                metrics: false,
                android: false,
                note: "Full Ubuntu VMs. Commands run through POST /sandboxes/{id}/commands. Boat v1 exposes no CPU/memory metrics endpoint, only cost, so live utilisation is reported as unavailable.",
                android_note: "Boat machines are Ubuntu 24.04 VMs with no nested virtualisation or GPU, so they cannot host an Android emulator.",
            },
            Provider::Boxd => Capabilities {
                transport: Transport::Cli,
                runtime: "Linux KVM microVM",
                terminal: "Host terminal (boxd CLI exec)",
                sizing: Sizing::CpuOnly,
                sizing_note: "The boxd CLI creates machines with --vcpu (1/2/4/8); memory and disk are not part of machine new.",
                gpu: GpuSupport::Unsupported,
                gpu_note: "boxd documents no GPU machines.",
                gpu_types: &[],
                provision: true,
                lifecycle: true,
                exec: true,
                metrics: false,
                android: false,
                note: "Driven through the official boxd CLI with --json. boxd publishes SDKs and a CLI rather than a stable public HTTP API (its console API rejects third-party clients), so the CLI is the supported integration path.",
                android_note: "boxd machines are Linux KVM microVMs without a running Android stack; Android is attached over ADB instead.",
            },
            Provider::Daytona => Capabilities {
                transport: Transport::Rest,
                runtime: "Container or Linux VM snapshot",
                terminal: "No in-app terminal (needs toolbox/SSH)",
                sizing: Sizing::Selectable,
                sizing_note: "Daytona takes cpu, memory (GiB) and disk on the create call.",
                gpu: GpuSupport::Supported,
                gpu_note: "GPU sandboxes are created through POST /sandboxes/gpu with a GPU type and unit count.",
                gpu_types: &["H100", "H200", "B200", "B300", "RTX-PRO-6000", "RTX-4090", "RTX-5090", "MI355X"],
                provision: true,
                lifecycle: true,
                exec: false,
                metrics: false,
                android: false,
                note: "Sandbox lifecycle through the Daytona API. Command execution needs a per-sandbox toolbox/SSH credential that the API key alone does not grant, so exec is reported unsupported.",
                android_note: "Daytona sandboxes are containers or Linux VMs without nested KVM or a GPU; Android cannot boot inside them.",
            },
            Provider::Freestyle => Capabilities {
                transport: Transport::Rest,
                runtime: "Linux virtual machine",
                terminal: "In-app terminal (REST exec)",
                sizing: Sizing::FixedByTemplate,
                sizing_note: "Freestyle boots a snapshot (freestyle/ubuntu or freestyle/ubuntu-lg); the snapshot decides the machine size.",
                gpu: GpuSupport::Unsupported,
                gpu_note: "Freestyle offers GPU sandboxes on Enterprise only; the create call this app uses has no GPU field.",
                gpu_types: &[],
                provision: true,
                lifecycle: true,
                exec: true,
                metrics: false,
                android: false,
                note: "Full Linux VMs. Commands run through POST /v5/vms/{id}/exec-await. The API reports cumulative CPU time per VM rather than live utilisation, so that is what the UI shows.",
                android_note: "Freestyle VMs are Linux guests without nested virtualisation, so Android must run on a dedicated device host reached over ADB.",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Rest,
    Cli,
}

impl Transport {
    pub fn label(&self) -> &'static str {
        match self {
            Transport::Rest => "REST API",
            Transport::Cli => "Official CLI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sizing {
    /// CPU, memory and disk are accepted on the create call.
    Selectable,
    /// Only the vCPU count is accepted; RAM follows from the machine class.
    CpuOnly,
    /// The template or snapshot fixes the machine size.
    FixedByTemplate,
}

impl Sizing {
    pub fn label(&self) -> &'static str {
        match self {
            Sizing::Selectable => "size at create time",
            Sizing::CpuOnly => "vCPU only",
            Sizing::FixedByTemplate => "fixed by template",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuSupport {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub transport: Transport,
    /// What the provider actually runs: a microVM, a full VM or a container.
    pub runtime: &'static str,
    /// Where the user gets a shell for these machines, if at all.
    pub terminal: &'static str,
    /// How machine size is decided at create time.
    pub sizing: Sizing,
    pub sizing_note: &'static str,
    pub gpu: GpuSupport,
    pub gpu_note: &'static str,
    /// GPU identifiers the provider documents, empty when it has none.
    pub gpu_types: &'static [&'static str],
    pub provision: bool,
    pub lifecycle: bool,
    pub exec: bool,
    pub metrics: bool,
    pub android: bool,
    pub note: &'static str,
    pub android_note: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceState {
    Creating,
    Starting,
    Running,
    Paused,
    Stopped,
    Archived,
    Error,
    Unknown,
}

impl InstanceState {
    pub fn label(&self) -> &'static str {
        match self {
            InstanceState::Creating => "Creating",
            InstanceState::Starting => "Starting",
            InstanceState::Running => "Running",
            InstanceState::Paused => "Paused",
            InstanceState::Stopped => "Stopped",
            InstanceState::Archived => "Archived",
            InstanceState::Error => "Error",
            InstanceState::Unknown => "Unknown",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, InstanceState::Running | InstanceState::Starting | InstanceState::Creating)
    }

    pub fn parse(raw: &str) -> Self {
        let v = raw.to_ascii_lowercase();
        match v.as_str() {
            "running" | "ready" | "started" | "active" | "provisioned" => Self::Running,
            "starting" | "provisioning" | "init" | "cloning" | "pending" | "creating" => Self::Starting,
            "paused" | "suspended" | "pausing" | "hibernated" | "archiving" => Self::Paused,
            "stopped" | "stopping" | "off" | "suspended_idle" | "shutdown" => Self::Stopped,
            "archived" | "deleted" | "destroyed" | "cancelled" | "terminated" => Self::Archived,
            "error" | "failed" | "unhealthy" => Self::Error,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInstance {
    pub id: String,
    pub provider: Provider,
    pub name: Option<String>,
    pub state: InstanceState,
    pub raw_state: String,
    pub template: Option<String>,
    pub cpu: Option<u32>,
    pub memory_mb: Option<u64>,
    pub disk_gb: Option<f64>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateInstanceRequest {
    pub name: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub cpu: Option<u32>,
    #[serde(default)]
    pub memory_mb: Option<u64>,
    #[serde(default)]
    pub disk_gb: Option<u64>,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
    /// Requested GPU units; only providers reporting `GpuSupport::Supported`
    /// accept this.
    #[serde(default)]
    pub gpu: Option<u32>,
    /// Provider GPU identifier, e.g. Daytona's `H100`.
    #[serde(default)]
    pub gpu_type: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl CreateInstanceRequest {
    /// Drops sizing fields the provider does not accept, so nothing is sent on
    /// the wire that the provider would silently ignore.
    pub fn normalized_for(&self, provider: Provider) -> CreateInstanceRequest {
        let caps = provider.capabilities();
        let mut out = self.clone();
        match caps.sizing {
            Sizing::Selectable => {}
            Sizing::CpuOnly => {
                out.memory_mb = None;
                out.disk_gb = None;
            }
            Sizing::FixedByTemplate => {
                out.cpu = None;
                out.memory_mb = None;
                out.disk_gb = None;
            }
        }
        if caps.gpu == GpuSupport::Unsupported {
            out.gpu = None;
            out.gpu_type = None;
        }
        out
    }

    /// Names of the fields that will not reach the provider, for logging and UI.
    pub fn ignored_fields(&self, provider: Provider) -> Vec<&'static str> {
        let caps = provider.capabilities();
        let mut ignored = Vec::new();
        if self.cpu.is_some() && caps.sizing == Sizing::FixedByTemplate {
            ignored.push("cpu");
        }
        if self.memory_mb.is_some() && caps.sizing != Sizing::Selectable {
            ignored.push("memory_mb");
        }
        if self.disk_gb.is_some() && caps.sizing != Sizing::Selectable {
            ignored.push("disk_gb");
        }
        if (self.gpu.is_some() || self.gpu_type.is_some()) && caps.gpu == GpuSupport::Unsupported {
            ignored.push("gpu");
        }
        ignored
    }

    pub fn sanitized_name(&self, provider: Provider) -> Option<String> {
        let raw = self.name.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        // Keep provider-safe names: lowercase alphanumerics and dashes only.
        let mut out = String::new();
        for ch in raw.chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch.to_ascii_lowercase());
            } else if !out.ends_with('-') {
                out.push('-');
            }
        }
        let trimmed = out.trim_matches('-').to_string();
        if trimmed.is_empty() {
            return None;
        }
        let max = if provider == Provider::Daytona { 40 } else { 63 };
        Some(trimmed.chars().take(max).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

impl ExecOutput {
    pub fn combined(&self) -> String {
        if self.stderr.trim().is_empty() {
            self.stdout.clone()
        } else if self.stdout.trim().is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout.trim_end(), self.stderr.trim_end())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetrics {
    pub cpu_percent: Option<f64>,
    pub memory_used_mb: Option<f64>,
    pub memory_total_mb: Option<f64>,
    pub disk_used_mb: Option<f64>,
    pub disk_total_mb: Option<f64>,
    pub sampled_at: DateTime<Utc>,
    /// Provider-specific remarks, e.g. "cumulative CPU time".
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account: String,
    pub plan: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub provider: Provider,
    pub label: String,
    pub enabled: bool,
    pub configured: bool,
    pub base_url: Option<String>,
    pub account: Option<String>,
    pub last_error: Option<String>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub capabilities: Capabilities,
    pub docs_url: String,
    pub console_url: String,
    pub credential_hint: String,
    pub instance_count: usize,
    pub active_instance_count: usize,
}

// ---------------------------------------------------------------------------
// Android devices
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceTransport {
    Usb,
    Tcp,
}

impl DeviceTransport {
    pub fn label(&self) -> &'static str {
        match self {
            DeviceTransport::Usb => "USB / local adb",
            DeviceTransport::Tcp => "Network adb",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub serial: String,
    pub label: String,
    pub transport: DeviceTransport,
    pub address: Option<String>,
    /// Raw adb state: device, offline, unauthorized, ...
    pub state: String,
    pub model: Option<String>,
    pub android: Option<String>,
    pub product: Option<String>,
    pub notes: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
}

impl Device {
    pub fn is_online(&self) -> bool {
        self.state == "device"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    Connecting,
    Streaming,
    Error,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatus {
    pub device_id: String,
    pub state: StreamState,
    pub fps: f32,
    pub frames: u64,
    pub message: Option<String>,
    pub recording: bool,
    pub recording_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdbStatus {
    pub available: bool,
    pub adb_path: Option<String>,
    pub version: Option<String>,
    pub server_running: bool,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Automation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target_device: Option<String>,
    pub steps: Vec<Step>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    pub action: StepAction,
    #[serde(default)]
    pub retries: u32,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub continue_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepAction {
    Tap { x: u32, y: u32 },
    Swipe { x1: u32, y1: u32, x2: u32, y2: u32, duration_ms: u32 },
    Text { text: String },
    Key { code: String },
    Shell { command: String },
    LaunchApp { package: String },
    StopApp { package: String },
    InstallApk { path: String },
    Wait { ms: u64 },
    WaitForActivity { package: String, timeout_ms: u64 },
    Screenshot { label: String },
    WdNavigate { url: String },
    WdClick { selector: String },
    WdType { selector: String, text: String },
    WdWaitFor { selector: String, timeout_ms: u64 },
    WdAssertText { selector: String, contains: String },
    WdScreenshot { label: String },
    HttpRequest {
        method: String,
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        expect_status: Option<u16>,
    },
    Notify { message: String },
}

impl StepAction {
    pub fn kind(&self) -> &'static str {
        match self {
            StepAction::Tap { .. } => "tap",
            StepAction::Swipe { .. } => "swipe",
            StepAction::Text { .. } => "text",
            StepAction::Key { .. } => "key",
            StepAction::Shell { .. } => "shell",
            StepAction::LaunchApp { .. } => "launch_app",
            StepAction::StopApp { .. } => "stop_app",
            StepAction::InstallApk { .. } => "install_apk",
            StepAction::Wait { .. } => "wait",
            StepAction::WaitForActivity { .. } => "wait_for_activity",
            StepAction::Screenshot { .. } => "screenshot",
            StepAction::WdNavigate { .. } => "wd_navigate",
            StepAction::WdClick { .. } => "wd_click",
            StepAction::WdType { .. } => "wd_type",
            StepAction::WdWaitFor { .. } => "wd_wait_for",
            StepAction::WdAssertText { .. } => "wd_assert_text",
            StepAction::WdScreenshot { .. } => "wd_screenshot",
            StepAction::HttpRequest { .. } => "http_request",
            StepAction::Notify { .. } => "notify",
        }
    }

    pub fn needs_device(&self) -> bool {
        matches!(
            self,
            StepAction::Tap { .. }
                | StepAction::Swipe { .. }
                | StepAction::Text { .. }
                | StepAction::Key { .. }
                | StepAction::Shell { .. }
                | StepAction::LaunchApp { .. }
                | StepAction::StopApp { .. }
                | StepAction::InstallApk { .. }
                | StepAction::WaitForActivity { .. }
                | StepAction::Screenshot { .. }
        )
    }

    pub fn needs_webdriver(&self) -> bool {
        matches!(
            self,
            StepAction::WdNavigate { .. }
                | StepAction::WdClick { .. }
                | StepAction::WdType { .. }
                | StepAction::WdWaitFor { .. }
                | StepAction::WdAssertText { .. }
                | StepAction::WdScreenshot { .. }
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            StepAction::Tap { x, y } => {
                if *x > 20_000 || *y > 20_000 {
                    return Err("tap coordinates are out of range".into());
                }
            }
            StepAction::Swipe { x1, y1, x2, y2, duration_ms } => {
                if [x1, y1, x2, y2].iter().any(|v| **v > 20_000) {
                    return Err("swipe coordinates are out of range".into());
                }
                if *duration_ms == 0 || *duration_ms > 10_000 {
                    return Err("swipe duration must be between 1 and 10000 ms".into());
                }
            }
            StepAction::Text { text } => {
                if text.is_empty() {
                    return Err("text step needs a value".into());
                }
                if text.chars().any(|c| c == '\n') {
                    return Err("adb text input cannot contain newlines; press KEYCODE_ENTER instead".into());
                }
            }
            StepAction::Key { code } => {
                if code.trim().is_empty() {
                    return Err("key step needs a keycode such as KEYCODE_HOME".into());
                }
            }
            StepAction::Shell { command } => {
                if command.trim().is_empty() {
                    return Err("shell step needs a command".into());
                }
            }
            StepAction::LaunchApp { package } | StepAction::StopApp { package } => {
                if !is_valid_package(package) {
                    return Err(format!("'{package}' is not a valid Android package name"));
                }
            }
            StepAction::InstallApk { path } => {
                if path.trim().is_empty() {
                    return Err("install step needs an APK path".into());
                }
            }
            StepAction::Wait { ms } => {
                if *ms > 600_000 {
                    return Err("wait steps are capped at 10 minutes".into());
                }
            }
            StepAction::WaitForActivity { package, timeout_ms } => {
                if !is_valid_package(package) {
                    return Err(format!("'{package}' is not a valid Android package name"));
                }
                if *timeout_ms == 0 || *timeout_ms > 600_000 {
                    return Err("activity timeout must be between 1 and 600000 ms".into());
                }
            }
            StepAction::Screenshot { label } | StepAction::WdScreenshot { label } => {
                if label.trim().is_empty() {
                    return Err("screenshot step needs a label".into());
                }
            }
            StepAction::WdNavigate { url } => {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err("navigate step needs an http(s) URL".into());
                }
            }
            StepAction::WdClick { selector } => {
                if selector.trim().is_empty() {
                    return Err("click step needs a CSS selector".into());
                }
            }
            StepAction::WdType { selector, text } => {
                if selector.trim().is_empty() {
                    return Err("type step needs a CSS selector".into());
                }
                if text.is_empty() {
                    return Err("type step needs text".into());
                }
            }
            StepAction::WdWaitFor { selector, timeout_ms } => {
                if selector.trim().is_empty() {
                    return Err("wait step needs a CSS selector".into());
                }
                if *timeout_ms == 0 || *timeout_ms > 600_000 {
                    return Err("webdriver wait must be between 1 and 600000 ms".into());
                }
            }
            StepAction::WdAssertText { selector, contains } => {
                if selector.trim().is_empty() {
                    return Err("assert step needs a CSS selector".into());
                }
                if contains.is_empty() {
                    return Err("assert step needs text to look for".into());
                }
            }
            StepAction::HttpRequest { method, url, expect_status, .. } => {
                if !matches!(method.to_ascii_uppercase().as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
                    return Err("http step supports GET, POST, PUT, PATCH and DELETE".into());
                }
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err("http step needs an http(s) URL".into());
                }
                if let Some(code) = expect_status {
                    if *code < 100 || *code > 599 {
                        return Err("expected status must be a valid HTTP status".into());
                    }
                }
            }
            StepAction::Notify { message } => {
                if message.trim().is_empty() {
                    return Err("notify step needs a message".into());
                }
            }
        }
        Ok(())
    }
}

pub fn is_valid_package(package: &str) -> bool {
    if package.len() < 3 || package.len() > 200 {
        return false;
    }
    let mut parts = package.split('.');
    let mut count = 0;
    for part in parts.by_ref() {
        count += 1;
        if part.is_empty() || !part.chars().next().unwrap_or(' ').is_ascii_alphabetic() {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    count >= 2
}

impl Workflow {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("workflow needs a name".into());
        }
        if self.steps.is_empty() {
            return Err("workflow needs at least one step".into());
        }
        for (idx, step) in self.steps.iter().enumerate() {
            step.action
                .validate()
                .map_err(|e| format!("step {} ({}): {e}", idx + 1, step.name))?;
        }
        Ok(())
    }
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn label(&self) -> &'static str {
        match self {
            RunStatus::Queued => "Queued",
            RunStatus::Running => "Running",
            RunStatus::Succeeded => "Succeeded",
            RunStatus::Failed => "Failed",
            RunStatus::Cancelled => "Cancelled",
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub device_id: Option<String>,
    pub status: RunStatus,
    pub trigger: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub device_id: Option<String>,
    /// Minutes between runs. 0 means "daily at `daily_at`".
    pub every_minutes: u32,
    /// "HH:MM" in local time when `every_minutes` is 0.
    pub daily_at: Option<String>,
    pub enabled: bool,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
}

impl Schedule {
    /// Computes the next UTC instant this schedule should fire. Daily schedules
    /// are expressed in local wall-clock time by the UI and stored as UTC here.
    pub fn compute_next(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if !self.enabled {
            return None;
        }
        if self.every_minutes > 0 {
            return Some(from + chrono::Duration::minutes(self.every_minutes as i64));
        }
        let daily = self.daily_at.as_deref()?;
        let (hh, mm) = parse_hhmm(daily)?;
        let base = from.with_timezone(&chrono::Local);
        let mut candidate = base
            .date_naive()
            .and_hms_opt(hh, mm, 0)?;
        if candidate <= base.naive_local() {
            candidate += chrono::Duration::days(1);
        }
        let local = candidate.and_local_timezone(chrono::Local).single()?;
        Some(local.with_timezone(&Utc))
    }
}

pub fn parse_hhmm(value: &str) -> Option<(u32, u32)> {
    let (h, m) = value.split_once(':')?;
    let hh: u32 = h.trim().parse().ok()?;
    let mm: u32 = m.trim().parse().ok()?;
    if hh > 23 || mm > 59 {
        return None;
    }
    Some((hh, mm))
}

// ---------------------------------------------------------------------------
// Publishing (authorised platforms only)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublisherKind {
    /// Official Mastodon REST API: POST /api/v1/statuses with a user token.
    Mastodon,
    /// Any official HTTP API the user supplies (a templated request).
    Webhook,
}

impl PublisherKind {
    pub fn label(&self) -> &'static str {
        match self {
            PublisherKind::Mastodon => "Mastodon API",
            PublisherKind::Webhook => "Official HTTP API",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub platform: String,
    pub label: String,
    pub kind: PublisherKind,
    /// Non-secret configuration: instance URL, request shape, headers.
    pub config: serde_json::Value,
    pub secret_set: bool,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_as: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostStatus {
    Draft,
    Scheduled,
    Publishing,
    Published,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub account_id: String,
    pub account_label: String,
    pub body: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub status: PostStatus,
    pub remote_id: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Settings & logging
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub adb_path: Option<String>,
    /// Path to the official boxd CLI, when the boxd provider is used.
    #[serde(default)]
    pub boxd_path: Option<String>,
    pub webdriver_url: String,
    pub stream_fps: u8,
    pub recordings_dir: Option<String>,
    pub ffmpeg_path: Option<String>,
    /// Ask GitHub for a newer release when the app starts.
    #[serde(default = "default_true")]
    pub check_updates_on_start: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            adb_path: None,
            boxd_path: None,
            webdriver_url: "http://127.0.0.1:4444".to_string(),
            stream_fps: 4,
            recordings_dir: None,
            ffmpeg_path: None,
            check_updates_on_start: true,
        }
    }
}

/// Everything `device_health` reports, parsed from real `dumpsys`/`df`/`proc`
/// output. Every field is optional because devices differ; nothing is invented.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DeviceHealth {
    pub serial: String,
    pub fetched_at: DateTime<Utc>,
    pub battery_level: Option<u8>,
    pub battery_status: Option<String>,
    pub battery_temp_c: Option<f32>,
    pub battery_health: Option<String>,
    pub storage_total_kb: Option<u64>,
    pub storage_free_kb: Option<u64>,
    pub ram_total_kb: Option<u64>,
    pub ram_free_kb: Option<u64>,
    pub uptime_seconds: Option<u64>,
    pub screen: Option<String>,
    pub sdk: Option<String>,
    pub abi: Option<String>,
    pub errors: Vec<String>,
}

impl DeviceHealth {
    /// Builds a health snapshot from the raw command output. Blank or
    /// unparseable values stay `None` rather than being guessed.
    #[allow(clippy::too_many_arguments)]
    pub fn from_outputs(
        serial: &str,
        battery: &str,
        storage: &str,
        memory: &str,
        uptime: &str,
        screen: &str,
        abi: &str,
        sdk: &str,
    ) -> Self {
        fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
            text.lines()
                .map(|l| l.trim())
                .find_map(|line| line.strip_prefix(key))
                .map(|v| v.trim())
        }
        let mut health = DeviceHealth {
            serial: serial.to_string(),
            fetched_at: Utc::now(),
            ..Default::default()
        };

        health.battery_level = field(battery, "level:").and_then(|v| v.parse().ok());
        health.battery_status = field(battery, "status:").map(|v| v.to_string());
        health.battery_health = field(battery, "health:").map(|v| v.to_string());
        health.battery_temp_c = field(battery, "temperature:").and_then(|v| v.parse::<f32>().ok()).map(|t| t / 10.0);
        health.ram_total_kb = field(memory, "MemTotal:").and_then(|v| v.split_whitespace().next()?.parse().ok());
        health.ram_free_kb = field(memory, "MemAvailable:")
            .or_else(|| field(memory, "MemFree:"))
            .and_then(|v| v.split_whitespace().next()?.parse().ok());
        health.uptime_seconds = uptime
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v as u64);
        health.screen = Some(screen.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());
        health.abi = Some(abi.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());
        health.sdk = Some(sdk.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());

        // `df -k /data` → Filesystem 1K-blocks Used Available Use% Mounted on
        if let Some(row) = storage
            .lines()
            .map(|l| l.trim())
            .find(|l| l.split_whitespace().count() >= 4 && l.split_whitespace().next() != Some("Filesystem"))
        {
            let cols: Vec<&str> = row.split_whitespace().collect();
            health.storage_total_kb = cols.get(1).and_then(|v| v.parse().ok());
            health.storage_free_kb = cols.get(3).and_then(|v| v.parse().ok());
        }
        health
    }
}

/// One device's result inside a batch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkTarget {
    pub provider: Provider,
    pub id: String,
}

/// One directory the app writes into, with its measured size.
#[derive(Debug, Clone, Serialize)]
pub struct StorageEntry {
    pub key: String,
    pub label: String,
    pub path: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageReport {
    pub data_dir: String,
    pub total_bytes: u64,
    pub entries: Vec<StorageEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageCleanup {
    pub key: String,
    pub deleted_files: usize,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchOutcome {
    pub serial: String,
    pub ok: bool,
    pub output: String,
}

/// A saved instance recipe so the same machine can be created again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancePreset {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub request: CreateInstanceRequest,
    pub created_at: DateTime<Utc>,
}

/// A file produced by the in-app recorder.
#[derive(Debug, Clone, Serialize)]
pub struct RecordingFile {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overview {
    pub providers_configured: usize,
    pub providers_total: usize,
    pub instances_running: usize,
    pub instances_total: usize,
    pub devices_online: usize,
    pub devices_total: usize,
    pub workflows: usize,
    pub runs_today: usize,
    pub runs_failed_today: usize,
    pub schedules_enabled: usize,
    pub posts_pending: usize,
    pub provider_breakdown: Vec<ProviderSummary>,
    pub recent_runs: Vec<WorkflowRun>,
    pub recent_logs: Vec<LogLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub provider: Provider,
    pub label: String,
    pub configured: bool,
    pub running: usize,
    pub total: usize,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_state_maps_provider_words() {
        assert_eq!(InstanceState::parse("running"), InstanceState::Running);
        assert_eq!(InstanceState::parse("ready"), InstanceState::Running);
        assert_eq!(InstanceState::parse("archived"), InstanceState::Archived);
        assert_eq!(InstanceState::parse("provisioning"), InstanceState::Starting);
        assert_eq!(InstanceState::parse("paused"), InstanceState::Paused);
        assert_eq!(InstanceState::parse("wat"), InstanceState::Unknown);
    }

    #[test]
    fn package_validation_rejects_junk() {
        assert!(is_valid_package("com.android.chrome"));
        assert!(is_valid_package("org.mozilla.firefox_beta"));
        assert!(!is_valid_package("chrome"));
        assert!(!is_valid_package("com..chrome"));
        assert!(!is_valid_package("com.9chrome"));
        assert!(!is_valid_package("com/chrome"));
    }

    #[test]
    fn names_are_sanitised_per_provider() {
        let req = CreateInstanceRequest {
            name: Some("  My Worker #1  ".to_string()),
            ..Default::default()
        };
        assert_eq!(req.sanitized_name(Provider::Boat).as_deref(), Some("my-worker-1"));
    }

    #[test]
    fn hhmm_parsing() {
        assert_eq!(parse_hhmm("09:30"), Some((9, 30)));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("nope"), None);
    }

    #[test]
    fn step_validation_catches_newlines() {
        let step = StepAction::Text { text: "a\nb".into() };
        assert!(step.validate().is_err());
    }
}

// ---------------------------------------------------------------------------
// User-created instance templates
// ---------------------------------------------------------------------------

/// A template the operator saved in this app: the provisioning request they
/// want to reuse, plus an optional setup command to run after boot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: Provider,
    pub workload: String,
    pub request: CreateInstanceRequest,
    pub setup: String,
    pub created_at: DateTime<Utc>,
}

/// What the UI sends when creating or updating a custom template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTemplateDraft {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub provider: Provider,
    #[serde(default)]
    pub workload: String,
    #[serde(default)]
    pub request: CreateInstanceRequest,
    #[serde(default)]
    pub setup: String,
}

/// Result of provisioning straight from a template, including whether the
/// setup command actually ran and what it printed.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateProvision {
    pub template_id: String,
    pub instance: RemoteInstance,
    pub setup_ran: bool,
    pub setup_exit_code: Option<i32>,
    pub setup_output: String,
    pub setup_note: String,
}

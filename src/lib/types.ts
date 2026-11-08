// Mirrors the serde types exported by farm-core. Enum tags are snake_case
// because that is what the Rust side serialises.

export type Provider = "e2b" | "boat" | "boxd" | "daytona" | "freestyle";

export type InstanceState =
  | "creating"
  | "starting"
  | "running"
  | "paused"
  | "stopped"
  | "archived"
  | "error"
  | "unknown";

export interface Capabilities {
  transport: "rest" | "cli";
  /** What the provider runs: microVM, full VM or container. */
  runtime: string;
  /** Where the user gets a shell for those machines, if at all. */
  terminal: string;
  /** How machine size is decided at create time. */
  sizing: "selectable" | "cpu_only" | "fixed_by_template";
  sizing_note: string;
  gpu: "supported" | "unsupported";
  gpu_note: string;
  gpu_types: string[];
  provision: boolean;
  lifecycle: boolean;
  exec: boolean;
  metrics: boolean;
  android: boolean;
  note: string;
  android_note: string;
}

export interface ProviderStatus {
  provider: Provider;
  label: string;
  enabled: boolean;
  configured: boolean;
  base_url: string | null;
  account: string | null;
  last_error: string | null;
  last_verified_at: string | null;
  capabilities: Capabilities;
  docs_url: string;
  console_url: string;
  credential_hint: string;
  instance_count: number;
  active_instance_count: number;
}

export interface RemoteInstance {
  id: string;
  provider: Provider;
  name: string | null;
  state: InstanceState;
  raw_state: string;
  template: string | null;
  cpu: number | null;
  memory_mb: number | null;
  disk_gb: number | null;
  region: string | null;
  endpoint: string | null;
  created_at: string | null;
  detail: string | null;
}

export interface ProviderError {
  provider: Provider;
  message: string;
  kind: string;
}

export interface InstancesSnapshot {
  instances: RemoteInstance[];
  errors: ProviderError[];
  refreshed_at: string | null;
}

export interface CreateInstanceRequest {
  name?: string | null;
  template?: string | null;
  size?: string | null;
  cpu?: number | null;
  memory_mb?: number | null;
  disk_gb?: number | null;
  ttl_seconds?: number | null;
  gpu?: number | null;
  gpu_type?: string | null;
  env: Record<string, string>;
}

export interface InstanceTemplate {
  id: string;
  provider: Provider;
  name: string;
  description: string;
  image: string | null;
  size: string | null;
  cpu: number | null;
  memory_gb: number | null;
  disk_gb: number | null;
  ttl_seconds: number | null;
  gpu: number | null;
  gpu_type: string | null;
  workload: string;
  source: string;
  /** Real shell command run after boot, or null when the template only provisions. */
  setup: string | null;
  builtin: boolean;
}

export interface CustomTemplateDraft {
  id?: string;
  name: string;
  description: string;
  provider: Provider;
  workload: string;
  request: Partial<CreateInstanceRequest>;
  setup: string;
}

export interface TemplateProvision {
  template_id: string;
  instance: RemoteInstance;
  setup_ran: boolean;
  setup_exit_code: number | null;
  setup_output: string;
  setup_note: string;
}

export interface BulkTarget {
  provider: Provider;
  id: string;
}

export interface StorageEntry {
  key: string;
  label: string;
  path: string;
  files: number;
  bytes: number;
}

export interface StorageReport {
  data_dir: string;
  total_bytes: number;
  entries: StorageEntry[];
}

export interface StorageCleanup {
  key: string;
  deleted_files: number;
  freed_bytes: number;
}

export interface ExecOutput {
  stdout: string;
  stderr: string;
  exit_code: number;
  duration_ms: number;
}

export interface InstanceMetrics {
  cpu_percent: number | null;
  memory_used_mb: number | null;
  memory_total_mb: number | null;
  disk_used_mb: number | null;
  disk_total_mb: number | null;
  sampled_at: string;
  note: string | null;
}

export interface AccountInfo {
  account: string;
  plan: string | null;
  detail: string | null;
}

export interface Device {
  id: string;
  serial: string;
  label: string;
  transport: "usb" | "tcp";
  address: string | null;
  state: string;
  model: string | null;
  android: string | null;
  product: string | null;
  notes: string | null;
  last_seen: string | null;
}

export interface StreamStatus {
  device_id: string;
  state: "connecting" | "streaming" | "error" | "stopped";
  fps: number;
  frames: number;
  message: string | null;
  recording: boolean;
  recording_dir: string | null;
}

export interface AdbStatus {
  available: boolean;
  adb_path: string | null;
  version: string | null;
  server_running: boolean;
  message: string;
}

export interface ScreenshotResult {
  path: string;
  data_url: string;
  width: number | null;
  height: number | null;
}

export type StepAction =
  | { kind: "tap"; x: number; y: number }
  | { kind: "swipe"; x1: number; y1: number; x2: number; y2: number; duration_ms: number }
  | { kind: "text"; text: string }
  | { kind: "key"; code: string }
  | { kind: "shell"; command: string }
  | { kind: "launch_app"; package: string }
  | { kind: "stop_app"; package: string }
  | { kind: "install_apk"; path: string }
  | { kind: "wait"; ms: number }
  | { kind: "wait_for_activity"; package: string; timeout_ms: number }
  | { kind: "screenshot"; label: string }
  | { kind: "wd_navigate"; url: string }
  | { kind: "wd_click"; selector: string }
  | { kind: "wd_type"; selector: string; text: string }
  | { kind: "wd_wait_for"; selector: string; timeout_ms: number }
  | { kind: "wd_assert_text"; selector: string; contains: string }
  | { kind: "wd_screenshot"; label: string }
  | {
      kind: "http_request";
      method: string;
      url: string;
      headers: Record<string, string>;
      body: string | null;
      expect_status: number | null;
    }
  | { kind: "notify"; message: string };

export interface Step {
  id: string;
  name: string;
  action: StepAction;
  retries: number;
  timeout_ms: number | null;
  continue_on_error: boolean;
}

export interface Workflow {
  id: string;
  name: string;
  description: string;
  target_device: string | null;
  steps: Step[];
  created_at: string;
  updated_at: string;
}

export type RunStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

export interface WorkflowRun {
  id: string;
  workflow_id: string;
  workflow_name: string;
  device_id: string | null;
  status: RunStatus;
  trigger: string;
  started_at: string;
  finished_at: string | null;
  error: string | null;
  lines: string[];
}

export interface Schedule {
  id: string;
  workflow_id: string;
  workflow_name: string;
  device_id: string | null;
  every_minutes: number;
  daily_at: string | null;
  enabled: boolean;
  next_run_at: string | null;
  last_run_at: string | null;
  last_status: string | null;
}

export type PublisherKind = "mastodon" | "webhook";

export interface Account {
  id: string;
  platform: string;
  label: string;
  kind: PublisherKind;
  config: Record<string, unknown>;
  secret_set: boolean;
  verified_at: string | null;
  verified_as: string | null;
  last_error: string | null;
  created_at: string;
}

export type PostStatus = "draft" | "scheduled" | "publishing" | "published" | "failed";

export interface Post {
  id: string;
  account_id: string;
  account_label: string;
  body: string;
  scheduled_at: string | null;
  status: PostStatus;
  remote_id: string | null;
  published_at: string | null;
  error: string | null;
  created_at: string;
}

export interface LogLine {
  id: number;
  ts: string;
  level: string;
  target: string;
  message: string;
}

export interface ProviderSummary {
  provider: Provider;
  label: string;
  configured: boolean;
  running: number;
  total: number;
  error: string | null;
}

export interface Overview {
  providers_configured: number;
  providers_total: number;
  instances_running: number;
  instances_total: number;
  devices_online: number;
  devices_total: number;
  workflows: number;
  runs_today: number;
  runs_failed_today: number;
  schedules_enabled: number;
  posts_pending: number;
  provider_breakdown: ProviderSummary[];
  recent_runs: WorkflowRun[];
  recent_logs: LogLine[];
}

export interface AppSettings {
  adb_path: string | null;
  boxd_path: string | null;
  webdriver_url: string;
  stream_fps: number;
  recordings_dir: string | null;
  ffmpeg_path: string | null;
  check_updates_on_start: boolean;
}

export interface CoreInfo {
  version: string;
  data_dir: string;
  log_dir: string;
  secret_backend: string;
  platform: string;
  adb_path: string | null;
  stream_fps: number;
}

export interface RecordingResult {
  path: string;
  note: string;
}

export interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version: string;
  release_name: string;
  notes: string;
  published_at: string | null;
  html_url: string;
  asset_name: string | null;
  asset_url: string | null;
  asset_size: number | null;
  digest: string | null;
  checked_at: string;
  note: string | null;
}

export interface UpdateDownload {
  path: string;
  bytes: number;
  verified: boolean;
  note: string;
}

export interface DeviceHealth {
  serial: string;
  fetched_at: string;
  battery_level: number | null;
  battery_status: string | null;
  battery_temp_c: number | null;
  battery_health: string | null;
  storage_total_kb: number | null;
  storage_free_kb: number | null;
  ram_total_kb: number | null;
  ram_free_kb: number | null;
  uptime_seconds: number | null;
  screen: string | null;
  sdk: string | null;
  abi: string | null;
  errors: string[];
}

export interface BatchOutcome {
  serial: string;
  ok: boolean;
  output: string;
}

export interface InstancePreset {
  id: string;
  name: string;
  provider: Provider;
  request: CreateInstanceRequest;
  created_at: string;
}

export interface RecordingFile {
  name: string;
  path: string;
  bytes: number;
  modified: string | null;
}

export type CoreEvent =
  | { kind: "stream_frame"; device_id: string; image: string; seq: number; bytes: number; ts: string }
  | { kind: "stream_status"; status: StreamStatus }
  | {
      kind: "run_update";
      run_id: string;
      workflow_id: string;
      status: RunStatus;
      line: string | null;
      error: string | null;
    }
  | { kind: "device_list_changed" }
  | { kind: "instance_changed"; provider: Provider; instance_id: string; message: string }
  | { kind: "post_update"; post_id: string; status: PostStatus; error: string | null }
  | { kind: "log"; line: LogLine };

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  AccountInfo,
  AdbStatus,
  AppSettings,
  BatchOutcome,
  CoreEvent,
  CoreInfo,
  CreateInstanceRequest,
  Device,
  DeviceHealth,
  ExecOutput,
  InstancesSnapshot,
  InstanceMetrics,
  InstancePreset,
  InstanceTemplate,
  CustomTemplateDraft,
  TemplateProvision,
  BulkTarget,
  StorageCleanup,
  StorageReport,
  LogLine,
  Overview,
  Post,
  Provider,
  ProviderStatus,
  PublisherKind,
  RecordingFile,
  RecordingResult,
  RemoteInstance,
  RunStatus,
  Schedule,
  ScreenshotResult,
  StreamStatus,
  UpdateDownload,
  UpdateInfo,
  Workflow,
  WorkflowRun,
} from "./types";

/** True when the UI is running inside the Tauri shell. */
export const isDesktop =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window as unknown as Record<string, unknown>);

export const BRIDGE_ERROR =
  "The desktop backend is not connected. Open BT Farm on Windows to use live data.";

export class BridgeError extends Error {
  constructor(message = BRIDGE_ERROR) {
    super(message);
    this.name = "BridgeError";
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop) {
    throw new BridgeError();
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    // Tauri rejects with { kind, message } from our ErrorPayload.
    if (error && typeof error === "object" && "message" in error) {
      const payload = error as { kind?: string; message?: string };
      throw new Error(payload.message ?? String(error));
    }
    throw new Error(typeof error === "string" ? error : String(error));
  }
}

export const api = {
  coreInfo: () => call<CoreInfo>("core_info"),
  overview: () => call<Overview>("overview"),

  providerStatuses: () => call<ProviderStatus[]>("provider_statuses"),
  saveProviderSettings: (
    provider: Provider,
    apiKey: string | null,
    baseUrl: string | null,
    enabled: boolean,
  ) => call<void>("save_provider_settings", { provider, apiKey, baseUrl, enabled }),
  testProvider: (provider: Provider) => call<AccountInfo>("test_provider", { provider }),

  instances: (refresh: boolean) => call<InstancesSnapshot>("instances", { refresh }),
  createInstance: (provider: Provider, request: CreateInstanceRequest) =>
    call<RemoteInstance>("create_instance", { provider, request }),
  instanceAction: (provider: Provider, id: string, action: "start" | "stop" | "delete") =>
    call<RemoteInstance>("instance_action", { provider, id, action }),
  instanceExec: (provider: Provider, id: string, command: string, timeoutSecs?: number) =>
    call<ExecOutput>("instance_exec", { provider, id, command, timeoutSecs }),
  instanceMetrics: (provider: Provider, id: string) =>
    call<InstanceMetrics | null>("instance_metrics", { provider, id }),

  adbStatus: () => call<AdbStatus>("adb_status"),
  devices: (refresh: boolean) => call<Device[]>("devices", { refresh }),
  connectDevice: (address: string) => call<string>("connect_device", { address }),
  disconnectDevice: (address: string) => call<string>("disconnect_device", { address }),
  setDeviceLabel: (serial: string, label: string, notes?: string | null) =>
    call<void>("set_device_label", { serial, label, notes }),
  deviceShell: (serial: string, command: string) =>
    call<string>("device_shell", { serial, command }),
  deviceInput: (
    serial: string,
    kind: string,
    opts: { a?: number; b?: number; c?: number; d?: number; text?: string } = {},
  ) => call<void>("device_input", { serial, kind, ...opts }),
  deviceScreenshot: (serial: string) => call<ScreenshotResult>("device_screenshot", { serial }),
  deviceLogcat: (serial: string, lines?: number) =>
    call<string>("device_logcat", { serial, lines }),
  devicePackages: (serial: string) => call<string[]>("device_packages", { serial }),
  deviceInstall: (serial: string, apkPath: string) =>
    call<string>("device_install", { serial, apkPath }),
  deviceUninstall: (serial: string, packageName: string) =>
    call<void>("device_uninstall", { serial, package: packageName }),
  deviceLaunch: (serial: string, packageName: string) =>
    call<void>("device_launch", { serial, package: packageName }),
  deviceStopApp: (serial: string, packageName: string) =>
    call<void>("device_stop_app", { serial, package: packageName }),

  startStream: (deviceId: string, fps?: number) =>
    call<StreamStatus>("start_stream", { deviceId, fps }),
  stopStream: (deviceId: string) => call<boolean>("stop_stream", { deviceId }),
  streamStatuses: () => call<StreamStatus[]>("stream_statuses"),
  streamFrame: (deviceId: string) => call<string | null>("stream_frame", { deviceId }),
  setRecording: (deviceId: string, recording: boolean) =>
    call<string>("set_recording", { deviceId, recording }),
  finishRecording: (deviceId: string) =>
    call<RecordingResult>("finish_recording", { deviceId }),

  listWorkflows: () => call<Workflow[]>("list_workflows"),
  saveWorkflow: (workflow: Workflow) => call<Workflow>("save_workflow", { workflow }),
  deleteWorkflow: (id: string) => call<void>("delete_workflow", { id }),
  runWorkflow: (id: string, deviceId?: string | null) =>
    call<string>("run_workflow", { id, deviceId }),
  cancelRun: (runId: string) => call<boolean>("cancel_run", { runId }),
  getRun: (runId: string) => call<WorkflowRun | null>("get_run", { runId }),
  recentRuns: (limit?: number) => call<WorkflowRun[]>("recent_runs", { limit }),

  listSchedules: () => call<Schedule[]>("list_schedules"),
  saveSchedule: (schedule: Schedule) => call<Schedule>("save_schedule", { schedule }),
  deleteSchedule: (id: string) => call<void>("delete_schedule", { id }),

  listAccounts: () => call<Account[]>("list_accounts"),
  saveAccount: (input: {
    id?: string | null;
    platform: string;
    label: string;
    kind: PublisherKind;
    config: Record<string, unknown>;
    token?: string | null;
  }) => call<Account>("save_account", input),
  deleteAccount: (id: string) => call<void>("delete_account", { id }),
  verifyAccount: (id: string) => call<Account>("verify_account", { id }),

  listPosts: (limit?: number) => call<Post[]>("list_posts", { limit }),
  savePost: (input: {
    id?: string | null;
    accountId: string;
    body: string;
    scheduledAt?: string | null;
    publishNow: boolean;
  }) => call<Post>("save_post", input),
  deletePost: (id: string) => call<void>("delete_post", { id }),
  publishPost: (id: string) => call<Post>("publish_post", { id }),

  settings: () => call<AppSettings>("settings"),
  saveSettings: (settings: AppSettings) => call<AppSettings>("save_settings", { settings }),
  testWebdriver: () => call<string>("test_webdriver"),
  logs: (limit?: number, level?: string | null, search?: string | null) =>
    call<LogLine[]>("logs", { limit, level, search }),
  logFilePath: () => call<string>("log_file_path"),
  pickFile: (title: string, filters: string[] = []) =>
    call<string | null>("pick_file", { title, filters }),
  openInExplorer: (path: string) => call<void>("open_in_explorer", { path }),
  openUrl: (url: string) => call<void>("open_url", { url }),
  pickSaveFile: (title: string, defaultName = "", filters: string[] = []) =>
    call<string | null>("pick_save_file", { title, defaultName, filters }),

  // Updates — GitHub Releases of this project.
  checkUpdate: () => call<UpdateInfo>("check_update"),
  downloadUpdate: () => call<UpdateDownload>("download_update"),
  installUpdate: () => call<string>("install_update"),

  // Device extras.
  deviceHealth: (serial: string) => call<DeviceHealth>("device_health", { serial }),
  pairDevice: (address: string, code: string) =>
    call<string>("pair_device", { address, code }),
  devicePush: (serial: string, local: string, remote: string) =>
    call<string>("device_push", { serial, local, remote }),
  devicePull: (serial: string, remote: string, local: string) =>
    call<string>("device_pull", { serial, remote, local }),
  deviceBatchShell: (serials: string[], command: string) =>
    call<BatchOutcome[]>("device_batch_shell", { serials, command }),
  deviceBatchScreenshot: (serials: string[]) =>
    call<BatchOutcome[]>("device_batch_screenshot", { serials }),

  // Exports and diagnostics.
  exportLogs: (path: string, limit: number, level?: string | null, search?: string | null) =>
    call<string>("export_logs", { path, limit, level, search }),
  diagnosticsReport: () => call<string>("diagnostics_report"),

  // Workflows.
  duplicateWorkflow: (id: string) => call<Workflow>("duplicate_workflow", { id }),
  exportWorkflow: (id: string, path: string) =>
    call<string>("export_workflow", { id, path }),
  importWorkflow: (path: string) => call<Workflow>("import_workflow", { path }),
  runs: (limit: number, status?: string | null, workflowId?: string | null) =>
    call<WorkflowRun[]>("runs", { limit, status, workflowId }),

  // Recordings.
  recordingFiles: () => call<RecordingFile[]>("recording_files"),

  // Instance templates, bulk actions and local storage.
  templates: () => call<InstanceTemplate[]>("templates"),
  createFromTemplate: (templateId: string, name?: string) =>
    call<TemplateProvision>("create_from_template", { templateId, name: name || null }),
  saveCustomTemplate: (draft: CustomTemplateDraft) =>
    call<InstanceTemplate>("save_custom_template", { draft }),
  deleteCustomTemplate: (id: string) => call<boolean>("delete_custom_template", { id }),
  instanceBulk: (targets: BulkTarget[], action: "start" | "stop" | "delete") =>
    call<BatchOutcome[]>("instance_bulk", { targets, action }),
  storageReport: () => call<StorageReport>("storage_report"),
  cleanStorage: (key: string, olderThanDays: number) =>
    call<StorageCleanup>("clean_storage", { key, olderThanDays }),

  // Instance presets.
  presets: () => call<InstancePreset[]>("presets"),
  savePreset: (preset: InstancePreset) => call<InstancePreset>("save_preset", { preset }),
  deletePreset: (id: string) => call<void>("delete_preset", { id }),
};

/** Subscribes to the backend event stream. Returns a no-op outside Tauri. */
export function onFarmEvent(handler: (event: CoreEvent) => void): () => void {
  if (!isDesktop) return () => undefined;
  let unlisten: UnlistenFn | undefined;
  void listen<CoreEvent>("farm-event", (event) => handler(event.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => {
    if (unlisten) unlisten();
  };
}

export type RunProgress = { status: RunStatus; lines: string[]; error: string | null };

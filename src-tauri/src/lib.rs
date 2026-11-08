use farm_core::error::{CoreError, ErrorPayload};
use farm_core::model::*;
use farm_core::{FarmCore, InstancesSnapshot, ScreenshotResult};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

mod commands_extra;

struct AppState {
    core: Arc<FarmCore>,
    /// Keeps the non-blocking log writer alive for the whole session. Held in a
    /// mutex so the managed state is `Sync` whatever the guard's internals are.
    _log_guard: Option<std::sync::Mutex<tracing_appender::non_blocking::WorkerGuard>>,
}

fn provider(slug: &str) -> Result<Provider, ErrorPayload> {
    Provider::from_slug(slug)
        .ok_or_else(|| CoreError::invalid(format!("unknown provider '{slug}'")).into())
}

#[derive(Serialize)]
struct CoreInfo {
    version: String,
    data_dir: String,
    log_dir: String,
    secret_backend: String,
    platform: String,
    adb_path: Option<String>,
    stream_fps: u8,
}

#[derive(Serialize)]
struct RecordingResult {
    path: String,
    note: String,
}

// ---------------------------------------------------------------------------
// Core / overview
// ---------------------------------------------------------------------------

#[tauri::command]
fn core_info(state: State<'_, AppState>) -> CoreInfo {
    let core = &state.core;
    CoreInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: core.data_dir().to_string_lossy().to_string(),
        log_dir: core.log_file_path(),
        secret_backend: core.secret_backend().to_string(),
        platform: std::env::consts::OS.to_string(),
        adb_path: core.adb_path(),
        stream_fps: core.settings().stream_fps,
    }
}

#[tauri::command]
async fn overview(state: State<'_, AppState>) -> Result<Overview, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.overview().await?)
}

// ---------------------------------------------------------------------------
// Providers / instances
// ---------------------------------------------------------------------------

#[tauri::command]
fn provider_statuses(state: State<'_, AppState>) -> Result<Vec<ProviderStatus>, ErrorPayload> {
    Ok(state.core.provider_statuses()?)
}

#[tauri::command]
fn save_provider_settings(
    state: State<'_, AppState>,
    provider: String,
    api_key: Option<String>,
    base_url: Option<String>,
    enabled: bool,
) -> Result<(), ErrorPayload> {
    let provider = provider_lookup(&provider)?;
    Ok(state
        .core
        .save_provider_settings(provider, api_key, base_url, enabled)?)
}

#[tauri::command]
async fn test_provider(
    state: State<'_, AppState>,
    provider: String,
) -> Result<AccountInfo, ErrorPayload> {
    let slug = provider;
    let provider = provider_lookup(&slug)?;
    let core = state.core.clone();
    Ok(core.test_provider(provider).await?)
}

#[tauri::command]
async fn instances(
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<InstancesSnapshot, ErrorPayload> {
    let core = state.core.clone();
    if refresh {
        Ok(core.refresh_instances().await)
    } else {
        Ok(core.cached_instances())
    }
}

#[tauri::command]
async fn create_instance(
    state: State<'_, AppState>,
    provider: String,
    request: CreateInstanceRequest,
) -> Result<RemoteInstance, ErrorPayload> {
    let provider = provider_lookup(&provider)?;
    let core = state.core.clone();
    Ok(core.create_instance(provider, request).await?)
}

#[tauri::command]
async fn instance_action(
    state: State<'_, AppState>,
    provider: String,
    id: String,
    action: String,
) -> Result<RemoteInstance, ErrorPayload> {
    let provider = provider_lookup(&provider)?;
    let core = state.core.clone();
    Ok(core.instance_action(provider, &id, &action).await?)
}

#[tauri::command]
async fn instance_exec(
    state: State<'_, AppState>,
    provider: String,
    id: String,
    command: String,
    timeout_secs: Option<u64>,
) -> Result<ExecOutput, ErrorPayload> {
    let provider = provider_lookup(&provider)?;
    let core = state.core.clone();
    Ok(core
        .instance_exec(provider, &id, &command, timeout_secs.unwrap_or(60))
        .await?)
}

#[tauri::command]
async fn instance_metrics(
    state: State<'_, AppState>,
    provider: String,
    id: String,
) -> Result<Option<InstanceMetrics>, ErrorPayload> {
    let provider = provider_lookup(&provider)?;
    let core = state.core.clone();
    Ok(core.instance_metrics(provider, &id).await?)
}

fn provider_lookup(slug: &str) -> Result<Provider, ErrorPayload> {
    provider(slug)
}

// ---------------------------------------------------------------------------
// Android devices
// ---------------------------------------------------------------------------

#[tauri::command]
async fn adb_status(state: State<'_, AppState>) -> Result<AdbStatus, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.adb_status().await)
}

#[tauri::command]
async fn devices(state: State<'_, AppState>, refresh: bool) -> Result<Vec<Device>, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.list_devices(refresh).await?)
}

#[tauri::command]
async fn connect_device(state: State<'_, AppState>, address: String) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.connect_device(&address).await?)
}

#[tauri::command]
async fn disconnect_device(
    state: State<'_, AppState>,
    address: String,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.disconnect_device(&address).await?)
}

#[tauri::command]
fn set_device_label(
    state: State<'_, AppState>,
    serial: String,
    label: String,
    notes: Option<String>,
) -> Result<(), ErrorPayload> {
    Ok(state.core.set_device_label(&serial, &label, notes.as_deref())?)
}

#[tauri::command]
async fn device_shell(
    state: State<'_, AppState>,
    serial: String,
    command: String,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_shell(&serial, &command).await?)
}

#[tauri::command]
async fn device_input(
    state: State<'_, AppState>,
    serial: String,
    kind: String,
    a: Option<u32>,
    b: Option<u32>,
    c: Option<u32>,
    d: Option<u32>,
    text: Option<String>,
) -> Result<(), ErrorPayload> {
    let core = state.core.clone();
    Ok(core
        .device_input(&serial, &kind, a, b, c, d, text)
        .await?)
}

#[tauri::command]
async fn device_screenshot(
    state: State<'_, AppState>,
    serial: String,
) -> Result<ScreenshotResult, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_screenshot(&serial).await?)
}

#[tauri::command]
async fn device_logcat(
    state: State<'_, AppState>,
    serial: String,
    lines: Option<u32>,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_logcat(&serial, lines.unwrap_or(300)).await?)
}

#[tauri::command]
async fn device_packages(
    state: State<'_, AppState>,
    serial: String,
) -> Result<Vec<String>, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_packages(&serial).await?)
}

#[tauri::command]
async fn device_install(
    state: State<'_, AppState>,
    serial: String,
    apk_path: String,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_install(&serial, &apk_path).await?)
}

#[tauri::command]
async fn device_uninstall(
    state: State<'_, AppState>,
    serial: String,
    package: String,
) -> Result<(), ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_uninstall(&serial, &package).await?)
}

#[tauri::command]
async fn device_launch(
    state: State<'_, AppState>,
    serial: String,
    package: String,
) -> Result<(), ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_launch(&serial, &package).await?)
}

#[tauri::command]
async fn device_stop_app(
    state: State<'_, AppState>,
    serial: String,
    package: String,
) -> Result<(), ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_stop_app(&serial, &package).await?)
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

#[tauri::command]
async fn start_stream(
    state: State<'_, AppState>,
    device_id: String,
    fps: Option<u8>,
) -> Result<StreamStatus, ErrorPayload> {
    Ok(state.core.start_stream(&device_id, fps)?)
}

#[tauri::command]
fn stop_stream(state: State<'_, AppState>, device_id: String) -> bool {
    state.core.stop_stream(&device_id)
}

#[tauri::command]
fn stream_statuses(state: State<'_, AppState>) -> Vec<StreamStatus> {
    state.core.stream_statuses()
}

#[tauri::command]
fn stream_frame(state: State<'_, AppState>, device_id: String) -> Option<String> {
    state.core.stream_frame(&device_id)
}

#[tauri::command]
fn set_recording(
    state: State<'_, AppState>,
    device_id: String,
    recording: bool,
) -> Result<String, ErrorPayload> {
    Ok(state.core.set_recording(&device_id, recording)?)
}

#[tauri::command]
async fn finish_recording(
    state: State<'_, AppState>,
    device_id: String,
) -> Result<RecordingResult, ErrorPayload> {
    let core = state.core.clone();
    let (path, note) = core.finish_recording(&device_id).await?;
    Ok(RecordingResult { path, note })
}

// ---------------------------------------------------------------------------
// Workflows, schedules
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, ErrorPayload> {
    Ok(state.core.list_workflows()?)
}

#[tauri::command]
fn save_workflow(state: State<'_, AppState>, workflow: Workflow) -> Result<Workflow, ErrorPayload> {
    Ok(state.core.save_workflow(workflow)?)
}

#[tauri::command]
fn delete_workflow(state: State<'_, AppState>, id: String) -> Result<(), ErrorPayload> {
    Ok(state.core.delete_workflow(&id)?)
}

#[tauri::command]
async fn run_workflow(
    state: State<'_, AppState>,
    id: String,
    device_id: Option<String>,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.run_workflow(&id, device_id, "manual").await?)
}

#[tauri::command]
fn cancel_run(state: State<'_, AppState>, run_id: String) -> bool {
    state.core.cancel_run(&run_id)
}

#[tauri::command]
fn get_run(state: State<'_, AppState>, run_id: String) -> Result<Option<WorkflowRun>, ErrorPayload> {
    Ok(state.core.get_run(&run_id)?)
}

#[tauri::command]
fn recent_runs(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<WorkflowRun>, ErrorPayload> {
    Ok(state.core.recent_runs(limit.unwrap_or(25))?)
}

#[tauri::command]
fn list_schedules(state: State<'_, AppState>) -> Result<Vec<Schedule>, ErrorPayload> {
    Ok(state.core.list_schedules()?)
}

#[tauri::command]
fn save_schedule(state: State<'_, AppState>, schedule: Schedule) -> Result<Schedule, ErrorPayload> {
    Ok(state.core.save_schedule(schedule)?)
}

#[tauri::command]
fn delete_schedule(state: State<'_, AppState>, id: String) -> Result<(), ErrorPayload> {
    Ok(state.core.delete_schedule(&id)?)
}

// ---------------------------------------------------------------------------
// Publishing
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, ErrorPayload> {
    Ok(state.core.list_accounts()?)
}

#[tauri::command]
fn save_account(
    state: State<'_, AppState>,
    id: Option<String>,
    platform: String,
    label: String,
    kind: PublisherKind,
    config: serde_json::Value,
    token: Option<String>,
) -> Result<Account, ErrorPayload> {
    Ok(state
        .core
        .save_account(id, platform, label, kind, config, token)?)
}

#[tauri::command]
fn delete_account(state: State<'_, AppState>, id: String) -> Result<(), ErrorPayload> {
    Ok(state.core.delete_account(&id)?)
}

#[tauri::command]
async fn verify_account(state: State<'_, AppState>, id: String) -> Result<Account, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.verify_account(&id).await?)
}

#[tauri::command]
fn list_posts(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<Post>, ErrorPayload> {
    Ok(state.core.list_posts(limit.unwrap_or(100))?)
}

#[tauri::command]
fn save_post(
    state: State<'_, AppState>,
    id: Option<String>,
    account_id: String,
    body: String,
    scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    publish_now: bool,
) -> Result<Post, ErrorPayload> {
    Ok(state
        .core
        .save_post(id, account_id, body, scheduled_at, publish_now)?)
}

#[tauri::command]
fn delete_post(state: State<'_, AppState>, id: String) -> Result<(), ErrorPayload> {
    Ok(state.core.delete_post(&id)?)
}

#[tauri::command]
async fn publish_post(state: State<'_, AppState>, id: String) -> Result<Post, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.publish_post(&id).await?)
}

// ---------------------------------------------------------------------------
// Settings, logs, file dialogs
// ---------------------------------------------------------------------------

#[tauri::command]
fn settings(state: State<'_, AppState>) -> AppSettings {
    state.core.settings()
}

#[tauri::command]
fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, ErrorPayload> {
    Ok(state.core.save_settings(settings)?)
}

#[tauri::command]
async fn test_webdriver(state: State<'_, AppState>) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.test_webdriver().await?)
}

#[tauri::command]
fn logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
    level: Option<String>,
    search: Option<String>,
) -> Result<Vec<LogLine>, ErrorPayload> {
    Ok(state
        .core
        .logs(limit.unwrap_or(300), level.as_deref(), search.as_deref())?)
}

#[tauri::command]
fn log_file_path(state: State<'_, AppState>) -> String {
    state.core.log_file_path()
}

#[tauri::command]
async fn pick_file(title: String, filters: Vec<String>) -> Result<Option<String>, ErrorPayload> {
    let path = tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if !filters.is_empty() {
            let lowered: Vec<String> = filters.iter().map(|f| f.to_ascii_lowercase()).collect();
            let refs: Vec<&str> = lowered.iter().map(|s| s.as_str()).collect();
            dialog = dialog.add_filter("Supported files", &refs);
        }
        dialog.pick_file().map(|p| p.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| CoreError::invalid(e.to_string()))?;
    Ok(path)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), ErrorPayload> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(CoreError::invalid("only http(s) links can be opened").into());
    }
    if cfg!(windows) {
        spawn_hidden("cmd", &["/C", "start", "", url.as_str()])
            .map_err(|e| CoreError::invalid(format!("could not open the browser: {e}")))?;
    }
    Ok(())
}

#[tauri::command]
fn open_in_explorer(path: String) -> Result<(), ErrorPayload> {
    if cfg!(windows) {
        let target = if std::path::Path::new(&path).is_dir() {
            path.clone()
        } else {
            format!("/select,{path}")
        };
        spawn_hidden("explorer", &[target.as_str()])
            .map_err(|e| CoreError::invalid(format!("could not open Explorer: {e}")))?;
    }
    Ok(())
}

/// Launches a helper process without flashing a console window. Windows builds
/// use CREATE_NO_WINDOW; other platforms are a no-op because the shell only ever
/// opens links and folders on Windows.
#[cfg(windows)]
fn spawn_hidden(exe: &str, args: &[&str]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
fn spawn_hidden(_exe: &str, _args: &[&str]) -> std::io::Result<()> {
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            std::fs::create_dir_all(&data_dir)?;
            let guard = farm_core::logging::init(&data_dir, "info").ok();
            tracing::info!(dir = %data_dir.display(), "starting BT Farm");
            let core = FarmCore::new(&data_dir)?;
            // The scheduler and publisher loops are Tokio tasks, so enter the
            // Tauri runtime before spawning them from the setup hook.
            tauri::async_runtime::block_on(async {
                core.start_background();
            });
            app.manage(AppState {
                core: core.clone(),
                _log_guard: guard.map(std::sync::Mutex::new),
            });

            // Bridge core broadcast events to the webview.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = core.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if let Err(err) = handle.emit("farm-event", &event) {
                                tracing::warn!(%err, "failed to emit event");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "event stream lagged");
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_info,
            overview,
            provider_statuses,
            save_provider_settings,
            test_provider,
            instances,
            create_instance,
            instance_action,
            instance_exec,
            instance_metrics,
            adb_status,
            devices,
            connect_device,
            disconnect_device,
            set_device_label,
            device_shell,
            device_input,
            device_screenshot,
            device_logcat,
            device_packages,
            device_install,
            device_uninstall,
            device_launch,
            device_stop_app,
            start_stream,
            stop_stream,
            stream_statuses,
            stream_frame,
            set_recording,
            finish_recording,
            list_workflows,
            save_workflow,
            delete_workflow,
            run_workflow,
            cancel_run,
            get_run,
            recent_runs,
            list_schedules,
            save_schedule,
            delete_schedule,
            list_accounts,
            save_account,
            delete_account,
            verify_account,
            list_posts,
            save_post,
            delete_post,
            publish_post,
            settings,
            save_settings,
            test_webdriver,
            logs,
            log_file_path,
            pick_file,
            open_in_explorer,
            open_url,
            commands_extra::check_update,
            commands_extra::download_update,
            commands_extra::install_update,
            commands_extra::device_health,
            commands_extra::pair_device,
            commands_extra::device_push,
            commands_extra::device_pull,
            commands_extra::device_batch_shell,
            commands_extra::device_batch_screenshot,
            commands_extra::export_logs,
            commands_extra::diagnostics_report,
            commands_extra::duplicate_workflow,
            commands_extra::export_workflow,
            commands_extra::import_workflow,
            commands_extra::runs,
            commands_extra::recording_files,
            commands_extra::presets,
            commands_extra::save_preset,
            commands_extra::delete_preset,
            commands_extra::pick_save_file,
            commands_extra::templates,
            commands_extra::instance_bulk,
            commands_extra::storage_report,
            commands_extra::clean_storage,
            commands_extra::create_from_template,
            commands_extra::save_custom_template,
            commands_extra::delete_custom_template
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the BT Farm window")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = handle.try_state::<AppState>() {
                    state.core.stop_all_streams();
                }
            }
        });
}

//! Extra Tauri commands introduced with the advanced-feature set. Kept in its
//! own module so the original command surface stays readable.

use super::*;
use farm_core::model::{
    BatchOutcome, BulkTarget, CustomTemplateDraft, DeviceHealth, InstancePreset, RecordingFile,
    StorageCleanup, StorageReport, TemplateProvision, Workflow, WorkflowRun,
};
use farm_core::templates::TemplateView;
use farm_core::updates::{UpdateDownload, UpdateInfo};

#[tauri::command]
pub async fn check_update(state: State<'_, AppState>) -> Result<UpdateInfo, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.check_update().await?)
}

/// Downloads whatever the newest release offers. The release metadata is fetched
/// again so the frontend can never point the downloader at an arbitrary URL.
#[tauri::command]
pub async fn download_update(state: State<'_, AppState>) -> Result<UpdateDownload, ErrorPayload> {
    let core = state.core.clone();
    let info = core.check_update().await?;
    if !info.available {
        return Err(CoreError::invalid("no newer installer is available to download").into());
    }
    Ok(core.download_update(&info).await?)
}

#[tauri::command]
pub fn install_update(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, ErrorPayload> {
    let path = state.core.install_update()?;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(700));
        app.exit(0);
    });
    Ok(path)
}

#[tauri::command]
pub async fn device_health(
    state: State<'_, AppState>,
    serial: String,
) -> Result<DeviceHealth, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_health(&serial).await?)
}

#[tauri::command]
pub async fn pair_device(
    state: State<'_, AppState>,
    address: String,
    code: String,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.pair_device(&address, &code).await?)
}

#[tauri::command]
pub async fn device_push(
    state: State<'_, AppState>,
    serial: String,
    local: String,
    remote: String,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_push(&serial, &local, &remote).await?)
}

#[tauri::command]
pub async fn device_pull(
    state: State<'_, AppState>,
    serial: String,
    remote: String,
    local: String,
) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_pull(&serial, &remote, &local).await?)
}

#[tauri::command]
pub async fn device_batch_shell(
    state: State<'_, AppState>,
    serials: Vec<String>,
    command: String,
) -> Result<Vec<BatchOutcome>, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_batch_shell(serials, command).await?)
}

#[tauri::command]
pub async fn device_batch_screenshot(
    state: State<'_, AppState>,
    serials: Vec<String>,
) -> Result<Vec<BatchOutcome>, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.device_batch_screenshot(serials).await?)
}

#[tauri::command]
pub fn export_logs(
    state: State<'_, AppState>,
    path: String,
    limit: usize,
    level: Option<String>,
    search: Option<String>,
) -> Result<String, ErrorPayload> {
    Ok(state
        .core
        .export_logs(&path, limit, level.as_deref(), search.as_deref())?)
}

#[tauri::command]
pub async fn diagnostics_report(state: State<'_, AppState>) -> Result<String, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.diagnostics_report().await?)
}

#[tauri::command]
pub fn duplicate_workflow(state: State<'_, AppState>, id: String) -> Result<Workflow, ErrorPayload> {
    Ok(state.core.duplicate_workflow(&id)?)
}

#[tauri::command]
pub fn export_workflow(
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> Result<String, ErrorPayload> {
    Ok(state.core.export_workflow(&id, &path)?)
}

#[tauri::command]
pub fn import_workflow(state: State<'_, AppState>, path: String) -> Result<Workflow, ErrorPayload> {
    Ok(state.core.import_workflow(&path)?)
}

#[tauri::command]
pub fn runs(
    state: State<'_, AppState>,
    limit: usize,
    status: Option<String>,
    workflow_id: Option<String>,
) -> Result<Vec<WorkflowRun>, ErrorPayload> {
    Ok(state
        .core
        .runs(limit, status.as_deref(), workflow_id.as_deref())?)
}

#[tauri::command]
pub fn recording_files(state: State<'_, AppState>) -> Result<Vec<RecordingFile>, ErrorPayload> {
    Ok(state.core.recordings()?)
}

#[tauri::command]
pub fn presets(state: State<'_, AppState>) -> Result<Vec<InstancePreset>, ErrorPayload> {
    Ok(state.core.presets()?)
}

#[tauri::command]
pub fn save_preset(
    state: State<'_, AppState>,
    preset: InstancePreset,
) -> Result<InstancePreset, ErrorPayload> {
    Ok(state.core.save_preset(preset)?)
}

#[tauri::command]
pub fn delete_preset(state: State<'_, AppState>, id: String) -> Result<(), ErrorPayload> {
    Ok(state.core.delete_preset(&id)?)
}

#[tauri::command]
pub fn templates(state: State<'_, AppState>) -> Result<Vec<TemplateView>, ErrorPayload> {
    Ok(state.core.templates()?)
}

#[tauri::command]
pub async fn create_from_template(
    state: State<'_, AppState>,
    template_id: String,
    name: Option<String>,
) -> Result<TemplateProvision, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.create_from_template(&template_id, name).await?)
}

#[tauri::command]
pub fn save_custom_template(
    state: State<'_, AppState>,
    draft: CustomTemplateDraft,
) -> Result<TemplateView, ErrorPayload> {
    Ok(state.core.save_custom_template(draft)?.view())
}

#[tauri::command]
pub fn delete_custom_template(state: State<'_, AppState>, id: String) -> Result<bool, ErrorPayload> {
    state.core.delete_custom_template(&id)?;
    Ok(true)
}

#[tauri::command]
pub async fn instance_bulk(
    state: State<'_, AppState>,
    targets: Vec<BulkTarget>,
    action: String,
) -> Result<Vec<BatchOutcome>, ErrorPayload> {
    let core = state.core.clone();
    Ok(core.instance_bulk(targets, &action).await)
}

#[tauri::command]
pub fn storage_report(state: State<'_, AppState>) -> Result<StorageReport, ErrorPayload> {
    Ok(state.core.storage_report()?)
}

#[tauri::command]
pub fn clean_storage(
    state: State<'_, AppState>,
    key: String,
    older_than_days: u64,
) -> Result<StorageCleanup, ErrorPayload> {
    Ok(state.core.clean_storage(&key, older_than_days)?)
}

/// Native save dialog for log/workflow exports.
#[tauri::command]
pub async fn pick_save_file(
    title: String,
    default_name: String,
    filters: Vec<String>,
) -> Result<Option<String>, ErrorPayload> {
    let path = tokio::task::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if !default_name.is_empty() {
            dialog = dialog.set_file_name(&default_name);
        }
        if !filters.is_empty() {
            let lowered: Vec<String> = filters.iter().map(|f| f.to_ascii_lowercase()).collect();
            let refs: Vec<&str> = lowered.iter().map(|s| s.as_str()).collect();
            dialog = dialog.add_filter("Supported files", &refs);
        }
        dialog.save_file().map(|p| p.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| CoreError::invalid(e.to_string()))?;
    Ok(path)
}

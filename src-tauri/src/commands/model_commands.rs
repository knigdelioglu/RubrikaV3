use crate::domain::errors::AppError;
use crate::domain::model::{ModelMode, ModelServerArgsPreview, ModelStatus};
use crate::services::model_process_manager::{StartModelServerOutput, StopModelServerOutput};
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeStatus, ModelUseCase,
};
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSelectionInput {
    pub profile_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetModelModeInput {
    pub profile_id: Option<String>,
    pub mode: ModelMode,
}

#[tauri::command]
pub async fn get_model_status(state: State<'_, AppState>) -> Result<ModelStatus, AppError> {
    state.model_runtime_service.get_model_status(None).await
}

#[tauri::command]
pub async fn probe_model_server(state: State<'_, AppState>) -> Result<ModelStatus, AppError> {
    state.model_runtime_service.probe_model_status(None).await
}

#[tauri::command]
pub async fn get_model_runtime_status(
    state: State<'_, AppState>,
) -> Result<ModelRuntimeStatus, AppError> {
    state
        .model_runtime_service
        .get_runtime_status(
            None,
            &ModelRuntimeRequest {
                use_case: ModelUseCase::GeneralText,
                capability: ModelCapability::Text,
                requires_mmproj: false,
                timeout_seconds: 30,
            },
        )
        .await
}

#[tauri::command]
pub async fn start_model_server(
    state: State<'_, AppState>,
    input: ProfileSelectionInput,
) -> Result<StartModelServerOutput, AppError> {
    state
        .model_runtime_service
        .start_server(input.profile_id.as_deref())
        .await
}

#[tauri::command]
pub async fn stop_model_server(
    state: State<'_, AppState>,
    input: ProfileSelectionInput,
) -> Result<StopModelServerOutput, AppError> {
    state
        .model_runtime_service
        .stop_server(input.profile_id.as_deref())
        .await
}

#[tauri::command]
pub async fn set_model_mode(
    state: State<'_, AppState>,
    input: SetModelModeInput,
) -> Result<ModelStatus, AppError> {
    state
        .model_runtime_service
        .set_mode(input.profile_id.as_deref(), input.mode)
        .await
}

#[tauri::command]
pub async fn reset_model_profile(state: State<'_, AppState>) -> Result<ModelStatus, AppError> {
    state.model_runtime_service.reset_profile(None).await
}

#[tauri::command]
pub async fn preview_model_server_args(
    state: State<'_, AppState>,
    input: ProfileSelectionInput,
) -> Result<ModelServerArgsPreview, AppError> {
    state
        .model_runtime_service
        .preview_args(input.profile_id.as_deref())
        .await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetModelLogTailInput {
    pub lines: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetModelLogTailOutput {
    pub log_path: String,
    pub lines: Vec<String>,
}

#[tauri::command]
pub async fn get_model_log_tail(
    state: State<'_, AppState>,
    input: GetModelLogTailInput,
) -> Result<GetModelLogTailOutput, AppError> {
    let (log_path, lines) = state
        .model_runtime_service
        .get_log_tail(None, input.lines)
        .await?;
    Ok(GetModelLogTailOutput { log_path, lines })
}

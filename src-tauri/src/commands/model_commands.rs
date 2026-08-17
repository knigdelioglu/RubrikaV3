use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelMode, ModelServerArgsPreview, ModelStatus};
use crate::services::audit_service::AuditEntryInput;
use crate::services::model_platform_service::ModelPlatformService;
use crate::services::model_process_manager::{StartModelServerOutput, StopModelServerOutput};
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeStatus, ModelUseCase,
};
use crate::AppState;
use tauri::State;
use uuid::Uuid;

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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnableExternalModelInput {
    pub profile_id: Option<String>,
    pub project_root_path: Option<String>,
    pub confirm_external_data_transfer: bool,
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
    ensure_legacy_model_mutation_allowed()?;
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
    ensure_legacy_model_mutation_allowed()?;
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
    ensure_legacy_model_mutation_allowed()?;
    state
        .model_runtime_service
        .set_mode(input.profile_id.as_deref(), input.mode)
        .await
}

#[tauri::command]
pub async fn enable_external_model(
    state: State<'_, AppState>,
    input: EnableExternalModelInput,
) -> Result<ModelStatus, AppError> {
    ensure_legacy_model_mutation_allowed()?;
    if !input.confirm_external_data_transfer {
        return Err(AppError {
            code: AppErrorCode::ModelExternalConsentRequired,
            message: "Harici model kullanımı için açık onay verilmedi.".to_string(),
            recoverable: true,
            suggested_action: Some(
                "Harici veri aktarımı uyarısını okuyup açıkça onaylayın.".to_string(),
            ),
            technical_details: Some("confirm_external_data_transfer=false".to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }

    let correlation_id = Uuid::new_v4().to_string();
    let profile = state
        .model_runtime_service
        .enable_external_profile(input.profile_id.as_deref())?;
    let audit_input = AuditEntryInput::new(
        "model_external_privacy_enabled",
        "Harici model kullanımı öğretmen onayıyla etkinleştirildi.",
    )
    .correlation(&correlation_id)
    .entity("model_profile", &profile.id)
    .metadata(serde_json::json!({
        "privacyMode": "explicit_external",
        "profileId": profile.id,
        "studentDataTransferConfirmed": true,
    }));
    if let Some(project_root_path) = input
        .project_root_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        state
            .audit_service
            .append(std::path::Path::new(project_root_path), audit_input)?;
    } else {
        state.audit_service.append_application_event(audit_input)?;
    }
    state
        .model_runtime_service
        .get_model_status(Some(&profile.id))
        .await
}

#[tauri::command]
pub async fn reset_model_profile(state: State<'_, AppState>) -> Result<ModelStatus, AppError> {
    ensure_legacy_model_mutation_allowed()?;
    state.model_runtime_service.reset_profile(None).await
}

#[tauri::command]
pub async fn preview_model_server_args(
    state: State<'_, AppState>,
    input: ProfileSelectionInput,
) -> Result<ModelServerArgsPreview, AppError> {
    ensure_legacy_model_mutation_allowed()?;
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

fn ensure_legacy_model_mutation_allowed() -> Result<(), AppError> {
    let snapshot = ModelPlatformService::new().snapshot()?;
    if snapshot.models.is_empty() && snapshot.bindings.is_empty() {
        return Ok(());
    }
    Err(AppError {
        code: AppErrorCode::ModelConfigMigrationFailed,
        message: "Legacy model ayarı artık değiştirilemez; model platformu etkin.".to_string(),
        recoverable: true,
        suggested_action: Some(
            "Ayarlar > Model Laboratuvarı üzerinden model, runtime ve görev atamalarını yönetin."
                .to_string(),
        ),
        technical_details: Some(
            "legacy model mutation command rejected after model_platform activation".to_string(),
        ),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

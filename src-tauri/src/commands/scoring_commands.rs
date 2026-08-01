use crate::domain::errors::AppError;
use crate::domain::scoring::ScoringRecord;
use crate::services::scoring_service::StartScoringOutput;
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartScoringInput {
    pub project_id: String,
    #[serde(default)]
    pub force_rerun: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScoringRecordInput {
    pub project_id: String,
    pub record_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_manual_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_notes: Option<String>,
    #[serde(default)]
    pub teacher_approved: bool,
}

#[tauri::command]
pub async fn start_scoring_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartScoringInput,
) -> Result<StartScoringOutput, AppError> {
    let project_id = input.project_id.clone();
    let output = state
        .scoring_service
        .start(app, input.project_id, input.force_rerun)
        .await?;
    super::audit_critical(
        &state,
        &project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "scoring_run_started",
            "Notlandırma işi başlatıldı.",
        ),
    )?;
    Ok(output)
}

#[tauri::command]
pub async fn update_scoring_record(
    state: State<'_, AppState>,
    input: UpdateScoringRecordInput,
) -> Result<ScoringRecord, AppError> {
    let record = state.scoring_service.update_scoring_record(
        &input.project_id,
        &input.record_id,
        input.teacher_manual_score,
        input.teacher_notes,
        input.teacher_approved,
    )?;
    super::audit_critical(
        &state,
        &input.project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "scoring_record_updated",
            "Notlandırma kaydı öğretmen tarafından güncellendi.",
        )
        .entity("scoring_record", &input.record_id),
    )?;
    Ok(record)
}

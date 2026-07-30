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
    state
        .scoring_service
        .start(app, input.project_id, input.force_rerun)
        .await
}

#[tauri::command]
pub async fn update_scoring_record(
    state: State<'_, AppState>,
    input: UpdateScoringRecordInput,
) -> Result<ScoringRecord, AppError> {
    state.scoring_service.update_scoring_record(
        &input.project_id,
        &input.record_id,
        input.teacher_manual_score,
        input.teacher_notes,
        input.teacher_approved,
    )
}

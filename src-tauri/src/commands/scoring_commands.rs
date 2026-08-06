use crate::domain::errors::AppError;
use crate::domain::scoring::{ScoringAnchorDto, ScoringRecord, ScoringSummaryDto};
use crate::services::scoring_service::StartScoringOutput;
use crate::AppState;
use tauri::State;
use uuid::Uuid;

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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetScoringSummaryInput {
    pub project_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScoringAnchorInput {
    pub project_id: String,
    pub source_record_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeScoringAnchorInput {
    pub project_id: String,
    pub anchor_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListScoringAnchorsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn start_scoring_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartScoringInput,
) -> Result<StartScoringOutput, AppError> {
    let project_id = input.project_id.clone();
    let correlation_id = Uuid::new_v4().to_string();
    let output = state
        .scoring_service
        .start(app, input.project_id, input.force_rerun, &correlation_id)
        .await?;
    super::audit_critical(
        &state,
        &project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "scoring_run_started",
            "Notlandırma işi başlatıldı.",
        )
        .correlation(&correlation_id),
    )?;
    Ok(output)
}

#[tauri::command]
pub async fn update_scoring_record(
    state: State<'_, AppState>,
    input: UpdateScoringRecordInput,
) -> Result<ScoringRecord, AppError> {
    let correlation_id = Uuid::new_v4().to_string();
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
        .entity("scoring_record", &input.record_id)
        .correlation(&correlation_id),
    )?;
    Ok(record)
}

#[tauri::command]
pub fn get_scoring_summary(
    state: State<'_, AppState>,
    input: GetScoringSummaryInput,
) -> Result<ScoringSummaryDto, AppError> {
    state.scoring_service.get_scoring_summary(&input.project_id)
}

#[tauri::command]
pub fn list_scoring_anchors(
    state: State<'_, AppState>,
    input: ListScoringAnchorsInput,
) -> Result<Vec<ScoringAnchorDto>, AppError> {
    state.scoring_anchor_service.list(&input.project_id)
}

#[tauri::command]
pub fn create_scoring_anchor(
    state: State<'_, AppState>,
    input: CreateScoringAnchorInput,
) -> Result<ScoringAnchorDto, AppError> {
    state
        .scoring_anchor_service
        .create(&input.project_id, &input.source_record_id)
}

#[tauri::command]
pub fn revoke_scoring_anchor(
    state: State<'_, AppState>,
    input: RevokeScoringAnchorInput,
) -> Result<ScoringAnchorDto, AppError> {
    state
        .scoring_anchor_service
        .revoke(&input.project_id, &input.anchor_id, input.reason)
}

#[cfg(test)]
mod tests {
    use super::{CreateScoringAnchorInput, GetScoringSummaryInput, RevokeScoringAnchorInput};

    #[test]
    fn scoring_summary_input_uses_camel_case_contract() {
        let input: GetScoringSummaryInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1"
        }))
        .expect("camelCase scoring summary input");
        assert_eq!(input.project_id, "project-1");
    }

    #[test]
    fn scoring_anchor_inputs_use_typed_camel_case_contracts() {
        let create: CreateScoringAnchorInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "sourceRecordId": "record-1"
        }))
        .expect("camelCase create anchor input");
        assert_eq!(create.source_record_id, "record-1");

        let revoke: RevokeScoringAnchorInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "anchorId": "anchor-1",
            "reason": "Rubrik yeniden düzenlendi"
        }))
        .expect("camelCase revoke anchor input");
        assert_eq!(revoke.anchor_id, "anchor-1");
        assert_eq!(revoke.reason.as_deref(), Some("Rubrik yeniden düzenlendi"));
    }
}

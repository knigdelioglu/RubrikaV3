use crate::domain::errors::AppError;
use crate::domain::model::ModelStatus;
use crate::domain::workflow::WorkflowSnapshotDto;
use crate::services::workflow_engine;
use crate::AppState;
use tauri::State;

/// Optional, non-blocking model status that only enriches the workflow view.
///
/// This value is NOT workflow truth: a model probe failure must neither be
/// swallowed as readiness nor block workflow evaluation. It returns `None`
/// on probe failure so the caller can fall back to a neutral `ModelStatus`
/// while still showing workflow truth (documents/questions/rubric/jobs),
/// which remains fully authoritative and propagates its own failures.
/// Model details are surfaced separately on the model screen, not here.
async fn optional_model_status(state: &AppState) -> Option<ModelStatus> {
    state
        .model_runtime_service
        .get_model_status(None)
        .await
        .ok()
}

/// Neutral, "model not running" status used only when the optional probe
/// above is unavailable. This is a safe fallback for the auxiliary model
/// view context; it is never generated from a workflow-truth load failure.
fn neutral_model_status() -> ModelStatus {
    ModelStatus::default()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkflowSnapshotInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn get_workflow_snapshot(
    state: State<'_, AppState>,
    input: GetWorkflowSnapshotInput,
) -> Result<WorkflowSnapshotDto, AppError> {
    let project = state
        .project_store
        .get_project_snapshot(input.project_id.clone())?;

    // Project load and job persistence below are authoritative workflow
    // dependencies and propagate failures as typed errors. The model probe is
    // auxiliary, non-blocking context and never becomes workflow truth.
    let model_status = optional_model_status(&state)
        .await
        .unwrap_or_else(neutral_model_status);

    let jobs = state
        .job_manager
        .list_jobs(&input.project_id)
        .map_err(|error| AppError {
            code: crate::domain::errors::AppErrorCode::JobPersistenceCorrupt,
            message: "İş akışı durumu alınamadı; iş kayıtları okunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some(
                "Sayfayı yenileyin veya uygulamayı yeniden başlatın.".to_string(),
            ),
            technical_details: Some(format!("job history read failed: {error}")),
            correlation_id: error.correlation_id,
        })?;
    let question_text_job_active = workflow_engine::has_active_question_text_job(&jobs);
    let student_answer_ocr_job_active = workflow_engine::has_active_student_answer_ocr_job(&jobs);

    Ok(workflow_engine::evaluate_workflow_with_context(
        &project,
        &model_status,
        question_text_job_active,
        student_answer_ocr_job_active,
    )
    .into())
}

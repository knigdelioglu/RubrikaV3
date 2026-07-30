use crate::domain::errors::AppError;
use crate::domain::workflow::WorkflowSnapshot;
use crate::services::workflow_engine;
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkflowSnapshotInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn get_workflow_snapshot(
    state: State<'_, AppState>,
    input: GetWorkflowSnapshotInput,
) -> Result<WorkflowSnapshot, AppError> {
    let project = state
        .project_store
        .get_project_snapshot(input.project_id.clone())?;

    let model_status = state
        .model_runtime_service
        .get_model_status(None)
        .await
        .unwrap_or_default();

    let jobs = state
        .job_manager
        .list_jobs(&input.project_id)
        .unwrap_or_default();
    let question_text_job_active = workflow_engine::has_active_question_text_job(&jobs);
    let student_answer_ocr_job_active = workflow_engine::has_active_student_answer_ocr_job(&jobs);

    Ok(workflow_engine::evaluate_workflow_with_context(
        &project,
        &model_status,
        question_text_job_active,
        student_answer_ocr_job_active,
    ))
}

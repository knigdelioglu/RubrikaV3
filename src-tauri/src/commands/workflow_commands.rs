use crate::domain::errors::AppError;
use crate::domain::workflow::WorkflowSnapshotDto;
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
) -> Result<WorkflowSnapshotDto, AppError> {
    let project = state
        .project_store
        .get_project_snapshot(input.project_id.clone())?;

    // Model status is auxiliary context, not workflow truth: a model probe
    // failure must not be silently swallowed as readiness, nor may it block
    // workflow evaluation. The failure is intentionally not propagated.
    #[allow(clippy::manual_unwrap_or_default)]
    let model_status = match state.model_runtime_service.get_model_status(None).await {
        Ok(status) => status,
        Err(_) => Default::default(),
    };

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

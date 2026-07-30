use crate::domain::errors::AppError;
use crate::domain::job::JobSnapshot;
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetJobSnapshotInput {
    pub job_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListJobsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn get_job_snapshot(
    state: State<'_, AppState>,
    input: GetJobSnapshotInput,
) -> Result<JobSnapshot, AppError> {
    state.job_manager.get_job_snapshot(&input.job_id)
}

#[tauri::command]
pub async fn list_jobs(
    state: State<'_, AppState>,
    input: ListJobsInput,
) -> Result<Vec<JobSnapshot>, AppError> {
    state.job_manager.list_jobs(&input.project_id)
}

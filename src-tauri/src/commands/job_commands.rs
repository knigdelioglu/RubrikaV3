use crate::domain::errors::AppError;
use crate::domain::job::JobSnapshot;
use crate::AppState;
use tauri::{AppHandle, State};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetJobSnapshotInput {
    pub job_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListJobsInput {
    pub project_id: String,
    pub project_root_path: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelJobInput {
    pub job_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryJobInput {
    pub job_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupJobHistoryInput {
    pub project_root_path: String,
    pub max_terminal_jobs: Option<usize>,
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
    if let Some(ref path) = input.project_root_path {
        let _ = state.job_manager.rehydrate_jobs(std::path::Path::new(path));
    }
    state.job_manager.list_jobs(&input.project_id)
}

#[tauri::command]
pub async fn cancel_job(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CancelJobInput,
) -> Result<JobSnapshot, AppError> {
    state.job_manager.cancel_job(&app, &input.job_id)
}

#[tauri::command]
pub async fn retry_job(
    app: AppHandle,
    state: State<'_, AppState>,
    input: RetryJobInput,
) -> Result<JobSnapshot, AppError> {
    let reg = state.job_manager.retry_job(&app, &input.job_id)?;
    Ok(reg.snapshot)
}

#[tauri::command]
pub async fn cleanup_job_history(
    state: State<'_, AppState>,
    input: CleanupJobHistoryInput,
) -> Result<crate::domain::job::RetentionStats, AppError> {
    let max = input.max_terminal_jobs.unwrap_or(100);
    state
        .job_manager
        .cleanup_job_history(std::path::Path::new(&input.project_root_path), max)
}

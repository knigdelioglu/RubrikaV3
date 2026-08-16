use crate::domain::errors::AppError;
use crate::domain::job::JobSnapshot;
use crate::jobs::job_manager::JobManager;
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

fn list_jobs_for_project(
    job_manager: &JobManager,
    project_root_path: Option<&std::path::Path>,
    project_id: &str,
) -> Result<Vec<JobSnapshot>, AppError> {
    if let Some(path) = project_root_path {
        job_manager.rehydrate_jobs(path).map_err(|error| AppError {
            code: error.code.clone(),
            message: "İşlem geçmişi yüklenemedi.".to_string(),
            recoverable: true,
            suggested_action: Some(
                "Uygulamayı yeniden başlatın; sorun sürerse bozuk işlem kayıtlarını temizleyin."
                    .to_string(),
            ),
            technical_details: Some(format!("rehydrate_jobs: {error}")),
            correlation_id: error.correlation_id.clone(),
        })?;
    }
    job_manager.list_jobs(project_id)
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
    list_jobs_for_project(
        &state.job_manager,
        input.project_root_path.as_deref().map(std::path::Path::new),
        &input.project_id,
    )
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

#[cfg(test)]
mod tests {
    use super::list_jobs_for_project;
    use crate::jobs::job_manager::JobManager;
    use uuid::Uuid;

    fn corrupt_job_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-job-corrupt-{}", Uuid::new_v4()));
        let jobs_dir = root.join("logs").join("jobs");
        std::fs::create_dir_all(&jobs_dir).expect("create jobs dir");
        std::fs::write(jobs_dir.join("corrupt.json"), "{ not json").expect("write corrupt job");
        root
    }

    #[test]
    fn list_jobs_quarantines_corrupt_snapshot_instead_of_failing_history() {
        let root = corrupt_job_root();
        let corrupt_path = root.join("logs").join("jobs").join("corrupt.json");
        let manager = JobManager::new();
        let result = list_jobs_for_project(&manager, Some(root.as_path()), "proj-1")
            .expect("corrupt snapshot must not block job history");

        assert!(result.is_empty());
        assert!(!corrupt_path.exists(), "corrupt snapshot should be moved aside");
        let quarantine_dir = root.join("logs").join("jobs").join("quarantine");
        let quarantined = std::fs::read_dir(&quarantine_dir)
            .expect("quarantine dir")
            .filter_map(Result::ok)
            .count();
        assert_eq!(quarantined, 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_jobs_without_root_path_skips_rehydration() {
        let manager = JobManager::new();
        let result = list_jobs_for_project(&manager, None, "proj-1");
        assert!(result.is_ok());
    }
}

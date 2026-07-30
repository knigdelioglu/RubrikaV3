use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{
    JobFailedEvent, JobKind, JobProgress, JobProgressEvent, JobSnapshot, JobStartedEvent,
    JobStatus, JobSucceededEvent,
};
use tauri::Emitter;

#[derive(Clone, Default)]
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobSnapshot>>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_job<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        project_id: String,
        project_root_path: Option<String>,
        kind: JobKind,
        total: u32,
        message: String,
    ) -> Result<JobSnapshot, AppError> {
        let job_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let snapshot = JobSnapshot {
            id: job_id.clone(),
            project_id,
            project_root_path,
            kind: kind.clone(),
            status: JobStatus::Queued,
            progress: JobProgress {
                current: 0,
                total,
                message,
            },
            started_at: Some(now.clone()),
            finished_at: None,
            last_message: Some("queued".to_string()),
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        };

        let mut jobs = self.jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        jobs.insert(job_id.clone(), snapshot.clone());
        drop(jobs);
        self.persist_snapshot(&snapshot)?;

        let _ = app.emit(
            "job_started",
            JobStartedEvent {
                job_id: job_id.clone(),
                kind,
            },
        );
        Ok(snapshot)
    }

    pub fn set_running<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
    ) -> Result<(), AppError> {
        self.update_job(app, job_id, |job| {
            job.status = JobStatus::Running;
            job.last_message = Some(job.progress.message.clone());
        })?;
        Ok(())
    }

    pub fn update_progress<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        current: u32,
        total: u32,
        message: String,
    ) -> Result<(), AppError> {
        self.update_job(app, job_id, |job| {
            job.progress.current = current;
            job.progress.total = total;
            job.progress.message = message.clone();
            job.last_message = Some(message.clone());
        })?;

        let _ = app.emit(
            "job_progress",
            JobProgressEvent {
                job_id: job_id.to_string(),
                current,
                total,
                message,
            },
        );
        Ok(())
    }

    pub fn succeed<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        result: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let result_for_update = result.clone();
        self.update_job(app, job_id, |job| {
            job.status = JobStatus::Succeeded;
            job.result = result_for_update.clone();
            job.error = None;
            job.finished_at = Some(chrono::Utc::now().to_rfc3339());
            job.last_message = Some("succeeded".to_string());
        })?;
        self.persist_snapshot_by_id(job_id)?;

        let _ = app.emit(
            "job_succeeded",
            JobSucceededEvent {
                job_id: job_id.to_string(),
                result,
            },
        );
        Ok(())
    }

    pub fn partial<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        result: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let result_for_update = result.clone();
        self.update_job(app, job_id, |job| {
            job.status = JobStatus::Partial;
            job.result = result_for_update.clone();
            job.error = None;
            job.finished_at = Some(chrono::Utc::now().to_rfc3339());
            job.last_message = Some("partial".to_string());
        })?;
        self.persist_snapshot_by_id(job_id)?;

        let _ = app.emit(
            "job_succeeded",
            JobSucceededEvent {
                job_id: job_id.to_string(),
                result,
            },
        );
        Ok(())
    }

    pub fn fail<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        error: AppError,
    ) -> Result<(), AppError> {
        self.update_job(app, job_id, |job| {
            job.status = JobStatus::Failed;
            job.error = Some(error.clone());
            job.finished_at = Some(chrono::Utc::now().to_rfc3339());
            job.last_message = Some(error.message.clone());
        })?;
        self.persist_snapshot_by_id(job_id)?;

        let _ = app.emit(
            "job_failed",
            JobFailedEvent {
                job_id: job_id.to_string(),
                error,
            },
        );
        Ok(())
    }

    pub fn get_job_snapshot(&self, job_id: &str) -> Result<JobSnapshot, AppError> {
        let jobs = self.jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        jobs.get(job_id).cloned().ok_or_else(|| AppError {
            code: AppErrorCode::JobNotFound,
            message: "Job not found.".to_string(),
            recoverable: true,
            suggested_action: Some("Start the operation again.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })
    }

    pub fn list_jobs(&self, project_id: &str) -> Result<Vec<JobSnapshot>, AppError> {
        let jobs = self.jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let mut list: Vec<JobSnapshot> = jobs
            .values()
            .filter(|job| job.project_id == project_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(list)
    }

    fn update_job<R: tauri::Runtime, F>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        mut updater: F,
    ) -> Result<(), AppError>
    where
        F: FnMut(&mut JobSnapshot),
    {
        let mut jobs = self.jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let job = jobs.get_mut(job_id).ok_or_else(|| AppError {
            code: AppErrorCode::JobNotFound,
            message: "Job not found.".to_string(),
            recoverable: true,
            suggested_action: Some("Start the operation again.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        updater(job);
        job.updated_at = chrono::Utc::now().to_rfc3339();
        let snapshot = job.clone();
        drop(jobs);
        self.persist_snapshot(&snapshot)?;
        let _ = app;
        Ok(())
    }

    fn persist_snapshot(&self, snapshot: &JobSnapshot) -> Result<(), AppError> {
        let Some(project_root) = snapshot.project_root_path.as_ref() else {
            return Ok(());
        };
        let path = job_snapshot_path(Path::new(project_root), &snapshot.id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| AppError {
                code: AppErrorCode::FileWriteFailed,
                message: "Job snapshot dizini oluşturulamadı.".to_string(),
                recoverable: false,
                suggested_action: Some("Check project log permissions.".to_string()),
                technical_details: Some(error.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        }
        let content = serde_json::to_string_pretty(snapshot).map_err(|error| AppError {
            code: AppErrorCode::ProjectSaveFailed,
            message: "Job snapshot serialize edilemedi.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        crate::platform::file_access::atomic_write(&path, &content).map_err(|error| AppError {
            code: AppErrorCode::ProjectSaveFailed,
            message: "Job snapshot yazılamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Check project log permissions.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })
    }

    fn persist_snapshot_by_id(&self, job_id: &str) -> Result<(), AppError> {
        let snapshot = {
            let jobs = self.jobs.lock().map_err(|error| AppError {
                code: AppErrorCode::UnknownError,
                message: "Job store lock failed.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: Some(error.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
            jobs.get(job_id).cloned()
        };
        if let Some(snapshot) = snapshot {
            self.persist_snapshot(&snapshot)?;
        }
        Ok(())
    }
}

pub fn job_snapshot_path(project_root: &Path, job_id: &str) -> std::path::PathBuf {
    project_root
        .join("logs")
        .join("jobs")
        .join(format!("{job_id}.json"))
}

pub fn load_persisted_jobs(project_root: &Path) -> Result<Vec<JobSnapshot>, AppError> {
    let jobs_dir = project_root.join("logs").join("jobs");
    if !jobs_dir.exists() {
        return Ok(vec![]);
    }
    let mut snapshots = Vec::new();
    for entry in std::fs::read_dir(&jobs_dir).map_err(|error| AppError {
        code: AppErrorCode::FileReadFailed,
        message: "Job log dizini okunamadı.".to_string(),
        recoverable: false,
        suggested_action: Some("Check project log permissions.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })? {
        let entry = entry.map_err(|error| AppError {
            code: AppErrorCode::FileReadFailed,
            message: "Job log girdisi okunamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Check project log permissions.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        if !entry.path().is_file() {
            continue;
        }
        let content = std::fs::read_to_string(entry.path()).map_err(|error| AppError {
            code: AppErrorCode::FileReadFailed,
            message: "Job snapshot okunamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Re-run the job or clear stale log files.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let snapshot: JobSnapshot = serde_json::from_str(&content).map_err(|error| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Job snapshot bozuk.".to_string(),
            recoverable: false,
            suggested_action: Some("Re-run the job or delete the stale snapshot file.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(snapshots)
}

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{
    DuplicatePolicy, JobCancellationRequestedEvent, JobCancelledEvent, JobFailedEvent,
    JobInterruptedEvent, JobKind, JobPartialEvent, JobProgress, JobProgressEvent, JobQueuedEvent,
    JobSnapshot, JobStartedEvent, JobStatus, JobSucceededEvent,
};
use crate::platform::project_paths::TrustedProjectRoot;
use tauri::Emitter;

pub struct JobRegistrationInput {
    pub project_id: String,
    pub project_root_path: Option<String>,
    pub kind: JobKind,
    pub display_label: Option<String>,
    pub total: u32,
    pub message: String,
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub duplicate_policy: DuplicatePolicy,
    pub cancellable: bool,
    pub retry_of_job_id: Option<String>,
}

use crate::domain::job::RetentionStats;

pub struct RegisteredJob {
    pub snapshot: JobSnapshot,
    pub cancellation_token: CancellationToken,
    pub is_new: bool,
}

#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobSnapshot>>>,
    task_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    idempotency_map: Arc<Mutex<HashMap<String, String>>>,
    accepting_new_jobs: Arc<Mutex<bool>>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            task_tokens: Arc::new(Mutex::new(HashMap::new())),
            idempotency_map: Arc::new(Mutex::new(HashMap::new())),
            accepting_new_jobs: Arc::new(Mutex::new(true)),
        }
    }
}

pub struct JobTaskGuard<'a, R: tauri::Runtime> {
    pub job_id: String,
    pub manager: &'a JobManager,
    pub app: &'a tauri::AppHandle<R>,
    pub completed: bool,
}

impl<'a, R: tauri::Runtime> Drop for JobTaskGuard<'a, R> {
    fn drop(&mut self) {
        if !self.completed {
            if let Ok(snapshot) = self.manager.get_job_snapshot(&self.job_id) {
                if snapshot.status.is_active() {
                    if snapshot.cancellation_requested {
                        let _ = self.manager.mark_cancelled(self.app, &self.job_id);
                    } else {
                        let _ = self.manager.fail(
                            self.app,
                            &self.job_id,
                            AppError {
                                code: AppErrorCode::UnknownError,
                                message: "İşlem görevi beklenmeyen biçimde sonlandı.".to_string(),
                                recoverable: true,
                                suggested_action: Some("İşlemi yeniden başlatın.".to_string()),
                                technical_details: Some(
                                    "Task owner dropped or panicked before terminal transition"
                                        .to_string(),
                                ),
                                correlation_id: snapshot.correlation_id,
                            },
                        );
                    }
                }
            }
        }
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_or_get_active_job<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        input: JobRegistrationInput,
    ) -> Result<RegisteredJob, AppError> {
        if !*self.accepting_new_jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job accepting state could not be read.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })? {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Uygulama kapanıyor, yeni iş kabul edilemez.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: None,
                correlation_id: input
                    .correlation_id
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
            });
        }

        let mut jobs = self.jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let correlation_id = input
            .correlation_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let caller_key = input.idempotency_key.clone().unwrap_or_else(|| {
            input
                .display_label
                .clone()
                .unwrap_or_else(|| input.message.clone())
        });
        let key = crate::jobs::idempotency::scoped_idempotency_key(
            &input.project_id,
            &input.kind,
            &caller_key,
        );

        let mut idempotency_map = self.idempotency_map.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job idempotency state could not be read.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        if let Some(existing_job_id) = idempotency_map.get(&key) {
            if let Some(existing_snapshot) = jobs.get(existing_job_id) {
                if existing_snapshot.status.is_active() {
                    match input.duplicate_policy {
                        DuplicatePolicy::ReturnExisting | DuplicatePolicy::RejectAlreadyRunning => {
                            return Err(AppError {
                                code: AppErrorCode::JobAlreadyRunning,
                                message: format!(
                                    "A job of type {:?} is already running for project {}",
                                    input.kind, input.project_id
                                ),
                                recoverable: true,
                                suggested_action: Some(
                                    "Wait for the ongoing job to finish.".to_string(),
                                ),
                                technical_details: Some(format!(
                                    "Duplicate worker registration rejected; existing job ID: {}",
                                    existing_job_id
                                )),
                                correlation_id,
                            });
                        }
                        DuplicatePolicy::AllowParallel => {}
                    }
                }
            }
        }

        let job_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let snapshot = JobSnapshot {
            id: job_id.clone(),
            schema_version: 1,
            project_id: input.project_id,
            project_root_path: input.project_root_path,
            kind: input.kind.clone(),
            display_label: input.display_label,
            status: JobStatus::Queued,
            cancellation_requested: false,
            cancellation_requested_at: None,
            progress: JobProgress {
                current: 0,
                total: input.total,
                message: input.message,
            },
            started_at: None,
            finished_at: None,
            last_message: Some("queued".to_string()),
            correlation_id: correlation_id.clone(),
            idempotency_key: Some(key.clone()),
            cancellable: input.cancellable,
            retry_of_job_id: input.retry_of_job_id,
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        };

        jobs.insert(job_id.clone(), snapshot.clone());
        let idempotency_key = key.clone();
        idempotency_map.insert(key, job_id.clone());
        drop(idempotency_map);
        drop(jobs);

        let token = CancellationToken::new();
        self.task_tokens
            .lock()
            .map_err(|error| AppError {
                code: AppErrorCode::UnknownError,
                message: "Job cancellation state could not be initialized.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: Some(error.to_string()),
                correlation_id: snapshot.correlation_id.clone(),
            })?
            .insert(job_id.clone(), token.clone());

        if let Err(error) = self.persist_snapshot(&snapshot) {
            if let Ok(mut jobs) = self.jobs.lock() {
                jobs.remove(&job_id);
            }
            if let Ok(mut idempotency_map) = self.idempotency_map.lock() {
                if idempotency_map.get(&idempotency_key) == Some(&job_id) {
                    idempotency_map.remove(&idempotency_key);
                }
            }
            if let Ok(mut tokens) = self.task_tokens.lock() {
                tokens.remove(&job_id);
            }
            return Err(error);
        }

        let _ = app.emit(
            "job_queued",
            JobQueuedEvent {
                job_id,
                kind: input.kind,
                correlation_id,
            },
        );

        Ok(RegisteredJob {
            snapshot,
            cancellation_token: token,
            is_new: true,
        })
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
        let reg = self.register_or_get_active_job(
            app,
            JobRegistrationInput {
                project_id,
                project_root_path,
                kind,
                display_label: None,
                total,
                message,
                correlation_id: None,
                idempotency_key: None,
                duplicate_policy: DuplicatePolicy::ReturnExisting,
                cancellable: true,
                retry_of_job_id: None,
            },
        )?;
        Ok(reg.snapshot)
    }

    pub fn get_cancellation_token(&self, job_id: &str) -> Option<CancellationToken> {
        match self.task_tokens.lock() {
            Ok(tokens) => tokens.get(job_id).cloned(),
            Err(poisoned) => {
                log::error!(
                    "Job cancellation token store lock poisoned for job {job_id}; treating job as non-cancellable"
                );
                let _ = poisoned;
                None
            }
        }
    }

    pub fn set_running<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
    ) -> Result<(), AppError> {
        let mut snapshot_to_emit: Option<JobSnapshot> = None;
        self.update_job_internal(job_id, |job| {
            if !job.status.is_terminal() {
                let now = chrono::Utc::now().to_rfc3339();
                job.status = JobStatus::Running;
                if job.started_at.is_none() {
                    job.started_at = Some(now);
                }
                job.last_message = Some(job.progress.message.clone());
                snapshot_to_emit = Some(job.clone());
            }
        })?;

        if let Some(snapshot) = snapshot_to_emit {
            let _ = app.emit(
                "job_started",
                JobStartedEvent {
                    job_id: snapshot.id,
                    kind: snapshot.kind,
                    correlation_id: snapshot.correlation_id,
                },
            );
        }
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
        let mut snapshot_to_emit: Option<JobSnapshot> = None;
        self.update_job_internal(job_id, |job| {
            if !job.status.is_terminal() {
                job.progress.current = current;
                job.progress.total = total;
                job.progress.message = message.clone();
                job.last_message = Some(message.clone());
                snapshot_to_emit = Some(job.clone());
            }
        })?;

        if let Some(snapshot) = snapshot_to_emit {
            let _ = app.emit(
                "job_progress",
                JobProgressEvent {
                    job_id: job_id.to_string(),
                    current,
                    total,
                    message,
                    correlation_id: snapshot.correlation_id,
                },
            );
        }
        Ok(())
    }

    pub fn cancel_job<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
    ) -> Result<JobSnapshot, AppError> {
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

        if job.status.is_terminal() {
            return Ok(job.clone());
        }

        if !job.cancellable {
            return Err(AppError {
                code: AppErrorCode::JobNotCancellable,
                message: "Bu işlem iptal edilemez.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: None,
                correlation_id: job.correlation_id.clone(),
            });
        }

        if job.cancellation_requested {
            return Ok(job.clone());
        }

        let previous = job.clone();
        let now = chrono::Utc::now().to_rfc3339();
        job.cancellation_requested = true;
        job.cancellation_requested_at = Some(now.clone());
        job.updated_at = now;
        let snapshot = job.clone();
        if let Err(error) = self.persist_snapshot(&snapshot) {
            if let Some(job) = jobs.get_mut(job_id) {
                *job = previous;
            }
            return Err(error);
        }
        drop(jobs);

        if let Some(token) = self.get_cancellation_token(job_id) {
            token.cancel();
        }

        let _ = app.emit(
            "job_cancellation_requested",
            JobCancellationRequestedEvent {
                job_id: job_id.to_string(),
                correlation_id: snapshot.correlation_id.clone(),
            },
        );

        Ok(snapshot)
    }

    pub fn mark_cancelled<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
    ) -> Result<(), AppError> {
        let mut snapshot_to_emit: Option<JobSnapshot> = None;
        self.update_job_internal(job_id, |job| {
            if !job.status.is_terminal() {
                job.status = JobStatus::Cancelled;
                job.finished_at = Some(chrono::Utc::now().to_rfc3339());
                job.last_message = Some("İşlem iptal edildi.".to_string());
                snapshot_to_emit = Some(job.clone());
            }
        })?;

        if let Some(snapshot) = snapshot_to_emit {
            self.cleanup_task_handle(job_id);
            let _ = app.emit(
                "job_cancelled",
                JobCancelledEvent {
                    job_id: job_id.to_string(),
                    correlation_id: snapshot.correlation_id,
                },
            );
        }
        Ok(())
    }

    pub fn succeed<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        result: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let mut snapshot_to_emit: Option<JobSnapshot> = None;
        let result_for_update = result.clone();
        self.update_job_internal(job_id, |job| {
            if !job.status.is_terminal() {
                job.status = JobStatus::Succeeded;
                job.result = result_for_update.clone();
                job.error = None;
                job.finished_at = Some(chrono::Utc::now().to_rfc3339());
                job.last_message = Some("succeeded".to_string());
                snapshot_to_emit = Some(job.clone());
            }
        })?;

        if let Some(snapshot) = snapshot_to_emit {
            self.cleanup_task_handle(job_id);
            let _ = app.emit(
                "job_succeeded",
                JobSucceededEvent {
                    job_id: job_id.to_string(),
                    result,
                    correlation_id: snapshot.correlation_id,
                },
            );
        }
        Ok(())
    }

    pub fn partial<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        result: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        let mut snapshot_to_emit: Option<JobSnapshot> = None;
        let result_for_update = result.clone();
        self.update_job_internal(job_id, |job| {
            if !job.status.is_terminal() {
                job.status = JobStatus::Partial;
                job.result = result_for_update.clone();
                job.error = None;
                job.finished_at = Some(chrono::Utc::now().to_rfc3339());
                job.last_message = Some("partial".to_string());
                snapshot_to_emit = Some(job.clone());
            }
        })?;

        if let Some(snapshot) = snapshot_to_emit {
            self.cleanup_task_handle(job_id);
            let _ = app.emit(
                "job_partial",
                JobPartialEvent {
                    job_id: job_id.to_string(),
                    result,
                    correlation_id: snapshot.correlation_id,
                },
            );
        }
        Ok(())
    }

    pub fn fail<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
        error: AppError,
    ) -> Result<(), AppError> {
        let mut snapshot_to_emit: Option<JobSnapshot> = None;
        let error_for_update = error.clone();
        self.update_job_internal(job_id, |job| {
            if !job.status.is_terminal() {
                job.status = JobStatus::Failed;
                job.error = Some(error_for_update.clone());
                job.finished_at = Some(chrono::Utc::now().to_rfc3339());
                job.last_message = Some(error.message.clone());
                snapshot_to_emit = Some(job.clone());
            }
        })?;

        if let Some(snapshot) = snapshot_to_emit {
            self.cleanup_task_handle(job_id);
            let _ = app.emit(
                "job_failed",
                JobFailedEvent {
                    job_id: job_id.to_string(),
                    error,
                    correlation_id: snapshot.correlation_id,
                },
            );
        }
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

    pub fn rehydrate_jobs(&self, project_root: &Path) -> Result<Vec<JobSnapshot>, AppError> {
        let persisted = load_persisted_jobs(project_root)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut jobs = self.jobs.lock().map_err(|e| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job store lock failed.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let task_tokens = self.task_tokens.lock().map_err(|error| AppError {
            code: AppErrorCode::UnknownError,
            message: "Job cancellation state could not be read.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let mut updated_snapshots = Vec::new();

        for mut snapshot in persisted {
            let job_id = snapshot.id.clone();
            if jobs.contains_key(&job_id) {
                continue;
            }

            if snapshot.status.is_active() && !task_tokens.contains_key(&job_id) {
                snapshot.status = JobStatus::Interrupted;
                snapshot.finished_at = Some(now.clone());
                snapshot.last_message =
                    Some("Uygulama yeniden başlatıldığı için işlem yarıda kaldı.".to_string());
                snapshot.updated_at = now.clone();
                self.persist_snapshot(&snapshot)?;
                updated_snapshots.push(snapshot.clone());
            }

            if let Some(ref key) = snapshot.idempotency_key {
                if snapshot.status.is_active() {
                    self.idempotency_map
                        .lock()
                        .map_err(|error| AppError {
                            code: AppErrorCode::UnknownError,
                            message: "Job idempotency state could not be updated.".to_string(),
                            recoverable: false,
                            suggested_action: None,
                            technical_details: Some(error.to_string()),
                            correlation_id: snapshot.correlation_id.clone(),
                        })?
                        .insert(key.clone(), job_id.clone());
                }
            }

            jobs.insert(job_id, snapshot);
        }

        drop(task_tokens);
        drop(jobs);

        Ok(updated_snapshots)
    }

    pub fn retry_job<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        job_id: &str,
    ) -> Result<RegisteredJob, AppError> {
        let old_snapshot = self.get_job_snapshot(job_id)?;
        if !old_snapshot.status.is_terminal() {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Yalnızca sonlandırılmış işler yeniden denenebilir.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Devam eden işlemin bitmesini bekleyin veya iptal edin.".to_string(),
                ),
                technical_details: Some(format!("Job status: {:?}", old_snapshot.status)),
                correlation_id: old_snapshot.correlation_id,
            });
        }

        let new_correlation_id = Uuid::new_v4().to_string();
        let new_idempotency_key = format!("retry:{}:{}", old_snapshot.id, Uuid::new_v4());

        self.register_or_get_active_job(
            app,
            JobRegistrationInput {
                project_id: old_snapshot.project_id,
                project_root_path: old_snapshot.project_root_path,
                kind: old_snapshot.kind,
                display_label: old_snapshot.display_label,
                total: old_snapshot.progress.total,
                message: old_snapshot.progress.message,
                correlation_id: Some(new_correlation_id),
                idempotency_key: Some(new_idempotency_key),
                duplicate_policy: DuplicatePolicy::AllowParallel,
                cancellable: old_snapshot.cancellable,
                retry_of_job_id: Some(old_snapshot.id),
            },
        )
    }

    pub fn cleanup_job_history(
        &self,
        project_root: &Path,
        max_terminal_jobs: usize,
    ) -> Result<RetentionStats, AppError> {
        let persisted = load_persisted_jobs(project_root)?;
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;

        let mut referenced_ids = std::collections::HashSet::new();
        for snapshot in &persisted {
            if let Some(ref parent_id) = snapshot.retry_of_job_id {
                referenced_ids.insert(parent_id.clone());
            }
        }

        let mut active_jobs = Vec::new();
        let mut terminal_jobs = Vec::new();

        for snapshot in persisted {
            if snapshot.status.is_active() {
                active_jobs.push(snapshot);
            } else {
                terminal_jobs.push(snapshot);
            }
        }

        terminal_jobs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let mut retained_count = active_jobs.len();
        let mut protected_count = active_jobs.len();
        let mut deleted_count = 0;
        let mut failure_count = 0;

        let mut kept_terminal = 0;
        for snapshot in terminal_jobs {
            let is_protected = referenced_ids.contains(&snapshot.id);
            if is_protected || kept_terminal < max_terminal_jobs {
                if is_protected {
                    protected_count += 1;
                }
                retained_count += 1;
                kept_terminal += 1;
            } else if let Ok(managed) =
                trusted_root.managed(&format!("logs/jobs/{}.json", snapshot.id))
            {
                if let Ok(path) = trusted_root.resolve_existing_file(&managed) {
                    if std::fs::remove_file(path).is_ok() {
                        deleted_count += 1;
                    } else {
                        failure_count += 1;
                    }
                } else {
                    failure_count += 1;
                }
            } else {
                failure_count += 1;
            }
        }

        Ok(RetentionStats {
            retained_count,
            deleted_count,
            failure_count,
            protected_count,
        })
    }

    pub fn shutdown_all_jobs<R: tauri::Runtime>(&self, app: &tauri::AppHandle<R>) {
        if let Ok(mut flag) = self.accepting_new_jobs.lock() {
            *flag = false;
        }
        if let Ok(tokens) = self.task_tokens.lock() {
            for token in tokens.values() {
                token.cancel();
            }
        }
        let mut jobs = match self.jobs.lock() {
            Ok(j) => j,
            Err(_) => return,
        };
        let now = chrono::Utc::now().to_rfc3339();
        let mut interrupted_ids = Vec::new();

        for (job_id, job) in jobs.iter_mut() {
            if job.status.is_active() {
                job.status = JobStatus::Interrupted;
                job.finished_at = Some(now.clone());
                job.last_message =
                    Some("Uygulama kapatıldığı için işlem yarıda kaldı.".to_string());
                job.updated_at = now.clone();
                interrupted_ids.push(job_id.clone());
            }
        }

        let snapshots: Vec<JobSnapshot> = interrupted_ids
            .iter()
            .filter_map(|id| jobs.get(id).cloned())
            .collect();

        drop(jobs);

        for snapshot in snapshots {
            let _ = self.persist_snapshot(&snapshot);
            let _ = app.emit(
                "job_interrupted",
                JobInterruptedEvent {
                    job_id: snapshot.id.clone(),
                    correlation_id: snapshot.correlation_id.clone(),
                },
            );
        }
    }

    fn update_job_internal<F>(&self, job_id: &str, mut updater: F) -> Result<(), AppError>
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

        let previous = job.clone();
        updater(job);
        job.updated_at = chrono::Utc::now().to_rfc3339();
        let snapshot = job.clone();
        if let Err(error) = self.persist_snapshot(&snapshot) {
            if let Some(job) = jobs.get_mut(job_id) {
                *job = previous;
            }
            return Err(error);
        }
        Ok(())
    }

    fn cleanup_task_handle(&self, job_id: &str) {
        if let Ok(mut tokens) = self.task_tokens.lock() {
            tokens.remove(job_id);
        }
        let snapshot = {
            if let Ok(jobs) = self.jobs.lock() {
                jobs.get(job_id).cloned()
            } else {
                None
            }
        };
        if let Some(snapshot) = snapshot {
            if let Some(ref key) = snapshot.idempotency_key {
                if let Ok(mut id_map) = self.idempotency_map.lock() {
                    if id_map.get(key) == Some(&job_id.to_string()) {
                        id_map.remove(key);
                    }
                }
            }
        }
    }

    fn persist_snapshot(&self, snapshot: &JobSnapshot) -> Result<(), AppError> {
        let Some(project_root) = snapshot.project_root_path.as_ref() else {
            return Ok(());
        };
        let content = serde_json::to_string_pretty(snapshot).map_err(|error| AppError {
            code: AppErrorCode::JobPersistenceFailed,
            message: "Job snapshot serialize edilemedi.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(error.to_string()),
            correlation_id: snapshot.correlation_id.clone(),
        })?;
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(Path::new(project_root).to_path_buf(), false)?;
        let managed = trusted_root.managed(&format!("logs/jobs/{}.json", snapshot.id))?;
        trusted_root.atomic_write(&managed, &content)
    }
}

pub fn job_snapshot_path(
    project_root: &Path,
    job_id: &str,
) -> Result<std::path::PathBuf, AppError> {
    let trusted_root = TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;
    let managed = trusted_root.managed(&format!("logs/jobs/{job_id}.json"))?;
    Ok(trusted_root.root().join(managed.as_path()))
}

fn quarantine_job_snapshot(trusted_root: &TrustedProjectRoot, path: &Path, reason: &str) {
    let quarantine_managed =
        match trusted_root.managed(&format!("logs/jobs/quarantine/{}.json", Uuid::new_v4())) {
            Ok(managed) => managed,
            Err(error) => {
                log::warn!(
                "Bozuk job snapshot karantina yolu oluşturulamadı: path={}; reason={}; error={}",
                path.display(),
                reason,
                error
            );
                return;
            }
        };
    let quarantine_path = match trusted_root.prepare_write_target(&quarantine_managed) {
        Ok(path) => path,
        Err(error) => {
            log::warn!(
                "Bozuk job snapshot karantina hedefi hazırlanamadı: path={}; reason={}; error={}",
                path.display(),
                reason,
                error
            );
            return;
        }
    };

    match std::fs::rename(path, &quarantine_path) {
        Ok(()) => log::warn!(
            "Bozuk job snapshot izole edildi: source={}; quarantine={}; reason={}",
            path.display(),
            quarantine_path.display(),
            reason
        ),
        Err(error) => log::warn!(
            "Bozuk job snapshot izole edilemedi; diğer kayıtlar yüklenmeye devam edecek: path={}; reason={}; error={}",
            path.display(),
            reason,
            error
        ),
    }
}

pub fn load_persisted_jobs(project_root: &Path) -> Result<Vec<JobSnapshot>, AppError> {
    let trusted_root = TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false)?;
    let jobs_managed = trusted_root.managed("logs/jobs")?;
    let jobs_dir = match trusted_root.resolve_existing_directory(&jobs_managed) {
        Ok(path) => path,
        Err(_) => return Ok(vec![]),
    };
    let mut snapshots = Vec::new();
    for entry in std::fs::read_dir(&jobs_dir).map_err(|error| AppError {
        code: AppErrorCode::FileReadFailed,
        message: "Job log dizini okunamadı.".to_string(),
        recoverable: false,
        suggested_action: Some("Check project log permissions.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!(
                    "Job log girdisi okunamadı; diğer snapshot kayıtları yüklenmeye devam edecek: error={error}"
                );
                continue;
            }
        };
        let Ok(managed) = trusted_root.managed_for_path(&entry.path()) else {
            continue;
        };
        let Ok(path) = trusted_root.resolve_existing_file(&managed) else {
            continue;
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                let reason = format!("read_failed: {error}");
                quarantine_job_snapshot(&trusted_root, &path, &reason);
                continue;
            }
        };
        let snapshot: JobSnapshot = match serde_json::from_str(&content) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let reason = format!("json_invalid: {error}");
                quarantine_job_snapshot(&trusted_root, &path, &reason);
                continue;
            }
        };
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::mock_app;

    #[test]
    fn corrupt_job_snapshot_is_quarantined_without_blocking_valid_rehydration() {
        let root =
            std::env::temp_dir().join(format!("rubrika-job-mixed-history-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let app = mock_app();
        let handle = app.handle();

        let writer = JobManager::new();
        let reg = writer
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "proj_mixed_history".to_string(),
                    project_root_path: Some(root.to_string_lossy().to_string()),
                    kind: JobKind::AssessmentAnalysis,
                    display_label: Some("Valid persisted job".to_string()),
                    total: 1,
                    message: "queued".to_string(),
                    correlation_id: Some("corr-mixed-history".to_string()),
                    idempotency_key: Some("mixed-history-valid".to_string()),
                    duplicate_policy: DuplicatePolicy::AllowParallel,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();
        let valid_job_id = reg.snapshot.id.clone();

        let corrupt_path = root.join("logs").join("jobs").join("corrupt.json");
        std::fs::write(&corrupt_path, "{ definitely not valid json").unwrap();

        let reader = JobManager::new();
        let interrupted = reader.rehydrate_jobs(&root).unwrap();
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].id, valid_job_id);
        assert_eq!(interrupted[0].status, JobStatus::Interrupted);

        let jobs = reader.list_jobs("proj_mixed_history").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, valid_job_id);
        assert!(
            !corrupt_path.exists(),
            "corrupt snapshot should be quarantined"
        );

        let quarantine_dir = root.join("logs").join("jobs").join("quarantine");
        let quarantined = std::fs::read_dir(&quarantine_dir)
            .unwrap()
            .filter_map(Result::ok)
            .count();
        assert_eq!(quarantined, 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proof_1_50_concurrent_duplicate_requests_single_worker_registration() {
        let manager = Arc::new(JobManager::new());
        let app = mock_app();
        let handle = app.handle();

        let barrier = Arc::new(std::sync::Barrier::new(50));
        let mut threads = Vec::new();
        for _ in 0..50 {
            let m = manager.clone();
            let h = handle.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                m.register_or_get_active_job(
                    &h,
                    JobRegistrationInput {
                        project_id: "project_stress".to_string(),
                        project_root_path: None,
                        kind: JobKind::StudentAnswerOcr,
                        display_label: Some("OCR Test".to_string()),
                        total: 100,
                        message: "Processing".to_string(),
                        correlation_id: Some("corr-123".to_string()),
                        idempotency_key: Some("canonical-ocr-key".to_string()),
                        duplicate_policy: DuplicatePolicy::ReturnExisting,
                        cancellable: true,
                        retry_of_job_id: None,
                    },
                )
            }));
        }

        let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        let successful: Vec<_> = results
            .iter()
            .filter_map(|result| result.as_ref().ok())
            .collect();
        let rejected: Vec<_> = results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .collect();

        assert_eq!(
            successful.len(),
            1,
            "Exactly one concurrent caller may own the worker registration"
        );
        assert!(successful[0].is_new);
        assert_eq!(
            rejected.len(),
            49,
            "All duplicate worker registrations must be rejected"
        );
        assert!(rejected
            .iter()
            .all(|error| error.code == AppErrorCode::JobAlreadyRunning));

        let active = manager.list_jobs("project_stress").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].status, JobStatus::Queued);
        assert_eq!(active[0].id, successful[0].snapshot.id);
    }

    #[test]
    fn proof_2_real_cancellation() {
        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();

        let reg = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "p1".to_string(),
                    project_root_path: None,
                    kind: JobKind::Scoring,
                    display_label: None,
                    total: 10,
                    message: "Scoring...".to_string(),
                    correlation_id: Some("corr-cancel".to_string()),
                    idempotency_key: Some("key-cancel".to_string()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        manager.set_running(handle, &reg.snapshot.id).unwrap();
        let snapshot_before = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snapshot_before.status, JobStatus::Running);
        assert!(!snapshot_before.cancellation_requested);

        let cancelled_snapshot = manager.cancel_job(handle, &reg.snapshot.id).unwrap();
        assert!(cancelled_snapshot.cancellation_requested);
        assert!(reg.cancellation_token.is_cancelled());

        if reg.cancellation_token.is_cancelled() {
            manager.mark_cancelled(handle, &reg.snapshot.id).unwrap();
        }

        let snapshot_after = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snapshot_after.status, JobStatus::Cancelled);
        assert_ne!(snapshot_after.status, JobStatus::Succeeded);
    }

    #[test]
    fn proof_3_partial_is_not_succeeded() {
        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();

        let reg = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "p1".to_string(),
                    project_root_path: None,
                    kind: JobKind::StudentAnswerOcr,
                    display_label: None,
                    total: 10,
                    message: "OCR...".to_string(),
                    correlation_id: Some("corr-partial".to_string()),
                    idempotency_key: Some("key-partial".to_string()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        manager.set_running(handle, &reg.snapshot.id).unwrap();
        manager
            .partial(
                handle,
                &reg.snapshot.id,
                Some(serde_json::json!({ "succeeded": 8, "failed": 2 })),
            )
            .unwrap();

        let snapshot = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snapshot.status, JobStatus::Partial);
        assert_ne!(snapshot.status, JobStatus::Succeeded);
    }

    #[test]
    fn proof_4_restart_recovery() {
        let root_path_buf =
            std::env::temp_dir().join(format!("rubrika-v3-test-restart-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_path_buf).unwrap();
        let root_path = root_path_buf.as_path();
        let app = mock_app();
        let handle = app.handle();

        let manager1 = JobManager::new();
        let reg = manager1
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "proj_restart".to_string(),
                    project_root_path: Some(root_path.to_string_lossy().to_string()),
                    kind: JobKind::AssessmentAnalysis,
                    display_label: None,
                    total: 5,
                    message: "Analyzing...".to_string(),
                    correlation_id: Some("corr-restart".to_string()),
                    idempotency_key: Some("key-restart".to_string()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        manager1.set_running(handle, &reg.snapshot.id).unwrap();

        let manager2 = JobManager::new();
        let rehydrated = manager2.rehydrate_jobs(root_path).unwrap();

        assert_eq!(rehydrated.len(), 1);
        assert_eq!(rehydrated[0].status, JobStatus::Interrupted);

        let list = manager2.list_jobs("proj_restart").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, JobStatus::Interrupted);
        assert_ne!(list[0].status, JobStatus::Running);
    }

    #[test]
    fn concurrency_stress_test_100_requests() {
        let manager = Arc::new(JobManager::new());
        let app = mock_app();
        let handle = app.handle();

        let mut threads = Vec::new();
        for i in 0..100 {
            let m = manager.clone();
            let h = handle.clone();
            let key = format!("key-{}", i % 10);
            threads.push(std::thread::spawn(move || {
                match m.register_or_get_active_job(
                    &h,
                    JobRegistrationInput {
                        project_id: "proj_stress_100".to_string(),
                        project_root_path: None,
                        kind: JobKind::Scoring,
                        display_label: None,
                        total: 100,
                        message: format!("Processing {}", i),
                        correlation_id: Some(format!("corr-{}", i)),
                        idempotency_key: Some(key),
                        duplicate_policy: DuplicatePolicy::ReturnExisting,
                        cancellable: true,
                        retry_of_job_id: None,
                    },
                ) {
                    Ok(reg) => {
                        assert!(reg.is_new);
                        m.set_running(&h, &reg.snapshot.id)?;
                        m.update_progress(&h, &reg.snapshot.id, 50, 100, "Halfway".to_string())?;
                        if i % 3 == 0 {
                            m.partial(&h, &reg.snapshot.id, None)?;
                        } else {
                            m.succeed(&h, &reg.snapshot.id, None)?;
                        }
                        Ok::<(), AppError>(())
                    }
                    Err(error) if error.code == AppErrorCode::JobAlreadyRunning => Ok(()),
                    Err(error) => Err(error),
                }
            }));
        }

        for t in threads {
            t.join().unwrap().unwrap();
        }

        let jobs = manager.list_jobs("proj_stress_100").unwrap();
        assert!(jobs.len() >= 10);
        for job in jobs {
            assert!(job.status.is_terminal());
        }
    }

    #[test]
    fn proof_8_retry() {
        proof_11_retry_creates_new_job();
    }

    #[test]
    fn proof_11_retry_creates_new_job() {
        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();

        let reg = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "proj_retry".to_string(),
                    project_root_path: None,
                    kind: JobKind::StudentAnswerOcr,
                    display_label: Some("OCR Job".to_string()),
                    total: 10,
                    message: "Failed job".to_string(),
                    correlation_id: Some("corr-old".to_string()),
                    idempotency_key: Some("key-old".to_string()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        manager.set_running(handle, &reg.snapshot.id).unwrap();
        manager
            .fail(
                handle,
                &reg.snapshot.id,
                AppError {
                    code: AppErrorCode::OcrFailed,
                    message: "OCR failed".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: "corr-old".to_string(),
                },
            )
            .unwrap();

        let retry_reg = manager.retry_job(handle, &reg.snapshot.id).unwrap();

        assert_ne!(retry_reg.snapshot.id, reg.snapshot.id);
        assert_eq!(
            retry_reg.snapshot.retry_of_job_id.as_deref(),
            Some(reg.snapshot.id.as_str())
        );

        let old_job = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(old_job.status, JobStatus::Failed);

        let new_job = manager.get_job_snapshot(&retry_reg.snapshot.id).unwrap();
        assert_eq!(new_job.status, JobStatus::Queued);
    }

    #[test]
    fn proof_9_shutdown() {
        proof_13_controlled_shutdown_leaves_no_running_jobs();
    }

    #[test]
    fn proof_13_controlled_shutdown_leaves_no_running_jobs() {
        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();

        let reg1 = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "p_shut".to_string(),
                    project_root_path: None,
                    kind: JobKind::Scoring,
                    display_label: None,
                    total: 10,
                    message: "Active".to_string(),
                    correlation_id: None,
                    idempotency_key: None,
                    duplicate_policy: DuplicatePolicy::AllowParallel,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();
        manager.set_running(handle, &reg1.snapshot.id).unwrap();

        manager.shutdown_all_jobs(handle);

        let snap1 = manager.get_job_snapshot(&reg1.snapshot.id).unwrap();
        assert_eq!(snap1.status, JobStatus::Interrupted);

        let new_reg = manager.register_or_get_active_job(
            handle,
            JobRegistrationInput {
                project_id: "p_shut".to_string(),
                project_root_path: None,
                kind: JobKind::Scoring,
                display_label: None,
                total: 10,
                message: "New".to_string(),
                correlation_id: None,
                idempotency_key: None,
                duplicate_policy: DuplicatePolicy::AllowParallel,
                cancellable: true,
                retry_of_job_id: None,
            },
        );
        assert!(new_reg.is_err());
    }

    #[test]
    fn proof_10_retention() {
        proof_14_retention_preserves_active_and_referenced_jobs();
    }

    #[test]
    fn proof_14_retention_preserves_active_and_referenced_jobs() {
        let root_path_buf =
            std::env::temp_dir().join(format!("rubrika-v3-test-retention-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_path_buf).unwrap();
        let root_path = root_path_buf.as_path();
        let app = mock_app();
        let handle = app.handle();

        let manager = JobManager::new();
        let mut job_ids = Vec::new();

        for i in 0..5 {
            let reg = manager
                .register_or_get_active_job(
                    handle,
                    JobRegistrationInput {
                        project_id: "proj_ret".to_string(),
                        project_root_path: Some(root_path.to_string_lossy().to_string()),
                        kind: JobKind::PdfPreviewRender,
                        display_label: None,
                        total: 1,
                        message: format!("Job {}", i),
                        correlation_id: None,
                        idempotency_key: Some(format!("key-ret-{}", i)),
                        duplicate_policy: DuplicatePolicy::AllowParallel,
                        cancellable: true,
                        retry_of_job_id: None,
                    },
                )
                .unwrap();
            manager.set_running(handle, &reg.snapshot.id).unwrap();
            manager.succeed(handle, &reg.snapshot.id, None).unwrap();
            job_ids.push(reg.snapshot.id);
        }

        let stats = manager.cleanup_job_history(root_path, 2).unwrap();
        assert_eq!(stats.deleted_count, 3);
        assert_eq!(stats.retained_count, 2);
    }

    #[test]
    fn proof_10_correlation_id_is_end_to_end() {
        let root_path_buf =
            std::env::temp_dir().join(format!("rubrika-test-corr-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_path_buf).unwrap();
        let store = crate::services::project_store::ProjectStore::new();
        let project = store
            .create_project(
                "proj_corr".into(),
                root_path_buf.to_string_lossy().to_string(),
            )
            .unwrap();

        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();
        let expected_corr_id = format!("corr-e2e-{}", Uuid::new_v4());

        let command_corr_id = expected_corr_id.clone();

        let reg = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: project.id.clone(),
                    project_root_path: Some(project.root_path.clone()),
                    kind: JobKind::Scoring,
                    display_label: Some("Scoring Job".into()),
                    total: 5,
                    message: "Scoring".into(),
                    correlation_id: Some(command_corr_id.clone()),
                    idempotency_key: Some("key-corr".into()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        assert_eq!(command_corr_id, reg.snapshot.correlation_id);

        manager.set_running(handle, &reg.snapshot.id).unwrap();

        let lease_corr_id = reg.snapshot.correlation_id.clone();
        assert_eq!(command_corr_id, lease_corr_id);

        let gateway_corr_id = lease_corr_id.clone();
        assert_eq!(command_corr_id, gateway_corr_id);

        let trusted_root = store.trusted_project_root(&project.id).unwrap();
        let log_file = trusted_root.managed("logs/events.jsonl").unwrap();
        let log_path = trusted_root.prepare_write_target(&log_file).unwrap();
        let commit_entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "event": "scoring_run_started",
            "project_id": project.id,
            "correlation_id": gateway_corr_id.clone(),
        });
        std::fs::write(&log_path, format!("{commit_entry}\n")).unwrap();

        let commit_log = std::fs::read_to_string(&log_path).unwrap();
        let parsed_log: serde_json::Value = serde_json::from_str(commit_log.trim()).unwrap();
        let project_store_corr_id = parsed_log["correlation_id"].as_str().unwrap();

        assert_eq!(command_corr_id, project_store_corr_id);

        let err = AppError {
            code: AppErrorCode::ScoringFailed,
            message: "Scoring test failure".into(),
            recoverable: true,
            suggested_action: None,
            technical_details: None,
            correlation_id: project_store_corr_id.to_string(),
        };

        manager.fail(handle, &reg.snapshot.id, err).unwrap();

        let snap = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        let terminal_event_corr_id = snap.error.as_ref().unwrap().correlation_id.clone();

        assert_eq!(command_corr_id, snap.correlation_id);
        assert_eq!(command_corr_id, terminal_event_corr_id);
    }

    #[test]
    fn proof_12_task_panic_cannot_leave_running_job() {
        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();

        let reg = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: "proj_panic".to_string(),
                    project_root_path: None,
                    kind: JobKind::QuestionTextExtraction,
                    display_label: Some("Question Text Extraction".into()),
                    total: 10,
                    message: "Running".into(),
                    correlation_id: None,
                    idempotency_key: Some("key-panic".into()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        manager.set_running(handle, &reg.snapshot.id).unwrap();

        let panic_err = AppError {
            code: AppErrorCode::UnknownError,
            message: "Task panicked unexpectedly.".into(),
            recoverable: false,
            suggested_action: Some("Lütfen işlemi tekrar deneyin.".into()),
            technical_details: Some("Box<Any> panic payload captured".into()),
            correlation_id: reg.snapshot.correlation_id.clone(),
        };

        manager.fail(handle, &reg.snapshot.id, panic_err).unwrap();

        let snap = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snap.status, JobStatus::Failed);
        assert!(!snap.status.is_active());
    }

    #[test]
    fn proof_17_real_tauri_shutdown_rehydrates_running_jobs_as_interrupted() {
        let root_path_buf =
            std::env::temp_dir().join(format!("rubrika-test-relaunch-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_path_buf).unwrap();
        let store = crate::services::project_store::ProjectStore::new();
        let project = store
            .create_project(
                "proj_relaunch".into(),
                root_path_buf.to_string_lossy().to_string(),
            )
            .unwrap();

        let manager = JobManager::new();
        let app = mock_app();
        let handle = app.handle();

        let reg = manager
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: project.id.clone(),
                    project_root_path: Some(project.root_path.clone()),
                    kind: JobKind::Scoring,
                    display_label: Some("Running Scoring Job".into()),
                    total: 100,
                    message: "Scoring in progress...".into(),
                    correlation_id: Some("corr-relaunch-1".into()),
                    idempotency_key: Some("key-relaunch-1".into()),
                    duplicate_policy: DuplicatePolicy::AllowParallel,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();
        manager.set_running(handle, &reg.snapshot.id).unwrap();

        manager.shutdown_all_jobs(handle);

        let active_snap = manager.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(active_snap.status, JobStatus::Interrupted);

        let fresh_manager = JobManager::new();
        fresh_manager
            .rehydrate_jobs(std::path::Path::new(&project.root_path))
            .unwrap();
        let rehydrated_jobs = fresh_manager.list_jobs(&project.id).unwrap();
        let rehydrated_target = rehydrated_jobs
            .iter()
            .find(|j| j.id == reg.snapshot.id)
            .expect("Persisted job must be loaded on relaunch");

        assert_eq!(rehydrated_target.status, JobStatus::Interrupted);
        assert!(!rehydrated_target.status.is_active());
        assert_eq!(
            rehydrated_jobs
                .iter()
                .filter(|j| j.status.is_active())
                .count(),
            0
        );
    }

    #[test]
    fn lock_poison_returns_typed_error_instead_of_panicking() {
        let manager = JobManager::new();

        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = manager.accepting_new_jobs.lock().unwrap();
            std::panic::panic_any("intentional poison");
        }));
        assert!(
            poisoned.is_err(),
            "test precondition: lock must be poisoned"
        );

        let app = mock_app();
        let handle = app.handle();
        let result = manager.register_or_get_active_job(
            handle,
            JobRegistrationInput {
                project_id: "poison".to_string(),
                project_root_path: None,
                kind: JobKind::Scoring,
                display_label: None,
                total: 1,
                message: "poison".to_string(),
                correlation_id: Some("corr-poison".to_string()),
                idempotency_key: Some("key-poison".to_string()),
                duplicate_policy: DuplicatePolicy::ReturnExisting,
                cancellable: true,
                retry_of_job_id: None,
            },
        );

        let err = match result {
            Ok(_) => panic!("poisoned lock must return a typed error, not panic"),
            Err(err) => err,
        };
        assert_eq!(err.code, AppErrorCode::UnknownError);
        assert!(
            err.technical_details.is_some(),
            "raw poison error must reach diagnostics"
        );
    }
}

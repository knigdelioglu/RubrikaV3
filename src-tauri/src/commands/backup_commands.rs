use std::path::PathBuf;

use crate::domain::errors::AppError;
use crate::services::backup_service;
use crate::services::integrity_recovery_service;
use crate::AppState;
use serde::Deserialize;
use serde::Serialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartBackupJobInput {
    pub project_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartBackupJobOutput {
    pub job_id: String,
    pub status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRestoreJobInput {
    pub archive_path: String,
    pub destination_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRestoreJobOutput {
    pub job_id: String,
    pub status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecoveryCopyJobInput {
    pub source_project_path: String,
    pub backup_path: String,
    pub destination_path: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecoveryCopyJobOutput {
    pub job_id: String,
    pub status: String,
}

#[tauri::command]
pub async fn start_backup_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartBackupJobInput,
) -> Result<StartBackupJobOutput, AppError> {
    let project = state
        .project_store
        .get_project_snapshot(input.project_id.clone())?;
    let job = state.job_manager.start_job(
        &app,
        input.project_id.clone(),
        // Backup is intentionally a source-preserving operation. Its job
        // snapshot lives in the application job store, not in the project
        // being snapshotted.
        None,
        crate::domain::job::JobKind::ProjectBackup,
        2,
        "Yedek hazırlanıyor...".to_string(),
    )?;

    let job_manager = state.job_manager.clone();
    let project_root = PathBuf::from(project.root_path);
    let job_id = job.id.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let worker_job_manager = job_manager.clone();
        let worker_app_handle = app_handle.clone();
        let worker_job_id = job_id.clone();
        let worker_root = project_root.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let _ = worker_job_manager.set_running(&worker_app_handle, &worker_job_id);
            let token = worker_job_manager
                .get_cancellation_token(&worker_job_id)
                .unwrap_or_default();
            let _ = worker_job_manager.update_progress(
                &worker_app_handle,
                &worker_job_id,
                1,
                2,
                "Dosyalar taranıyor ve doğrulanıyor...".to_string(),
            );
            backup_service::create_backup(&worker_root, &token)
        })
        .await;

        match result {
            Ok(Ok(summary)) => {
                let json = serde_json::to_value(&summary).unwrap_or(serde_json::Value::Null);
                let _ = job_manager.succeed(&app_handle, &job_id, Some(json));
            }
            Ok(Err(error)) => {
                let _ = job_manager.fail(&app_handle, &job_id, error);
            }
            Err(_) => {
                let _ = job_manager.fail(
                    &app_handle,
                    &job_id,
                    crate::domain::errors::AppError {
                        code: crate::domain::errors::AppErrorCode::BackupFailed,
                        message: "Yedek görevi beklenmedik şekilde sonlandı.".to_string(),
                        recoverable: true,
                        suggested_action: Some("Tekrar deneyin.".to_string()),
                        technical_details: Some("backup worker panicked".to_string()),
                        correlation_id: uuid::Uuid::new_v4().to_string(),
                    },
                );
            }
        }
    });

    Ok(StartBackupJobOutput {
        job_id: job.id,
        status: "queued".to_string(),
    })
}

#[tauri::command]
pub async fn start_restore_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartRestoreJobInput,
) -> Result<StartRestoreJobOutput, AppError> {
    if input.archive_path.trim().is_empty() || input.destination_path.trim().is_empty() {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::RestoreFailed,
            message: "Arşiv ve hedef klasör seçilmelidir.".to_string(),
            recoverable: true,
            suggested_action: None,
            technical_details: None,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    let job = state.job_manager.start_job(
        &app,
        "restore".to_string(),
        None,
        crate::domain::job::JobKind::ProjectRestore,
        2,
        "Yedek geri yükleniyor...".to_string(),
    )?;

    let job_manager = state.job_manager.clone();
    let archive_path = PathBuf::from(input.archive_path);
    let destination_path = PathBuf::from(input.destination_path);
    let job_id = job.id.clone();
    let app_handle = app.clone();
    let audit_service = state.audit_service.clone();

    tauri::async_runtime::spawn(async move {
        let worker_job_manager = job_manager.clone();
        let worker_app_handle = app_handle.clone();
        let worker_job_id = job_id.clone();
        let worker_archive = archive_path.clone();
        let worker_destination = destination_path.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let _ = worker_job_manager.set_running(&worker_app_handle, &worker_job_id);
            let token = worker_job_manager
                .get_cancellation_token(&worker_job_id)
                .unwrap_or_default();
            let _ = worker_job_manager.update_progress(
                &worker_app_handle,
                &worker_job_id,
                1,
                2,
                "Arşiv doğrulanıyor ve çıkarılıyor...".to_string(),
            );
            backup_service::restore_backup(&worker_archive, &worker_destination, &token)
        })
        .await;

        match result {
            Ok(Ok(summary)) => {
                let json = serde_json::to_value(&summary).unwrap_or(serde_json::Value::Null);
                let audit_result = audit_service.append_transactionally(
                    &destination_path,
                    crate::services::audit_service::AuditEntryInput::new(
                        "project_restored",
                        "Yedek yeni proje klasörüne geri yüklendi.",
                    )
                    .project(&summary.restored_project_id),
                    None,
                    None,
                );
                match audit_result {
                    Ok(_) => {
                        let _ = job_manager.succeed(&app_handle, &job_id, Some(json));
                    }
                    Err(error) => {
                        let _ = job_manager.fail(&app_handle, &job_id, error);
                    }
                }
            }
            Ok(Err(error)) => {
                let _ = job_manager.fail(&app_handle, &job_id, error);
            }
            Err(_) => {
                let _ = job_manager.fail(
                    &app_handle,
                    &job_id,
                    crate::domain::errors::AppError {
                        code: crate::domain::errors::AppErrorCode::RestoreFailed,
                        message: "Geri yükleme görevi beklenmedik şekilde sonlandı.".to_string(),
                        recoverable: true,
                        suggested_action: Some("Tekrar deneyin.".to_string()),
                        technical_details: Some("restore worker panicked".to_string()),
                        correlation_id: uuid::Uuid::new_v4().to_string(),
                    },
                );
            }
        }
    });

    Ok(StartRestoreJobOutput {
        job_id: job.id,
        status: "queued".to_string(),
    })
}

/// Starts recovery only against a new destination produced from a verified
/// external backup. The source path is passed for read-only evidence checks;
/// no source project command is opened in writable mode.
#[tauri::command]
pub async fn start_recovery_copy_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartRecoveryCopyJobInput,
) -> Result<StartRecoveryCopyJobOutput, AppError> {
    if input.source_project_path.trim().is_empty()
        || input.backup_path.trim().is_empty()
        || input.destination_path.trim().is_empty()
    {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::WorkflowBlocked,
            message: "Kaynak, doğrulanmış yedek ve recovery hedefi seçilmelidir.".to_string(),
            recoverable: true,
            suggested_action: Some("Yeni ve boş bir recovery hedefi seçin.".to_string()),
            technical_details: None,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    let job = state.job_manager.start_job(
        &app,
        "recovery-copy".to_string(),
        None,
        crate::domain::job::JobKind::ProjectRecovery,
        3,
        if input.dry_run {
            "Recovery dry-run doğrulanıyor...".to_string()
        } else {
            "Recovery kopyası hazırlanıyor...".to_string()
        },
    )?;
    let job_manager = state.job_manager.clone();
    let source_project = PathBuf::from(input.source_project_path);
    let backup_path = PathBuf::from(input.backup_path);
    let destination_path = PathBuf::from(input.destination_path);
    let dry_run = input.dry_run;
    let job_id = job.id.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let worker_manager = job_manager.clone();
        let worker_app = app_handle.clone();
        let worker_job = job_id.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let _ = worker_manager.set_running(&worker_app, &worker_job);
            let _ = worker_manager.update_progress(
                &worker_app,
                &worker_job,
                1,
                3,
                "Yedek, traversal ve kaynak güvenlik sınırları doğrulanıyor...".to_string(),
            );
            integrity_recovery_service::recover_copy(
                &backup_path,
                &destination_path,
                Some(&source_project),
                dry_run,
            )
        })
        .await;
        match result {
            Ok(Ok(report)) => {
                let json = match serde_json::to_value(&report) {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = job_manager.fail(
                            &app_handle,
                            &job_id,
                            AppError {
                                code: crate::domain::errors::AppErrorCode::UnknownError,
                                message: "Recovery sonucu hazırlanamadı.".to_string(),
                                recoverable: true,
                                suggested_action: Some("Tanılama kaydını inceleyin.".to_string()),
                                technical_details: Some(error.to_string()),
                                correlation_id: uuid::Uuid::new_v4().to_string(),
                            },
                        );
                        return;
                    }
                };
                let _ = job_manager.succeed(&app_handle, &job_id, Some(json));
            }
            Ok(Err(error)) => {
                let _ = job_manager.fail(&app_handle, &job_id, error);
            }
            Err(error) => {
                let _ = job_manager.fail(
                    &app_handle,
                    &job_id,
                    AppError {
                        code: crate::domain::errors::AppErrorCode::UnknownError,
                        message: "Recovery görevi beklenmedik şekilde sonlandı.".to_string(),
                        recoverable: true,
                        suggested_action: Some("Görevi yeniden deneyin.".to_string()),
                        technical_details: Some(error.to_string()),
                        correlation_id: uuid::Uuid::new_v4().to_string(),
                    },
                );
            }
        }
    });

    Ok(StartRecoveryCopyJobOutput {
        job_id: job.id,
        status: "queued".to_string(),
    })
}

use std::path::PathBuf;

use crate::domain::errors::AppError;
use crate::services::backup_service;
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
        Some(project.root_path.clone()),
        crate::domain::job::JobKind::ProjectBackup,
        2,
        "Yedek hazırlanıyor...".to_string(),
    )?;

    let job_manager = state.job_manager.clone();
    let project_root = PathBuf::from(project.root_path);
    let project_id = input.project_id.clone();
    let job_id = job.id.clone();
    let app_handle = app.clone();
    let audit_service = state.audit_service.clone();

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
                let _ = audit_service.append(
                    &project_root,
                    crate::services::audit_service::AuditEntryInput::new(
                        "backup_created",
                        "Proje yedeği oluşturuldu.",
                    )
                    .project(&project_id),
                );
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
                let _ = job_manager.succeed(&app_handle, &job_id, Some(json));
                let _ = audit_service.append(
                    &destination_path,
                    crate::services::audit_service::AuditEntryInput::new(
                        "project_restored",
                        "Yedek yeni proje klasörüne geri yüklendi.",
                    )
                    .project(&summary.restored_project_id),
                );
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

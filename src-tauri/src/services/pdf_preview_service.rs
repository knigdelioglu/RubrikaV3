use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::domain::document::{
    Document, DocumentRole, PdfPagePreview, PdfPreviewState, PdfPreviewStatus,
};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{JobKind, JobStatus};
use crate::domain::project::Project;
use crate::jobs::job_manager::JobManager;
use crate::platform::file_access::atomic_write;
use crate::platform::project_paths::TrustedProjectRoot;
use crate::services::pdf_service::PdfService;
use crate::services::project_store::ProjectStore;

#[derive(Clone)]
pub struct PdfPreviewService {
    project_store: ProjectStore,
    pdf_service: Arc<dyn PdfService>,
    job_manager: Arc<JobManager>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PdfPreviewIndex {
    document_id: String,
    page_count: u32,
    rendered_at: String,
    pages: Vec<PdfPagePreview>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PdfPreviewStatusSnapshot {
    pub document_id: String,
    pub status: PdfPreviewStatus,
    pub page_count: u32,
    pub rendered_at: Option<String>,
    pub job_id: Option<String>,
    pub preview_count: u32,
    pub message: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StartPdfPreviewRenderOutput {
    pub job_id: String,
    pub status: JobStatus,
}

impl PdfPreviewService {
    pub fn new(
        project_store: ProjectStore,
        pdf_service: Arc<dyn PdfService>,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            project_store,
            pdf_service,
            job_manager,
        }
    }

    pub fn get_pdf_page_count(&self, project_id: &str, document_id: &str) -> Result<u32, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let document = find_pdf_document(&project, document_id)?;
        Ok(document.page_count)
    }

    pub fn get_pdf_preview_status(
        &self,
        project_id: &str,
        document_id: &str,
    ) -> Result<PdfPreviewStatusSnapshot, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let document = find_pdf_document(&project, document_id)?;
        let trusted_root = self.project_store.trusted_project_root(project_id)?;
        let metadata_path = active_preview_metadata_path(&trusted_root, document)?;
        let index = read_preview_index(&metadata_path).ok();
        let job_snapshot = document
            .preview
            .as_ref()
            .and_then(|preview| preview.job_id.as_ref())
            .and_then(|job_id| self.job_manager.get_job_snapshot(job_id).ok());
        let status = document
            .preview
            .as_ref()
            .map(|preview| preview.status.clone())
            .unwrap_or_else(|| {
                if index.is_some() {
                    PdfPreviewStatus::Ready
                } else {
                    PdfPreviewStatus::Missing
                }
            });
        let preview_count = index
            .as_ref()
            .map(|index| index.pages.len() as u32)
            .or_else(|| job_snapshot.as_ref().map(|job| job.progress.current))
            .unwrap_or(0);
        let rendered_at = document
            .preview
            .as_ref()
            .and_then(|preview| preview.rendered_at.clone())
            .or_else(|| index.as_ref().map(|index| index.rendered_at.clone()));
        let job_id = document
            .preview
            .as_ref()
            .and_then(|preview| preview.job_id.clone());
        let mut total_pages = document
            .preview
            .as_ref()
            .and_then(|preview| preview.page_count)
            .or_else(|| index.as_ref().map(|index| index.page_count))
            .unwrap_or(document.page_count);
        // Older queued preview jobs were persisted before page_count was
        // resolved. Recover the display total from the source PDF so those
        // projects do not remain visible as an unexplained 0/0 operation.
        if total_pages == 0 {
            if let Ok(source_path) = document.resolve_path_with_root(&trusted_root) {
                total_pages = self.pdf_service.page_count(&source_path).unwrap_or(0);
            }
        }
        let subject = if document.role == DocumentRole::StudentScan {
            "Öğrenci PDF"
        } else {
            "PDF"
        };
        let message = match status {
            PdfPreviewStatus::Missing => {
                if document.role == DocumentRole::StudentScan {
                    format!("{subject} yüklendi. Sayfa önizlemeleri henüz oluşturulmadı.")
                } else {
                    "PDF sayfa önizlemeleri oluşturulmadı.".to_string()
                }
            }
            PdfPreviewStatus::Queued | PdfPreviewStatus::Running => {
                format!(
                    "{subject} önizlemesi oluşturuluyor: {preview_count}/{total_pages} sayfa hazır."
                )
            }
            PdfPreviewStatus::Ready => {
                if preview_count < total_pages {
                    format!(
                        "{subject} önizlemesi kısmi: {preview_count}/{total_pages} sayfa hazır."
                    )
                } else {
                    format!("{subject} önizlemesi hazır: {preview_count}/{total_pages} sayfa.")
                }
            }
            PdfPreviewStatus::Failed => {
                format!("{subject} önizlemesi oluşturulamadı.")
            }
        };

        Ok(PdfPreviewStatusSnapshot {
            document_id: document.id.clone(),
            status,
            page_count: total_pages,
            rendered_at,
            job_id,
            preview_count,
            message,
            error_message: document
                .preview
                .as_ref()
                .and_then(|preview| preview.error_message.clone()),
        })
    }

    pub fn list_pdf_page_previews(
        &self,
        project_id: &str,
        document_id: &str,
    ) -> Result<Vec<PdfPagePreview>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let document = find_pdf_document(&project, document_id)?;
        let trusted_root = self.project_store.trusted_project_root(project_id)?;
        let metadata_path = active_preview_metadata_path(&trusted_root, document)?;
        read_active_preview_index(&metadata_path, document)
            // The command result is a frontend read model. Keep the managed
            // relative path here so the UI can resolve it through the
            // `managed-asset` protocol without exposing a filesystem path.
            .and_then(|index| normalize_preview_pages(&trusted_root, index.pages))
            .map_err(|_| AppError {
                code: AppErrorCode::PdfPreviewNotFound,
                message: "Sayfa önizleme önbelleği bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
                technical_details: Some(metadata_path.to_string_lossy().to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })
    }

    pub fn require_ready_page_previews(
        &self,
        project_id: &str,
        document_id: &str,
    ) -> Result<Vec<PdfPagePreview>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let document = find_pdf_document(&project, document_id)?;
        let preview = document.preview.as_ref();
        let trusted_root = self.project_store.trusted_project_root(project_id)?;
        let metadata_path = active_preview_metadata_path(&trusted_root, document)?;
        let index = read_active_preview_index(&metadata_path, document).map_err(|error| {
            let detail = error
                .technical_details
                .clone()
                .unwrap_or_else(|| error.message.clone());
            AppError {
                code: AppErrorCode::PdfPreviewNotReady,
                message: "Soru metni çıkarılmadan önce PDF sayfa önizlemeleri oluşturulmalıdır."
                    .to_string(),
                recoverable: true,
                suggested_action: Some("Önce PDF sayfa önizlemelerini oluşturun.".to_string()),
                technical_details: Some(format!(
                    "metadata_path={}; {}",
                    metadata_path.to_string_lossy(),
                    detail
                )),
                correlation_id: Uuid::new_v4().to_string(),
            }
        })?;

        let is_ready = matches!(
            preview.map(|state| &state.status),
            Some(PdfPreviewStatus::Ready)
        );
        if !is_ready || index.pages.is_empty() {
            return Err(AppError {
                code: AppErrorCode::PdfPreviewNotReady,
                message: "Soru metni çıkarılmadan önce PDF sayfa önizlemeleri oluşturulmalıdır."
                    .to_string(),
                recoverable: true,
                suggested_action: Some("Önce PDF sayfa önizlemelerini oluşturun.".to_string()),
                technical_details: Some(format!(
                    "document_id={}; preview_status={:?}; preview_count={}",
                    document.id,
                    preview.map(|state| state.status.clone()),
                    index.pages.len()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        materialize_preview_pages(&trusted_root, index.pages).map_err(|error| AppError {
            code: AppErrorCode::PdfPreviewNotReady,
            message: "Soru metni çıkarılmadan önce PDF sayfa önizlemeleri oluşturulmalıdır."
                .to_string(),
            recoverable: true,
            suggested_action: Some("Önce PDF sayfa önizlemelerini oluşturun.".to_string()),
            technical_details: error.technical_details,
            correlation_id: Uuid::new_v4().to_string(),
        })
    }

    pub fn get_pdf_page_preview(
        &self,
        project_id: &str,
        document_id: &str,
        page_number: u32,
    ) -> Result<PdfPagePreview, AppError> {
        self.list_pdf_page_previews(project_id, document_id)?
            .into_iter()
            .find(|page| page.page_number == page_number)
            .ok_or_else(|| AppError {
                code: AppErrorCode::PdfPreviewNotFound,
                message: "İstenen sayfa önizlemesi bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Önizlemeleri yeniden oluşturun.".to_string()),
                technical_details: Some(format!("page_number={page_number}")),
                correlation_id: Uuid::new_v4().to_string(),
            })
    }

    pub fn start_render<R: tauri::Runtime>(
        &self,
        app: AppHandle<R>,
        project_id: String,
        document_id: String,
    ) -> Result<StartPdfPreviewRenderOutput, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let document = find_pdf_document(&project, &document_id)?.clone();
        let trusted_root = self.project_store.trusted_project_root(&project_id)?;
        validate_pdf_source(&document, &trusted_root)?;
        let source_path = document.resolve_path_with_root(&trusted_root)?;
        let expected_page_count = if document.page_count > 0 {
            document.page_count
        } else {
            self.pdf_service.page_count(&source_path)?
        };
        if expected_page_count == 0 {
            return Err(AppError {
                code: AppErrorCode::PreviewGenerationFailed,
                message: "PDF sayfa sayısı belirlenemedi.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Geçerli bir PDF seçip önizlemeyi yeniden oluşturun.".to_string(),
                ),
                technical_details: Some(format!("document_id={document_id}; page_count=0")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let source_fingerprint = file_fingerprint(&source_path)?;
        let generation_id = Uuid::new_v4().to_string();

        let renderer_status = self.pdf_service.get_renderer_status()?;
        if !renderer_status.available {
            return Err(AppError {
                code: AppErrorCode::PdfRendererNotFound,
                message: "PDF önizleme aracı bulunamadı. Poppler kurulu olmayabilir. Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string(),
                recoverable: true,
                suggested_action: Some("Terminalde `brew install poppler` komutunu çalıştırıp tekrar deneyin.".to_string()),
                technical_details: Some(format!("Searched paths: {:?}", renderer_status.searched_paths)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let job = self.job_manager.start_job(
            &app,
            project_id.clone(),
            Some(project.root_path.clone()),
            JobKind::PdfPreviewRender,
            expected_page_count,
            "PDF önizlemeleri hazırlanıyor...".to_string(),
        )?;

        let queue_result = self
            .project_store
            .mutate(
                &project_id,
                crate::services::project_store::MutationOptions::new("queue_preview_generation"),
                |current, _context| {
                    let current_preview = current
                        .documents
                        .iter()
                        .find(|entry| entry.id == document_id)
                        .and_then(|entry| entry.preview.clone());
                    set_document_preview_state(
                        current,
                        &document_id,
                        PdfPreviewState {
                            status: PdfPreviewStatus::Queued,
                            rendered_at: current_preview
                                .as_ref()
                                .and_then(|value| value.rendered_at.clone()),
                            page_count: Some(expected_page_count),
                            job_id: Some(job.id.clone()),
                            error_message: None,
                            active_generation_id: current_preview
                                .as_ref()
                                .and_then(|value| value.active_generation_id.clone()),
                            pending_generation_id: Some(generation_id.clone()),
                            source_fingerprint: Some(source_fingerprint.clone()),
                        },
                    )
                },
            )
            .map(|_| ());
        if let Err(error) = queue_result {
            let _ = self.job_manager.fail(&app, &job.id, error.clone());
            return Err(error);
        }

        let service = self.clone();
        let job_id = job.id.clone();
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service
                .run_render(
                    app_handle.clone(),
                    job_id.clone(),
                    project_id.clone(),
                    document_id.clone(),
                    generation_id.clone(),
                    source_fingerprint.clone(),
                )
                .await
            {
                let _ = service.mark_render_failed(
                    &app_handle,
                    &project_id,
                    &document_id,
                    &job_id,
                    error,
                );
            }
        });

        Ok(StartPdfPreviewRenderOutput {
            job_id: job.id,
            status: job.status,
        })
    }

    async fn run_render<R: tauri::Runtime>(
        &self,
        app: AppHandle<R>,
        job_id: String,
        project_id: String,
        document_id: String,
        generation_id: String,
        source_fingerprint: String,
    ) -> Result<(), AppError> {
        self.job_manager.set_running(&app, &job_id)?;

        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let document = find_pdf_document(&project, &document_id)?.clone();
        let trusted_root = self.project_store.trusted_project_root(&project_id)?;
        validate_pdf_source(&document, &trusted_root)?;

        let pdf_path = document.resolve_path_with_root(&trusted_root)?;
        if file_fingerprint(&pdf_path)? != source_fingerprint {
            return Err(preview_stale_error());
        }
        let staging_dir = preview_staging_dir(&trusted_root, &document.id, &generation_id)?;
        let generation_dir = preview_generation_dir(&trusted_root, &document.id, &generation_id)?;
        trusted_root.ensure_managed_directory(
            staging_dir
                .parent()
                .ok_or_else(|| preview_write_error("Önizleme staging yolu belirlenemedi."))?,
        )?;
        trusted_root.ensure_managed_directory(&staging_dir)?;

        let cancel_token = self.job_manager.get_cancellation_token(&job_id);

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                cleanup_preview_directory(&trusted_root, &staging_dir);
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Önizleme işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        let build_result = (|| -> Result<(Vec<PdfPagePreview>, u32, String), AppError> {
            let rendered = self.pdf_service.render_all_pages(&pdf_path, &staging_dir)?;
            if rendered.is_empty() {
                return Err(AppError {
                    code: AppErrorCode::PreviewGenerationFailed,
                    message: "PDF önizlemesi oluşturulamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Lütfen geçerli bir PDF deneyin.".to_string()),
                    technical_details: Some("No rendered pages returned.".to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            if document.page_count > 0 && rendered.len() as u32 != document.page_count {
                return Err(AppError {
                    code: AppErrorCode::PreviewGenerationFailed,
                    message: "Yeni PDF önizlemesi beklenen sayfa sayısını üretmedi.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Önizlemeyi yeniden oluşturun.".to_string()),
                    technical_details: Some(format!(
                        "expected_pages={}; rendered_pages={}",
                        document.page_count,
                        rendered.len()
                    )),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
            let rendered_at = chrono::Utc::now().to_rfc3339();
            let page_count = rendered.len() as u32;
            let mut previews = Vec::new();
            for (index, image_path) in rendered.iter().enumerate() {
                if let Some(ref token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(AppError {
                            code: AppErrorCode::JobCancelled,
                            message: "Önizleme işlemi iptal edildi.".to_string(),
                            recoverable: true,
                            suggested_action: None,
                            technical_details: None,
                            correlation_id: Uuid::new_v4().to_string(),
                        });
                    }
                }

                validate_rendered_preview_file(&trusted_root, &staging_dir, image_path)?;
                let file_name = image_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| preview_write_error("Önizleme dosya adı belirlenemedi."))?;
                if file_name == "."
                    || file_name == ".."
                    || file_name.contains('/')
                    || file_name.contains('\\')
                {
                    return Err(AppError {
                        code: AppErrorCode::ManagedPathOutsideProject,
                        message: "Önizleme dosya adı güvenli değil.".to_string(),
                        recoverable: false,
                        suggested_action: Some("PDF önizlemesini yeniden oluşturun.".to_string()),
                        technical_details: None,
                        correlation_id: Uuid::new_v4().to_string(),
                    });
                }
                let final_image_path = generation_dir.join(file_name);
                let (width, height) =
                    image::image_dimensions(image_path).map_err(|error| AppError {
                        code: AppErrorCode::PreviewGenerationFailed,
                        message: "Önizleme görseli okunamadı.".to_string(),
                        recoverable: true,
                        suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
                        technical_details: Some(error.to_string()),
                        correlation_id: Uuid::new_v4().to_string(),
                    })?;
                previews.push(PdfPagePreview {
                    document_id: document.id.clone(),
                    page_number: (index + 1) as u32,
                    image_path: trusted_root
                        .relative_for_existing(&final_image_path)
                        .or_else(|_| {
                            trusted_root.managed(&format!(
                                "outputs/previews/{}/generations/{}/{}",
                                document.id, generation_id, file_name
                            ))
                        })?
                        .as_str()
                        .to_string(),
                    width,
                    height,
                    rendered_at: rendered_at.clone(),
                });
                let current = (index + 1) as u32;
                self.job_manager.update_progress(
                    &app,
                    &job_id,
                    current,
                    page_count,
                    format!("Sayfa {current}/{page_count} işleniyor"),
                )?;
            }
            if let Some(ref token) = cancel_token {
                if token.is_cancelled() {
                    return Err(AppError {
                        code: AppErrorCode::JobCancelled,
                        message: "Önizleme işlemi iptal edildi.".to_string(),
                        recoverable: true,
                        suggested_action: None,
                        technical_details: None,
                        correlation_id: Uuid::new_v4().to_string(),
                    });
                }
            }
            let manifest = PdfPreviewIndex {
                document_id: document.id.clone(),
                page_count,
                rendered_at: rendered_at.clone(),
                pages: previews.clone(),
            };
            write_preview_index(&staging_dir.join("manifest.json"), &manifest)?;
            Ok((previews, page_count, rendered_at))
        })();

        let (previews, page_count, rendered_at) = match build_result {
            Ok(value) => value,
            Err(error) => {
                cleanup_preview_directory(&trusted_root, &staging_dir);
                if error.code == AppErrorCode::JobCancelled {
                    let _ = self.job_manager.mark_cancelled(&app, &job_id);
                }
                return Err(error);
            }
        };

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                cleanup_preview_directory(&trusted_root, &staging_dir);
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Önizleme işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }
        if file_fingerprint(&pdf_path)? != source_fingerprint {
            cleanup_preview_directory(&trusted_root, &staging_dir);
            return Err(preview_stale_error());
        }
        trusted_root.ensure_managed_directory(
            generation_dir
                .parent()
                .ok_or_else(|| preview_write_error("Önizleme generation yolu belirlenemedi."))?,
        )?;
        crate::platform::file_access::durable_rename_directory(&staging_dir, &generation_dir)
            .map_err(|error| {
                cleanup_preview_directory(&trusted_root, &staging_dir);
                AppError {
                    code: AppErrorCode::PreviewGenerationFailed,
                    message: "Yeni PDF önizlemesi etkinleştirilemedi.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Önizlemeyi yeniden oluşturun.".to_string()),
                    technical_details: Some(error.to_string()),
                    correlation_id: Uuid::new_v4().to_string(),
                }
            })?;

        let commit = self.project_store.commit_job(
            &project_id,
            crate::services::project_store::MutationOptions::new("activate_preview_generation"),
            |current, _context| {
                let current_document = find_pdf_document(current, &document_id)?;
                let current_path = current_document.resolve_path_with_root(&trusted_root)?;
                if file_fingerprint(&current_path)? != source_fingerprint {
                    return Err(preview_stale_error());
                }
                if let Some(entry) = current
                    .documents
                    .iter_mut()
                    .find(|entry| entry.id == document_id)
                {
                    entry.page_count = page_count;
                    entry.preview = Some(PdfPreviewState {
                        status: PdfPreviewStatus::Ready,
                        rendered_at: Some(rendered_at.clone()),
                        page_count: Some(page_count),
                        job_id: Some(job_id.clone()),
                        error_message: None,
                        active_generation_id: Some(generation_id.clone()),
                        pending_generation_id: None,
                        source_fingerprint: Some(source_fingerprint.clone()),
                    });
                }
                Ok(())
            },
        );
        match commit {
            crate::services::project_store::JobCommitResult::Applied(_) => {}
            crate::services::project_store::JobCommitResult::Stale { .. }
            | crate::services::project_store::JobCommitResult::EntityMissing => {
                cleanup_preview_directory(&trusted_root, &generation_dir);
                return Err(preview_stale_error());
            }
            crate::services::project_store::JobCommitResult::Conflict(error)
            | crate::services::project_store::JobCommitResult::Rejected(error) => {
                cleanup_preview_directory(&trusted_root, &generation_dir);
                return Err(error);
            }
        }

        self.job_manager.succeed(
            &app,
            &job_id,
            Some(serde_json::json!({
                "documentId": document.id,
                "pageCount": page_count,
                "previewCount": previews.len(),
                "renderedAt": rendered_at,
            })),
        )?;
        Ok(())
    }

    pub fn mark_render_failed<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        project_id: &str,
        document_id: &str,
        job_id: &str,
        error: AppError,
    ) -> Result<(), AppError> {
        let error_message = error.message.clone();
        self.project_store
            .mutate(
                project_id,
                crate::services::project_store::MutationOptions::new(
                    "mark_preview_generation_failed",
                ),
                |project, _context| {
                    let previous = project
                        .documents
                        .iter()
                        .find(|document| document.id == document_id)
                        .and_then(|document| document.preview.clone());
                    set_document_preview_state(
                        project,
                        document_id,
                        PdfPreviewState {
                            status: PdfPreviewStatus::Failed,
                            rendered_at: previous
                                .as_ref()
                                .and_then(|value| value.rendered_at.clone()),
                            page_count: previous.as_ref().and_then(|value| value.page_count),
                            job_id: Some(job_id.to_string()),
                            error_message: Some(error_message.clone()),
                            active_generation_id: previous
                                .as_ref()
                                .and_then(|value| value.active_generation_id.clone()),
                            pending_generation_id: None,
                            source_fingerprint: previous
                                .as_ref()
                                .and_then(|value| value.source_fingerprint.clone()),
                        },
                    )
                },
            )
            .map(|_| ())?;
        self.job_manager.fail(app, job_id, error)?;
        Ok(())
    }
}

fn find_pdf_document<'a>(
    project: &'a Project,
    document_id: &str,
) -> Result<&'a Document, AppError> {
    project
        .documents
        .iter()
        .find(|document| {
            document.id == document_id
                && matches!(
                    document.role,
                    DocumentRole::ExamSource | DocumentRole::AnswerKey | DocumentRole::StudentScan
                )
        })
        .ok_or_else(|| AppError {
            code: AppErrorCode::PdfDocumentNotFound,
            message: "PDF belgesi bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("PDF belgesini yeniden yükleyin.".to_string()),
            technical_details: Some(format!("document_id={document_id}")),
            correlation_id: Uuid::new_v4().to_string(),
        })
}

fn validate_pdf_source(
    document: &Document,
    trusted_root: &TrustedProjectRoot,
) -> Result<(), AppError> {
    document
        .resolve_path_with_root(trusted_root)
        .map(|_| ())
        .map_err(|error| AppError {
            code: error.code,
            message: "Sınav PDF dosyası bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("PDF dosyasını tekrar içe aktarın.".to_string()),
            technical_details: error.technical_details,
            correlation_id: Uuid::new_v4().to_string(),
        })
}

#[cfg(test)]
fn preview_dir(trusted_root: &TrustedProjectRoot, document_id: &str) -> Result<PathBuf, AppError> {
    let managed = trusted_root.managed(&format!("cache/page_previews/{document_id}"))?;
    Ok(trusted_root.root().join(managed.as_path()))
}

fn preview_generations_dir(
    trusted_root: &TrustedProjectRoot,
    document_id: &str,
) -> Result<PathBuf, AppError> {
    let managed = trusted_root.managed(&format!("outputs/previews/{document_id}/generations"))?;
    Ok(trusted_root.root().join(managed.as_path()))
}

fn preview_generation_dir(
    trusted_root: &TrustedProjectRoot,
    document_id: &str,
    generation_id: &str,
) -> Result<PathBuf, AppError> {
    let parent = preview_generations_dir(trusted_root, document_id)?;
    let managed = trusted_root.managed(&format!(
        "outputs/previews/{document_id}/generations/{generation_id}"
    ))?;
    let candidate = trusted_root.root().join(managed.as_path());
    if candidate.parent() != Some(parent.as_path()) {
        return Err(AppError {
            code: AppErrorCode::UnsafeManagedPath,
            message: "Önizleme generation yolu güvenli değil.".to_string(),
            recoverable: false,
            suggested_action: Some("Önizlemeyi yeniden oluşturun.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    Ok(candidate)
}

fn preview_staging_dir(
    trusted_root: &TrustedProjectRoot,
    document_id: &str,
    generation_id: &str,
) -> Result<PathBuf, AppError> {
    let managed = trusted_root.managed(&format!(
        "outputs/previews/{document_id}/.staging/{generation_id}"
    ))?;
    Ok(trusted_root.root().join(managed.as_path()))
}

fn active_preview_metadata_path(
    trusted_root: &TrustedProjectRoot,
    document: &Document,
) -> Result<PathBuf, AppError> {
    if let Some(generation_id) = document
        .preview
        .as_ref()
        .and_then(|preview| preview.active_generation_id.as_deref())
    {
        return Ok(
            preview_generation_dir(trusted_root, &document.id, generation_id)?
                .join("manifest.json"),
        );
    }
    preview_metadata_path(trusted_root, &document.id)
}

fn preview_metadata_path(
    trusted_root: &TrustedProjectRoot,
    document_id: &str,
) -> Result<PathBuf, AppError> {
    let managed = trusted_root.managed(&format!(
        "cache/page_previews/{document_id}/page_previews.json"
    ))?;
    Ok(trusted_root.root().join(managed.as_path()))
}

fn write_preview_index(path: &Path, index: &PdfPreviewIndex) -> Result<(), AppError> {
    let serialized = serde_json::to_string_pretty(index).map_err(|e| AppError {
        code: AppErrorCode::FileWriteFailed,
        message: "Önizleme metadata'sı yazılamadı.".to_string(),
        recoverable: false,
        suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    atomic_write(path, &serialized).map_err(|e| AppError {
        code: AppErrorCode::FileWriteFailed,
        message: "Önizleme metadata'sı kaydedilemedi.".to_string(),
        recoverable: false,
        suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn validate_rendered_preview_file(
    trusted_root: &TrustedProjectRoot,
    staging_dir: &Path,
    image_path: &Path,
) -> Result<(), AppError> {
    if !image_path.starts_with(staging_dir) {
        return Err(AppError {
            code: AppErrorCode::ManagedPathOutsideProject,
            message: "Önizleme çıktısı güvenilen staging alanının dışında.".to_string(),
            recoverable: false,
            suggested_action: Some("PDF önizlemesini yeniden oluşturun.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    let metadata = std::fs::symlink_metadata(image_path).map_err(|error| AppError {
        code: AppErrorCode::PreviewGenerationFailed,
        message: "Önizleme sayfası oluşturulamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(AppError {
            code: AppErrorCode::PreviewGenerationFailed,
            message: "Önizleme sayfası boş veya güvenli bir dosya değil.".to_string(),
            recoverable: true,
            suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    trusted_root.relative_for_existing(image_path).map(|_| ())
}

fn cleanup_preview_directory(trusted_root: &TrustedProjectRoot, directory: &Path) {
    if directory.exists() {
        let _ = crate::platform::file_access::remove_dir_within(trusted_root.root(), directory);
    }
}

fn preview_write_error(message: &str) -> AppError {
    AppError {
        code: AppErrorCode::PreviewGenerationFailed,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
        technical_details: None,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn preview_stale_error() -> AppError {
    AppError {
        code: AppErrorCode::PreviewGenerationStale,
        message: "PDF değiştiği için yeni önizleme etkinleştirilmedi; önceki önizleme korundu."
            .to_string(),
        recoverable: true,
        suggested_action: Some("Belgeyi yenileyip önizlemeyi yeniden oluşturun.".to_string()),
        technical_details: None,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn file_fingerprint(path: &Path) -> Result<String, AppError> {
    let bytes = std::fs::read(path).map_err(|error| AppError {
        code: AppErrorCode::FileReadFailed,
        message: "PDF kaynak dosyası okunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Belgeyi yeniden içe aktarın.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

fn read_preview_index(path: &Path) -> Result<PdfPreviewIndex, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| AppError {
        code: AppErrorCode::FileReadFailed,
        message: "Önizleme metadata'sı okunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    serde_json::from_str(&content).map_err(|e| AppError {
        code: AppErrorCode::PdfPreviewNotFound,
        message: "Önizleme metadata'sı geçersiz.".to_string(),
        recoverable: true,
        suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn read_active_preview_index(
    path: &Path,
    document: &Document,
) -> Result<PdfPreviewIndex, AppError> {
    if document
        .preview
        .as_ref()
        .and_then(|preview| preview.active_generation_id.as_ref())
        .is_some()
        && !path.exists()
    {
        return Err(AppError {
            code: AppErrorCode::PreviewActiveGenerationMissing,
            message: "Aktif PDF önizlemesi bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("PDF önizlemesini yeniden oluşturun.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    read_preview_index(path)
}

fn materialize_preview_pages(
    trusted_root: &TrustedProjectRoot,
    pages: Vec<PdfPagePreview>,
) -> Result<Vec<PdfPagePreview>, AppError> {
    let normalized = normalize_preview_pages(trusted_root, pages)?;
    normalized
        .into_iter()
        .map(|mut page| {
            let managed = trusted_root.managed(&page.image_path)?;
            let resolved = trusted_root.resolve_existing_file(&managed)?;
            page.image_path = resolved.to_string_lossy().to_string();
            Ok(page)
        })
        .collect()
}

fn normalize_preview_pages(
    trusted_root: &TrustedProjectRoot,
    pages: Vec<PdfPagePreview>,
) -> Result<Vec<PdfPagePreview>, AppError> {
    pages
        .into_iter()
        .map(|mut page| {
            let managed = trusted_root.adapt_legacy_document_path(&page.image_path)?;
            trusted_root.resolve_existing_file(&managed)?;
            page.image_path = managed.as_str().to_string();
            Ok(page)
        })
        .collect()
}

fn set_document_preview_state(
    project: &mut Project,
    document_id: &str,
    preview: PdfPreviewState,
) -> Result<(), AppError> {
    let document = project
        .documents
        .iter_mut()
        .find(|document| document.id == document_id)
        .ok_or_else(|| AppError {
            code: AppErrorCode::PdfDocumentNotFound,
            message: "Sınav PDF'i bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Orijinal sınav PDF'ini yeniden yükleyin.".to_string()),
            technical_details: Some(format!("document_id={document_id}")),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
    document.preview = Some(preview);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, DocumentRole, PdfPreviewState, PdfPreviewStatus};
    use crate::domain::project::Project;
    use crate::domain::workflow::{WorkflowAction, WorkflowSnapshot, WorkflowStage};
    use crate::jobs::job_manager::JobManager;
    use crate::services::pdf_service::SystemPdfService;
    use crate::services::project_store::ProjectStore;

    fn temp_project_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-v3-preview-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn service_for_tests(project_store: ProjectStore) -> PdfPreviewService {
        PdfPreviewService::new(
            project_store,
            Arc::new(SystemPdfService),
            Arc::new(JobManager::new()),
        )
    }

    #[test]
    fn preview_dir_and_metadata_path_are_stable() {
        let project = Project {
            expected_question_count: None,
            exam_package_freeze: None,
            id: "proj-1".to_string(),
            name: "p".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            root_path: temp_project_root().to_string_lossy().to_string(),
            storage_revision: 0,
            academic_year_id: None,
            course_id: None,
            course_name: None,
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            teaching_assignments: vec![],
            assessment_activities: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_answer_ocr_records: vec![],
            student_answer_ocr_generations: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            student_scan_document_id: None,
            student_grouping_mode: None,
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            documents: vec![],
            questions: vec![],
            scoring_records: vec![],
            scoring_anchors: vec![],
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            workflow: WorkflowSnapshot {
                current_stage: WorkflowStage::DocumentsMissing,
                blocking_reasons: vec![],
                next_actions: vec![],
                current_stage_label: "Test".to_string(),
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
        };
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)
                .unwrap();
        let dir = preview_dir(&trusted_root, "doc-1").unwrap();
        assert!(dir.ends_with("cache/page_previews/doc-1"));
        let metadata = preview_metadata_path(&trusted_root, "doc-1").unwrap();
        assert!(metadata.ends_with("cache/page_previews/doc-1/page_previews.json"));
    }

    #[test]
    fn failed_staging_cleanup_preserves_active_preview_bytes() {
        let root = temp_project_root();
        let trusted_root = TrustedProjectRoot::from_canonical_root(root, false).unwrap();
        let active = preview_generation_dir(&trusted_root, "doc-1", "active-generation").unwrap();
        let staging = preview_staging_dir(&trusted_root, "doc-1", "failed-generation").unwrap();
        std::fs::create_dir_all(&active).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        let manifest = active.join("manifest.json");
        let page = active.join("page-001.png");
        std::fs::write(&manifest, b"active-manifest").unwrap();
        std::fs::write(&page, b"active-page").unwrap();
        std::fs::write(staging.join("partial-page.png"), b"partial").unwrap();

        cleanup_preview_directory(&trusted_root, &staging);

        assert_eq!(std::fs::read(&manifest).unwrap(), b"active-manifest");
        assert_eq!(std::fs::read(&page).unwrap(), b"active-page");
        assert!(!staging.exists());
    }

    #[test]
    fn set_preview_state_updates_matching_document_and_answer_key_is_previewable() {
        let mut project = Project {
            expected_question_count: None,
            exam_package_freeze: None,
            id: "p".into(),
            name: "p".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            root_path: temp_project_root().to_string_lossy().to_string(),
            storage_revision: 0,
            academic_year_id: None,
            course_id: None,
            course_name: None,
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            teaching_assignments: vec![],
            assessment_activities: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_answer_ocr_records: vec![],
            student_answer_ocr_generations: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            student_scan_document_id: None,
            student_grouping_mode: None,
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            documents: vec![Document {
                id: "doc-1".into(),
                role: DocumentRole::ExamSource,
                file_name: "exam.pdf".into(),
                stored_path: "exam.pdf".into(),
                page_count: 2,
                added_at: "now".into(),
                checksum: None,
                preview: None,
            }],
            questions: vec![],
            scoring_records: vec![],
            scoring_anchors: vec![],
            speaking_exams: vec![],
            latest_scoring_run_id: None,
            workflow: WorkflowSnapshot {
                current_stage: WorkflowStage::DocumentsMissing,
                blocking_reasons: vec![],
                next_actions: vec![WorkflowAction {
                    code: "noop".into(),
                    label: "noop".into(),
                    enabled: true,
                    disabled_reason: None,
                    command: None,
                    requires: None,
                }],
                current_stage_label: "Test".to_string(),
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
        };

        set_document_preview_state(
            &mut project,
            "doc-1",
            PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some("now".into()),
                page_count: Some(2),
                job_id: Some("job".into()),
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            },
        )
        .expect("preview state");

        assert!(matches!(
            project.documents[0]
                .preview
                .as_ref()
                .map(|preview| &preview.status),
            Some(PdfPreviewStatus::Ready)
        ));

        project.documents[0].role = DocumentRole::AnswerKey;
        assert!(find_pdf_document(&project, "doc-1").is_ok());
    }

    #[test]
    fn require_ready_page_previews_returns_not_ready_when_metadata_missing() {
        let store = ProjectStore::new();
        let root = temp_project_root();
        let mut project = store
            .create_project("p".into(), root.to_string_lossy().to_string())
            .expect("project");
        project.student_scan_document_id = None;
        project.documents.push(Document {
            id: "doc-1".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: root.join("exam.pdf").to_string_lossy().to_string(),
            page_count: 1,
            added_at: "now".into(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some("now".into()),
                page_count: Some(1),
                job_id: Some("job".into()),
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });
        store.save_project(&project).expect("save");

        let service = service_for_tests(store);
        let error = service
            .require_ready_page_previews(&project.id, "doc-1")
            .expect_err("preview should be missing");
        assert_eq!(error.code, AppErrorCode::PdfPreviewNotReady);
    }

    #[test]
    fn preview_status_keeps_known_total_when_no_page_is_rendered_yet() {
        let store = ProjectStore::new();
        let root = temp_project_root();
        let mut project = store
            .create_project("p".into(), root.to_string_lossy().to_string())
            .expect("project");
        project.student_scan_document_id = None;
        project.documents.push(Document {
            id: "doc-1".into(),
            role: DocumentRole::AnswerKey,
            file_name: "rubric.pdf".into(),
            stored_path: "documents/rubric.pdf".into(),
            page_count: 0,
            added_at: "now".into(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Queued,
                rendered_at: None,
                page_count: Some(2),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: Some("generation".into()),
                source_fingerprint: None,
            }),
        });
        store.save_project(&project).expect("save project");

        let service = service_for_tests(store);
        let status = service
            .get_pdf_preview_status(&project.id, "doc-1")
            .expect("status");
        assert_eq!(status.preview_count, 0);
        assert_eq!(status.page_count, 2);
        assert!(status.message.contains("0/2"));
    }

    #[test]
    fn require_ready_page_previews_returns_not_ready_when_image_missing() {
        let store = ProjectStore::new();
        let root = temp_project_root();
        let mut project = store
            .create_project("p".into(), root.to_string_lossy().to_string())
            .expect("project");
        project.student_scan_document_id = None;
        let trusted_root =
            TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)
                .unwrap();
        let preview_dir = preview_dir(&trusted_root, "doc-1").unwrap();
        std::fs::create_dir_all(&preview_dir).expect("preview dir");
        let image_path = preview_dir.join("page_1.png");
        let metadata = PdfPreviewIndex {
            document_id: "doc-1".into(),
            page_count: 1,
            rendered_at: "now".into(),
            pages: vec![PdfPagePreview {
                document_id: "doc-1".into(),
                page_number: 1,
                image_path: image_path.to_string_lossy().to_string(),
                width: 100,
                height: 100,
                rendered_at: "now".into(),
            }],
        };
        write_preview_index(
            &preview_metadata_path(&trusted_root, "doc-1").unwrap(),
            &metadata,
        )
        .expect("metadata");
        project.documents.push(Document {
            id: "doc-1".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: root.join("exam.pdf").to_string_lossy().to_string(),
            page_count: 1,
            added_at: "now".into(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some("now".into()),
                page_count: Some(1),
                job_id: Some("job".into()),
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });
        store.save_project(&project).expect("save");

        let service = service_for_tests(store);
        let error = service
            .require_ready_page_previews(&project.id, "doc-1")
            .expect_err("image should be missing");
        assert_eq!(error.code, AppErrorCode::PdfPreviewNotReady);
    }

    #[test]
    fn list_page_previews_returns_managed_relative_paths_for_frontend() {
        let store = ProjectStore::new();
        let root = temp_project_root();
        let mut project = store
            .create_project("p".into(), root.to_string_lossy().to_string())
            .expect("project");
        project.student_scan_document_id = None;

        let trusted_root =
            TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)
                .expect("trusted root");
        let image_path = root.join("outputs/previews/doc-1/page_1.png");
        std::fs::create_dir_all(image_path.parent().expect("image parent")).expect("image dir");
        std::fs::write(&image_path, b"PNGDATA").expect("image");
        let metadata = PdfPreviewIndex {
            document_id: "doc-1".into(),
            page_count: 1,
            rendered_at: "now".into(),
            pages: vec![PdfPagePreview {
                document_id: "doc-1".into(),
                page_number: 1,
                image_path: image_path.to_string_lossy().to_string(),
                width: 100,
                height: 100,
                rendered_at: "now".into(),
            }],
        };
        write_preview_index(
            &preview_metadata_path(&trusted_root, "doc-1").expect("metadata path"),
            &metadata,
        )
        .expect("metadata");
        project.documents.push(Document {
            id: "doc-1".into(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".into(),
            stored_path: "documents/exam.pdf".into(),
            page_count: 1,
            added_at: "now".into(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some("now".into()),
                page_count: Some(1),
                job_id: Some("job".into()),
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });
        store.save_project(&project).expect("save project");

        let service = service_for_tests(store);
        let pages = service
            .list_pdf_page_previews(&project.id, "doc-1")
            .expect("pages");
        assert_eq!(pages[0].image_path, "outputs/previews/doc-1/page_1.png");
    }

    #[test]
    fn proof_5_preview_cancel_preserves_active_generation() {
        use crate::domain::job::DuplicatePolicy;
        use crate::jobs::job_manager::JobRegistrationInput;

        let store = ProjectStore::new();
        let root = temp_project_root();
        let mut project = store
            .create_project("proj_p5".into(), root.to_string_lossy().to_string())
            .expect("project");
        let pdf_path = root.join("multi_page.pdf");
        std::fs::write(&pdf_path, b"%PDF-1.4 dummy content for test").expect("write pdf");
        let fingerprint = file_fingerprint(&pdf_path).unwrap();

        let doc_id = "doc-p5";
        project.documents.push(Document {
            id: doc_id.into(),
            role: DocumentRole::ExamSource,
            file_name: "multi_page.pdf".into(),
            stored_path: pdf_path.to_string_lossy().to_string(),
            page_count: 2,
            added_at: "now".into(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some("now".into()),
                page_count: Some(2),
                job_id: Some("initial_job".into()),
                error_message: None,
                active_generation_id: Some("gen_v1".into()),
                pending_generation_id: None,
                source_fingerprint: Some(fingerprint.clone()),
            }),
        });
        store.save_project(&project).expect("save project");

        let jm = Arc::new(JobManager::new());
        let pdf_service = Arc::new(SystemPdfService);
        let service = PdfPreviewService::new(store.clone(), pdf_service, jm.clone());
        let app = tauri::test::mock_app();
        let handle = app.handle();

        let reg = jm
            .register_or_get_active_job(
                handle,
                JobRegistrationInput {
                    project_id: project.id.clone(),
                    project_root_path: Some(project.root_path.clone()),
                    kind: JobKind::PdfPreviewRender,
                    display_label: Some("Preview Render".into()),
                    total: 2,
                    message: "Rendering".into(),
                    correlation_id: Some("corr-p5".into()),
                    idempotency_key: Some("key-p5".into()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        // Request cancellation before run_render
        jm.cancel_job(handle, &reg.snapshot.id).unwrap();

        // Perform run_render with cancellation requested
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(service.run_render(
            handle.clone(),
            reg.snapshot.id.clone(),
            project.id.clone(),
            doc_id.to_string(),
            "gen_v2".to_string(),
            fingerprint,
        ));

        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code, AppErrorCode::JobCancelled);

        let snap = jm.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snap.status, JobStatus::Cancelled);

        // Verify active preview in project snapshot remains gen_v1
        let updated_project = store.get_project_snapshot(project.id).unwrap();
        let doc = updated_project
            .documents
            .iter()
            .find(|d| d.id == doc_id)
            .unwrap();
        assert_eq!(
            doc.preview
                .as_ref()
                .and_then(|p| p.active_generation_id.as_deref()),
            Some("gen_v1")
        );
    }
}

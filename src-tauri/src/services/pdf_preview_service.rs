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
use crate::services::pdf_service::PdfService;
use crate::services::project_store::ProjectStore;
use crate::services::workflow_engine;

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
        let metadata_path = preview_metadata_path(&project, &document.id);
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
        let total_pages = document
            .preview
            .as_ref()
            .and_then(|preview| preview.page_count)
            .or_else(|| index.as_ref().map(|index| index.page_count))
            .unwrap_or(document.page_count);
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
        let metadata_path = preview_metadata_path(&project, &document.id);
        read_preview_index(&metadata_path)
            .map(|index| index.pages)
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
        let metadata_path = preview_metadata_path(&project, &document.id);
        let index = read_preview_index(&metadata_path).map_err(|error| {
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

        for page in &index.pages {
            let image_path = Path::new(&page.image_path);
            if !image_path.exists() || !image_path.is_file() {
                return Err(AppError {
                    code: AppErrorCode::PdfPreviewNotReady,
                    message:
                        "Soru metni çıkarılmadan önce PDF sayfa önizlemeleri oluşturulmalıdır."
                            .to_string(),
                    recoverable: true,
                    suggested_action: Some("Önce PDF sayfa önizlemelerini oluşturun.".to_string()),
                    technical_details: Some(format!("missing_preview_image={}", page.image_path)),
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        Ok(index.pages)
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
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let document = find_pdf_document(&project, &document_id)?.clone();
        validate_pdf_source(&document)?;

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
            1,
            "PDF önizlemeleri hazırlanıyor...".to_string(),
        )?;

        if let Err(error) = (|| -> Result<(), AppError> {
            set_document_preview_state(
                &mut project,
                &document_id,
                PdfPreviewState {
                    status: PdfPreviewStatus::Queued,
                    rendered_at: None,
                    page_count: None,
                    job_id: Some(job.id.clone()),
                    error_message: None,
                },
            )?;
            project.workflow = workflow_engine::evaluate_workflow(&project);
            self.project_store.save_project(&project)?;
            Ok(())
        })() {
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
    ) -> Result<(), AppError> {
        self.job_manager.set_running(&app, &job_id)?;

        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let document = find_pdf_document(&project, &document_id)?.clone();
        validate_pdf_source(&document)?;

        let pdf_path = PathBuf::from(&document.stored_path);
        let preview_dir = preview_dir(&project, &document.id);
        clear_directory(&preview_dir)?;
        std::fs::create_dir_all(&preview_dir).map_err(|e| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "PDF önizleme klasörü oluşturulamadı.".to_string(),
            recoverable: false,
            suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let rendered = self.pdf_service.render_all_pages(&pdf_path, &preview_dir)?;

        if rendered.is_empty() {
            return Err(AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "PDF önizlemesi oluşturulamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Lütfen geçerli bir PDF deneyin.".to_string()),
                technical_details: Some("No rendered pages returned.".to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let rendered_at = chrono::Utc::now().to_rfc3339();
        let page_count = rendered.len() as u32;
        let mut previews = Vec::new();
        for (index, image_path) in rendered.iter().enumerate() {
            let (width, height) = image::image_dimensions(image_path).map_err(|e| AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "Önizleme görseli okunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
                technical_details: Some(e.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?;

            previews.push(PdfPagePreview {
                document_id: document.id.clone(),
                page_number: (index + 1) as u32,
                image_path: image_path.to_string_lossy().to_string(),
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

        let metadata = PdfPreviewIndex {
            document_id: document.id.clone(),
            page_count,
            rendered_at: rendered_at.clone(),
            pages: previews.clone(),
        };
        write_preview_index(&preview_metadata_path(&project, &document.id), &metadata)?;

        let mut updated_project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        if let Some(document_entry) = updated_project
            .documents
            .iter_mut()
            .find(|entry| entry.id == document.id)
        {
            document_entry.page_count = page_count;
        }
        set_document_preview_state(
            &mut updated_project,
            &document_id,
            PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(rendered_at.clone()),
                page_count: Some(page_count),
                job_id: Some(job_id.clone()),
                error_message: None,
            },
        )?;
        updated_project.workflow = workflow_engine::evaluate_workflow(&updated_project);
        self.project_store.save_project(&updated_project)?;

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
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        set_document_preview_state(
            &mut project,
            document_id,
            PdfPreviewState {
                status: PdfPreviewStatus::Failed,
                rendered_at: None,
                page_count: None,
                job_id: Some(job_id.to_string()),
                error_message: Some(error.message.clone()),
            },
        )?;
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store.save_project(&project)?;
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

fn validate_pdf_source(document: &Document) -> Result<(), AppError> {
    let path = Path::new(&document.stored_path);
    if !path.exists() || !path.is_file() {
        return Err(AppError {
            code: AppErrorCode::PdfDocumentNotFound,
            message: "Sınav PDF dosyası bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("PDF dosyasını tekrar içe aktarın.".to_string()),
            technical_details: Some(document.stored_path.clone()),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
    Ok(())
}

fn preview_dir(project: &Project, document_id: &str) -> PathBuf {
    Path::new(&project.root_path)
        .join("cache")
        .join("page_previews")
        .join(document_id)
}

fn preview_metadata_path(project: &Project, document_id: &str) -> PathBuf {
    preview_dir(project, document_id).join("page_previews.json")
}

fn clear_directory(path: &Path) -> Result<(), AppError> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(|e| AppError {
            code: AppErrorCode::FileWriteFailed,
            message: "Eski önizleme önbelleği temizlenemedi.".to_string(),
            recoverable: false,
            suggested_action: Some("Klasör izinlerini kontrol edin.".to_string()),
            technical_details: Some(e.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
    }
    Ok(())
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
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_answer_ocr_records: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            student_scan_document_id: None,
            student_grouping_mode: None,
            student_pages_per_student: None,
            student_grouping_complete_at: None,
            documents: vec![],
            questions: vec![],
            scoring_records: vec![],
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
        let dir = preview_dir(&project, "doc-1");
        assert!(dir.ends_with("cache/page_previews/doc-1"));
        let metadata = preview_metadata_path(&project, "doc-1");
        assert!(metadata.ends_with("cache/page_previews/doc-1/page_previews.json"));
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
            sections: vec![],
            students: vec![],
            school_classes: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![],
            student_answer_ocr_records: vec![],
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
    fn require_ready_page_previews_returns_not_ready_when_image_missing() {
        let store = ProjectStore::new();
        let root = temp_project_root();
        let mut project = store
            .create_project("p".into(), root.to_string_lossy().to_string())
            .expect("project");
        project.student_scan_document_id = None;
        let preview_dir = preview_dir(&project, "doc-1");
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
        write_preview_index(&preview_metadata_path(&project, "doc-1"), &metadata)
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
            }),
        });
        store.save_project(&project).expect("save");

        let service = service_for_tests(store);
        let error = service
            .require_ready_page_previews(&project.id, "doc-1")
            .expect_err("image should be missing");
        assert_eq!(error.code, AppErrorCode::PdfPreviewNotReady);
    }
}

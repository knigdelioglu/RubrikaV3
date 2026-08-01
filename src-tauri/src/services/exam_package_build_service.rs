use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;
use tokio::time::sleep;
use uuid::Uuid;

use crate::domain::document::{DocumentRole, PdfPreviewStatus};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{JobKind, JobSnapshot, JobStatus};
use crate::domain::project::Project;
use crate::domain::question::{is_question_text_ready, TextFieldStatus};
use crate::domain::rubric::RubricStatus;
use crate::domain::workflow::{WorkflowAction, WorkflowSnapshot, WorkflowStage};
use crate::jobs::job_manager::JobManager;
use crate::platform::project_paths::TrustedProjectRoot;
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeService, ModelUseCase,
};
use crate::services::pdf_preview_service::PdfPreviewService;
use crate::services::project_store::ProjectStore;
use crate::services::question_text_service::{QuestionTextService, QuestionTextSource};
use crate::services::rubric_extraction_service::{
    RubricExtractionService, StartRubricPdfImportInput,
};

const MAX_EXPECTED_QUESTION_COUNT: u32 = 50;
const EXAM_PACKAGE_REVIEW_ROUTE: &str = "/exam-package-review";

#[derive(Clone)]
pub struct ExamPackageBuildService {
    project_store: ProjectStore,
    pdf_preview_service: Arc<PdfPreviewService>,
    model_runtime_service: ModelRuntimeService,
    question_text_service: Arc<QuestionTextService>,
    rubric_extraction_service: Arc<RubricExtractionService>,
    job_manager: Arc<JobManager>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartExamPackageBuildOutput {
    pub job_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageBuildPreviewResult {
    pub skipped: bool,
    pub page_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageBuildModelResult {
    pub skipped: bool,
    pub health_ok: bool,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageBuildQuestionTextResult {
    pub skipped: bool,
    pub confirmed: Vec<u32>,
    pub extracted: Vec<u32>,
    pub missing: Vec<u32>,
    pub partial_success: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageBuildRubricResult {
    pub skipped: bool,
    pub imported: Vec<u32>,
    pub missing: Vec<u32>,
    pub failed: Vec<u32>,
    pub partial_success: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageBuildResult {
    pub expected_question_count: u32,
    pub preview: ExamPackageBuildPreviewResult,
    pub model: ExamPackageBuildModelResult,
    pub question_text: ExamPackageBuildQuestionTextResult,
    pub rubric: ExamPackageBuildRubricResult,
    pub next_route: Option<String>,
}

impl ExamPackageBuildService {
    pub fn new(
        project_store: ProjectStore,
        pdf_preview_service: Arc<PdfPreviewService>,
        model_runtime_service: ModelRuntimeService,
        question_text_service: Arc<QuestionTextService>,
        rubric_extraction_service: Arc<RubricExtractionService>,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            project_store,
            pdf_preview_service,
            model_runtime_service,
            question_text_service,
            rubric_extraction_service,
            job_manager,
        }
    }

    pub async fn start<R: tauri::Runtime>(
        &self,
        app: AppHandle<R>,
        project_id: String,
        expected_question_count: u32,
    ) -> Result<StartExamPackageBuildOutput, AppError> {
        let project = self.preflight(&project_id, expected_question_count)?;
        let job = self.job_manager.start_job(
            &app,
            project_id.clone(),
            Some(project.root_path.clone()),
            JobKind::ExamPackageBuild,
            6,
            "Sınav paketi hazırlanıyor...".to_string(),
        )?;

        self.store_running_snapshot(&project, &project_id, &job.id)?;

        let service = self.clone();
        let app_handle = app.clone();
        let job_id = job.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service
                .run_build(
                    app_handle.clone(),
                    job_id.clone(),
                    project_id.clone(),
                    expected_question_count,
                )
                .await
            {
                let _ = service
                    .job_manager
                    .fail(&app_handle, &job_id, error.clone());
                if let Ok(mut project) = service.project_store.get_project_snapshot(project_id) {
                    project.workflow = WorkflowSnapshot {
                        current_stage: WorkflowStage::ExamPackageIncomplete,
                        blocking_reasons: vec![],
                        next_actions: vec![WorkflowAction {
                            code: "open_exam_package_review_page".to_string(),
                            label: "Sınav Paketini İncele".to_string(),
                            enabled: true,
                            disabled_reason: None,
                            command: Some("open_exam_package_review_page".to_string()),
                            requires: None,
                        }],
                        current_stage_label: "Sınav Paketi Eksik".to_string(),
                        summary: crate::domain::workflow::WorkflowSummary {
                            text: Some("Sınav paketi oluşturma başarısız oldu.".to_string()),
                            ..Default::default()
                        },
                    };
                    service.write_project_log(
                        &project.id,
                        "exam_package_build_failed",
                        &job_id,
                        Some(error.message.as_str()),
                    );
                    let _ = service.project_store.commit_snapshot_cas(&project);
                }
            }
        });

        Ok(StartExamPackageBuildOutput {
            job_id: job.id,
            status: "queued".to_string(),
        })
    }

    fn preflight(
        &self,
        project_id: &str,
        expected_question_count: u32,
    ) -> Result<Project, AppError> {
        if expected_question_count == 0 {
            return Err(AppError {
                code: AppErrorCode::QuestionCountMissing,
                message: "Soru sayısı belirlenmedi.".to_string(),
                recoverable: true,
                suggested_action: Some("1 ile 50 arasında bir soru sayısı girin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if expected_question_count > MAX_EXPECTED_QUESTION_COUNT {
            return Err(AppError {
                code: AppErrorCode::ExamPackageBuildPrecheckFailed,
                message: "Soru sayısı çok büyük.".to_string(),
                recoverable: true,
                suggested_action: Some("1 ile 50 arasında bir sayı girin.".to_string()),
                technical_details: Some(format!(
                    "expected_question_count={expected_question_count}"
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;

        if project.expected_question_count != Some(expected_question_count) {
            project.expected_question_count = Some(expected_question_count);
            self.project_store
                .commit_snapshot_cas(&project)
                .map(|_| ())?;
        }

        let exam_source_exists = project
            .documents
            .iter()
            .any(|document| document.role == DocumentRole::ExamSource);
        if !exam_source_exists {
            return Err(AppError {
                code: AppErrorCode::ExamSourcePdfMissing,
                message: "Orijinal sınav PDF’i yüklenmedi.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce orijinal sınav PDF’ini yükleyin.".to_string()),
                technical_details: Some(format!("project_id={project_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let rubric_exists = project.documents.iter().any(|document| {
            matches!(
                document.role,
                DocumentRole::AnswerKey | DocumentRole::Rubric
            )
        });
        if !rubric_exists {
            return Err(AppError {
                code: AppErrorCode::RubricDocumentMissing,
                message: "Rubrik / cevap anahtarı PDF’i yüklenmedi.".to_string(),
                recoverable: true,
                suggested_action: Some("Cevap anahtarı veya rubrik PDF’ini yükleyin.".to_string()),
                technical_details: Some(format!("project_id={project_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let trusted_root = self.project_store.trusted_project_root(project_id)?;
        self.ensure_root_writable(&trusted_root)?;
        Ok(project)
    }

    fn ensure_root_writable(&self, trusted_root: &TrustedProjectRoot) -> Result<(), AppError> {
        let probe = trusted_root.managed(&format!(".rubrika-write-probe-{}", Uuid::new_v4()))?;
        let probe_path = trusted_root.prepare_write_target(&probe)?;
        std::fs::write(&probe_path, b"probe").map_err(|error| AppError {
            code: AppErrorCode::ExamPackageBuildPrecheckFailed,
            message: "Proje kök dizinine yazılamıyor.".to_string(),
            recoverable: false,
            suggested_action: Some("Proje klasörü izinlerini kontrol edin.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;
        let _ = std::fs::remove_file(probe_path);
        Ok(())
    }

    fn store_running_snapshot(
        &self,
        project: &Project,
        project_id: &str,
        job_id: &str,
    ) -> Result<(), AppError> {
        let mut project = project.clone();
        project.workflow = WorkflowSnapshot {
            current_stage: WorkflowStage::ExamPackageBuildRunning,
            blocking_reasons: vec![],
            next_actions: vec![],
            current_stage_label: "Sınav Paketi Oluşturuluyor".to_string(),
            summary: crate::domain::workflow::WorkflowSummary {
                text: Some("Sınav paketi hazırlanıyor...".to_string()),
                ..Default::default()
            },
        };
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        self.write_project_log(
            project_id,
            "exam_package_build_started",
            job_id,
            Some("Sınav paketi hazırlanıyor..."),
        );
        Ok(())
    }

    async fn run_build<R: tauri::Runtime>(
        &self,
        app: AppHandle<R>,
        job_id: String,
        project_id: String,
        expected_question_count: u32,
    ) -> Result<(), AppError> {
        self.job_manager.set_running(&app, &job_id)?;
        let cancel_token = self.job_manager.get_cancellation_token(&job_id);
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Sınav paketi oluşturma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        self.job_manager.update_progress(
            &app,
            &job_id,
            1,
            6,
            "PDF önizlemeleri kontrol ediliyor...".to_string(),
        )?;

        let mut project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let exam_source = resolve_document(&project, DocumentRole::ExamSource)?;
        let preview_result = self
            .ensure_exam_preview(&app, &project_id, &project, &exam_source.id)
            .await?;

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Sınav paketi oluşturma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        self.job_manager.update_progress(
            &app,
            &job_id,
            2,
            6,
            "Gemma model sunucusu hazırlanıyor...".to_string(),
        )?;
        let model_result = self.ensure_model_ready().await?;

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Sınav paketi oluşturma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        self.job_manager.update_progress(
            &app,
            &job_id,
            3,
            6,
            "Soru metinleri çıkarılıyor...".to_string(),
        )?;
        let _question_text_result = self
            .ensure_question_texts(&app, &project_id, &project, expected_question_count)
            .await?;

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Sınav paketi oluşturma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        self.job_manager.update_progress(
            &app,
            &job_id,
            4,
            6,
            "Rubrik PDF’den bilgiler çıkarılıyor...".to_string(),
        )?;

        let _rubric_result = self
            .ensure_rubrics(&app, &project_id, &project, expected_question_count)
            .await?;

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Sınav paketi oluşturma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        self.job_manager.update_progress(
            &app,
            &job_id,
            5,
            6,
            "İnceleme paketi hazırlanıyor...".to_string(),
        )?;

        project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let question_text_final = self.summarize_question_text(&project, expected_question_count);
        let rubric_final = self.summarize_rubric(&project, expected_question_count);
        let (current_stage, summary) = self.final_stage(&question_text_final, &rubric_final);

        let next_actions = vec![WorkflowAction {
            code: "open_exam_package_review_page".to_string(),
            label: "Sınav Paketini İncele".to_string(),
            enabled: true,
            disabled_reason: None,
            command: Some("open_exam_package_review_page".to_string()),
            requires: None,
        }];

        let workflow = WorkflowSnapshot {
            current_stage,
            blocking_reasons: vec![],
            next_actions,
            current_stage_label: "Sınav Paketi".to_string(),
            summary: crate::domain::workflow::WorkflowSummary {
                text: Some(summary),
                ..Default::default()
            },
        };
        project.workflow = workflow;
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;

        let result = ExamPackageBuildResult {
            expected_question_count,
            preview: preview_result,
            model: model_result,
            question_text: question_text_final,
            rubric: rubric_final,
            next_route: Some(format!(
                "{}?projectId={}",
                EXAM_PACKAGE_REVIEW_ROUTE, project_id
            )),
        };

        self.write_project_log(
            &project_id,
            "exam_package_build_finished",
            &job_id,
            result.next_route.as_deref(),
        );
        self.job_manager.succeed(
            &app,
            &job_id,
            Some(serde_json::to_value(&result).map_err(|error| AppError {
                code: AppErrorCode::ProjectSaveFailed,
                message: "Sınav paketi sonucu serialize edilemedi.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: Some(error.to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            })?),
        )?;
        Ok(())
    }

    async fn ensure_exam_preview<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        project_id: &str,
        _project: &Project,
        document_id: &str,
    ) -> Result<ExamPackageBuildPreviewResult, AppError> {
        let preview_status = self
            .pdf_preview_service
            .get_pdf_preview_status(project_id, document_id)?;
        if preview_status.status == PdfPreviewStatus::Ready && preview_status.preview_count > 0 {
            return Ok(ExamPackageBuildPreviewResult {
                skipped: true,
                page_count: Some(preview_status.page_count),
            });
        }

        let active_job_id = preview_status.job_id.clone();
        let job_id = if let Some(job_id) = active_job_id {
            job_id
        } else {
            let render = self.pdf_preview_service.start_render(
                app.clone(),
                project_id.to_string(),
                document_id.to_string(),
            )?;
            render.job_id
        };
        self.wait_for_job_terminal(&job_id).await?;
        let refreshed = self
            .pdf_preview_service
            .get_pdf_preview_status(project_id, document_id)?;
        if refreshed.status != PdfPreviewStatus::Ready {
            return Err(AppError {
                code: AppErrorCode::PdfRenderFailed,
                message: "PDF önizlemeleri hazırlanamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("PDF önizlemelerini yeniden oluşturun.".to_string()),
                technical_details: Some(format!("status={:?}", refreshed.status)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        Ok(ExamPackageBuildPreviewResult {
            skipped: false,
            page_count: Some(refreshed.page_count),
        })
    }

    async fn ensure_model_ready(&self) -> Result<ExamPackageBuildModelResult, AppError> {
        let runtime_status = self
            .model_runtime_service
            .ensure_ready(
                None,
                ModelRuntimeRequest {
                    use_case: ModelUseCase::RubricPdfImport,
                    capability: ModelCapability::Vision,
                    requires_mmproj: true,
                    timeout_seconds: 180,
                },
            )
            .await?;
        Ok(ExamPackageBuildModelResult {
            skipped: false,
            health_ok: runtime_status.health_ok,
            mode: "managed".to_string(),
        })
    }

    async fn ensure_question_texts<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        project_id: &str,
        project: &Project,
        expected_question_count: u32,
    ) -> Result<ExamPackageBuildQuestionTextResult, AppError> {
        let summary = self.summarize_question_text(project, expected_question_count);
        if summary.skipped {
            return Ok(summary);
        }

        let all_available = !project.questions.is_empty()
            && project.questions.len() >= expected_question_count as usize
            && project
                .questions
                .iter()
                .take(expected_question_count as usize)
                .all(|question| {
                    matches!(
                        question.question_text.status,
                        TextFieldStatus::Confirmed
                            | TextFieldStatus::Suggested
                            | TextFieldStatus::Edited
                    )
                });
        if all_available {
            return Ok(summary);
        }

        let existing_active_job =
            self.latest_active_job(project_id, JobKind::QuestionTextExtraction);
        let job = if let Some(job) = existing_active_job {
            job
        } else {
            self.question_text_service
                .start_extraction(
                    app.clone(),
                    project_id.to_string(),
                    None,
                    QuestionTextSource::ExamPdf,
                )
                .await?
        };
        let job_id = job.id.clone();
        let finished = self.wait_for_job_terminal(&job_id).await?;
        if finished.status == JobStatus::Failed {
            return Err(finished.error.unwrap_or_else(|| AppError {
                code: AppErrorCode::QuestionTextExtractionFailed,
                message: "Soru metni çıkarımı başarısız oldu.".to_string(),
                recoverable: true,
                suggested_action: Some("Soru metni çıkarımını tekrar deneyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            }));
        }

        let refreshed = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        Ok(self.summarize_question_text(&refreshed, expected_question_count))
    }

    async fn ensure_rubrics<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        project_id: &str,
        project: &Project,
        expected_question_count: u32,
    ) -> Result<ExamPackageBuildRubricResult, AppError> {
        let summary = self.summarize_rubric(project, expected_question_count);
        if summary.skipped {
            return Ok(summary);
        }

        let all_available = !project.questions.is_empty()
            && project.questions.len() >= expected_question_count as usize
            && project
                .questions
                .iter()
                .take(expected_question_count as usize)
                .all(|question| {
                    matches!(
                        question.rubric.status,
                        RubricStatus::Confirmed
                            | RubricStatus::Suggested
                            | RubricStatus::Imported
                            | RubricStatus::Manual
                    )
                });
        if all_available {
            return Ok(summary);
        }

        let _ = self
            .model_runtime_service
            .ensure_ready(
                None,
                ModelRuntimeRequest {
                    use_case: ModelUseCase::RubricPdfImport,
                    capability: ModelCapability::Vision,
                    requires_mmproj: true,
                    timeout_seconds: 180,
                },
            )
            .await?;

        let existing_active_job = self.latest_active_job(project_id, JobKind::RubricPdfImport);
        let job_id = if let Some(job) = existing_active_job {
            job.id
        } else {
            let output = self
                .rubric_extraction_service
                .start_import(
                    app.clone(),
                    StartRubricPdfImportInput {
                        project_id: project_id.to_string(),
                        document_id: None,
                        expected_question_count: Some(expected_question_count),
                    },
                )
                .await?;
            output.job_id
        };
        let finished = self.wait_for_job_terminal(&job_id).await?;
        if finished.status == JobStatus::Failed {
            return Err(finished.error.unwrap_or_else(|| AppError {
                code: AppErrorCode::RubricImportEmpty,
                message: "Rubrik içe aktarılamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Rubrik PDF’ini yeniden deneyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            }));
        }

        let refreshed = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        Ok(self.summarize_rubric(&refreshed, expected_question_count))
    }

    fn summarize_question_text(
        &self,
        project: &Project,
        expected_question_count: u32,
    ) -> ExamPackageBuildQuestionTextResult {
        let confirmed = project
            .questions
            .iter()
            .filter(|question| is_question_text_ready(&question.question_text))
            .map(|question| question.number)
            .collect::<Vec<_>>();
        let extracted = project
            .questions
            .iter()
            .filter(|question| question.question_text.status == TextFieldStatus::Suggested)
            .map(|question| question.number)
            .collect::<Vec<_>>();
        let missing = project
            .questions
            .iter()
            .filter(|question| {
                matches!(
                    question.question_text.status,
                    TextFieldStatus::Missing | TextFieldStatus::Failed
                )
            })
            .map(|question| question.number)
            .collect::<Vec<_>>();
        let partial_success =
            !missing.is_empty() || confirmed.len() < expected_question_count as usize;
        ExamPackageBuildQuestionTextResult {
            skipped: extracted.is_empty() && missing.is_empty() && !project.questions.is_empty(),
            confirmed,
            extracted,
            missing,
            partial_success,
        }
    }

    fn summarize_rubric(
        &self,
        project: &Project,
        expected_question_count: u32,
    ) -> ExamPackageBuildRubricResult {
        let imported = project
            .questions
            .iter()
            .filter(|question| {
                matches!(
                    question.rubric.status,
                    RubricStatus::Imported | RubricStatus::Manual | RubricStatus::Confirmed
                )
            })
            .map(|question| question.number)
            .collect::<Vec<_>>();
        let missing = project
            .questions
            .iter()
            .filter(|question| question.rubric.status == RubricStatus::Missing)
            .map(|question| question.number)
            .collect::<Vec<_>>();
        let failed = project
            .questions
            .iter()
            .filter(|question| question.rubric.status == RubricStatus::Invalid)
            .map(|question| question.number)
            .collect::<Vec<_>>();
        let partial_success = !missing.is_empty()
            || !failed.is_empty()
            || imported.len() < expected_question_count as usize;
        ExamPackageBuildRubricResult {
            skipped: imported.is_empty()
                && missing.is_empty()
                && failed.is_empty()
                && !project.questions.is_empty(),
            imported,
            missing,
            failed,
            partial_success,
        }
    }

    fn final_stage(
        &self,
        question_text: &ExamPackageBuildQuestionTextResult,
        rubric: &ExamPackageBuildRubricResult,
    ) -> (WorkflowStage, String) {
        if question_text.partial_success || rubric.partial_success {
            if question_text.missing.is_empty()
                && rubric.missing.is_empty()
                && rubric.failed.is_empty()
            {
                return (
                    WorkflowStage::ExamPackageReviewNeeded,
                    "Sınav paketi incelemeye hazır.".to_string(),
                );
            }
            return (
                WorkflowStage::ExamPackageIncomplete,
                "Sınav paketi eksik içerikle tamamlandı.".to_string(),
            );
        }

        (
            WorkflowStage::ExamPackageReadyForQep,
            "Sınav paketi incelemeye hazır.".to_string(),
        )
    }

    async fn wait_for_job_terminal(&self, job_id: &str) -> Result<JobSnapshot, AppError> {
        loop {
            let snapshot = self.job_manager.get_job_snapshot(job_id)?;
            match snapshot.status {
                JobStatus::Queued | JobStatus::Running => {
                    sleep(Duration::from_millis(500)).await;
                }
                JobStatus::Succeeded
                | JobStatus::Partial
                | JobStatus::Failed
                | JobStatus::Cancelled
                | JobStatus::Interrupted => {
                    return Ok(snapshot);
                }
            }
        }
    }

    fn latest_active_job(&self, project_id: &str, kind: JobKind) -> Option<JobSnapshot> {
        let now = chrono::Utc::now();
        self.job_manager
            .list_jobs(project_id)
            .ok()
            .and_then(|jobs| {
                jobs.into_iter().find(|job| {
                    if job.kind != kind
                        || !matches!(job.status, JobStatus::Queued | JobStatus::Running)
                    {
                        return false;
                    }
                    let updated_at = chrono::DateTime::parse_from_rfc3339(&job.updated_at)
                        .ok()
                        .map(|date| date.with_timezone(&chrono::Utc))
                        .unwrap_or(now);
                    now.signed_duration_since(updated_at).num_minutes() < 15
                })
            })
    }

    fn write_project_log(
        &self,
        project_id: &str,
        event: &str,
        correlation_id: &str,
        message: Option<&str>,
    ) {
        let Ok(trusted_root) = self.project_store.trusted_project_root(project_id) else {
            return;
        };
        let Ok(path) = trusted_root.managed("logs/events.jsonl") else {
            return;
        };
        let Ok(path) = trusted_root.prepare_write_target(&path) else {
            return;
        };
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "event": event,
            "project_id": project_id,
            "correlation_id": correlation_id,
            "message": message,
        });
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = std::io::Write::write_all(&mut file, format!("{entry}\n").as_bytes());
        }
    }
}

fn resolve_document(
    project: &Project,
    role: DocumentRole,
) -> Result<&crate::domain::document::Document, AppError> {
    project
        .documents
        .iter()
        .find(|document| document.role == role)
        .ok_or_else(|| AppError {
            code: match role {
                DocumentRole::ExamSource => AppErrorCode::ExamSourcePdfMissing,
                DocumentRole::AnswerKey | DocumentRole::Rubric => {
                    AppErrorCode::RubricDocumentMissing
                }
                _ => AppErrorCode::ExamPackageBuildPrecheckFailed,
            },
            message: "Gerekli belge bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Gerekli belgeyi yükleyin.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, DocumentRole};
    use crate::services::document_content_extraction_service::DocumentContentExtractionService;
    use crate::services::llama_server_gateway::LlamaServerGateway;
    use crate::services::model_config_service::ModelConfigService;
    use crate::services::model_input_image_service::ModelInputImageService;
    use crate::services::model_process_manager::ModelProcessManager;
    use crate::services::model_runtime_service::ModelRuntimeService;
    use crate::services::pdf_service::SystemPdfService;

    fn temp_project_root() -> String {
        let root =
            std::env::temp_dir().join(format!("rubrika-v3-exam-package-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().to_string()
    }

    fn test_service(project_store: ProjectStore) -> ExamPackageBuildService {
        test_service_with_job_manager(project_store, Arc::new(JobManager::new()))
    }

    fn test_service_with_job_manager(
        project_store: ProjectStore,
        job_manager: Arc<JobManager>,
    ) -> ExamPackageBuildService {
        let pdf_preview_service = Arc::new(PdfPreviewService::new(
            project_store.clone(),
            Arc::new(SystemPdfService),
            job_manager.clone(),
        ));
        let model_gateway_impl = Arc::new(LlamaServerGateway::default());
        let model_config = ModelConfigService::new();
        let model_process_manager =
            ModelProcessManager::new(model_config.clone(), model_gateway_impl.clone());
        let model_runtime_service = ModelRuntimeService::new(model_config, model_process_manager);
        let model_input_image_service = Arc::new(ModelInputImageService::default());
        let document_content_extraction_service = Arc::new(DocumentContentExtractionService::new(
            model_input_image_service,
        ));
        let question_text_service = Arc::new(QuestionTextService::new(
            project_store.clone(),
            model_gateway_impl.clone(),
            model_runtime_service.clone(),
            pdf_preview_service.clone(),
            document_content_extraction_service.clone(),
            job_manager.clone(),
        ));
        let rubric_extraction_service = Arc::new(RubricExtractionService::new(
            project_store.clone(),
            model_gateway_impl,
            job_manager.clone(),
            model_runtime_service.clone(),
            Arc::new(SystemPdfService),
            document_content_extraction_service,
        ));

        ExamPackageBuildService::new(
            project_store,
            pdf_preview_service,
            model_runtime_service,
            question_text_service,
            rubric_extraction_service,
            job_manager,
        )
    }

    fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone()
    }

    #[tokio::test]
    async fn start_fails_without_exam_source() {
        let store = ProjectStore::new();
        let project = store
            .create_project("Test".to_string(), temp_project_root())
            .unwrap();
        let service = test_service(store);

        let err = service
            .start(mock_app(), project.id.clone(), 6)
            .await
            .unwrap_err();

        assert_eq!(err.code, AppErrorCode::ExamSourcePdfMissing);
    }

    #[tokio::test]
    async fn start_fails_without_rubric_document() {
        let store = ProjectStore::new();
        let project = store
            .create_project("Test".to_string(), temp_project_root())
            .unwrap();
        let mut opened = store.get_project_snapshot(project.id.clone()).unwrap();
        opened.documents.push(Document {
            id: "exam-source".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: None,
        });
        store.save_project(&opened).unwrap();
        let service = test_service(store);

        let err = service
            .start(mock_app(), project.id.clone(), 6)
            .await
            .unwrap_err();

        assert_eq!(err.code, AppErrorCode::RubricDocumentMissing);
    }

    #[tokio::test]
    async fn start_fails_for_zero_question_count() {
        let store = ProjectStore::new();
        let project = store
            .create_project("Test".to_string(), temp_project_root())
            .unwrap();
        let service = test_service(store);

        let err = service
            .start(mock_app(), project.id.clone(), 0)
            .await
            .unwrap_err();

        assert_eq!(err.code, AppErrorCode::QuestionCountMissing);
    }

    #[test]
    fn summarize_rubric_marks_confirmed_questions_ready() {
        let store = ProjectStore::new();
        let service = test_service(store.clone());
        let mut first = crate::domain::question::default_question(1);
        first.rubric.status = RubricStatus::Confirmed;
        first.rubric.max_score = Some(10.0);
        first.rubric.expected_answer = Some("A".to_string());
        let mut second = crate::domain::question::default_question(2);
        second.rubric.status = RubricStatus::Confirmed;
        second.rubric.max_score = Some(5.0);
        second.rubric.expected_answer = Some("B".to_string());
        let project = Project {
            expected_question_count: None,
            exam_package_freeze: None,
            id: "p1".to_string(),
            name: "Project".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            root_path: temp_project_root(),
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
            questions: vec![first, second],
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
        let summary = service.summarize_rubric(&project, 2);
        assert!(summary.missing.is_empty());
        assert!(summary.failed.is_empty());
        assert_eq!(summary.imported, vec![1, 2]);
        assert!(!summary.partial_success);
    }

    #[tokio::test]
    async fn proof_16_exam_package_build_cancel_preserves_unfrozen_state() {
        use crate::domain::job::{DuplicatePolicy, JobKind, JobStatus};
        use crate::jobs::job_manager::{JobManager, JobRegistrationInput};

        let store = ProjectStore::new();
        let mut project = store
            .create_project("proj_p16".to_string(), temp_project_root())
            .unwrap();

        project.documents.push(Document {
            id: "exam-source-p16".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: None,
        });
        project.documents.push(Document {
            id: "rubric-doc-p16".to_string(),
            role: DocumentRole::AnswerKey,
            file_name: "rubric.pdf".to_string(),
            stored_path: "rubric.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: None,
        });
        store.save_project(&project).unwrap();

        let jm = std::sync::Arc::new(JobManager::new());
        let service = test_service_with_job_manager(store.clone(), jm.clone());
        let app = mock_app();

        let reg = jm
            .register_or_get_active_job(
                &app,
                JobRegistrationInput {
                    project_id: project.id.clone(),
                    project_root_path: Some(project.root_path.clone()),
                    kind: JobKind::ExamPackageBuild,
                    display_label: Some("Exam Package Build".into()),
                    total: 6,
                    message: "Building".into(),
                    correlation_id: Some("corr-p16".into()),
                    idempotency_key: Some("key-p16".into()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        // Request cancellation
        jm.cancel_job(&app, &reg.snapshot.id).unwrap();

        let res = service
            .run_build(app, reg.snapshot.id.clone(), project.id.clone(), 1)
            .await;

        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code, AppErrorCode::JobCancelled);

        let snap = jm.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snap.status, JobStatus::Cancelled);

        // Verify package freeze is NOT populated
        let updated = store.get_project_snapshot(project.id).unwrap();
        assert!(updated.exam_package_freeze.is_none());
    }
}

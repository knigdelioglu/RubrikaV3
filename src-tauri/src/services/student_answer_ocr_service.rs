use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use uuid::Uuid;

use crate::domain::document::{DocumentRole, PdfPreviewStatus};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::JobKind;
use crate::domain::model::{
    ModelInputImageKind, StudentAnswerOcrIssueCorrectionRequest, StudentAnswerOcrRequest,
    StudentIdentityOcrRequest,
};
use crate::domain::project::Project;
use crate::domain::question::{is_question_text_ready, AnswerType};
use crate::domain::scoring::ScoringReviewStatus;
use crate::domain::student::{
    OcrCriticalTermWarning, OcrGeneration, OcrGenerationStatus, OcrImagePreprocessDiagnostics,
    OcrImagePreprocessMode, OcrSuggestedCorrection, OcrTeacherReviewStatus, OcrUncertainSpan,
    StudentAnswerOcrCropBBox, StudentAnswerOcrParseDiagnostics, StudentAnswerOcrRecord,
    StudentAnswerOcrRenderDiagnostics, StudentAnswerOcrStatus, StudentIdentityOcrRecord,
    StudentSubmission, StudentSubmissionStatus,
};
use crate::jobs::job_manager::JobManager;
use crate::services::model_gateway::ModelGateway;
use crate::services::model_input_image_service::ModelInputImageService;
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeService, ModelUseCase,
};
use crate::services::ocr_image_preprocess_service::OcrImagePreprocessService;
use crate::services::pdf_preview_service::PdfPreviewService;
use crate::services::project_store::ProjectStore;
use crate::services::student_answer_crop_service::StudentAnswerCropService;
use crate::services::workflow_engine;

const PROMPT_VERSION: &str = "student_answer_ocr_v3_verbatim";
const ISSUE_CORRECTION_PROMPT_VERSION: &str = "student_answer_ocr_issue_correction_v1";
const PREPROCESS_VERSION: &str = "ocr_image_preprocess_v2";
const CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING: &str = "critical_keyword_ocr_uncertain";
const CRITICAL_KEYWORD_OCR_UNCERTAIN_REASON: &str = "critical_keyword_similarity";
const PREPROCESS_VARIANTS: [OcrImagePreprocessMode; 5] = [
    OcrImagePreprocessMode::Original,
    OcrImagePreprocessMode::CleanGrayscale,
    OcrImagePreprocessMode::HandwritingEnhanced,
    OcrImagePreprocessMode::HighContrast,
    OcrImagePreprocessMode::HighContrastBw,
];

#[derive(Clone)]
pub struct StudentAnswerOcrService {
    project_store: ProjectStore,
    model_gateway: Arc<dyn ModelGateway>,
    model_runtime_service: ModelRuntimeService,
    _pdf_preview_service: Arc<PdfPreviewService>,
    model_input_image_service: Arc<ModelInputImageService>,
    ocr_image_preprocess_service: Arc<OcrImagePreprocessService>,
    crop_service: Arc<StudentAnswerCropService>,
    job_manager: Arc<JobManager>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStudentAnswerOcrOutput {
    pub job_id: String,
    pub status: String,
    pub rerun: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStudentIdentityOcrOutput {
    pub job_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrJobResult {
    pub total: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub reviewed: u32,
    pub needs_review: u32,
    pub partial: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildStudentAnswerOcrIssuesOutput {
    pub project_id: String,
    pub updated_records: u32,
    pub updated_issues: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestStudentAnswerOcrIssueCorrectionWithModelOutput {
    pub suggestion: crate::domain::model::StudentAnswerOcrIssueCorrectionOutput,
    pub raw_model_output: String,
    pub used_image_ref: Option<String>,
    pub prompt_version: String,
    pub model_request_metadata: Option<serde_json::Value>,
}

impl StudentAnswerOcrService {
    pub fn new(
        project_store: ProjectStore,
        model_gateway: Arc<dyn ModelGateway>,
        model_runtime_service: ModelRuntimeService,
        pdf_preview_service: Arc<PdfPreviewService>,
        model_input_image_service: Arc<ModelInputImageService>,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            project_store: project_store.clone(),
            model_gateway,
            model_runtime_service,
            _pdf_preview_service: pdf_preview_service.clone(),
            model_input_image_service,
            ocr_image_preprocess_service: Arc::new(OcrImagePreprocessService),
            crop_service: Arc::new(StudentAnswerCropService::new(
                project_store.clone(),
                pdf_preview_service,
            )),
            job_manager,
        }
    }

    pub async fn start<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        project_id: String,
        force_rerun: bool,
    ) -> Result<StartStudentAnswerOcrOutput, AppError> {
        let project = self.preflight(&project_id, force_rerun).await?;
        let active_job_exists = self
            .job_manager
            .list_jobs(&project_id)?
            .into_iter()
            .any(|job| {
                job.kind == JobKind::StudentAnswerOcr
                    && matches!(
                        job.status,
                        crate::domain::job::JobStatus::Queued
                            | crate::domain::job::JobStatus::Running
                    )
            });
        if active_job_exists {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Öğrenci cevap OCR işi zaten çalışıyor.".to_string(),
                recoverable: true,
                suggested_action: Some("Mevcut OCR işinin bitmesini bekleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if !force_rerun && !project.student_answer_ocr_records.is_empty() {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Öğrenci cevap OCR’ı zaten başlatılmış.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "OCR’ı yeniden çalıştırmak için onaylı yeniden çalıştırma butonunu kullanın."
                        .to_string(),
                ),
                technical_details: Some(format!(
                    "existing_records={}",
                    project.student_answer_ocr_records.len()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let total = (project.student_submissions.len() * project.questions.len()) as u32;
        let job = self.job_manager.start_job(
            &app,
            project_id.clone(),
            Some(project.root_path.clone()),
            JobKind::StudentAnswerOcr,
            total,
            "Öğrenci cevap OCR’ı hazırlanıyor...".to_string(),
        )?;

        let document = self
            .active_student_scan_document(&project)
            .cloned()
            .ok_or_else(|| {
                ocr_error(
                    AppErrorCode::StudentScanNotFound,
                    "Öğrenci cevap PDF’i bulunamadı.",
                )
            })?;
        let trusted_root = self.project_store.trusted_project_root(&project_id)?;
        let source_path = document.resolve_path_with_root(&trusted_root)?;
        let source_fingerprint = file_fingerprint(&source_path)?;
        let generation_ids = project
            .student_submissions
            .iter()
            .map(|submission| (submission.id.clone(), Uuid::new_v4().to_string()))
            .collect::<Vec<_>>();
        let generation_ids_for_commit = generation_ids.clone();
        let job_id_for_generation = job.id.clone();
        let source_document_id = document.id.clone();
        let source_fingerprint_for_queue = source_fingerprint.clone();
        let queue_result = self
            .project_store
            .mutate(
                &project_id,
                crate::services::project_store::MutationOptions::new("queue_ocr_generation"),
                move |current, context| {
                    let now = chrono::Utc::now();
                    for (submission_id, generation_id) in &generation_ids_for_commit {
                        if current
                            .student_answer_ocr_generations
                            .iter()
                            .any(|generation| generation.generation_id == *generation_id)
                        {
                            return Err(ocr_error(
                                AppErrorCode::OcrGenerationConflict,
                                "OCR yeniden çalıştırma generation çakışması oluştu.",
                            ));
                        }
                        current.student_answer_ocr_generations.push(OcrGeneration {
                            generation_id: generation_id.clone(),
                            submission_id: submission_id.clone(),
                            source_fingerprint: source_fingerprint_for_queue.clone(),
                            created_at: now,
                            model_name: Some("gemma".to_string()),
                            prompt_version: PROMPT_VERSION.to_string(),
                            status: OcrGenerationStatus::Candidate,
                            result: vec![],
                            diagnostics: None,
                            teacher_review_status: OcrTeacherReviewStatus::Pending,
                            created_by_job_id: job_id_for_generation.clone(),
                            source_document_id: source_document_id.clone(),
                            source_storage_revision: context.current_revision,
                            failure_reason: None,
                        });
                    }
                    for submission in &mut current.student_submissions {
                        if generation_ids_for_commit
                            .iter()
                            .any(|(id, _)| id == &submission.id)
                        {
                            submission.status = StudentSubmissionStatus::OcrRunning;
                            submission.updated_at = Some(now.to_rfc3339());
                        }
                    }
                    Ok(())
                },
            )
            .map(|_| ());
        if let Err(error) = queue_result {
            let _ = self.job_manager.fail(&app, &job.id, error.clone());
            return Err(error);
        }

        let service = self.clone();
        let app_handle = app.clone();
        let job_id = job.id.clone();
        let project_id_for_run = project_id.clone();
        let generation_ids_for_run = generation_ids.clone();
        let source_fingerprint_for_run = source_fingerprint.clone();
        tauri::async_runtime::spawn(async move {
            let run_result = service
                .run(
                    app_handle.clone(),
                    job_id.clone(),
                    project_id_for_run.clone(),
                    generation_ids_for_run.clone(),
                    source_fingerprint_for_run,
                )
                .await;
            if let Err(error) = run_result {
                let generation_status = if error.code == AppErrorCode::OcrGenerationStale {
                    OcrGenerationStatus::Stale
                } else {
                    OcrGenerationStatus::Failed
                };
                let _ = service.mark_generation_status(
                    &project_id_for_run,
                    &generation_ids_for_run,
                    generation_status,
                    Some(error.message.clone()),
                );
                let _ = service
                    .job_manager
                    .fail(&app_handle, &job_id, error.clone());
            }
        });

        Ok(StartStudentAnswerOcrOutput {
            job_id: job.id,
            status: "queued".to_string(),
            rerun: force_rerun,
        })
    }

    pub async fn start_identity_ocr<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        project_id: String,
    ) -> Result<StartStudentIdentityOcrOutput, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        if project.student_identity_crop_template.is_none() {
            return Err(AppError {
                code: AppErrorCode::CropRegionMissing,
                message: "Kimlik alanı crop şablonu eksik.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Önce Crop Şablonu sayfasında kimlik alanını seçin.".to_string(),
                ),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let document = self
            .active_student_scan_document(&project)
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentScanNotFound,
                message: "Öğrenci cevap PDF’i bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci cevap PDF’ini yükleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        if !matches!(
            document.preview.as_ref().map(|preview| &preview.status),
            Some(PdfPreviewStatus::Ready)
        ) {
            return Err(AppError {
                code: AppErrorCode::StudentScanPreviewNotReady,
                message: "Öğrenci cevap PDF önizlemeleri hazır değil.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce öğrenci PDF önizlemelerini oluşturun.".to_string()),
                technical_details: Some(format!("document_id={}", document.id)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let active_job_exists = self
            .job_manager
            .list_jobs(&project_id)?
            .into_iter()
            .any(|job| {
                job.kind == JobKind::StudentIdentityOcr
                    && matches!(
                        job.status,
                        crate::domain::job::JobStatus::Queued
                            | crate::domain::job::JobStatus::Running
                    )
            });
        if active_job_exists {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Kimlik OCR işi zaten çalışıyor.".to_string(),
                recoverable: true,
                suggested_action: Some("Mevcut kimlik OCR işinin bitmesini bekleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let total = project.student_submissions.len() as u32;
        let job = self.job_manager.start_job(
            &app,
            project_id.clone(),
            Some(project.root_path.clone()),
            JobKind::StudentIdentityOcr,
            total,
            "Kimlik OCR’ı hazırlanıyor...".to_string(),
        )?;
        let service = self.clone();
        let app_handle = app.clone();
        let job_id = job.id.clone();
        tauri::async_runtime::spawn(async move {
            let run_result = service
                .run_identity_ocr(app_handle.clone(), job_id.clone(), project_id)
                .await;
            if let Err(error) = run_result {
                let _ = service.job_manager.fail(&app_handle, &job_id, error);
            }
        });
        Ok(StartStudentIdentityOcrOutput {
            job_id: job.id,
            status: "queued".to_string(),
        })
    }

    pub fn rebuild_issues(
        &self,
        project_id: &str,
    ) -> Result<RebuildStudentAnswerOcrIssuesOutput, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let mut updated_records = 0u32;
        let mut updated_issues = 0u32;

        for question in &project.questions {
            for record in project
                .student_answer_ocr_records
                .iter_mut()
                .filter(|record| record.question_id == question.id)
            {
                let before = (
                    record.critical_keyword_uncertain,
                    record.uncertain_spans.len(),
                    record.suggested_corrections.len(),
                    record.critical_term_warnings.len(),
                    record.ocr_semantic_warnings.len(),
                );
                apply_deterministic_critical_term_analysis(record, question);
                let after = (
                    record.critical_keyword_uncertain,
                    record.uncertain_spans.len(),
                    record.suggested_corrections.len(),
                    record.critical_term_warnings.len(),
                    record.ocr_semantic_warnings.len(),
                );
                if before != after {
                    updated_records += 1;
                    if record.critical_keyword_uncertain
                        && (record.uncertain_spans.len() > before.1
                            || record.suggested_corrections.len() > before.2
                            || record.critical_term_warnings.len() > before.3
                            || record.ocr_semantic_warnings.len() > before.4)
                    {
                        updated_issues += 1;
                    }
                }
            }
        }

        if updated_records > 0 {
            project.workflow = workflow_engine::evaluate_workflow(&project);
            self.project_store
                .commit_snapshot_cas(&project)
                .map(|_| ())?;
        }

        Ok(RebuildStudentAnswerOcrIssuesOutput {
            project_id: project_id.to_string(),
            updated_records,
            updated_issues,
        })
    }

    pub fn update_student_answer_text(
        &self,
        project_id: &str,
        submission_id: &str,
        question_id: &str,
        text: String,
    ) -> Result<StudentAnswerOcrRecord, AppError> {
        if text.trim().is_empty() {
            return Err(AppError {
                code: AppErrorCode::OcrFailed,
                message: "OCR metni boş olamaz.".to_string(),
                recoverable: true,
                suggested_action: Some("Boş olmayan bir metin girin.".to_string()),
                technical_details: Some(format!(
                    "submission_id={submission_id}; question_id={question_id}"
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let now = chrono::Utc::now();
        let updated = {
            let record = find_record_mut(&mut project, submission_id, question_id)?;
            record.teacher_corrected_text = Some(text);
            record.status = StudentAnswerOcrStatus::TeacherCorrected;
            record.needs_review = true;
            record.updated_at = now;
            record.teacher_reviewed_at = None;
            record.clone()
        };
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn mark_student_answer_reviewed(
        &self,
        project_id: &str,
        submission_id: &str,
        question_id: &str,
    ) -> Result<StudentAnswerOcrRecord, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let now = chrono::Utc::now();
        let updated = {
            let record = find_record_mut(&mut project, submission_id, question_id)?;
            record.status = StudentAnswerOcrStatus::TeacherApproved;
            record.needs_review = false;
            record.teacher_reviewed_at = Some(now);
            record.updated_at = now;
            record.clone()
        };
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn mark_all_student_answers_reviewed(&self, project_id: &str) -> Result<Project, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let now = chrono::Utc::now();
        if project.student_answer_ocr_records.is_empty() {
            return Err(AppError {
                code: AppErrorCode::OcrNotReady,
                message: "Öğrenci cevap OCR kaydı bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce OCR işini çalıştırın.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        for record in &mut project.student_answer_ocr_records {
            record.status = StudentAnswerOcrStatus::TeacherApproved;
            record.needs_review = false;
            record.teacher_reviewed_at = Some(now);
            record.updated_at = now;
        }
        for submission in &mut project.student_submissions {
            submission.status = StudentSubmissionStatus::OcrConfirmed;
            submission.updated_at = Some(now.to_rfc3339());
        }
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(project)
    }

    // Mirrors the typed command payload at this boundary; grouping it is a separate API migration.
    #[allow(clippy::too_many_arguments)]
    pub async fn suggest_ocr_issue_correction_with_model(
        &self,
        project_path: String,
        ocr_record_id: String,
        issue_id: Option<String>,
        observed_text: String,
        suggested_text_from_analyzer: String,
        question_number: u32,
        highlight_region: Option<StudentAnswerOcrCropBBox>,
        crop_ref: Option<String>,
        model_input_crop_ref: Option<String>,
    ) -> Result<SuggestStudentAnswerOcrIssueCorrectionWithModelOutput, AppError> {
        let project = self.project_store.open_project(project_path.clone())?;
        let record = project
            .student_answer_ocr_records
            .iter()
            .find(|record| record.id == ocr_record_id)
            .cloned()
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentSubmissionNotFound,
                message: "OCR kaydı bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("OCR kayıtlarını yenileyin.".to_string()),
                technical_details: Some(format!("ocr_record_id={ocr_record_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        if record.question_number != question_number {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "OCR kayıt numarası ile soru numarası uyuşmuyor.".to_string(),
                recoverable: true,
                suggested_action: Some("İşaretli OCR kaydını yeniden seçin.".to_string()),
                technical_details: Some(format!(
                    "record_question_number={}; input_question_number={question_number}",
                    record.question_number
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let question = project
            .questions
            .iter()
            .find(|question| question.id == record.question_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::QuestionTextMissing,
                message: "Soru bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Soru metni kaydını kontrol edin.".to_string()),
                technical_details: Some(format!("question_id={}", record.question_id)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;

        let effective_answer_text = record
            .teacher_corrected_text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or(record.answer_text.trim());
        let nearby_context = extract_issue_context(effective_answer_text, &observed_text);
        let base_image_ref = select_issue_base_image_ref(
            &record,
            crop_ref.as_deref(),
            model_input_crop_ref.as_deref(),
        )
        .ok_or_else(|| AppError {
            code: AppErrorCode::PdfRenderFailed,
            message: "OCR issue için kullanılabilir görsel bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("OCR crop cache'ini kontrol edin.".to_string()),
            technical_details: Some(format!("ocr_record_id={ocr_record_id}")),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let issue_slug = issue_id.as_deref().unwrap_or("ocr_issue");
        let trusted_root = self.project_store.trusted_project_root(&project.id)?;
        let base_image_managed = trusted_root.adapt_legacy_document_path(&base_image_ref)?;
        let base_image_path = trusted_root.resolve_existing_file(&base_image_managed)?;
        let issue_relative = trusted_root.managed(&format!(
            "crops/student_answer_ocr_issue_correction/{ocr_record_id}/{issue_slug}"
        ))?;
        let issue_output_dir = trusted_root.root().join(issue_relative.as_path());
        trusted_root.ensure_managed_directory(&issue_output_dir)?;
        let issue_image = self.crop_service.crop_issue_region(
            &issue_output_dir,
            "issue_focus.png",
            &base_image_path,
            highlight_region.as_ref(),
        )?;
        let source_image_ref = Some(base_image_path.to_string_lossy().to_string());
        let model_input_sources = vec![(question.number, issue_image.model_input_image.clone())];
        let prepared_inputs = self.model_input_image_service.prepare_inputs(
            trusted_root.root(),
            ModelInputImageKind::StudentAnswerOcrIssueCorrection,
            &format!("{ocr_record_id}_{issue_slug}"),
            &model_input_sources,
        )?;

        let runtime_request = ModelRuntimeRequest {
            use_case: ModelUseCase::StudentAnswerOcrIssueCorrection,
            capability: ModelCapability::Vision,
            requires_mmproj: true,
            timeout_seconds: 180,
        };
        let _runtime_lease = self
            .model_runtime_service
            .acquire_runtime(
                None,
                &runtime_request,
                "student_answer_ocr_issue_correction",
                None,
            )
            .await?;

        let critical_term_hint = extract_question_critical_term_hint(question);
        let prompt = build_student_answer_issue_correction_prompt(
            question.number,
            &observed_text,
            &suggested_text_from_analyzer,
            &nearby_context,
            critical_term_hint.as_deref(),
            issue_id.as_deref(),
            highlight_region.as_ref(),
        );
        let result = self
            .model_gateway
            .suggest_student_answer_issue_correction(StudentAnswerOcrIssueCorrectionRequest {
                prompt,
                project_root_path: Some(project.root_path.clone()),
                job_id: None,
                ocr_record_id: ocr_record_id.clone(),
                issue_id: issue_id.clone(),
                observed_text: observed_text.clone(),
                suggested_text_from_analyzer: suggested_text_from_analyzer.clone(),
                question_number: question.number,
                highlight_region,
                model_input_crop_ref: Some(
                    issue_image.model_input_image.to_string_lossy().to_string(),
                ),
                model_input_images: prepared_inputs.clone(),
                source_image_ref,
                nearby_context: nearby_context.clone(),
                context_hints: vec![
                    format!("question_number={}", question.number),
                    format!("scope={}", issue_scope_hint(&observed_text)),
                ],
            })
            .await?;

        Ok(SuggestStudentAnswerOcrIssueCorrectionWithModelOutput {
            suggestion: result.output,
            raw_model_output: result.raw_response,
            used_image_ref: Some(issue_image.model_input_image.to_string_lossy().to_string()),
            prompt_version: ISSUE_CORRECTION_PROMPT_VERSION.to_string(),
            model_request_metadata: result.model_request_metadata,
        })
    }

    async fn preflight(&self, project_id: &str, force_rerun: bool) -> Result<Project, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        if project.questions.is_empty() {
            return Err(AppError {
                code: AppErrorCode::QuestionCountMissing,
                message: "Soru sayısı bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Önce soru metni ve rubrik hazırlığını tamamlayın.".to_string(),
                ),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if project.student_submissions.is_empty() || project.student_grouping_complete_at.is_none()
        {
            return Err(AppError {
                code: AppErrorCode::StudentGroupingNotReady,
                message: "Öğrenci gruplaması tamamlanmadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci gruplarını onaylayın.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if !project
            .questions
            .iter()
            .all(|question| is_question_text_ready(&question.question_text))
        {
            return Err(AppError {
                code: AppErrorCode::QuestionTextMissing,
                message: "Soru metni eksik.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce soru metinlerini onaylayın.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let total_expected = project.student_submissions.len() * project.questions.len();
        if total_expected == 0 {
            return Err(AppError {
                code: AppErrorCode::OcrNotReady,
                message: "OCR için işlenecek kayıt bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci grup ve soru sayısını kontrol edin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let document = self
            .active_student_scan_document(&project)
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentScanNotFound,
                message: "Öğrenci cevap PDF’i bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci cevap PDF’ini yükleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let preview_ready = matches!(
            document.preview.as_ref().map(|preview| &preview.status),
            Some(PdfPreviewStatus::Ready)
        );
        if !preview_ready {
            return Err(AppError {
                code: AppErrorCode::StudentScanPreviewNotReady,
                message: "Öğrenci cevap PDF önizlemeleri hazır değil.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce öğrenci PDF önizlemelerini oluşturun.".to_string()),
                technical_details: Some(format!("document_id={}", document.id)),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        if !force_rerun && !project.student_answer_ocr_records.is_empty() {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Öğrenci cevap OCR’ı zaten başlatılmış.".to_string(),
                recoverable: true,
                suggested_action: Some("Mevcut OCR sonuçlarını kontrol edin.".to_string()),
                technical_details: Some(format!(
                    "existing_records={}",
                    project.student_answer_ocr_records.len()
                )),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        Ok(project)
    }

    async fn run<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        job_id: String,
        project_id: String,
        generation_ids: Vec<(String, String)>,
        source_fingerprint: String,
    ) -> Result<(), AppError> {
        self.job_manager.set_running(&app, &job_id).ok();
        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let document = self
            .active_student_scan_document(&project)
            .cloned()
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentScanNotFound,
                message: "Öğrenci cevap PDF’i bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci cevap PDF’ini yükleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let trusted_root = self.project_store.trusted_project_root(&project.id)?;
        let source_path = document.resolve_path_with_root(&trusted_root)?;
        if file_fingerprint(&source_path)? != source_fingerprint {
            return Err(ocr_stale_error());
        }
        let total = (project.student_submissions.len() * project.questions.len()) as u32;
        self.job_manager
            .update_progress(
                &app,
                &job_id,
                0,
                total,
                "Model sunucusu kontrol ediliyor...".to_string(),
            )
            .ok();
        let runtime_request = ModelRuntimeRequest {
            use_case: ModelUseCase::StudentAnswerOcr,
            capability: ModelCapability::Vision,
            requires_mmproj: true,
            timeout_seconds: 180,
        };
        let runtime_status = self
            .model_runtime_service
            .get_runtime_status(None, &runtime_request)
            .await?;
        if !runtime_status.health_ok {
            self.job_manager
                .update_progress(
                    &app,
                    &job_id,
                    0,
                    total,
                    "Model sunucusu başlatılıyor...".to_string(),
                )
                .ok();
        }
        let _runtime_lease = self
            .model_runtime_service
            .acquire_runtime(None, &runtime_request, "student_answer_ocr", Some(&job_id))
            .await?;
        self.job_manager
            .update_progress(&app, &job_id, 0, total, "Model yükleniyor...".to_string())
            .ok();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        self.job_manager
            .update_progress(
                &app,
                &job_id,
                0,
                total,
                "Model hazır, OCR başlatılıyor...".to_string(),
            )
            .ok();
        let mut current = 0u32;
        let mut succeeded = 0u32;
        let mut failed = 0u32;
        let mut needs_review = 0u32;
        let mut records = Vec::new();

        let cancel_token = self.job_manager.get_cancellation_token(&job_id);
        for submission in project.student_submissions.clone() {
            for question in &project.questions {
                if let Some(ref t) = cancel_token {
                    if t.is_cancelled() {
                        let _ = self.job_manager.mark_cancelled(&app, &job_id);
                        return Ok(());
                    }
                }
                current += 1;
                self.job_manager
                    .update_progress(
                        &app,
                        &job_id,
                        current,
                        total,
                        format!(
                            "Öğrenci {} / Soru {} okunuyor...",
                            submission.id, question.number
                        ),
                    )
                    .ok();

                let source_artifacts = match self.crop_service.prepare_source_artifacts(
                    &project_id,
                    &project.root_path,
                    &document.id,
                    &submission,
                    question,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        failed += 1;
                        records.push(self.failed_record(
                            &submission,
                            question,
                            StudentAnswerOcrStatus::CropMissing,
                            error.message.clone(),
                            submission.page_numbers.clone(),
                            vec![],
                        ));
                        continue;
                    }
                };
                let batch_id = format!("{}_q{}", submission.id, question.number);
                let preprocessed_inputs = self.preprocess_model_inputs(
                    trusted_root.root(),
                    &source_artifacts.model_input_images,
                    OcrImagePreprocessMode::HandwritingEnhanced,
                )?;
                let prepared_inputs = self.model_input_image_service.prepare_inputs(
                    trusted_root.root(),
                    ModelInputImageKind::StudentOcr,
                    &batch_id,
                    &preprocessed_inputs.model_input_images,
                )?;

                let prompt = build_student_answer_ocr_prompt(
                    question.number,
                    &question.question_text.value,
                    &question.answer_type,
                    &source_artifacts.layout_hint,
                );
                let result = self
                    .model_gateway
                    .extract_student_answer_ocr(StudentAnswerOcrRequest {
                        prompt,
                        project_root_path: Some(project.root_path.clone()),
                        job_id: Some(job_id.clone()),
                        submission_id: submission.id.clone(),
                        question_id: question.id.clone(),
                        question_number: question.number,
                        question_text: question.question_text.value.clone(),
                        answer_type: answer_type_label(&question.answer_type).to_string(),
                        preprocess_mode: Some(preprocessed_inputs.preprocess_mode),
                        preprocess_version: Some(preprocessed_inputs.preprocess_version.clone()),
                        model_input_crop_ref: preprocessed_inputs.model_input_crop_ref.clone(),
                        source_page_numbers: source_artifacts.source_page_numbers.clone(),
                        model_input_images: prepared_inputs.clone(),
                    })
                    .await;

                match result {
                    Ok(result) => {
                        succeeded += 1;
                        if result.output.needs_review || !result.output.review_reasons.is_empty() {
                            needs_review += 1;
                        }
                        let fallback_used = source_artifacts
                            .render_diagnostics
                            .answer_region_source
                            .as_deref()
                            .is_some_and(|source| source.starts_with("full_page_fallback"));
                        let now = chrono::Utc::now();
                        let status = derive_student_answer_status(
                            result.parse_error.is_some(),
                            source_artifacts.render_diagnostics.crop_missing,
                            source_artifacts.render_diagnostics.partial_answer_suspected,
                            result.printed_question_leak_detected,
                            result.output.needs_review,
                        );
                        let mut record = StudentAnswerOcrRecord {
                            id: Uuid::new_v4().to_string(),
                            submission_id: submission.id.clone(),
                            question_id: question.id.clone(),
                            question_number: question.number,
                            source_page_numbers: source_artifacts.source_page_numbers.clone(),
                            source_image_refs: result
                                .diagnostics
                                .model_input_images
                                .iter()
                                .map(|image| image.output_image_path.clone())
                                .collect(),
                            crop_refs: preprocessed_inputs.original_crop_refs.clone(),
                            original_crop_refs: preprocessed_inputs.original_crop_refs.clone(),
                            preprocessed_crop_refs: preprocessed_inputs
                                .preprocessed_crop_refs
                                .clone(),
                            preprocess_mode: Some(preprocessed_inputs.preprocess_mode),
                            preprocess_version: Some(
                                preprocessed_inputs.preprocess_version.clone(),
                            ),
                            model_input_crop_ref: preprocessed_inputs.model_input_crop_ref.clone(),
                            preprocess_applied: preprocessed_inputs.preprocess_applied,
                            preprocess_warnings: preprocessed_inputs.preprocess_warnings.clone(),
                            preprocess_diagnostics: preprocessed_inputs
                                .preprocess_diagnostics
                                .clone(),
                            available_preprocess_variants: preprocessed_inputs
                                .available_preprocess_variants
                                .clone(),
                            full_page_preview_refs: source_artifacts
                                .render_diagnostics
                                .full_page_preview_refs
                                .clone(),
                            answer_text: result.output.answer_text,
                            structured_answer: result.output.structured_answer,
                            confidence: Some(result.output.confidence),
                            uncertain_spans: result.output.uncertain_spans,
                            suggested_corrections: result.output.suggested_corrections,
                            critical_term_warnings: result.output.critical_term_warnings,
                            ocr_semantic_warnings: result.output.ocr_semantic_warnings,
                            critical_keyword_uncertain: result.output.critical_keyword_uncertain,
                            status,
                            needs_review: result.output.needs_review
                                || result.output.critical_keyword_uncertain
                                || result.parse_error.is_some()
                                || source_artifacts.render_diagnostics.crop_missing
                                || source_artifacts.render_diagnostics.partial_answer_suspected
                                || result.printed_question_leak_detected
                                || fallback_used,
                            review_reasons: merge_review_reasons(
                                {
                                    let mut reasons = result.output.review_reasons;
                                    if fallback_used {
                                        reasons
                                            .push("full_page_fallback_review_required".to_string());
                                    }
                                    reasons
                                },
                                result.parse_error.as_ref(),
                                &source_artifacts.render_diagnostics,
                                result.printed_question_leak_detected,
                            ),
                            warnings: merge_warnings(
                                {
                                    let mut warnings = result.output.warnings;
                                    if fallback_used {
                                        warnings
                                            .push("full_page_fallback_review_required".to_string());
                                    }
                                    warnings
                                },
                                result.parse_error.as_ref(),
                                &source_artifacts.render_diagnostics,
                                result.printed_text_mixed,
                            ),
                            model_name: Some("gemma".to_string()),
                            prompt_version: PROMPT_VERSION.to_string(),
                            created_at: now,
                            updated_at: now,
                            teacher_corrected_text: None,
                            teacher_reviewed_at: None,
                            parse_diagnostics: Some(StudentAnswerOcrParseDiagnostics {
                                raw_model_output: result.raw_response.clone(),
                                parse_error: result.parse_error.clone(),
                                parsed_json: result.parsed_json.clone(),
                                salvaged_answer_text: result.salvaged_answer_text.clone(),
                                parse_strategy: result.parse_strategy.clone(),
                                model_request_metadata: result.model_request_metadata.clone(),
                            }),
                            render_diagnostics: Some(source_artifacts.render_diagnostics.clone()),
                        };
                        apply_deterministic_critical_term_analysis(&mut record, question);
                        records.push(record);
                    }
                    Err(error) if self.is_soft_model_error(&error) => {
                        failed += 1;
                        records.push(self.failed_record(
                            &submission,
                            question,
                            StudentAnswerOcrStatus::ModelError,
                            error.message.clone(),
                            source_artifacts.source_page_numbers.clone(),
                            vec![error.message.clone()],
                        ));
                        if let Some(record) = records.last_mut() {
                            record.crop_refs =
                                source_artifacts.render_diagnostics.crop_refs.clone();
                            record.full_page_preview_refs = source_artifacts
                                .render_diagnostics
                                .full_page_preview_refs
                                .clone();
                            record.render_diagnostics =
                                Some(source_artifacts.render_diagnostics.clone());
                        }
                    }
                    Err(error) => {
                        let _ = self.job_manager.fail(&app, &job_id, error.clone());
                        return Err(error);
                    }
                }
            }
        }

        let records_by_submission = records.iter().fold(
            std::collections::HashMap::<String, Vec<StudentAnswerOcrRecord>>::new(),
            |mut grouped, record| {
                grouped
                    .entry(record.submission_id.clone())
                    .or_default()
                    .push(record.clone());
                grouped
            },
        );
        let candidate_result = serde_json::json!({
            "total": total,
            "succeeded": succeeded,
            "failed": failed,
            "needsReview": needs_review,
        });
        if let Some(ref t) = cancel_token {
            if t.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Ok(());
            }
        }

        let generation_ids_for_commit = generation_ids.clone();
        let commit = self.project_store.commit_job(
            &project_id,
            crate::services::project_store::MutationOptions::new("commit_ocr_generation"),
            move |current, _context| {
                let current_document = current
                    .documents
                    .iter()
                    .find(|entry| {
                        entry.id == document.id && entry.role == DocumentRole::StudentScan
                    })
                    .ok_or_else(|| {
                        ocr_error(
                            AppErrorCode::ProjectEntityNotFound,
                            "Öğrenci cevap belgesi artık mevcut değil.",
                        )
                    })?;
                let current_path = current_document.resolve_path_with_root(&trusted_root)?;
                if file_fingerprint(&current_path)? != source_fingerprint {
                    return Err(ocr_stale_error());
                }
                for (submission_id, generation_id) in &generation_ids_for_commit {
                    let submission = current
                        .student_submissions
                        .iter()
                        .find(|submission| submission.id == *submission_id)
                        .ok_or_else(|| {
                            ocr_entity_missing_error("Öğrenci kaydı artık mevcut değil.")
                        })?;
                    let candidate_records = records_by_submission
                        .get(submission_id)
                        .cloned()
                        .unwrap_or_default();
                    if candidate_records.len() != current.questions.len()
                        || candidate_records.iter().any(|record| {
                            record.answer_text.trim().is_empty() && !record.needs_review
                        })
                    {
                        return Err(ocr_candidate_failed_error(
                            "OCR sonucu beklenen soru kapsamını doğrulayamadı.",
                        ));
                    }
                    let generation = current
                        .student_answer_ocr_generations
                        .iter_mut()
                        .find(|generation| generation.generation_id == *generation_id)
                        .ok_or_else(|| {
                            ocr_entity_missing_error("OCR candidate artık mevcut değil.")
                        })?;
                    generation.result = candidate_records.clone();
                    generation.diagnostics = Some(candidate_result.clone());
                    let failed_candidate = generation.result.iter().any(|record| {
                        matches!(
                            record.status,
                            StudentAnswerOcrStatus::Failed
                                | StudentAnswerOcrStatus::CropMissing
                                | StudentAnswerOcrStatus::ModelError
                                | StudentAnswerOcrStatus::ParseFailed
                        )
                    });
                    let protected = current
                        .student_answer_ocr_records
                        .iter()
                        .filter(|record| record.submission_id == submission.id)
                        .any(|record| record.status == StudentAnswerOcrStatus::TeacherApproved)
                        || current
                            .scoring_records
                            .iter()
                            .any(|record| record.submission_id == submission.id);
                    if failed_candidate {
                        generation.status = OcrGenerationStatus::Failed;
                        generation.teacher_review_status = OcrTeacherReviewStatus::Pending;
                        generation.failure_reason =
                            Some("OCR sonucu ek kontrol gerektiriyor.".to_string());
                    } else if protected {
                        generation.status = OcrGenerationStatus::ReadyForReview;
                        generation.teacher_review_status = OcrTeacherReviewStatus::Pending;
                    } else {
                        generation.status = OcrGenerationStatus::Active;
                        generation.teacher_review_status = OcrTeacherReviewStatus::NotRequired;
                        current
                            .student_answer_ocr_records
                            .retain(|record| record.submission_id != submission.id);
                        current.student_answer_ocr_records.extend(candidate_records);
                    }
                }
                refresh_submission_ocr_statuses(current);
                Ok(())
            },
        );
        let committed_project = match commit {
            crate::services::project_store::JobCommitResult::Applied(output) => {
                output.snapshot.project.clone()
            }
            crate::services::project_store::JobCommitResult::Stale { .. }
            | crate::services::project_store::JobCommitResult::EntityMissing => {
                return Err(ocr_stale_error());
            }
            crate::services::project_store::JobCommitResult::Conflict(error)
            | crate::services::project_store::JobCommitResult::Rejected(error) => {
                return Err(error)
            }
        };

        let result = StudentAnswerOcrJobResult {
            total,
            succeeded,
            failed,
            reviewed: committed_project
                .student_answer_ocr_records
                .iter()
                .filter(|record| record.status == StudentAnswerOcrStatus::TeacherApproved)
                .count() as u32,
            needs_review,
            partial: failed > 0,
        };

        self.job_manager.update_progress(
            &app,
            &job_id,
            total,
            total,
            if failed > 0 {
                "Öğrenci cevap OCR’ı kısmi tamamlandı.".to_string()
            } else {
                "Öğrenci cevap OCR’ı tamamlandı.".to_string()
            },
        )?;
        if failed > 0 {
            self.job_manager.partial(
                &app,
                &job_id,
                Some(serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}))),
            )?;
        } else {
            self.job_manager.succeed(
                &app,
                &job_id,
                Some(serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}))),
            )?;
        }

        Ok(())
    }

    async fn run_identity_ocr<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        job_id: String,
        project_id: String,
    ) -> Result<(), AppError> {
        self.job_manager.set_running(&app, &job_id).ok();
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let document = self
            .active_student_scan_document(&project)
            .cloned()
            .ok_or_else(|| AppError {
                code: AppErrorCode::StudentScanNotFound,
                message: "Öğrenci cevap PDF’i bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Öğrenci cevap PDF’ini yükleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let trusted_root = self.project_store.trusted_project_root(&project.id)?;
        let total = project.student_submissions.len() as u32;
        self.job_manager
            .update_progress(
                &app,
                &job_id,
                0,
                total,
                "Model sunucusu kontrol ediliyor...".to_string(),
            )
            .ok();
        let runtime_request = ModelRuntimeRequest {
            use_case: ModelUseCase::StudentAnswerOcr,
            capability: ModelCapability::Vision,
            requires_mmproj: true,
            timeout_seconds: 180,
        };
        let _runtime_lease = self
            .model_runtime_service
            .acquire_runtime(
                None,
                &runtime_request,
                "student_identity_ocr",
                Some(&job_id),
            )
            .await?;

        let mut current = 0u32;
        let mut failed = 0u32;
        for submission in project.student_submissions.clone() {
            current += 1;
            self.job_manager
                .update_progress(
                    &app,
                    &job_id,
                    current,
                    total,
                    format!("Öğrenci {} kimliği okunuyor...", submission.id),
                )
                .ok();
            let artifacts = self.crop_service.prepare_identity_artifacts(
                &project_id,
                &project.root_path,
                &document.id,
                &submission,
            )?;
            let preprocessed_inputs = self.preprocess_model_inputs(
                trusted_root.root(),
                &artifacts.model_input_images,
                OcrImagePreprocessMode::HandwritingEnhanced,
            )?;
            let prepared_inputs = self.model_input_image_service.prepare_inputs(
                trusted_root.root(),
                ModelInputImageKind::StudentIdentityOcr,
                &submission.id,
                &preprocessed_inputs.model_input_images,
            )?;
            let result = self
                .model_gateway
                .extract_student_identity_ocr(StudentIdentityOcrRequest {
                    prompt: build_student_identity_ocr_prompt(),
                    project_root_path: Some(project.root_path.clone()),
                    job_id: Some(job_id.clone()),
                    submission_id: submission.id.clone(),
                    preprocess_mode: Some(preprocessed_inputs.preprocess_mode),
                    preprocess_version: Some(preprocessed_inputs.preprocess_version.clone()),
                    model_input_crop_ref: preprocessed_inputs.model_input_crop_ref.clone(),
                    source_page_numbers: artifacts.source_page_numbers.clone(),
                    model_input_images: prepared_inputs,
                })
                .await;
            let student = project
                .students
                .iter_mut()
                .find(|student| student.id == submission.student_id)
                .ok_or_else(|| AppError {
                    code: AppErrorCode::StudentSubmissionNotFound,
                    message: "Öğrenci kaydı bulunamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Öğrenci gruplamasını kontrol edin.".to_string()),
                    technical_details: Some(format!("student_id={}", submission.student_id)),
                    correlation_id: Uuid::new_v4().to_string(),
                })?;
            let now = chrono::Utc::now();
            match result {
                Ok(result) => {
                    student.identity_ocr = Some(StudentIdentityOcrRecord {
                        display_name: result.output.display_name,
                        number: result.output.number,
                        class_name: result.output.class_name,
                        confidence: result.output.confidence,
                        needs_review: true,
                        warnings: result.output.warnings,
                        raw_model_output: result.raw_response,
                        crop_refs: preprocessed_inputs.original_crop_refs.clone(),
                        original_crop_refs: preprocessed_inputs.original_crop_refs.clone(),
                        preprocessed_crop_refs: preprocessed_inputs.preprocessed_crop_refs.clone(),
                        preprocess_mode: Some(preprocessed_inputs.preprocess_mode),
                        preprocess_version: Some(preprocessed_inputs.preprocess_version.clone()),
                        model_input_crop_ref: preprocessed_inputs.model_input_crop_ref.clone(),
                        preprocess_applied: preprocessed_inputs.preprocess_applied,
                        preprocess_warnings: preprocessed_inputs.preprocess_warnings.clone(),
                        preprocess_diagnostics: preprocessed_inputs.preprocess_diagnostics.clone(),
                        available_preprocess_variants: preprocessed_inputs
                            .available_preprocess_variants
                            .clone(),
                        source_page_numbers: artifacts.source_page_numbers,
                        model_request_metadata: result.model_request_metadata,
                        created_at: now,
                        updated_at: now,
                    });
                }
                Err(error) => {
                    failed += 1;
                    let error_code = format!("{:?}", error.code);
                    student.identity_ocr = Some(StudentIdentityOcrRecord {
                        display_name: None,
                        number: None,
                        class_name: None,
                        confidence: 0.0,
                        needs_review: true,
                        warnings: vec![format!("{error_code}: {}", error.message)],
                        raw_model_output: String::new(),
                        crop_refs: preprocessed_inputs.original_crop_refs.clone(),
                        original_crop_refs: preprocessed_inputs.original_crop_refs.clone(),
                        preprocessed_crop_refs: preprocessed_inputs.preprocessed_crop_refs.clone(),
                        preprocess_mode: Some(preprocessed_inputs.preprocess_mode),
                        preprocess_version: Some(preprocessed_inputs.preprocess_version.clone()),
                        model_input_crop_ref: preprocessed_inputs.model_input_crop_ref.clone(),
                        preprocess_applied: preprocessed_inputs.preprocess_applied,
                        preprocess_warnings: preprocessed_inputs.preprocess_warnings.clone(),
                        preprocess_diagnostics: preprocessed_inputs.preprocess_diagnostics.clone(),
                        available_preprocess_variants: preprocessed_inputs
                            .available_preprocess_variants
                            .clone(),
                        source_page_numbers: artifacts.source_page_numbers,
                        model_request_metadata: Some(serde_json::json!({
                            "requestKind": "student_identity_ocr",
                            "errorCode": error_code,
                            "message": error.message,
                            "technicalDetails": error.technical_details,
                        })),
                        created_at: now,
                        updated_at: now,
                    });
                }
            }
        }
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        self.job_manager.update_progress(
            &app,
            &job_id,
            total,
            total,
            "Kimlik OCR’ı tamamlandı.".to_string(),
        )?;
        if failed > 0 {
            self.job_manager.partial(
                &app,
                &job_id,
                Some(serde_json::json!({
                    "total": total,
                    "failed": failed,
                    "needsReview": failed,
                })),
            )?;
        } else {
            self.job_manager.succeed(&app, &job_id, None)?;
        }
        Ok(())
    }

    fn failed_record(
        &self,
        submission: &StudentSubmission,
        question: &crate::domain::question::Question,
        status: StudentAnswerOcrStatus,
        error_message: String,
        source_page_numbers: Vec<u32>,
        mut warnings: Vec<String>,
    ) -> StudentAnswerOcrRecord {
        warnings.push(error_message.clone());
        let now = chrono::Utc::now();
        StudentAnswerOcrRecord {
            id: Uuid::new_v4().to_string(),
            submission_id: submission.id.clone(),
            question_id: question.id.clone(),
            question_number: question.number,
            source_page_numbers,
            source_image_refs: vec![],
            crop_refs: vec![],
            original_crop_refs: vec![],
            preprocessed_crop_refs: vec![],
            model_input_crop_ref: None,
            preprocess_mode: Some(OcrImagePreprocessMode::Original),
            preprocess_version: Some(PREPROCESS_VERSION.to_string()),
            preprocess_applied: false,
            preprocess_warnings: vec![error_message.clone()],
            preprocess_diagnostics: vec![],
            available_preprocess_variants: PREPROCESS_VARIANTS.to_vec(),
            full_page_preview_refs: vec![],
            answer_text: String::new(),
            structured_answer: None,
            confidence: None,
            uncertain_spans: vec![],
            suggested_corrections: vec![],
            critical_term_warnings: vec![],
            ocr_semantic_warnings: vec![],
            critical_keyword_uncertain: false,
            status,
            needs_review: true,
            review_reasons: vec![error_message],
            warnings,
            model_name: None,
            prompt_version: PROMPT_VERSION.to_string(),
            created_at: now,
            updated_at: now,
            teacher_corrected_text: None,
            teacher_reviewed_at: None,
            parse_diagnostics: None,
            render_diagnostics: None,
        }
    }

    fn is_soft_model_error(&self, error: &AppError) -> bool {
        matches!(
            error.code,
            AppErrorCode::ModelResponseEmpty
                | AppErrorCode::ModelResponseInvalidJson
                | AppErrorCode::ModelResponseInvalidSchema
                | AppErrorCode::ModelResponseReasoningOnly
        )
    }

    fn active_student_scan_document<'a>(
        &self,
        project: &'a Project,
    ) -> Option<&'a crate::domain::document::Document> {
        if let Some(document_id) = project.student_scan_document_id.as_deref() {
            project.documents.iter().find(|document| {
                document.id == document_id && document.role == DocumentRole::StudentScan
            })
        } else {
            project
                .documents
                .iter()
                .find(|document| document.role == DocumentRole::StudentScan)
        }
    }

    pub fn accept_student_answer_ocr_generation(
        &self,
        project_id: &str,
        generation_id: &str,
    ) -> Result<OcrGeneration, AppError> {
        let generation_id = generation_id.to_string();
        let output = self.project_store.mutate(
            project_id,
            crate::services::project_store::MutationOptions::new("accept_ocr_generation"),
            move |project, _context| {
                let index = project
                    .student_answer_ocr_generations
                    .iter()
                    .position(|generation| generation.generation_id == generation_id)
                    .ok_or_else(|| ocr_entity_missing_error("OCR önerisi bulunamadı."))?;
                let candidate = project.student_answer_ocr_generations[index].clone();
                if candidate.status != OcrGenerationStatus::ReadyForReview
                    || candidate.result.is_empty()
                {
                    return Err(ocr_error(
                        AppErrorCode::OcrGenerationConflict,
                        "OCR önerisi öğretmen karşılaştırmasına hazır değil.",
                    ));
                }
                for generation in &mut project.student_answer_ocr_generations {
                    if generation.submission_id == candidate.submission_id
                        && generation.status == OcrGenerationStatus::Active
                    {
                        generation.status = OcrGenerationStatus::Superseded;
                    }
                }
                project.student_answer_ocr_generations[index].status = OcrGenerationStatus::Active;
                project.student_answer_ocr_generations[index].teacher_review_status =
                    OcrTeacherReviewStatus::Approved;
                project
                    .student_answer_ocr_records
                    .retain(|record| record.submission_id != candidate.submission_id);
                project
                    .student_answer_ocr_records
                    .extend(candidate.result.clone());
                let now = chrono::Utc::now();
                for record in &mut project.scoring_records {
                    if record.submission_id == candidate.submission_id {
                        record.teacher_review_status = ScoringReviewStatus::Invalidated;
                        record.invalidated_at = Some(now);
                        record.invalidation_reason = Some(
                            "Yeni OCR generation öğretmen tarafından kabul edildi.".to_string(),
                        );
                    }
                }
                refresh_submission_ocr_statuses(project);
                Ok(project.student_answer_ocr_generations[index].clone())
            },
        )?;
        Ok(output.result)
    }

    pub fn reject_student_answer_ocr_generation(
        &self,
        project_id: &str,
        generation_id: &str,
    ) -> Result<OcrGeneration, AppError> {
        let generation_id = generation_id.to_string();
        let output = self.project_store.mutate(
            project_id,
            crate::services::project_store::MutationOptions::new("reject_ocr_generation"),
            move |project, _context| {
                let generation = project
                    .student_answer_ocr_generations
                    .iter_mut()
                    .find(|generation| generation.generation_id == generation_id)
                    .ok_or_else(|| ocr_entity_missing_error("OCR önerisi bulunamadı."))?;
                if generation.status != OcrGenerationStatus::ReadyForReview {
                    return Err(ocr_error(
                        AppErrorCode::OcrGenerationConflict,
                        "OCR önerisi reddedilebilir durumda değil.",
                    ));
                }
                generation.status = OcrGenerationStatus::Rejected;
                generation.teacher_review_status = OcrTeacherReviewStatus::Rejected;
                Ok(generation.clone())
            },
        )?;
        Ok(output.result)
    }

    fn mark_generation_status(
        &self,
        project_id: &str,
        generation_ids: &[(String, String)],
        status: OcrGenerationStatus,
        reason: Option<String>,
    ) -> Result<(), AppError> {
        self.project_store
            .mutate(
                project_id,
                crate::services::project_store::MutationOptions::new("mark_ocr_generation_status"),
                |project, _context| {
                    for (_, generation_id) in generation_ids {
                        if let Some(generation) = project
                            .student_answer_ocr_generations
                            .iter_mut()
                            .find(|generation| generation.generation_id == *generation_id)
                        {
                            if generation.status != OcrGenerationStatus::Active {
                                generation.status = status.clone();
                                generation.failure_reason = reason.clone();
                            }
                        }
                    }
                    refresh_submission_ocr_statuses(project);
                    Ok(())
                },
            )
            .map(|_| ())
    }
}

fn refresh_submission_ocr_statuses(project: &mut Project) {
    let now = chrono::Utc::now().to_rfc3339();
    for submission in &mut project.student_submissions {
        let related = project
            .student_answer_ocr_records
            .iter()
            .filter(|record| record.submission_id == submission.id)
            .collect::<Vec<_>>();
        if related.is_empty() {
            if submission.status == StudentSubmissionStatus::OcrRunning {
                submission.status = StudentSubmissionStatus::Failed;
                submission.updated_at = Some(now.clone());
            }
            continue;
        }
        submission.status = if related
            .iter()
            .all(|record| record.status == StudentAnswerOcrStatus::TeacherApproved)
        {
            StudentSubmissionStatus::OcrConfirmed
        } else {
            StudentSubmissionStatus::OcrSuggested
        };
        submission.updated_at = Some(now.clone());
    }
}

fn ocr_error(code: AppErrorCode, message: &str) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some("Mevcut OCR sonucu korunarak işlemi yeniden deneyin.".to_string()),
        technical_details: None,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

fn ocr_candidate_failed_error(message: &str) -> AppError {
    ocr_error(AppErrorCode::OcrRerunCandidateFailed, message)
}

fn ocr_entity_missing_error(message: &str) -> AppError {
    ocr_error(AppErrorCode::ProjectEntityNotFound, message)
}

fn ocr_stale_error() -> AppError {
    ocr_error(
        AppErrorCode::OcrGenerationStale,
        "Kaynak belge değiştiği için yeni OCR sonucu etkinleştirilmedi; mevcut sonuç korundu.",
    )
}

fn file_fingerprint(path: &Path) -> Result<String, AppError> {
    let bytes = std::fs::read(path).map_err(|error| AppError {
        code: AppErrorCode::FileReadFailed,
        message: "OCR kaynak belgesi okunamadı.".to_string(),
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

fn find_record_mut<'a>(
    project: &'a mut Project,
    submission_id: &str,
    question_id: &str,
) -> Result<&'a mut StudentAnswerOcrRecord, AppError> {
    project
        .student_answer_ocr_records
        .iter_mut()
        .find(|record| record.submission_id == submission_id && record.question_id == question_id)
        .ok_or_else(|| AppError {
            code: AppErrorCode::StudentSubmissionNotFound,
            message: "Öğrenci OCR kaydı bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("OCR sonuçlarını yenileyin.".to_string()),
            technical_details: Some(format!(
                "submission_id={submission_id}; question_id={question_id}"
            )),
            correlation_id: Uuid::new_v4().to_string(),
        })
}

fn answer_type_label(answer_type: &AnswerType) -> &'static str {
    match answer_type {
        AnswerType::GeneralText => "general_text",
        AnswerType::ShortText => "short_text",
        AnswerType::Essay => "essay",
        AnswerType::Table => "table",
        AnswerType::CorrectionTable => "correction_table",
        AnswerType::FillBlank => "fill_blank",
        AnswerType::Matching => "matching",
        AnswerType::MultipleChoice => "multiple_choice",
        AnswerType::TrueFalse => "true_false",
        AnswerType::Ordering => "ordering",
        AnswerType::Numeric => "numeric",
        AnswerType::DiagramLabeling => "diagram_labeling",
        AnswerType::SentenceAnnotation => "sentence_annotation",
        AnswerType::GrammarAnalysis => "grammar_analysis",
    }
}

fn derive_student_answer_status(
    parse_failed: bool,
    crop_missing: bool,
    partial_answer_suspected: bool,
    printed_question_leak_detected: bool,
    needs_review: bool,
) -> StudentAnswerOcrStatus {
    if parse_failed {
        StudentAnswerOcrStatus::ParseFailed
    } else if crop_missing {
        StudentAnswerOcrStatus::CropMissing
    } else if printed_question_leak_detected {
        StudentAnswerOcrStatus::PrintedTextLeakSuspected
    } else if partial_answer_suspected {
        StudentAnswerOcrStatus::PartialAnswerSuspected
    } else if needs_review {
        StudentAnswerOcrStatus::ReviewNeeded
    } else {
        StudentAnswerOcrStatus::Succeeded
    }
}

fn merge_review_reasons(
    mut reasons: Vec<String>,
    parse_error: Option<&String>,
    render_diagnostics: &StudentAnswerOcrRenderDiagnostics,
    printed_question_leak_detected: bool,
) -> Vec<String> {
    if let Some(parse_error) = parse_error {
        reasons.push("parse_failed".to_string());
        reasons.push(parse_error.clone());
    }
    if render_diagnostics.crop_missing {
        reasons.push("crop_missing".to_string());
    }
    if render_diagnostics.partial_answer_suspected {
        reasons.push("answer_crop_may_be_incomplete".to_string());
    }
    if printed_question_leak_detected {
        reasons.push("printed_question_leak_detected".to_string());
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn merge_warnings(
    mut warnings: Vec<String>,
    parse_error: Option<&String>,
    render_diagnostics: &StudentAnswerOcrRenderDiagnostics,
    printed_text_mixed: bool,
) -> Vec<String> {
    if let Some(parse_error) = parse_error {
        warnings.push(format!("parse_error:{parse_error}"));
    }
    if render_diagnostics.crop_missing {
        warnings.push("crop_missing".to_string());
    }
    if render_diagnostics.page_preview_missing {
        warnings.push("page_preview_missing".to_string());
    }
    if render_diagnostics.partial_answer_suspected {
        warnings.push("answer_crop_may_be_incomplete".to_string());
    }
    if printed_text_mixed {
        warnings.push("printed_text_mixed".to_string());
    }
    warnings.sort();
    warnings.dedup();
    warnings
}

fn apply_deterministic_critical_term_analysis(
    record: &mut StudentAnswerOcrRecord,
    question: &crate::domain::question::Question,
) {
    let analysis = analyze_critical_term_uncertainty(record, question);
    if !analysis.critical_keyword_uncertain {
        return;
    }

    record.critical_keyword_uncertain = true;
    record.needs_review = true;
    record.ocr_semantic_warnings = merge_string_lists(
        record.ocr_semantic_warnings.clone(),
        analysis.ocr_semantic_warnings,
    );
    record.ocr_semantic_warnings.sort();
    record.ocr_semantic_warnings.dedup();
    record.critical_term_warnings = merge_struct_lists(
        record.critical_term_warnings.clone(),
        analysis.critical_term_warnings,
    );
    record.suggested_corrections = merge_struct_lists(
        record.suggested_corrections.clone(),
        analysis.suggested_corrections,
    );
    record.uncertain_spans =
        merge_struct_lists(record.uncertain_spans.clone(), analysis.uncertain_spans);
    record.review_reasons =
        merge_string_lists(record.review_reasons.clone(), analysis.review_reasons);
    record.warnings = merge_string_lists(record.warnings.clone(), analysis.warnings);
}

fn derive_semantic_issue_from_answer(
    record: &StudentAnswerOcrRecord,
    question: &crate::domain::question::Question,
) -> Option<(String, String)> {
    let expected_answer = question.rubric.expected_answer.as_deref()?.trim();
    if expected_answer.is_empty() {
        return None;
    }

    let answer_tokens = tokenize_critical_term_words(&record.answer_text);
    let expected_tokens = tokenize_critical_term_words(expected_answer);
    if answer_tokens.is_empty() || expected_tokens.is_empty() {
        return None;
    }

    let normalized_answer: Vec<String> = answer_tokens
        .iter()
        .map(|token| normalize_for_critical_term_analysis(token))
        .collect();
    let normalized_expected: Vec<String> = expected_tokens
        .iter()
        .map(|token| normalize_for_critical_term_analysis(token))
        .collect();

    for suffix_len in (2..=normalized_answer.len().min(normalized_expected.len())).rev() {
        let expected_suffix = &normalized_expected[normalized_expected.len() - suffix_len..];
        for start in 0..=normalized_answer.len().saturating_sub(suffix_len) {
            if normalized_answer[start..start + suffix_len] != *expected_suffix {
                continue;
            }
            if start == 0 {
                continue;
            }
            let observed = answer_tokens.get(start - 1)?.trim().to_string();
            let suggested = expected_tokens
                .get(expected_tokens.len().checked_sub(suffix_len + 1)?)?
                .trim()
                .to_string();
            if observed.is_empty() || suggested.is_empty() || observed == suggested {
                continue;
            }
            return Some((observed, suggested));
        }
    }

    None
}

fn tokenize_critical_term_words(text: &str) -> Vec<String> {
    normalize_for_critical_term_analysis(text)
        .split_whitespace()
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

struct CriticalTermAnalysis {
    critical_keyword_uncertain: bool,
    uncertain_spans: Vec<OcrUncertainSpan>,
    suggested_corrections: Vec<OcrSuggestedCorrection>,
    critical_term_warnings: Vec<OcrCriticalTermWarning>,
    ocr_semantic_warnings: Vec<String>,
    review_reasons: Vec<String>,
    warnings: Vec<String>,
}

fn analyze_critical_term_uncertainty(
    record: &StudentAnswerOcrRecord,
    question: &crate::domain::question::Question,
) -> CriticalTermAnalysis {
    let answer_text = record.answer_text.trim();
    if answer_text.is_empty() {
        return CriticalTermAnalysis {
            critical_keyword_uncertain: false,
            uncertain_spans: vec![],
            suggested_corrections: vec![],
            critical_term_warnings: vec![],
            ocr_semantic_warnings: vec![],
            review_reasons: vec![],
            warnings: vec![],
        };
    }

    let candidates = collect_critical_term_candidates(question);
    let answer_norm = normalize_for_critical_term_analysis(answer_text);
    let mut uncertain_spans = Vec::new();
    let mut suggested_corrections = Vec::new();
    let mut critical_term_warnings = Vec::new();
    let mut ocr_semantic_warnings = Vec::new();
    let mut review_reasons = Vec::new();
    let mut warnings = Vec::new();
    let mut critical_keyword_uncertain = false;

    for candidate in candidates {
        let candidate_norm = normalize_for_critical_term_analysis(&candidate);
        if candidate_norm.is_empty() {
            continue;
        }
        if candidate_norm == answer_norm {
            continue;
        }
        if !candidate_is_eligible_for_near_match(&candidate_norm) {
            continue;
        }
        let distance = normalized_edit_distance(&answer_norm, &candidate_norm);
        if !is_critical_term_near_match(&answer_norm, &candidate_norm, distance) {
            continue;
        }

        critical_keyword_uncertain = true;
        let warning_code = CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string();
        let reason = CRITICAL_KEYWORD_OCR_UNCERTAIN_REASON.to_string();
        let warning = OcrCriticalTermWarning {
            observed_text: answer_text.to_string(),
            expected_or_related_term: candidate.clone(),
            reason: reason.clone(),
            warning_code: warning_code.clone(),
            highlight_region: None,
        };
        if !critical_term_warnings.contains(&warning) {
            critical_term_warnings.push(warning);
        }
        let suggested_correction = OcrSuggestedCorrection {
            original_text: answer_text.to_string(),
            suggested_text: candidate.clone(),
            reason: reason.clone(),
            confidence: None,
            applied: false,
            highlight_region: None,
        };
        if !suggested_corrections.contains(&suggested_correction) {
            suggested_corrections.push(suggested_correction);
        }
        let uncertain_span = OcrUncertainSpan {
            text: answer_text.to_string(),
            start: None,
            end: None,
            alternatives: vec![candidate.clone()],
            confidence: None,
            reason: reason.clone(),
            highlight_region: None,
        };
        if !uncertain_spans.contains(&uncertain_span) {
            uncertain_spans.push(uncertain_span);
        }
        if !ocr_semantic_warnings.contains(&warning_code) {
            ocr_semantic_warnings.push(warning_code.clone());
        }
        if !review_reasons.contains(&warning_code) {
            review_reasons.push(warning_code.clone());
        }
        if !warnings.contains(&warning_code) {
            warnings.push(warning_code);
        }
    }

    if record.critical_keyword_uncertain || critical_keyword_uncertain {
        if let Some((observed_text, suggested_text)) =
            derive_semantic_issue_from_answer(record, question)
        {
            let warning_code = CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string();
            let reason = CRITICAL_KEYWORD_OCR_UNCERTAIN_REASON.to_string();
            critical_term_warnings.push(OcrCriticalTermWarning {
                observed_text: observed_text.clone(),
                expected_or_related_term: suggested_text.clone(),
                reason: reason.clone(),
                warning_code: warning_code.clone(),
                highlight_region: None,
            });
            suggested_corrections.push(OcrSuggestedCorrection {
                original_text: observed_text.clone(),
                suggested_text: suggested_text.clone(),
                reason: reason.clone(),
                confidence: None,
                applied: false,
                highlight_region: None,
            });
            uncertain_spans.push(OcrUncertainSpan {
                text: observed_text,
                start: None,
                end: None,
                alternatives: vec![suggested_text],
                confidence: None,
                reason,
                highlight_region: None,
            });
            ocr_semantic_warnings.push(warning_code.clone());
            review_reasons.push(warning_code.clone());
            warnings.push(warning_code);
            return CriticalTermAnalysis {
                critical_keyword_uncertain: true,
                uncertain_spans,
                suggested_corrections,
                critical_term_warnings,
                ocr_semantic_warnings,
                review_reasons,
                warnings,
            };
        }
    }

    CriticalTermAnalysis {
        critical_keyword_uncertain,
        uncertain_spans,
        suggested_corrections,
        critical_term_warnings,
        ocr_semantic_warnings,
        review_reasons,
        warnings,
    }
}

fn collect_critical_term_candidates(question: &crate::domain::question::Question) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(expected_answer) = question.rubric.expected_answer.as_deref() {
        candidates.extend(split_critical_term_text(expected_answer));
    }
    for criterion in &question.rubric.criteria {
        candidates.extend(split_critical_term_text(&criterion.label));
        candidates.extend(split_critical_term_text(&criterion.description));
    }
    for hint in &question.rubric.partial_credit_hints {
        candidates.extend(split_critical_term_text(hint));
    }
    for condition in &question.rubric.zero_score_conditions {
        candidates.extend(split_critical_term_text(condition));
    }
    for mistake in &question.rubric.common_mistakes {
        candidates.extend(split_critical_term_text(mistake));
    }
    if candidates.is_empty() {
        candidates.extend(split_critical_term_text(&question.question_text.value));
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn split_critical_term_text(text: &str) -> Vec<String> {
    text.split(|ch: char| {
        matches!(
            ch,
            '\n' | ',' | ';' | ':' | '/' | '|' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    })
    .map(str::trim)
    .map(normalize_text_for_critical_term_analysis)
    .filter(|segment| !segment.is_empty())
    .filter(|segment| !crate::domain::rubric::is_placeholder_text(segment))
    .collect()
}

fn normalize_for_critical_term_analysis(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut previous_space = false;
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            normalized.push(ch);
            previous_space = false;
        } else if ch.is_whitespace() && !previous_space {
            normalized.push(' ');
            previous_space = true;
        }
    }
    normalized.trim().to_string()
}

fn normalize_text_for_critical_term_analysis(text: &str) -> String {
    normalize_for_critical_term_analysis(text)
}

fn candidate_is_eligible_for_near_match(candidate_norm: &str) -> bool {
    let token_count = candidate_norm.split_whitespace().count();
    candidate_norm.len() >= 10 && token_count >= 2
}

fn is_critical_term_near_match(answer_norm: &str, candidate_norm: &str, distance: usize) -> bool {
    if answer_norm.is_empty() || candidate_norm.is_empty() {
        return false;
    }
    if answer_norm == candidate_norm {
        return false;
    }
    let max_len = answer_norm.len().max(candidate_norm.len()).max(1);
    let ratio = distance as f32 / max_len as f32;
    distance <= 2 || ratio <= 0.12
}

fn normalized_edit_distance(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (i, left_char) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_char) in right.iter().enumerate() {
            let substitution = previous[j] + usize::from(left_char != right_char);
            let insertion = current[j] + 1;
            let deletion = previous[j + 1] + 1;
            current[j + 1] = substitution.min(insertion).min(deletion);
        }
        previous.clone_from(&current);
    }

    previous[right.len()]
}

fn merge_string_lists(mut left: Vec<String>, right: Vec<String>) -> Vec<String> {
    for item in right {
        if !left.contains(&item) {
            left.push(item);
        }
    }
    left.sort();
    left.dedup();
    left
}

fn merge_struct_lists<T>(mut left: Vec<T>, right: Vec<T>) -> Vec<T>
where
    T: PartialEq,
{
    for item in right {
        if !left.contains(&item) {
            left.push(item);
        }
    }
    left
}

fn build_student_answer_ocr_prompt(
    question_number: u32,
    question_text: &str,
    answer_type: &AnswerType,
    layout_hint: &str,
) -> String {
    let type_specific_instruction = answer_type_ocr_instruction(answer_type);
    format!(
        "Sen bir OCR transkripsiyon motorusun. Görevin yorumlamak değil, görseldeki öğrenci yazısını harfiyen aktarmaktır. Prompt sürümü: {PROMPT_VERSION}.\n\
Sadece öğrencinin el yazısı veya öğrencinin doldurduğu işaretleri çıkar.\n\
Basılı soru kökü, yönerge, puan bilgisi, şıklar, başlıklar ve soru numarasını cevaba ekleme.\n\
Soru metnini yalnızca basılı alanı ayırt etmek için kullan; cevabı tahmin etmek, tamamlamak, düzeltmek veya özetlemek için kullanma.\n\
Öğrencinin yazım, dilbilgisi, bilgi ve anlatım hatalarını aynen koru. Eş anlamlı sözcükle değiştirme, düzgün cümleye çevirme, açıklama veya yorum ekleme.\n\
Görselde bulunmayan hiçbir sözcüğü answerText alanına koyma. 'Öğrenci şunu demek istemiştir', 'cevap şudur' gibi yorum dili kullanma.\n\
Okunabilen bölümleri aynen yaz; yalnızca okunamayan en küçük parçayı [okunamadı] ile işaretle. Boş cevap alanında answerText boş string olsun.\n\
Karışım varsa needsReview true yap ve warnings içine printed_text_mixed ekle.\n\
Cevap okunmuyorsa [okunamadı] yaz, confidence değerini düşür ve belirsizliği reviewReasons içine ekle.\n\
Kritik bir terimden emin değilsen answerText'i otomatik düzeltme; uncertainSpans, suggestedCorrections, criticalTermWarnings ve ocrSemanticWarnings alanlarını doldur.\n\
criticalKeywordUncertain alanını kritik terim belirsizliği varsa true yap.\n\
structuredAnswer yalnızca answerText içeriğinin tablo/işaretleme gibi yapısal gösterimi olabilir; yeni bilgi ekleyemez.\n\
Yalnızca geçerli JSON döndür.\n\
\n\
JSON schema:\n\
{{\"answerText\":\"...\",\"structuredAnswer\":null,\"confidence\":0.0,\"uncertainSpans\":[],\"suggestedCorrections\":[],\"criticalTermWarnings\":[],\"ocrSemanticWarnings\":[],\"criticalKeywordUncertain\":false,\"needsReview\":true,\"reviewReasons\":[],\"warnings\":[]}}\n\
\n\
Soru numarası: {question_number}\n\
Soru metni: {question_text}\n\
Cevap tipi: {}\n\
Türe özel okuma kuralı: {type_specific_instruction}\n\
Layout ipucu: {layout_hint}\n\
Eger bir ifade ya da duzeltme crop üzerinde lokalize edilebiliyorsa highlightRegion alanına normalize bbox ekle. Bilmiyorsan highlightRegion alanını null bırak.\n",
        answer_type_label(answer_type)
    )
}

fn answer_type_ocr_instruction(answer_type: &AnswerType) -> &'static str {
    match answer_type {
        AnswerType::GeneralText | AnswerType::ShortText | AnswerType::Essay => {
            "Metni satır sırasını koruyarak birebir aktar; özetleme ve dil düzeltmesi yapma."
        }
        AnswerType::FillBlank => {
            "Yalnızca doldurulan boşlukları soldan sağa ve yukarıdan aşağıya sırala; structuredAnswer içinde kind=fill_blank ve index/text öğeleri kullan."
        }
        AnswerType::Table | AnswerType::CorrectionTable => {
            "Satır ve sütun konumlarını koru; yalnızca öğrencinin yazdığı hücreleri structuredAnswer içinde row/column/text olarak ver."
        }
        AnswerType::Matching => {
            "Yalnızca öğrencinin çizdiği veya yazdığı eşleri çıkar; structuredAnswer içinde kind=matching ve left/right çiftleri kullan. Bağlantıyı tahmin etme."
        }
        AnswerType::MultipleChoice => {
            "Yalnızca açıkça işaretlenmiş seçenekleri çıkar; silik, çift veya çelişkili işarette needsReview=true yap."
        }
        AnswerType::TrueFalse => {
            "Her madde için yalnızca öğrencinin işaretlediği doğru/yanlış seçimini sıra numarasıyla çıkar; işaretsiz maddeyi tahmin etme."
        }
        AnswerType::Ordering => {
            "Yalnızca öğrencinin verdiği sıra numaralarını veya sıralanmış öğeleri aynen çıkar; eksik sırayı tamamlama."
        }
        AnswerType::Numeric => {
            "Rakamları, ondalık ayıracı, eksi işaretini, birimi ve işlem satırlarını göründüğü gibi koru; hesaplama yapma."
        }
        AnswerType::DiagramLabeling => {
            "Yalnızca öğrencinin şema/görsel üzerine yazdığı etiketleri ve açık bağlantılarını çıkar; nesneyi tanıyıp eksik etiket üretme."
        }
        AnswerType::SentenceAnnotation => {
            "Yalnızca öğrencinin altını çizdiği, daire içine aldığı veya işaretlediği bölümü ve işaret türünü çıkar; yeni metin üretme."
        }
        AnswerType::GrammarAnalysis => {
            "Öğrencinin yazdığı dilbilgisi etiketlerini ve bağlandıkları sözcükleri aynen çıkar; doğru çözümlemeyi kendin üretme."
        }
    }
}

fn build_student_answer_issue_correction_prompt(
    question_number: u32,
    observed_text: &str,
    suggested_text_from_analyzer: &str,
    nearby_context: &str,
    critical_term_hint: Option<&str>,
    issue_id: Option<&str>,
    highlight_region: Option<&StudentAnswerOcrCropBBox>,
) -> String {
    let scope = issue_scope_hint(observed_text);
    let critical_term_hint = critical_term_hint.unwrap_or("none");
    let highlight_description = highlight_region
        .map(|bbox| {
            format!(
                "pageIndex={} x={} y={} width={} height={}",
                bbox.page_index, bbox.x, bbox.y, bbox.width, bbox.height
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let issue_id = issue_id.unwrap_or("unknown");
    format!(
        "Sen OCR sorun doğrulama asistanısın.\n\
İki aşama izle:\n\
A. Görsel okuma: yalnızca işaretli veya belirtilen sorunlu kelimeyi oku. Tüm cevabı yeniden yazma. Eksik cevabı tamamlamaya çalışma. Emin değilsen visualReading alanına null döndür.\n\
B. Bağlamlı öneri: OCR metni, yakın OCR bağlamı ve analizör önerisini kullanarak yalnızca aynı kapsamda düzeltme öner.\n\
Kesin olmayan görsel okumayı kesinmiş gibi sunma.\n\
Rubriğe bakıp öğrencinin yazmadığı doğru cevabı uydurma.\n\
Kapsamı genişletme. Tek kelimelik issue için suggestedText tek kelime kalmalı. Kısa ifade için de en fazla kısa ifade kalmalı.\n\
Sadece geçerli JSON döndür ve başka açıklama ekleme.\n\
\n\
JSON schema:\n\
{{\"decision\":\"suggest_correction|no_change|needs_teacher_review\",\"originalText\":\"string\",\"suggestedText\":\"string|null\",\"scope\":\"single_word|short_phrase\",\"visualReading\":\"string|null\",\"contextReason\":\"string\",\"confidence\":0.0,\"requiresTeacherApproval\":true,\"warnings\":[]}}\n\
\n\
Soru numarası: {question_number}\n\
Issue id: {issue_id}\n\
Scope: {scope}\n\
İşaretli bölge: {highlight_description}\n\
OCR observedText: {observed_text}\n\
Analizör önerisi: {suggested_text_from_analyzer}\n\
Kritik terim ipucu: {critical_term_hint}\n\
Yakın OCR bağlamı: {nearby_context}\n\
Kurallar: tüm cevabı yazma, eksik cevap tamamlamaya çalışma, rubriği kullanarak uydurma yapma, aynı kapsam dışına çıkma.\n",
        scope = scope,
        question_number = question_number,
        issue_id = issue_id,
        highlight_description = highlight_description,
        observed_text = observed_text,
        suggested_text_from_analyzer = suggested_text_from_analyzer,
        critical_term_hint = critical_term_hint,
        nearby_context = nearby_context,
    )
}

fn build_student_identity_ocr_prompt() -> String {
    r#"Sen öğrenci kimlik alanı OCR asistanısın.
Sadece verilen crop görselindeki ad soyad, okul numarası ve sınıf bilgisini çıkar.
Rubrik, cevap anahtarı, soru metni veya puanlama ile ilgili hiçbir çıkarım yapma.
Sadece geçerli JSON döndür:
{
  "displayName": string | null,
  "number": string | null,
  "className": string | null,
  "confidence": number,
  "needsReview": boolean,
  "warnings": string[]
}
Okuyamadığın alanları null yap. Emin değilsen needsReview=true kullan."#
        .to_string()
}

fn issue_scope_hint(text: &str) -> &'static str {
    if text.split_whitespace().count() <= 1 {
        "single_word"
    } else {
        "short_phrase"
    }
}

fn select_issue_base_image_ref(
    record: &StudentAnswerOcrRecord,
    crop_ref: Option<&str>,
    model_input_crop_ref: Option<&str>,
) -> Option<String> {
    crop_ref
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            model_input_crop_ref
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
        })
        .or_else(|| {
            record
                .model_input_crop_ref
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
        })
        .or_else(|| record.preprocessed_crop_refs.first().cloned())
        .or_else(|| record.original_crop_refs.first().cloned())
        .or_else(|| record.crop_refs.first().cloned())
        .or_else(|| record.full_page_preview_refs.first().cloned())
}

fn extract_issue_context(answer_text: &str, observed_text: &str) -> String {
    let answer_text = answer_text.trim();
    if answer_text.is_empty() {
        return String::new();
    }
    let needle = observed_text.trim();
    if needle.is_empty() {
        return answer_text.chars().take(220).collect();
    }
    if let Some(start) = answer_text.find(needle) {
        let window_start = start.saturating_sub(40);
        let window_end = (start + needle.len() + 40).min(answer_text.len());
        let mut snippet = answer_text[window_start..window_end].trim().to_string();
        if window_start > 0 {
            snippet = format!("…{snippet}");
        }
        if window_end < answer_text.len() {
            snippet.push('…');
        }
        return snippet;
    }
    answer_text.chars().take(220).collect()
}

fn extract_question_critical_term_hint(
    question: &crate::domain::question::Question,
) -> Option<String> {
    if let Some(expected_answer) = question.rubric.expected_answer.as_deref() {
        if let Some(token) = expected_answer
            .split_whitespace()
            .map(str::trim)
            .find(|token| !token.is_empty())
        {
            return Some(token.to_string());
        }
    }
    question
        .rubric
        .criteria
        .iter()
        .map(|criterion| criterion.label.trim())
        .find(|label| !label.is_empty())
        .map(ToString::to_string)
}

struct PreprocessedOcrInputs {
    model_input_images: Vec<(u32, PathBuf)>,
    model_input_crop_ref: Option<String>,
    original_crop_refs: Vec<String>,
    preprocessed_crop_refs: Vec<String>,
    preprocess_mode: OcrImagePreprocessMode,
    preprocess_version: String,
    available_preprocess_variants: Vec<OcrImagePreprocessMode>,
    preprocess_applied: bool,
    preprocess_warnings: Vec<String>,
    preprocess_diagnostics: Vec<OcrImagePreprocessDiagnostics>,
}

impl StudentAnswerOcrService {
    fn preprocess_model_inputs(
        &self,
        project_root: &Path,
        sources: &[(u32, PathBuf)],
        _mode: OcrImagePreprocessMode,
    ) -> Result<PreprocessedOcrInputs, AppError> {
        if sources.is_empty() {
            return Err(AppError {
                code: AppErrorCode::FileReadFailed,
                message: "OCR giriş görüntüsü bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce crop önizlemesini oluşturun.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let mut model_input_images = Vec::with_capacity(sources.len());
        let mut original_crop_refs = Vec::with_capacity(sources.len());
        let mut preprocessed_crop_refs = Vec::with_capacity(sources.len());
        let mut preprocess_warnings = Vec::new();
        let mut preprocess_diagnostics = Vec::new();
        let mut source_results = Vec::with_capacity(sources.len());

        for (page_number, source_path) in sources {
            let source_ref = source_path.to_string_lossy().to_string();
            original_crop_refs.push(source_ref.clone());
            let mut variant_results: Vec<(OcrImagePreprocessMode, Option<PathBuf>)> = Vec::new();
            for variant in PREPROCESS_VARIANTS {
                match self.ocr_image_preprocess_service.preprocess_image(
                    project_root,
                    source_path,
                    variant,
                ) {
                    Ok(result) => {
                        preprocess_diagnostics.push(result.diagnostics.clone());
                        variant_results
                            .push((variant, Some(PathBuf::from(result.output_image_path))));
                    }
                    Err(error) => {
                        preprocess_diagnostics.push(preprocess_failure_diagnostics(
                            variant,
                            source_path,
                            error.message.clone(),
                        ));
                        variant_results.push((variant, None));
                    }
                }
            }
            source_results.push((*page_number, source_path.clone(), variant_results));
        }

        let handwriting_available = source_results.iter().all(|(_, _, variants)| {
            variants.iter().any(|(variant, path)| {
                *variant == OcrImagePreprocessMode::HandwritingEnhanced && path.is_some()
            })
        });
        let clean_available = source_results.iter().all(|(_, _, variants)| {
            variants.iter().any(|(variant, path)| {
                *variant == OcrImagePreprocessMode::CleanGrayscale && path.is_some()
            })
        });
        let selected_mode = if handwriting_available {
            OcrImagePreprocessMode::HandwritingEnhanced
        } else if clean_available {
            OcrImagePreprocessMode::CleanGrayscale
        } else {
            OcrImagePreprocessMode::Original
        };

        if selected_mode != OcrImagePreprocessMode::HandwritingEnhanced {
            preprocess_warnings.push("preprocess_failed".to_string());
            preprocess_warnings.push("preprocess_fallback_used".to_string());
        }

        for (page_number, source_path, variants) in &source_results {
            let selected_path = if selected_mode == OcrImagePreprocessMode::Original {
                source_path.clone()
            } else {
                variants
                    .iter()
                    .find(|(variant, path)| *variant == selected_mode && path.is_some())
                    .and_then(|(_, path)| path.clone())
                    .unwrap_or_else(|| source_path.clone())
            };
            if selected_mode != OcrImagePreprocessMode::Original {
                preprocessed_crop_refs.push(selected_path.to_string_lossy().to_string());
            }
            model_input_images.push((*page_number, selected_path));
        }

        let model_input_crop_ref = model_input_images
            .first()
            .map(|(_, path)| path.to_string_lossy().to_string());
        let preprocess_applied = selected_mode != OcrImagePreprocessMode::Original;

        Ok(PreprocessedOcrInputs {
            model_input_images,
            model_input_crop_ref,
            original_crop_refs,
            preprocessed_crop_refs,
            preprocess_mode: selected_mode,
            preprocess_version: PREPROCESS_VERSION.to_string(),
            available_preprocess_variants: PREPROCESS_VARIANTS.to_vec(),
            preprocess_applied,
            preprocess_warnings,
            preprocess_diagnostics,
        })
    }
}

fn preprocess_failure_diagnostics(
    mode: OcrImagePreprocessMode,
    source_path: &Path,
    warning: String,
) -> OcrImagePreprocessDiagnostics {
    OcrImagePreprocessDiagnostics {
        mode,
        preprocess_version: PREPROCESS_VERSION.to_string(),
        source_image_path: source_path.to_string_lossy().to_string(),
        output_image_path: source_path.to_string_lossy().to_string(),
        source_width: 0,
        source_height: 0,
        output_width: 0,
        output_height: 0,
        source_bytes: 0,
        output_bytes: 0,
        cache_hit: false,
        applied: false,
        warnings: vec![warning],
        error_message: Some("preprocess_failed".to_string()),
        technical_details: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, DocumentRole, PdfPreviewState, PdfPreviewStatus};
    use crate::domain::errors::AppErrorCode;
    use crate::domain::job::JobStatus;
    use crate::domain::question::{
        default_question, AnswerType, TextFieldSource, TextFieldState, TextFieldStatus,
    };
    use crate::domain::rubric::RubricCriterion;
    use crate::domain::student::{
        new_student_id, Student, StudentAnswerCropTemplateItem, StudentAnswerOcrCropBBox,
        StudentSubmission, StudentSubmissionStatus,
    };
    use crate::jobs::job_manager::JobManager;
    use crate::services::model_config_service::ModelConfigService;
    use crate::services::model_process_manager::test_support::{
        available_loopback_port, blocking_lock_model_runtime_test,
    };
    use crate::services::model_process_manager::ModelProcessManager;
    use crate::services::model_runtime_service::ModelRuntimeService;
    use crate::services::pdf_preview_service::PdfPreviewService;
    use crate::services::pdf_service::SystemPdfService;
    use crate::services::project_store::ProjectStore;
    use crate::services::student_answer_crop_service::crop_rect;
    use image::{DynamicImage, ImageBuffer, Rgba};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("rubrika-ocr-preflight-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn test_config_service() -> ModelConfigService {
        ModelConfigService::new_with_path(
            std::env::temp_dir().join(format!("rubrika-model-config-{}.json", Uuid::new_v4())),
        )
    }

    fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone()
    }

    fn preprocess_service() -> StudentAnswerOcrService {
        let project_store = ProjectStore::new();
        let config_service = test_config_service();
        let runtime_service = ModelRuntimeService::new(
            config_service.clone(),
            ModelProcessManager::new_with_state_path(
                config_service,
                Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
                std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
            ),
        );
        StudentAnswerOcrService::new(
            project_store.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                project_store,
                Arc::new(SystemPdfService),
                Arc::new(JobManager::new()),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            Arc::new(JobManager::new()),
        )
    }

    fn write_mock_llama_server_script(
        startup_delay_seconds: u64,
        health_ok: bool,
        ocr_content: Option<&str>,
    ) -> PathBuf {
        let path = std::env::temp_dir().join(format!("rubrika-mock-llama-{}.sh", Uuid::new_v4()));
        let health_response = if health_ok {
            r#"            self._write_json({"status": "ok"})"#
        } else {
            r#"            self._write_json({"status": "booting"}, 503)"#
        };
        let ocr_response = if let Some(content) = ocr_content {
            let content = serde_json::to_string(content).unwrap();
            format!(
                "            self._write_json({{\"choices\": [{{\"message\": {{\"content\": {content}}}}}]}})"
            )
        } else {
            r#"            self._write_json({"error": "not found"}, 404)"#.to_string()
        };
        let script = r#"#!/bin/sh
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  cat <<'EOF'
--cache-type-k
--cache-type-v
--mmproj-offload
EOF
  exit 0
fi
host=127.0.0.1
port=8080
while [ "$#" -gt 0 ]; do
  case "$1" in
    --host)
      host="$2"
      shift 2
      ;;
    --port)
      port="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
exec python3 - "$host" "$port" <<'PY'
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

host = sys.argv[1]
port = int(sys.argv[2])

time.sleep(__STARTUP_DELAY__)

class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        return

    def _write_json(self, body, status=200):
        payload = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self):
        if self.path == "/health":
__HEALTH_RESPONSE__
            return
        self._write_json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path == "/v1/chat/completions":
            length = int(self.headers.get("Content-Length", "0") or "0")
            _ = self.rfile.read(length)
__OCR_RESPONSE__
            return
        self._write_json({"error": "not found"}, 404)

HTTPServer((host, port), Handler).serve_forever()
PY
"#;
        let script = script
            .replace("__STARTUP_DELAY__", &startup_delay_seconds.to_string())
            .replace("__HEALTH_RESPONSE__", health_response)
            .replace("__OCR_RESPONSE__", &ocr_response);
        fs::write(&path, script).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[test]
    fn start_job_auto_starts_model_and_records_progress() {
        let _model_runtime_guard = blocking_lock_model_runtime_test();
        let root = temp_root();
        let port = available_loopback_port();
        let base_url = format!("http://127.0.0.1:{port}");
        let host = "127.0.0.1";

        let project_store = ProjectStore::new();
        let mut project = project_store
            .create_project("OCR Test".to_string(), root.to_string_lossy().to_string())
            .unwrap();
        fs::write(root.join("student.pdf"), b"student-source").unwrap();
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.max_score = 1.0;
        question.question_text = TextFieldState {
            value: "Soru 1".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        project.questions = vec![question];
        let document = Document {
            id: Uuid::new_v4().to_string(),
            role: DocumentRole::StudentScan,
            file_name: "student.pdf".to_string(),
            stored_path: "student.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        };
        project.documents = vec![document.clone()];
        project.student_scan_document_id = Some(document.id.clone());
        let student_id = new_student_id();
        project.students = vec![Student {
            id: student_id.clone(),
            display_name: Some("Öğrenci".to_string()),
            number: Some("1".to_string()),
            class_name: Some("10-A".to_string()),
            warnings: vec![],
            identity_ocr: None,
        }];
        project.student_submissions = vec![StudentSubmission {
            id: Uuid::new_v4().to_string(),
            student_id,
            document_id: document.id,
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        }];
        project.student_grouping_complete_at = Some(chrono::Utc::now().to_rfc3339());
        project.expected_question_count = Some(1);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        project_store.save_project(&project).unwrap();

        let config_service = test_config_service();
        let server_path = write_mock_llama_server_script(
            2,
            true,
            Some(
                r#"{"answerText":"Cevap","confidence":0.91,"needsReview":true,"reviewReasons":["mock"],"warnings":[]}"#,
            ),
        );
        let model_path =
            std::env::temp_dir().join(format!("rubrika-model-{}.gguf", Uuid::new_v4()));
        let mmproj_path =
            std::env::temp_dir().join(format!("rubrika-mmproj-{}.bin", Uuid::new_v4()));
        fs::write(&model_path, b"dummy").unwrap();
        fs::write(&mmproj_path, b"dummy").unwrap();
        let mut profile = config_service.get_profile(None).unwrap();
        profile.id = format!("ocr-autostart-{}", Uuid::new_v4());
        profile.display_name = "OCR autostart test".to_string();
        profile.mode = crate::domain::model::ModelMode::External;
        profile.server_path = server_path.to_string_lossy().to_string();
        profile.model_path = model_path.to_string_lossy().to_string();
        profile.mmproj_path = mmproj_path.to_string_lossy().to_string();
        profile.base_url = base_url;
        profile.host = host.to_string();
        profile.port = port;
        config_service.update_profile(profile).unwrap();

        let manager = ModelProcessManager::new_with_state_path(
            config_service.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
        );
        let runtime_service = ModelRuntimeService::new(config_service, manager.clone());
        let job_manager = Arc::new(JobManager::new());
        let service = StudentAnswerOcrService::new(
            project_store.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                project_store.clone(),
                Arc::new(SystemPdfService),
                job_manager.clone(),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            job_manager.clone(),
        );

        let app = mock_app();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let started = service.start(app, project.id.clone(), false).await.unwrap();
            assert_eq!(started.status, "queued");

            let mut saw_start_message = false;
            let mut terminal_job = None;
            for _ in 0..40 {
                let job = job_manager.get_job_snapshot(&started.job_id).unwrap();
                if job.progress.message == "Model sunucusu başlatılıyor..." {
                    saw_start_message = true;
                }
                if job.status.is_terminal() {
                    terminal_job = Some(job);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }

            assert!(
                saw_start_message,
                "startup progress message was not recorded"
            );

            let job = terminal_job.unwrap_or_else(|| {
                job_manager
                    .get_job_snapshot(&started.job_id)
                    .expect("job snapshot")
            });
            assert!(job.status.is_terminal());
        });
    }

    #[test]
    fn start_job_returns_specific_model_configuration_errors() {
        let root = temp_root();
        let project_store = ProjectStore::new();
        let mut project = project_store
            .create_project(
                "OCR Error Test".to_string(),
                root.to_string_lossy().to_string(),
            )
            .unwrap();
        fs::write(root.join("student.pdf"), b"student-source").unwrap();
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.max_score = 1.0;
        question.question_text = TextFieldState {
            value: "Soru 1".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        project.questions = vec![question];
        let document = Document {
            id: Uuid::new_v4().to_string(),
            role: DocumentRole::StudentScan,
            file_name: "student.pdf".to_string(),
            stored_path: "student.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        };
        project.documents = vec![document.clone()];
        project.student_scan_document_id = Some(document.id.clone());
        let student_id = new_student_id();
        project.students = vec![Student {
            id: student_id.clone(),
            display_name: Some("Öğrenci".to_string()),
            number: Some("1".to_string()),
            class_name: Some("10-A".to_string()),
            warnings: vec![],
            identity_ocr: None,
        }];
        project.student_submissions = vec![StudentSubmission {
            id: Uuid::new_v4().to_string(),
            student_id,
            document_id: document.id,
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        }];
        project.student_grouping_complete_at = Some(chrono::Utc::now().to_rfc3339());
        project.expected_question_count = Some(1);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        project_store.save_project(&project).unwrap();

        let app = mock_app();
        let job_manager = Arc::new(JobManager::new());
        let config_service = test_config_service();
        let mut profile = config_service.get_profile(None).unwrap();
        profile.server_path.clear();
        profile.model_path.clear();
        profile.mmproj_path.clear();
        config_service.update_profile(profile).unwrap();
        let runtime_service = ModelRuntimeService::new(
            config_service.clone(),
            ModelProcessManager::new_with_state_path(
                config_service,
                Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
                std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
            ),
        );
        let service = StudentAnswerOcrService::new(
            project_store.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                project_store.clone(),
                Arc::new(SystemPdfService),
                job_manager.clone(),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            job_manager.clone(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let started = service
                .start(app.clone(), project.id.clone(), false)
                .await
                .unwrap();
            let mut job = None;
            for _ in 0..80 {
                let snapshot = job_manager.get_job_snapshot(&started.job_id).unwrap();
                if matches!(snapshot.status, JobStatus::Failed | JobStatus::Succeeded) {
                    job = Some(snapshot);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            let job = job.expect("config_missing");
            assert_eq!(
                job.error.as_ref().map(|error| error.code.clone()),
                Some(AppErrorCode::ModelConfigMissing)
            );
        });
    }

    #[test]
    fn start_job_returns_model_mmproj_missing() {
        let _model_runtime_guard = blocking_lock_model_runtime_test();
        let root = temp_root();
        let project_store = ProjectStore::new();
        let mut project = project_store
            .create_project(
                "OCR Error Test".to_string(),
                root.to_string_lossy().to_string(),
            )
            .unwrap();
        fs::write(root.join("student.pdf"), b"student-source").unwrap();
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.max_score = 1.0;
        question.question_text = TextFieldState {
            value: "Soru 1".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        project.questions = vec![question];
        let document = Document {
            id: Uuid::new_v4().to_string(),
            role: DocumentRole::StudentScan,
            file_name: "student.pdf".to_string(),
            stored_path: "student.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        };
        project.documents = vec![document.clone()];
        project.student_scan_document_id = Some(document.id.clone());
        let student_id = new_student_id();
        project.students = vec![Student {
            id: student_id.clone(),
            display_name: Some("Öğrenci".to_string()),
            number: Some("1".to_string()),
            class_name: Some("10-A".to_string()),
            warnings: vec![],
            identity_ocr: None,
        }];
        project.student_submissions = vec![StudentSubmission {
            id: Uuid::new_v4().to_string(),
            student_id,
            document_id: document.id,
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        }];
        project.student_grouping_complete_at = Some(chrono::Utc::now().to_rfc3339());
        project.expected_question_count = Some(1);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        project_store.save_project(&project).unwrap();

        let app = mock_app();
        let job_manager = Arc::new(JobManager::new());
        let config_service = test_config_service();
        let mut profile = config_service.get_profile(None).unwrap();
        profile.mode = crate::domain::model::ModelMode::Managed;
        profile.server_path = write_mock_llama_server_script(0, true, None)
            .to_string_lossy()
            .to_string();
        let model_path =
            std::env::temp_dir().join(format!("rubrika-model-{}.gguf", Uuid::new_v4()));
        fs::write(&model_path, b"dummy").unwrap();
        profile.model_path = model_path.to_string_lossy().to_string();
        profile.mmproj_path = std::env::temp_dir()
            .join(format!("missing-mmproj-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        config_service.update_profile(profile).unwrap();
        let runtime_service = ModelRuntimeService::new(
            config_service.clone(),
            ModelProcessManager::new_with_state_path(
                config_service,
                Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
                std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
            ),
        );
        let service = StudentAnswerOcrService::new(
            project_store.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                project_store.clone(),
                Arc::new(SystemPdfService),
                job_manager.clone(),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            job_manager.clone(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let started = service
                .start(app.clone(), project.id.clone(), false)
                .await
                .unwrap();
            let mut job = None;
            for _ in 0..20 {
                let snapshot = job_manager.get_job_snapshot(&started.job_id).unwrap();
                if matches!(snapshot.status, JobStatus::Failed | JobStatus::Succeeded) {
                    job = Some(snapshot);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            let job = job.expect("mmproj_missing");
            assert_eq!(
                job.error.as_ref().map(|error| error.code.clone()),
                Some(AppErrorCode::ModelMmprojMissing)
            );
        });
    }

    #[test]
    fn start_job_returns_model_start_failed_when_binary_exits() {
        let _model_runtime_guard = blocking_lock_model_runtime_test();
        let root = temp_root();
        let project_store = ProjectStore::new();
        let mut project = project_store
            .create_project(
                "OCR Start Fail Test".to_string(),
                root.to_string_lossy().to_string(),
            )
            .unwrap();
        fs::write(root.join("student.pdf"), b"student-source").unwrap();
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.max_score = 1.0;
        question.question_text = TextFieldState {
            value: "Soru 1".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        project.questions = vec![question];
        let document = Document {
            id: Uuid::new_v4().to_string(),
            role: DocumentRole::StudentScan,
            file_name: "student.pdf".to_string(),
            stored_path: "student.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        };
        project.documents = vec![document.clone()];
        project.student_scan_document_id = Some(document.id.clone());
        let student_id = new_student_id();
        project.students = vec![Student {
            id: student_id.clone(),
            display_name: Some("Öğrenci".to_string()),
            number: Some("1".to_string()),
            class_name: Some("10-A".to_string()),
            warnings: vec![],
            identity_ocr: None,
        }];
        project.student_submissions = vec![StudentSubmission {
            id: Uuid::new_v4().to_string(),
            student_id,
            document_id: document.id,
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        }];
        project.student_grouping_complete_at = Some(chrono::Utc::now().to_rfc3339());
        project.expected_question_count = Some(1);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        project_store.save_project(&project).unwrap();

        let app = mock_app();
        let job_manager = Arc::new(JobManager::new());
        let config_service = test_config_service();
        let mut profile = config_service.get_profile(None).unwrap();
        profile.id = format!("ocr-start-failure-{}", Uuid::new_v4());
        profile.display_name = "OCR start failure test".to_string();
        profile.mode = crate::domain::model::ModelMode::Managed;
        let port = available_loopback_port();
        profile.host = "127.0.0.1".to_string();
        profile.port = port;
        profile.base_url = format!("http://127.0.0.1:{port}");
        let server_path =
            std::env::temp_dir().join(format!("rubrika-nonexec-{}.txt", Uuid::new_v4()));
        fs::write(&server_path, b"not executable").unwrap();
        let mut permissions = fs::metadata(&server_path).unwrap().permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&server_path, permissions).unwrap();
        profile.server_path = server_path.to_string_lossy().to_string();
        let model_path =
            std::env::temp_dir().join(format!("rubrika-model-{}.gguf", Uuid::new_v4()));
        let mmproj_path =
            std::env::temp_dir().join(format!("rubrika-mmproj-{}.bin", Uuid::new_v4()));
        fs::write(&model_path, b"dummy").unwrap();
        fs::write(&mmproj_path, b"dummy").unwrap();
        profile.model_path = model_path.to_string_lossy().to_string();
        profile.mmproj_path = mmproj_path.to_string_lossy().to_string();
        config_service.update_profile(profile).unwrap();
        let runtime_service = ModelRuntimeService::new(
            config_service.clone(),
            ModelProcessManager::new_with_state_path(
                config_service,
                Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
                std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
            ),
        );
        let service = StudentAnswerOcrService::new(
            project_store.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                project_store.clone(),
                Arc::new(SystemPdfService),
                job_manager.clone(),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            job_manager.clone(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let started = service
                .start(app.clone(), project.id.clone(), false)
                .await
                .unwrap();
            let mut job = None;
            for _ in 0..20 {
                let snapshot = job_manager.get_job_snapshot(&started.job_id).unwrap();
                if snapshot.status.is_terminal() {
                    job = Some(snapshot);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            let job = job.expect("start_failed");
            assert!(matches!(
                job.error.as_ref().map(|error| error.code.clone()),
                Some(AppErrorCode::ModelStartFailed | AppErrorCode::ModelRuntimeStartFailed)
            ));
        });
    }

    #[test]
    fn prompt_does_not_include_scoring_fields() {
        let prompt = build_student_answer_ocr_prompt(
            4,
            "Soru metni",
            &AnswerType::GeneralText,
            "crop page 4",
        );

        for forbidden in [
            "expectedAnswer",
            "rubric",
            "criteria",
            "maxScore",
            "partialCreditHints",
            "zeroPointConditions",
            "commonMistakes",
            "answer key",
        ] {
            assert!(
                !prompt.contains(forbidden),
                "{forbidden} leaked into OCR prompt"
            );
        }
        assert!(prompt.contains("Soru numarası: 4"));
        assert!(prompt.contains("Soru metni: Soru metni"));
        assert!(prompt.contains("Layout ipucu: crop page 4"));
        assert!(prompt.contains("Yalnızca geçerli JSON döndür"));
        assert!(prompt.contains("highlightRegion"));
    }

    #[test]
    fn prompt_for_fill_blank_requires_ordered_verbatim_slots() {
        let prompt = build_student_answer_ocr_prompt(
            3,
            "Boşlukları doldurunuz.",
            &AnswerType::FillBlank,
            "crop page 3",
        );
        assert!(prompt.contains(PROMPT_VERSION));
        assert!(prompt.contains("doldurulan boşlukları"));
        assert!(prompt.contains("kind=fill_blank"));
        assert!(prompt.contains("tahmin etmek, tamamlamak, düzeltmek veya özetlemek için kullanma"));
    }

    #[test]
    fn every_structured_answer_type_has_specific_ocr_instruction() {
        for answer_type in [
            AnswerType::FillBlank,
            AnswerType::Table,
            AnswerType::CorrectionTable,
            AnswerType::Matching,
            AnswerType::MultipleChoice,
            AnswerType::TrueFalse,
            AnswerType::Ordering,
            AnswerType::Numeric,
            AnswerType::DiagramLabeling,
            AnswerType::SentenceAnnotation,
            AnswerType::GrammarAnalysis,
        ] {
            let instruction = answer_type_ocr_instruction(&answer_type);
            assert!(!instruction.trim().is_empty());
            assert_ne!(
                instruction,
                answer_type_ocr_instruction(&AnswerType::GeneralText)
            );
        }
    }

    #[test]
    fn preprocess_model_inputs_prefers_handwriting_enhanced() {
        let root = temp_root();
        let image_path = root.join("crop.png");
        let mut image = ImageBuffer::from_pixel(24, 24, Rgba([245, 245, 245, 255]));
        for y in 7..17 {
            for x in 5..19 {
                image.put_pixel(x, y, Rgba([60, 60, 60, 255]));
            }
        }
        DynamicImage::ImageRgba8(image).save(&image_path).unwrap();

        let service = preprocess_service();
        let inputs = service
            .preprocess_model_inputs(
                &root,
                &[(1, image_path.clone())],
                OcrImagePreprocessMode::HandwritingEnhanced,
            )
            .unwrap();

        assert_eq!(
            inputs.preprocess_mode,
            OcrImagePreprocessMode::HandwritingEnhanced
        );
        assert!(inputs.preprocess_applied);
        assert!(inputs.preprocess_warnings.is_empty());
        assert_eq!(inputs.available_preprocess_variants.len(), 5);
        assert!(inputs
            .model_input_crop_ref
            .as_deref()
            .is_some_and(|path| path.contains("handwriting_enhanced")));
        assert_eq!(inputs.model_input_images.len(), 1);
    }

    #[test]
    fn preprocess_model_inputs_falls_back_without_crashing() {
        let root = temp_root();
        let missing_path = root.join("missing.png");
        let service = preprocess_service();
        let inputs = service
            .preprocess_model_inputs(
                &root,
                &[(1, missing_path.clone())],
                OcrImagePreprocessMode::HandwritingEnhanced,
            )
            .unwrap();

        assert_eq!(inputs.preprocess_mode, OcrImagePreprocessMode::Original);
        assert!(!inputs.preprocess_applied);
        assert_eq!(
            inputs.model_input_crop_ref,
            Some(missing_path.to_string_lossy().to_string())
        );
        assert!(inputs
            .preprocess_warnings
            .contains(&"preprocess_failed".to_string()));
        assert!(inputs
            .preprocess_warnings
            .contains(&"preprocess_fallback_used".to_string()));
        assert!(inputs
            .preprocess_diagnostics
            .iter()
            .any(|diag| diag.mode == OcrImagePreprocessMode::Original));
    }

    #[test]
    fn status_priority_marks_parse_failure_before_review() {
        assert_eq!(
            derive_student_answer_status(true, false, false, false, false),
            StudentAnswerOcrStatus::ParseFailed
        );
        assert_eq!(
            derive_student_answer_status(false, true, false, false, false),
            StudentAnswerOcrStatus::CropMissing
        );
        assert_eq!(
            derive_student_answer_status(false, false, false, true, false),
            StudentAnswerOcrStatus::PrintedTextLeakSuspected
        );
        assert_eq!(
            derive_student_answer_status(false, false, true, false, false),
            StudentAnswerOcrStatus::PartialAnswerSuspected
        );
        assert_eq!(
            derive_student_answer_status(false, false, false, false, true),
            StudentAnswerOcrStatus::ReviewNeeded
        );
    }

    #[test]
    fn deterministic_critical_term_analyzer_flags_near_match_and_keeps_original_answer_text() {
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.question_text = TextFieldState {
            value: "Çelişen sözcük kullanımıyla ilgili ifadeyi yazın.".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        question.rubric.expected_answer = Some("çelişen sözcük kullanımı".to_string());
        question.rubric.criteria = vec![RubricCriterion {
            id: "c1".to_string(),
            label: "Kritik terim".to_string(),
            description: "Çelişen sözcük kullanımı doğru kullanılmalı.".to_string(),
            points: 1.0,
        }];
        question.rubric.partial_credit_hints = vec!["Kritik terim doğru olmalı.".to_string()];
        question.rubric.zero_score_conditions = vec!["Kritik terim yanlışsa sıfır.".to_string()];
        question.rubric.common_mistakes = vec!["gelişen sözcük kullanımı".to_string()];

        let mut record = StudentAnswerOcrRecord {
            answer_text: "gelişen sözcük kullanımı".to_string(),
            ..StudentAnswerOcrRecord::default()
        };

        apply_deterministic_critical_term_analysis(&mut record, &question);

        assert!(record.critical_keyword_uncertain);
        assert!(record.needs_review);
        assert_eq!(record.answer_text, "gelişen sözcük kullanımı");
        assert!(record
            .critical_term_warnings
            .iter()
            .any(
                |warning| warning.warning_code == CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING
                    && warning.expected_or_related_term == "çelişen sözcük kullanımı"
            ));
        assert!(record.suggested_corrections.iter().any(|correction| {
            correction.suggested_text == "çelişen sözcük kullanımı" && !correction.applied
        }));
    }

    #[test]
    fn deterministic_critical_term_analyzer_finds_suffix_match_inside_full_answer_text() {
        let mut question = default_question(5);
        question.answer_type = AnswerType::GeneralText;
        question.question_text = TextFieldState {
            value: "Anlatım bozukluğunu bulun.".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        question.rubric.expected_answer = Some("çelişen sözcük kullanımı".to_string());

        let mut record = StudentAnswerOcrRecord {
            answer_text: "2. Anlatım Bozukluğunun Nedeni: gelşeqiz sözcük kullanımı".to_string(),
            critical_keyword_uncertain: true,
            ocr_semantic_warnings: vec!["critical_keyword_ocr_uncertain".to_string()],
            ..StudentAnswerOcrRecord::default()
        };

        apply_deterministic_critical_term_analysis(&mut record, &question);

        assert!(record.critical_keyword_uncertain);
        assert_eq!(record.suggested_corrections.len(), 1);
        assert_eq!(record.suggested_corrections[0].original_text, "gelşeqiz");
        assert_eq!(record.suggested_corrections[0].suggested_text, "çelişen");
        assert!(record
            .critical_term_warnings
            .iter()
            .any(|warning| warning.expected_or_related_term == "çelişen"));
    }

    #[test]
    fn deterministic_critical_term_analyzer_ignores_short_exact_terms() {
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.rubric.expected_answer = Some("özne".to_string());

        let mut record = StudentAnswerOcrRecord {
            answer_text: "özne".to_string(),
            ..StudentAnswerOcrRecord::default()
        };

        apply_deterministic_critical_term_analysis(&mut record, &question);

        assert!(!record.critical_keyword_uncertain);
        assert!(!record.needs_review);
        assert!(record.critical_term_warnings.is_empty());
        assert!(record.suggested_corrections.is_empty());
    }

    #[test]
    fn deterministic_critical_term_analyzer_ignores_irrelevant_similarity() {
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.rubric.expected_answer = Some("elektrik devresi".to_string());

        let mut record = StudentAnswerOcrRecord {
            answer_text: "orman gezisi".to_string(),
            ..StudentAnswerOcrRecord::default()
        };

        apply_deterministic_critical_term_analysis(&mut record, &question);

        assert!(!record.critical_keyword_uncertain);
        assert!(!record.needs_review);
        assert!(record.critical_term_warnings.is_empty());
        assert!(record.suggested_corrections.is_empty());
    }

    #[test]
    fn crop_rect_applies_margin_and_clamps_to_page_edges() {
        let template = StudentAnswerCropTemplateItem {
            question_id: "q1".to_string(),
            question_number: 1,
            page_index_within_submission: 0,
            bbox: StudentAnswerOcrCropBBox {
                x: 0.92,
                y: 0.90,
                width: 0.12,
                height: 0.18,
                page_index: 0,
            },
            label: None,
            note: None,
        };

        let (x, y, w, h, clamped, margin_applied) = crop_rect(&template, 1000, 800);
        assert!(x < 1000);
        assert!(y < 800);
        assert!(w > 0);
        assert!(h > 0);
        assert!(clamped);
        assert!(margin_applied);
    }

    #[test]
    fn failed_record_is_reviewable_and_keeps_empty_crop_refs() {
        let submission = StudentSubmission {
            id: "submission-1".to_string(),
            student_id: "student-1".to_string(),
            document_id: "document-1".to_string(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        };
        let question = default_question(1);

        let project_store = ProjectStore::new();
        let job_manager = Arc::new(JobManager::new());
        let config_service = test_config_service();
        let runtime_service = ModelRuntimeService::new(
            config_service.clone(),
            ModelProcessManager::new_with_state_path(
                config_service,
                Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
                std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
            ),
        );
        let service = StudentAnswerOcrService::new(
            project_store,
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                ProjectStore::new(),
                Arc::new(SystemPdfService),
                job_manager.clone(),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            job_manager,
        );

        let record = service.failed_record(
            &submission,
            &question,
            StudentAnswerOcrStatus::ModelError,
            "boom".to_string(),
            vec![1],
            vec!["boom".to_string()],
        );

        assert_eq!(record.status, StudentAnswerOcrStatus::ModelError);
        assert!(record.needs_review);
        assert!(record.crop_refs.is_empty());
        assert!(record.full_page_preview_refs.is_empty());
    }

    #[test]
    fn force_rerun_preserves_active_records_and_stages_a_candidate() {
        let root = temp_root();
        let project_store = ProjectStore::new();
        let mut project = project_store
            .create_project(
                "OCR Rerun Test".to_string(),
                root.to_string_lossy().to_string(),
            )
            .unwrap();
        std::fs::write(root.join("student.pdf"), b"student-source").unwrap();
        let mut question = default_question(1);
        question.answer_type = AnswerType::GeneralText;
        question.max_score = 1.0;
        question.question_text = TextFieldState {
            value: "Soru 1".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: Some(1.0),
            warnings: vec![],
            updated_at: None,
        };
        project.questions = vec![question];
        let document = Document {
            id: Uuid::new_v4().to_string(),
            role: DocumentRole::StudentScan,
            file_name: "student.pdf".to_string(),
            stored_path: "student.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(PdfPreviewState {
                status: PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        };
        project.documents = vec![document.clone()];
        project.student_scan_document_id = Some(document.id.clone());
        let student_id = new_student_id();
        project.students = vec![Student {
            id: student_id.clone(),
            display_name: Some("Öğrenci".to_string()),
            number: Some("1".to_string()),
            class_name: Some("10-A".to_string()),
            warnings: vec![],
            identity_ocr: None,
        }];
        project.student_submissions = vec![StudentSubmission {
            id: Uuid::new_v4().to_string(),
            student_id,
            document_id: document.id,
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![1],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        }];
        project.student_grouping_complete_at = Some(chrono::Utc::now().to_rfc3339());
        project.expected_question_count = Some(1);
        project.student_answer_ocr_records = vec![StudentAnswerOcrRecord {
            id: Uuid::new_v4().to_string(),
            submission_id: project.student_submissions[0].id.clone(),
            question_id: project.questions[0].id.clone(),
            question_number: 1,
            source_page_numbers: vec![1],
            source_image_refs: vec!["old.png".to_string()],
            crop_refs: vec!["old-crop.png".to_string()],
            original_crop_refs: vec!["old-crop.png".to_string()],
            preprocessed_crop_refs: vec![],
            model_input_crop_ref: Some("old-crop.png".to_string()),
            preprocess_mode: Some(OcrImagePreprocessMode::CleanGrayscale),
            preprocess_version: Some(PREPROCESS_VERSION.to_string()),
            preprocess_applied: false,
            preprocess_warnings: vec![],
            preprocess_diagnostics: vec![],
            available_preprocess_variants: PREPROCESS_VARIANTS.to_vec(),
            full_page_preview_refs: vec!["old-page.png".to_string()],
            answer_text: "old".to_string(),
            structured_answer: None,
            confidence: Some(0.8),
            uncertain_spans: vec![],
            suggested_corrections: vec![],
            critical_term_warnings: vec![],
            ocr_semantic_warnings: vec![],
            critical_keyword_uncertain: false,
            status: StudentAnswerOcrStatus::TeacherApproved,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            model_name: Some("gemma".to_string()),
            prompt_version: PROMPT_VERSION.to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            teacher_corrected_text: None,
            teacher_reviewed_at: Some(chrono::Utc::now()),
            parse_diagnostics: None,
            render_diagnostics: None,
        }];
        project.workflow = workflow_engine::evaluate_workflow(&project);
        project_store.save_project(&project).unwrap();

        let app = mock_app();
        let job_manager = Arc::new(JobManager::new());
        let config_service = test_config_service();
        let mut profile = config_service.get_profile(None).unwrap();
        profile.server_path.clear();
        profile.model_path.clear();
        profile.mmproj_path.clear();
        config_service.update_profile(profile).unwrap();
        let runtime_service = ModelRuntimeService::new(
            config_service,
            ModelProcessManager::new_with_state_path(
                test_config_service(),
                Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
                std::env::temp_dir().join(format!("rubrika-model-state-{}.json", Uuid::new_v4())),
            ),
        );
        let service = StudentAnswerOcrService::new(
            project_store.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            runtime_service,
            Arc::new(PdfPreviewService::new(
                project_store.clone(),
                Arc::new(SystemPdfService),
                job_manager.clone(),
            )),
            Arc::new(crate::services::model_input_image_service::ModelInputImageService::default()),
            job_manager.clone(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let started = service
                .start(app.clone(), project.id.clone(), true)
                .await
                .unwrap();
            assert!(started.rerun);
            for _ in 0..100 {
                let snapshot = job_manager.get_job_snapshot(&started.job_id).unwrap();
                if matches!(snapshot.status, JobStatus::Failed | JobStatus::Partial) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let stored_project = service
                .project_store
                .get_project_snapshot(project.id.clone())
                .unwrap();
            assert_eq!(
                stored_project.student_answer_ocr_records[0].answer_text,
                "old"
            );
            assert_eq!(stored_project.student_answer_ocr_generations.len(), 1);
            assert!(matches!(
                stored_project.student_answer_ocr_generations[0].status,
                OcrGenerationStatus::Candidate
                    | OcrGenerationStatus::Failed
                    | OcrGenerationStatus::Stale
                    | OcrGenerationStatus::Interrupted
            ));
        });
    }
}

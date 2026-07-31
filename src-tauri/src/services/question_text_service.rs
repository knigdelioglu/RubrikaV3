use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use uuid::Uuid;

use crate::domain::document::{DocumentRole, PdfPreviewStatus};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{JobKind, JobSnapshot, JobStatus};
use crate::domain::model::{
    ExtractedQuestionCandidate, ModelInputImage, QuestionTextExtractionRequest,
};
use crate::domain::model::{QuestionTextExtractionStatus, QuestionTextSuggestion};
use crate::domain::question::{default_question, TextFieldSource, TextFieldStatus};
use crate::domain::workflow::WorkflowStage;
use crate::jobs::job_manager::JobManager;
use crate::services::document_content_extraction_service::{
    clamp_question_markers, detect_question_markers, normalize_question_detection_text,
    DocumentContentExtractionMethod, DocumentContentExtractionRequest,
    DocumentContentExtractionService, DocumentContentKind,
};
use crate::services::model_gateway::ModelGateway;
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeService, ModelUseCase,
};
use crate::services::pdf_preview_service::PdfPreviewService;
use crate::services::project_store::ProjectStore;
use crate::services::workflow_engine;

#[derive(Clone)]
pub struct QuestionTextService {
    project_store: ProjectStore,
    model_gateway: Arc<dyn ModelGateway>,
    model_runtime_service: ModelRuntimeService,
    pdf_preview_service: Arc<PdfPreviewService>,
    document_content_extraction_service: Arc<DocumentContentExtractionService>,
    job_manager: Arc<JobManager>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionTextSource {
    ExamPdf,
}

struct QuestionTextExtractionRunInput {
    project_id: String,
    document_id: String,
    project_root: String,
    preview_count: u32,
}

struct QuestionTextVisionFallbackRunInput {
    project_id: String,
    expected_question_count: u32,
    target_question_numbers: Vec<u32>,
    model_inputs: Vec<ModelInputImage>,
}

impl QuestionTextService {
    pub fn new(
        project_store: ProjectStore,
        model_gateway: Arc<dyn ModelGateway>,
        model_runtime_service: ModelRuntimeService,
        pdf_preview_service: Arc<PdfPreviewService>,
        document_content_extraction_service: Arc<DocumentContentExtractionService>,
        job_manager: Arc<JobManager>,
    ) -> Self {
        Self {
            project_store,
            model_gateway,
            model_runtime_service,
            pdf_preview_service,
            document_content_extraction_service,
            job_manager,
        }
    }

    pub async fn start_extraction<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        project_id: String,
        document_id: Option<String>,
        source: QuestionTextSource,
    ) -> Result<JobSnapshot, AppError> {
        if source != QuestionTextSource::ExamPdf {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Only exam source PDFs can be used for question text extraction."
                    .to_string(),
                recoverable: true,
                suggested_action: Some("Select the exam source PDF and try again.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let exam_doc = resolve_exam_source_document(&project, document_id.as_deref())?;
        let exam_doc_id = exam_doc.id.clone();
        let page_previews = self
            .pdf_preview_service
            .require_ready_page_previews(&project_id, &exam_doc_id)?;
        let project_root = project.root_path.clone();
        let preview_count = page_previews.len().max(1) as u32;
        drop(project);

        let job = self.job_manager.start_job(
            &app,
            project_id.clone(),
            Some(project_root.clone()),
            JobKind::QuestionTextExtraction,
            preview_count,
            "PDF metni taranıyor...".to_string(),
        )?;

        let mut running_project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        running_project.workflow.current_stage = WorkflowStage::QuestionTextExtractionRunning;
        running_project.workflow.summary.text = Some("PDF metni taranıyor.".to_string());
        self.project_store
            .commit_snapshot_cas(&running_project)
            .map(|_| ())?;

        let service = self.clone();
        let job_id = job.id.clone();
        let app_handle = app.clone();
        let app_handle_for_failure = app.clone();
        let job_id_for_failure = job_id.clone();
        let project_id_for_workflow = project_id.clone();
        tauri::async_runtime::spawn(async move {
            let run_result = service
                .run_text_extraction(
                    app_handle,
                    job_id.clone(),
                    QuestionTextExtractionRunInput {
                        project_id,
                        document_id: exam_doc_id.clone(),
                        project_root,
                        preview_count,
                    },
                )
                .await;
            if let Err(error) = run_result {
                if !matches!(
                    service
                        .job_manager
                        .get_job_snapshot(&job_id_for_failure)
                        .map(|job| job.status),
                    Ok(JobStatus::Failed)
                ) {
                    let _ = service.job_manager.fail(
                        &app_handle_for_failure,
                        &job_id_for_failure,
                        error,
                    );
                }

                // Update project workflow to clear the running state
                if let Ok(mut proj) = service
                    .project_store
                    .get_project_snapshot(project_id_for_workflow)
                {
                    proj.workflow = workflow_engine::evaluate_workflow(&proj);
                    let _ = service.project_store.commit_snapshot_cas(&proj);
                }
            } else {
                // Also update on success just to be safe
                if let Ok(mut proj) = service
                    .project_store
                    .get_project_snapshot(project_id_for_workflow)
                {
                    proj.workflow = workflow_engine::evaluate_workflow(&proj);
                    let _ = service.project_store.commit_snapshot_cas(&proj);
                }
            }
        });

        Ok(job)
    }

    pub async fn start_vision_fallback<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        project_id: String,
    ) -> Result<JobSnapshot, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        let exam_source = project
            .documents
            .iter()
            .find(|document| document.role == DocumentRole::ExamSource)
            .ok_or_else(|| AppError {
                code: AppErrorCode::DocumentNotFound,
                message: "Exam source PDF is missing.".to_string(),
                recoverable: true,
                suggested_action: Some("Upload the original exam PDF first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let page_previews = self
            .pdf_preview_service
            .require_ready_page_previews(&project_id, &exam_source.id)?;
        let project_root = project.root_path.clone();
        let expected_question_count = project.expected_question_count.unwrap_or_else(|| {
            project
                .questions
                .iter()
                .map(|question| question.number)
                .max()
                .unwrap_or(0)
                .max(project.questions.len() as u32)
        });
        let target_question_numbers =
            fallback_target_question_numbers(&project, expected_question_count);
        if expected_question_count == 0 {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Soru sayısı bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Önce PDF metni taramasıyla soru sayısını bulun.".to_string(),
                ),
                technical_details: Some("question coverage could not be inferred".to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if target_question_numbers.is_empty() {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Gemma vision fallback için eksik soru metni yok.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce metin tabanlı sonucu inceleyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        let preview_sources = page_previews
            .iter()
            .map(|preview| (preview.page_number, PathBuf::from(&preview.image_path)))
            .collect::<Vec<_>>();
        let content =
            self.document_content_extraction_service
                .extract(DocumentContentExtractionRequest {
                    project_id: project_id.clone(),
                    project_root: PathBuf::from(&project_root),
                    document_id: exam_source.id.clone(),
                    document_path: exam_source.resolve_path(&project_root)?,
                    kind: DocumentContentKind::ExamSource,
                    expected_question_count: Some(expected_question_count),
                    force_refresh: true,
                    vision_sources: preview_sources,
                })?;
        drop(project);
        let fallback_total = target_question_numbers.len() as u32;

        let job = self.job_manager.start_job(
            &app,
            project_id.clone(),
            Some(project_root.clone()),
            JobKind::QuestionTextExtraction,
            fallback_total,
            "Gemma vision fallback hazırlanıyor...".to_string(),
        )?;

        let mut running_project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        running_project.workflow.current_stage = WorkflowStage::QuestionTextExtractionRunning;
        running_project.workflow.summary.text =
            Some("Gemma vision fallback eksik sorular için çalışıyor.".to_string());
        self.project_store
            .commit_snapshot_cas(&running_project)
            .map(|_| ())?;

        let service = self.clone();
        let job_id = job.id.clone();
        let app_handle = app.clone();
        let app_handle_for_failure = app.clone();
        let job_id_for_failure = job_id.clone();
        let project_id_for_workflow = project_id.clone();
        let target_numbers_for_run = target_question_numbers.clone();
        tauri::async_runtime::spawn(async move {
            let run_result = service
                .run_vision_fallback(
                    app_handle,
                    job_id.clone(),
                    QuestionTextVisionFallbackRunInput {
                        project_id,
                        expected_question_count,
                        target_question_numbers: target_numbers_for_run,
                        model_inputs: content.model_input_images.clone(),
                    },
                )
                .await;
            if let Err(error) = run_result {
                if !matches!(
                    service
                        .job_manager
                        .get_job_snapshot(&job_id_for_failure)
                        .map(|job| job.status),
                    Ok(JobStatus::Failed)
                ) {
                    let _ = service.job_manager.fail(
                        &app_handle_for_failure,
                        &job_id_for_failure,
                        error,
                    );
                }

                if let Ok(mut proj) = service
                    .project_store
                    .get_project_snapshot(project_id_for_workflow)
                {
                    proj.workflow = workflow_engine::evaluate_workflow(&proj);
                    let _ = service.project_store.commit_snapshot_cas(&proj);
                }
            } else if let Ok(mut proj) = service
                .project_store
                .get_project_snapshot(project_id_for_workflow)
            {
                proj.workflow = workflow_engine::evaluate_workflow(&proj);
                let _ = service.project_store.commit_snapshot_cas(&proj);
            }
        });

        Ok(job)
    }

    async fn run_text_extraction<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        job_id: String,
        input: QuestionTextExtractionRunInput,
    ) -> Result<(), AppError> {
        let QuestionTextExtractionRunInput {
            project_id,
            document_id,
            project_root,
            preview_count,
        } = input;

        self.job_manager.set_running(&app, &job_id).ok();

        let cancel_token = self.job_manager.get_cancellation_token(&job_id);
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Soru metni çıkarma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        let project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;

        self.job_manager
            .update_progress(
                &app,
                &job_id,
                0,
                preview_count,
                "PDF metni okunuyor...".to_string(),
            )
            .ok();

        let exam_doc = project
            .documents
            .iter()
            .find(|d| d.id == document_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::DocumentNotFound,
                message: "Exam source PDF is missing.".to_string(),
                recoverable: true,
                suggested_action: Some("Upload the original exam PDF first.".to_string()),
                technical_details: Some(format!("document_id={document_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let pdf_path = exam_doc.resolve_path(&project_root)?;
        let page_previews = self
            .pdf_preview_service
            .require_ready_page_previews(&project_id, &document_id)?;
        let preview_sources = page_previews
            .iter()
            .map(|preview| (preview.page_number, PathBuf::from(&preview.image_path)))
            .collect::<Vec<_>>();
        let content =
            self.document_content_extraction_service
                .extract(DocumentContentExtractionRequest {
                    project_id: project_id.clone(),
                    project_root: PathBuf::from(&project_root),
                    document_id: document_id.clone(),
                    document_path: pdf_path,
                    kind: DocumentContentKind::ExamSource,
                    expected_question_count: Some(preview_count),
                    force_refresh: true,
                    vision_sources: preview_sources.clone(),
                })?;
        let raw_text = content.raw_text.clone().unwrap_or_default();
        let split_text = normalize_question_detection_text(&raw_text);
        let markers = detect_question_markers(&split_text);
        let detected_question_count = markers.keys().copied().max().unwrap_or(0);
        let expected_question_count = project
            .expected_question_count
            .unwrap_or(preview_count.max(detected_question_count));
        if expected_question_count == 0 {
            let error = AppError {
                code: AppErrorCode::QuestionTextExtractionFailed,
                message: "Soru numaraları pdftotext metninde bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Gemma vision fallback'ı çalıştırın veya PDF metnini kontrol edin.".to_string(),
                ),
                technical_details: Some(
                    "pdftotext marker detection returned zero questions".to_string(),
                ),
                correlation_id: Uuid::new_v4().to_string(),
            };
            let _ = self.job_manager.fail(&app, &job_id, error.clone());
            return Err(error);
        }

        let (markers, ignored_question_numbers) =
            clamp_question_markers(markers, Some(expected_question_count));
        let candidates = extract_candidates_from_marker_positions(&split_text, &markers);
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;
        project.expected_question_count = Some(expected_question_count);
        let coverage = apply_extraction_to_project_with_expected(
            &mut project,
            candidates,
            content.warnings.clone(),
            expected_question_count,
        );
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;

        if !coverage.coverage_ok {
            let fallback_targets =
                fallback_target_question_numbers(&project, expected_question_count);
            if fallback_targets.is_empty() {
                self.job_manager
                    .update_progress(
                        &app,
                        &job_id,
                        expected_question_count,
                        expected_question_count,
                        "Soru metinleri kullanıcı onayına hazır.".to_string(),
                    )
                    .ok();
                self.job_manager.succeed(
                    &app,
                    &job_id,
                    Some(serde_json::json!({
                        "deterministic": {
                            "detected": markers.keys().copied().collect::<Vec<_>>(),
                            "missing": coverage.missing_numbers,
                            "contaminated": coverage.contaminated_numbers,
                            "coverageOk": coverage.coverage_ok,
                        },
                        "visionFallback": {
                            "skipped": true,
                            "targetQuestions": [],
                            "attemptedQuestions": [],
                            "succeededQuestions": [],
                            "failedQuestions": [],
                            "calls": 0,
                        },
                        "detectedQuestionCount": detected_question_count,
                        "expectedQuestionCount": expected_question_count,
                        "ignoredQuestionNumbers": ignored_question_numbers,
                        "questionsExtracted": project.questions.iter().filter(|q| q.question_text.status == TextFieldStatus::Suggested).count(),
                        "questionsTotal": project.questions.len(),
                        "coverageOk": coverage.coverage_ok,
                        "missingQuestions": coverage.missing_numbers,
                        "duplicateQuestions": coverage.duplicate_numbers,
                        "contaminatedQuestions": coverage.contaminated_numbers,
                        "coverageWarnings": coverage.warnings,
                        "method": match content.method {
                            DocumentContentExtractionMethod::Cached => "cached",
                            DocumentContentExtractionMethod::PdfToText => "pdftotext",
                            DocumentContentExtractionMethod::VisionFallbackPrepared => "vision_fallback_prepared",
                        },
                        "documentContentMethod": match content.method {
                            DocumentContentExtractionMethod::Cached => "cached",
                            DocumentContentExtractionMethod::PdfToText => "pdftotext",
                            DocumentContentExtractionMethod::VisionFallbackPrepared => "vision_fallback_prepared",
                        },
                        "rawTextLength": content.raw_text_length,
                        "normalizedTextLength": content.normalized_text_length,
                        "visionFallbackNeeded": content.vision_fallback_needed,
                        "contentArtifactDir": content.artifact_dir,
                        "modelInputImageCount": content.model_input_images.len(),
                    })),
                )?;
                return Ok(());
            }
            let mut model_inputs = content.model_input_images.clone();
            if model_inputs.is_empty() && !preview_sources.is_empty() {
                let model_input_kind = crate::domain::model::ModelInputImageKind::QuestionText;
                if let Ok(inputs) = self
                    .document_content_extraction_service
                    .model_input_image_service
                    .prepare_inputs(
                        &PathBuf::from(&project_root),
                        model_input_kind,
                        &document_id,
                        &preview_sources,
                    )
                {
                    model_inputs = inputs;
                }
            }
            self.job_manager
                .update_progress(
                    &app,
                    &job_id,
                    0,
                    fallback_targets.len() as u32,
                    "Soru metinleri kısmi kaldı, otomatik vision fallback çalıştırılıyor..."
                        .to_string(),
                )
                .ok();
            let fallback_input = QuestionTextVisionFallbackRunInput {
                project_id: project_id.clone(),
                expected_question_count,
                target_question_numbers: fallback_targets,
                model_inputs,
            };
            return self.run_vision_fallback(app, job_id, fallback_input).await;
        }

        self.job_manager
            .update_progress(
                &app,
                &job_id,
                expected_question_count,
                expected_question_count,
                "Soru metinleri kullanıcı onayına hazır.".to_string(),
            )
            .ok();

        self.job_manager.succeed(
            &app,
            &job_id,
            Some(serde_json::json!({
                "detectedQuestionCount": detected_question_count,
                "expectedQuestionCount": expected_question_count,
                "ignoredQuestionNumbers": ignored_question_numbers,
                "questionsExtracted": project.questions.iter().filter(|q| q.question_text.status == TextFieldStatus::Suggested).count(),
                "questionsTotal": project.questions.len(),
                "coverageOk": coverage.coverage_ok,
                "missingQuestions": coverage.missing_numbers,
                "duplicateQuestions": coverage.duplicate_numbers,
                "contaminatedQuestions": coverage.contaminated_numbers,
                "coverageWarnings": coverage.warnings,
                "method": match content.method {
                    DocumentContentExtractionMethod::Cached => "cached",
                    DocumentContentExtractionMethod::PdfToText => "pdftotext",
                    DocumentContentExtractionMethod::VisionFallbackPrepared => "vision_fallback_prepared",
                },
                "documentContentMethod": match content.method {
                    DocumentContentExtractionMethod::Cached => "cached",
                    DocumentContentExtractionMethod::PdfToText => "pdftotext",
                    DocumentContentExtractionMethod::VisionFallbackPrepared => "vision_fallback_prepared",
                },
                "rawTextLength": content.raw_text_length,
                "normalizedTextLength": content.normalized_text_length,
                "visionFallbackNeeded": content.vision_fallback_needed,
                "contentArtifactDir": content.artifact_dir,
                "modelInputImageCount": content.model_input_images.len(),
            })),
        )?;
        Ok(())
    }

    async fn run_vision_fallback<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        job_id: String,
        input: QuestionTextVisionFallbackRunInput,
    ) -> Result<(), AppError> {
        let QuestionTextVisionFallbackRunInput {
            project_id,
            expected_question_count,
            target_question_numbers,
            model_inputs: all_prepared_inputs,
        } = input;

        if target_question_numbers
            .iter()
            .any(|number| *number == 0 || *number > expected_question_count)
        {
            let error = AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Gemma vision fallback hedef listesi geçersiz.".to_string(),
                recoverable: false,
                suggested_action: Some("Fallback hedeflerini yeniden hesaplayın.".to_string()),
                technical_details: Some(format!(
                    "expected_question_count={expected_question_count}; targets={target_question_numbers:?}"
                )),
                correlation_id: Uuid::new_v4().to_string(),
            };
            let _ = self.job_manager.fail(&app, &job_id, error.clone());
            return Err(error);
        }
        let target_question_numbers =
            sanitize_target_question_numbers(target_question_numbers, expected_question_count);
        if target_question_numbers.is_empty() {
            let error = AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Gemma vision fallback için hedef soru bulunamadı.".to_string(),
                recoverable: true,
                suggested_action: Some("Önce soru metni ayrımını kontrol edin.".to_string()),
                technical_details: Some(
                    "target_question_numbers was empty after sanitization".to_string(),
                ),
                correlation_id: Uuid::new_v4().to_string(),
            };
            let _ = self.job_manager.fail(&app, &job_id, error.clone());
            return Err(error);
        }

        let fallback_total = target_question_numbers.len() as u32;
        self.job_manager.set_running(&app, &job_id).ok();

        let cancel_token = self.job_manager.get_cancellation_token(&job_id);
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Soru metni çıkarma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }
        self.job_manager
            .update_progress(
                &app,
                &job_id,
                0,
                fallback_total,
                "Gemma vision fallback çalışıyor...".to_string(),
            )
            .ok();

        let _runtime_lease = self
            .model_runtime_service
            .acquire_runtime(
                None,
                &ModelRuntimeRequest {
                    use_case: ModelUseCase::QuestionTextExtraction,
                    capability: ModelCapability::Vision,
                    requires_mmproj: true,
                    timeout_seconds: 180,
                },
                "question_text_extraction",
                Some(&job_id),
            )
            .await?;

        let fallback_image_path = all_prepared_inputs
            .first()
            .map(|input| input.output_image_path.clone())
            .unwrap_or_default();

        let mut merged: BTreeMap<u32, ExtractedQuestionCandidate> = BTreeMap::new();
        let mut page_warnings = Vec::new();
        let mut successful_questions = 0u32;
        let mut attempted_questions = Vec::new();
        let mut failed_questions = Vec::new();
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.clone())?;

        let cancel_token = self.job_manager.get_cancellation_token(&job_id);

        for (index, question_number) in target_question_numbers.iter().copied().enumerate() {
            if let Some(ref token) = cancel_token {
                if token.is_cancelled() {
                    let _ = self.job_manager.mark_cancelled(&app, &job_id);
                    return Err(AppError {
                        code: AppErrorCode::JobCancelled,
                        message: "Soru metni çıkarma işlemi iptal edildi.".to_string(),
                        recoverable: true,
                        suggested_action: None,
                        technical_details: None,
                        correlation_id: Uuid::new_v4().to_string(),
                    });
                }
            }

            attempted_questions.push(question_number);
            self.job_manager
                .update_progress(
                    &app,
                    &job_id,
                    index as u32 + 1,
                    fallback_total,
                    format!(
                        "Gemma vision hedefi {}/{} — Soru {} tamamlanıyor...",
                        index + 1,
                        fallback_total,
                        question_number
                    ),
                )
                .ok();

            let request = QuestionTextExtractionRequest {
                prompt: build_question_text_prompt(question_number, expected_question_count),
                image_path: fallback_image_path.clone(),
                page_index: question_number,
                page_count: expected_question_count,
                target_question_number: question_number,
                model_input_images: all_prepared_inputs.clone(),
            };

            match self.model_gateway.extract_question_text(request).await {
                Ok(result) => {
                    successful_questions += 1;
                    page_warnings.extend(result.output.page_warnings.clone());
                    persist_raw_response(
                        &self.project_store,
                        &project_id,
                        &job_id,
                        question_number,
                        &result.raw_response,
                    )?;
                    merge_candidates(
                        &mut merged,
                        result
                            .output
                            .questions
                            .into_iter()
                            .filter(|candidate| candidate.number == question_number)
                            .collect(),
                    );
                }
                Err(mut error) => {
                    if matches!(
                        error.code,
                        AppErrorCode::ModelResponseEmpty
                            | AppErrorCode::ModelResponseInvalidJson
                            | AppErrorCode::ModelResponseInvalidSchema
                            | AppErrorCode::ModelResponseReasoningOnly
                    ) {
                        page_warnings.push(error.message.clone());
                        failed_questions.push(question_number);
                        continue;
                    }

                    let old_details = error.technical_details.unwrap_or_default();
                    error.technical_details = Some(format!(
                        "Question: {question_number}/{expected_question_count}\n{old_details}"
                    ));
                    let _ = self.job_manager.fail(&app, &job_id, error.clone());
                    return Err(error);
                }
            }
        }

        if successful_questions == 0 {
            let error = AppError {
                code: AppErrorCode::QuestionTextExtractionFailed,
                message: "Gemma vision fallback soru metni çıkaramadı.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Model çıktısını ve PDF sayfalarını kontrol edin.".to_string(),
                ),
                technical_details: Some("No page produced a valid extraction result.".to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            };
            let _ = self.job_manager.fail(&app, &job_id, error.clone());
            return Err(error);
        }

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Soru metni çıkarma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        let coverage = apply_extraction_to_project_with_expected(
            &mut project,
            merged.into_values().collect::<Vec<_>>(),
            page_warnings,
            expected_question_count,
        );
        let succeeded_questions = coverage
            .normalized_candidates
            .iter()
            .map(|candidate| candidate.number)
            .collect::<Vec<_>>();
        if project.expected_question_count.is_none() {
            project.expected_question_count = Some(expected_question_count);
        }
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;

        self.job_manager
            .update_progress(
                &app,
                &job_id,
                fallback_total,
                fallback_total,
                "Gemma vision fallback tamamlandı.".to_string(),
            )
            .ok();

        self.job_manager.succeed(
            &app,
            &job_id,
            Some(serde_json::json!({
                "deterministic": {
                    "detected": project.questions.iter().map(|q| q.number).collect::<Vec<_>>(),
                    "missing": coverage.missing_numbers,
                    "contaminated": coverage.contaminated_numbers,
                    "coverageOk": coverage.coverage_ok,
                },
                "visionFallback": {
                    "skipped": false,
                    "targetQuestions": target_question_numbers,
                    "attemptedQuestions": attempted_questions,
                    "succeededQuestions": succeeded_questions,
                    "failedQuestions": failed_questions,
                    "calls": attempted_questions.len(),
                },
                "questionsExtracted": project.questions.iter().filter(|q| q.question_text.status == TextFieldStatus::Suggested).count(),
                "questionsTotal": project.questions.len(),
                "questionsSucceeded": successful_questions,
                "expectedQuestionCount": expected_question_count,
                "coverageOk": coverage.coverage_ok,
                "missingQuestions": coverage.missing_numbers,
                "duplicateQuestions": coverage.duplicate_numbers,
                "contaminatedQuestions": coverage.contaminated_numbers,
                "coverageWarnings": coverage.warnings,
                "method": "gemma_vision_fallback",
            })),
        )?;
        Ok(())
    }

    pub fn confirm_question_text(
        &self,
        project_id: &str,
        question_id: &str,
    ) -> Result<crate::domain::question::Question, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let question = project
            .questions
            .iter_mut()
            .find(|q| q.id == question_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::QuestionTextSuggestionNotFound,
                message: "Question not found.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        if question.question_text.value.trim().is_empty() {
            return Err(AppError {
                code: AppErrorCode::QuestionTextConfirmFailed,
                message: "Question text cannot be empty.".to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Run extraction again or edit the question text.".to_string(),
                ),
                technical_details: Some(format!("question_id={question_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        question.question_text.status = TextFieldStatus::Confirmed;
        question.question_text.source = TextFieldSource::ExamPdf;
        question.question_text.updated_at = Some(now);
        let updated = question.clone();
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub fn confirm_all_question_texts(
        &self,
        project_id: &str,
    ) -> Result<crate::domain::project::Project, AppError> {
        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let mut any_changed = false;
        let mut has_blocking_missing = false;
        let now = chrono::Utc::now().to_rfc3339();
        for question in &mut project.questions {
            match question.question_text.status {
                TextFieldStatus::Missing | TextFieldStatus::Failed => {
                    has_blocking_missing = true;
                }
                TextFieldStatus::Suggested => {
                    question.question_text.status = TextFieldStatus::Confirmed;
                    question.question_text.source = TextFieldSource::ExamPdf;
                    question.question_text.updated_at = Some(now.clone());
                    any_changed = true;
                }
                TextFieldStatus::Edited | TextFieldStatus::Confirmed => {
                    any_changed = true;
                }
            }
        }
        if has_blocking_missing {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "No suggested question texts are available to confirm.".to_string(),
                recoverable: true,
                suggested_action: Some("Run question text extraction first.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        if !any_changed {
            return Err(AppError {
                code: AppErrorCode::QuestionTextConfirmFailed,
                message: "Question texts are already confirmed.".to_string(),
                recoverable: true,
                suggested_action: Some("Review the suggestion list.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(project)
    }

    pub fn edit_question_text(
        &self,
        project_id: &str,
        question_id: &str,
        text: String,
    ) -> Result<crate::domain::question::Question, AppError> {
        if text.trim().is_empty() {
            return Err(AppError {
                code: AppErrorCode::QuestionTextMissing,
                message: "Question text cannot be empty.".to_string(),
                recoverable: true,
                suggested_action: Some("Enter a question text and try again.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let question = project
            .questions
            .iter_mut()
            .find(|q| q.id == question_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::QuestionTextSuggestionNotFound,
                message: "Question not found.".to_string(),
                recoverable: false,
                suggested_action: None,
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            })?;

        question.question_text.value = text;
        question.question_text.status = TextFieldStatus::Edited;
        question.question_text.source = if question.question_text.source == TextFieldSource::Unknown
        {
            TextFieldSource::Manual
        } else {
            question.question_text.source.clone()
        };
        question.question_text.updated_at = Some(now);

        let updated = question.clone();
        project.invalidate_exam_package_if_frozen("package_changed_after_freeze");
        project.workflow = workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(updated)
    }

    pub async fn get_extraction_status(
        &self,
        project_id: &str,
    ) -> Result<QuestionTextExtractionStatus, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;

        let model_status = self
            .model_runtime_service
            .get_model_status(None)
            .await
            .unwrap_or_default();

        let jobs = self.job_manager.list_jobs(project_id).unwrap_or_default();
        let question_text_job_active = workflow_engine::has_active_question_text_job(&jobs);

        let workflow = workflow_engine::evaluate_workflow_with_context(
            &project,
            &model_status,
            question_text_job_active,
            false,
        );

        let exam_source = project
            .documents
            .iter()
            .find(|document| document.role == DocumentRole::ExamSource);
        let preview_ready = if let Some(document) = exam_source {
            self.pdf_preview_service
                .require_ready_page_previews(&project.id, &document.id)
                .is_ok()
        } else {
            false
        };
        let preview_status = if let Some(document) = exam_source {
            match self
                .pdf_preview_service
                .get_pdf_preview_status(&project.id, &document.id)
            {
                Ok(snapshot) => snapshot.status,
                Err(_) => document
                    .preview
                    .as_ref()
                    .map(|preview| preview.status.clone())
                    .unwrap_or(PdfPreviewStatus::Missing),
            }
        } else {
            PdfPreviewStatus::Missing
        };
        let preview_status = match preview_status {
            PdfPreviewStatus::Missing => "missing",
            PdfPreviewStatus::Queued => "queued",
            PdfPreviewStatus::Running => "running",
            PdfPreviewStatus::Ready => "ready",
            PdfPreviewStatus::Failed => "failed",
        }
        .to_string();

        let suggested_count = project
            .questions
            .iter()
            .filter(|question| question.question_text.status == TextFieldStatus::Suggested)
            .count() as u32;
        let confirmed_count = project
            .questions
            .iter()
            .filter(|question| {
                matches!(
                    question.question_text.status,
                    TextFieldStatus::Confirmed | TextFieldStatus::Edited
                )
            })
            .count() as u32;
        let missing_count = project
            .questions
            .iter()
            .filter(|question| {
                matches!(
                    question.question_text.status,
                    TextFieldStatus::Missing | TextFieldStatus::Failed
                )
            })
            .count() as u32;
        let missing_question_numbers = project
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
        let latest_job = jobs
            .iter()
            .filter(|job| job.kind == JobKind::QuestionTextExtraction)
            .max_by_key(|job| job.updated_at.clone());
        let latest_job_copied = latest_job.cloned();
        let latest_job_result = latest_job_copied
            .as_ref()
            .and_then(|job| job.result.as_ref());
        let detected_question_count = latest_job_result
            .and_then(|result| result.get("detectedQuestionCount"))
            .and_then(|value| value.as_u64())
            .map(|value| value as u32);
        let coverage_ok = latest_job_result
            .and_then(|result| result.get("coverageOk"))
            .and_then(|value| value.as_bool())
            .unwrap_or(missing_count == 0);
        let extraction_method = latest_job_result
            .and_then(|result| result.get("method"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        Ok(QuestionTextExtractionStatus {
            project_id: project.id,
            document_id: exam_source.map(|document| document.id.clone()),
            preview_status,
            preview_ready,
            current_stage: workflow_key(&workflow.current_stage),
            blocking_reasons: workflow.blocking_reasons.iter().map(workflow_key).collect(),
            next_actions: workflow
                .next_actions
                .iter()
                .map(|action| action.code.clone())
                .collect(),
            detected_question_count,
            suggested_count,
            confirmed_count,
            missing_count,
            missing_question_numbers,
            coverage_ok,
            extraction_method,
            vision_fallback_available: missing_count > 0,
            running_job_id: latest_job_copied.as_ref().map(|job| job.id.clone()),
            latest_job_status: latest_job_copied
                .as_ref()
                .map(|job| workflow_key(&job.status)),
            summary: workflow.summary.text.clone(),
        })
    }

    pub fn list_suggestions(
        &self,
        project_id: &str,
    ) -> Result<Vec<QuestionTextSuggestion>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let mut suggestions = Vec::new();

        for question in project.questions {
            suggestions.push(QuestionTextSuggestion {
                question_id: question.id,
                number: question.number,
                text: question.question_text.value,
                confidence: question.question_text.confidence.unwrap_or(0.0),
                source: "exam_pdf".to_string(),
                status: match question.question_text.status {
                    TextFieldStatus::Missing => "missing",
                    TextFieldStatus::Suggested => "suggested",
                    TextFieldStatus::Confirmed => "confirmed",
                    TextFieldStatus::Edited => "edited",
                    TextFieldStatus::Failed => "failed",
                }
                .to_string(),
                warnings: question.question_text.warnings,
            });
        }

        suggestions.sort_by_key(|item| item.number);
        Ok(suggestions)
    }
}

fn workflow_key<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|json| json.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn resolve_exam_source_document<'a>(
    project: &'a crate::domain::project::Project,
    document_id: Option<&str>,
) -> Result<&'a crate::domain::document::Document, AppError> {
    if let Some(document_id) = document_id {
        return project
            .documents
            .iter()
            .find(|document| {
                document.id == document_id && document.role == DocumentRole::ExamSource
            })
            .ok_or_else(|| AppError {
                code: AppErrorCode::DocumentNotFound,
                message: "Exam source PDF is missing.".to_string(),
                recoverable: true,
                suggested_action: Some("Upload the original exam PDF first.".to_string()),
                technical_details: Some(format!("document_id={document_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            });
    }

    project
        .documents
        .iter()
        .find(|document| document.role == DocumentRole::ExamSource)
        .ok_or_else(|| AppError {
            code: AppErrorCode::DocumentNotFound,
            message: "Exam source PDF is missing.".to_string(),
            recoverable: true,
            suggested_action: Some("Upload the original exam PDF first.".to_string()),
            technical_details: None,
            correlation_id: Uuid::new_v4().to_string(),
        })
}

fn build_question_text_prompt(question_number: u32, expected_question_count: u32) -> String {
    [
        "You are extracting printed exam question stems from a source exam PDF image.",
        "Return strict JSON only.",
        &format!(
            "Extract only question number {question_number} of {expected_question_count}."
        ),
        "Do not return any other question.",
        "Do not include answer spaces, student handwriting, blank lines, or rubric text.",
        "If a question spans pages, capture the full stem you can see and include warnings.",
        "Do not wrap the answer in markdown fences.",
        &format!(
            r#"Return exactly: {{"questions":[{{"number":{question_number},"question_text":"...","confidence":0.92,"warnings":[]}}],"page_warnings":[]}}"#
        ),
    ]
    .join(" ")
}

fn extract_candidates_from_marker_positions(
    text: &str,
    markers: &BTreeMap<u32, usize>,
) -> Vec<ExtractedQuestionCandidate> {
    let mut positions: Vec<(u32, usize)> = markers
        .iter()
        .map(|(number, start)| (*number, *start))
        .collect();
    positions.sort_by_key(|(_, start)| *start);

    let mut candidates = Vec::new();
    for (index, (number, start)) in positions.iter().enumerate() {
        let end = positions
            .get(index + 1)
            .map(|(_, next_start)| *next_start)
            .unwrap_or(text.len());
        let mut question_text = text[*start..end]
            .replace('\u{c}', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(footer_start) = question_text.find("Türk Dili ve Edebiyatı Zümresi") {
            if *number
                == positions
                    .last()
                    .map(|(last_number, _)| *last_number)
                    .unwrap_or(*number)
            {
                question_text.truncate(footer_start);
            }
        }
        if let Some(footer_start) = question_text.find("BAŞARILAR") {
            if *number
                == positions
                    .last()
                    .map(|(last_number, _)| *last_number)
                    .unwrap_or(*number)
            {
                question_text.truncate(footer_start);
            }
        }
        question_text = question_text.trim().to_string();
        if question_text.is_empty() {
            question_text = format!("S{number}");
        }
        candidates.push(ExtractedQuestionCandidate {
            number: *number,
            question_text,
            confidence: 1.0,
            warnings: vec!["pdftotext_auto_detected".to_string()],
        });
    }

    candidates
}

pub(crate) fn extract_numbered_questions_from_text(
    text: &str,
    expected_question_count: u32,
) -> Option<Vec<ExtractedQuestionCandidate>> {
    let normalized = normalize_question_detection_text(text);
    let markers = detect_question_markers(&normalized);
    if markers.is_empty() {
        return None;
    }
    let (markers, _) = clamp_question_markers(markers, Some(expected_question_count));
    if markers.is_empty() {
        return None;
    }
    let candidates = extract_candidates_from_marker_positions(&normalized, &markers);
    let max_number = markers
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .max(expected_question_count);
    let report = validate_question_coverage(max_number, candidates);
    if report.normalized_candidates.is_empty() {
        None
    } else {
        Some(report.normalized_candidates)
    }
}

fn merge_candidates(
    merged: &mut BTreeMap<u32, ExtractedQuestionCandidate>,
    candidates: Vec<ExtractedQuestionCandidate>,
) {
    for candidate in candidates {
        if candidate.number == 0 {
            continue;
        }

        match merged.get_mut(&candidate.number) {
            Some(existing) => {
                let is_better = candidate.question_text.len() > existing.question_text.len()
                    || candidate.confidence > existing.confidence;
                if is_better {
                    let mut warnings = existing.warnings.clone();
                    warnings.extend(candidate.warnings.clone());
                    warnings.push("duplicate_question_candidate".to_string());
                    *existing = ExtractedQuestionCandidate {
                        warnings,
                        ..candidate
                    };
                } else {
                    existing
                        .warnings
                        .push("duplicate_question_candidate".to_string());
                }
            }
            None => {
                merged.insert(candidate.number, candidate);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct QuestionCoverageValidationResult {
    normalized_candidates: Vec<ExtractedQuestionCandidate>,
    missing_numbers: Vec<u32>,
    duplicate_numbers: Vec<u32>,
    contaminated_numbers: Vec<u32>,
    warnings: Vec<String>,
    pub(crate) coverage_ok: bool,
}

fn sanitize_target_question_numbers(
    target_question_numbers: Vec<u32>,
    expected_question_count: u32,
) -> Vec<u32> {
    let mut targets = target_question_numbers
        .into_iter()
        .filter(|number| (1..=expected_question_count).contains(number))
        .collect::<Vec<_>>();
    targets.sort_unstable();
    targets.dedup();
    targets
}

fn contaminated_marker_numbers(
    question_text: &str,
    question_number: u32,
    expected_question_count: u32,
) -> Vec<u32> {
    detect_question_markers(question_text)
        .into_keys()
        .filter(|number| *number != question_number && *number <= expected_question_count)
        .collect()
}

fn question_text_is_contaminated(
    question_text: &str,
    question_number: u32,
    expected_question_count: u32,
) -> bool {
    !contaminated_marker_numbers(question_text, question_number, expected_question_count).is_empty()
}

fn fallback_target_question_numbers(
    project: &crate::domain::project::Project,
    expected_question_count: u32,
) -> Vec<u32> {
    let mut target_question_numbers = Vec::new();
    for question_number in 1..=expected_question_count {
        let needs_fallback = project
            .questions
            .iter()
            .find(|question| question.number == question_number)
            .map(|question| match question.question_text.status {
                TextFieldStatus::Missing | TextFieldStatus::Failed => true,
                TextFieldStatus::Suggested => question_text_is_contaminated(
                    &question.question_text.value,
                    question.number,
                    expected_question_count,
                ),
                TextFieldStatus::Confirmed | TextFieldStatus::Edited => false,
            })
            .unwrap_or(true);
        if needs_fallback {
            target_question_numbers.push(question_number);
        }
    }
    target_question_numbers
}

fn validate_question_coverage(
    expected_count: u32,
    extracted: Vec<ExtractedQuestionCandidate>,
) -> QuestionCoverageValidationResult {
    let mut merged: BTreeMap<u32, ExtractedQuestionCandidate> = BTreeMap::new();
    let mut duplicate_numbers = Vec::new();

    for candidate in extracted {
        if candidate.number == 0 {
            continue;
        }

        if let Some(existing) = merged.get_mut(&candidate.number) {
            duplicate_numbers.push(candidate.number);
            let is_better = candidate.question_text.len() > existing.question_text.len()
                || candidate.confidence > existing.confidence;
            if is_better {
                let mut warnings = existing.warnings.clone();
                warnings.extend(candidate.warnings.clone());
                warnings.push("duplicate_question_candidate".to_string());
                *existing = ExtractedQuestionCandidate {
                    warnings,
                    ..candidate
                };
            } else {
                existing
                    .warnings
                    .push("duplicate_question_candidate".to_string());
            }
        } else {
            merged.insert(candidate.number, candidate);
        }
    }

    let contaminated_numbers = merged
        .iter()
        .filter_map(|(number, candidate)| {
            if contaminated_marker_numbers(&candidate.question_text, *number, expected_count)
                .is_empty()
            {
                None
            } else {
                Some(*number)
            }
        })
        .collect::<Vec<_>>();

    let mut missing_numbers = Vec::new();
    let max_expected = expected_count.max(merged.keys().copied().max().unwrap_or(0));
    for number in 1..=max_expected {
        if !merged.contains_key(&number) {
            missing_numbers.push(number);
        }
    }

    let mut warnings = Vec::new();
    if !missing_numbers.is_empty() {
        warnings.push(format!(
            "QUESTION_COVERAGE_INCOMPLETE: expected {}, found {}, missing {:?}",
            max_expected,
            merged.len(),
            missing_numbers
        ));
        if missing_numbers.contains(&max_expected) {
            warnings.push(format!(
                "QUESTION_LAST_ITEM_MISSING: question {} missing",
                max_expected
            ));
        }
    }
    if !duplicate_numbers.is_empty() {
        warnings.push(format!(
            "duplicate_question_numbers={:?}",
            duplicate_numbers
        ));
    }
    if !contaminated_numbers.is_empty() {
        warnings.push(format!(
            "question_contamination_detected={:?}",
            contaminated_numbers
        ));
    }

    QuestionCoverageValidationResult {
        normalized_candidates: merged.into_values().collect(),
        missing_numbers,
        duplicate_numbers,
        contaminated_numbers,
        warnings: warnings.clone(),
        coverage_ok: warnings.is_empty(),
    }
}

#[cfg(test)]
fn apply_extraction_to_project(
    project: &mut crate::domain::project::Project,
    candidates: Vec<ExtractedQuestionCandidate>,
    page_warnings: Vec<String>,
    page_count: u32,
) -> QuestionCoverageValidationResult {
    let current_question_count = project.questions.len() as u32;
    let extracted_max = candidates
        .iter()
        .map(|candidate| candidate.number)
        .max()
        .unwrap_or(0);
    let layout_hint_count = page_count.saturating_mul(3);
    let expected_count = current_question_count
        .max(extracted_max)
        .max(layout_hint_count);
    apply_extraction_to_project_with_expected(project, candidates, page_warnings, expected_count)
}

pub(crate) fn apply_extraction_to_project_with_expected(
    project: &mut crate::domain::project::Project,
    candidates: Vec<ExtractedQuestionCandidate>,
    page_warnings: Vec<String>,
    expected_count: u32,
) -> QuestionCoverageValidationResult {
    let coverage = validate_question_coverage(expected_count, candidates);
    let normalized_candidates = coverage.normalized_candidates.clone();
    let mut by_number: BTreeMap<u32, ExtractedQuestionCandidate> = BTreeMap::new();
    merge_candidates(&mut by_number, normalized_candidates);
    let contaminated_numbers = coverage
        .contaminated_numbers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();

    project.questions.retain(|question| {
        question.number <= expected_count
            || matches!(
                question.question_text.status,
                TextFieldStatus::Confirmed | TextFieldStatus::Edited
            )
    });

    if project.questions.is_empty() && expected_count > 0 {
        for number in 1..=expected_count {
            project.questions.push(default_question(number));
        }
    }

    if project.questions.len() < expected_count as usize {
        let start = project.questions.len() as u32 + 1;
        for number in start..=expected_count {
            project.questions.push(default_question(number));
        }
    }

    for question in &mut project.questions {
        if let Some(candidate) = by_number.get(&question.number) {
            let contaminated = contaminated_numbers.contains(&question.number);
            let existing_contaminated =
                matches!(question.question_text.status, TextFieldStatus::Suggested)
                    && question_text_is_contaminated(
                        &question.question_text.value,
                        question.number,
                        expected_count,
                    );
            if !matches!(
                question.question_text.status,
                TextFieldStatus::Missing | TextFieldStatus::Failed
            ) && !existing_contaminated
                && !contaminated
            {
                continue;
            }
            question.question_text.value = candidate.question_text.clone();
            question.question_text.source = TextFieldSource::ExamPdf;
            question.question_text.status = if contaminated {
                TextFieldStatus::Failed
            } else {
                TextFieldStatus::Suggested
            };
            question.question_text.confidence = Some(candidate.confidence);
            let mut warnings = candidate.warnings.clone();
            warnings.extend(page_warnings.clone());
            if contaminated {
                warnings.push("question_split_contaminated".to_string());
            }
            question.question_text.warnings = warnings;
            question.question_text.updated_at = Some(chrono::Utc::now().to_rfc3339());
        } else if question.number <= expected_count {
            let mut warnings = question.question_text.warnings.clone();
            warnings.extend(page_warnings.clone());
            warnings.push(format!(
                "QUESTION_LAST_ITEM_MISSING: question {} missing",
                question.number
            ));
            if question_text_is_contaminated(
                &question.question_text.value,
                question.number,
                expected_count,
            ) {
                warnings.push("question_split_contaminated".to_string());
                question.question_text.status = TextFieldStatus::Failed;
            }
            question.question_text.warnings = warnings;
        }
    }

    for candidate in by_number.values() {
        if !project
            .questions
            .iter()
            .any(|q| q.number == candidate.number)
        {
            let mut question = default_question(candidate.number);
            question.question_text.value = candidate.question_text.clone();
            question.question_text.source = TextFieldSource::ExamPdf;
            question.question_text.status = TextFieldStatus::Suggested;
            question.question_text.confidence = Some(candidate.confidence);
            let mut warnings = candidate.warnings.clone();
            warnings.extend(page_warnings.clone());
            question.question_text.warnings = warnings;
            question.question_text.updated_at = Some(chrono::Utc::now().to_rfc3339());
            project.questions.push(question);
        }
    }

    project.questions.sort_by_key(|q| q.number);
    coverage
}

fn persist_raw_response(
    project_store: &ProjectStore,
    project_id: &str,
    job_id: &str,
    page_index: u32,
    raw_response: &str,
) -> Result<(), AppError> {
    let project = project_store.get_project_snapshot(project_id.to_string())?;
    let trusted_root = project_store.trusted_project_root(&project.id)?;
    let raw_dir = trusted_root
        .root()
        .join(trusted_root.managed("cache/model_raw")?.as_path());
    trusted_root.ensure_managed_directory(&raw_dir)?;
    let file_relative = trusted_root.managed(&format!(
        "cache/model_raw/question_text_{job_id}_page_{page_index}.json"
    ))?;
    let file_path = trusted_root.root().join(file_relative.as_path());
    std::fs::write(&file_path, raw_response).map_err(|e| AppError {
        code: AppErrorCode::ProjectSaveFailed,
        message: "Failed to write raw model response.".to_string(),
        recoverable: false,
        suggested_action: Some("Check project cache permissions.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, DocumentRole};
    use crate::domain::project::{ExamPackageFreeze, ExamPackageFreezeStatus, Project};
    use crate::domain::question::{AnswerType, Question, TextFieldSource, TextFieldState};
    use crate::domain::rubric::{RubricState, RubricStatus};
    use crate::domain::workflow::{WorkflowSnapshot, WorkflowStage};
    use crate::jobs::job_manager::JobManager;
    use crate::services::document_content_extraction_service::DocumentContentExtractionService;
    use crate::services::llama_server_gateway::LlamaServerGateway;
    use crate::services::model_input_image_service::ModelInputImageService;
    use crate::services::pdf_preview_service::PdfPreviewService;
    use crate::services::pdf_service::SystemPdfService;
    use crate::services::project_store::ProjectStore;
    use crate::services::workflow_engine;

    fn temp_project_root() -> String {
        let root = std::env::temp_dir().join(format!("rubrika-v3-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().to_string()
    }

    fn service_for_tests(project_store: ProjectStore) -> QuestionTextService {
        service_for_tests_with_job_manager(project_store, std::sync::Arc::new(JobManager::new()))
    }

    fn service_for_tests_with_job_manager(
        project_store: ProjectStore,
        job_manager: std::sync::Arc<JobManager>,
    ) -> QuestionTextService {
        let pdf_preview_service = std::sync::Arc::new(PdfPreviewService::new(
            project_store.clone(),
            std::sync::Arc::new(SystemPdfService),
            job_manager.clone(),
        ));
        let model_gateway_impl = std::sync::Arc::new(LlamaServerGateway::default());
        let model_config = crate::services::model_config_service::ModelConfigService::new();
        let model_process_manager =
            crate::services::model_process_manager::ModelProcessManager::new(
                model_config.clone(),
                model_gateway_impl.clone(),
            );
        let model_runtime_service =
            crate::services::model_runtime_service::ModelRuntimeService::new(
                model_config,
                model_process_manager,
            );
        let model_input_image_service = std::sync::Arc::new(ModelInputImageService::default());
        let document_content_extraction_service = std::sync::Arc::new(
            DocumentContentExtractionService::new(model_input_image_service),
        );
        QuestionTextService::new(
            project_store,
            model_gateway_impl,
            model_runtime_service,
            pdf_preview_service,
            document_content_extraction_service,
            job_manager,
        )
    }

    #[test]
    fn prompt_contains_json_schema() {
        let prompt = build_question_text_prompt(1, 6);
        assert!(prompt.contains(r#""questions""#));
        assert!(prompt.contains("strict JSON"));
        assert!(prompt.contains("question number 1"));
    }

    #[test]
    fn edit_question_text_after_freeze_invalidates_package() {
        let store = ProjectStore::new();
        let service = service_for_tests(store.clone());
        let mut project = store
            .create_project("Project".to_string(), temp_project_root())
            .expect("project");
        let mut question = default_question(1);
        question.question_text.value = "Eski soru".to_string();
        question.question_text.status = TextFieldStatus::Confirmed;
        question.question_text.source = TextFieldSource::Manual;
        let question_id = question.id.clone();
        project.questions = vec![question];
        project.exam_package_freeze = Some(ExamPackageFreeze {
            exam_package_version: 1,
            freeze_status: ExamPackageFreezeStatus::Frozen,
            frozen_at: "now".to_string(),
            frozen_by: Some("teacher".to_string()),
            source_hash: "source".to_string(),
            rubric_hash: "rubric".to_string(),
            question_text_hash: "question".to_string(),
            invalidated_at: None,
            invalidation_reason: None,
        });
        store.save_project(&project).expect("save");

        service
            .edit_question_text(&project.id, &question_id, "Yeni soru".to_string())
            .expect("edit");

        let updated = store
            .get_project_snapshot(project.id.clone())
            .expect("snapshot");
        let freeze = updated.exam_package_freeze.expect("freeze metadata");
        assert_eq!(freeze.freeze_status, ExamPackageFreezeStatus::Invalidated);
        assert_eq!(
            freeze.invalidation_reason.as_deref(),
            Some("package_changed_after_freeze")
        );
    }

    #[test]
    fn extracts_all_questions_from_text_based_exam_pdf_output() {
        let text = "Başlık\n\
S1. Birinci soru metni. (10 P)\n\
S2. İkinci soru metni. (10 P)\n\
S3.Yaşar: (Hızla yürüyerek gelir.) Ne olur, dinle! Ben bir Türküm...\n\
Yaşar’ın konuşmasında hangi milli değerler öne çıkıyor?(10 P)\n\
\u{c}S4. Aşağıda Nurullah Ataç’ın Mona Lisa'nın Gülüşü Niçin Bu Kadar Özel adlı yazısından bir parça verilmiştir.\n\
Bu parçanın ana düşüncesini yazınız. (10 P)\n\
S5. Aşağıdaki tabloyu doldurunuz. (20 P)\n\
S6.Aşağıdaki cümleleri ögelerine ayırınız. (20 P)\n\
Türk Dili ve Edebiyatı Zümresi\n\
BAŞARILAR...";

        let candidates = extract_numbered_questions_from_text(text, 6).expect("questions");

        assert_eq!(candidates.len(), 6);
        assert_eq!(candidates[0].number, 1);
        assert!(candidates[0].question_text.starts_with("S1."));
        assert_eq!(candidates[2].number, 3);
        assert!(candidates[2].question_text.contains("Yaşar"));
        assert!(!candidates[2].question_text.contains("S4."));
        assert_eq!(candidates[3].number, 4);
        assert!(candidates[3].question_text.starts_with("S4."));
        assert_eq!(candidates[5].number, 6);
        assert!(candidates[5].question_text.contains("ögelerine ayırınız"));
        assert!(!candidates[5].question_text.contains("BAŞARILAR"));
    }

    #[test]
    fn detects_question_count_from_pdf_text_without_manual_input() {
        let text = "S1. Birinci soru metni.\r\nS2. İkinci soru metni.\rS3.Yaşar:\u{c}S4. Dördüncü soru metni.\nS5. Beşinci soru metni.\nS6.Altıncı soru metni.";

        let markers = detect_question_markers(text);
        assert_eq!(markers.keys().copied().max(), Some(6));
        assert_eq!(markers.len(), 6);
    }

    #[test]
    fn validate_question_coverage_marks_contaminated_candidates() {
        let report = validate_question_coverage(
            6,
            vec![
                ExtractedQuestionCandidate {
                    number: 3,
                    question_text: "S3.Yaşar...\nS4. sonraki soru".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 4,
                    question_text: "S4. temiz".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
            ],
        );

        assert_eq!(report.contaminated_numbers, vec![3]);
        assert!(!report.coverage_ok);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("question_contamination_detected")));
    }

    #[test]
    fn fallback_targets_are_clamped_and_deduped() {
        let mut project = Project {
            expected_question_count: Some(6),
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
            questions: vec![
                Question {
                    id: "q1".to_string(),
                    number: 1,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "Question 1".to_string(),
                        source: TextFieldSource::ExamPdf,
                        status: TextFieldStatus::Confirmed,
                        confidence: Some(1.0),
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
                Question {
                    id: "q2".to_string(),
                    number: 2,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "Question 2".to_string(),
                        source: TextFieldSource::ExamPdf,
                        status: TextFieldStatus::Confirmed,
                        confidence: Some(1.0),
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
                Question {
                    id: "q3".to_string(),
                    number: 3,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "S3.Yaşar...\nS4. contamination".to_string(),
                        source: TextFieldSource::ExamPdf,
                        status: TextFieldStatus::Suggested,
                        confidence: Some(0.9),
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
                Question {
                    id: "q4".to_string(),
                    number: 4,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: String::new(),
                        source: TextFieldSource::Unknown,
                        status: TextFieldStatus::Missing,
                        confidence: None,
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
                Question {
                    id: "q5".to_string(),
                    number: 5,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "Question 5".to_string(),
                        source: TextFieldSource::ExamPdf,
                        status: TextFieldStatus::Confirmed,
                        confidence: Some(1.0),
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
                Question {
                    id: "q6".to_string(),
                    number: 6,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "Question 6".to_string(),
                        source: TextFieldSource::ExamPdf,
                        status: TextFieldStatus::Confirmed,
                        confidence: Some(1.0),
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
            ],
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
        project.questions[3].question_text.status = TextFieldStatus::Missing;
        project.questions[4].question_text.status = TextFieldStatus::Confirmed;
        project.questions[5].question_text.status = TextFieldStatus::Confirmed;

        let targets = fallback_target_question_numbers(&project, 6);
        assert_eq!(targets, vec![3, 4]);
        assert_eq!(
            sanitize_target_question_numbers(vec![0, 1, 1, 2, 7, 3, 3], 6),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn merge_prefers_longer_candidate_and_marks_duplicate() {
        let mut merged = BTreeMap::new();
        merge_candidates(
            &mut merged,
            vec![
                ExtractedQuestionCandidate {
                    number: 1,
                    question_text: "Short".to_string(),
                    confidence: 0.6,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 1,
                    question_text: "Longer question text".to_string(),
                    confidence: 0.5,
                    warnings: vec!["page_warning".to_string()],
                },
            ],
        );

        let candidate = merged.get(&1).expect("candidate");
        assert_eq!(candidate.question_text, "Longer question text");
        assert!(candidate
            .warnings
            .iter()
            .any(|w| w == "duplicate_question_candidate"));
    }

    #[test]
    fn apply_extraction_creates_missing_skeletons_and_suggests_values() {
        let mut project = Project {
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
            documents: vec![Document {
                id: "d1".to_string(),
                role: DocumentRole::ExamSource,
                file_name: "exam.pdf".to_string(),
                stored_path: "exam.pdf".to_string(),
                page_count: 2,
                added_at: "now".to_string(),
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
                next_actions: vec![],
                current_stage_label: "Test".to_string(),
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
        };

        apply_extraction_to_project(
            &mut project,
            vec![
                ExtractedQuestionCandidate {
                    number: 1,
                    question_text: "Question 1".to_string(),
                    confidence: 0.95,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 3,
                    question_text: "Question 3".to_string(),
                    confidence: 0.85,
                    warnings: vec![],
                },
            ],
            vec!["page_warning".to_string()],
            2,
        );

        assert_eq!(project.questions.len(), 6);
        assert_eq!(
            project.questions[0].question_text.status,
            TextFieldStatus::Suggested
        );
        assert_eq!(
            project.questions[0].question_text.source,
            TextFieldSource::ExamPdf
        );
        assert_eq!(
            project.questions[1].question_text.status,
            TextFieldStatus::Missing
        );
        assert_eq!(
            project.questions[2].question_text.status,
            TextFieldStatus::Suggested
        );
        assert_eq!(
            project.questions[3].question_text.status,
            TextFieldStatus::Missing
        );
        assert_eq!(
            project.questions[4].question_text.status,
            TextFieldStatus::Missing
        );
        assert_eq!(
            project.questions[5].question_text.status,
            TextFieldStatus::Missing
        );
        assert!(project.questions[0]
            .question_text
            .warnings
            .contains(&"page_warning".to_string()));
    }

    #[test]
    fn apply_extraction_preserves_confirmed_texts() {
        let mut project = Project {
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
            documents: vec![Document {
                id: "d1".to_string(),
                role: DocumentRole::ExamSource,
                file_name: "exam.pdf".to_string(),
                stored_path: "exam.pdf".to_string(),
                page_count: 2,
                added_at: "now".to_string(),
                checksum: None,
                preview: None,
            }],
            questions: vec![
                Question {
                    id: "q1".to_string(),
                    number: 1,
                    max_score: 0.0,
                    answer_type: AnswerType::GeneralText,
                    question_text: TextFieldState {
                        value: "Confirmed text".to_string(),
                        source: TextFieldSource::Manual,
                        status: TextFieldStatus::Confirmed,
                        confidence: None,
                        warnings: vec![],
                        updated_at: None,
                    },
                    rubric: RubricState {
                        status: RubricStatus::Missing,
                        source: None,
                        max_score: None,
                        expected_answer: None,
                        criteria: vec![],
                        partial_credit_hints: vec![],
                        zero_score_conditions: vec![],
                        common_mistakes: vec![],
                        warnings: vec![],
                        updated_at: None,
                    },
                    crop_template: None,
                },
                default_question(2),
            ],
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

        apply_extraction_to_project_with_expected(
            &mut project,
            vec![
                ExtractedQuestionCandidate {
                    number: 1,
                    question_text: "Overwritten".to_string(),
                    confidence: 0.95,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 2,
                    question_text: "Question 2".to_string(),
                    confidence: 0.95,
                    warnings: vec![],
                },
            ],
            vec![],
            2,
        );

        assert_eq!(project.questions[0].question_text.value, "Confirmed text");
        assert_eq!(
            project.questions[0].question_text.status,
            TextFieldStatus::Confirmed
        );
        assert_eq!(
            project.questions[1].question_text.status,
            TextFieldStatus::Suggested
        );
    }

    #[test]
    fn validate_question_coverage_marks_missing_last_item() {
        let report = validate_question_coverage(
            6,
            vec![
                ExtractedQuestionCandidate {
                    number: 1,
                    question_text: "Question 1".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 2,
                    question_text: "Question 2".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 3,
                    question_text: "Question 3".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 4,
                    question_text: "Question 4".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 5,
                    question_text: "Question 5".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
            ],
        );

        assert_eq!(report.missing_numbers, vec![6]);
        assert!(!report.coverage_ok);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("QUESTION_LAST_ITEM_MISSING")));
    }

    #[test]
    fn validate_question_coverage_keeps_complete_sequence_clean() {
        let report = validate_question_coverage(
            6,
            (1..=6)
                .map(|number| ExtractedQuestionCandidate {
                    number,
                    question_text: format!("Question {number}"),
                    confidence: 0.9,
                    warnings: vec![],
                })
                .collect(),
        );

        assert!(report.coverage_ok);
        assert!(report.missing_numbers.is_empty());
        assert!(report.duplicate_numbers.is_empty());
    }

    #[test]
    fn test_dynamic_question_coverage_scenarios() {
        // Scenario 1: expected_question_count=6, extracted=[1..5] -> missing=[6]
        let candidates1: Vec<ExtractedQuestionCandidate> = (1..=5)
            .map(|number| ExtractedQuestionCandidate {
                number,
                question_text: format!("Question {number}"),
                confidence: 0.9,
                warnings: vec![],
            })
            .collect();
        let report1 = validate_question_coverage(6, candidates1);
        assert_eq!(report1.missing_numbers, vec![6]);
        assert!(!report1.coverage_ok);

        // Scenario 2: expected_question_count=20, extracted=[1..19] -> missing=[20]
        let candidates2: Vec<ExtractedQuestionCandidate> = (1..=19)
            .map(|number| ExtractedQuestionCandidate {
                number,
                question_text: format!("Question {number}"),
                confidence: 0.9,
                warnings: vec![],
            })
            .collect();
        let report2 = validate_question_coverage(20, candidates2);
        assert_eq!(report2.missing_numbers, vec![20]);
        assert!(!report2.coverage_ok);

        // Scenario 3: expected_question_count=20, extracted=[1,2,3,10,20] -> missing the rest
        let extracted_numbers = [1, 2, 3, 10, 20];
        let candidates3: Vec<ExtractedQuestionCandidate> = extracted_numbers
            .iter()
            .map(|&number| ExtractedQuestionCandidate {
                number,
                question_text: format!("Question {number}"),
                confidence: 0.9,
                warnings: vec![],
            })
            .collect();
        let report3 = validate_question_coverage(20, candidates3);
        let expected_missing = vec![4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19];
        assert_eq!(report3.missing_numbers, expected_missing);
        assert!(!report3.coverage_ok);
    }

    #[test]
    fn validate_question_coverage_detects_duplicates() {
        let report = validate_question_coverage(
            6,
            vec![
                ExtractedQuestionCandidate {
                    number: 5,
                    question_text: "Question 5".to_string(),
                    confidence: 0.5,
                    warnings: vec![],
                },
                ExtractedQuestionCandidate {
                    number: 5,
                    question_text: "Question 5 longer".to_string(),
                    confidence: 0.9,
                    warnings: vec!["page_warning".to_string()],
                },
                ExtractedQuestionCandidate {
                    number: 6,
                    question_text: "Question 6".to_string(),
                    confidence: 0.9,
                    warnings: vec![],
                },
            ],
        );

        assert_eq!(report.duplicate_numbers, vec![5]);
        assert!(!report.coverage_ok);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("duplicate_question_numbers")));
    }

    #[test]
    fn confirm_all_question_texts_moves_workflow_to_rubric_missing() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Project".to_string(), root.clone())
            .expect("project");
        project.documents.push(Document {
            id: "d1".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: crate::domain::document::PdfPreviewStatus::Ready,
                rendered_at: Some("now".to_string()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });
        project.questions = vec![Question {
            id: "q1".to_string(),
            number: 1,
            max_score: 10.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "Suggested".to_string(),
                source: TextFieldSource::ExamPdf,
                status: TextFieldStatus::Suggested,
                confidence: Some(0.9),
                warnings: vec![],
                updated_at: None,
            },
            rubric: RubricState {
                status: RubricStatus::Missing,
                source: None,
                max_score: None,
                expected_answer: None,
                criteria: vec![],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
                warnings: vec![],
                updated_at: None,
            },
            crop_template: None,
        }];
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        let service = service_for_tests(store.clone());
        let updated = service
            .confirm_all_question_texts(&project.id)
            .expect("confirm all");

        assert_eq!(updated.workflow.current_stage, WorkflowStage::RubricMissing);
        assert_eq!(
            updated.questions[0].question_text.status,
            TextFieldStatus::Confirmed
        );
    }

    #[tokio::test]
    async fn test_start_extraction_checks_preview_before_model_fallback() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Project".to_string(), root.clone())
            .expect("project");
        project.documents.push(Document {
            id: "d1".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        });
        store.save_project(&project).expect("save");

        let service = service_for_tests(store.clone());
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone();

        let result = service
            .start_extraction(
                app,
                project.id,
                Some("d1".to_string()),
                QuestionTextSource::ExamPdf,
            )
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, AppErrorCode::PdfPreviewNotReady);
    }

    #[tokio::test]
    async fn proof_7_question_cancel_preserves_teacher_text() {
        use crate::domain::job::{DuplicatePolicy, JobStatus};
        use crate::jobs::job_manager::JobRegistrationInput;

        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("proj_p7".to_string(), root.clone())
            .expect("project");

        project.questions.push(Question {
            id: "q1-p7".to_string(),
            number: 1,
            max_score: 10.0,
            answer_type: AnswerType::GeneralText,
            question_text: TextFieldState {
                value: "Öğretmenin yazdığı soru metni".to_string(),
                source: TextFieldSource::Manual,
                status: TextFieldStatus::Edited,
                confidence: None,
                warnings: vec![],
                updated_at: Some("now".to_string()),
            },
            rubric: RubricState {
                status: RubricStatus::Missing,
                source: None,
                max_score: None,
                expected_answer: None,
                criteria: vec![],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
                warnings: vec![],
                updated_at: None,
            },
            crop_template: None,
        });
        store.save_project(&project).expect("save project");

        let jm = Arc::new(JobManager::new());
        let service = service_for_tests_with_job_manager(store.clone(), jm.clone());
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone();

        let reg = jm
            .register_or_get_active_job(
                &app,
                JobRegistrationInput {
                    project_id: project.id.clone(),
                    project_root_path: Some(project.root_path.clone()),
                    kind: JobKind::QuestionTextExtraction,
                    display_label: Some("Question Text Extraction".into()),
                    total: 1,
                    message: "Extracting".into(),
                    correlation_id: Some("corr-p7".into()),
                    idempotency_key: Some("key-p7".into()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        // Request cancellation
        jm.cancel_job(&app, &reg.snapshot.id).unwrap();

        let input = QuestionTextVisionFallbackRunInput {
            project_id: project.id.clone(),
            expected_question_count: 1,
            target_question_numbers: vec![1],
            model_inputs: vec![],
        };

        let res = service
            .run_vision_fallback(app, reg.snapshot.id.clone(), input)
            .await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code, AppErrorCode::JobCancelled);

        let snap = jm.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snap.status, JobStatus::Cancelled);

        // Verify teacher text remains preserved
        let updated = store.get_project_snapshot(project.id).unwrap();
        assert_eq!(
            updated.questions[0].question_text.value,
            "Öğretmenin yazdığı soru metni"
        );
        assert_eq!(
            updated.questions[0].question_text.status,
            TextFieldStatus::Edited
        );
    }
}

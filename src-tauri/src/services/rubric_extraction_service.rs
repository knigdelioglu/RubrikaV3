use crate::domain::document::DocumentRole;
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::JobKind;
use crate::domain::model::{ModelInputImage, RubricExtractionRequest};
use crate::domain::rubric::{
    has_meaningful_rubric_content, validate_rubric_state, RubricSource, RubricStatus,
};
use crate::jobs::job_manager::JobManager;
use crate::services::document_content_extraction_service::{
    DocumentContentExtractionRequest, DocumentContentExtractionService, DocumentContentKind,
};
use crate::services::model_gateway::ModelGateway;
use crate::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeService, ModelUseCase,
};
use crate::services::project_store::ProjectStore;
use crate::services::prompt_contract::{build_prompt_contract, default_sampling};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;

use crate::services::pdf_service::PdfService;

#[derive(Clone)]
pub struct RubricExtractionService {
    pub project_store: ProjectStore,
    pub model_gateway: Arc<dyn ModelGateway>,
    pub job_manager: Arc<JobManager>,
    pub model_runtime_service: ModelRuntimeService,
    pub document_content_extraction_service: Arc<DocumentContentExtractionService>,
    pub pdf_service: Arc<dyn PdfService>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRubricPdfImportInput {
    pub project_id: String,
    pub document_id: Option<String>,
    pub expected_question_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartJobOutput {
    pub job_id: String,
    pub status: String,
}

impl RubricExtractionService {
    pub fn new(
        project_store: ProjectStore,
        model_gateway: Arc<dyn ModelGateway>,
        job_manager: Arc<JobManager>,
        model_runtime_service: ModelRuntimeService,
        pdf_service: Arc<dyn PdfService>,
        document_content_extraction_service: Arc<DocumentContentExtractionService>,
    ) -> Self {
        Self {
            project_store,
            model_gateway,
            job_manager,
            model_runtime_service,
            document_content_extraction_service,
            pdf_service,
        }
    }

    pub async fn start_import<R: tauri::Runtime>(
        &self,
        app: AppHandle<R>,
        input: StartRubricPdfImportInput,
    ) -> Result<StartJobOutput, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(input.project_id.clone())?;
        let inferred_question_count = project
            .expected_question_count
            .unwrap_or(project.questions.len() as u32);
        let expected_question_count = input
            .expected_question_count
            .unwrap_or(inferred_question_count)
            .max(inferred_question_count);
        if expected_question_count == 0 {
            return Err(AppError {
                code: AppErrorCode::WorkflowBlocked,
                message: "Soru sayısı belirtilmelidir.".to_string(),
                recoverable: true,
                suggested_action: Some("Sınavdaki soru sayısını girip tekrar deneyin.".to_string()),
                technical_details: None,
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        let job = self.job_manager.start_job(
            &app,
            input.project_id.clone(),
            Some(project.root_path.clone()),
            JobKind::RubricPdfImport,
            expected_question_count,
            "Rubrik PDF import başlatılıyor...".to_string(),
        )?;

        let job_id = job.id.clone();
        let service = self.clone();
        let project_id = input.project_id.clone();
        let doc_id = input.document_id.clone();
        let job_id_for_failure = job_id.clone();
        let app_handle = app.clone();

        tauri::async_runtime::spawn(async move {
            let run_result = service
                .run_import(
                    app_handle.clone(),
                    job_id_for_failure.clone(),
                    &project_id,
                    doc_id,
                    expected_question_count,
                )
                .await;
            if let Err(mut error) = run_result {
                error.correlation_id = job_id_for_failure.clone();
                let _ = service
                    .job_manager
                    .fail(&app_handle, &job_id_for_failure, error.clone());

                if let Ok(mut proj) = service
                    .project_store
                    .get_project_snapshot(project_id.clone())
                {
                    proj.workflow = crate::services::workflow_engine::evaluate_workflow(&proj);
                    if let Err(error) = service.project_store.commit_snapshot_cas(&proj) {
                        log::error!(
                            "Rubrik import workflow güncellemesi kalıcı yazılamadı: {error}"
                        );
                    }
                }
            } else {
                if let Ok(mut proj) = service
                    .project_store
                    .get_project_snapshot(project_id.clone())
                {
                    proj.workflow = crate::services::workflow_engine::evaluate_workflow(&proj);
                    if let Err(error) = service.project_store.commit_snapshot_cas(&proj) {
                        log::error!(
                            "Rubrik import workflow güncellemesi kalıcı yazılamadı: {error}"
                        );
                    }
                }
            }
        });

        Ok(StartJobOutput {
            job_id,
            status: "queued".to_string(),
        })
    }

    async fn run_import<R: tauri::Runtime>(
        &self,
        app: AppHandle<R>,
        job_id: String,
        project_id: &str,
        document_id: Option<String>,
        expected_question_count: u32,
    ) -> Result<(), AppError> {
        let cancel_token = self.job_manager.get_cancellation_token(&job_id);
        let _ = self.job_manager.set_running(&app, &job_id);

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Rubrik alma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }
        let mut failed_questions = Vec::new();
        let mut last_error = None;

        let mut project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;

        let rubric_doc = if let Some(doc_id) = document_id {
            project.documents.iter().find(|d| d.id == doc_id).cloned()
        } else {
            project
                .documents
                .iter()
                .find(|d| d.role == DocumentRole::Rubric || d.role == DocumentRole::AnswerKey)
                .cloned()
        };

        let rubric_doc = rubric_doc.ok_or_else(|| AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Rubrik PDF belgesi bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Lütfen önce Rubrik PDF dosyasını projeye ekleyin.".to_string()),
            technical_details: Some("No document with role Rubric".to_string()),
            correlation_id: Uuid::new_v4().to_string(),
        })?;

        let mut target_questions = Vec::new();
        for number in 1..=expected_question_count {
            let exists_and_active = project
                .questions
                .iter()
                .find(|q| q.number == number)
                .map(|q| {
                    let validation = validate_rubric_state(&q.rubric, Some(&q.answer_type));
                    !matches!(
                        q.rubric.status,
                        RubricStatus::Missing | RubricStatus::Invalid | RubricStatus::Legacy
                    ) && validation.valid
                })
                .unwrap_or(false);
            if !exists_and_active {
                target_questions.push(number);
            }
        }
        if target_questions.is_empty() {
            let _ = self.job_manager.succeed(&app, &job_id, None);
            return Ok(());
        }

        let _ = self.job_manager.update_progress(
            &app,
            &job_id,
            0,
            expected_question_count,
            "Gemma model lease'i hazırlanıyor...".to_string(),
        );
        let runtime_lease = self
            .model_runtime_service
            .acquire_ready_runtime_lease(
                None,
                "rubric_extraction",
                ModelRuntimeRequest {
                    use_case: ModelUseCase::RubricPdfImport,
                    capability: ModelCapability::Vision,
                    requires_mmproj: true,
                    timeout_seconds: 180,
                },
                &job_id,
            )
            .await?;

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Rubrik alma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }
        let _ = self.job_manager.update_progress(
            &app,
            &job_id,
            0,
            expected_question_count,
            "Gemma model sunucusu hazır.".to_string(),
        );

        let pdf_path = rubric_doc.resolve_path(&project.root_path)?;
        let content =
            self.document_content_extraction_service
                .extract(DocumentContentExtractionRequest {
                    project_id: project.id.clone(),
                    project_root: Path::new(&project.root_path).to_path_buf(),
                    document_id: rubric_doc.id.clone(),
                    document_path: pdf_path.clone(),
                    kind: DocumentContentKind::Rubric,
                    expected_question_count: Some(expected_question_count),
                    force_refresh: false,
                    vision_sources: vec![],
                })?;
        let raw_text = content.raw_text.clone().unwrap_or_default();
        let is_text_based = !content.vision_fallback_needed;
        let prompt = format!(
            "Sen deneyimli bir öğretmensin. Verilen cevap anahtarı veya rubrik içeriğinden her soru için canonical rubric suggestion üret. {}\nKurallar:\n- Metinde olmayanı uydurma.\n- Bulunmayan alanları boş dizi veya null bırak ve warnings alanına ekle.\n- source, status ve updatedAt üretme; backend bunları suggested olarak atar.\n- Placeholder kullanma.\n- Çıktı yalnızca geçerli JSON olsun.",
            crate::domain::rubric::canonical_rubric_extraction_prompt()
        );

        let _ = self.job_manager.update_progress(
            &app,
            &job_id,
            2,
            3,
            "Modelden rubrik verisi çekiliyor...".to_string(),
        );

        let start_time = std::time::Instant::now();

        let mut page_count = 1;
        let mut extraction_method = if is_text_based {
            "pdftotext"
        } else {
            "vision_fallback_prepared"
        };
        let mut image_paths = Vec::new();
        let mut image_dimensions = Vec::new();
        let mut image_byte_sizes = Vec::new();
        let mut page_usage: std::collections::BTreeMap<u32, serde_json::Value> =
            std::collections::BTreeMap::new();

        let extraction_result: Result<crate::domain::model::RubricExtractionResult, AppError> =
            if is_text_based {
                let mut merged_questions = std::collections::BTreeMap::new();
                let mut merged_warnings = Vec::new();

                for question_number in target_questions.clone() {
                    let _ = self.job_manager.update_progress(
                    &app,
                    &job_id,
                    question_number,
                    expected_question_count,
                    format!("Soru {question_number}/{expected_question_count} rubriği modelden alınıyor..."),
                );
                    let question_info = project
                        .questions
                        .iter()
                        .find(|q| q.number == question_number);
                    let question_text = question_info.and_then(|q| {
                        if matches!(
                            q.question_text.status,
                            crate::domain::question::TextFieldStatus::Missing
                                | crate::domain::question::TextFieldStatus::Failed
                        ) {
                            None
                        } else {
                            Some(q.question_text.value.as_str())
                        }
                    });
                    let question_text_status = question_info
                        .map(|q| match q.question_text.status {
                            crate::domain::question::TextFieldStatus::Missing => "missing",
                            crate::domain::question::TextFieldStatus::Failed => "failed",
                            crate::domain::question::TextFieldStatus::Suggested => "suggested",
                            crate::domain::question::TextFieldStatus::Confirmed => "confirmed",
                            crate::domain::question::TextFieldStatus::Edited => "edited",
                        })
                        .unwrap_or("missing");
                    let rubric_prompt = build_rubric_question_prompt(
                        &prompt,
                        question_number,
                        expected_question_count,
                        question_text,
                        question_text_status,
                    );
                    let prompt_contract = build_prompt_contract(
                        crate::domain::model::ModelRequestKind::RubricDraft,
                        "rubric_extraction_v2_typed_user_data",
                        crate::domain::rubric::RUBRIC_EXTRACTION_SCHEMA_VERSION,
                        "rubric_extraction_policy_v1",
                        rubric_prompt.clone(),
                        json!({
                            "rawText": raw_text,
                            "targetQuestionNumber": question_number,
                            "expectedQuestionCount": expected_question_count,
                            "questionText": question_text,
                            "questionTextStatus": question_text_status,
                            "strictJsonOnly": false,
                            "attempt": 1,
                        }),
                        default_sampling(8192),
                        Some(crate::domain::model::ModelResponseFormat::JsonSchema {
                            name: "rubric_extraction_suggestion".to_string(),
                            schema: crate::domain::rubric::canonical_rubric_extraction_schema(),
                        }),
                        None,
                    );
                    let req = RubricExtractionRequest {
                        prompt: rubric_prompt,
                        prompt_contract: Some(prompt_contract),
                        raw_text: Some(raw_text.clone()),
                        image_path: None,
                        target_question_number: question_number,
                        model_input_images: vec![],
                        strict_json_only: false,
                        attempt: 1,
                        project_root_path: Some(project.root_path.clone()),
                        job_id: Some(format!("{job_id}_q{question_number}")),
                    };
                    match self.draft_rubric_with_retry(req).await {
                        Ok(res) => {
                            let mut found = false;
                            for q in res.output.questions {
                                if q.number == question_number {
                                    merged_questions.insert(q.number, q);
                                    found = true;
                                }
                            }
                            if !found {
                                failed_questions.push(question_number);
                                merged_warnings.push(format!(
                                    "question_{question_number}_missing_in_model_response"
                                ));
                            }
                            merged_warnings.extend(res.output.document_warnings);
                        }
                        Err(error) => {
                            failed_questions.push(question_number);
                            merged_warnings.push(format!(
                                "question_{question_number}_failed: {:?}",
                                error.code
                            ));
                            last_error = Some(error);
                        }
                    }
                }

                if failed_questions.len() == target_questions.len() && !target_questions.is_empty()
                {
                    Err(last_error.clone().unwrap_or_else(|| AppError {
                        code: AppErrorCode::RubricImportEmpty,
                        message:
                            "Rubrik PDF'inden soru puanları veya beklenen cevaplar çıkarılamadı."
                                .to_string(),
                        recoverable: true,
                        suggested_action: None,
                        technical_details: Some(
                            "All questions failed during text extraction".to_string(),
                        ),
                        correlation_id: job_id.clone(),
                    }))
                } else {
                    Ok(crate::domain::model::RubricExtractionResult {
                        output: crate::domain::model::RubricExtractionOutput {
                            questions: merged_questions.into_values().collect(),
                            document_warnings: merged_warnings,
                        },
                        raw_response: "Merged per-question text extraction".to_string(),
                        diagnostics: crate::domain::model::ModelDiagnostics {
                            endpoint: "".to_string(),
                            request_kind: crate::domain::model::ModelRequestKind::RubricDraft,
                            http_status: Some(200),
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            prompt_length: Some(prompt.len() as u32),
                            image_count: Some(0),
                            image_total_bytes: Some(0),
                            base64_approx_total_bytes: Some(0),
                            model_input_images: vec![],
                            timeout_seconds: Some(600),
                            max_tokens: None,
                            finish_reason: Some("merged_per_question".to_string()),
                            content_length: Some(0),
                            reasoning_content_length: None,
                            raw_text_stored_path: None,
                            error_code: None,
                            provenance: None,
                        },
                        retry_metadata: None,
                    })
                }
            } else {
                // Vision fallback page-by-page
                let render_root = std::path::Path::new(&project.root_path)
                    .join("cache")
                    .join("model_inputs")
                    .join("rubric")
                    .join(&rubric_doc.id);
                let rendered_pages_dir = render_root.join("rendered_pages");

                std::fs::create_dir_all(&rendered_pages_dir).map_err(|e| AppError {
                    code: AppErrorCode::FileWriteFailed,
                    message: "Önizleme klasörü oluşturulamadı.".to_string(),
                    recoverable: false,
                    suggested_action: None,
                    technical_details: Some(e.to_string()),
                    correlation_id: job_id.clone(),
                })?;

                let rendered_pages = self
                    .pdf_service
                    .render_all_pages(&pdf_path, &rendered_pages_dir)?;
                page_count = rendered_pages.len();

                let preview_sources = rendered_pages
                    .iter()
                    .enumerate()
                    .map(|(index, page_path)| ((index + 1) as u32, page_path.clone()))
                    .collect::<Vec<_>>();
                let content = self.document_content_extraction_service.extract(
                    DocumentContentExtractionRequest {
                        project_id: project.id.clone(),
                        project_root: Path::new(&project.root_path).to_path_buf(),
                        document_id: rubric_doc.id.clone(),
                        document_path: pdf_path.clone(),
                        kind: DocumentContentKind::Rubric,
                        expected_question_count: Some(expected_question_count),
                        force_refresh: false,
                        vision_sources: preview_sources,
                    },
                )?;
                extraction_method = match content.method {
                crate::services::document_content_extraction_service::DocumentContentExtractionMethod::Cached => {
                    "cached"
                }
                crate::services::document_content_extraction_service::DocumentContentExtractionMethod::PdfToText => {
                    "pdftotext"
                }
                crate::services::document_content_extraction_service::DocumentContentExtractionMethod::VisionFallbackPrepared => {
                    "vision_fallback_prepared"
                }
            };

                let mut merged_questions = std::collections::BTreeMap::new();
                let mut merged_warnings = Vec::new();

                let prepared_inputs_by_page: std::collections::BTreeMap<u32, ModelInputImage> =
                    content
                        .model_input_images
                        .clone()
                        .into_iter()
                        .map(|input| (input.page_number, input))
                        .collect();

                for (index, page_path) in rendered_pages.iter().enumerate() {
                    let page_number = (index + 1) as u32;
                    let prepared_input = prepared_inputs_by_page.get(&page_number).cloned();
                    let opt_path = prepared_input
                        .as_ref()
                        .map(|input| input.output_image_path.clone())
                        .unwrap_or_else(|| page_path.to_string_lossy().to_string());

                    let size = std::fs::metadata(&opt_path).map(|m| m.len()).unwrap_or(0);
                    let dims = image::image_dimensions(&opt_path).unwrap_or((0, 0));

                    image_paths.push(opt_path.clone());
                    image_dimensions.push(format!("{:?}", dims));
                    image_byte_sizes.push(size);
                }

                let all_prepared_inputs = prepared_inputs_by_page
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                let fallback_image_path = image_paths.first().cloned();
                let page_count = all_prepared_inputs.len() as u32;
                let page_questions =
                    crate::services::page_window_service::question_numbers_by_page(&raw_text);
                for question_number in target_questions.clone() {
                    let _ = self.job_manager.update_progress(
                    &app,
                    &job_id,
                    question_number,
                    expected_question_count,
                    format!("Soru {question_number}/{expected_question_count} rubriği modelden alınıyor..."),
                );
                    let question_info = project
                        .questions
                        .iter()
                        .find(|q| q.number == question_number);
                    let question_text = question_info.and_then(|q| {
                        if matches!(
                            q.question_text.status,
                            crate::domain::question::TextFieldStatus::Missing
                                | crate::domain::question::TextFieldStatus::Failed
                        ) {
                            None
                        } else {
                            Some(q.question_text.value.as_str())
                        }
                    });
                    let question_text_status = question_info
                        .map(|q| match q.question_text.status {
                            crate::domain::question::TextFieldStatus::Missing => "missing",
                            crate::domain::question::TextFieldStatus::Failed => "failed",
                            crate::domain::question::TextFieldStatus::Suggested => "suggested",
                            crate::domain::question::TextFieldStatus::Confirmed => "confirmed",
                            crate::domain::question::TextFieldStatus::Edited => "edited",
                        })
                        .unwrap_or("missing");
                    let rubric_prompt = build_rubric_question_prompt(
                        &prompt,
                        question_number,
                        expected_question_count,
                        question_text,
                        question_text_status,
                    );
                    // TD-19: escalate from the exact target page to a ±1 window
                    // and finally to the whole document (bounded) instead of
                    // resending every page for every question.
                    let base_pages =
                        crate::services::page_window_service::candidate_pages_for_question(
                            question_number,
                            &page_questions,
                            expected_question_count,
                            page_count,
                        );
                    let window_pages = crate::services::page_window_service::expand_page_window(
                        &base_pages,
                        page_count,
                        crate::services::page_window_service::WINDOW_RADIUS,
                    );
                    let all_pages = (1..=page_count).collect::<Vec<_>>();

                    let mut question_found = false;
                    let mut attempts = 0u32;
                    let mut pages_used: Vec<u32> = Vec::new();
                    let mut last_stage = "target";
                    for (stage, tier_pages) in [
                        ("target", base_pages),
                        ("window", window_pages),
                        ("fallback", all_pages),
                    ] {
                        if tier_pages.is_empty() {
                            continue;
                        }
                        let tier_inputs =
                            crate::services::page_window_service::select_inputs_by_pages(
                                &all_prepared_inputs,
                                &tier_pages,
                            );
                        if tier_inputs.is_empty() {
                            continue;
                        }
                        attempts += 1;
                        pages_used = tier_pages.clone();
                        last_stage = stage;
                        let tier_image_path = tier_inputs
                            .first()
                            .map(|input| input.output_image_path.clone())
                            .or_else(|| fallback_image_path.clone());
                        let req = self.build_vision_rubric_request(
                            &RubricVisionQuestionContext {
                                prompt: &rubric_prompt,
                                question_number,
                                expected_question_count,
                                project_root: &project.root_path,
                                job_id: &job_id,
                                question_text,
                                question_text_status,
                            },
                            tier_image_path,
                            tier_inputs,
                            false,
                            1,
                            &pages_used,
                        );
                        match self.draft_rubric_with_retry(req).await {
                            Ok(page_res) => {
                                let mut found = false;
                                for q in page_res.output.questions {
                                    if q.number == question_number {
                                        merged_questions.insert(q.number, q);
                                        found = true;
                                    }
                                }
                                merged_warnings.extend(page_res.output.document_warnings);
                                if found {
                                    question_found = true;
                                    break;
                                }
                            }
                            Err(e) => {
                                last_error = Some(e);
                            }
                        }
                    }
                    if !question_found {
                        failed_questions.push(question_number);
                        merged_warnings.push(format!(
                            "question_{question_number}_missing_in_model_response"
                        ));
                    }
                    page_usage.insert(
                        question_number,
                        json!({
                            "pages": pages_used,
                            "attempts": attempts,
                            "stage": last_stage,
                            "found": question_found,
                        }),
                    );
                }

                if failed_questions.len() == target_questions.len() && !target_questions.is_empty()
                {
                    Err(last_error.clone().unwrap_or_else(|| AppError {
                        code: AppErrorCode::RubricImportEmpty,
                        message:
                            "Rubrik PDF'inden soru puanları veya beklenen cevaplar çıkarılamadı."
                                .to_string(),
                        recoverable: true,
                        suggested_action: None,
                        technical_details: Some(
                            "All questions failed during vision extraction".to_string(),
                        ),
                        correlation_id: job_id.clone(),
                    }))
                } else {
                    Ok(crate::domain::model::RubricExtractionResult {
                        output: crate::domain::model::RubricExtractionOutput {
                            questions: merged_questions.into_values().collect(),
                            document_warnings: merged_warnings,
                        },
                        raw_response: "Merged vision fallback".to_string(),
                        diagnostics: crate::domain::model::ModelDiagnostics {
                            endpoint: "".to_string(),
                            request_kind: crate::domain::model::ModelRequestKind::RubricDraft,
                            http_status: Some(200),
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            prompt_length: Some(prompt.len() as u32),
                            image_count: Some(image_paths.len() as u32),
                            image_total_bytes: Some(image_byte_sizes.iter().sum()),
                            base64_approx_total_bytes: Some(
                                image_byte_sizes
                                    .iter()
                                    .map(|bytes| bytes.div_ceil(3) * 4)
                                    .sum(),
                            ),
                            model_input_images: prepared_inputs_by_page.into_values().collect(),
                            timeout_seconds: Some(600),
                            max_tokens: None,
                            finish_reason: Some("stop".to_string()),
                            content_length: Some(0),
                            reasoning_content_length: None,
                            raw_text_stored_path: None,
                            error_code: None,
                            provenance: None,
                        },
                        retry_metadata: None,
                    })
                }
            };

        let result = match extraction_result {
            Ok(res) => res,
            Err(mut original_error) => {
                let elapsed = start_time.elapsed().as_millis() as u64;
                let model_status_after = self
                    .model_runtime_service
                    .get_model_status(None)
                    .await
                    .unwrap_or_default();

                // Log tail formatting
                let log_tail = model_status_after
                    .log_path
                    .as_ref()
                    .map(|p| read_log_tail(p, 100))
                    .unwrap_or_default();

                let mut highlights = Vec::new();
                for line in &log_tail {
                    let lower = line.to_lowercase();
                    if [
                        "error",
                        "exception",
                        "segmentation",
                        "killed",
                        "out of memory",
                        "oom",
                        "ggml",
                        "alloc",
                        "failed",
                    ]
                    .iter()
                    .any(|&kw| lower.contains(kw))
                    {
                        highlights.push(line.clone());
                    }
                }

                let details = format!(
                    "request_kind = RubricExtraction\n\
                     model_status_at_start = verified_by_runtime_lease\n\
                     model_status_before_model_request = verified_by_runtime_lease\n\
                     model_status_after_failure = {:?}\n\
                     endpoint = {}\n\
                     elapsed_time = {} ms\n\
                     rubric_document_id = {}\n\
                     rubric_document_path = {}\n\
                     extraction_method = {}\n\
                    pdf_page_count = {}\n\
                    payload_text_length = {}\n\
                    prompt_length = {}\n\
                    image_count = {}\n\
                    image_paths = {:?}\n\
                    image_dimensions = {:?}\n\
                    image_byte_sizes = {:?}\n\
                     payload_summary = {}\n\
                     model_profile_id = {}\n\
                     log_tail_path = {}\n\
                     log_tail_highlights = {:#?}\n\
                     original_details = {:?}",
                    model_status_after,
                    runtime_lease.base_url(),
                    elapsed,
                    rubric_doc.id,
                    pdf_path.to_string_lossy(),
                    extraction_method,
                    page_count,
                    raw_text.len(),
                    prompt.len(),
                    image_paths.len(),
                    image_paths,
                    image_dimensions,
                    image_byte_sizes,
                    rubric_payload_details(
                        &prompt,
                        raw_text.len(),
                        600,
                        &image_paths,
                        &image_byte_sizes
                    ),
                    runtime_lease.profile_id(),
                    model_status_after
                        .log_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    highlights,
                    original_error.technical_details
                );

                original_error.technical_details = Some(details);

                if !model_status_after.server_running {
                    original_error.code = AppErrorCode::ModelServerCrashedDuringRequest;
                    original_error.message =
                        "Gemma model sunucusu rubrik çıkarma sırasında kapandı.".to_string();
                    original_error.suggested_action = Some(
                        "Model loglarını kontrol edin, modeli yeniden başlatıp daha küçük/optimize edilmiş girişle tekrar deneyin."
                            .to_string(),
                    );
                } else if !model_status_after.health_ok {
                    original_error.code = AppErrorCode::ModelServerLostDuringRequest;
                    original_error.message =
                        "Gemma model sunucu bağlantısı istek sırasında koptu.".to_string();
                    original_error.suggested_action = Some(
                        "Model loglarını kontrol edin, modeli yeniden başlatıp daha küçük/optimize edilmiş girişle tekrar deneyin."
                            .to_string(),
                    );
                } else if original_error.code == AppErrorCode::ModelTimeout {
                    original_error.code = AppErrorCode::ModelRequestTimeout;
                }

                return Err(original_error);
            }
        };

        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                let _ = self.job_manager.mark_cancelled(&app, &job_id);
                return Err(AppError {
                    code: AppErrorCode::JobCancelled,
                    message: "Rubrik alma işlemi iptal edildi.".to_string(),
                    recoverable: true,
                    suggested_action: None,
                    technical_details: None,
                    correlation_id: Uuid::new_v4().to_string(),
                });
            }
        }

        let mut imported_numbers = Vec::new();
        let mut successful_imports = 0usize;
        let mut partial_model_json_recovered = false;
        for q_cand in result.output.questions {
            if let Some(question) = project
                .questions
                .iter_mut()
                .find(|q| q.number == q_cand.number)
            {
                if !matches!(
                    question.rubric.status,
                    RubricStatus::Missing | RubricStatus::Invalid | RubricStatus::Legacy
                ) {
                    continue;
                }
                partial_model_json_recovered |= q_cand
                    .warnings
                    .iter()
                    .any(|warning| warning == "partial_model_json_recovered");
                question.rubric.status = RubricStatus::Suggested;
                question.rubric.source = Some(RubricSource::GemmaDraft);
                if let Some(mp) = q_cand.max_points {
                    question.rubric.max_score = Some(mp);
                }
                if let Some(ea) = q_cand.expected_answer {
                    question.rubric.expected_answer = Some(ea);
                }
                if !q_cand.key_concepts.is_empty() {
                    question.rubric.key_concepts = q_cand.key_concepts;
                }
                if !q_cand.criteria.is_empty() {
                    question.rubric.criteria = q_cand.criteria;
                }
                if !q_cand.partial_credit_hints.is_empty() {
                    question.rubric.partial_credit_hints = q_cand.partial_credit_hints;
                }
                if !q_cand.zero_score_conditions.is_empty() {
                    question.rubric.zero_score_conditions = q_cand.zero_score_conditions;
                }
                if !q_cand.common_mistakes.is_empty() {
                    question.rubric.common_mistakes = q_cand.common_mistakes;
                }
                question.rubric.warnings = q_cand.warnings;
                if !has_meaningful_rubric_content(&question.rubric) {
                    question
                        .rubric
                        .warnings
                        .push("rubric_empty_content".to_string());
                }
                let validation =
                    validate_rubric_state(&question.rubric, Some(&question.answer_type));
                if !validation.valid {
                    question.rubric.status = RubricStatus::Invalid;
                    failed_questions.push(question.number);
                } else {
                    successful_imports += 1;
                    imported_numbers.push(question.number);
                }
            }
        }

        for &fq in &failed_questions {
            if let Some(question) = project.questions.iter_mut().find(|q| q.number == fq) {
                if matches!(
                    question.rubric.status,
                    RubricStatus::Missing | RubricStatus::Invalid | RubricStatus::Legacy
                ) {
                    question.rubric.status = RubricStatus::Invalid;
                    question
                        .rubric
                        .warnings
                        .push("Model extraction failed for this question".to_string());
                }
            }
        }

        let missing_rubric_numbers = project
            .questions
            .iter()
            .filter(|question| question.rubric.status == RubricStatus::Missing)
            .map(|question| question.number)
            .collect::<Vec<_>>();

        let failed_question_numbers = failed_questions.clone();
        let partial_success =
            !missing_rubric_numbers.is_empty() || !failed_question_numbers.is_empty();

        if successful_imports == 0 {
            if let Some(err) = last_error {
                return Err(err);
            }
            project.workflow = crate::services::workflow_engine::evaluate_workflow(&project);
            self.project_store
                .commit_snapshot_cas(&project)
                .map(|_| ())?;
            return Err(AppError {
                code: AppErrorCode::RubricEmptyContent,
                message: "Rubrik PDF'inden soru puanları veya beklenen cevaplar çıkarılamadı."
                    .to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Soru numaraları ile PDF içindeki numaralar uyuşmuyor olabilir.".to_string(),
                ),
                technical_details: Some("No matching question numbers found".to_string()),
                correlation_id: Uuid::new_v4().to_string(),
            });
        }

        project.workflow = crate::services::workflow_engine::evaluate_workflow(&project);
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;

        let mut per_question_results = Vec::new();
        for question_number in 1..=expected_question_count {
            let status = if imported_numbers.contains(&question_number) {
                "imported"
            } else if failed_question_numbers.contains(&question_number) {
                "failed"
            } else {
                "missing"
            };

            let artifact_dir = format!(
                "logs/model_responses/rubric_import/{}/question_{}/attempt_1",
                job_id, question_number
            );
            let q_warnings = project
                .questions
                .iter()
                .find(|q| q.number == question_number)
                .map(|q| q.rubric.warnings.clone())
                .unwrap_or_default();

            per_question_results.push(serde_json::json!({
                "questionNumber": question_number,
                "status": status,
                "artifactDir": artifact_dir,
                "warnings": q_warnings,
                "errorCode": if status == "failed" { Some("MODEL_EXTRACTION_FAILED") } else { None },
                "pageUsage": page_usage.get(&question_number),
            }));
        }

        let _ = self.job_manager.succeed(
            &app,
            &job_id,
            Some(serde_json::json!({
                "imported_count": successful_imports,
                "importedQuestionNumbers": imported_numbers,
                "missingRubricQuestions": missing_rubric_numbers,
                "failedQuestionNumbers": failed_question_numbers,
                "partialSuccess": partial_success,
                "partialReason": if partial_model_json_recovered {
                    Some("model_response_truncated_or_incomplete_json")
                } else if partial_success {
                    Some("some_questions_failed_or_missing")
                } else {
                    None
                },
                "perQuestionResults": per_question_results,
            })),
        );

        Ok(())
    }
}

fn should_retry_rubric_parse(error: &AppError) -> bool {
    matches!(
        error.code,
        AppErrorCode::RubricJsonInvalid
            | AppErrorCode::RubricJsonParseFailed
            | AppErrorCode::RubricJsonSchemaUnsupported
            | AppErrorCode::RubricSchemaValidationFailed
            | AppErrorCode::RubricEmptyContent
            | AppErrorCode::ModelResponseInvalidJson
            | AppErrorCode::ModelResponseInvalidSchema
            | AppErrorCode::ModelResponseEmpty
            | AppErrorCode::ModelResponseReasoningOnly
    )
}

/// Per-question context needed to build a vision rubric extraction request.
struct RubricVisionQuestionContext<'a> {
    prompt: &'a str,
    question_number: u32,
    expected_question_count: u32,
    project_root: &'a str,
    job_id: &'a str,
    question_text: Option<&'a str>,
    question_text_status: &'a str,
}

impl RubricExtractionService {
    /// Builds a vision rubric extraction request targeting a bounded page set.
    fn build_vision_rubric_request(
        &self,
        context: &RubricVisionQuestionContext<'_>,
        image_path: Option<String>,
        model_input_images: Vec<ModelInputImage>,
        strict_json_only: bool,
        attempt: u32,
        pages_used: &[u32],
    ) -> RubricExtractionRequest {
        let mut user_data = json!({
            "targetQuestionNumber": context.question_number,
            "expectedQuestionCount": context.expected_question_count,
            "questionText": context.question_text,
            "questionTextStatus": context.question_text_status,
            "strictJsonOnly": strict_json_only,
            "attempt": attempt,
            "imageInput": true,
        });
        if !pages_used.is_empty() {
            user_data["includedPages"] = json!(pages_used);
        }
        let prompt_contract = build_prompt_contract(
            crate::domain::model::ModelRequestKind::RubricDraft,
            "rubric_extraction_v2_typed_user_data",
            crate::domain::rubric::RUBRIC_EXTRACTION_SCHEMA_VERSION,
            "rubric_extraction_policy_v1",
            context.prompt.to_string(),
            user_data,
            default_sampling(8192),
            Some(crate::domain::model::ModelResponseFormat::JsonSchema {
                name: "rubric_extraction_suggestion".to_string(),
                schema: crate::domain::rubric::canonical_rubric_extraction_schema(),
            }),
            None,
        );
        RubricExtractionRequest {
            prompt: context.prompt.to_string(),
            prompt_contract: Some(prompt_contract),
            raw_text: None,
            image_path,
            target_question_number: context.question_number,
            model_input_images,
            strict_json_only,
            attempt,
            project_root_path: Some(context.project_root.to_string()),
            job_id: Some(format!("{}_q{}", context.job_id, context.question_number)),
        }
    }

    async fn draft_rubric_with_retry(
        &self,
        request: RubricExtractionRequest,
    ) -> Result<crate::domain::model::RubricExtractionResult, AppError> {
        let first_attempt = self.model_gateway.draft_rubric(request.clone()).await;
        match first_attempt {
            Ok(result) => Ok(result),
            Err(first_error) if should_retry_rubric_parse(&first_error) => {
                let had_images =
                    !request.model_input_images.is_empty() || request.image_path.is_some();
                let mut salvage_used = false;
                let mut text_only_repair_used = false;
                let mut repair_attempted = false;
                let mut retry_reason = None;

                // TD-20: recover the malformed first response without resending
                // the images. The gateway already attempts a deterministic
                // salvage while parsing; this layer adds a full-text salvage and
                // a text-only JSON repair that never sends images.
                if had_images {
                    if let Some(raw_response) =
                        crate::services::llama_server_gateway::read_saved_rubric_raw_response(
                            &request,
                        )
                    {
                        let cleaned =
                            crate::services::llama_server_gateway::strip_reasoning_and_fences(
                                &raw_response,
                            );
                        if let Some(payload) =
                            crate::services::llama_server_gateway::parse_partial_rubric_questions(
                                &cleaned,
                            )
                        {
                            salvage_used = true;
                            return Ok(rubric_result_from_salvage(
                                payload,
                                &request,
                                cleaned,
                                crate::domain::model::RubricExtractionRetryMetadata {
                                    attempts: 1,
                                    image_count: request.model_input_images.len() as u32,
                                    pages_used: vec![],
                                    retry_reason: None,
                                    salvage_used,
                                    text_only_repair_used: false,
                                    targeted_pages: vec![],
                                },
                            ));
                        }
                        if !cleaned.trim().is_empty() {
                            repair_attempted = true;
                            let mut repair_request = request.clone();
                            repair_request.raw_text = Some(cleaned.clone());
                            repair_request.model_input_images = vec![];
                            repair_request.image_path = None;
                            repair_request.strict_json_only = true;
                            repair_request.attempt = 2;
                            if let Some(prompt_contract) = repair_request.prompt_contract.as_mut() {
                                if let Some(user_data) = prompt_contract.user_data.as_object_mut() {
                                    user_data.insert("strictJsonOnly".to_string(), json!(true));
                                    user_data.insert("attempt".to_string(), json!(2));
                                    user_data.insert("rawText".to_string(), json!(cleaned));
                                }
                            }
                            match self.model_gateway.draft_rubric(repair_request).await {
                                Ok(mut result) => {
                                    text_only_repair_used = true;
                                    result.retry_metadata =
                                        Some(crate::domain::model::RubricExtractionRetryMetadata {
                                            attempts: 2,
                                            image_count: 0,
                                            pages_used: vec![],
                                            retry_reason: None,
                                            salvage_used: false,
                                            text_only_repair_used,
                                            targeted_pages: vec![],
                                        });
                                    return Ok(result);
                                }
                                Err(repair_error) => {
                                    retry_reason = Some(format!(
                                        "text_only_repair_failed:{:?}",
                                        repair_error.code
                                    ));
                                }
                            }
                        } else {
                            retry_reason = Some("first_response_empty".to_string());
                        }
                    }
                }

                // Explicit-reason multimodal retry (last resort).
                let reason = retry_reason
                    .unwrap_or_else(|| format!("first_error_code={:?}", first_error.code));
                let retry_attempt = if repair_attempted { 3 } else { 2 };
                let mut retry_request = request;
                retry_request.strict_json_only = true;
                retry_request.attempt = retry_attempt;
                if let Some(prompt_contract) = retry_request.prompt_contract.as_mut() {
                    if let Some(user_data) = prompt_contract.user_data.as_object_mut() {
                        user_data.insert("strictJsonOnly".to_string(), json!(true));
                        user_data.insert("attempt".to_string(), json!(retry_attempt));
                    }
                }
                let retry_result = self.model_gateway.draft_rubric(retry_request.clone()).await;
                match retry_result {
                    Ok(mut result) => {
                        result.retry_metadata =
                            Some(crate::domain::model::RubricExtractionRetryMetadata {
                                attempts: retry_attempt,
                                image_count: retry_request.model_input_images.len() as u32,
                                pages_used: vec![],
                                retry_reason: Some(reason.clone()),
                                salvage_used,
                                text_only_repair_used,
                                targeted_pages: vec![],
                            });
                        Ok(result)
                    }
                    Err(mut retry_error) => {
                        let details = format!(
                            "retry_attempted=true\nfirst_error_code={:?}\nfirst_error_message={}\nfirst_error_details={:?}\nsalvage_used={}\ntext_only_repair_used={}\nretry_reason={}\nretry_error_code={:?}\nretry_error_message={}\nretry_error_details={:?}",
                            first_error.code,
                            first_error.message,
                            first_error.technical_details,
                            salvage_used,
                            text_only_repair_used,
                            reason,
                            retry_error.code,
                            retry_error.message,
                            retry_error.technical_details
                        );
                        retry_error.technical_details = Some(details);
                        Err(retry_error)
                    }
                }
            }
            Err(error) => Err(error),
        }
    }
}

/// Builds a `RubricExtractionResult` from a deterministically salvaged payload.
fn rubric_result_from_salvage(
    payload: crate::domain::model::RubricImportPayload,
    request: &RubricExtractionRequest,
    raw_response: String,
    retry_metadata: crate::domain::model::RubricExtractionRetryMetadata,
) -> crate::domain::model::RubricExtractionResult {
    let output = crate::services::llama_server_gateway::rubric_payload_to_output(payload);
    let raw_length = raw_response.len() as u32;
    crate::domain::model::RubricExtractionResult {
        output,
        raw_response,
        diagnostics: crate::domain::model::ModelDiagnostics {
            endpoint: "".to_string(),
            request_kind: crate::domain::model::ModelRequestKind::RubricDraft,
            http_status: Some(200),
            duration_ms: 0,
            prompt_length: None,
            image_count: Some(request.model_input_images.len() as u32),
            image_total_bytes: None,
            base64_approx_total_bytes: None,
            model_input_images: request.model_input_images.clone(),
            timeout_seconds: None,
            max_tokens: None,
            finish_reason: Some("salvaged".to_string()),
            content_length: Some(raw_length),
            reasoning_content_length: None,
            raw_text_stored_path: None,
            error_code: None,
            provenance: None,
        },
        retry_metadata: Some(retry_metadata),
    }
}

fn read_log_tail(path: &std::path::Path, line_count: usize) -> Vec<String> {
    if !path.exists() {
        return vec![format!(
            "Log file does not exist at {}",
            path.to_string_lossy()
        )];
    }
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return vec![format!("Failed to open log file: {e}")],
    };
    use std::io::BufRead;
    let reader = std::io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
    let len = lines.len();
    if len <= line_count {
        lines
    } else {
        lines[len - line_count..].to_vec()
    }
}

fn rubric_payload_details(
    prompt: &str,
    payload_text_length: usize,
    timeout_seconds: u64,
    image_paths: &[String],
    image_byte_sizes: &[u64],
) -> String {
    let image_total_bytes: u64 = image_byte_sizes.iter().sum();
    let base64_approx_total_bytes: u64 = image_byte_sizes
        .iter()
        .map(|bytes| bytes.div_ceil(3) * 4)
        .sum();

    format!(
        "prompt_length={}; payload_text_length={}; timeout_seconds={}; image_count={}; image_total_bytes={}; base64_approx_total_bytes={}; per_image={{paths={:?}, bytes={:?}}}",
        prompt.len(),
        payload_text_length,
        timeout_seconds,
        image_paths.len(),
        image_total_bytes,
        base64_approx_total_bytes,
        image_paths,
        image_byte_sizes,
    )
}

fn build_rubric_question_prompt(
    base_prompt: &str,
    question_number: u32,
    expected_question_count: u32,
    question_text: Option<&str>,
    question_text_status: &str,
) -> String {
    let _ = (
        question_number,
        expected_question_count,
        question_text,
        question_text_status,
    );
    format!(
        "{}\n\nPrompt sürümü: {}. Kaynak içerik, hedef soru ve soru metni yalnız typed user-data JSON olarak değerlendirilir. Başka soru döndürme; yalnızca şemaya uygun JSON üret.",
        base_prompt.trim(),
        crate::domain::rubric::RUBRIC_EXTRACTION_SCHEMA_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::Document;
    use crate::domain::errors::AppErrorCode;
    use crate::domain::model::{ModelMode, ModelProfile};
    use crate::jobs::job_manager::JobManager;
    use crate::services::document_content_extraction_service::DocumentContentExtractionService;
    use crate::services::llama_server_gateway::LlamaServerGateway;
    use crate::services::model_config_service::ModelConfigService;
    use crate::services::model_input_image_service::ModelInputImageService;
    use crate::services::model_process_manager::test_support::{
        available_loopback_port, lock_model_runtime_test,
    };
    use crate::services::model_process_manager::ModelProcessManager;
    use crate::services::model_runtime_service::ModelRuntimeService;
    use crate::services::project_store::ProjectStore;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::thread;
    use std::{env, fs};

    fn temp_project_root() -> String {
        let root = std::env::temp_dir().join(format!("rubrika-v3-rubric-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().to_string()
    }

    fn service_for_tests(
        project_store: ProjectStore,
        model_config: ModelConfigService,
    ) -> RubricExtractionService {
        let profile = model_config
            .get_profile(None)
            .expect("test model profile should exist");
        let model_gateway_impl = std::sync::Arc::new(LlamaServerGateway::new(profile.base_url));
        let model_process_manager = ModelProcessManager::new_with_state_path(
            model_config.clone(),
            model_gateway_impl.clone(),
            std::env::temp_dir().join(format!("rubrika-model-state-{}.json", uuid::Uuid::new_v4())),
        );
        let model_runtime_service =
            ModelRuntimeService::new(model_config, model_process_manager.clone());
        let document_content_extraction_service =
            std::sync::Arc::new(DocumentContentExtractionService::new(std::sync::Arc::new(
                ModelInputImageService::default(),
            )));
        RubricExtractionService::new(
            project_store,
            model_gateway_impl,
            std::sync::Arc::new(JobManager::new()),
            model_runtime_service,
            std::sync::Arc::new(crate::services::pdf_service::SystemPdfService),
            document_content_extraction_service,
        )
    }

    fn spawn_test_server() -> (String, String, u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("rubric test server requires a loopback TCP listener");
        let addr = listener
            .local_addr()
            .expect("rubric test server listener must have a local address");
        let port = addr.port();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\",\"choices\":[{\"message\":{\"content\":\"OK\"}}]}";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (
            format!("http://{}", addr),
            "127.0.0.1".to_string(),
            port,
            handle,
        )
    }

    fn write_mock_llama_server_script() -> PathBuf {
        let path = env::temp_dir().join(format!("rubrika-mock-rubric-{}.py", uuid::Uuid::new_v4()));
        let script = r#"#!/usr/bin/python3
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

args = sys.argv[1:]
if "--help" in args or "-h" in args:
    print("--cache-type-k\n--cache-type-v\n--mmproj-offload")
    sys.exit(0)

host = "127.0.0.1"
port = 8080
for i, arg in enumerate(args):
    if arg == "--host" and i + 1 < len(args):
        host = args[i + 1]
    elif arg == "--port" and i + 1 < len(args):
        port = int(args[i + 1])

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
            self._write_json({"status": "ok"})
            return
        self._write_json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path == "/v1/chat/completions":
            length = int(self.headers.get("Content-Length", "0") or "0")
            _ = self.rfile.read(length)
            self._write_json({"choices": [{"message": {"content": "OK"}}]})
            return
        self._write_json({"error": "not found"}, 404)

try:
    sys.stderr.write(f"Listening on {host}:{port}\n")
    sys.stderr.flush()
    server = HTTPServer((host, port), Handler)
    server.serve_forever()
except Exception as e:
    sys.stderr.write(f"HTTPServer failed: {e}\n")
    sys.stderr.flush()
    sys.exit(1)
"#;
        fs::write(&path, script).expect("mock rubric llama-server script should be writable");
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&path)
                .expect("mock rubric llama-server metadata should be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)
                .expect("mock rubric llama-server script should be executable");
        }
        path
    }

    #[tokio::test]
    async fn test_start_import_fails_when_model_server_not_running() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let project = store
            .create_project("Test Project".to_string(), root)
            .unwrap();

        let config_path = std::env::temp_dir().join(format!(
            "rubrika-model-config-{}.json",
            uuid::Uuid::new_v4()
        ));
        let model_config = ModelConfigService::new_with_path(config_path);

        let service = service_for_tests(store, model_config);
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone();

        let result = service
            .start_import(
                app,
                StartRubricPdfImportInput {
                    project_id: project.id.clone(),
                    document_id: None,
                    expected_question_count: Some(1),
                },
            )
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.status, "queued");

        let jobs = service.job_manager.list_jobs(&project.id).unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[tokio::test]
    async fn test_start_import_succeeds_when_model_server_running() {
        let _model_runtime_guard = lock_model_runtime_test().await;
        let root = temp_project_root();
        let store = ProjectStore::new();
        let project = store
            .create_project("Test Project".to_string(), root)
            .unwrap();

        let config_path = std::env::temp_dir().join(format!(
            "rubrika-model-config-{}.json",
            uuid::Uuid::new_v4()
        ));
        let model_config = ModelConfigService::new_with_path(config_path);

        // Spawn mock server
        let (base_url, host, port, _server_handle) = spawn_test_server();

        // Create dummy server file path
        let dummy_server_path =
            std::env::temp_dir().join(format!("dummy-llama-server-{}", uuid::Uuid::new_v4()));
        std::fs::File::create(&dummy_server_path).unwrap();

        // Create a custom model profile and update active profile in config service
        let profile = ModelProfile {
            id: format!("test-gemma-{}", uuid::Uuid::new_v4()),
            display_name: "Test Gemma".to_string(),
            mode: ModelMode::External,
            host,
            port,
            base_url,
            server_path: dummy_server_path.to_string_lossy().to_string(),
            model_path: "".to_string(),
            mmproj_path: "".to_string(),
            runtime_preset: crate::domain::model::ModelRuntimePreset::Standard,
            privacy_mode: crate::domain::model::PrivacyMode::StrictLocal,
        };
        model_config.update_profile(profile).unwrap();

        let service = service_for_tests(store, model_config);
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone();

        let result = service
            .start_import(
                app,
                StartRubricPdfImportInput {
                    project_id: project.id.clone(),
                    document_id: None,
                    expected_question_count: Some(1),
                },
            )
            .await;

        // Cleanup dummy file
        let _ = std::fs::remove_file(dummy_server_path);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.status, "queued");

        // Verify that a job is registered in job history/manager
        let jobs = service.job_manager.list_jobs(&project.id).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, output.job_id);
    }

    #[tokio::test]
    async fn test_start_import_auto_starts_managed_model_when_closed() {
        let _model_runtime_guard = lock_model_runtime_test().await;
        let server_path = write_mock_llama_server_script();
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Test Project".to_string(), root.clone())
            .unwrap();

        let rubric_path = std::path::Path::new(&root)
            .join("documents")
            .join("rubric.pdf");
        std::fs::create_dir_all(rubric_path.parent().unwrap()).unwrap();
        std::fs::write(&rubric_path, b"%PDF-1.4\n%%EOF").unwrap();
        project.documents.push(Document {
            id: "rubric-doc".to_string(),
            role: DocumentRole::Rubric,
            file_name: "rubric.pdf".to_string(),
            stored_path: rubric_path.to_string_lossy().to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: None,
        });
        store.save_project(&project).unwrap();

        let config_path = std::env::temp_dir().join(format!(
            "rubrika-model-config-{}.json",
            uuid::Uuid::new_v4()
        ));
        let model_config = ModelConfigService::new_with_path(config_path);
        let port = available_loopback_port();
        let profile = ModelProfile {
            id: format!("managed-test-{}", uuid::Uuid::new_v4()),
            display_name: "Managed Test".to_string(),
            mode: ModelMode::Managed,
            server_path: server_path.to_string_lossy().to_string(),
            model_path: server_path.to_string_lossy().to_string(),
            mmproj_path: server_path.to_string_lossy().to_string(),
            host: "127.0.0.1".to_string(),
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            runtime_preset: crate::domain::model::ModelRuntimePreset::Standard,
            privacy_mode: crate::domain::model::PrivacyMode::StrictLocal,
        };
        model_config.update_profile(profile).unwrap();

        let service = service_for_tests(store, model_config);
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone();

        let result = service
            .start_import(
                app,
                StartRubricPdfImportInput {
                    project_id: project.id.clone(),
                    document_id: Some("rubric-doc".to_string()),
                    expected_question_count: Some(1),
                },
            )
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.status, "queued");

        let mut status = None;
        for _ in 0..20 {
            let current = service
                .model_runtime_service
                .get_model_status(None)
                .await
                .unwrap();
            if current.health_ok {
                status = Some(current);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        let status = status.expect("model never became healthy");
        assert!(status.server_running);
        assert!(status.health_ok);
        assert!(status.started_by_app || status.server_running);
    }

    struct MockPdfService {
        image_path: std::path::PathBuf,
    }
    impl PdfService for MockPdfService {
        fn page_count(&self, _pdf_path: &std::path::Path) -> Result<u32, AppError> {
            Ok(1)
        }
        fn render_pages(
            &self,
            _pdf_path: &std::path::Path,
            _output_dir: &std::path::Path,
            _pages: &[u32],
        ) -> Result<Vec<std::path::PathBuf>, AppError> {
            Ok(vec![self.image_path.clone()])
        }
        fn render_all_pages(
            &self,
            _pdf_path: &std::path::Path,
            _output_dir: &std::path::Path,
        ) -> Result<Vec<std::path::PathBuf>, AppError> {
            Ok(vec![self.image_path.clone()])
        }
        fn get_renderer_status(
            &self,
        ) -> Result<crate::services::pdf_service::PdfRendererStatus, AppError> {
            Ok(crate::services::pdf_service::PdfRendererStatus {
                available: true,
                backend: "mock".to_string(),
                pdfinfo_path: None,
                pdftoppm_path: None,
                searched_paths: vec![],
                path_env: None,
                install_hint: None,
                warnings: vec![],
            })
        }
    }

    fn spawn_crashing_test_server() -> (String, String, u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("crashing rubric test server requires a loopback TCP listener");
        let addr = listener
            .local_addr()
            .expect("crashing rubric test server listener must have a local address");
        let port = addr.port();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(_) => break,
                };
                let mut buffer = [0u8; 1024];
                let read_len = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read_len]);
                if request.contains("POST /v1/chat/completions") {
                    if request.contains("Reply with exactly one word") {
                        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}";
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                        continue;
                    }
                    drop(stream);
                    break;
                }
                let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (
            format!("http://{}", addr),
            "127.0.0.1".to_string(),
            port,
            handle,
        )
    }

    #[tokio::test]
    async fn test_run_import_crashed_diagnostics() {
        let _model_runtime_guard = lock_model_runtime_test().await;
        let root = temp_project_root();
        let store = ProjectStore::new();

        let mut project = store
            .create_project("Test Project".to_string(), root)
            .unwrap();
        project.questions.push(crate::domain::question::Question {
            assessment_activity_id: None,
            id: "q-1".to_string(),
            number: 1,
            max_score: 10.0,
            answer_type: crate::domain::question::AnswerType::GeneralText,
            question_text: crate::domain::question::TextFieldState {
                value: "Soru 1".to_string(),
                source: crate::domain::question::TextFieldSource::Manual,
                status: crate::domain::question::TextFieldStatus::Confirmed,
                confidence: None,
                warnings: vec![],
                updated_at: None,
            },
            rubric: crate::domain::rubric::RubricState {
                expected_answer: None,
                key_concepts: vec![],
                criteria: vec![],
                partial_credit_hints: vec![],
                zero_score_conditions: vec![],
                common_mistakes: vec![],
                source: Some(crate::domain::rubric::RubricSource::Manual),
                status: crate::domain::rubric::RubricStatus::Missing,
                warnings: vec![],
                updated_at: None,
                max_score: None,
            },
            crop_template: None,
        });

        let doc_id = "rubric-doc-123".to_string();
        let document = crate::domain::document::Document {
            id: doc_id.clone(),
            role: DocumentRole::Rubric,
            file_name: "rubrik.pdf".to_string(),
            stored_path: "rubrik.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        };
        project.documents.push(document);
        store.save_project(&project).unwrap();

        let doc_dir = std::path::Path::new(&project.root_path).join("documents");
        std::fs::create_dir_all(&doc_dir).unwrap();
        std::fs::write(doc_dir.join("rubrik.pdf"), "fake content").unwrap();

        let page_img_path = doc_dir.join(format!("page-{}.png", uuid::Uuid::new_v4()));
        let img = image::ImageBuffer::from_pixel(100, 100, image::Rgb([0, 0, 0]));
        let dynamic_img = image::DynamicImage::ImageRgb8(img);
        dynamic_img.save(&page_img_path).unwrap();

        let config_path = std::env::temp_dir().join(format!(
            "rubrika-model-config-{}.json",
            uuid::Uuid::new_v4()
        ));
        let model_config = ModelConfigService::new_with_path(config_path);

        let (base_url, host, port, _server_handle) = spawn_crashing_test_server();

        let dummy_server_path =
            std::env::temp_dir().join(format!("dummy-llama-server-{}", uuid::Uuid::new_v4()));
        std::fs::File::create(&dummy_server_path).unwrap();

        let profile = ModelProfile {
            id: format!("test-gemma-crash-{}", uuid::Uuid::new_v4()),
            display_name: "Test Gemma".to_string(),
            mode: ModelMode::External,
            host,
            port,
            base_url,
            server_path: dummy_server_path.to_string_lossy().to_string(),
            model_path: dummy_server_path.to_string_lossy().to_string(),
            mmproj_path: dummy_server_path.to_string_lossy().to_string(),
            runtime_preset: crate::domain::model::ModelRuntimePreset::Standard,
            privacy_mode: crate::domain::model::PrivacyMode::StrictLocal,
        };
        model_config.update_profile(profile).unwrap();

        let model_gateway_impl = std::sync::Arc::new(LlamaServerGateway::new(
            model_config.get_profile(None).unwrap().base_url,
        ));
        let model_process_manager = ModelProcessManager::new_with_state_path(
            model_config.clone(),
            model_gateway_impl.clone(),
            std::env::temp_dir().join(format!("rubrika-model-state-{}.json", uuid::Uuid::new_v4())),
        );
        let model_runtime_service =
            ModelRuntimeService::new(model_config, model_process_manager.clone());

        let mock_pdf_service = std::sync::Arc::new(MockPdfService {
            image_path: page_img_path.clone(),
        });

        let service = RubricExtractionService::new(
            store,
            model_gateway_impl,
            std::sync::Arc::new(JobManager::new()),
            model_runtime_service,
            mock_pdf_service,
            std::sync::Arc::new(DocumentContentExtractionService::new(std::sync::Arc::new(
                ModelInputImageService::default(),
            ))),
        );

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
            .handle()
            .clone();

        let result = service
            .run_import(app, "test-job-id".to_string(), &project.id, Some(doc_id), 1)
            .await;

        let _ = std::fs::remove_file(dummy_server_path);
        let _ = std::fs::remove_file(page_img_path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, AppErrorCode::ModelServerCrashedDuringRequest);
        assert!(err.message.contains("rubrik çıkarma sırasında kapandı"));

        let details = err.technical_details.unwrap();
        assert!(details.contains("request_kind = RubricExtraction"));
        assert!(details.contains("model_status_before_model_request"));
        assert!(details.contains("model_status_after_failure"));
    }

    #[tokio::test]
    async fn proof_6_rubric_cancel_preserves_existing_state() {
        use crate::domain::job::{DuplicatePolicy, JobStatus};
        use crate::jobs::job_manager::JobRegistrationInput;

        let root_path_buf =
            std::env::temp_dir().join(format!("rubrika-test-p6-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root_path_buf).unwrap();
        let store = ProjectStore::new();
        let mut project = store
            .create_project(
                "proj_p6".into(),
                root_path_buf.to_string_lossy().to_string(),
            )
            .unwrap();

        let doc_id = "rubric-doc-p6".to_string();
        let document = crate::domain::document::Document {
            id: doc_id.clone(),
            role: DocumentRole::Rubric,
            file_name: "rubrik.pdf".to_string(),
            stored_path: "rubrik.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        };
        project.documents.push(document);

        let initial_rubric = crate::domain::rubric::RubricState {
            status: RubricStatus::Confirmed,
            source: Some(RubricSource::Manual),
            max_score: Some(10.0),
            expected_answer: Some("Teacher answer".to_string()),
            key_concepts: vec![],
            criteria: vec![],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };

        project.questions.push(crate::domain::question::Question {
            assessment_activity_id: None,
            id: "q1-p6".to_string(),
            number: 1,
            max_score: 10.0,
            answer_type: crate::domain::question::AnswerType::GeneralText,
            question_text: crate::domain::question::TextFieldState {
                value: "Question 1?".to_string(),
                status: crate::domain::question::TextFieldStatus::Confirmed,
                source: crate::domain::question::TextFieldSource::Manual,
                confidence: None,
                warnings: vec![],
                updated_at: None,
            },
            rubric: initial_rubric.clone(),
            crop_template: None,
        });

        store.save_project(&project).unwrap();

        let jm = std::sync::Arc::new(JobManager::new());
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
                    kind: JobKind::RubricPdfImport,
                    display_label: Some("Rubric Import".into()),
                    total: 1,
                    message: "Extracting".into(),
                    correlation_id: Some("corr-p6".into()),
                    idempotency_key: Some("key-p6".into()),
                    duplicate_policy: DuplicatePolicy::ReturnExisting,
                    cancellable: true,
                    retry_of_job_id: None,
                },
            )
            .unwrap();

        // Request cancellation
        jm.cancel_job(&app, &reg.snapshot.id).unwrap();

        let mock_pdf_service = std::sync::Arc::new(MockPdfService {
            image_path: root_path_buf.join("dummy.png"),
        });

        let config_path = root_path_buf.join("model-config.json");
        let model_config = ModelConfigService::new_with_path(config_path);
        let model_gateway_impl =
            std::sync::Arc::new(LlamaServerGateway::new("http://localhost:8080".to_string()));
        let model_process_manager = ModelProcessManager::new_with_state_path(
            model_config.clone(),
            model_gateway_impl.clone(),
            root_path_buf.join("model-state.json"),
        );
        let model_runtime_service = ModelRuntimeService::new(model_config, model_process_manager);

        let service = RubricExtractionService::new(
            store.clone(),
            model_gateway_impl,
            jm.clone(),
            model_runtime_service,
            mock_pdf_service,
            std::sync::Arc::new(DocumentContentExtractionService::new(std::sync::Arc::new(
                ModelInputImageService::default(),
            ))),
        );

        let res = service
            .run_import(app, reg.snapshot.id.clone(), &project.id, Some(doc_id), 1)
            .await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code, AppErrorCode::JobCancelled);

        let snap = jm.get_job_snapshot(&reg.snapshot.id).unwrap();
        assert_eq!(snap.status, JobStatus::Cancelled);

        // Verify teacher rubric remains intact and confirmed
        let updated = store.get_project_snapshot(project.id).unwrap();
        assert_eq!(updated.questions[0].rubric.status, RubricStatus::Confirmed);
        assert_eq!(
            updated.questions[0].rubric.expected_answer,
            Some("Teacher answer".to_string())
        );
    }

    // ------------------------------------------------------------------
    // TD-20 rubric parse retry chain tests (fake gateway + artifacts)
    // ------------------------------------------------------------------

    fn rubric_image(page_number: u32) -> ModelInputImage {
        ModelInputImage {
            kind: crate::domain::model::ModelInputImageKind::Rubric,
            document_id: "doc-1".to_string(),
            page_number,
            source_image_path: format!("src-{page_number}.jpg"),
            output_image_path: format!("out-{page_number}.jpg"),
            source_width: 100,
            source_height: 100,
            output_width: 100,
            output_height: 100,
            source_bytes: 0,
            output_bytes: 0,
            base64_approx_bytes: 0,
            long_edge_max: 2000,
            jpeg_quality: 92,
            created_at: "now".to_string(),
            source_sha256: None,
            output_sha256: None,
            cache_key: None,
            cache_transaction_id: None,
            cache_hit: false,
        }
    }

    fn rubric_ok_result() -> crate::domain::model::RubricExtractionResult {
        crate::domain::model::RubricExtractionResult {
            output: crate::domain::model::RubricExtractionOutput {
                questions: vec![crate::domain::model::ExtractedRubricCandidate {
                    number: 1,
                    max_points: Some(10.0),
                    expected_answer: Some("A".to_string()),
                    key_concepts: vec![],
                    criteria: vec![],
                    partial_credit_hints: vec![],
                    zero_score_conditions: vec![],
                    common_mistakes: vec![],
                    confidence: 1.0,
                    warnings: vec![],
                }],
                document_warnings: vec![],
            },
            raw_response: "raw".to_string(),
            diagnostics: crate::domain::model::ModelDiagnostics {
                endpoint: "".to_string(),
                request_kind: crate::domain::model::ModelRequestKind::RubricDraft,
                http_status: None,
                duration_ms: 0,
                prompt_length: None,
                image_count: Some(1),
                image_total_bytes: None,
                base64_approx_total_bytes: None,
                model_input_images: vec![],
                timeout_seconds: None,
                max_tokens: None,
                finish_reason: None,
                content_length: None,
                reasoning_content_length: None,
                raw_text_stored_path: None,
                error_code: None,
                provenance: None,
            },
            retry_metadata: None,
        }
    }

    fn rubric_parse_error(code: AppErrorCode) -> AppError {
        AppError {
            code,
            message: "parse error".to_string(),
            recoverable: true,
            suggested_action: None,
            technical_details: None,
            correlation_id: "corr".to_string(),
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct RubricCallRecord {
        attempt: u32,
        image_count: usize,
        has_raw_text: bool,
        strict_json_only: bool,
    }

    struct RubricRecordingGateway {
        responses: std::sync::Mutex<
            std::collections::VecDeque<
                Result<crate::domain::model::RubricExtractionResult, AppError>,
            >,
        >,
        calls: std::sync::Mutex<Vec<RubricCallRecord>>,
    }

    impl RubricRecordingGateway {
        fn new(
            responses: Vec<Result<crate::domain::model::RubricExtractionResult, AppError>>,
        ) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses.into()),
                calls: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::services::model_gateway::ModelGateway for RubricRecordingGateway {
        async fn get_status(&self) -> Result<crate::domain::model::ModelStatus, AppError> {
            Ok(crate::domain::model::ModelStatus::default())
        }
        async fn probe_server(&self) -> Result<crate::domain::model::ModelStatus, AppError> {
            Ok(crate::domain::model::ModelStatus::default())
        }
        async fn health_status(
            &self,
            _base_url: &str,
        ) -> Result<crate::domain::model::ModelStatus, AppError> {
            Ok(crate::domain::model::ModelStatus::default())
        }
        async fn probe_status(
            &self,
            _base_url: &str,
        ) -> Result<crate::domain::model::ModelStatus, AppError> {
            Ok(crate::domain::model::ModelStatus::default())
        }
        async fn extract_question_text(
            &self,
            _input: crate::domain::model::QuestionTextExtractionRequest,
        ) -> Result<crate::domain::model::QuestionTextExtractionResult, AppError> {
            unreachable!()
        }
        async fn draft_rubric(
            &self,
            input: RubricExtractionRequest,
        ) -> Result<crate::domain::model::RubricExtractionResult, AppError> {
            self.calls.lock().unwrap().push(RubricCallRecord {
                attempt: input.attempt,
                image_count: input.model_input_images.len(),
                has_raw_text: input.raw_text.is_some(),
                strict_json_only: input.strict_json_only,
            });
            match self.responses.lock().unwrap().pop_front() {
                Some(response) => response,
                None => Err(rubric_parse_error(AppErrorCode::RubricJsonParseFailed)),
            }
        }
        async fn extract_student_answer_ocr(
            &self,
            _input: crate::domain::model::StudentAnswerOcrRequest,
        ) -> Result<crate::domain::model::StudentAnswerOcrResult, AppError> {
            unreachable!()
        }
        async fn suggest_student_answer_issue_correction(
            &self,
            _input: crate::domain::model::StudentAnswerOcrIssueCorrectionRequest,
        ) -> Result<crate::domain::model::StudentAnswerOcrIssueCorrectionResult, AppError> {
            unreachable!()
        }
        async fn extract_student_identity_ocr(
            &self,
            _input: crate::domain::model::StudentIdentityOcrRequest,
        ) -> Result<crate::domain::model::StudentIdentityOcrResult, AppError> {
            unreachable!()
        }
        async fn cleanup_speaking_transcript(
            &self,
            _input: crate::domain::model::SpeakingTranscriptCleanupRequest,
        ) -> Result<crate::domain::model::SpeakingTranscriptCleanupResult, AppError> {
            unreachable!()
        }
        async fn generate_analysis_report(
            &self,
            _input: crate::domain::model::AnalysisReportRequest,
        ) -> Result<crate::domain::model::AnalysisReportResult, AppError> {
            unreachable!()
        }
        async fn score_answer(
            &self,
            _input: crate::domain::model::ScoringRequest,
        ) -> Result<crate::domain::model::ScoringResult, AppError> {
            unreachable!()
        }
    }

    fn rubric_request_for_retry(project_root: &str) -> RubricExtractionRequest {
        let prompt_contract = build_prompt_contract(
            crate::domain::model::ModelRequestKind::RubricDraft,
            "rubric_extraction_v2_typed_user_data",
            crate::domain::rubric::RUBRIC_EXTRACTION_SCHEMA_VERSION,
            "rubric_extraction_policy_v1",
            "prompt".to_string(),
            serde_json::json!({"targetQuestionNumber": 1, "expectedQuestionCount": 1}),
            default_sampling(8192),
            Some(crate::domain::model::ModelResponseFormat::JsonSchema {
                name: "rubric_extraction_suggestion".to_string(),
                schema: crate::domain::rubric::canonical_rubric_extraction_schema(),
            }),
            None,
        );
        RubricExtractionRequest {
            prompt: "prompt".to_string(),
            prompt_contract: Some(prompt_contract),
            raw_text: None,
            image_path: Some("out-1.jpg".to_string()),
            target_question_number: 1,
            model_input_images: vec![rubric_image(1)],
            strict_json_only: false,
            attempt: 1,
            project_root_path: Some(project_root.to_string()),
            job_id: Some("job_retry_q1".to_string()),
        }
    }

    fn write_rubric_attempt_raw_response(project_root: &str, content: &str) {
        let dir = std::path::Path::new(project_root)
            .join("logs")
            .join("model_responses")
            .join("rubric_import")
            .join("job_retry")
            .join("question_1")
            .join("attempt_1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("response_raw.txt"), content).unwrap();
    }

    fn service_for_retry_tests(
        project_store: ProjectStore,
        model_gateway: std::sync::Arc<dyn crate::services::model_gateway::ModelGateway>,
    ) -> RubricExtractionService {
        let config_path = std::env::temp_dir().join(format!(
            "rubrika-model-config-retry-{}.json",
            uuid::Uuid::new_v4()
        ));
        let model_config = ModelConfigService::new_with_path(config_path);
        let runtime_gateway = std::sync::Arc::new(LlamaServerGateway::default());
        let model_process_manager = ModelProcessManager::new_with_state_path(
            model_config.clone(),
            runtime_gateway,
            std::env::temp_dir().join(format!(
                "rubrika-model-state-retry-{}.json",
                uuid::Uuid::new_v4()
            )),
        );
        let model_runtime_service = ModelRuntimeService::new(model_config, model_process_manager);
        RubricExtractionService::new(
            project_store,
            model_gateway,
            std::sync::Arc::new(JobManager::new()),
            model_runtime_service,
            std::sync::Arc::new(crate::services::pdf_service::SystemPdfService),
            std::sync::Arc::new(DocumentContentExtractionService::new(std::sync::Arc::new(
                ModelInputImageService::default(),
            ))),
        )
    }

    #[tokio::test]
    async fn rubric_retry_salvages_response_without_second_vision_call() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        store.create_project("p".to_string(), root.clone()).unwrap();
        // Truncated JSON: full parse fails but the single question item is
        // deterministically salvageable.
        write_rubric_attempt_raw_response(
            &root,
            r#"{"questions":[{"questionNumber":1,"maxPoints":10,"expectedAnswer":"A"}]"#,
        );
        let gateway = std::sync::Arc::new(RubricRecordingGateway::new(vec![Err(
            rubric_parse_error(AppErrorCode::RubricJsonParseFailed),
        )]));
        let service = service_for_retry_tests(store, gateway.clone());
        let req = rubric_request_for_retry(&root);
        let result = service.draft_rubric_with_retry(req).await.unwrap();
        let calls = gateway.calls.lock().unwrap().clone();
        // Salvage success -> no second model call at all.
        assert_eq!(calls.len(), 1);
        let meta = result.retry_metadata.expect("retry metadata");
        assert!(meta.salvage_used);
        assert!(!meta.text_only_repair_used);
        assert_eq!(result.output.questions.len(), 1);
        assert_eq!(result.output.questions[0].number, 1);
    }

    #[tokio::test]
    async fn rubric_retry_uses_text_only_repair_without_resending_images() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        store.create_project("p".to_string(), root.clone()).unwrap();
        // Malformed prose: not salvageable, but non-empty so text-only repair
        // is attempted instead of resending the images.
        write_rubric_attempt_raw_response(&root, "model produced prose, no questions array here");
        let gateway = std::sync::Arc::new(RubricRecordingGateway::new(vec![
            Err(rubric_parse_error(AppErrorCode::RubricJsonParseFailed)),
            Ok(rubric_ok_result()),
        ]));
        let service = service_for_retry_tests(store, gateway.clone());
        let req = rubric_request_for_retry(&root);
        let result = service.draft_rubric_with_retry(req).await.unwrap();
        let calls = gateway.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        // Second call is text-only JSON repair: no images, strict JSON.
        assert_eq!(calls[1].image_count, 0);
        assert!(calls[1].has_raw_text);
        assert!(calls[1].strict_json_only);
        assert_eq!(calls[1].attempt, 2);
        let meta = result.retry_metadata.expect("retry metadata");
        assert!(meta.text_only_repair_used);
        assert!(!meta.salvage_used);
    }

    #[tokio::test]
    async fn rubric_retry_multimodal_resend_only_with_explicit_reason() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        store.create_project("p".to_string(), root.clone()).unwrap();
        // Neither salvage nor text-only repair succeed -> multimodal retry is
        // the explicit last resort.
        write_rubric_attempt_raw_response(&root, "prose not repairable by JSON");
        let gateway = std::sync::Arc::new(RubricRecordingGateway::new(vec![
            Err(rubric_parse_error(AppErrorCode::RubricJsonParseFailed)),
            Err(rubric_parse_error(AppErrorCode::RubricJsonParseFailed)),
            Ok(rubric_ok_result()),
        ]));
        let service = service_for_retry_tests(store, gateway.clone());
        let req = rubric_request_for_retry(&root);
        let result = service.draft_rubric_with_retry(req).await.unwrap();
        let calls = gateway.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        // Third call re-sends images (multimodal) and is strict.
        assert_eq!(calls[2].image_count, 1);
        assert!(calls[2].strict_json_only);
        assert_eq!(calls[2].attempt, 3);
        let meta = result.retry_metadata.expect("retry metadata");
        assert_eq!(meta.attempts, 3);
        assert_eq!(
            meta.retry_reason.as_deref(),
            Some("text_only_repair_failed:RubricJsonParseFailed")
        );
    }

    #[tokio::test]
    async fn rubric_retry_empty_first_response_records_explicit_reason() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        store.create_project("p".to_string(), root.clone()).unwrap();
        write_rubric_attempt_raw_response(&root, "   \n  ");
        let gateway = std::sync::Arc::new(RubricRecordingGateway::new(vec![
            Err(rubric_parse_error(AppErrorCode::ModelResponseEmpty)),
            Ok(rubric_ok_result()),
        ]));
        let service = service_for_retry_tests(store, gateway.clone());
        let req = rubric_request_for_retry(&root);
        let result = service.draft_rubric_with_retry(req).await.unwrap();
        let calls = gateway.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].image_count, 1);
        let meta = result.retry_metadata.expect("retry metadata");
        assert_eq!(meta.retry_reason.as_deref(), Some("first_response_empty"));
    }

    #[tokio::test]
    async fn rubric_retry_text_only_request_preserves_existing_strict_resend() {
        // The text-based path has no images: on parse failure it must keep
        // resending the text (strict) and never try the vision repair tiers.
        let root = temp_project_root();
        let store = ProjectStore::new();
        store.create_project("p".to_string(), root.clone()).unwrap();
        let gateway = std::sync::Arc::new(RubricRecordingGateway::new(vec![
            Err(rubric_parse_error(AppErrorCode::RubricJsonParseFailed)),
            Ok(rubric_ok_result()),
        ]));
        let service = service_for_retry_tests(store, gateway.clone());
        let mut req = rubric_request_for_retry(&root);
        req.raw_text = Some("raw rubric text".to_string());
        req.model_input_images = vec![];
        req.image_path = None;
        let result = service.draft_rubric_with_retry(req).await.unwrap();
        let calls = gateway.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].image_count, 0);
        assert!(calls[1].strict_json_only);
        let meta = result.retry_metadata.expect("retry metadata");
        // The text path falls straight to the explicit-reason strict resend.
        assert!(meta.retry_reason.is_some());
    }
}

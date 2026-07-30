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
use serde::{Deserialize, Serialize};
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
            let _ = service.model_runtime_service.stop_server(None).await;
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
                    let _ = service.project_store.save_project(&proj);
                }
            } else {
                if let Ok(mut proj) = service
                    .project_store
                    .get_project_snapshot(project_id.clone())
                {
                    proj.workflow = crate::services::workflow_engine::evaluate_workflow(&proj);
                    let _ = service.project_store.save_project(&proj);
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
        let _ = self.job_manager.set_running(&app, &job_id);
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
            "Gemma model sunucusu kontrol ediliyor...".to_string(),
        );

        let model_status_at_start = self
            .model_runtime_service
            .get_model_status(None)
            .await
            .unwrap_or_default();
        if !model_status_at_start.server_running || !model_status_at_start.health_ok {
            let _ = self.job_manager.update_progress(
                &app,
                &job_id,
                0,
                expected_question_count,
                "Gemma model sunucusu başlatılıyor...".to_string(),
            );
        }
        let _model_status_ready = self
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
        let prompt = r#"
Sen deneyimli bir öğretmensin. Verilen metin veya görsel bir sınavın cevap anahtarı veya rubriği (puanlama rehberi).
Verilen içerikten her bir soru için puanlama kriterlerini, beklenen cevapları ve maksimum puanları çıkarman gerekiyor.

Aşağıdaki JSON formatında kesin ve eksiksiz bir yanıt dön:
```json
{
  "questions": [
    {
      "number": 1,
      "max_points": 10.0,
      "expected_answer": "Türkiye'nin başkenti Ankara'dır.",
      "criteria": [
        {
          "label": "Doğru Şehir",
          "points": 10.0,
          "description": "Ankara cevabı tam puan alır."
        }
      ],
      "confidence": 0.95,
      "warnings": []
    }
  ],
  "document_warnings": []
}
```
Kurallar:
- Soru numaralarını doğru tespit et.
- Max puanları ve beklenen cevapları bul. Bulamazsan max_points'i null yap ve warnings listesine "Max puan bulunamadı" ekle.
- Asla halüsinasyon yapma (metinde olmayan bir şeyi uydurma).
- Çıktı sadece ve sadece geçerli bir JSON olmalıdır.
"#.to_string();

        let _ = self.job_manager.update_progress(
            &app,
            &job_id,
            2,
            3,
            "Modelden rubrik verisi çekiliyor...".to_string(),
        );

        let start_time = std::time::Instant::now();
        let model_status_before = self
            .model_runtime_service
            .get_model_status(None)
            .await
            .unwrap_or_default();

        let mut page_count = 1;
        let mut extraction_method = if is_text_based {
            "pdftotext"
        } else {
            "vision_fallback_prepared"
        };
        let mut image_paths = Vec::new();
        let mut image_dimensions = Vec::new();
        let mut image_byte_sizes = Vec::new();

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
                    let req = RubricExtractionRequest {
                        prompt: build_rubric_question_prompt(
                            &prompt,
                            question_number,
                            expected_question_count,
                            question_text,
                            question_text_status,
                        ),
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
                        },
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
                    let req = RubricExtractionRequest {
                        prompt: build_rubric_question_prompt(
                            &prompt,
                            question_number,
                            expected_question_count,
                            question_text,
                            question_text_status,
                        ),
                        raw_text: None,
                        image_path: fallback_image_path.clone(),
                        target_question_number: question_number,
                        model_input_images: all_prepared_inputs.clone(),
                        strict_json_only: false,
                        attempt: 1,
                        project_root_path: Some(project.root_path.clone()),
                        job_id: Some(format!("{job_id}_q{question_number}")),
                    };

                    let page_res_result = self.draft_rubric_with_retry(req).await;
                    match page_res_result {
                        Ok(page_res) => {
                            let mut found = false;
                            for q in page_res.output.questions {
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
                            merged_warnings.extend(page_res.output.document_warnings);
                        }
                        Err(e) => {
                            failed_questions.push(question_number);
                            merged_warnings
                                .push(format!("question_{question_number}_failed: {:?}", e.code));
                            last_error = Some(e);
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
                        },
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
                     model_status_at_start = {:?}\n\
                     model_status_before_model_request = {:?}\n\
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
                    model_status_at_start,
                    model_status_before,
                    model_status_after,
                    model_status_before.base_url,
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
                    model_status_before.profile_id,
                    model_status_after
                        .log_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    highlights,
                    original_error.technical_details
                );

                original_error.technical_details = Some(details);

                if model_status_before.server_running {
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
                    }
                } else if original_error.code == AppErrorCode::ModelTimeout {
                    original_error.code = AppErrorCode::ModelRequestTimeout;
                }

                return Err(original_error);
            }
        };

        let _ = self.job_manager.update_progress(
            &app,
            &job_id,
            expected_question_count,
            expected_question_count,
            "Rubrikler kaydediliyor...".to_string(),
        );

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
                question.rubric.status = RubricStatus::Imported;
                question.rubric.source = Some(RubricSource::RubricPdf);
                if let Some(mp) = q_cand.max_points {
                    question.rubric.max_score = Some(mp);
                }
                if let Some(ea) = q_cand.expected_answer {
                    question.rubric.expected_answer = Some(ea);
                }
                if !q_cand.criteria.is_empty() {
                    question.rubric.criteria = q_cand.criteria;
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
            self.project_store.save_project(&project)?;
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
        self.project_store.save_project(&project)?;

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

impl RubricExtractionService {
    async fn draft_rubric_with_retry(
        &self,
        request: RubricExtractionRequest,
    ) -> Result<crate::domain::model::RubricExtractionResult, AppError> {
        let first_attempt = self.model_gateway.draft_rubric(request.clone()).await;
        match first_attempt {
            Ok(result) => Ok(result),
            Err(first_error) if should_retry_rubric_parse(&first_error) => {
                let mut retry_request = request;
                retry_request.strict_json_only = true;
                retry_request.attempt = 2;
                let retry_result = self.model_gateway.draft_rubric(retry_request).await;
                match retry_result {
                    Ok(result) => Ok(result),
                    Err(mut retry_error) => {
                        let details = format!(
                            "retry_attempted=true\nfirst_error_code={:?}\nfirst_error_message={}\nfirst_error_details={:?}\nretry_error_code={:?}\nretry_error_message={}\nretry_error_details={:?}",
                            first_error.code,
                            first_error.message,
                            first_error.technical_details,
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
    let q_text_str = if question_text_status == "missing" || question_text_status == "failed" {
        "Hedef Sorunun Metni: [Eksik] (questionTextStatus=\"missing\")".to_string()
    } else {
        match question_text {
            Some(text) if !text.trim().is_empty() => format!("Hedef Sorunun Metni: \"{}\"", text),
            _ => "Hedef Sorunun Metni: [Bilinmiyor / Eksik] (questionTextStatus=\"missing\")"
                .to_string(),
        }
    };
    format!(
        "{}\n\n{}\nSadece {} numaralı soruyu işle. Toplam soru sayısı: {}. Başka soru döndürme. JSON içinde yalnızca bu soru yer alsın.",
        base_prompt.trim(),
        q_text_str,
        question_number,
        expected_question_count
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
        let path = env::temp_dir().join(format!("rubrika-mock-rubric-{}.sh", uuid::Uuid::new_v4()));
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
from http.server import BaseHTTPRequestHandler, HTTPServer

host = sys.argv[1]
port = int(sys.argv[2])

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

HTTPServer((host, port), Handler).serve_forever()
PY
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
            mode: ModelMode::External,
            server_path: server_path.to_string_lossy().to_string(),
            model_path: server_path.to_string_lossy().to_string(),
            mmproj_path: server_path.to_string_lossy().to_string(),
            host: "127.0.0.1".to_string(),
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            runtime_preset: crate::domain::model::ModelRuntimePreset::Standard,
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
        assert_eq!(status.mode, ModelMode::Managed);
        assert!(status.server_running);
        assert!(status.health_ok);
        assert!(status.started_by_app);
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

        let page_img_path = std::env::temp_dir().join(format!("page-{}.png", uuid::Uuid::new_v4()));
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
            model_path: "".to_string(),
            mmproj_path: "".to_string(),
            runtime_preset: crate::domain::model::ModelRuntimePreset::Standard,
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
}

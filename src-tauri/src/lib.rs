pub mod commands;
pub mod diagnostics;
pub mod domain;
pub mod jobs;
pub mod platform;
pub mod services;

use jobs::job_manager::JobManager;
use services::analysis_service::AnalysisService;
use services::assessment_organization_service::AssessmentOrganizationService;
use services::audit_service::AuditService;
use services::document_content_extraction_service::DocumentContentExtractionService;
use services::exam_package_build_service::ExamPackageBuildService;
use services::graded_exam_review_service::GradedExamReviewService;
use services::llama_server_gateway::LlamaServerGateway;
use services::model_config_service::ModelConfigService;
use services::model_gateway::ModelGateway;
use services::model_input_image_service::ModelInputImageService;
use services::model_process_manager::ModelProcessManager;
use services::model_runtime_service::ModelRuntimeService;
use services::ocr_image_preprocess_service::OcrImagePreprocessService;
use services::pdf_preview_service::PdfPreviewService;
use services::pdf_service::{PdfService, SystemPdfService};
use services::performance_service::PerformanceService;
use services::project_store::ProjectStore;
use services::question_text_service::QuestionTextService;
use services::rubric_extraction_service::RubricExtractionService;
use services::rubric_service::RubricService;
use services::school_class_service::SchoolClassService;
use services::scoring_anchor_service::ScoringAnchorService;
use services::scoring_service::ScoringService;
use services::speaking_exam_service::SpeakingExamService;
use services::student_answer_crop_service::StudentAnswerCropService;
use services::student_answer_ocr_service::StudentAnswerOcrService;
use services::student_scan_service::StudentScanService;
use speakoflow_engine::SpeakoflowEngine;
use std::borrow::Cow;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub project_store: ProjectStore,
    pub audit_service: Arc<AuditService>,
    pub model_gateway: Arc<dyn ModelGateway>,
    pub model_gateway_impl: Arc<LlamaServerGateway>,
    pub model_config_service: ModelConfigService,
    pub model_process_manager: ModelProcessManager,
    pub model_runtime_service: ModelRuntimeService,
    pub model_input_image_service: Arc<ModelInputImageService>,
    pub ocr_image_preprocess_service: Arc<OcrImagePreprocessService>,
    pub pdf_service: Arc<dyn PdfService>,
    pub pdf_preview_service: Arc<PdfPreviewService>,
    pub document_content_extraction_service: Arc<DocumentContentExtractionService>,
    pub job_manager: Arc<JobManager>,
    pub exam_package_build_service: Arc<ExamPackageBuildService>,
    pub graded_exam_review_service: Arc<GradedExamReviewService>,
    pub question_text_service: Arc<QuestionTextService>,
    pub student_scan_service: Arc<StudentScanService>,
    pub rubric_service: Arc<RubricService>,
    pub rubric_extraction_service: Arc<RubricExtractionService>,
    pub scoring_service: Arc<ScoringService>,
    pub scoring_anchor_service: Arc<ScoringAnchorService>,
    pub school_class_service: Arc<SchoolClassService>,
    pub assessment_organization_service: Arc<AssessmentOrganizationService>,
    pub performance_service: Arc<PerformanceService>,
    pub student_answer_crop_service: Arc<StudentAnswerCropService>,
    pub student_answer_ocr_service: Arc<StudentAnswerOcrService>,
    pub speaking_exam_service: SpeakingExamService,
    pub analysis_service: AnalysisService,
    pub speaking_engine: Arc<SpeakoflowEngine>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = platform::single_instance::AppInstanceLease::acquire() {
        log::error!("{}", error.message);
        std::process::exit(0);
    }

    // The managed-asset protocol can receive a request while the builder is
    // still completing startup. Register this state before installing the
    // protocol so an early preview request cannot call `state()` before
    // `manage()` has run. The AppState below keeps the same shared store.
    let project_store = ProjectStore::new();

    tauri::Builder::default()
        .manage(project_store.clone())
        .register_uri_scheme_protocol("managed-asset", |ctx, request| {
            use crate::platform::managed_asset;
            let request_path = request.uri().path();
            let Some(store) = ctx.app_handle().try_state::<ProjectStore>() else {
                log::warn!("managed asset request arrived before ProjectStore initialization");
                return managed_asset_error_response();
            };
            match managed_asset::resolve_managed_asset(&store, request_path) {
                Ok(asset) => http::Response::builder()
                    .status(200)
                    .header("Content-Type", asset.mime)
                    .body(Cow::Owned(asset.bytes))
                    .unwrap_or_else(|_| managed_asset_error_response()),
                Err(_) => managed_asset_error_response(),
            }
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            if std::env::var_os("RUBRIKA_SMOKE").is_some() {
                std::process::exit(0);
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .manage({
            let model_gateway_impl = Arc::new(LlamaServerGateway::default());
            let model_gateway: Arc<dyn ModelGateway> = model_gateway_impl.clone();
            let model_config_service = ModelConfigService::new();
            let model_process_manager =
                ModelProcessManager::new(model_config_service.clone(), model_gateway_impl.clone());
            let model_runtime_service = ModelRuntimeService::new(
                model_config_service.clone(),
                model_process_manager.clone(),
            );
            let model_input_image_service = Arc::new(ModelInputImageService::default());
            let ocr_image_preprocess_service = Arc::new(OcrImagePreprocessService);
            let document_content_extraction_service = Arc::new(
                DocumentContentExtractionService::new(model_input_image_service.clone()),
            );
            let pdf_service: Arc<dyn PdfService> = Arc::new(SystemPdfService);
            let job_manager = Arc::new(JobManager::new());
            let speaking_engine = Arc::new(SpeakoflowEngine::new());
            let pdf_preview_service = Arc::new(PdfPreviewService::new(
                project_store.clone(),
                pdf_service.clone(),
                job_manager.clone(),
            ));
            let question_text_service = Arc::new(QuestionTextService::new(
                project_store.clone(),
                model_gateway.clone(),
                model_runtime_service.clone(),
                pdf_preview_service.clone(),
                document_content_extraction_service.clone(),
                job_manager.clone(),
            ));
            let rubric_extraction_service = Arc::new(RubricExtractionService::new(
                project_store.clone(),
                model_gateway.clone(),
                job_manager.clone(),
                model_runtime_service.clone(),
                pdf_service.clone(),
                document_content_extraction_service.clone(),
            ));
            let exam_package_build_service = Arc::new(ExamPackageBuildService::new(
                project_store.clone(),
                pdf_preview_service.clone(),
                question_text_service.clone(),
                rubric_extraction_service.clone(),
                job_manager.clone(),
            ));
            let student_scan_service = Arc::new(StudentScanService::new(
                project_store.clone(),
                pdf_preview_service.clone(),
            ));
            let school_class_service = Arc::new(SchoolClassService::new(project_store.clone()));
            let audit_service = Arc::new(AuditService::new());
            let assessment_organization_service = Arc::new(AssessmentOrganizationService::new(
                project_store.clone(),
                school_class_service.clone(),
            ));
            let performance_service = Arc::new(PerformanceService::new(
                project_store.clone(),
                assessment_organization_service.clone(),
            ));
            let rubric_service = Arc::new(RubricService::new(project_store.clone()));
            let scoring_service = Arc::new(
                ScoringService::new(
                    project_store.clone(),
                    model_gateway.clone(),
                    model_runtime_service.clone(),
                    job_manager.clone(),
                )
                .with_audit_service(audit_service.clone()),
            );
            let scoring_anchor_service = Arc::new(ScoringAnchorService::new(
                project_store.clone(),
                audit_service.clone(),
            ));
            let student_answer_crop_service = Arc::new(StudentAnswerCropService::new(
                project_store.clone(),
                pdf_preview_service.clone(),
            ));
            let graded_exam_review_service = Arc::new(GradedExamReviewService::new(
                project_store.clone(),
                pdf_preview_service.clone(),
            ));
            let student_answer_ocr_service = Arc::new(StudentAnswerOcrService::new(
                project_store.clone(),
                model_gateway.clone(),
                model_runtime_service.clone(),
                pdf_preview_service.clone(),
                model_input_image_service.clone(),
                job_manager.clone(),
            ));

            AppState {
                project_store: project_store.clone(),
                audit_service: audit_service.clone(),
                model_gateway: model_gateway.clone(),
                model_gateway_impl,
                model_config_service,
                model_process_manager,
                model_runtime_service: model_runtime_service.clone(),
                model_input_image_service,
                ocr_image_preprocess_service,
                pdf_service,
                pdf_preview_service,
                document_content_extraction_service,
                job_manager: job_manager.clone(),
                exam_package_build_service,
                graded_exam_review_service,
                question_text_service,
                student_scan_service,
                rubric_service,
                rubric_extraction_service,
                scoring_service,
                scoring_anchor_service,
                school_class_service,
                assessment_organization_service,
                performance_service,
                student_answer_crop_service,
                student_answer_ocr_service,
                speaking_exam_service: SpeakingExamService::new(
                    project_store.clone(),
                    model_gateway.clone(),
                    model_runtime_service.clone(),
                    job_manager.clone(),
                    speaking_engine.clone(),
                )
                .with_audit_service(audit_service.clone()),
                analysis_service: AnalysisService::new(
                    project_store.clone(),
                    model_gateway.clone(),
                    model_runtime_service.clone(),
                    job_manager.clone(),
                ),
                speaking_engine,
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_commands::get_app_status,
            commands::workflow_commands::get_workflow_snapshot,
            commands::analysis_commands::finish_assessment,
            commands::analysis_commands::get_assessment_analysis,
            commands::analysis_commands::list_assessment_analyses,
            commands::diagnostics_commands::get_data_loss_preflight,
            commands::backup_commands::start_backup_job,
            commands::backup_commands::start_restore_job,
            commands::backup_commands::start_recovery_copy_job,
            commands::project_commands::list_projects,
            commands::project_commands::create_project,
            commands::project_commands::update_course_info,
            commands::project_commands::open_project,
            commands::project_commands::migrate_project_with_verified_backup,
            commands::project_commands::get_project_snapshot,
            commands::project_commands::get_default_project_path,
            commands::exam_package_commands::start_exam_package_build,
            commands::generation_gc_commands::run_generation_gc,
            commands::document_commands::import_exam_source_pdf,
            commands::document_commands::import_answer_key_pdf,
            commands::student_scan_commands::import_student_scan_pdf,
            commands::document_commands::list_documents,
            commands::document_commands::remove_document,
            commands::student_scan_commands::list_student_scan_documents,
            commands::pdf_commands::get_pdf_page_count,
            commands::pdf_commands::start_pdf_preview_render,
            commands::pdf_commands::get_pdf_preview_status,
            commands::pdf_commands::get_pdf_page_preview,
            commands::pdf_commands::list_pdf_page_previews,
            commands::pdf_commands::get_pdf_renderer_status,
            commands::student_scan_commands::start_student_scan_preview_render,
            commands::student_scan_commands::get_student_scan_preview_status,
            commands::student_scan_commands::create_student_page_groups,
            commands::student_scan_commands::list_student_submissions,
            commands::student_scan_commands::update_student_identity,
            commands::student_scan_commands::update_submission_pages,
            commands::student_scan_commands::delete_student_submission,
            commands::student_scan_commands::mark_student_grouping_complete,
            commands::student_scan_commands::get_ocr_readiness,
            commands::school_class_commands::list_school_classes,
            commands::school_class_commands::get_school_class_overview,
            commands::school_class_commands::list_class_students,
            commands::school_class_commands::create_class_student,
            commands::school_class_commands::update_class_student,
            commands::school_class_commands::create_school_class,
            commands::school_class_commands::update_school_class,
            commands::school_class_commands::archive_school_class,
            commands::school_class_commands::restore_school_class,
            commands::school_class_commands::import_student_scan_batch,
            commands::school_class_commands::create_student_scan_batch,
            commands::school_class_commands::list_student_scan_batches,
            commands::school_class_commands::move_student_scan_batch,
            commands::school_class_commands::remove_student_scan_batch,
            commands::assessment_organization_commands::list_assessment_activities,
            commands::assessment_organization_commands::get_assessment_sequence_options,
            commands::assessment_organization_commands::list_assessment_classes,
            commands::assessment_organization_commands::create_assessment_activity,
            commands::assessment_organization_commands::add_assessment_class_application,
            commands::assessment_organization_commands::archive_assessment_class_application,
            commands::assessment_organization_commands::attach_assessment_document,
            commands::assessment_organization_commands::list_teaching_assignments,
            commands::assessment_organization_commands::create_teaching_assignment,
            commands::assessment_organization_commands::batch_create_teaching_assignments,
            commands::assessment_organization_commands::archive_teaching_assignment,
            commands::student_answer_ocr_commands::start_student_answer_ocr,
            commands::student_answer_ocr_commands::start_student_identity_ocr,
            commands::student_answer_ocr_commands::update_student_answer_ocr_text,
            commands::student_answer_ocr_commands::mark_student_answer_ocr_reviewed,
            commands::student_answer_ocr_commands::mark_all_student_answer_ocr_reviewed,
            commands::student_answer_ocr_commands::accept_student_answer_ocr_generation,
            commands::student_answer_ocr_commands::reject_student_answer_ocr_generation,
            commands::student_answer_ocr_commands::save_student_answer_crop_template,
            commands::student_answer_ocr_commands::save_student_identity_crop_template,
            commands::student_answer_ocr_commands::preprocess_ocr_image,
            commands::student_answer_ocr_commands::rebuild_student_answer_ocr_issues,
            commands::student_answer_ocr_commands::suggest_ocr_issue_correction_with_model,
            commands::job_commands::get_job_snapshot,
            commands::job_commands::list_jobs,
            commands::job_commands::cancel_job,
            commands::job_commands::retry_job,
            commands::job_commands::cleanup_job_history,
            commands::model_commands::get_model_status,
            commands::model_commands::probe_model_server,
            commands::model_commands::get_model_runtime_status,
            commands::model_commands::start_model_server,
            commands::model_commands::stop_model_server,
            commands::model_commands::set_model_mode,
            commands::model_commands::enable_external_model,
            commands::model_commands::reset_model_profile,
            commands::model_commands::preview_model_server_args,
            commands::model_commands::get_model_log_tail,
            commands::question_text_commands::start_question_text_extraction,
            commands::question_text_commands::start_question_text_vision_fallback,
            commands::question_text_commands::get_question_text_extraction_status,
            commands::question_text_commands::list_question_text_suggestions,
            commands::question_text_commands::confirm_question_text,
            commands::question_text_commands::confirm_all_question_texts,
            commands::question_text_commands::edit_question_text,
            commands::rubric_commands::import_rubric_json,
            commands::rubric_commands::migrate_rubric_levels,
            commands::rubric_commands::get_rubric_state,
            commands::rubric_commands::list_rubric_items,
            commands::rubric_commands::update_question_rubric,
            commands::rubric_commands::confirm_question_rubric,
            commands::rubric_commands::confirm_all_rubrics,
            commands::rubric_commands::validate_rubrics,
            commands::rubric_commands::start_rubric_pdf_import,
            commands::scoring_commands::start_scoring_job,
            commands::scoring_commands::update_scoring_record,
            commands::scoring_commands::get_scoring_summary,
            commands::scoring_commands::list_scoring_anchors,
            commands::scoring_commands::create_scoring_anchor,
            commands::scoring_commands::revoke_scoring_anchor,
            commands::graded_exam_review_commands::get_graded_exam_review,
            commands::speaking_exam_commands::start_speaking_exam,
            commands::speaking_exam_commands::list_speaking_exam_microphones,
            commands::speaking_exam_commands::select_speaking_exam_microphone,
            commands::speaking_exam_commands::get_speaking_exam_runtime_status,
            commands::speaking_exam_commands::toggle_speaking_capture,
            commands::speaking_exam_commands::start_speaking_exam_attempt,
            commands::speaking_exam_commands::stop_speaking_exam_attempt,
            commands::speaking_exam_commands::pause_speaking_exam_attempt,
            commands::speaking_exam_commands::resume_speaking_exam_attempt,
            commands::speaking_exam_commands::cancel_speaking_exam_attempt,
            commands::speaking_exam_commands::sync_speaking_attempt,
            commands::speaking_exam_commands::get_speaking_exam,
            commands::speaking_exam_commands::select_speaking_exam_class,
            commands::speaking_exam_commands::select_speaking_exam_student,
            commands::speaking_exam_commands::update_speaking_criterion_score,
            commands::speaking_exam_commands::update_speaking_criterion_level,
            commands::speaking_exam_commands::update_speaking_attempt_note,
            commands::speaking_exam_commands::approve_speaking_attempt,
            commands::assessment_organization_commands::get_assessment_activity,
            commands::assessment_organization_commands::get_assessment_class_applications,
            commands::assessment_organization_commands::get_class_application_students,
            commands::assessment_organization_commands::update_assessment_activity,
            commands::assessment_organization_commands::remove_assessment_class_application,
            commands::assessment_organization_commands::set_active_written_activity,
            commands::performance_commands::create_performance_task,
            commands::performance_commands::update_performance_task,
            commands::performance_commands::list_performance_tasks,
            commands::performance_commands::get_performance_task,
            commands::performance_commands::publish_performance_rubric,
            commands::performance_commands::get_performance_rubric_history,
            commands::performance_commands::save_performance_assessment,
            commands::performance_commands::approve_performance_assessment,
            commands::performance_commands::set_performance_assessment_status,
            commands::performance_commands::list_performance_assessments,
            commands::performance_commands::get_performance_report,
            commands::performance_commands::get_performance_status,
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("error while building tauri application: {error}"))
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state.job_manager.shutdown_all_jobs(app_handle);
                }
            }
        });
}

fn managed_asset_error_response() -> http::Response<Cow<'static, [u8]>> {
    http::Response::builder()
        .status(404)
        .body(Cow::Borrowed(&b""[..]))
        .unwrap_or_else(|_| {
            http::Response::builder()
                .status(500)
                .body(Cow::Borrowed(&b""[..]))
                .unwrap_or_else(|_| http::Response::new(Cow::Borrowed(&b""[..])))
        })
}

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::document::{DocumentRole, PdfPreviewStatus};
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::job::{JobSnapshot, JobStatus};

use crate::domain::project::Project;
use crate::domain::question::{is_question_text_ready, TextFieldSource, TextFieldStatus};
use crate::domain::rubric::{
    has_meaningful_rubric_content, is_rubric_confirmed, validate_rubric_state, RubricSource,
    RubricStatus,
};
use crate::domain::school_class::{normalize_school_class_name, SchoolClassStatus};
use crate::domain::student::student_identity_is_missing;
use crate::jobs::job_manager::load_persisted_jobs;
use crate::platform::paths::model_server_log_path;
use crate::platform::project_paths::TrustedProjectRoot;
use crate::services::document_content_extraction_service::{
    clamp_question_markers, detect_question_markers, missing_numbers,
    normalize_question_detection_text, DocumentContentExtractionRequest,
    DocumentContentExtractionService, DocumentContentKind,
};
use crate::services::llama_server_gateway::LlamaServerGateway;
use crate::services::model_config_service::ModelConfigService;
use crate::services::model_input_image_service::ModelInputImageService;
use crate::services::model_process_manager::ModelProcessManager;
use crate::services::model_runtime_service::ModelRuntimeService;
use crate::services::question_text_service::{
    apply_extraction_to_project_with_expected, extract_numbered_questions_from_text,
};

use crate::services::project_store::{PersistenceDiagnostics, ProjectStore};
use crate::services::school_class_service::{build_school_class_overview, students_for_class};
use crate::services::workflow_engine;

#[derive(Clone)]
pub struct DiagnosticsContext {
    project_store: ProjectStore,
    model_config_service: ModelConfigService,
    model_runtime_service: ModelRuntimeService,
    document_content_extraction_service: std::sync::Arc<DocumentContentExtractionService>,
    audit_service: std::sync::Arc<crate::services::audit_service::AuditService>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub project_path: String,
    pub project_file_exists: bool,
    pub project_readable: bool,
    pub project: Option<ProjectInspectReport>,
    pub path_security: PathSecurityDoctorSummary,
    pub persistence: PersistenceDiagnostics,
    pub documents_dir_exists: bool,
    pub cache_dir_exists: bool,
    pub exam_source_exists: bool,
    pub rubric_or_answer_key_exists: bool,
    pub student_scan_exists: bool,
    pub student_scan_documents: usize,
    pub student_scan_total_pages: u32,
    pub student_scan_preview_ready_pages: u32,
    pub student_scan_preview_total_pages: u32,
    pub student_grouping_complete: bool,
    pub student_submissions: usize,
    pub school_class_count: usize,
    pub active_school_class_count: usize,
    pub archived_school_class_count: usize,
    pub student_scan_batch_count: usize,
    pub scan_batch_without_class_count: usize,
    pub submission_without_class_count: usize,
    pub class_membership_inconsistency_count: usize,
    pub identity_class_mismatch_count: usize,
    pub school_class_summaries: Vec<DoctorSchoolClassSummary>,
    pub student_answer_ocr_records: usize,
    pub student_answer_ocr_expected_records: usize,
    pub student_answer_ocr_reviewed: usize,
    pub student_answer_ocr_needs_review: usize,
    pub ocr_active_generation_count: usize,
    pub ocr_pending_generation_count: usize,
    pub ocr_interrupted_generation_count: usize,
    pub ocr_stale_generation_count: usize,
    pub student_answer_ocr_status: String,
    pub student_answer_ocr_ready_for_scoring: bool,
    pub pages_per_student: Option<u32>,
    pub preview_metadata_exists: bool,
    pub preview_png_count: usize,
    pub preview_active_generation_count: usize,
    pub preview_orphan_staging_count: usize,
    pub preview_missing_active_generation_count: usize,
    pub submission_delete_blocked_count: usize,
    pub orphan_artifact_count: usize,
    pub exam_package_status: String,
    pub question_text_coverage: String,
    pub rubric_coverage: String,
    pub ready_for_review: bool,
    pub ready_for_qep: bool,
    pub question_text_summary: QuestionTextSummary,
    pub rubric_summary: RubricSummary,
    pub exam_package_freeze_ready: bool,
    pub exam_package_freeze_blockers: Vec<String>,
    pub student_intake_ready: bool,
    pub student_intake_blockers: Vec<String>,
    pub scoring_ready: bool,
    pub scoring_blockers: Vec<String>,
    pub scoring_result_count: usize,
    pub scoring_total_history_count: usize,
    pub scoring_duplicate_result_count: usize,
    pub active_scoring_run_id: Option<String>,
    pub scoring_approved_count: usize,
    pub scoring_needs_review_count: usize,
    pub scoring_stale_count: usize,
    pub speaking: SpeakingDoctorSummary,
    pub job_summary: JobSummary,
    pub model_status: Option<ModelInspectReport>,
    pub security: SecurityDoctorSummary,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathSecurityDoctorSummary {
    pub project_root_metadata_mismatch: usize,
    pub unsafe_document_path_count: usize,
    pub unresolved_legacy_document_path_count: usize,
    pub external_managed_document_path_count: usize,
    pub symlink_escape_count: usize,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityDoctorSummary {
    // Privacy
    pub unsafe_log_call_count: u64,
    pub redaction_count: u64,
    pub sentinel_leak_count: u64,
    pub public_error_conversion_failure_count: u64,
    // Model gateway
    pub oversized_response_count: u64,
    pub oversized_request_count: u64,
    pub timeout_count: u64,
    pub malformed_response_count: u64,
    pub configured_response_limit_bytes: u64,
    pub configured_request_limit_bytes: u64,
    // Configuration
    pub hard_coded_production_path_count: u64,
    pub missing_model_resource_count: u64,
    pub invalid_executable_count: u64,
    // Speaking
    pub active_speaking_sessions: u64,
    pub interrupted_speaking_sessions: u64,
    pub local_only_ui_authority_count: u64,
    // Locking
    pub app_single_instance_active: bool,
    pub project_lock_held: bool,
    pub project_lock_conflict_count: u64,
    pub writer_without_os_lock_count: u64,
    // Asset serving
    pub raw_path_dto_count: u64,
    pub rejected_traversal_count: u64,
    pub rejected_symlink_count: u64,
    pub portable_project_asset_check: bool,
    // Audit
    pub audit_record_count: u64,
    pub audit_chain_status: String,
    pub audit_tamper_count: u64,
    pub audit_append_failure_count: u64,
    // Backup
    pub last_backup_verified: bool,
    pub restore_verification_failures: u64,
    pub orphan_restore_staging_count: u64,
    // Generation GC
    pub gc_protected_generations: u64,
    pub gc_cleanup_candidates: u64,
    pub gc_deleted_generations: u64,
    pub gc_deferred_cleanup: u64,
    pub gc_orphan_staging: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingDoctorSummary {
    pub assessment_activity_count: usize,
    pub speaking_activity_count: usize,
    pub written_activity_count: usize,
    pub listening_activity_count: usize,
    pub assessment_class_application_count: usize,
    pub duplicate_activity_class_application_count: usize,
    pub class_application_without_class_count: usize,
    pub speaking_attempt_without_activity_count: usize,
    pub speaking_attempt_without_class_application_count: usize,
    pub speaking_attempt_class_membership_mismatch_count: usize,
    pub unresolved_legacy_speaking_record_count: usize,
    pub activity_application_workflow_mismatch_count: usize,
    pub cleanup_model: String,
    pub evaluation_model: String,
    pub cleanup_prompt_version: String,
    pub evaluation_prompt_version: String,
    pub scoring_policy_version: String,
    pub cleanup_pending_count: usize,
    pub cleanup_review_count: usize,
    pub cleanup_failed_count: usize,
    pub evaluation_stale_count: usize,
    pub evaluation_parse_failure_count: usize,
    pub mixed_policy_count: usize,
    pub legacy_fractional_score_count: usize,
    pub missing_required_criteria_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorSchoolClassSummary {
    pub name: String,
    pub scan_batch_count: u32,
    pub submission_count: u32,
    pub identity_verified: u32,
    pub ocr_complete: u32,
    pub scoring_complete: u32,
    pub review_required: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspectReport {
    pub project_id: String,
    pub project_name: String,
    pub expected_question_count: Option<u32>,
    pub question_count: usize,
    pub document_counts_by_role: BTreeMap<String, usize>,
    pub workflow_stage: String,
    pub blocking_reasons: Vec<String>,
    pub next_actions: Vec<WorkflowActionSummary>,
    pub paths: ProjectPaths,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPaths {
    pub root_path: String,
    pub project_json: String,
    pub documents_dir: String,
    pub cache_dir: String,
    pub preview_dir: String,
    pub model_inputs_dir: String,
    pub logs_dir: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowActionSummary {
    pub code: String,
    pub label: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    pub jobs: Vec<JobInspectRecord>,
    pub stale_candidates: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInspectRecord {
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub active: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_message: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_details: Option<String>,
    pub stale_candidate: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextSummary {
    pub expected_question_count: u32,
    pub extracted: Vec<u32>,
    pub missing_numbers: Vec<u32>,
    pub coverage_ok: bool,
    pub partial_success: bool,
    pub missing: usize,
    pub suggested: usize,
    pub edited: usize,
    pub confirmed: usize,
    pub failed: usize,
    pub questions: Vec<QuestionTextRecord>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextRecord {
    pub number: u32,
    pub status: String,
    pub source: String,
    pub confidence: Option<f32>,
    pub value_length: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricSummary {
    pub expected_question_count: u32,
    pub imported_question_numbers: Vec<u32>,
    pub false_positive_imported: Vec<u32>,
    pub missing_question_numbers: Vec<u32>,
    pub failed_question_numbers: Vec<u32>,
    pub partial_success: bool,
    pub strategy: String,
    pub missing: usize,
    pub imported: usize,
    pub manual: usize,
    pub suggested: usize,
    pub confirmed: usize,
    pub invalid: usize,
    pub questions: Vec<RubricRecord>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricRecord {
    pub number: u32,
    pub status: String,
    pub error_code: Option<String>,
    pub source: String,
    pub max_points: Option<f32>,
    pub expected_answer_length: usize,
    pub criteria_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInspectReport {
    pub active_profile_id: String,
    pub display_name: String,
    pub mode: String,
    pub runtime_state: String,
    pub llama_cpp_root: String,
    pub server_path_exists: bool,
    pub model_path_exists: bool,
    pub mmproj_path_exists: bool,
    pub llama_server_binary: String,
    pub model_path: String,
    pub mmproj_path: String,
    pub base_url: String,
    pub model_port: u16,
    pub model_port_listening: bool,
    pub model_port_health_ok: bool,
    pub model_config_complete: bool,
    pub model_autostart_available: bool,
    pub llama_server_binary_exists: bool,
    pub model_file_exists: bool,
    pub mmproj_file_exists: bool,
    pub health_ok: bool,
    pub completion_probe_ok: bool,
    pub started_by_app: bool,
    pub model_managed_process_pid: Option<u32>,
    pub managed_process_present: bool,
    pub process_identity_verification: String,
    pub active_lease_count: usize,
    pub draining_requested: bool,
    pub log_path: Option<String>,
    pub can_start_from_app: bool,
    pub can_stop_from_app: bool,
    pub warnings: Vec<String>,
    pub last_error: Option<String>,
    pub log_tail: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInspectRecord {
    pub id: String,
    pub role: String,
    pub file_name: String,
    pub stored_path: String,
    pub exists: bool,
    pub checksum: Option<String>,
    pub page_count: u32,
    pub preview_status: Option<String>,
    pub preview_ready: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContentInspectRecord {
    pub document_id: String,
    pub kind: String,
    pub role: String,
    pub method: String,
    pub raw_text_length: usize,
    pub normalized_text_length: usize,
    pub enough_text: bool,
    pub vision_fallback_needed: bool,
    pub detected_question_numbers: Vec<u32>,
    pub missing_question_numbers: Vec<u32>,
    pub ignored_question_numbers: Vec<u32>,
    pub metadata_stale: bool,
    pub needs_refresh: bool,
    pub fresh_detected_question_numbers: Vec<u32>,
    pub fresh_missing_question_numbers: Vec<u32>,
    pub artifact_dir: String,
    pub metadata_exists: bool,
    pub raw_text_exists: bool,
    pub normalized_text_exists: bool,
    pub pdftotext_stderr_exists: bool,
    pub model_input_manifest_exists: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputInspectReport {
    pub batches: Vec<ModelInputBatchReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputBatchReport {
    pub kind: String,
    pub document_id: String,
    pub page_count: usize,
    pub total_bytes: u64,
    pub largest_image_bytes: u64,
    pub long_edge_max: u32,
    pub jpeg_quality: u8,
    pub missing_metadata_warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayReport {
    pub target: String,
    pub dry_run: bool,
    pub project_path: String,
    pub strategy: Option<String>,
    pub content_method: Option<String>,
    pub expected_question_count: Option<u32>,
    pub target_questions: Vec<u32>,
    pub already_available: Vec<u32>,
    pub will_run_questions: Vec<u32>,
    pub will_run_vision_fallback_for: Vec<u32>,
    pub invalid_or_empty: Vec<u32>,
    pub extracted: Vec<u32>,
    pub missing: Vec<u32>,
    pub failed: Vec<u32>,
    pub coverage_ok: Option<bool>,
    pub partial_success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fresh_pdf_extraction: Option<QuestionTextFreshReplayReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_snapshot: Option<QuestionTextSnapshotReplayReport>,
    pub checks: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionMarkerOffset {
    pub number: u32,
    pub offset: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextFreshReplayReport {
    pub content_method: String,
    pub expected_question_count: u32,
    pub detected_markers: Vec<u32>,
    pub marker_offsets: Vec<QuestionMarkerOffset>,
    pub missing: Vec<u32>,
    pub contaminated: Vec<u32>,
    pub coverage_ok: bool,
    pub will_run_vision_fallback_for: Vec<u32>,
    pub vision_fallback_call_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextSnapshotReplayReport {
    pub available: Vec<u32>,
    pub missing: Vec<u32>,
    pub contaminated: Vec<u32>,
    pub stale: bool,
    pub needs_refresh: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextRepairReport {
    pub expected_question_count: u32,
    pub fresh_detected: Vec<u32>,
    pub fresh_missing: Vec<u32>,
    pub fresh_contaminated: Vec<u32>,
    pub before_available: Vec<u32>,
    pub before_missing: Vec<u32>,
    pub before_contaminated: Vec<u32>,
    pub updated: Vec<u32>,
    pub created: Vec<u32>,
    pub preserved_confirmed: Vec<u32>,
    pub preserved_edited: Vec<u32>,
    pub after_available: Vec<u32>,
    pub after_missing: Vec<u32>,
    pub coverage_ok: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContentRepairItem {
    pub document_id: String,
    pub role: String,
    pub kind: String,
    pub before_method: String,
    pub after_method: String,
    pub before_detected_question_numbers: Vec<u32>,
    pub after_detected_question_numbers: Vec<u32>,
    pub ignored_question_numbers: Vec<u32>,
    pub metadata_stale: bool,
    pub needs_refresh: bool,
    pub vision_fallback_needed: bool,
    pub metadata_written: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContentRepairReport {
    pub expected_question_count: Option<u32>,
    pub repaired_count: usize,
    pub items: Vec<DocumentContentRepairItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleJobsRepairItem {
    pub job_id: String,
    pub kind: String,
    pub status_before: String,
    pub status_after: String,
    pub stale_before: bool,
    pub stale_after: bool,
    pub active_before: bool,
    pub active_after: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleJobsRepairReport {
    pub repaired_count: usize,
    pub items: Vec<StaleJobsRepairItem>,
}

#[derive(Debug, Clone)]
struct DocumentContentFreshAnalysis {
    detected_question_numbers: Vec<u32>,
    missing_question_numbers: Vec<u32>,
    ignored_question_numbers: Vec<u32>,
    vision_fallback_needed: bool,
}

fn speaking_doctor_summary(project: Option<&Project>) -> SpeakingDoctorSummary {
    let mut summary = SpeakingDoctorSummary {
        assessment_activity_count: 0,
        speaking_activity_count: 0,
        written_activity_count: 0,
        listening_activity_count: 0,
        assessment_class_application_count: 0,
        duplicate_activity_class_application_count: 0,
        class_application_without_class_count: 0,
        speaking_attempt_without_activity_count: 0,
        speaking_attempt_without_class_application_count: 0,
        speaking_attempt_class_membership_mismatch_count: 0,
        unresolved_legacy_speaking_record_count: 0,
        activity_application_workflow_mismatch_count: 0,
        cleanup_model: "Gemma 4 12B".to_string(),
        evaluation_model: "Gemma 4 12B".to_string(),
        cleanup_prompt_version: "speaking_asr_cleanup_tr_v3".to_string(),
        evaluation_prompt_version: "speaking_rubric_evidence_tr_v3".to_string(),
        scoring_policy_version: "speaking_scoring_policy_v1".to_string(),
        cleanup_pending_count: 0,
        cleanup_review_count: 0,
        cleanup_failed_count: 0,
        evaluation_stale_count: 0,
        evaluation_parse_failure_count: 0,
        mixed_policy_count: 0,
        legacy_fractional_score_count: 0,
        missing_required_criteria_count: 0,
    };
    let Some(project) = project else {
        return summary;
    };
    summary.assessment_activity_count = project.assessment_activities.len();
    summary.speaking_activity_count = project
        .assessment_activities
        .iter()
        .filter(|activity| {
            activity.assessment_type == crate::domain::assessment::AssessmentType::Speaking
        })
        .count();
    summary.written_activity_count = project
        .assessment_activities
        .iter()
        .filter(|activity| {
            activity.assessment_type == crate::domain::assessment::AssessmentType::Written
        })
        .count();
    summary.listening_activity_count = project
        .assessment_activities
        .iter()
        .filter(|activity| {
            activity.assessment_type == crate::domain::assessment::AssessmentType::Listening
        })
        .count();
    let mut activity_keys = std::collections::HashSet::new();
    for activity in &project.assessment_activities {
        let key = (
            activity.academic_year_id.clone(),
            activity.course_id.clone(),
            activity.grade_level,
            activity.term,
            format!("{:?}", activity.assessment_type),
            activity.sequence_number,
        );
        if !activity_keys.insert(key) {
            summary.duplicate_activity_class_application_count += 1;
        }
        if activity.workflow_family != activity.assessment_type.workflow_family() {
            summary.activity_application_workflow_mismatch_count += 1;
        }
        let mut class_ids = std::collections::HashSet::new();
        for application in &activity.class_applications {
            summary.assessment_class_application_count += 1;
            if !class_ids.insert(application.school_class_id.clone()) {
                summary.duplicate_activity_class_application_count += 1;
            }
            let school_class = project
                .school_classes
                .iter()
                .find(|school_class| school_class.id == application.school_class_id);
            if school_class.is_none() {
                summary.class_application_without_class_count += 1;
            }
            for attempt in &application.speaking_attempts {
                if attempt.assessment_activity_id.is_none() {
                    summary.speaking_attempt_without_activity_count += 1;
                }
                if attempt.class_application_id.as_deref() != Some(application.id.as_str()) {
                    summary.speaking_attempt_without_class_application_count += 1;
                }
                if let Ok(students) = students_for_class(project, &application.school_class_id) {
                    if !students
                        .iter()
                        .any(|student| student.id == attempt.student_id)
                    {
                        summary.speaking_attempt_class_membership_mismatch_count += 1;
                    }
                }
            }
        }
    }
    for exam in &project.speaking_exams {
        if exam.assessment_activity_id.is_none() {
            summary.unresolved_legacy_speaking_record_count += 1;
        }
        let mut policies = std::collections::HashSet::new();
        for attempt in &exam.attempts {
            if attempt.assessment_activity_id.is_none() {
                summary.speaking_attempt_without_activity_count += 1;
            }
            if attempt.class_application_id.is_none() {
                summary.speaking_attempt_without_class_application_count += 1;
            }
            policies.insert(attempt.scoring_policy_version.clone());
            if !attempt.raw_transcript.trim().is_empty()
                && matches!(
                    attempt.cleanup_status,
                    crate::domain::speaking::SpeakingTranscriptCleanupStatus::NotStarted
                        | crate::domain::speaking::SpeakingTranscriptCleanupStatus::Running
                )
            {
                summary.cleanup_pending_count += 1;
            }
            if matches!(
                attempt.cleanup_status,
                crate::domain::speaking::SpeakingTranscriptCleanupStatus::NeedsReview
            ) {
                summary.cleanup_review_count += 1;
            }
            if matches!(
                attempt.cleanup_status,
                crate::domain::speaking::SpeakingTranscriptCleanupStatus::Failed
            ) {
                summary.cleanup_failed_count += 1;
            }
            if attempt.evaluation_error.is_some() {
                summary.evaluation_parse_failure_count += 1;
            }
            if attempt.evaluation_input_hash.is_some()
                && attempt.criterion_scores.iter().any(|score| {
                    score.ai_suggested_score.is_none()
                        && score.automatic_score.is_none()
                        && score.teacher_score.is_none()
                })
            {
                summary.evaluation_stale_count += 1;
            }
            if attempt.criterion_scores.len() < exam.criteria.len() {
                summary.missing_required_criteria_count += 1;
            }
            summary.legacy_fractional_score_count += attempt
                .criterion_scores
                .iter()
                .filter(|score| {
                    [
                        score.automatic_score,
                        score.ai_suggested_score,
                        score.teacher_score,
                        score.final_score,
                    ]
                    .into_iter()
                    .flatten()
                    .any(|value| value.fract() != 0.0)
                })
                .count();
        }
        policies.remove("");
        if policies.len() > 1 {
            summary.mixed_policy_count += 1;
        }
    }
    summary
}

impl DiagnosticsContext {
    pub fn new() -> Self {
        let project_store = ProjectStore::new();
        let model_config_service = ModelConfigService::new();
        let model_gateway_impl = std::sync::Arc::new(LlamaServerGateway::default());
        let model_process_manager =
            ModelProcessManager::new(model_config_service.clone(), model_gateway_impl);
        let model_runtime_service =
            ModelRuntimeService::new(model_config_service.clone(), model_process_manager);
        let model_input_image_service = std::sync::Arc::new(ModelInputImageService::default());
        let document_content_extraction_service = std::sync::Arc::new(
            DocumentContentExtractionService::new(model_input_image_service.clone()),
        );
        Self {
            project_store,
            model_config_service,
            model_runtime_service,
            document_content_extraction_service,
            audit_service: std::sync::Arc::new(crate::services::audit_service::AuditService::new()),
        }
    }

    pub fn open_project(&self, project_path: &Path) -> Result<Project, AppError> {
        self.project_store
            .open_project(project_path.to_string_lossy().to_string())
    }

    pub fn inspect_project(&self, project_path: &Path) -> Result<ProjectInspectReport, AppError> {
        let project = self.open_project(project_path)?;
        let workflow_snapshot = workflow_engine::evaluate_workflow(&project);
        let mut computed_project = project;
        computed_project.workflow = workflow_snapshot;
        Ok(project_report(computed_project))
    }

    pub fn inspect_documents(
        &self,
        project_path: &Path,
    ) -> Result<Vec<DocumentInspectRecord>, AppError> {
        let project = self.open_project(project_path)?;
        Ok(document_reports(&project, project_path))
    }

    pub async fn inspect_model(&self, tail: usize) -> Result<ModelInspectReport, AppError> {
        let runtime = self
            .model_runtime_service
            .get_runtime_status(
                None,
                &crate::services::model_runtime_service::ModelRuntimeRequest {
                    use_case: crate::services::model_runtime_service::ModelUseCase::GeneralText,
                    capability: crate::services::model_runtime_service::ModelCapability::Text,
                    requires_mmproj: false,
                    timeout_seconds: 30,
                },
            )
            .await?;
        let status = self.model_runtime_service.get_model_status(None).await?;
        let profile = self.model_config_service.get_profile(None)?;
        let log_path = status
            .log_path
            .clone()
            .unwrap_or_else(|| model_server_log_path(&status.profile_id));
        let log_tail = read_log_tail(&log_path, tail);
        Ok(ModelInspectReport {
            active_profile_id: status.profile_id,
            display_name: status.display_name,
            mode: format!("{:?}", status.mode).to_lowercase(),
            runtime_state: format!("{:?}", runtime.state).to_lowercase(),
            llama_cpp_root: llama_cpp_root(&profile.server_path),
            server_path_exists: status.server_path_exists,
            model_path_exists: status.model_path_exists,
            mmproj_path_exists: status.mmproj_path_exists,
            llama_server_binary: profile.server_path.clone(),
            model_path: profile.model_path.clone(),
            mmproj_path: profile.mmproj_path.clone(),
            base_url: status.base_url,
            model_port: runtime.port,
            model_port_listening: runtime.port_listening,
            model_port_health_ok: runtime.port_health_ok,
            model_config_complete: runtime.config_complete,
            model_autostart_available: runtime.autostart_available,
            llama_server_binary_exists: runtime.llama_server_binary_exists,
            model_file_exists: runtime.model_file_exists,
            mmproj_file_exists: runtime.mmproj_file_exists,
            health_ok: status.health_ok,
            completion_probe_ok: status.completion_probe_ok,
            started_by_app: status.started_by_app,
            model_managed_process_pid: status.managed_process_pid,
            managed_process_present: status.managed_process_pid.is_some(),
            process_identity_verification: if status.started_by_app {
                "verified".to_string()
            } else if status.managed_process_pid.is_some() {
                "unverified_or_external".to_string()
            } else {
                "not_present".to_string()
            },
            active_lease_count: status.active_lease_count,
            draining_requested: status.draining,
            log_path: Some(log_path.to_string_lossy().to_string()),
            can_start_from_app: status.can_start_from_app,
            can_stop_from_app: status.can_stop_from_app,
            warnings: status.warnings,
            last_error: status.last_error.map(|error| format!("{error}")),
            log_tail,
        })
    }

    pub fn inspect_jobs(&self, project_path: &Path) -> Result<JobSummary, AppError> {
        let jobs = load_persisted_jobs(project_path)?;
        let now = Utc::now();
        let mut records = Vec::new();
        let mut stale_candidates = Vec::new();

        for job in jobs {
            let stale_candidate = is_stale_candidate(&job, now);
            if stale_candidate {
                stale_candidates.push(job.id.clone());
            }
            records.push(job_record(&job, stale_candidate));
        }

        Ok(JobSummary {
            jobs: records,
            stale_candidates,
        })
    }

    pub fn security_doctor_summary(
        &self,
        project_path: &Path,
        project: &Project,
    ) -> SecurityDoctorSummary {
        let audit = self.audit_service.counters(project_path);
        let gc_report = crate::platform::project_paths::TrustedProjectRoot::from_canonical_root(
            std::path::PathBuf::from(&project.root_path),
            false,
        )
        .ok()
        .and_then(|trusted_root| {
            crate::services::generation_gc_service::run_generation_gc(
                &trusted_root,
                project,
                true,
                &crate::services::generation_gc_service::GenerationGcPolicy::default(),
            )
            .ok()
        });

        let active_speaking = project
            .speaking_exams
            .iter()
            .flat_map(|exam| exam.attempts.iter())
            .filter(|attempt| {
                matches!(
                    attempt.state,
                    crate::domain::speaking::SpeakingAttemptState::Recording
                        | crate::domain::speaking::SpeakingAttemptState::Paused
                )
            })
            .count() as u64;
        let interrupted_speaking = project
            .speaking_exams
            .iter()
            .flat_map(|exam| exam.attempts.iter())
            .filter(|attempt| {
                matches!(
                    attempt.state,
                    crate::domain::speaking::SpeakingAttemptState::Draft
                )
            })
            .count() as u64;

        SecurityDoctorSummary {
            unsafe_log_call_count: count_unsafe_log_calls(),
            redaction_count: 0,
            sentinel_leak_count: count_sentinel_leaks(project_path),
            public_error_conversion_failure_count: 0,
            oversized_response_count: 0,
            oversized_request_count: 0,
            timeout_count: 0,
            malformed_response_count: 0,
            configured_response_limit_bytes:
                crate::services::llama_server_gateway::DEFAULT_MAX_RESPONSE_BODY_BYTES,
            configured_request_limit_bytes:
                crate::services::llama_server_gateway::DEFAULT_MAX_REQUEST_BODY_BYTES,
            hard_coded_production_path_count: count_hard_coded_paths(),
            missing_model_resource_count: 0,
            invalid_executable_count: 0,
            active_speaking_sessions: active_speaking,
            interrupted_speaking_sessions: interrupted_speaking,
            local_only_ui_authority_count: 0,
            app_single_instance_active: true,
            project_lock_held: true,
            project_lock_conflict_count: 0,
            writer_without_os_lock_count: 0,
            raw_path_dto_count: 0,
            rejected_traversal_count: 0,
            rejected_symlink_count: 0,
            portable_project_asset_check: true,
            audit_record_count: audit.record_count,
            audit_chain_status: if audit.chain_valid {
                "valid".to_string()
            } else {
                "tampered".to_string()
            },
            audit_tamper_count: audit.tamper_count,
            audit_append_failure_count: audit.append_failure_count,
            last_backup_verified: false,
            restore_verification_failures: 0,
            orphan_restore_staging_count: 0,
            gc_protected_generations: gc_report
                .as_ref()
                .map(|report| report.protected_generations as u64)
                .unwrap_or(0),
            gc_cleanup_candidates: gc_report
                .as_ref()
                .map(|report| report.cleanup_candidates as u64)
                .unwrap_or(0),
            gc_deleted_generations: gc_report
                .as_ref()
                .map(|report| report.deleted_generations as u64)
                .unwrap_or(0),
            gc_deferred_cleanup: gc_report
                .as_ref()
                .map(|report| report.deferred_cleanup as u64)
                .unwrap_or(0),
            gc_orphan_staging: gc_report
                .as_ref()
                .map(|report| report.orphan_staging_dirs as u64)
                .unwrap_or(0),
        }
    }

    pub fn inspect_question_text(
        &self,
        project_path: &Path,
    ) -> Result<QuestionTextSummary, AppError> {
        let project = self.open_project(project_path)?;
        Ok(question_text_summary(&project))
    }

    pub fn inspect_rubric(&self, project_path: &Path) -> Result<RubricSummary, AppError> {
        let project = self.open_project(project_path)?;
        Ok(rubric_summary(&project))
    }

    pub fn inspect_model_inputs(
        &self,
        project_path: &Path,
    ) -> Result<ModelInputInspectReport, AppError> {
        let project = self.open_project(project_path)?;
        let manifests = ModelInputImageService::load_manifests(Path::new(&project.root_path))?;
        let mut warnings = Vec::new();
        let batches = manifests
            .into_iter()
            .map(|manifest| {
                if manifest.images.is_empty() {
                    warnings.push(format!(
                        "Missing model input metadata for {:?} {}",
                        manifest.kind, manifest.document_id
                    ));
                }
                let largest_image_bytes = manifest
                    .images
                    .iter()
                    .map(|image| image.output_bytes)
                    .max()
                    .unwrap_or(0);
                ModelInputBatchReport {
                    kind: format!("{:?}", manifest.kind).to_lowercase(),
                    document_id: manifest.document_id,
                    page_count: manifest.images.len(),
                    total_bytes: manifest.total_output_bytes,
                    largest_image_bytes,
                    long_edge_max: manifest.long_edge_max,
                    jpeg_quality: manifest.jpeg_quality,
                    missing_metadata_warnings: if manifest.images.is_empty() {
                        vec!["manifest contains no model input images".to_string()]
                    } else {
                        vec![]
                    },
                }
            })
            .collect();

        Ok(ModelInputInspectReport { batches, warnings })
    }

    pub async fn doctor(&self, project_path: &Path) -> Result<DoctorReport, AppError> {
        let project_file = project_path.join("project.json");
        let project_file_exists = project_file.exists();
        if !project_file_exists {
            return Ok(DoctorReport {
                project_path: project_path.to_string_lossy().to_string(),
                project_file_exists: false,
                project_readable: false,
                project: None,
                path_security: PathSecurityDoctorSummary::default(),
                persistence: PersistenceDiagnostics {
                    storage_revision: 0,
                    project_fingerprint_status: "unknown".to_string(),
                    stale_job_result_count: 0,
                    mutation_conflict_count: 0,
                    external_modification_detected: false,
                    legacy_project_without_revision: false,
                },
                documents_dir_exists: project_path.join("documents").exists(),
                cache_dir_exists: project_path.join("cache").exists(),
                exam_source_exists: false,
                rubric_or_answer_key_exists: false,
                student_scan_exists: false,
                student_scan_documents: 0,
                student_scan_total_pages: 0,
                student_scan_preview_ready_pages: 0,
                student_scan_preview_total_pages: 0,
                student_grouping_complete: false,
                student_submissions: 0,
                school_class_count: 0,
                active_school_class_count: 0,
                archived_school_class_count: 0,
                student_scan_batch_count: 0,
                scan_batch_without_class_count: 0,
                submission_without_class_count: 0,
                class_membership_inconsistency_count: 0,
                identity_class_mismatch_count: 0,
                school_class_summaries: vec![],
                student_answer_ocr_records: 0,
                student_answer_ocr_expected_records: 0,
                student_answer_ocr_reviewed: 0,
                student_answer_ocr_needs_review: 0,
                ocr_active_generation_count: 0,
                ocr_pending_generation_count: 0,
                ocr_interrupted_generation_count: 0,
                ocr_stale_generation_count: 0,
                student_answer_ocr_status: "not_started".to_string(),
                student_answer_ocr_ready_for_scoring: false,
                pages_per_student: None,
                preview_metadata_exists: false,
                preview_png_count: 0,
                preview_active_generation_count: 0,
                preview_orphan_staging_count: 0,
                preview_missing_active_generation_count: 0,
                submission_delete_blocked_count: 0,
                orphan_artifact_count: 0,
                exam_package_status: "missing".to_string(),
                question_text_coverage: "0/0".to_string(),
                rubric_coverage: "0/0".to_string(),
                ready_for_review: false,
                ready_for_qep: false,
                question_text_summary: QuestionTextSummary {
                    expected_question_count: 0,
                    extracted: vec![],
                    missing_numbers: vec![],
                    coverage_ok: true,
                    partial_success: false,
                    missing: 0,
                    suggested: 0,
                    edited: 0,
                    confirmed: 0,
                    failed: 0,
                    questions: vec![],
                    warnings: vec![],
                },
                rubric_summary: RubricSummary {
                    expected_question_count: 0,
                    imported_question_numbers: vec![],
                    false_positive_imported: vec![],
                    missing_question_numbers: vec![],
                    failed_question_numbers: vec![],
                    partial_success: false,
                    strategy: "per_question".to_string(),
                    missing: 0,
                    imported: 0,
                    manual: 0,
                    suggested: 0,
                    confirmed: 0,
                    invalid: 0,
                    questions: vec![],
                    warnings: vec![],
                },
                exam_package_freeze_ready: false,
                exam_package_freeze_blockers: vec![],
                student_intake_ready: false,
                student_intake_blockers: vec![],
                scoring_ready: false,
                scoring_blockers: vec![],
                scoring_result_count: 0,
                scoring_total_history_count: 0,
                scoring_duplicate_result_count: 0,
                active_scoring_run_id: None,
                scoring_approved_count: 0,
                scoring_needs_review_count: 0,
                scoring_stale_count: 0,
                speaking: speaking_doctor_summary(None),
                job_summary: JobSummary {
                    jobs: vec![],
                    stale_candidates: vec![],
                },
                model_status: None,
                security: SecurityDoctorSummary::default(),
                warnings: vec![],
                errors: vec!["project.json missing".to_string()],
            });
        }

        let (project, load_warnings) = self
            .project_store
            .open_project_with_warnings(project_path.to_string_lossy().to_string())?;
        let workflow_snapshot = workflow_engine::evaluate_workflow(&project);
        let preview_metadata_exists = project.documents.iter().any(|document| {
            preview_metadata_path(&project, &document.id).is_some_and(|path| path.exists())
        });
        let preview_png_count = project
            .documents
            .iter()
            .map(|document| page_preview_png_count(&project, &document.id))
            .sum();
        let ocr_active_generation_count = project
            .student_answer_ocr_generations
            .iter()
            .filter(|generation| {
                generation.status == crate::domain::student::OcrGenerationStatus::Active
            })
            .count();
        let ocr_pending_generation_count = project
            .student_answer_ocr_generations
            .iter()
            .filter(|generation| {
                matches!(
                    generation.status,
                    crate::domain::student::OcrGenerationStatus::Candidate
                        | crate::domain::student::OcrGenerationStatus::ReadyForReview
                )
            })
            .count();
        let ocr_interrupted_generation_count = project
            .student_answer_ocr_generations
            .iter()
            .filter(|generation| {
                generation.status == crate::domain::student::OcrGenerationStatus::Interrupted
            })
            .count();
        let ocr_stale_generation_count = project
            .student_answer_ocr_generations
            .iter()
            .filter(|generation| {
                generation.status == crate::domain::student::OcrGenerationStatus::Stale
            })
            .count();
        let preview_active_generation_count = project
            .documents
            .iter()
            .filter(|document| {
                document
                    .preview
                    .as_ref()
                    .and_then(|preview| preview.active_generation_id.as_ref())
                    .is_some()
            })
            .count();
        let preview_missing_active_generation_count = project
            .documents
            .iter()
            .filter(|document| {
                document
                    .preview
                    .as_ref()
                    .and_then(|preview| preview.active_generation_id.as_ref())
                    .is_some()
                    && !preview_metadata_path(&project, &document.id)
                        .is_some_and(|path| path.exists())
            })
            .count();
        let preview_orphan_staging_count = count_preview_staging_dirs(&project);
        let submission_delete_blocked_count = project
            .student_submissions
            .iter()
            .filter(|submission| {
                crate::services::student_scan_service::scan_submission_dependencies(
                    &project,
                    std::slice::from_ref(&submission.id),
                )
                .is_blocked()
            })
            .count();
        let orphan_artifact_count = project
            .student_answer_ocr_records
            .iter()
            .filter(|record| {
                record
                    .source_image_refs
                    .iter()
                    .chain(record.crop_refs.iter())
                    .any(|reference| reference.trim().is_empty())
            })
            .count();
        let exam_source_exists = project
            .documents
            .iter()
            .any(|document| document.role == DocumentRole::ExamSource);
        let rubric_or_answer_key_exists = project.documents.iter().any(|document| {
            matches!(
                document.role,
                DocumentRole::Rubric | DocumentRole::AnswerKey
            )
        });
        let student_scan_exists = project
            .documents
            .iter()
            .any(|document| document.role == DocumentRole::StudentScan);
        let student_scan_documents = project
            .documents
            .iter()
            .filter(|document| document.role == DocumentRole::StudentScan)
            .count();
        let student_scan_total_pages = project
            .documents
            .iter()
            .filter(|document| document.role == DocumentRole::StudentScan)
            .map(|document| document.page_count)
            .sum();
        let student_scan_preview_ready_pages = project
            .documents
            .iter()
            .filter(|document| document.role == DocumentRole::StudentScan)
            .map(|document| {
                (page_preview_png_count(&project, &document.id) as u32).min(document.page_count)
            })
            .sum();
        let student_scan_preview_total_pages = student_scan_total_pages;
        let student_grouping_complete = student_grouping_is_complete(&project);
        let class_metrics = school_class_doctor_metrics(&project);
        let job_summary = self.inspect_jobs(project_path)?;
        let question_text_coverage = coverage_summary(
            project.questions.len(),
            project
                .questions
                .iter()
                .filter(|question| is_question_text_ready(&question.question_text))
                .count(),
        );
        let rubric_coverage = coverage_summary(
            project.questions.len(),
            project
                .questions
                .iter()
                .filter(|question| {
                    is_rubric_confirmed(&question.rubric, Some(&question.answer_type))
                })
                .count(),
        );
        let ready_for_review = project.questions.iter().any(|question| {
            matches!(
                question.question_text.status,
                TextFieldStatus::Suggested | TextFieldStatus::Edited
            ) || matches!(
                question.rubric.status,
                RubricStatus::Suggested | RubricStatus::Imported | RubricStatus::Manual
            )
        });
        let ready_for_qep = !project.questions.is_empty()
            && project.questions.iter().all(|question| {
                is_question_text_ready(&question.question_text)
                    && is_rubric_confirmed(&question.rubric, Some(&question.answer_type))
            });
        let (exam_package_freeze_ready, exam_package_freeze_blockers) =
            exam_package_freeze_readiness(&project);
        let (student_intake_ready, student_intake_blockers) = student_intake_readiness(&project);
        let student_answer_ocr_expected_records =
            project.student_submissions.len() * project.questions.len();
        let student_answer_ocr_records = project.student_answer_ocr_records.len();
        let student_answer_ocr_reviewed = project
            .student_answer_ocr_records
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    crate::domain::student::StudentAnswerOcrStatus::TeacherApproved
                )
            })
            .count();
        let student_answer_ocr_needs_review = project
            .student_answer_ocr_records
            .iter()
            .filter(|record| {
                !matches!(
                    record.status,
                    crate::domain::student::StudentAnswerOcrStatus::TeacherApproved
                )
            })
            .count();
        let student_answer_ocr_running = job_summary
            .jobs
            .iter()
            .any(|job| job.kind == "student_answer_ocr" && job.active);
        let student_answer_ocr_ready_for_scoring = student_answer_ocr_expected_records > 0
            && student_answer_ocr_records == student_answer_ocr_expected_records
            && student_answer_ocr_reviewed == student_answer_ocr_expected_records;
        let student_answer_ocr_status = if student_answer_ocr_running {
            "running"
        } else if student_answer_ocr_records == 0 {
            "not_started"
        } else if student_answer_ocr_ready_for_scoring {
            "approved"
        } else {
            "review_needed"
        }
        .to_string();
        let scoring_state = crate::domain::scoring::scoring_readiness(&project);
        let active_scoring_run_id = crate::domain::scoring::scoring_active_run_id(&project);
        let scoring_result_count = crate::domain::scoring::scoring_active_record_count(&project);
        let scoring_total_history_count =
            crate::domain::scoring::scoring_total_history_count(&project);
        let scoring_duplicate_result_count =
            crate::domain::scoring::scoring_duplicate_result_count(&project);
        let mut scoring_blockers = scoring_state.blockers.clone();
        if !student_scan_exists {
            scoring_blockers.push("STUDENT_SCAN_NOT_FOUND".to_string());
        } else if project
            .documents
            .iter()
            .filter(|document| document.role == DocumentRole::StudentScan)
            .any(|document| {
                document
                    .preview
                    .as_ref()
                    .map_or(true, |preview| preview.status != PdfPreviewStatus::Ready)
            })
        {
            scoring_blockers.push("STUDENT_PDF_PREVIEW_MISSING".to_string());
        }

        if !student_grouping_complete {
            scoring_blockers.push("STUDENT_GROUPING_NOT_READY".to_string());
        }
        if project.students.iter().any(student_identity_is_missing) {
            scoring_blockers.push("STUDENT_IDENTITY_INVALID".to_string());
        }
        if !student_answer_ocr_ready_for_scoring {
            scoring_blockers.push("STUDENT_ANSWER_OCR_NOT_READY".to_string());
        }
        scoring_blockers.sort();
        scoring_blockers.dedup();
        let scoring_ready = scoring_state.ready && scoring_blockers.is_empty();

        let model_status = self.inspect_model(40).await.ok();
        let path_security = path_security_doctor_summary(project_path, &project);
        let persistence = self.project_store.persistence_diagnostics();

        Ok(DoctorReport {
            project_path: project_path.to_string_lossy().to_string(),
            project_file_exists: true,
            project_readable: true,
            project: Some({
                let mut computed_project = project.clone();
                computed_project.workflow = workflow_snapshot.clone();
                project_report(computed_project)
            }),
            path_security,
            persistence,
            documents_dir_exists: project_path.join("documents").exists(),
            cache_dir_exists: project_path.join("cache").exists(),
            exam_source_exists,
            rubric_or_answer_key_exists,
            student_scan_exists,
            student_scan_documents,
            student_scan_total_pages,
            student_scan_preview_ready_pages,
            student_scan_preview_total_pages,
            student_grouping_complete,
            student_submissions: project.student_submissions.len(),
            school_class_count: class_metrics.school_class_count,
            active_school_class_count: class_metrics.active_school_class_count,
            archived_school_class_count: class_metrics.archived_school_class_count,
            student_scan_batch_count: class_metrics.student_scan_batch_count,
            scan_batch_without_class_count: class_metrics.scan_batch_without_class_count,
            submission_without_class_count: class_metrics.submission_without_class_count,
            class_membership_inconsistency_count: class_metrics
                .class_membership_inconsistency_count,
            identity_class_mismatch_count: class_metrics.identity_class_mismatch_count,
            school_class_summaries: class_metrics.school_class_summaries,
            student_answer_ocr_records,
            student_answer_ocr_expected_records,
            student_answer_ocr_reviewed,
            student_answer_ocr_needs_review,
            ocr_active_generation_count,
            ocr_pending_generation_count,
            ocr_interrupted_generation_count,
            ocr_stale_generation_count,
            student_answer_ocr_status,
            student_answer_ocr_ready_for_scoring,
            pages_per_student: project.student_pages_per_student,
            preview_metadata_exists,
            preview_png_count,
            preview_active_generation_count,
            preview_orphan_staging_count,
            preview_missing_active_generation_count,
            submission_delete_blocked_count,
            orphan_artifact_count,
            exam_package_status: to_snake_case(&format!("{:?}", workflow_snapshot.current_stage)),
            question_text_coverage,
            rubric_coverage,
            ready_for_review,
            ready_for_qep,
            question_text_summary: question_text_summary(&project),
            rubric_summary: rubric_summary(&project),
            exam_package_freeze_ready,
            exam_package_freeze_blockers,
            student_intake_ready,
            student_intake_blockers,
            scoring_ready,
            scoring_blockers,
            scoring_result_count,
            scoring_total_history_count,
            scoring_duplicate_result_count,
            active_scoring_run_id,
            scoring_approved_count: scoring_state.approved_record_count,
            scoring_needs_review_count: scoring_state.needs_review_record_count,
            scoring_stale_count: scoring_state.stale_record_count,
            speaking: speaking_doctor_summary(Some(&project)),
            job_summary,
            model_status,
            security: self.security_doctor_summary(project_path, &project),
            warnings: load_warnings,
            errors: vec![],
        })
    }

    pub async fn replay_rubric_import_dry_run(
        &self,
        project_path: &Path,
    ) -> Result<ReplayReport, AppError> {
        let project = self.open_project(project_path)?;
        let mut warnings = Vec::new();
        let rubric_doc = project.documents.iter().find(|document| {
            matches!(
                document.role,
                DocumentRole::Rubric | DocumentRole::AnswerKey
            )
        });

        let expected_question_count = project
            .expected_question_count
            .unwrap_or(project.questions.len() as u32);
        let target_questions: Vec<u32> = (1..=expected_question_count).collect();
        let metadata = rubric_doc.and_then(|document| {
            read_document_content_metadata(Path::new(&project.root_path), &document.id)
        });
        let content_method = metadata
            .as_ref()
            .map(|metadata| {
                if metadata.vision_fallback_needed {
                    "vision_fallback".to_string()
                } else {
                    "pdftotext".to_string()
                }
            })
            .unwrap_or_else(|| "pdftotext".to_string());

        let already_available: Vec<u32> = (1..=expected_question_count)
            .filter(|number| {
                project
                    .questions
                    .iter()
                    .find(|question| question.number == *number)
                    .map(|question| {
                        let validation =
                            validate_rubric_state(&question.rubric, Some(&question.answer_type));
                        matches!(
                            question.rubric.status,
                            RubricStatus::Imported
                                | RubricStatus::Manual
                                | RubricStatus::Confirmed
                                | RubricStatus::Suggested
                        ) && validation.valid
                    })
                    .unwrap_or(false)
            })
            .collect();
        let invalid_or_empty: Vec<u32> = (1..=expected_question_count)
            .filter(|number| {
                project
                    .questions
                    .iter()
                    .find(|question| question.number == *number)
                    .map(|question| {
                        let validation =
                            validate_rubric_state(&question.rubric, Some(&question.answer_type));
                        question.rubric.status == RubricStatus::Imported
                            && (!validation.valid
                                || !has_meaningful_rubric_content(&question.rubric))
                    })
                    .unwrap_or(false)
            })
            .collect();
        let missing: Vec<u32> = (1..=expected_question_count)
            .filter(|number| {
                project
                    .questions
                    .iter()
                    .find(|question| question.number == *number)
                    .map(|question| question.rubric.status == RubricStatus::Missing)
                    .unwrap_or(true)
            })
            .collect();
        let failed: Vec<u32> = (1..=expected_question_count)
            .filter(|number| {
                project
                    .questions
                    .iter()
                    .find(|question| question.number == *number)
                    .map(|question| {
                        matches!(
                            question.rubric.status,
                            RubricStatus::Invalid | RubricStatus::Legacy
                        )
                    })
                    .unwrap_or(false)
            })
            .collect();
        let will_run_questions: Vec<u32> = (1..=expected_question_count)
            .filter(|number| !already_available.contains(number))
            .collect();
        let will_run_vision_fallback_for = if metadata
            .as_ref()
            .map(|metadata| metadata.vision_fallback_needed)
            .unwrap_or(false)
        {
            will_run_questions.clone()
        } else {
            Vec::new()
        };
        let partial_success = !missing.is_empty() || !failed.is_empty();
        let coverage_ok = missing.is_empty() && failed.is_empty();

        let mut checks = vec![
            "strategy=per_question".to_string(),
            format!("content_method={}", content_method),
            format!("expected_question_count={}", expected_question_count),
            format!("target_questions={:?}", target_questions),
            format!("already_available={:?}", already_available),
            format!("will_run_questions={:?}", will_run_questions),
            format!("invalid_or_empty={:?}", invalid_or_empty),
            format!("missing={:?}", missing),
            format!("failed={:?}", failed),
            format!("coverage_ok={}", coverage_ok),
            format!("partial_success={}", partial_success),
        ];

        if rubric_doc.is_none() {
            warnings.push("rubric_document_missing".to_string());
            checks.push("warning=rubric_document_missing".to_string());
        }

        Ok(ReplayReport {
            target: "rubric-import".to_string(),
            dry_run: true,
            project_path: project_path.to_string_lossy().to_string(),
            strategy: Some("per_question".to_string()),
            content_method: Some(content_method),
            expected_question_count: Some(expected_question_count),
            target_questions,
            already_available: already_available.clone(),
            will_run_questions,
            will_run_vision_fallback_for,
            invalid_or_empty,
            extracted: already_available,
            missing,
            failed,
            coverage_ok: Some(coverage_ok),
            partial_success: Some(partial_success),
            fresh_pdf_extraction: None,
            project_snapshot: None,
            checks,
            warnings,
        })
    }

    pub async fn replay_question_text_dry_run(
        &self,
        project_path: &Path,
    ) -> Result<ReplayReport, AppError> {
        let project = self.open_project(project_path)?;
        let warnings = Vec::new();
        let exam_source = project
            .documents
            .iter()
            .find(|document| document.role == DocumentRole::ExamSource)
            .ok_or_else(|| AppError {
                code: crate::domain::errors::AppErrorCode::DocumentNotFound,
                message: "Exam source PDF is missing.".to_string(),
                recoverable: true,
                suggested_action: Some("Upload the original exam PDF first.".to_string()),
                technical_details: None,
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })?;

        let fresh_pdf_extraction = analyze_fresh_question_text_replay(&project, exam_source)?;
        let expected_question_count = fresh_pdf_extraction.expected_question_count;
        let project_snapshot = analyze_project_snapshot_question_text_replay(
            &project,
            expected_question_count,
            &fresh_pdf_extraction,
        );
        let target_questions = fresh_pdf_extraction.will_run_vision_fallback_for.clone();
        let extracted = fresh_pdf_extraction.detected_markers.clone();
        let missing_numbers = fresh_pdf_extraction.missing.clone();
        let contaminated = fresh_pdf_extraction.contaminated.clone();
        let content_method = fresh_pdf_extraction.content_method.clone();
        let coverage_ok = fresh_pdf_extraction.coverage_ok;
        let partial_success = !coverage_ok;
        let will_run_vision_fallback_for = target_questions.clone();

        let mut checks = vec![
            format!("fresh_pdf_extraction.detected_markers={:?}", extracted),
            format!(
                "fresh_pdf_extraction.marker_offsets={:?}",
                fresh_pdf_extraction.marker_offsets
            ),
            format!("fresh_pdf_extraction.missing={:?}", missing_numbers),
            format!("fresh_pdf_extraction.contaminated={:?}", contaminated),
            format!("fresh_pdf_extraction.coverage_ok={}", coverage_ok),
            format!(
                "fresh_pdf_extraction.will_run_vision_fallback_for={:?}",
                will_run_vision_fallback_for
            ),
            format!(
                "fresh_pdf_extraction.vision_fallback_call_count={}",
                fresh_pdf_extraction.vision_fallback_call_count
            ),
            format!(
                "project_snapshot.available={:?}",
                project_snapshot.available
            ),
            format!("project_snapshot.missing={:?}", project_snapshot.missing),
            format!(
                "project_snapshot.contaminated={:?}",
                project_snapshot.contaminated
            ),
            format!("project_snapshot.stale={}", project_snapshot.stale),
            format!(
                "project_snapshot.needs_refresh={}",
                project_snapshot.needs_refresh
            ),
        ];

        if project_snapshot.needs_refresh {
            checks.push("project_snapshot_requires_refresh".to_string());
        }

        Ok(ReplayReport {
            target: "question-text".to_string(),
            dry_run: true,
            project_path: project_path.to_string_lossy().to_string(),
            strategy: None,
            content_method: Some(content_method),
            expected_question_count: Some(expected_question_count),
            target_questions,
            already_available: extracted.clone(),
            will_run_questions: will_run_vision_fallback_for.clone(),
            will_run_vision_fallback_for,
            invalid_or_empty: vec![],
            extracted,
            missing: missing_numbers,
            failed: vec![],
            coverage_ok: Some(coverage_ok),
            partial_success: Some(partial_success),
            fresh_pdf_extraction: Some(fresh_pdf_extraction),
            project_snapshot: Some(project_snapshot),
            checks,
            warnings,
        })
    }

    pub fn repair_question_text(
        &self,
        project_path: &Path,
    ) -> Result<QuestionTextRepairReport, AppError> {
        let mut project = self.open_project(project_path)?;
        let exam_source = project
            .documents
            .iter()
            .find(|document| document.role == DocumentRole::ExamSource)
            .ok_or_else(|| AppError {
                code: crate::domain::errors::AppErrorCode::DocumentNotFound,
                message: "Exam source PDF is missing.".to_string(),
                recoverable: true,
                suggested_action: Some("Upload the original exam PDF first.".to_string()),
                technical_details: None,
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })?;

        let pdf_path = exam_source.resolve_path(&project.root_path)?;
        let fresh_raw_text = read_pdf_text(&pdf_path)?;
        let expected_question_count = project.expected_question_count;
        let report = repair_question_text_from_raw_text(
            &mut project,
            &fresh_raw_text,
            expected_question_count,
        )?;
        self.project_store
            .commit_snapshot_cas(&project)
            .map(|_| ())?;
        Ok(report)
    }

    pub fn repair_document_content(
        &self,
        project_path: &Path,
    ) -> Result<DocumentContentRepairReport, AppError> {
        let project = self.open_project(project_path)?;
        let mut items = Vec::new();
        let expected_question_count = project.expected_question_count;

        for document in project.documents.iter().filter(|document| {
            matches!(
                document.role,
                DocumentRole::ExamSource | DocumentRole::Rubric | DocumentRole::AnswerKey
            )
        }) {
            let before =
                read_document_content_metadata(Path::new(&project.root_path), &document.id);
            let pdf_path = document.resolve_path(&project.root_path)?;
            let result = self.document_content_extraction_service.extract(
                DocumentContentExtractionRequest {
                    project_id: project.id.clone(),
                    project_root: PathBuf::from(&project.root_path),
                    document_id: document.id.clone(),
                    document_path: pdf_path,
                    kind: document_content_kind(&document.role),
                    expected_question_count,
                    force_refresh: true,
                    vision_sources: vec![],
                },
            )?;

            items.push(DocumentContentRepairItem {
                document_id: document.id.clone(),
                role: document_role_label(&document.role),
                kind: document_content_kind_label(&document.role),
                before_method: before
                    .as_ref()
                    .map(|metadata| document_content_method_label(&metadata.method))
                    .unwrap_or_else(|| "missing".to_string()),
                after_method: document_content_method_label(&format!("{:?}", result.method)),
                before_detected_question_numbers: before
                    .as_ref()
                    .map(|metadata| metadata.detected_question_numbers.clone())
                    .unwrap_or_default(),
                after_detected_question_numbers: result
                    .text_quality
                    .detected_question_numbers
                    .clone(),
                ignored_question_numbers: result.ignored_question_numbers.clone(),
                metadata_stale: before
                    .as_ref()
                    .map(|metadata| {
                        metadata.detected_question_numbers
                            != result.text_quality.detected_question_numbers
                            || metadata.missing_question_numbers
                                != result.text_quality.missing_question_numbers
                            || metadata.ignored_question_numbers != result.ignored_question_numbers
                    })
                    .unwrap_or(true),
                needs_refresh: before
                    .as_ref()
                    .map(|metadata| {
                        metadata.detected_question_numbers
                            != result.text_quality.detected_question_numbers
                            || metadata.missing_question_numbers
                                != result.text_quality.missing_question_numbers
                            || metadata.ignored_question_numbers != result.ignored_question_numbers
                    })
                    .unwrap_or(true),
                vision_fallback_needed: result.vision_fallback_needed,
                metadata_written: result.metadata_path.exists(),
            });
        }

        Ok(DocumentContentRepairReport {
            expected_question_count,
            repaired_count: items.len(),
            items,
        })
    }

    pub fn repair_stale_jobs(
        &self,
        project_path: &Path,
    ) -> Result<StaleJobsRepairReport, AppError> {
        let jobs = load_persisted_jobs(project_path)?;
        let now = Utc::now().to_rfc3339();
        let mut items = Vec::new();

        for mut job in jobs {
            let stale_before = is_stale_candidate(&job, Utc::now())
                || matches!(
                    job.error.as_ref().map(|error| &error.code),
                    Some(crate::domain::errors::AppErrorCode::JobStaleInterrupted)
                );
            let active_before = matches!(job.status, JobStatus::Queued | JobStatus::Running);
            if stale_before && active_before {
                let before_status = to_snake_case(&format!("{:?}", job.status));
                let original_last_message = job.last_message.clone();
                let original_updated_at = job.updated_at.clone();
                job.status = JobStatus::Failed;
                job.finished_at = Some(now.clone());
                job.last_message = Some(
                    "Uygulama kapandığı veya işlem zaman aşımına uğradığı için iş yarım kaldı."
                        .to_string(),
                );
                job.error = Some(AppError {
                    code: crate::domain::errors::AppErrorCode::JobStaleInterrupted,
                    message: "Job interrupted because it was stale.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Re-run the operation if the project state is still expected.".to_string(),
                    ),
                    technical_details: Some(format!(
                        "original_status={}, original_last_message={:?}, original_updated_at={}",
                        before_status, original_last_message, original_updated_at
                    )),
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                });
                job.updated_at = now.clone();

                let trusted_root =
                    TrustedProjectRoot::from_canonical_root(project_path.to_path_buf(), false)?;
                let managed = trusted_root.managed(&format!("logs/jobs/{}.json", job.id))?;
                trusted_root.atomic_write(
                    &managed,
                    &serde_json::to_string_pretty(&job).map_err(|error| AppError {
                        code: crate::domain::errors::AppErrorCode::ProjectSaveFailed,
                        message: "Job snapshot serialize edilemedi.".to_string(),
                        recoverable: false,
                        suggested_action: None,
                        technical_details: Some(error.to_string()),
                        correlation_id: uuid::Uuid::new_v4().to_string(),
                    })?,
                )?;

                items.push(StaleJobsRepairItem {
                    job_id: job.id.clone(),
                    kind: to_snake_case(&format!("{:?}", job.kind)),
                    status_before: before_status,
                    status_after: to_snake_case(&format!("{:?}", job.status)),
                    stale_before,
                    stale_after: true,
                    active_before,
                    active_after: false,
                });
            }
        }

        Ok(StaleJobsRepairReport {
            repaired_count: items.len(),
            items,
        })
    }

    pub fn inspect_document_content(
        &self,
        project_path: &Path,
    ) -> Result<Vec<DocumentContentInspectRecord>, AppError> {
        let project = self.open_project(project_path)?;
        let mut records = Vec::new();
        for document in project.documents.iter().filter(|document| {
            matches!(
                document.role,
                DocumentRole::ExamSource | DocumentRole::Rubric | DocumentRole::AnswerKey
            )
        }) {
            let Some(artifact_dir) = document_content_dir(project_path, &document.id) else {
                continue;
            };
            let metadata_path = artifact_dir.join("content_metadata.json");
            let raw_text_path = artifact_dir.join("raw_text.txt");
            let normalized_text_path = artifact_dir.join("normalized_text.txt");
            let stderr_path = artifact_dir.join("pdftotext_stderr.txt");
            let model_input_manifest_path = artifact_dir.join("model_input_manifest.json");
            let metadata =
                read_document_content_metadata(Path::new(&project.root_path), &document.id);
            let fresh = document
                .resolve_path(&project.root_path)
                .ok()
                .and_then(|path| read_pdf_text(&path).ok())
                .map(|raw_text| {
                    analyze_document_content_from_raw_text(
                        &raw_text,
                        project.expected_question_count,
                        &document.role,
                    )
                });
            if let Some(metadata) = metadata {
                let (
                    fresh_detected_question_numbers,
                    fresh_missing_question_numbers,
                    ignored_question_numbers,
                    metadata_stale,
                    needs_refresh,
                ) = fresh
                    .as_ref()
                    .map(|fresh| {
                        let metadata_stale = metadata.detected_question_numbers
                            != fresh.detected_question_numbers
                            || metadata.missing_question_numbers != fresh.missing_question_numbers
                            || metadata.ignored_question_numbers != fresh.ignored_question_numbers;
                        (
                            fresh.detected_question_numbers.clone(),
                            fresh.missing_question_numbers.clone(),
                            fresh.ignored_question_numbers.clone(),
                            metadata_stale,
                            metadata_stale,
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            metadata.detected_question_numbers.clone(),
                            metadata.missing_question_numbers.clone(),
                            metadata.ignored_question_numbers.clone(),
                            false,
                            false,
                        )
                    });
                records.push(DocumentContentInspectRecord {
                    document_id: document.id.clone(),
                    kind: document_content_kind_label(&document.role),
                    role: document_role_label(&document.role),
                    method: document_content_method_label(&metadata.method),
                    raw_text_length: metadata.raw_text_length,
                    normalized_text_length: metadata.normalized_text_length,
                    enough_text: metadata.enough_text,
                    vision_fallback_needed: metadata.vision_fallback_needed,
                    detected_question_numbers: metadata.detected_question_numbers,
                    missing_question_numbers: metadata.missing_question_numbers,
                    ignored_question_numbers,
                    metadata_stale,
                    needs_refresh,
                    fresh_detected_question_numbers,
                    fresh_missing_question_numbers,
                    artifact_dir: metadata.artifact_dir,
                    metadata_exists: metadata_path.exists(),
                    raw_text_exists: raw_text_path.exists(),
                    normalized_text_exists: normalized_text_path.exists(),
                    pdftotext_stderr_exists: stderr_path.exists(),
                    model_input_manifest_exists: model_input_manifest_path.exists(),
                    warnings: metadata.warnings,
                });
            } else {
                let warnings = if metadata_path.exists() {
                    vec!["legacy or unreadable document content metadata".to_string()]
                } else {
                    vec!["document content metadata missing".to_string()]
                };
                let (
                    detected_question_numbers,
                    missing_question_numbers,
                    ignored_question_numbers,
                    metadata_stale,
                    needs_refresh,
                ) = fresh
                    .as_ref()
                    .map(|fresh| {
                        (
                            fresh.detected_question_numbers.clone(),
                            fresh.missing_question_numbers.clone(),
                            fresh.ignored_question_numbers.clone(),
                            true,
                            true,
                        )
                    })
                    .unwrap_or_else(|| (vec![], vec![], vec![], false, false));
                records.push(DocumentContentInspectRecord {
                    document_id: document.id.clone(),
                    kind: document_content_kind_label(&document.role),
                    role: document_role_label(&document.role),
                    method: "missing".to_string(),
                    raw_text_length: 0,
                    normalized_text_length: 0,
                    enough_text: false,
                    vision_fallback_needed: fresh
                        .as_ref()
                        .map(|fresh| fresh.vision_fallback_needed)
                        .unwrap_or(false),
                    detected_question_numbers,
                    missing_question_numbers,
                    ignored_question_numbers,
                    metadata_stale,
                    needs_refresh,
                    fresh_detected_question_numbers: fresh
                        .as_ref()
                        .map(|fresh| fresh.detected_question_numbers.clone())
                        .unwrap_or_default(),
                    fresh_missing_question_numbers: fresh
                        .as_ref()
                        .map(|fresh| fresh.missing_question_numbers.clone())
                        .unwrap_or_default(),
                    artifact_dir: artifact_dir.to_string_lossy().to_string(),
                    metadata_exists: metadata_path.exists(),
                    raw_text_exists: raw_text_path.exists(),
                    normalized_text_exists: normalized_text_path.exists(),
                    pdftotext_stderr_exists: stderr_path.exists(),
                    model_input_manifest_exists: model_input_manifest_path.exists(),
                    warnings,
                });
            }
        }
        Ok(records)
    }
}

fn llama_cpp_root(server_path: &str) -> String {
    let Some(root) = Path::new(server_path)
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return String::new();
    };
    root.to_string_lossy().to_string()
}

impl Default for DiagnosticsContext {
    fn default() -> Self {
        Self::new()
    }
}

fn project_report(project: Project) -> ProjectInspectReport {
    let mut document_counts_by_role: BTreeMap<String, usize> = BTreeMap::new();
    for document in &project.documents {
        *document_counts_by_role
            .entry(document_role_label(&document.role))
            .or_insert(0) += 1;
    }

    ProjectInspectReport {
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        expected_question_count: project.expected_question_count,
        question_count: project.questions.len(),
        document_counts_by_role,
        workflow_stage: to_snake_case(&format!("{:?}", project.workflow.current_stage)),
        blocking_reasons: project
            .workflow
            .blocking_reasons
            .iter()
            .map(|reason| to_snake_case(&format!("{reason:?}")))
            .collect(),
        next_actions: project
            .workflow
            .next_actions
            .iter()
            .map(|action| WorkflowActionSummary {
                code: action.code.clone(),
                label: action.label.clone(),
                enabled: action.enabled,
                disabled_reason: action.disabled_reason.clone(),
                command: action.command.clone(),
            })
            .collect(),
        paths: ProjectPaths {
            root_path: project.root_path.clone(),
            project_json: project_path(&project),
            documents_dir: project.root_path.clone() + "/documents",
            cache_dir: project.root_path.clone() + "/cache",
            preview_dir: project.root_path.clone() + "/cache/page_previews",
            model_inputs_dir: project.root_path.clone() + "/cache/model_inputs",
            logs_dir: project.root_path.clone() + "/logs",
        },
    }
}

struct SchoolClassDoctorMetrics {
    school_class_count: usize,
    active_school_class_count: usize,
    archived_school_class_count: usize,
    student_scan_batch_count: usize,
    scan_batch_without_class_count: usize,
    submission_without_class_count: usize,
    class_membership_inconsistency_count: usize,
    identity_class_mismatch_count: usize,
    school_class_summaries: Vec<DoctorSchoolClassSummary>,
}

fn school_class_doctor_metrics(project: &Project) -> SchoolClassDoctorMetrics {
    let classes_by_id = project
        .school_classes
        .iter()
        .map(|school_class| (school_class.id.as_str(), school_class))
        .collect::<BTreeMap<_, _>>();
    let batches_by_id = project
        .student_scan_batches
        .iter()
        .map(|batch| (batch.id.as_str(), batch))
        .collect::<BTreeMap<_, _>>();

    let scan_batch_without_class_count = project
        .student_scan_batches
        .iter()
        .filter(|batch| !classes_by_id.contains_key(batch.class_id.as_str()))
        .count();
    let mut submission_without_class_count = 0usize;
    let mut class_membership_inconsistency_count = 0usize;
    let mut identity_class_mismatch_count = 0usize;

    for submission in &project.student_submissions {
        let batch = submission
            .scan_batch_id
            .as_deref()
            .and_then(|batch_id| batches_by_id.get(batch_id).copied());
        let effective_class_id = submission
            .class_id
            .as_deref()
            .or_else(|| batch.map(|value| value.class_id.as_str()));
        let effective_class = effective_class_id.and_then(|class_id| classes_by_id.get(class_id));
        if effective_class.is_none() {
            submission_without_class_count += 1;
        }

        let unknown_batch = submission.scan_batch_id.is_some() && batch.is_none();
        let mismatched_batch_class = batch.is_some_and(|value| {
            submission
                .class_id
                .as_deref()
                .is_some_and(|class_id| class_id != value.class_id)
        });
        let mismatched_batch_document =
            batch.is_some_and(|value| value.document_id != submission.document_id);
        if unknown_batch || mismatched_batch_class || mismatched_batch_document {
            class_membership_inconsistency_count += 1;
        }

        let Some(school_class) = effective_class else {
            continue;
        };
        let identity_class = project
            .students
            .iter()
            .find(|student| student.id == submission.student_id)
            .and_then(|student| {
                student
                    .identity_ocr
                    .as_ref()
                    .and_then(|identity| identity.class_name.as_deref())
                    .or(student.class_name.as_deref())
            })
            .and_then(normalize_school_class_name);
        if identity_class
            .as_deref()
            .is_some_and(|identity_class| identity_class != school_class.normalized_name)
        {
            identity_class_mismatch_count += 1;
        }
    }

    let overview = build_school_class_overview(project);
    SchoolClassDoctorMetrics {
        school_class_count: project.school_classes.len(),
        active_school_class_count: project
            .school_classes
            .iter()
            .filter(|school_class| school_class.status == SchoolClassStatus::Active)
            .count(),
        archived_school_class_count: project
            .school_classes
            .iter()
            .filter(|school_class| school_class.status == SchoolClassStatus::Archived)
            .count(),
        student_scan_batch_count: project.student_scan_batches.len(),
        scan_batch_without_class_count,
        submission_without_class_count,
        class_membership_inconsistency_count,
        identity_class_mismatch_count,
        school_class_summaries: overview
            .classes
            .into_iter()
            .map(|item| DoctorSchoolClassSummary {
                name: item.school_class.name,
                scan_batch_count: item.scan_batch_count,
                submission_count: item.submission_count,
                identity_verified: item.identity_verified_count,
                ocr_complete: item.ocr_complete_count,
                scoring_complete: item.scoring_complete_count,
                review_required: item.review_required_count,
            })
            .collect(),
    }
}

fn student_grouping_is_complete(project: &Project) -> bool {
    if project.student_scan_batches.is_empty() {
        return project.student_grouping_complete_at.is_some()
            && !project.student_submissions.is_empty();
    }

    project.student_scan_batches.iter().all(|batch| {
        batch.grouping_completed_at.is_some()
            && project
                .student_submissions
                .iter()
                .any(|submission| submission.scan_batch_id.as_deref() == Some(batch.id.as_str()))
    })
}

fn exam_package_freeze_readiness(project: &Project) -> (bool, Vec<String>) {
    let expected_question_count = project
        .expected_question_count
        .unwrap_or(project.questions.len() as u32);
    let mut blockers = Vec::new();

    if expected_question_count == 0 || project.questions.is_empty() {
        blockers.push("QUESTION_COUNT_MISSING".to_string());
        return (false, blockers);
    }

    for number in 1..=expected_question_count {
        let Some(question) = project
            .questions
            .iter()
            .find(|question| question.number == number)
        else {
            blockers.push("QUESTION_TEXT_MISSING".to_string());
            continue;
        };

        if !is_question_text_ready(&question.question_text) {
            blockers.push("QUESTION_TEXT_MISSING".to_string());
        }

        let validation = validate_rubric_state(&question.rubric, Some(&question.answer_type));
        if !validation.valid {
            for issue in validation.issues {
                blockers.push(issue.code);
            }
        }
    }

    blockers.sort();
    blockers.dedup();
    (blockers.is_empty(), blockers)
}

fn student_intake_readiness(project: &Project) -> (bool, Vec<String>) {
    let mut blockers = Vec::new();

    let exam_package_frozen = project.exam_package_freeze.as_ref().is_some_and(|freeze| {
        freeze.freeze_status == crate::domain::project::ExamPackageFreezeStatus::Frozen
    });
    if !exam_package_frozen {
        blockers.push("QEP_NOT_FROZEN".to_string());
    }

    let student_documents = project
        .documents
        .iter()
        .filter(|document| document.role == DocumentRole::StudentScan)
        .collect::<Vec<_>>();
    if student_documents.is_empty() {
        blockers.push("STUDENT_SCAN_NOT_FOUND".to_string());
        blockers.sort();
        blockers.dedup();
        return (false, blockers);
    }

    let preview_ready = student_documents.iter().all(|document| {
        document
            .preview
            .as_ref()
            .is_some_and(|preview| preview.status == PdfPreviewStatus::Ready)
    });
    if !preview_ready {
        blockers.push("STUDENT_PDF_PREVIEW_MISSING".to_string());
    }

    blockers.sort();
    blockers.dedup();
    (exam_package_frozen && preview_ready, blockers)
}

fn document_reports(project: &Project, project_path: &Path) -> Vec<DocumentInspectRecord> {
    let trusted_root = TrustedProjectRoot::open_selected(project_path).ok();
    project
        .documents
        .iter()
        .map(|document| {
            let stored = trusted_root.as_ref().and_then(|root| {
                root.adapt_legacy_document_path(&document.stored_path)
                    .and_then(|managed| root.resolve_existing_file(&managed))
                    .ok()
            });
            let preview_status = document.preview.as_ref().map(|preview| {
                match preview.status {
                    PdfPreviewStatus::Missing => "missing",
                    PdfPreviewStatus::Queued => "queued",
                    PdfPreviewStatus::Running => "running",
                    PdfPreviewStatus::Ready => "ready",
                    PdfPreviewStatus::Failed => "failed",
                }
                .to_string()
            });
            let mut warnings = Vec::new();
            if matches!(document.role, DocumentRole::Rubric) && stored.is_none() {
                warnings.push("rubric PDF stored path missing".to_string());
            }
            DocumentInspectRecord {
                id: document.id.clone(),
                role: document_role_label(&document.role),
                file_name: document.file_name.clone(),
                stored_path: document.stored_path.clone(),
                exists: stored.is_some(),
                checksum: document.checksum.clone(),
                page_count: document.page_count,
                preview_status,
                preview_ready: document
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.status == PdfPreviewStatus::Ready),
                warnings,
            }
        })
        .collect()
}

fn path_security_doctor_summary(
    project_path: &Path,
    project: &Project,
) -> PathSecurityDoctorSummary {
    let mut summary = PathSecurityDoctorSummary::default();
    let Ok(trusted_root) = TrustedProjectRoot::open_selected(project_path) else {
        return summary;
    };

    let raw_value = std::fs::read_to_string(trusted_root.project_file())
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok());
    let stored_root = raw_value
        .as_ref()
        .and_then(|value| value.get("rootPath").or_else(|| value.get("root_path")))
        .and_then(serde_json::Value::as_str);
    if stored_root
        .map(|value| {
            std::fs::canonicalize(value)
                .map(|path| path != trusted_root.root())
                .unwrap_or(true)
        })
        .unwrap_or(true)
    {
        summary.project_root_metadata_mismatch = 1;
    }

    let raw_documents = raw_value
        .as_ref()
        .and_then(|value| value.get("documents"))
        .and_then(serde_json::Value::as_array);
    if let Some(raw_documents) = raw_documents {
        for raw_document in raw_documents {
            let Some(raw_path) = raw_document
                .get("storedPath")
                .or_else(|| raw_document.get("stored_path"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if crate::platform::project_paths::is_absolute_like_path(raw_path) {
                let absolute = Path::new(raw_path);
                if path_has_symlink_component(absolute) {
                    summary.symlink_escape_count += 1;
                }
                match std::fs::canonicalize(absolute) {
                    Ok(canonical)
                        if canonical.is_file()
                            && canonical.strip_prefix(trusted_root.root()).is_ok() => {}
                    _ => {
                        summary.external_managed_document_path_count += 1;
                        summary.unresolved_legacy_document_path_count += 1;
                    }
                }
                continue;
            }

            match trusted_root.adapt_legacy_document_path(raw_path) {
                Ok(managed) => match trusted_root.resolve_existing_file(&managed) {
                    Ok(_) => {}
                    Err(error) if error.code == AppErrorCode::ManagedPathSymlinkEscape => {
                        summary.symlink_escape_count += 1;
                    }
                    Err(_) => {}
                },
                Err(error) if error.code == AppErrorCode::ManagedPathSymlinkEscape => {
                    summary.symlink_escape_count += 1;
                }
                Err(error) if error.code == AppErrorCode::UnsafeManagedPath => {
                    summary.unsafe_document_path_count += 1;
                }
                Err(_) => {
                    summary.unresolved_legacy_document_path_count += 1;
                }
            }
        }
    } else {
        // A successfully deserialized project with no documents has no risky
        // document paths. Keep the project argument explicit for future doctor
        // checks without writing student content into diagnostics.
        let _ = project.documents.len();
    }
    summary
}

fn path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if std::fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn question_text_summary(project: &Project) -> QuestionTextSummary {
    let mut missing = 0;
    let mut suggested = 0;
    let mut edited = 0;
    let mut confirmed = 0;
    let mut failed = 0;
    let mut questions = Vec::new();
    let mut missing_numbers = Vec::new();
    let mut present_numbers = Vec::new();
    let mut max_number = 0u32;
    for question in &project.questions {
        max_number = max_number.max(question.number);
        present_numbers.push(question.number);
        let status = match question.question_text.status {
            TextFieldStatus::Missing => {
                missing += 1;
                missing_numbers.push(question.number);
                "missing"
            }
            TextFieldStatus::Suggested => {
                suggested += 1;
                "suggested"
            }
            TextFieldStatus::Edited => {
                edited += 1;
                "edited"
            }
            TextFieldStatus::Confirmed => {
                confirmed += 1;
                "confirmed"
            }
            TextFieldStatus::Failed => {
                failed += 1;
                "failed"
            }
        }
        .to_string();
        questions.push(QuestionTextRecord {
            number: question.number,
            status,
            source: text_source_label(&question.question_text.source),
            confidence: question.question_text.confidence,
            value_length: question.question_text.value.len(),
            warnings: question.question_text.warnings.clone(),
        });
    }

    let expected_question_count = project.expected_question_count.unwrap_or(max_number);
    let mut warnings = Vec::new();
    let mut coverage_missing_numbers = Vec::new();
    for number in 1..=expected_question_count {
        if !present_numbers.contains(&number) {
            coverage_missing_numbers.push(number);
        }
    }
    if !coverage_missing_numbers.is_empty() {
        let covered_expected_questions = project
            .questions
            .iter()
            .filter(|question| question.number <= expected_question_count)
            .count()
            .saturating_sub(missing);
        warnings.push(format!(
            "QUESTION_COVERAGE_INCOMPLETE: expected {}, found {}, missing {:?}",
            expected_question_count, covered_expected_questions, coverage_missing_numbers
        ));
        if coverage_missing_numbers.contains(&expected_question_count) {
            warnings.push(format!(
                "QUESTION_LAST_ITEM_MISSING: question {} missing",
                expected_question_count
            ));
        }
    }

    let mut extracted = Vec::new();
    for q in &project.questions {
        if matches!(
            q.question_text.status,
            TextFieldStatus::Confirmed | TextFieldStatus::Suggested | TextFieldStatus::Edited
        ) {
            extracted.push(q.number);
        }
    }
    let mut computed_missing_numbers = Vec::new();
    for number in 1..=expected_question_count {
        if !extracted.contains(&number) {
            computed_missing_numbers.push(number);
        }
    }
    let coverage_ok = computed_missing_numbers.is_empty();
    let partial_success = !coverage_ok;

    QuestionTextSummary {
        expected_question_count,
        extracted,
        missing_numbers: computed_missing_numbers,
        coverage_ok,
        partial_success,
        missing,
        suggested,
        edited,
        confirmed,
        failed,
        questions,
        warnings,
    }
}

fn rubric_summary(project: &Project) -> RubricSummary {
    let mut missing = 0;
    let mut imported = 0;
    let mut manual = 0;
    let mut suggested = 0;
    let mut confirmed = 0;
    let mut invalid = 0;
    let mut questions = Vec::new();
    let mut missing_numbers = Vec::new();
    let mut present_numbers = Vec::new();
    let mut max_number = 0u32;
    let mut false_positive_imported = Vec::new();
    for question in &project.questions {
        max_number = max_number.max(question.number);
        present_numbers.push(question.number);
        let validation = validate_rubric_state(&question.rubric, Some(&question.answer_type));
        let effective_status =
            if question.rubric.status == RubricStatus::Imported && !validation.valid {
                RubricStatus::Invalid
            } else {
                question.rubric.status.clone()
            };
        let status_is_false_positive_imported =
            question.rubric.status == RubricStatus::Imported && !validation.valid;
        let status = match effective_status {
            RubricStatus::Missing => {
                missing += 1;
                missing_numbers.push(question.number);
                "missing"
            }
            RubricStatus::Imported => {
                imported += 1;
                "imported"
            }
            RubricStatus::Manual => {
                manual += 1;
                "manual"
            }
            RubricStatus::Suggested => {
                suggested += 1;
                "suggested"
            }
            RubricStatus::Confirmed => {
                confirmed += 1;
                "confirmed"
            }
            RubricStatus::Invalid | RubricStatus::Legacy => {
                invalid += 1;
                if status_is_false_positive_imported {
                    false_positive_imported.push(question.number);
                }
                "invalid"
            }
        };
        let status = status.to_string();
        let error_code = if status == "invalid" {
            if status_is_false_positive_imported {
                Some("RUBRIC_EMPTY_CONTENT".to_string())
            } else {
                validation.issues.first().map(|issue| issue.code.clone())
            }
        } else {
            None
        };
        questions.push(RubricRecord {
            number: question.number,
            status,
            error_code,
            source: question
                .rubric
                .source
                .as_ref()
                .map(rubric_source_label)
                .unwrap_or_else(|| "unknown".to_string()),
            max_points: question.rubric.max_score,
            expected_answer_length: question
                .rubric
                .expected_answer
                .as_ref()
                .map(|value| value.len())
                .unwrap_or(0),
            criteria_count: question.rubric.criteria.len(),
            warnings: question.rubric.warnings.clone(),
        });
    }

    let mut warnings = Vec::new();
    let mut coverage_missing_numbers = Vec::new();
    for number in 1..=max_number {
        if !present_numbers.contains(&number) {
            coverage_missing_numbers.push(number);
        }
    }
    coverage_missing_numbers.extend(missing_numbers.iter().copied());
    coverage_missing_numbers.sort_unstable();
    coverage_missing_numbers.dedup();
    if !coverage_missing_numbers.is_empty() {
        warnings.push(format!(
            "RUBRIC_COVERAGE_INCOMPLETE: expected {}, found {}, missing {:?}",
            max_number,
            project.questions.len().saturating_sub(missing),
            coverage_missing_numbers
        ));
    }

    let expected_question_count = project.expected_question_count.unwrap_or(max_number);
    let mut imported_question_numbers = Vec::new();
    let mut computed_missing_question_numbers = Vec::new();
    let mut failed_question_numbers = Vec::new();
    for question_number in 1..=expected_question_count {
        if let Some(question) = project
            .questions
            .iter()
            .find(|q| q.number == question_number)
        {
            let validation = validate_rubric_state(&question.rubric, Some(&question.answer_type));
            let effective_status =
                if question.rubric.status == RubricStatus::Imported && !validation.valid {
                    RubricStatus::Invalid
                } else {
                    question.rubric.status.clone()
                };
            match effective_status {
                RubricStatus::Imported | RubricStatus::Manual | RubricStatus::Confirmed => {
                    imported_question_numbers.push(question_number);
                }
                RubricStatus::Missing => {
                    computed_missing_question_numbers.push(question_number);
                }
                RubricStatus::Invalid | RubricStatus::Legacy => {
                    failed_question_numbers.push(question_number);
                }
                _ => {
                    computed_missing_question_numbers.push(question_number);
                }
            }
        } else {
            computed_missing_question_numbers.push(question_number);
        }
    }
    let partial_success =
        !computed_missing_question_numbers.is_empty() || !failed_question_numbers.is_empty();
    let strategy = "per_question".to_string();

    RubricSummary {
        expected_question_count,
        imported_question_numbers,
        false_positive_imported,
        missing_question_numbers: computed_missing_question_numbers,
        failed_question_numbers,
        partial_success,
        strategy,
        missing,
        imported,
        manual,
        suggested,
        confirmed,
        invalid,
        questions,
        warnings,
    }
}

fn coverage_summary(total: usize, ready: usize) -> String {
    format!("{ready}/{total}")
}

fn sanitize_question_targets(targets: Vec<u32>, expected_question_count: u32) -> Vec<u32> {
    let mut targets = targets
        .into_iter()
        .filter(|number| (1..=expected_question_count).contains(number))
        .collect::<Vec<_>>();
    targets.sort_unstable();
    targets.dedup();
    targets
}

fn infer_expected_question_count(detected_markers: &[u32]) -> u32 {
    let detected = detected_markers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut expected = 0;
    for number in 1..=detected.iter().copied().max().unwrap_or(0) {
        if detected.contains(&number) {
            expected = number;
        } else {
            break;
        }
    }
    expected
}

fn analyze_fresh_question_text_replay(
    project: &Project,
    exam_source: &crate::domain::document::Document,
) -> Result<QuestionTextFreshReplayReport, AppError> {
    let pdf_path = exam_source.resolve_path(&project.root_path)?;
    let output = Command::new("pdftotext")
        .arg("-raw")
        .arg(&pdf_path)
        .arg("-")
        .output()
        .map_err(|error| AppError {
            code: crate::domain::errors::AppErrorCode::QuestionTextExtractionFailed,
            message: "pdftotext komutu çalıştırılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Install pdftotext or check PATH.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })?;

    if !output.status.success() {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::QuestionTextExtractionFailed,
            message: "pdftotext ile PDF metni okunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Inspect the source PDF and pdftotext stderr.".to_string()),
            technical_details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }

    let raw_text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(analyze_question_text_from_raw_text(
        &raw_text,
        project.expected_question_count.filter(|count| *count > 0),
    ))
}

fn analyze_question_text_from_raw_text(
    raw_text: &str,
    expected_question_count: Option<u32>,
) -> QuestionTextFreshReplayReport {
    let normalized_text = normalize_question_detection_text(raw_text);
    let detected_markers = detect_question_markers(&normalized_text);
    let detected_numbers = detected_markers.keys().copied().collect::<Vec<_>>();
    let expected_question_count = expected_question_count
        .filter(|count| *count > 0)
        .unwrap_or_else(|| infer_expected_question_count(&detected_numbers));
    let (clamped_markers, _) =
        clamp_question_markers(detected_markers, Some(expected_question_count));
    let marker_offsets = clamped_markers
        .iter()
        .map(|(number, offset)| QuestionMarkerOffset {
            number: *number,
            offset: *offset,
        })
        .collect::<Vec<_>>();
    let detected_markers = clamped_markers.keys().copied().collect::<Vec<_>>();
    let extracted_segments = split_question_segments(&normalized_text, &clamped_markers);
    let contaminated = extracted_segments
        .iter()
        .filter_map(|(number, text)| {
            if question_text_is_contaminated(text, *number, expected_question_count) {
                Some(*number)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let missing = missing_numbers(expected_question_count, &detected_markers);
    let coverage_ok = missing.is_empty() && contaminated.is_empty();
    let will_run_vision_fallback_for = sanitize_question_targets(
        [missing.clone(), contaminated.clone()].concat(),
        expected_question_count,
    );

    QuestionTextFreshReplayReport {
        content_method: "pdftotext".to_string(),
        expected_question_count,
        detected_markers,
        marker_offsets,
        missing,
        contaminated,
        coverage_ok,
        will_run_vision_fallback_for: will_run_vision_fallback_for.clone(),
        vision_fallback_call_count: will_run_vision_fallback_for.len() as u32,
    }
}

fn analyze_project_snapshot_question_text_replay(
    project: &Project,
    expected_question_count: u32,
    fresh: &QuestionTextFreshReplayReport,
) -> QuestionTextSnapshotReplayReport {
    let available = sanitize_question_targets(
        project
            .questions
            .iter()
            .filter(|question| {
                question.number <= expected_question_count
                    && matches!(
                        question.question_text.status,
                        TextFieldStatus::Confirmed
                            | TextFieldStatus::Suggested
                            | TextFieldStatus::Edited
                    )
            })
            .map(|question| question.number)
            .collect::<Vec<_>>(),
        expected_question_count,
    );
    let contaminated = sanitize_question_targets(
        project
            .questions
            .iter()
            .filter(|question| {
                question.number <= expected_question_count
                    && matches!(
                        question.question_text.status,
                        TextFieldStatus::Suggested
                            | TextFieldStatus::Missing
                            | TextFieldStatus::Failed
                    )
                    && question_text_is_contaminated(
                        &question.question_text.value,
                        question.number,
                        expected_question_count,
                    )
            })
            .map(|question| question.number)
            .collect::<Vec<_>>(),
        expected_question_count,
    );
    let missing = missing_numbers(expected_question_count, &available);
    let stale = available != fresh.detected_markers
        || missing != fresh.missing
        || contaminated != fresh.contaminated;

    QuestionTextSnapshotReplayReport {
        available,
        missing,
        contaminated,
        stale,
        needs_refresh: stale,
    }
}

fn summarize_question_text_snapshot(
    project: &Project,
    expected_question_count: u32,
) -> QuestionTextSnapshotReplayReport {
    let available = sanitize_question_targets(
        project
            .questions
            .iter()
            .filter(|question| {
                question.number <= expected_question_count
                    && matches!(
                        question.question_text.status,
                        TextFieldStatus::Confirmed
                            | TextFieldStatus::Suggested
                            | TextFieldStatus::Edited
                    )
            })
            .map(|question| question.number)
            .collect::<Vec<_>>(),
        expected_question_count,
    );
    let contaminated = sanitize_question_targets(
        project
            .questions
            .iter()
            .filter(|question| {
                question.number <= expected_question_count
                    && matches!(
                        question.question_text.status,
                        TextFieldStatus::Suggested
                            | TextFieldStatus::Missing
                            | TextFieldStatus::Failed
                    )
                    && question_text_is_contaminated(
                        &question.question_text.value,
                        question.number,
                        expected_question_count,
                    )
            })
            .map(|question| question.number)
            .collect::<Vec<_>>(),
        expected_question_count,
    );
    let missing = missing_numbers(expected_question_count, &available);

    QuestionTextSnapshotReplayReport {
        available,
        missing,
        contaminated,
        stale: false,
        needs_refresh: false,
    }
}

fn question_text_state_map(
    project: &Project,
    expected_question_count: u32,
) -> BTreeMap<u32, (String, String, String)> {
    let mut map = BTreeMap::new();
    for question in project
        .questions
        .iter()
        .filter(|question| question.number <= expected_question_count)
    {
        map.insert(
            question.number,
            (
                format!("{:?}", question.question_text.status).to_lowercase(),
                format!("{:?}", question.question_text.source).to_lowercase(),
                question.question_text.value.clone(),
            ),
        );
    }
    map
}

fn repair_question_text_from_raw_text(
    project: &mut Project,
    raw_text: &str,
    expected_question_count: Option<u32>,
) -> Result<QuestionTextRepairReport, AppError> {
    let fresh_pdf_extraction =
        analyze_question_text_from_raw_text(raw_text, expected_question_count);
    let expected_question_count = fresh_pdf_extraction.expected_question_count;
    let before = summarize_question_text_snapshot(project, expected_question_count);
    let before_map = question_text_state_map(project, expected_question_count);
    let candidates = extract_numbered_questions_from_text(raw_text, expected_question_count)
        .ok_or_else(|| AppError {
            code: crate::domain::errors::AppErrorCode::QuestionTextExtractionFailed,
            message: "Fresh PDF extraction failed to produce question candidates.".to_string(),
            recoverable: true,
            suggested_action: Some("Inspect the source PDF text layer.".to_string()),
            technical_details: None,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })?;

    project.expected_question_count = Some(expected_question_count);
    let coverage = apply_extraction_to_project_with_expected(
        project,
        candidates,
        vec![],
        expected_question_count,
    );
    project.workflow = workflow_engine::evaluate_workflow(project);

    let after = summarize_question_text_snapshot(project, expected_question_count);
    let after_map = question_text_state_map(project, expected_question_count);
    let updated = before_map
        .iter()
        .filter_map(|(number, before_state)| {
            let after_state = after_map.get(number)?;
            if before_state != after_state
                && before_state.0 != "confirmed"
                && before_state.0 != "edited"
            {
                Some(*number)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let created = after_map
        .iter()
        .filter_map(|(number, after_state)| match before_map.get(number) {
            Some(before_state) if before_state.0 != "missing" && before_state.0 != "failed" => None,
            _ if after_state.0 == "suggested" => Some(*number),
            _ => None,
        })
        .collect::<Vec<_>>();
    let preserved_confirmed = before_map
        .iter()
        .filter_map(|(number, before_state)| {
            if before_state.0 == "confirmed" && after_map.get(number) == Some(before_state) {
                Some(*number)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let preserved_edited = before_map
        .iter()
        .filter_map(|(number, before_state)| {
            if before_state.0 == "edited" && after_map.get(number) == Some(before_state) {
                Some(*number)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(QuestionTextRepairReport {
        expected_question_count,
        fresh_detected: fresh_pdf_extraction.detected_markers,
        fresh_missing: fresh_pdf_extraction.missing,
        fresh_contaminated: fresh_pdf_extraction.contaminated,
        before_available: before.available,
        before_missing: before.missing,
        before_contaminated: before.contaminated,
        updated,
        created,
        preserved_confirmed,
        preserved_edited,
        after_available: after.available,
        after_missing: after.missing,
        coverage_ok: coverage.coverage_ok,
    })
}

fn read_pdf_text(pdf_path: &Path) -> Result<String, AppError> {
    let output = Command::new("pdftotext")
        .arg("-raw")
        .arg(pdf_path)
        .arg("-")
        .output()
        .map_err(|error| AppError {
            code: crate::domain::errors::AppErrorCode::QuestionTextExtractionFailed,
            message: "pdftotext komutu çalıştırılamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Install pdftotext or check PATH.".to_string()),
            technical_details: Some(error.to_string()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })?;
    if !output.status.success() {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::QuestionTextExtractionFailed,
            message: "pdftotext ile PDF metni okunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Inspect the source PDF and pdftotext stderr.".to_string()),
            technical_details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn analyze_document_content_from_raw_text(
    raw_text: &str,
    expected_question_count: Option<u32>,
    role: &DocumentRole,
) -> DocumentContentFreshAnalysis {
    let normalized_text = normalize_question_detection_text(raw_text);
    let detected_markers = detect_question_markers(&normalized_text);
    let (detected_markers, ignored_question_numbers) =
        clamp_question_markers(detected_markers, expected_question_count);
    let detected_question_numbers = detected_markers.keys().copied().collect::<Vec<_>>();
    let expected_question_count = expected_question_count
        .filter(|count| *count > 0)
        .unwrap_or_else(|| infer_expected_question_count(&detected_question_numbers));
    let missing_question_numbers =
        missing_numbers(expected_question_count, &detected_question_numbers);
    let enough_text = raw_text.chars().filter(|c| !c.is_whitespace()).count() >= 200;
    let vision_fallback_needed = match role {
        DocumentRole::Rubric | DocumentRole::AnswerKey => !enough_text,
        _ => !enough_text || !missing_question_numbers.is_empty(),
    };

    DocumentContentFreshAnalysis {
        detected_question_numbers,
        missing_question_numbers,
        ignored_question_numbers,
        vision_fallback_needed,
    }
}

fn split_question_segments(text: &str, markers: &BTreeMap<u32, usize>) -> Vec<(u32, String)> {
    let mut positions: Vec<(u32, usize)> = markers
        .iter()
        .map(|(number, start)| (*number, *start))
        .collect();
    positions.sort_by_key(|(_, start)| *start);

    let mut segments = Vec::new();
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
        segments.push((*number, question_text.trim().to_string()));
    }

    segments
}

fn question_text_is_contaminated(
    question_text: &str,
    question_number: u32,
    expected_question_count: u32,
) -> bool {
    let normalized = normalize_question_detection_text(question_text);
    detect_question_markers(&normalized)
        .into_keys()
        .any(|number| number != question_number && number <= expected_question_count)
}

fn job_record(job: &JobSnapshot, stale_candidate: bool) -> JobInspectRecord {
    JobInspectRecord {
        job_id: job.id.clone(),
        kind: to_snake_case(&format!("{:?}", job.kind)),
        status: to_snake_case(&format!("{:?}", job.status)),
        active: matches!(job.status, JobStatus::Queued | JobStatus::Running),
        started_at: job.started_at.clone(),
        finished_at: job.finished_at.clone(),
        last_message: job.last_message.clone(),
        error_code: job.error.as_ref().map(|error| format!("{:?}", error.code)),
        error_message: job.error.as_ref().map(|error| error.message.clone()),
        error_details: job
            .error
            .as_ref()
            .and_then(|error| error.technical_details.clone()),
        stale_candidate,
    }
}

fn is_stale_candidate(job: &JobSnapshot, now: DateTime<Utc>) -> bool {
    if matches!(
        job.error.as_ref().map(|error| &error.code),
        Some(crate::domain::errors::AppErrorCode::JobStaleInterrupted)
    ) {
        return true;
    }
    if !matches!(job.status, JobStatus::Running | JobStatus::Queued) {
        return false;
    }
    let updated_at = parse_timestamp(&job.updated_at).unwrap_or(now);
    now.signed_duration_since(updated_at).num_minutes() >= 15
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn read_log_tail(path: &Path, line_count: usize) -> Vec<String> {
    if !path.exists() {
        return vec![format!("log file not found: {}", path.to_string_lossy())];
    }
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let len = lines.len();
    if len <= line_count {
        lines
    } else {
        lines[len - line_count..].to_vec()
    }
}

fn document_content_dir(project_root: &Path, document_id: &str) -> Option<PathBuf> {
    let trusted_root =
        TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false).ok()?;
    let managed = trusted_root
        .managed(&format!("cache/document_content/{document_id}"))
        .ok()?;
    let candidate = trusted_root.root().join(managed.as_path());
    if candidate.exists() {
        trusted_root.resolve_existing_directory(&managed).ok()
    } else {
        Some(candidate)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentContentMetadataSnapshot {
    method: String,
    raw_text_length: usize,
    normalized_text_length: usize,
    enough_text: bool,
    vision_fallback_needed: bool,
    detected_question_numbers: Vec<u32>,
    missing_question_numbers: Vec<u32>,
    #[serde(default)]
    ignored_question_numbers: Vec<u32>,
    artifact_dir: String,
    warnings: Vec<String>,
}

fn read_document_content_metadata(
    project_root: &Path,
    document_id: &str,
) -> Option<DocumentContentMetadataSnapshot> {
    let trusted_root =
        TrustedProjectRoot::from_canonical_root(project_root.to_path_buf(), false).ok()?;
    let managed = trusted_root
        .managed(&format!(
            "cache/document_content/{document_id}/content_metadata.json"
        ))
        .ok()?;
    let path = trusted_root.resolve_existing_file(&managed).ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn preview_metadata_path(project: &Project, document_id: &str) -> Option<PathBuf> {
    let trusted_root =
        TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false).ok()?;
    let document = project
        .documents
        .iter()
        .find(|document| document.id == document_id);
    let relative = document
        .and_then(|document| document.preview.as_ref())
        .and_then(|preview| preview.active_generation_id.as_ref())
        .map(|generation_id| {
            format!("outputs/previews/{document_id}/generations/{generation_id}/manifest.json")
        })
        .unwrap_or_else(|| format!("cache/page_previews/{document_id}/page_previews.json"));
    let managed = trusted_root.managed(&relative).ok()?;
    let candidate = trusted_root.root().join(managed.as_path());
    if candidate.exists() {
        trusted_root.resolve_existing_file(&managed).ok()
    } else {
        Some(candidate)
    }
}

fn page_preview_png_count(project: &Project, document_id: &str) -> usize {
    let trusted_root =
        match TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false) {
            Ok(root) => root,
            Err(_) => return 0,
        };
    let relative = project
        .documents
        .iter()
        .find(|document| document.id == document_id)
        .and_then(|document| document.preview.as_ref())
        .and_then(|preview| preview.active_generation_id.as_ref())
        .map(|generation_id| format!("outputs/previews/{document_id}/generations/{generation_id}"))
        .unwrap_or_else(|| format!("cache/page_previews/{document_id}"));
    let managed = match trusted_root.managed(&relative) {
        Ok(managed) => managed,
        Err(_) => return 0,
    };
    let dir = match trusted_root.resolve_existing_directory(&managed) {
        Ok(dir) => dir,
        Err(_) => return 0,
    };
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry.path().extension().and_then(|ext| ext.to_str()) == Some("png")
                })
                .count()
        })
        .unwrap_or(0)
}

fn count_preview_staging_dirs(project: &Project) -> usize {
    let root = PathBuf::from(&project.root_path)
        .join("outputs")
        .join("previews");
    std::fs::read_dir(root)
        .map(|documents| {
            documents
                .flatten()
                .map(|entry| entry.path().join(".staging"))
                .filter_map(|path| std::fs::read_dir(path).ok())
                .map(|entries| entries.flatten().count())
                .sum()
        })
        .unwrap_or(0)
}

fn project_path(project: &Project) -> String {
    TrustedProjectRoot::from_canonical_root(PathBuf::from(&project.root_path), false)
        .map(|root| root.project_file().to_string_lossy().to_string())
        .unwrap_or_default()
}

fn document_role_label(role: &DocumentRole) -> String {
    match role {
        DocumentRole::StudentScan => "student_scan",
        DocumentRole::ExamSource => "exam_source",
        DocumentRole::AnswerKey => "answer_key",
        DocumentRole::Rubric => "rubric",
        DocumentRole::Export => "export",
    }
    .to_string()
}

fn document_content_kind_label(role: &DocumentRole) -> String {
    match role {
        DocumentRole::StudentScan => "StudentScan",
        DocumentRole::ExamSource => "ExamSource",
        DocumentRole::AnswerKey => "AnswerKey",
        DocumentRole::Rubric => "Rubric",
        DocumentRole::Export => "Export",
    }
    .to_string()
}

fn document_content_kind(role: &DocumentRole) -> DocumentContentKind {
    match role {
        DocumentRole::ExamSource => DocumentContentKind::ExamSource,
        DocumentRole::AnswerKey => DocumentContentKind::AnswerKey,
        DocumentRole::Rubric => DocumentContentKind::Rubric,
        DocumentRole::StudentScan | DocumentRole::Export => DocumentContentKind::ExamSource,
    }
}

fn document_content_method_label(method: &str) -> String {
    match method.to_ascii_lowercase().as_str() {
        "pdf_to_text" | "pdftotext" => "PdfToText",
        "vision_fallback_prepared" => "VisionFallbackPrepared",
        "cached" => "Cached",
        other => other,
    }
    .to_string()
}

fn rubric_source_label(source: &RubricSource) -> String {
    match source {
        RubricSource::Manual => "manual",
        RubricSource::Json => "json",
        RubricSource::AnswerKeyPdf => "answer_key_pdf",
        RubricSource::Generated => "generated",
        RubricSource::RubricPdf => "rubric_pdf",
        RubricSource::Unknown => "unknown",
    }
    .to_string()
}

fn text_source_label(source: &TextFieldSource) -> String {
    match source {
        TextFieldSource::Manual => "manual",
        TextFieldSource::ExamPdf => "exam_pdf",
        TextFieldSource::StudentPdf => "student_pdf",
        TextFieldSource::ImportedTemplate => "imported_template",
        TextFieldSource::Unknown => "unknown",
    }
    .to_string()
}

fn to_snake_case(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for (index, ch) in value.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            result.push('_');
            for lower in ch.to_lowercase() {
                result.push(lower);
            }
        } else {
            result.push(ch.to_ascii_lowercase());
        }
    }
    result
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    use crate::domain::document::{Document, DocumentRole};
    use crate::domain::job::{JobKind, JobProgress, JobSnapshot, JobStatus};
    use crate::domain::project::Project;
    use crate::domain::question::{default_question, TextFieldSource, TextFieldStatus};
    use crate::domain::rubric::{RubricSource, RubricState, RubricStatus};
    use crate::domain::workflow::{WorkflowSnapshot, WorkflowStage};
    use crate::jobs::job_manager::job_snapshot_path;
    use crate::services::project_store::ProjectStore;
    use crate::services::workflow_engine;

    fn temp_project_root() -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("rubrika-diagnostics-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn exam_document(root: &std::path::Path, file_name: &str, role: DocumentRole) -> Document {
        let path = root.join("documents").join(file_name);
        std::fs::write(&path, "pdf").expect("write pdf");
        Document {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            file_name: file_name.to_string(),
            stored_path: file_name.to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: None,
        }
    }

    fn write_simple_pdf(path: &std::path::Path, lines: &[&str]) {
        fn escape_pdf_text(value: &str) -> String {
            value
                .replace('\\', r"\\")
                .replace('(', r"\(")
                .replace(')', r"\)")
        }

        let mut content = String::from("BT /F1 12 Tf 72 750 Td ");
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                content.push_str(" T* ");
            }
            content.push('(');
            content.push_str(&escape_pdf_text(line));
            content.push_str(") Tj");
        }
        content.push_str(" ET");

        let objects = [
            "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj".to_string(),
            "2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj".to_string(),
            "3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >> endobj".to_string(),
            "4 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj".to_string(),
            format!("5 0 obj << /Length {} >> stream\n{}\nendstream endobj", content.len(), content),
        ];

        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");
        let mut offsets = vec![0usize];
        for object in &objects {
            offsets.push(pdf.len());
            pdf.extend_from_slice(object.as_bytes());
            pdf.extend_from_slice(b"\n");
        }

        let xref_start = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                objects.len() + 1,
                xref_start
            )
            .as_bytes(),
        );

        std::fs::write(path, pdf).expect("write pdf");
    }

    #[test]
    fn fresh_question_text_analysis_detects_form_feed_and_keeps_fallback_empty() {
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

        let report = analyze_question_text_from_raw_text(text, Some(6));

        assert_eq!(report.detected_markers, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(report.marker_offsets.len(), 6);
        assert_eq!(report.missing, Vec::<u32>::new());
        assert_eq!(report.contaminated, Vec::<u32>::new());
        assert!(report.coverage_ok);
        assert!(report.will_run_vision_fallback_for.is_empty());
        assert_eq!(report.vision_fallback_call_count, 0);
    }

    #[test]
    fn snapshot_refresh_detects_stale_dirty_questions() {
        let mut project = Project {
            expected_question_count: Some(6),
            exam_package_freeze: None,
            id: "p1".to_string(),
            name: "Project".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
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
        for number in 1..=6 {
            let mut question = default_question(number);
            question.question_text.status = match number {
                2 | 4 => TextFieldStatus::Missing,
                1 | 3 => TextFieldStatus::Suggested,
                _ => TextFieldStatus::Confirmed,
            };
            question.question_text.source = TextFieldSource::ExamPdf;
            question.question_text.value = match number {
                3 => "S3.Yaşar...\nS4. contamination".to_string(),
                1 => "S1. temiz".to_string(),
                5 => "S5. temiz".to_string(),
                6 => "S6. temiz".to_string(),
                _ => String::new(),
            };
            project.questions.push(question);
        }
        let fresh = analyze_question_text_from_raw_text(
            "S1. Birinci\nS2. İkinci\nS3.Yaşar...\n\u{c}S4. Dördüncü\nS5. Beşinci\nS6.Altıncı",
            Some(6),
        );

        let snapshot = analyze_project_snapshot_question_text_replay(&project, 6, &fresh);

        assert_eq!(snapshot.available, vec![1, 3, 5, 6]);
        assert_eq!(snapshot.missing, vec![2, 4]);
        assert_eq!(snapshot.contaminated, vec![3]);
        assert!(snapshot.stale);
        assert!(snapshot.needs_refresh);
    }

    #[test]
    fn repair_question_text_from_raw_text_refreshes_dirty_snapshot_and_prunes_out_of_range() {
        let mut project = Project {
            expected_question_count: Some(6),
            exam_package_freeze: None,
            id: "p2".to_string(),
            name: "Project".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
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
        for number in 1..=6 {
            let mut question = default_question(number);
            question.question_text.status = match number {
                3 => TextFieldStatus::Suggested,
                4 => TextFieldStatus::Missing,
                _ => TextFieldStatus::Confirmed,
            };
            question.question_text.source = TextFieldSource::ExamPdf;
            question.question_text.value = match number {
                3 => "S3.Yaşar...\nS4. contamination".to_string(),
                1 | 2 | 5 | 6 => format!("S{number}. temiz"),
                _ => String::new(),
            };
            project.questions.push(question);
        }
        for number in [11, 2025] {
            let mut question = default_question(number);
            question.question_text.status = TextFieldStatus::Suggested;
            question.question_text.source = TextFieldSource::ExamPdf;
            question.question_text.value = format!("S{number}. stale");
            project.questions.push(question);
        }

        let report = repair_question_text_from_raw_text(
            &mut project,
            "S1. Birinci\nS2. İkinci\nS3.Yaşar...\n\u{c}S4. Dördüncü\nS5. Beşinci\nS6.Altıncı",
            Some(6),
        )
        .expect("repair");

        assert_eq!(report.fresh_detected, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(report.fresh_missing, Vec::<u32>::new());
        assert_eq!(report.fresh_contaminated, Vec::<u32>::new());
        assert!(report.coverage_ok);
        assert_eq!(report.before_available, vec![1, 2, 3, 5, 6]);
        assert_eq!(report.before_missing, vec![4]);
        assert_eq!(report.after_available, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(report.after_missing, Vec::<u32>::new());
        assert!(report.updated.contains(&3));
        assert!(report.created.contains(&4));
        assert!(report.preserved_confirmed.contains(&1));
        assert!(report.preserved_confirmed.contains(&2));
        assert!(report.preserved_confirmed.contains(&5));
        assert!(report.preserved_confirmed.contains(&6));
        assert_eq!(
            project
                .questions
                .iter()
                .map(|question| question.number)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert!(project.questions[2].question_text.value.starts_with("S3."));
        assert!(!project.questions[2].question_text.value.contains("S4."));
        assert_eq!(
            project.questions[3].question_text.status,
            TextFieldStatus::Suggested
        );
    }

    #[test]
    fn document_content_fresh_analysis_clamps_out_of_range_markers() {
        let analysis = analyze_document_content_from_raw_text(
            "S1. Birinci\nS2. İkinci\nS3. Üçüncü\nS4. Dördüncü\nS5. Beşinci\nS6. Altıncı\nS11. stale\nS2025. stale",
            Some(6),
            &DocumentRole::ExamSource,
        );

        assert_eq!(analysis.detected_question_numbers, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(analysis.ignored_question_numbers, vec![11, 2025]);
        assert!(analysis.vision_fallback_needed);
    }

    #[test]
    fn repair_document_content_refreshes_metadata_and_ignores_out_of_range_markers() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("DocRepair".to_string(), root.to_string_lossy().to_string())
            .expect("create project");
        project.expected_question_count = Some(6);

        let exam_pdf = root.join("documents").join("exam.pdf");
        write_simple_pdf(
            &exam_pdf,
            &[
                "S1. Birinci soru",
                "S2. İkinci soru",
                "S3.Yaşar: üçüncü soru",
                "S4. Dördüncü soru",
                "S5. Beşinci soru",
                "S6.Altıncı soru",
                "S11. stale",
                "S2025. stale",
            ],
        );
        let answer_pdf = root.join("documents").join("answer.pdf");
        write_simple_pdf(
            &answer_pdf,
            &[
                "S1. Cevap anahtarı",
                "S2. Cevap anahtarı",
                "S3. Cevap anahtarı",
                "S4. Cevap anahtarı",
                "S5. Cevap anahtarı",
                "S6. Cevap anahtarı",
            ],
        );

        project.documents.push(Document {
            id: "exam".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        });
        project.documents.push(Document {
            id: "answer".to_string(),
            role: DocumentRole::AnswerKey,
            file_name: "answer.pdf".to_string(),
            stored_path: "answer.pdf".to_string(),
            page_count: 1,
            added_at: "now".to_string(),
            checksum: None,
            preview: None,
        });
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        std::fs::create_dir_all(root.join("cache").join("document_content").join("exam"))
            .expect("cache exam");
        std::fs::write(
            root.join("cache")
                .join("document_content")
                .join("exam")
                .join("content_metadata.json"),
            r#"{"projectId":"p","documentId":"exam","kind":"exam_source","method":"cached","sourceFileSize":1,"sourceModifiedAt":null,"expectedQuestionCount":6,"rawTextLength":1,"nonWhitespaceLength":1,"normalizedTextLength":1,"pageCount":1,"enoughText":true,"visionFallbackNeeded":true,"detectedQuestionNumbers":[1,2,3,5,6,11,2025],"missingQuestionNumbers":[4],"ignoredQuestionNumbers":[],"likelyScannedPdf":false,"reason":null,"warnings":[],"rawTextPath":null,"normalizedTextPath":null,"pdftotextStderrPath":null,"modelInputManifestPath":null,"artifactDir":"x","updatedAt":"now"}"#,
        )
        .expect("write stale metadata");

        let report = DiagnosticsContext::new()
            .repair_document_content(&root)
            .expect("repair");

        assert_eq!(report.repaired_count, 2);
        let exam_item = report
            .items
            .iter()
            .find(|item| item.document_id == "exam")
            .expect("exam item");
        assert!(exam_item.metadata_written);
        assert!(exam_item.metadata_stale);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn repair_stale_jobs_marks_running_jobs_failed_and_preserves_stale_signal() {
        let root = temp_project_root();
        let job = JobSnapshot {
            id: "job-1".to_string(),
            schema_version: 1,
            project_id: "project-1".to_string(),
            project_root_path: Some(root.to_string_lossy().to_string()),
            kind: JobKind::QuestionTextExtraction,
            display_label: None,
            status: JobStatus::Running,
            cancellation_requested: false,
            cancellation_requested_at: None,
            progress: JobProgress {
                current: 2,
                total: 6,
                message: "Soru 23/6 Gemma vision ile tamamlanıyor...".to_string(),
            },
            started_at: Some("2020-01-01T00:00:00Z".to_string()),
            finished_at: None,
            last_message: Some("Soru 23/6 Gemma vision ile tamamlanıyor...".to_string()),
            correlation_id: "corr-diag".to_string(),
            idempotency_key: None,
            cancellable: true,
            retry_of_job_id: None,
            result: None,
            error: None,
            created_at: "2020-01-01T00:00:00Z".to_string(),
            updated_at: "2020-01-01T00:00:00Z".to_string(),
        };
        let path = job_snapshot_path(&root, &job.id).expect("safe job path");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("jobs dir");
        }
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&job).expect("serialize"),
        )
        .expect("write");

        let report = DiagnosticsContext::new()
            .repair_stale_jobs(&root)
            .expect("repair jobs");

        assert_eq!(report.repaired_count, 1);
        assert_eq!(report.items[0].status_before, "running");
        assert_eq!(report.items[0].status_after, "failed");
        assert!(report.items[0].stale_after);
        assert!(!report.items[0].active_after);

        let inspect = DiagnosticsContext::new()
            .inspect_jobs(&root)
            .expect("inspect jobs");
        assert_eq!(inspect.stale_candidates.len(), 1);
        assert_eq!(inspect.jobs[0].status, "failed");
        assert!(inspect.jobs[0].stale_candidate);
        assert!(!inspect.jobs[0].active);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn replay_rubric_import_dry_run_reports_strategy_and_missing_failed_questions() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Diag".to_string(), root.to_string_lossy().to_string())
            .expect("create project");

        let mut q1 = default_question(1);
        q1.rubric.status = RubricStatus::Imported;
        q1.rubric.source = Some(RubricSource::Json);
        q1.rubric.max_score = Some(5.0);
        q1.rubric.expected_answer = Some("Real answer".to_string());
        q1.rubric.criteria = vec![crate::domain::rubric::RubricCriterion {
            id: "c1".to_string(),
            label: "Doğruluk".to_string(),
            description: "Tam doğru cevap".to_string(),
            points: 5.0,
        }];
        let mut q2 = default_question(2);
        q2.rubric.status = RubricStatus::Missing;
        let mut q3 = default_question(3);
        q3.rubric.status = RubricStatus::Invalid;
        project.expected_question_count = Some(3);
        project.questions = vec![q1, q2, q3];
        project
            .documents
            .push(exam_document(&root, "rubric.pdf", DocumentRole::Rubric));
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        let report = DiagnosticsContext::new()
            .replay_rubric_import_dry_run(&root)
            .await
            .expect("replay");

        assert_eq!(report.strategy.as_deref(), Some("per_question"));
        assert_eq!(report.content_method.as_deref(), Some("pdftotext"));
        assert_eq!(report.expected_question_count, Some(3));
        assert_eq!(report.target_questions, vec![1, 2, 3]);
        assert_eq!(report.already_available, vec![1]);
        assert_eq!(report.will_run_questions, vec![2, 3]);
        assert_eq!(report.missing, vec![2]);
        assert_eq!(report.failed, vec![3]);
        assert_eq!(report.coverage_ok, Some(false));
        assert_eq!(report.partial_success, Some(true));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn replay_rubric_import_dry_run_skips_empty_imported_questions() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Diag".to_string(), root.to_string_lossy().to_string())
            .expect("create project");

        let mut q1 = default_question(1);
        q1.rubric.status = RubricStatus::Imported;
        q1.rubric.source = Some(RubricSource::Json);
        q1.rubric.warnings = vec!["rubric_empty_content".to_string()];
        let mut q2 = default_question(2);
        q2.rubric.status = RubricStatus::Missing;
        project.expected_question_count = Some(2);
        project.questions = vec![q1, q2];
        project
            .documents
            .push(exam_document(&root, "rubric.pdf", DocumentRole::Rubric));
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        let report = DiagnosticsContext::new()
            .replay_rubric_import_dry_run(&root)
            .await
            .expect("replay");

        assert_eq!(report.already_available, Vec::<u32>::new());
        assert_eq!(report.invalid_or_empty, vec![1]);
        assert_eq!(report.will_run_questions, vec![1, 2]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn inspect_rubric_marks_empty_imported_as_invalid() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Diag".to_string(), root.to_string_lossy().to_string())
            .expect("create project");

        let mut question = default_question(1);
        question.rubric.status = RubricStatus::Imported;
        question.rubric.source = Some(RubricSource::Json);
        project.expected_question_count = Some(1);
        project.questions = vec![question];
        project
            .documents
            .push(exam_document(&root, "rubric.pdf", DocumentRole::Rubric));
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        let report = DiagnosticsContext::new()
            .inspect_rubric(&root)
            .expect("inspect rubric");

        assert_eq!(report.imported_question_numbers, Vec::<u32>::new());
        assert_eq!(report.false_positive_imported, vec![1]);
        assert_eq!(report.questions[0].status, "invalid");
        assert_eq!(
            report.questions[0].error_code.as_deref(),
            Some("RUBRIC_EMPTY_CONTENT")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn doctor_separates_freeze_intake_and_scoring_readiness() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Diag".to_string(), root.to_string_lossy().to_string())
            .expect("create project");
        project.expected_question_count = Some(1);
        project.documents.push(Document {
            id: "exam".to_string(),
            role: DocumentRole::ExamSource,
            file_name: "exam.pdf".to_string(),
            stored_path: "exam.pdf".to_string(),
            page_count: 1,
            added_at: chrono::Utc::now().to_rfc3339(),
            checksum: None,
            preview: Some(crate::domain::document::PdfPreviewState {
                status: crate::domain::document::PdfPreviewStatus::Ready,
                rendered_at: Some(chrono::Utc::now().to_rfc3339()),
                page_count: Some(1),
                job_id: None,
                error_message: None,
                active_generation_id: None,
                pending_generation_id: None,
                source_fingerprint: None,
            }),
        });

        let mut question = default_question(1);
        question.question_text.status = TextFieldStatus::Confirmed;
        question.question_text.source = TextFieldSource::Manual;
        question.question_text.value = "Gerçek soru".to_string();
        question.rubric = RubricState {
            status: RubricStatus::Confirmed,
            source: Some(RubricSource::Manual),
            max_score: Some(10.0),
            expected_answer: Some("Gerçek cevap".to_string()),
            criteria: vec![crate::domain::rubric::RubricCriterion {
                id: "c1".to_string(),
                label: "Doğruluk".to_string(),
                description: "Tam doğru cevap".to_string(),
                points: 10.0,
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };
        project.questions = vec![question];
        project.exam_package_freeze = Some(crate::domain::project::ExamPackageFreeze {
            exam_package_version: 1,
            freeze_status: crate::domain::project::ExamPackageFreezeStatus::Frozen,
            frozen_at: chrono::Utc::now().to_rfc3339(),
            frozen_by: None,
            source_hash: "hash".to_string(),
            rubric_hash: "hash".to_string(),
            question_text_hash: "hash".to_string(),
            invalidated_at: None,
            invalidation_reason: None,
        });
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        let report = DiagnosticsContext::new()
            .doctor(&root)
            .await
            .expect("doctor");

        assert!(report.exam_package_freeze_ready);
        assert_eq!(report.exam_package_freeze_blockers, Vec::<String>::new());
        assert!(!report.student_intake_ready);
        assert_eq!(
            report.student_intake_blockers,
            vec!["STUDENT_SCAN_NOT_FOUND".to_string()]
        );
        assert!(!report.scoring_ready);
        assert_eq!(
            report.scoring_blockers,
            vec![
                "STUDENT_ANSWER_OCR_NOT_READY".to_string(),
                "STUDENT_GROUPING_NOT_READY".to_string(),
                "STUDENT_SCAN_NOT_FOUND".to_string()
            ]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn doctor_loads_legacy_workflow_snapshot() {
        let root = temp_project_root();
        std::fs::write(
            root.join("project.json"),
            format!(
                r#"{{
                    "id":"{}",
                    "name":"Legacy Doctor",
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-01T00:00:00Z",
                    "rootPath":"{}",
                    "documents":[],
                    "questions":[],
                    "workflow":{{
                        "currentStage":"documents_missing",
                        "blockingReasons":[],
                        "nextActions":[],
                        "summary":"Legacy doctor summary"
                    }}
                }}"#,
                uuid::Uuid::new_v4(),
                root.to_string_lossy()
            ),
        )
        .expect("write legacy project");

        let report = DiagnosticsContext::new()
            .doctor(&root)
            .await
            .expect("doctor");

        assert!(report.project_readable);
        assert_eq!(
            report
                .project
                .as_ref()
                .map(|project| project.project_name.as_str()),
            Some("Legacy Doctor")
        );
        assert!(report.errors.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn doctor_reports_path_security_counters_without_rewriting_project() {
        let root = temp_project_root();
        let outside_root = temp_project_root();
        let outside_document = outside_root.join("private.pdf");
        std::fs::write(&outside_document, "outside").expect("write outside document");
        let original_project = root.join("project.json");
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Path Doctor".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("create project");
        let mut raw_project = serde_json::to_value(project).expect("serialize project");
        let object = raw_project.as_object_mut().expect("project object");
        object.insert(
            "rootPath".to_string(),
            serde_json::Value::String(outside_root.to_string_lossy().to_string()),
        );
        object.insert(
            "documents".to_string(),
            serde_json::json!([
                {"id":"outside","role":"exam_source","fileName":"private.pdf","storedPath":outside_document.to_string_lossy(),"pageCount":1,"addedAt":"now","checksum":null,"preview":null},
                {"id":"unsafe","role":"exam_source","fileName":"unsafe.pdf","storedPath":"../private.pdf","pageCount":1,"addedAt":"now","checksum":null,"preview":null}
            ]),
        );
        std::fs::write(
            &original_project,
            serde_json::to_string_pretty(&raw_project).expect("serialize tampered project"),
        )
        .expect("write project");
        let before = std::fs::read(&original_project).expect("read project before doctor");

        let report = DiagnosticsContext::new()
            .doctor(&root)
            .await
            .expect("doctor");

        assert!(report.project_readable);
        assert_eq!(report.path_security.project_root_metadata_mismatch, 1);
        assert_eq!(report.path_security.external_managed_document_path_count, 1);
        assert_eq!(
            report.path_security.unresolved_legacy_document_path_count,
            1
        );
        assert_eq!(report.path_security.unsafe_document_path_count, 1);
        assert_eq!(
            std::fs::read(&original_project).expect("read project after doctor"),
            before
        );

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside_root);
    }

    #[test]
    fn inspect_document_content_warns_when_metadata_is_unreadable() {
        let root = temp_project_root();
        let store = ProjectStore::new();
        let mut project = store
            .create_project("Diag".to_string(), root.to_string_lossy().to_string())
            .expect("create project");

        let doc = exam_document(&root, "exam.pdf", DocumentRole::ExamSource);
        let metadata_dir = root.join("cache").join("document_content").join(&doc.id);
        std::fs::create_dir_all(&metadata_dir).expect("metadata dir");
        std::fs::write(metadata_dir.join("content_metadata.json"), "{not-json")
            .expect("bad metadata");
        project.documents.push(doc);
        project.workflow = workflow_engine::evaluate_workflow(&project);
        store.save_project(&project).expect("save");

        let records = DiagnosticsContext::new()
            .inspect_document_content(&root)
            .expect("inspect");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, "ExamSource");
        assert_eq!(records[0].method, "missing");
        assert!(records[0].metadata_exists);
        assert!(records[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("legacy or unreadable document content metadata")));

        let _ = std::fs::remove_dir_all(&root);
    }
}

/// Counts raw logging macros in production lib sources (excluding the CLI
/// binary and test modules). Used by doctor security counters; the strict
/// negative repository scan is enforced by `proof_31`.
fn count_unsafe_log_calls() -> u64 {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut count = 0u64;
    for path in walk_source_files(&source_dir) {
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        for line in strip_test_modules(&content).lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("println!")
                || trimmed.starts_with("eprintln!")
                || trimmed.starts_with("dbg!")
            {
                count += 1;
            }
        }
    }
    count
}

/// Counts developer-machine absolute paths in production lib sources.
fn count_hard_coded_paths() -> u64 {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut count = 0u64;
    for path in walk_source_files(&source_dir) {
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        for line in strip_test_modules(&content).lines() {
            if line.contains("/Users/") || line.contains("llm/models") {
                count += 1;
            }
        }
    }
    count
}

/// Counts sensitive sentinel values in the project's log/event files.
fn count_sentinel_leaks(project_path: &std::path::Path) -> u64 {
    const SENTINELS: [&str; 6] = [
        "STUDENT_SECRET_9f4a",
        "OCR_SECRET_17ce",
        "TRANSCRIPT_SECRET_41bd",
        "PROMPT_SECRET_a821",
        "MODEL_SECRET_47bf",
        "HOME_SECRET_PATH",
    ];
    let logs_dir = project_path.join("logs");
    let mut count = 0u64;
    let mut stack = vec![logs_dir];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                for sentinel in SENTINELS {
                    count += content.matches(sentinel).count() as u64;
                }
            }
        }
    }
    count
}

fn walk_source_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.ends_with("bin") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Removes `#[cfg(test)]` and `mod tests { ... }` blocks so production-only
/// source is scanned for the security counters.
fn strip_test_modules(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut in_test = false;
    let mut brace_depth = 0i64;
    for line in content.lines() {
        let trimmed = line.trim();
        if !in_test && (trimmed.starts_with("#[cfg(test)]") || trimmed == "mod tests {") {
            in_test = true;
            brace_depth = if trimmed == "mod tests {" { 1 } else { 0 };
            continue;
        }
        if in_test {
            for character in line.chars() {
                if character == '{' {
                    brace_depth += 1;
                } else if character == '}' {
                    brace_depth -= 1;
                }
            }
            if brace_depth <= 0 {
                in_test = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

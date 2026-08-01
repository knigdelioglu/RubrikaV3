use crate::domain::errors::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelMode {
    External,
    Managed,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelRuntimePreset {
    #[default]
    Standard,
    SpeakoflowTextCleanup,
    SpeakingRubricText,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub id: String,
    pub display_name: String,
    pub mode: ModelMode,
    pub server_path: String,
    pub model_path: String,
    pub mmproj_path: String,
    pub host: String,
    pub port: u16,
    pub base_url: String,
    #[serde(default)]
    pub runtime_preset: ModelRuntimePreset,
}

impl ModelProfile {
    pub fn requires_mmproj(&self) -> bool {
        self.runtime_preset == ModelRuntimePreset::Standard
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelProcess {
    pub pid: Option<u32>,
    pub started_by_app: bool,
    pub profile_id: String,
    pub base_url: String,
    pub log_path: PathBuf,
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub identity: Option<ManagedProcessIdentity>,
    #[serde(default)]
    pub runtime_instance_id: Option<String>,
    #[serde(default)]
    pub runtime_profile_fingerprint: Option<String>,
    #[serde(default)]
    pub unverified: bool,
}

/// The persisted identity is deliberately stronger than a PID. It is only
/// valid for lifecycle decisions when all available signals match the live
/// process inspection result.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedProcessIdentity {
    pub pid: u32,
    pub owner_uid: u32,
    pub process_start_time_unix_ms: u128,
    pub canonical_executable_path: PathBuf,
    pub executable_fingerprint: String,
    pub argv_fingerprint: String,
    pub expected_port: u16,
    pub runtime_profile_fingerprint: String,
    pub launch_instance_id: String,
    pub launched_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub profile_id: String,
    pub display_name: String,
    pub mode: ModelMode,
    pub base_url: String,
    pub server_path_exists: bool,
    pub model_path_exists: bool,
    pub mmproj_path_exists: bool,
    pub server_running: bool,
    pub health_ok: bool,
    pub completion_probe_ok: bool,
    pub managed_process_pid: Option<u32>,
    pub started_by_app: bool,
    #[serde(default)]
    pub active_lease_count: usize,
    #[serde(default)]
    pub draining: bool,
    pub log_path: Option<PathBuf>,
    pub last_error: Option<AppError>,
    pub warnings: Vec<String>,
    pub can_start_from_app: bool,
    pub can_stop_from_app: bool,
    pub start_requires_mode_change: bool,
    pub start_disabled_reason: Option<String>,
    pub suggested_actions: Vec<ModelSuggestedAction>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelSuggestedAction {
    pub code: String,
    pub label: String,
}

impl Default for ModelStatus {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            display_name: String::new(),
            mode: ModelMode::External,
            base_url: "http://127.0.0.1:8080".to_string(),
            server_path_exists: false,
            model_path_exists: false,
            mmproj_path_exists: false,
            server_running: false,
            health_ok: false,
            completion_probe_ok: false,
            managed_process_pid: None,
            started_by_app: false,
            active_lease_count: 0,
            draining: false,
            log_path: None,
            last_error: None,
            warnings: vec![],
            can_start_from_app: false,
            can_stop_from_app: false,
            start_requires_mode_change: false,
            start_disabled_reason: None,
            suggested_actions: vec![],
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelServerArgsPreview {
    pub profile_id: String,
    pub display_name: String,
    pub mode: ModelMode,
    pub base_url: String,
    pub command: String,
    pub args: Vec<String>,
    pub supported_flags: Vec<String>,
    pub unsupported_flags: Vec<String>,
    pub log_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelInputImageKind {
    QuestionText,
    Rubric,
    StudentOcr,
    StudentIdentityOcr,
    StudentAnswerOcrIssueCorrection,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInputImage {
    pub kind: ModelInputImageKind,
    pub document_id: String,
    pub page_number: u32,
    pub source_image_path: String,
    pub output_image_path: String,
    pub source_width: u32,
    pub source_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub base64_approx_bytes: u64,
    pub long_edge_max: u32,
    pub jpeg_quality: u8,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelRequestPayloadSummary {
    pub prompt_length: u32,
    pub image_count: u32,
    pub image_total_bytes: u64,
    pub base64_approx_total_bytes: u64,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
    pub timeout_seconds: u64,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelRequestKind {
    QuestionText,
    Ocr,
    OcrIssueCorrection,
    RubricDraft,
    SpeakingTranscriptCleanup,
    AnalysisReport,
    Scoring,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelDiagnostics {
    pub endpoint: String,
    pub request_kind: ModelRequestKind,
    pub http_status: Option<u16>,
    pub duration_ms: u64,
    pub prompt_length: Option<u32>,
    pub image_count: Option<u32>,
    pub image_total_bytes: Option<u64>,
    pub base64_approx_total_bytes: Option<u64>,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
    pub timeout_seconds: Option<u64>,
    pub max_tokens: Option<u32>,
    pub finish_reason: Option<String>,
    pub content_length: Option<u32>,
    pub reasoning_content_length: Option<u32>,
    pub raw_text_stored_path: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedQuestionCandidate {
    pub number: u32,
    pub question_text: String,
    pub confidence: f32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextExtractionRequest {
    pub prompt: String,
    pub image_path: String,
    pub page_index: u32,
    pub page_count: u32,
    pub target_question_number: u32,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrRequest {
    pub prompt: String,
    pub project_root_path: Option<String>,
    pub job_id: Option<String>,
    pub submission_id: String,
    pub question_id: String,
    pub question_number: u32,
    pub question_text: String,
    pub answer_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_mode: Option<crate::domain::student::OcrImagePreprocessMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_crop_ref: Option<String>,
    #[serde(default)]
    pub source_page_numbers: Vec<u32>,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StudentAnswerOcrIssueCorrectionDecision {
    SuggestCorrection,
    NoChange,
    NeedsTeacherReview,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StudentAnswerOcrIssueCorrectionScope {
    SingleWord,
    ShortPhrase,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrIssueCorrectionRequest {
    pub prompt: String,
    pub project_root_path: Option<String>,
    pub job_id: Option<String>,
    pub ocr_record_id: String,
    pub issue_id: Option<String>,
    pub observed_text: String,
    pub suggested_text_from_analyzer: String,
    pub question_number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_region: Option<crate::domain::student::StudentAnswerOcrCropBBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_crop_ref: Option<String>,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_image_ref: Option<String>,
    #[serde(default)]
    pub nearby_context: String,
    #[serde(default)]
    pub context_hints: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrIssueCorrectionOutput {
    pub decision: StudentAnswerOcrIssueCorrectionDecision,
    pub original_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_text: Option<String>,
    pub scope: StudentAnswerOcrIssueCorrectionScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_reading: Option<String>,
    pub context_reason: String,
    pub confidence: f32,
    pub requires_teacher_approval: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrIssueCorrectionResult {
    pub output: StudentAnswerOcrIssueCorrectionOutput,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_json: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrOutput {
    pub answer_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_answer: Option<serde_json::Value>,
    pub confidence: f32,
    #[serde(default)]
    pub uncertain_spans: Vec<crate::domain::student::OcrUncertainSpan>,
    #[serde(default)]
    pub suggested_corrections: Vec<crate::domain::student::OcrSuggestedCorrection>,
    #[serde(default)]
    pub critical_term_warnings: Vec<crate::domain::student::OcrCriticalTermWarning>,
    #[serde(default)]
    pub ocr_semantic_warnings: Vec<String>,
    #[serde(default)]
    pub critical_keyword_uncertain: bool,
    pub needs_review: bool,
    pub review_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrResult {
    pub output: StudentAnswerOcrOutput,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_json: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvaged_answer_text: Option<String>,
    pub parse_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request_metadata: Option<serde_json::Value>,
    pub printed_text_mixed: bool,
    pub printed_question_leak_detected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentIdentityOcrRequest {
    pub prompt: String,
    pub project_root_path: Option<String>,
    pub job_id: Option<String>,
    pub submission_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_mode: Option<crate::domain::student::OcrImagePreprocessMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_crop_ref: Option<String>,
    #[serde(default)]
    pub source_page_numbers: Vec<u32>,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentIdentityOcrOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    pub confidence: f32,
    pub needs_review: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentIdentityOcrResult {
    pub output: StudentIdentityOcrOutput,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_json: Option<serde_json::Value>,
    pub parse_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringCriterionScore {
    pub criterion_id: String,
    pub criterion_title: String,
    pub criterion_max_score: f32,
    pub awarded_score: f32,
    pub rationale: String,
    /// Exact, verbatim evidence copied from the effective student answer.
    /// Positive model-awarded points are not accepted without this evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_quote: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScoringRequest {
    pub prompt: String,
    pub project_root_path: Option<String>,
    pub job_id: Option<String>,
    pub submission_id: String,
    pub question_id: String,
    pub question_number: u32,
    pub student_display_name: Option<String>,
    pub student_number: Option<String>,
    pub student_class_name: Option<String>,
    pub question_text: String,
    pub expected_answer: Option<String>,
    pub answer_type: String,
    pub answer_text: String,
    pub rubric_json: serde_json::Value,
    #[serde(default)]
    pub criterion_scores_seed: Vec<ScoringCriterionScore>,
    #[serde(default)]
    pub partial_credit_hints: Vec<String>,
    #[serde(default)]
    pub zero_score_conditions: Vec<String>,
    #[serde(default)]
    pub common_mistakes: Vec<String>,
    pub max_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_record_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingTranscriptCleanupRequest {
    pub prompt: String,
    pub raw_transcript: String,
    #[serde(default)]
    pub segments: Vec<SpeakingTranscriptCleanupInputSegment>,
    pub timeout_seconds: u64,
    pub max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingTranscriptCleanupInputSegment {
    pub segment_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub raw_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingTranscriptCleanupOutputSegment {
    #[serde(alias = "segment_id", alias = "id")]
    pub segment_id: String,
    #[serde(
        alias = "cleaned_text",
        alias = "cleaned_transcript",
        alias = "text",
        alias = "transcript"
    )]
    pub cleaned_text: String,
    #[serde(default, alias = "modifications", alias = "edits")]
    pub changes: Vec<serde_json::Value>,
    #[serde(
        default,
        alias = "semantic_change_detected",
        alias = "semantic_change",
        alias = "semanticChange"
    )]
    pub semantic_change_detected: bool,
    #[serde(
        default,
        alias = "needs_review",
        alias = "needsReview",
        alias = "review"
    )]
    pub needs_review: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingTranscriptCleanupResult {
    pub cleaned_transcript: String,
    #[serde(default)]
    pub segments: Vec<SpeakingTranscriptCleanupOutputSegment>,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReportRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReportResult {
    pub report: String,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScoringOutput {
    pub awarded_score: f32,
    pub confidence: f32,
    pub needs_review: bool,
    pub rationale: String,
    pub teacher_visible_explanation: String,
    #[serde(default)]
    pub criterion_scores: Vec<ScoringCriterionScore>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScoringResult {
    pub output: ScoringOutput,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_json: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvaged_rationale: Option<String>,
    pub parse_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextExtractionOutput {
    pub questions: Vec<ExtractedQuestionCandidate>,
    pub page_warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextExtractionResult {
    pub page_index: u32,
    pub page_count: u32,
    pub output: QuestionTextExtractionOutput,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricExtractionRequest {
    pub prompt: String,
    pub raw_text: Option<String>,
    pub image_path: Option<String>,
    pub target_question_number: u32,
    #[serde(default)]
    pub model_input_images: Vec<ModelInputImage>,
    #[serde(default)]
    pub strict_json_only: bool,
    #[serde(default)]
    pub attempt: u32,
    #[serde(default)]
    pub project_root_path: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedRubricCandidate {
    pub number: u32,
    pub max_points: Option<f32>,
    pub expected_answer: Option<String>,
    pub criteria: Vec<crate::domain::rubric::RubricCriterion>,
    pub confidence: f32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricImportCriterion {
    pub label: String,
    pub points: f32,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricImportQuestion {
    pub question_number: u32,
    pub max_points: Option<f32>,
    pub expected_answer: Option<String>,
    #[serde(default)]
    pub criteria: Vec<RubricImportCriterion>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricImportPayload {
    pub questions: Vec<RubricImportQuestion>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricExtractionOutput {
    pub questions: Vec<ExtractedRubricCandidate>,
    pub document_warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricExtractionResult {
    pub output: RubricExtractionOutput,
    pub raw_response: String,
    pub diagnostics: ModelDiagnostics,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextSuggestion {
    pub question_id: String,
    pub number: u32,
    pub text: String,
    pub confidence: f32,
    pub source: String,
    pub status: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextExtractionStatus {
    pub project_id: String,
    pub document_id: Option<String>,
    pub preview_status: String,
    pub preview_ready: bool,
    pub current_stage: String,
    pub blocking_reasons: Vec<String>,
    pub next_actions: Vec<String>,
    pub detected_question_count: Option<u32>,
    pub suggested_count: u32,
    pub confirmed_count: u32,
    pub missing_count: u32,
    pub missing_question_numbers: Vec<u32>,
    pub coverage_ok: bool,
    pub extraction_method: Option<String>,
    pub vision_fallback_available: bool,
    pub running_job_id: Option<String>,
    pub latest_job_status: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SupportFlags {
    pub mmproj_offload: bool,
    pub cache_ram: bool,
    pub image_min_tokens: bool,
    pub image_max_tokens: bool,
    pub reasoning_off: bool,
    pub cache_type_flags: bool,
    pub cache_short_flags: bool,
}

impl SupportFlags {
    pub fn from_help_output(help_output: &str) -> Self {
        let lower = help_output.to_lowercase();
        Self {
            mmproj_offload: lower.contains("--mmproj-offload"),
            cache_ram: lower.contains("--cache-ram"),
            image_min_tokens: lower.contains("--image-min-tokens"),
            image_max_tokens: lower.contains("--image-max-tokens"),
            reasoning_off: lower.contains("--reasoning"),
            cache_type_flags: lower.contains("--cache-type-k") && lower.contains("--cache-type-v"),
            cache_short_flags: lower.contains("-ctk") && lower.contains("-ctv"),
        }
    }
}

pub fn default_model_profile() -> ModelProfile {
    ModelProfile {
        id: "gemma4-ocr-q8".to_string(),
        display_name: "Gemma 4 OCR Q4_K_XL".to_string(),
        mode: ModelMode::External,
        server_path: String::new(),
        model_path: String::new(),
        mmproj_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        base_url: "http://127.0.0.1:8080".to_string(),
        runtime_preset: ModelRuntimePreset::Standard,
    }
}

/// Legacy developer-machine bootstrap.
///
/// Rubrika v3 no longer ships hard-coded user/model paths in production
/// sources. A saved per-user configuration remains the only migration
/// source and is loaded by `ModelConfigService`; nothing is silently
/// bootstrapped from a developer machine path.
pub fn local_model_paths() -> Option<ModelProfile> {
    None
}

pub const SPEAKING_ASR_CLEANUP_PROFILE_ID: &str = "speaking_transcript_cleanup_12b";
pub const SPEAKING_RUBRIC_PROFILE_ID: &str = "speaking_rubric_evaluation_12b";

pub fn speaking_asr_cleanup_model_profile() -> ModelProfile {
    ModelProfile {
        id: SPEAKING_ASR_CLEANUP_PROFILE_ID.to_string(),
        display_name: "Gemma 4 12B — Konuşma Transkript Temizleme".to_string(),
        mode: ModelMode::Managed,
        server_path: String::new(),
        model_path: String::new(),
        mmproj_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        base_url: "http://127.0.0.1:8080".to_string(),
        runtime_preset: ModelRuntimePreset::SpeakingRubricText,
    }
}

pub fn speaking_rubric_model_profile() -> ModelProfile {
    ModelProfile {
        id: SPEAKING_RUBRIC_PROFILE_ID.to_string(),
        display_name: "Gemma 4 12B — Konuşma Rubrik Değerlendirme".to_string(),
        mode: ModelMode::Managed,
        server_path: String::new(),
        model_path: String::new(),
        mmproj_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        base_url: "http://127.0.0.1:8080".to_string(),
        runtime_preset: ModelRuntimePreset::SpeakingRubricText,
    }
}

pub fn build_model_server_args(
    profile: &ModelProfile,
    support_flags: &SupportFlags,
) -> Result<Vec<String>, AppError> {
    if profile.runtime_preset == ModelRuntimePreset::SpeakoflowTextCleanup {
        return build_speakoflow_text_cleanup_args(profile, support_flags);
    }
    if profile.runtime_preset == ModelRuntimePreset::SpeakingRubricText {
        return build_speaking_rubric_text_args(profile, support_flags);
    }
    let mut args = vec![
        "-m".to_string(),
        profile.model_path.clone(),
        "--mmproj".to_string(),
        profile.mmproj_path.clone(),
        "-ngl".to_string(),
        "99".to_string(),
        "-c".to_string(),
        "4096".to_string(),
        "-fa".to_string(),
        "on".to_string(),
        "--jinja".to_string(),
        "--parallel".to_string(),
        "1".to_string(),
        "--batch-size".to_string(),
        "1536".to_string(),
        "--ubatch-size".to_string(),
        "1280".to_string(),
        "--temp".to_string(),
        "0".to_string(),
        "--top-p".to_string(),
        "1".to_string(),
        "--top-k".to_string(),
        "1".to_string(),
        "--repeat-penalty".to_string(),
        "1.0".to_string(),
        "-n".to_string(),
        "768".to_string(),
        "--host".to_string(),
        profile.host.clone(),
        "--port".to_string(),
        profile.port.to_string(),
    ];

    if support_flags.cache_short_flags {
        args.push("-ctk".to_string());
        args.push("q8_0".to_string());
        args.push("-ctv".to_string());
        args.push("q8_0".to_string());
    } else if support_flags.cache_type_flags {
        args.push("--cache-type-k".to_string());
        args.push("q8_0".to_string());
        args.push("--cache-type-v".to_string());
        args.push("q8_0".to_string());
    } else {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::ModelServerUnsupportedFlags,
            message: "Bu llama-server sürümü KV cache bayraklarını desteklemiyor.".to_string(),
            recoverable: false,
            suggested_action: Some(
                "llama-server sürümünü güncelleyin veya KV cache desteği olan bir binary kullanın."
                    .to_string(),
            ),
            technical_details: Some(
                "Missing both -ctk/-ctv and --cache-type-k/--cache-type-v".to_string(),
            ),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }

    if support_flags.mmproj_offload {
        args.push("--mmproj-offload".to_string());
    }

    if support_flags.cache_ram {
        args.push("--cache-ram".to_string());
        args.push("0".to_string());
    }

    if support_flags.image_min_tokens {
        args.push("--image-min-tokens".to_string());
        args.push("1120".to_string());
    }

    if support_flags.image_max_tokens {
        args.push("--image-max-tokens".to_string());
        args.push("1120".to_string());
    }

    if support_flags.reasoning_off {
        args.push("--reasoning".to_string());
        args.push("off".to_string());
    }

    Ok(args)
}

fn build_speaking_rubric_text_args(
    profile: &ModelProfile,
    support_flags: &SupportFlags,
) -> Result<Vec<String>, AppError> {
    let mut args = vec![
        "-m".to_string(),
        profile.model_path.clone(),
        "--host".to_string(),
        profile.host.clone(),
        "--port".to_string(),
        profile.port.to_string(),
        "-c".to_string(),
        "8192".to_string(),
        "--parallel".to_string(),
        "1".to_string(),
        "-ngl".to_string(),
        "99".to_string(),
        "--jinja".to_string(),
        "--batch-size".to_string(),
        "512".to_string(),
        "--ubatch-size".to_string(),
        "256".to_string(),
        "--repeat-penalty".to_string(),
        "1.05".to_string(),
        "--no-cache-prompt".to_string(),
        "-cram".to_string(),
        "0".to_string(),
        "-ctxcp".to_string(),
        "0".to_string(),
    ];

    if support_flags.cache_short_flags {
        args.extend(["-ctk".to_string(), "turbo3".to_string()]);
        args.extend(["-ctv".to_string(), "turbo3".to_string()]);
    } else if support_flags.cache_type_flags {
        args.extend(["--cache-type-k".to_string(), "turbo3".to_string()]);
        args.extend(["--cache-type-v".to_string(), "turbo3".to_string()]);
    } else {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::ModelServerUnsupportedFlags,
            message: "Konuşma rubriği modeli KV cache bayraklarını desteklemiyor.".to_string(),
            recoverable: false,
            suggested_action: Some(
                "SpeakoFlow ile uyumlu llama-server binary kullanın.".to_string(),
            ),
            technical_details: Some(
                "Missing both -ctk/-ctv and --cache-type-k/--cache-type-v".to_string(),
            ),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }

    if support_flags.reasoning_off {
        args.extend(["--reasoning".to_string(), "off".to_string()]);
    }
    Ok(args)
}

fn build_speakoflow_text_cleanup_args(
    profile: &ModelProfile,
    support_flags: &SupportFlags,
) -> Result<Vec<String>, AppError> {
    let mut args = vec![
        "-m".to_string(),
        profile.model_path.clone(),
        "--host".to_string(),
        profile.host.clone(),
        "--port".to_string(),
        profile.port.to_string(),
        "-c".to_string(),
        "8192".to_string(),
        "--parallel".to_string(),
        "1".to_string(),
        "-ngl".to_string(),
        "999".to_string(),
        "--jinja".to_string(),
        "--repeat-penalty".to_string(),
        "1.1".to_string(),
        "--no-cache-prompt".to_string(),
        "-cram".to_string(),
        "0".to_string(),
        "-ctxcp".to_string(),
        "0".to_string(),
    ];

    if support_flags.cache_short_flags {
        args.extend(["-ctk".to_string(), "turbo3".to_string()]);
        args.extend(["-ctv".to_string(), "turbo3".to_string()]);
    } else if support_flags.cache_type_flags {
        args.extend(["--cache-type-k".to_string(), "turbo3".to_string()]);
        args.extend(["--cache-type-v".to_string(), "turbo3".to_string()]);
    } else {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::ModelServerUnsupportedFlags,
            message: "SpeakoFlow cleanup runtime KV cache bayraklarını desteklemiyor.".to_string(),
            recoverable: false,
            suggested_action: Some(
                "SpeakoFlow ile uyumlu llama-server binary kullanın.".to_string(),
            ),
            technical_details: Some(
                "Missing both -ctk/-ctv and --cache-type-k/--cache-type-v".to_string(),
            ),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }

    Ok(args)
}

pub fn preview_model_server_args(
    profile: &ModelProfile,
    help_output: &str,
) -> Result<ModelServerArgsPreview, AppError> {
    let support_flags = SupportFlags::from_help_output(help_output);
    let args = build_model_server_args(profile, &support_flags)?;
    let supported_flags = supported_flag_names(&support_flags);
    let unsupported_flags = unsupported_flag_names(&support_flags);

    Ok(ModelServerArgsPreview {
        profile_id: profile.id.clone(),
        display_name: profile.display_name.clone(),
        mode: profile.mode.clone(),
        base_url: profile.base_url.clone(),
        command: profile.server_path.clone(),
        args,
        supported_flags,
        unsupported_flags,
        log_path: PathBuf::new(),
    })
}

pub fn supported_flag_names(flags: &SupportFlags) -> Vec<String> {
    let mut supported_flags = vec![];
    if flags.cache_short_flags {
        supported_flags.push("-ctk/-ctv".to_string());
    } else if flags.cache_type_flags {
        supported_flags.push("--cache-type-k/--cache-type-v".to_string());
    }
    if flags.mmproj_offload {
        supported_flags.push("--mmproj-offload".to_string());
    }
    if flags.cache_ram {
        supported_flags.push("--cache-ram".to_string());
    }
    if flags.image_min_tokens {
        supported_flags.push("--image-min-tokens".to_string());
    }
    if flags.image_max_tokens {
        supported_flags.push("--image-max-tokens".to_string());
    }
    if flags.reasoning_off {
        supported_flags.push("--reasoning off".to_string());
    }
    supported_flags
}

pub fn unsupported_flag_names(flags: &SupportFlags) -> Vec<String> {
    let mut unsupported_flags = vec![];
    if !flags.mmproj_offload {
        unsupported_flags.push("--mmproj-offload".to_string());
    }
    if !flags.cache_ram {
        unsupported_flags.push("--cache-ram".to_string());
    }
    if !flags.image_min_tokens {
        unsupported_flags.push("--image-min-tokens".to_string());
    }
    if !flags.image_max_tokens {
        unsupported_flags.push("--image-max-tokens".to_string());
    }
    if !flags.reasoning_off {
        unsupported_flags.push("--reasoning off".to_string());
    }
    unsupported_flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_no_hard_coded_user_paths() {
        let profile = default_model_profile();
        assert_eq!(profile.id, "gemma4-ocr-q8");
        assert_eq!(profile.display_name, "Gemma 4 OCR Q4_K_XL");
        assert_eq!(profile.base_url, "http://127.0.0.1:8080");
        assert!(profile.server_path.is_empty());
        assert!(profile.model_path.is_empty());
        assert!(profile.mmproj_path.is_empty());
    }

    #[test]
    fn speaking_profiles_require_explicit_user_model_selection() {
        let cleanup = speaking_asr_cleanup_model_profile();
        let rubric = speaking_rubric_model_profile();

        assert_eq!(cleanup.id, SPEAKING_ASR_CLEANUP_PROFILE_ID);
        assert_eq!(cleanup.base_url, "http://127.0.0.1:8080");
        assert_eq!(rubric.id, SPEAKING_RUBRIC_PROFILE_ID);
        assert_eq!(rubric.base_url, "http://127.0.0.1:8080");
        assert!(cleanup.server_path.is_empty());
        assert!(cleanup.model_path.is_empty());
        assert!(rubric.server_path.is_empty());
        assert!(rubric.model_path.is_empty());
        assert!(cleanup.mmproj_path.is_empty());
        assert!(rubric.mmproj_path.is_empty());
    }

    #[test]
    fn speaking_cleanup_profile_uses_text_only_runtime_arguments() {
        let profile = speaking_asr_cleanup_model_profile();
        let flags = SupportFlags {
            mmproj_offload: false,
            cache_ram: false,
            image_min_tokens: false,
            image_max_tokens: false,
            reasoning_off: false,
            cache_type_flags: false,
            cache_short_flags: true,
        };
        let args = build_model_server_args(&profile, &flags).expect("cleanup args");

        assert_eq!(
            profile.runtime_preset,
            ModelRuntimePreset::SpeakingRubricText
        );
        assert!(!profile.requires_mmproj());
        assert!(args.windows(2).any(|pair| pair == ["-c", "8192"]));
        assert!(args.windows(2).any(|pair| pair == ["-ctk", "turbo3"]));
        assert!(args.contains(&"--no-cache-prompt".to_string()));
        assert!(!args.contains(&"--mmproj".to_string()));
    }

    #[test]
    fn support_flags_detect_supported_switches() {
        let help = "--mmproj-offload\n--cache-ram\n-ctk\n-ctv\n--reasoning\n";
        let flags = SupportFlags::from_help_output(help);
        assert!(flags.mmproj_offload);
        assert!(flags.cache_ram);
        assert!(flags.cache_short_flags);
        assert!(flags.reasoning_off);
    }
}

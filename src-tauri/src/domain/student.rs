use serde::{de, Deserialize, Deserializer, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StudentSubmissionStatus {
    Grouped,
    IdentityMissing,
    ReadyForOcr,
    OcrRunning,
    OcrSuggested,
    OcrConfirmed,
    Failed,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OcrImagePreprocessMode {
    #[default]
    HandwritingEnhanced,
    Original,
    CleanGrayscale,
    HighContrast,
    #[serde(alias = "high_contrast_bw_optional")]
    HighContrastBw,
}

impl<'de> Deserialize<'de> for OcrImagePreprocessMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ModeVisitor;

        impl<'de> de::Visitor<'de> for ModeVisitor {
            type Value = OcrImagePreprocessMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a supported OCR preprocess mode string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(match value {
                    "original" => OcrImagePreprocessMode::Original,
                    "clean_grayscale" => OcrImagePreprocessMode::CleanGrayscale,
                    "handwriting_enhanced" => OcrImagePreprocessMode::HandwritingEnhanced,
                    "high_contrast" => OcrImagePreprocessMode::HighContrast,
                    "high_contrast_bw" | "high_contrast_bw_optional" => {
                        OcrImagePreprocessMode::HighContrastBw
                    }
                    _ => OcrImagePreprocessMode::CleanGrayscale,
                })
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(ModeVisitor)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrImagePreprocessDiagnostics {
    pub mode: OcrImagePreprocessMode,
    #[serde(default)]
    pub preprocess_version: String,
    pub source_image_path: String,
    pub output_image_path: String,
    pub source_width: u32,
    pub source_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub cache_hit: bool,
    pub applied: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technical_details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StudentAnswerSlotStatus {
    Empty,
    PendingOcr,
    OcrSuggested,
    Confirmed,
    Edited,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StudentAnswerOcrStatus {
    Pending,
    Running,
    Succeeded,
    Partial,
    Failed,
    ReviewNeeded,
    ParseFailed,
    CropMissing,
    PartialAnswerSuspected,
    PrintedTextLeakSuspected,
    ModelError,
    TeacherCorrected,
    TeacherApproved,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OcrGenerationStatus {
    Candidate,
    ReadyForReview,
    Active,
    Rejected,
    Failed,
    Stale,
    Interrupted,
    Superseded,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OcrTeacherReviewStatus {
    NotRequired,
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PageGroupingMode {
    OnePdfOneStudent,
    FixedPagesPerStudent,
    Manual,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClassMembershipSource {
    InheritedFromBatch,
    #[serde(other)]
    TeacherOverride,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_ocr: Option<StudentIdentityOcrRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerSlot {
    pub question_id: String,
    pub question_number: u32,
    pub status: StudentAnswerSlotStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentSubmission {
    pub id: String,
    pub student_id: String,
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_batch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_membership_source: Option<ClassMembershipSource>,
    #[serde(default)]
    pub page_numbers: Vec<u32>,
    pub status: StudentSubmissionStatus,
    #[serde(default)]
    pub answer_slots: Vec<StudentAnswerSlot>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// A versioned OCR result set. Each submission gets its own generation so a
/// partial or stale rerun can never replace another submission's active data.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OcrGeneration {
    pub generation_id: String,
    pub submission_id: String,
    pub source_fingerprint: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    pub prompt_version: String,
    pub status: OcrGenerationStatus,
    #[serde(default)]
    pub result: Vec<StudentAnswerOcrRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<serde_json::Value>,
    pub teacher_review_status: OcrTeacherReviewStatus,
    pub created_by_job_id: String,
    pub source_document_id: String,
    #[serde(default)]
    pub source_storage_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrCropBBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub page_index: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerCropTemplateItem {
    pub question_id: String,
    pub question_number: u32,
    pub page_index_within_submission: u32,
    pub bbox: StudentAnswerOcrCropBBox,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerCropTemplate {
    #[serde(default)]
    pub items: Vec<StudentAnswerCropTemplateItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StudentIdentityCropTemplate {
    pub page_index_within_submission: u32,
    pub bbox: StudentAnswerOcrCropBBox,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentIdentityOcrRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    pub confidence: f32,
    pub needs_review: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub raw_model_output: String,
    #[serde(default)]
    pub crop_refs: Vec<String>,
    #[serde(default)]
    pub original_crop_refs: Vec<String>,
    #[serde(default)]
    pub preprocessed_crop_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_crop_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_mode: Option<OcrImagePreprocessMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_version: Option<String>,
    #[serde(default)]
    pub preprocess_applied: bool,
    #[serde(default)]
    pub preprocess_warnings: Vec<String>,
    #[serde(default)]
    pub preprocess_diagnostics: Vec<OcrImagePreprocessDiagnostics>,
    #[serde(default)]
    pub available_preprocess_variants: Vec<OcrImagePreprocessMode>,
    #[serde(default)]
    pub source_page_numbers: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request_metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrParseDiagnostics {
    pub raw_model_output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_json: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvaged_answer_text: Option<String>,
    pub parse_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrRenderDiagnostics {
    #[serde(default)]
    pub crop_refs: Vec<String>,
    #[serde(default)]
    pub full_page_preview_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_bbox: Option<StudentAnswerOcrCropBBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_page_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_region_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_region_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_region_end: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_question_anchor: Option<String>,
    pub crop_was_clamped: bool,
    pub crop_margin_applied: bool,
    pub rendered_crop_exists: bool,
    pub rendered_page_preview_exists: bool,
    pub crop_missing: bool,
    pub page_preview_missing: bool,
    pub partial_answer_suspected: bool,
    pub printed_text_mixed: bool,
    pub printed_question_leak_detected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrUncertainSpan {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_region: Option<StudentAnswerOcrCropBBox>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrSuggestedCorrection {
    pub original_text: String,
    pub suggested_text: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_region: Option<StudentAnswerOcrCropBBox>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrCriticalTermWarning {
    pub observed_text: String,
    pub expected_or_related_term: String,
    pub reason: String,
    pub warning_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_region: Option<StudentAnswerOcrCropBBox>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrRecord {
    pub id: String,
    pub submission_id: String,
    pub question_id: String,
    pub question_number: u32,
    #[serde(default)]
    pub source_page_numbers: Vec<u32>,
    #[serde(default)]
    pub source_image_refs: Vec<String>,
    #[serde(default)]
    pub crop_refs: Vec<String>,
    #[serde(default)]
    pub original_crop_refs: Vec<String>,
    #[serde(default)]
    pub preprocessed_crop_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_crop_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_mode: Option<OcrImagePreprocessMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_version: Option<String>,
    #[serde(default)]
    pub preprocess_applied: bool,
    #[serde(default)]
    pub preprocess_warnings: Vec<String>,
    #[serde(default)]
    pub preprocess_diagnostics: Vec<OcrImagePreprocessDiagnostics>,
    #[serde(default)]
    pub available_preprocess_variants: Vec<OcrImagePreprocessMode>,
    #[serde(default)]
    pub full_page_preview_refs: Vec<String>,
    pub answer_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_answer: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub uncertain_spans: Vec<OcrUncertainSpan>,
    #[serde(default)]
    pub suggested_corrections: Vec<OcrSuggestedCorrection>,
    #[serde(default)]
    pub critical_term_warnings: Vec<OcrCriticalTermWarning>,
    #[serde(default)]
    pub ocr_semantic_warnings: Vec<String>,
    #[serde(default)]
    pub critical_keyword_uncertain: bool,
    pub status: StudentAnswerOcrStatus,
    pub needs_review: bool,
    #[serde(default)]
    pub review_reasons: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    pub prompt_version: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_corrected_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_diagnostics: Option<StudentAnswerOcrParseDiagnostics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_diagnostics: Option<StudentAnswerOcrRenderDiagnostics>,
}

impl Default for StudentIdentityOcrRecord {
    fn default() -> Self {
        Self {
            display_name: None,
            number: None,
            class_name: None,
            confidence: 0.0,
            needs_review: false,
            warnings: vec![],
            raw_model_output: String::new(),
            crop_refs: vec![],
            original_crop_refs: vec![],
            preprocessed_crop_refs: vec![],
            model_input_crop_ref: None,
            preprocess_mode: None,
            preprocess_version: None,
            preprocess_applied: false,
            preprocess_warnings: vec![],
            preprocess_diagnostics: vec![],
            available_preprocess_variants: vec![],
            source_page_numbers: vec![],
            model_request_metadata: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

impl Default for StudentAnswerOcrRecord {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: String::new(),
            submission_id: String::new(),
            question_id: String::new(),
            question_number: 0,
            source_page_numbers: vec![],
            source_image_refs: vec![],
            crop_refs: vec![],
            original_crop_refs: vec![],
            preprocessed_crop_refs: vec![],
            model_input_crop_ref: None,
            preprocess_mode: None,
            preprocess_version: None,
            preprocess_applied: false,
            preprocess_warnings: vec![],
            preprocess_diagnostics: vec![],
            available_preprocess_variants: vec![],
            full_page_preview_refs: vec![],
            answer_text: String::new(),
            structured_answer: None,
            confidence: None,
            uncertain_spans: vec![],
            suggested_corrections: vec![],
            critical_term_warnings: vec![],
            ocr_semantic_warnings: vec![],
            critical_keyword_uncertain: false,
            status: StudentAnswerOcrStatus::Pending,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            model_name: None,
            prompt_version: String::new(),
            created_at: now,
            updated_at: now,
            teacher_corrected_text: None,
            teacher_reviewed_at: None,
            parse_diagnostics: None,
            render_diagnostics: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageGroup {
    pub submission_id: String,
    pub document_id: String,
    #[serde(default)]
    pub page_numbers: Vec<u32>,
    pub student_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentScanReadinessSnapshot {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_status: Option<String>,
    pub current_stage: String,
    pub blocking_reasons: Vec<String>,
    pub next_actions: Vec<String>,
    pub submission_count: u32,
    pub preview_ready: bool,
    pub preview_current: u32,
    pub preview_total: u32,
    pub grouping_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_per_student: Option<u32>,
    pub warnings: Vec<String>,
    pub message: String,
}

pub fn new_student_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn student_identity_is_missing(student: &Student) -> bool {
    student
        .display_name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
        && student
            .number
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
}

pub fn submission_identity_missing(submission: &StudentSubmission, student: &Student) -> bool {
    submission.page_numbers.is_empty() || student_identity_is_missing(student)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn student(
        display_name: Option<&str>,
        number: Option<&str>,
        class_name: Option<&str>,
    ) -> Student {
        Student {
            id: "student-1".to_string(),
            display_name: display_name.map(ToString::to_string),
            number: number.map(ToString::to_string),
            class_name: class_name.map(ToString::to_string),
            warnings: vec![],
            identity_ocr: None,
        }
    }

    #[test]
    fn identity_requires_name_or_number() {
        assert!(student_identity_is_missing(&student(
            None,
            None,
            Some("11-A")
        )));
        assert!(!student_identity_is_missing(&student(
            Some("Ali Veli"),
            None,
            None
        )));
        assert!(!student_identity_is_missing(&student(
            None,
            Some("42"),
            None
        )));
    }
}

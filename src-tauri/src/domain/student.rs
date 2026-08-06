use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const OCR_REVIEW_POLICY_VERSION: &str = "ocr_review_policy_v1";
pub const OCR_REVIEW_POLICY_FINGERPRINT: &str =
    "ocr_review_policy_v1:low=0.72:critical=0.30:reasons=v1";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrReviewPolicy {
    pub version: String,
    pub fingerprint: String,
    pub low_confidence_threshold: f32,
    pub critical_confidence_threshold: f32,
    #[serde(default)]
    pub reason_labels: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrReviewPolicyDto {
    pub version: String,
    pub fingerprint: String,
    pub low_confidence_threshold: f32,
    pub critical_confidence_threshold: f32,
    #[serde(default)]
    pub reason_labels: BTreeMap<String, String>,
}

fn default_ocr_review_policy_dto() -> OcrReviewPolicyDto {
    default_ocr_review_policy().to_dto()
}

pub fn default_ocr_review_policy() -> OcrReviewPolicy {
    let reason_labels = BTreeMap::from([
        (
            "ocr_low_confidence".to_string(),
            "OCR güveni düşük".to_string(),
        ),
        (
            "critical_keyword_uncertain".to_string(),
            "Kritik ifade inceleme bekliyor".to_string(),
        ),
        (
            "critical_keyword_ocr_uncertain".to_string(),
            "Kritik ifade inceleme bekliyor".to_string(),
        ),
        (
            "ocr_answer_empty".to_string(),
            "Cevap okunamadı veya boş".to_string(),
        ),
        (
            "ocr_schema_incomplete".to_string(),
            "OCR yapısal çıktısı eksik".to_string(),
        ),
        (
            "parse_failed".to_string(),
            "OCR çıktısı doğrulanamadı".to_string(),
        ),
        (
            "ocr_parse_failed".to_string(),
            "OCR çıktısı doğrulanamadı".to_string(),
        ),
    ]);
    OcrReviewPolicy {
        version: OCR_REVIEW_POLICY_VERSION.to_string(),
        fingerprint: OCR_REVIEW_POLICY_FINGERPRINT.to_string(),
        low_confidence_threshold: 0.72,
        critical_confidence_threshold: 0.30,
        reason_labels,
    }
}

impl OcrReviewPolicy {
    pub fn should_review_confidence(&self, confidence: f32) -> bool {
        confidence < self.low_confidence_threshold
    }

    pub fn to_dto(&self) -> OcrReviewPolicyDto {
        OcrReviewPolicyDto {
            version: self.version.clone(),
            fingerprint: self.fingerprint.clone(),
            low_confidence_threshold: self.low_confidence_threshold,
            critical_confidence_threshold: self.critical_confidence_threshold,
            reason_labels: self.reason_labels.clone(),
        }
    }
}

impl From<OcrReviewPolicy> for OcrReviewPolicyDto {
    fn from(policy: OcrReviewPolicy) -> Self {
        policy.to_dto()
    }
}

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
    /// TD-01: owning written-family assessment activity. Additive field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_activity_id: Option<String>,
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
    #[serde(default)]
    pub job_mode: StudentAnswerOcrJobMode,
    /// TD-01: owning written-family assessment activity. Additive field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_activity_id: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedBBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnswerRegionRole {
    #[default]
    Primary,
    Continuation,
    Supporting,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationPolicy {
    #[default]
    Independent,
    ContinuesPrevious,
    Optional,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuestionAnswerRegion {
    /// Stable within a question so the exact region sent to the model can be
    /// traced from the OCR record and diagnostics.
    pub region_id: String,
    pub page_offset: u32,
    pub order: u32,
    pub normalized_bbox: NormalizedBBox,
    pub region_role: AnswerRegionRole,
    pub continuation_policy: ContinuationPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuestionAnswerTemplate {
    pub question_id: String,
    #[serde(default)]
    pub regions: Vec<QuestionAnswerRegion>,
}

impl QuestionAnswerTemplate {
    pub fn normalize_order(&mut self) {
        self.regions.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then(left.page_offset.cmp(&right.page_offset))
                .then(left.region_id.cmp(&right.region_id))
        });
        for (index, region) in self.regions.iter_mut().enumerate() {
            region.order = index as u32;
            if region.region_id.trim().is_empty() {
                region.region_id = format!("{}-region-{index}", self.question_id);
            }
        }
    }

    pub fn sorted_regions(&self) -> Vec<&QuestionAnswerRegion> {
        let mut regions = self.regions.iter().collect::<Vec<_>>();
        regions.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then(left.page_offset.cmp(&right.page_offset))
                .then(left.region_id.cmp(&right.region_id))
        });
        regions
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerCropTemplate {
    #[serde(default)]
    pub templates: Vec<QuestionAnswerTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CropTemplateCoverage {
    pub expected_question_count: usize,
    pub covered_question_count: usize,
    pub expected_region_count: usize,
    pub configured_region_count: usize,
    pub missing_question_ids: Vec<String>,
}

impl StudentAnswerCropTemplate {
    pub fn template_for_question(&self, question_id: &str) -> Option<&QuestionAnswerTemplate> {
        self.templates
            .iter()
            .find(|template| template.question_id == question_id)
    }

    pub fn coverage(&self, question_ids: &[String]) -> CropTemplateCoverage {
        let mut missing_question_ids = Vec::new();
        let mut covered_question_count = 0;
        let mut configured_region_count = 0;
        for question_id in question_ids {
            match self.template_for_question(question_id) {
                Some(template) if !template.regions.is_empty() => {
                    covered_question_count += 1;
                    configured_region_count += template.regions.len();
                }
                _ => missing_question_ids.push(question_id.clone()),
            }
        }
        CropTemplateCoverage {
            expected_question_count: question_ids.len(),
            covered_question_count,
            expected_region_count: configured_region_count,
            configured_region_count,
            missing_question_ids,
        }
    }

    pub fn normalize(&mut self) {
        for template in &mut self.templates {
            template.normalize_order();
        }
        self.templates
            .sort_by(|left, right| left.question_id.cmp(&right.question_id));
    }

    pub fn from_legacy_items(items: Vec<StudentAnswerCropTemplateItem>) -> Self {
        let templates = items
            .into_iter()
            .map(|item| QuestionAnswerTemplate {
                question_id: item.question_id.clone(),
                regions: vec![QuestionAnswerRegion {
                    region_id: format!("{}-region-0", item.question_id),
                    page_offset: item.page_index_within_submission,
                    order: 0,
                    normalized_bbox: NormalizedBBox {
                        x: item.bbox.x,
                        y: item.bbox.y,
                        width: item.bbox.width,
                        height: item.bbox.height,
                    },
                    region_role: AnswerRegionRole::Primary,
                    continuation_policy: ContinuationPolicy::Independent,
                    label: item.label,
                    note: item.note,
                }],
            })
            .collect::<Vec<_>>();
        let mut template = Self {
            templates,
            updated_at: None,
        };
        template.normalize();
        template
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StudentAnswerOcrJobMode {
    #[default]
    Production,
    ExperimentalFullPageReviewOnly,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provenance: Option<crate::domain::model::ModelProvenance>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrRenderDiagnostics {
    #[serde(default)]
    pub crop_refs: Vec<String>,
    #[serde(default)]
    pub region_ids: Vec<String>,
    #[serde(default)]
    pub region_orders: Vec<u32>,
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
pub struct OcrRegionProvenance {
    pub region_id: String,
    pub order: u32,
    pub page_offset: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrResizeDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrInputBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_images: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_bytes: Option<u64>,
    pub actual_image_count: u32,
    pub actual_input_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StudentAnswerOcrProvenance {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_checksum: Option<String>,
    #[serde(default)]
    pub source_page_numbers: Vec<u32>,
    #[serde(default)]
    pub region_ids: Vec<String>,
    #[serde(default)]
    pub region_orders: Vec<u32>,
    #[serde(default)]
    pub regions: Vec<OcrRegionProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_dpi: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_variant: Option<OcrImagePreprocessMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocess_version: Option<String>,
    #[serde(default)]
    pub resize_dimensions: Vec<OcrResizeDimensions>,
    #[serde(default)]
    pub jpeg_cache_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation: Option<crate::domain::model::ModelInvocationContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<OcrInputBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_diagnostics: Option<crate::domain::model::ModelDiagnostics>,
    pub approvable_for_scoring: bool,
    #[serde(default)]
    pub provenance_notes: Vec<String>,
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
    #[serde(
        default,
        deserialize_with = "crate::domain::structured_answer::deserialize_compat",
        skip_serializing_if = "Option::is_none"
    )]
    pub structured_answer: Option<crate::domain::structured_answer::StructuredAnswer>,
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
    pub review_policy: Option<OcrReviewPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provenance: Option<crate::domain::model::ModelProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_provenance: Option<StudentAnswerOcrProvenance>,
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
    /// TD-01: owning written-family assessment activity. Additive field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_activity_id: Option<String>,
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
            review_policy: None,
            model_provenance: None,
            model_name: None,
            prompt_version: String::new(),
            created_at: now,
            updated_at: now,
            teacher_corrected_text: None,
            teacher_reviewed_at: None,
            parse_diagnostics: None,
            render_diagnostics: None,
            ocr_provenance: None,
            assessment_activity_id: None,
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
    #[serde(default = "default_ocr_review_policy_dto")]
    pub ocr_review_policy: OcrReviewPolicyDto,
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

    #[test]
    fn multi_region_template_order_and_coverage_are_deterministic() {
        let region = |region_id: &str, page_offset: u32, order: u32| QuestionAnswerRegion {
            region_id: region_id.to_string(),
            page_offset,
            order,
            normalized_bbox: NormalizedBBox {
                x: 0.1,
                y: 0.1,
                width: 0.8,
                height: 0.2,
            },
            region_role: AnswerRegionRole::Continuation,
            continuation_policy: ContinuationPolicy::ContinuesPrevious,
            label: None,
            note: None,
        };
        let mut question_template = QuestionAnswerTemplate {
            question_id: "q1".to_string(),
            regions: vec![
                region("q1-region-b", 1, 0),
                region("q1-region-a", 0, 0),
                region("q1-region-c", 2, 2),
            ],
        };
        question_template.normalize_order();
        assert_eq!(
            question_template
                .regions
                .iter()
                .map(|region| (region.region_id.as_str(), region.order, region.page_offset))
                .collect::<Vec<_>>(),
            vec![
                ("q1-region-a", 0, 0),
                ("q1-region-b", 1, 1),
                ("q1-region-c", 2, 2),
            ]
        );

        let template = StudentAnswerCropTemplate {
            templates: vec![question_template],
            updated_at: None,
        };
        let coverage = template.coverage(&["q1".to_string(), "q2".to_string()]);
        assert_eq!(coverage.expected_question_count, 2);
        assert_eq!(coverage.covered_question_count, 1);
        assert_eq!(coverage.expected_region_count, 3);
        assert_eq!(coverage.configured_region_count, 3);
        assert_eq!(coverage.missing_question_ids, vec!["q2".to_string()]);
    }

    #[test]
    fn ocr_review_policy_is_versioned_and_backend_owned() {
        let policy = default_ocr_review_policy();
        assert_eq!(policy.version, OCR_REVIEW_POLICY_VERSION);
        assert_eq!(policy.fingerprint, OCR_REVIEW_POLICY_FINGERPRINT);
        assert!(!policy.should_review_confidence(0.72));
        assert!(policy.should_review_confidence(0.71));
        assert_eq!(
            policy.reason_labels.get("ocr_low_confidence"),
            Some(&"OCR güveni düşük".to_string())
        );
    }

    #[test]
    fn old_ocr_record_without_policy_or_provenance_still_deserializes() {
        let mut legacy_json = serde_json::to_value(StudentAnswerOcrRecord::default())
            .expect("serialize legacy-compatible record");
        let object = legacy_json.as_object_mut().expect("record object");
        object.remove("reviewPolicy");
        object.remove("modelProvenance");

        let restored: StudentAnswerOcrRecord =
            serde_json::from_value(legacy_json).expect("deserialize old record");
        assert!(restored.review_policy.is_none());
        assert!(restored.model_provenance.is_none());
    }

    #[test]
    fn old_readiness_without_ocr_policy_uses_backend_default() {
        let legacy_json = serde_json::json!({
            "projectId": "project-1",
            "ready": true,
            "currentStage": "ocr_ready",
            "blockingReasons": [],
            "nextActions": [],
            "submissionCount": 1,
            "previewReady": true,
            "previewCurrent": 1,
            "previewTotal": 1,
            "groupingComplete": true,
            "warnings": [],
            "message": "OCR hazır."
        });
        let restored: StudentScanReadinessSnapshot =
            serde_json::from_value(legacy_json).expect("deserialize old readiness");
        assert_eq!(
            restored.ocr_review_policy.version,
            OCR_REVIEW_POLICY_VERSION
        );
        assert_eq!(
            restored.ocr_review_policy.fingerprint,
            OCR_REVIEW_POLICY_FINGERPRINT
        );
    }
}

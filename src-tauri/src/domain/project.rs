use serde::{Deserialize, Serialize};

use super::document::Document;
use super::question::Question;
use super::school_class::{SchoolClass, StudentScanBatch};
use super::scoring::ScoringRecord;
use super::speaking::SpeakingExam;
use super::student::{
    PageGroupingMode, Student, StudentAnswerCropTemplate, StudentAnswerOcrRecord,
    StudentIdentityCropTemplate, StudentSubmission,
};
use super::workflow::WorkflowSnapshot;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub root_path: String,
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub students: Vec<Student>,
    #[serde(default)]
    pub school_classes: Vec<SchoolClass>,
    #[serde(default)]
    pub student_scan_batches: Vec<StudentScanBatch>,
    #[serde(default)]
    pub student_submissions: Vec<StudentSubmission>,
    #[serde(default)]
    pub student_answer_ocr_records: Vec<StudentAnswerOcrRecord>,
    #[serde(default)]
    pub student_answer_crop_template: StudentAnswerCropTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_identity_crop_template: Option<StudentIdentityCropTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_scan_document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_grouping_mode: Option<PageGroupingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_pages_per_student: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_grouping_complete_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_question_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exam_package_freeze: Option<ExamPackageFreeze>,
    pub documents: Vec<Document>,
    pub questions: Vec<Question>,
    #[serde(default)]
    pub scoring_records: Vec<ScoringRecord>,
    #[serde(default)]
    pub speaking_exams: Vec<SpeakingExam>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_scoring_run_id: Option<String>,
    pub workflow: WorkflowSnapshot,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExamPackageFreezeStatus {
    Frozen,
    Invalidated,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExamPackageFreeze {
    pub exam_package_version: u32,
    pub freeze_status: ExamPackageFreezeStatus,
    pub frozen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_by: Option<String>,
    pub source_hash: String,
    pub rubric_hash: String,
    pub question_text_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidation_reason: Option<String>,
}

impl Project {
    pub fn invalidate_exam_package_if_frozen(&mut self, reason: &str) {
        if let Some(freeze) = self.exam_package_freeze.as_mut() {
            if freeze.freeze_status == ExamPackageFreezeStatus::Frozen {
                freeze.freeze_status = ExamPackageFreezeStatus::Invalidated;
                freeze.invalidated_at = Some(chrono::Utc::now().to_rfc3339());
                freeze.invalidation_reason = Some(reason.to_string());
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub name: String,
    pub students: Vec<Student>,
}

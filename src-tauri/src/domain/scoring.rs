use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::model::{SamplingParameters, SemanticCriterionDecision};
use super::project::{ExamPackageFreezeStatus, Project};
use super::question::is_question_text_ready;
use super::rubric::{validate_rubric_state, RubricCriterion};
use super::student::{student_identity_is_missing, StudentAnswerOcrRecord};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScoringReviewStatus {
    PendingReview,
    Approved,
    Edited,
    Invalidated,
}

/// No calibration model is enabled in this phase. The version is still
/// persisted so an anchor can never silently cross a future calibration
/// policy boundary.
pub const SCORING_ANCHOR_CALIBRATION_VERSION: &str = "not_calibrated_v1";

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoringAnchorStatus {
    Active,
    Revoked,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoringAnchorActionKind {
    Created,
    Revoked,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoringAnchorEligibility {
    Eligible,
    Stale,
    Ineligible,
    Revoked,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringAnchorAction {
    pub action: ScoringAnchorActionKind,
    pub actor_kind: String,
    pub occurred_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringAnchorEvidence {
    #[serde(default)]
    pub answer_normalized_hash: String,
    #[serde(default)]
    pub answer_raw_hash: String,
    #[serde(default)]
    pub ocr_record_hash: String,
    pub awarded_score: f32,
    pub max_score: f32,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub criterion_scores: Vec<ScoringCriterionScore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_notes: Option<String>,
}

/// Canonical, append-preserving anchor state. Eligibility is intentionally
/// not stored here: it is derived against the currently open project/QEP by
/// `ScoringAnchorService`, so rubric and policy changes cannot be hidden by a
/// stale persisted flag.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringAnchor {
    pub id: String,
    pub version: String,
    pub source_record_id: String,
    pub question_id: String,
    pub question_number: u32,
    pub qep_fingerprint: String,
    pub question_text_hash: String,
    pub rubric_hash: String,
    pub policy_version: String,
    #[serde(default)]
    pub scoring_fingerprint: String,
    pub calibration_version: String,
    pub final_score: f32,
    pub max_score: f32,
    pub evidence: ScoringAnchorEvidence,
    pub status: ScoringAnchorStatus,
    #[serde(default)]
    pub actions: Vec<ScoringAnchorAction>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_reason: Option<String>,
}

/// Teacher-facing read DTO. The canonical anchor remains immutable apart from
/// explicit revoke metadata; the eligibility fields are a current read model.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringAnchorDto {
    #[serde(flatten)]
    pub anchor: ScoringAnchor,
    pub eligibility: ScoringAnchorEligibility,
    pub eligibility_reasons: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScoringDecisionState {
    #[default]
    Provisional,
    #[serde(alias = "candidate")]
    ModelCandidate,
    DeterministicAccepted,
    AutoAccepted,
    TeacherApproved,
    Rejected,
    Failed,
}

pub type ScoringCriterionScore = crate::domain::model::ScoringCriterionScore;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoringExecutionKind {
    Deterministic,
    Model,
    CandidateCache,
    ExactDuplicateReuse,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringExecutionDiagnostics {
    pub kind: ScoringExecutionKind,
    pub model_called: bool,
    pub model_call_count: u32,
    pub scorer_version: String,
    pub policy_version: String,
    #[serde(default)]
    pub cache_hit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_fingerprint: Option<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringCacheProvenance {
    pub fingerprint: String,
    pub artifact_schema_version: String,
    pub cache_hit: bool,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringReuseProvenance {
    pub source_record_id: String,
    pub source_decision_version: String,
    pub target_decision_version: String,
    pub match_key: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringConsistencyReview {
    pub reason_code: String,
    pub teacher_message: String,
    pub cluster_key: String,
    pub conflicting_record_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringFingerprintComponents {
    pub qep_fingerprint: String,
    pub question_id: String,
    pub answer_hash: String,
    pub ocr_generation: String,
    pub prompt_version: String,
    pub schema_version: String,
    pub policy_version: String,
    pub policy_fingerprint: String,
    pub model_file_fingerprint: String,
    pub runtime_fingerprint: String,
    pub sampling_parameters: SamplingParameters,
    pub calibration_version: String,
    pub anchor_version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringFingerprint {
    pub value: String,
    pub components: ScoringFingerprintComponents,
}

impl ScoringFingerprint {
    pub fn from_components(components: ScoringFingerprintComponents) -> Self {
        use sha2::{Digest, Sha256};

        let bytes = serde_json::to_vec(&components).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Self {
            value: hex::encode(hasher.finalize()),
            components,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringParseDiagnostics {
    pub raw_model_output: String,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringReconciliationDiagnostics {
    pub model_awarded_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criterion_sum: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criterion_max_sum: Option<f32>,
    pub question_max_score: f32,
    pub corrected_awarded_score: f32,
    pub needs_review: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringRecord {
    pub id: String,
    #[serde(default)]
    pub run_id: String,
    pub submission_id: String,
    pub student_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub student_class_name: Option<String>,
    pub question_id: String,
    pub question_number: u32,
    pub max_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awarded_score: Option<f32>,
    #[serde(default = "default_scoring_applied")]
    pub scoring_applied: bool,
    #[serde(default)]
    pub decision_state: ScoringDecisionState,
    #[serde(default)]
    pub decision_version: String,
    #[serde(default)]
    pub criterion_scores: Vec<ScoringCriterionScore>,
    #[serde(default)]
    pub semantic_decisions: Vec<SemanticCriterionDecision>,
    pub rationale: String,
    pub confidence: f32,
    pub needs_review: bool,
    #[serde(default)]
    pub review_reasons: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub raw_model_output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_diagnostics: Option<ScoringParseDiagnostics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation_diagnostics: Option<ScoringReconciliationDiagnostics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_diagnostics: Option<ScoringExecutionDiagnostics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_provenance: Option<ScoringCacheProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_provenance: Option<ScoringReuseProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consistency_review: Option<ScoringConsistencyReview>,
    #[serde(default)]
    pub scoring_fingerprint: String,
    #[serde(default)]
    pub policy_version: String,
    #[serde(default)]
    pub answer_normalized_hash: String,
    #[serde(default)]
    pub answer_raw_hash: String,
    #[serde(default)]
    pub ocr_generation: String,
    pub source_hash: String,
    pub package_hash: String,
    pub ocr_record_hash: String,
    pub question_text_hash: String,
    pub rubric_hash: String,
    pub teacher_review_status: ScoringReviewStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_manual_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidated_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalidation_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_scoring_applied() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringJobResult {
    pub total: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub needs_review: u32,
    pub approved: u32,
    pub partial: bool,
    #[serde(default)]
    pub summary: ScoringSummaryDto,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScoringSubmissionSummaryDto {
    pub submission_id: String,
    pub provisional_score: f32,
    pub accepted_score: f32,
    pub final_score: Option<f32>,
    pub max_score: f32,
    pub is_complete: bool,
    pub expected_record_count: u32,
    pub accepted_record_count: u32,
    pub provisional_record_count: u32,
    pub review_required_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScoringSummaryDto {
    pub provisional_score: f32,
    pub accepted_score: f32,
    pub final_score: Option<f32>,
    pub max_score: f32,
    pub is_complete: bool,
    pub expected_record_count: u32,
    pub accepted_record_count: u32,
    pub provisional_record_count: u32,
    pub review_required_count: u32,
    pub submissions: Vec<ScoringSubmissionSummaryDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoringReadiness {
    pub ready: bool,
    pub blockers: Vec<String>,
    pub expected_records: usize,
    pub scoring_record_count: usize,
    pub approved_record_count: usize,
    pub needs_review_record_count: usize,
    pub invalidated_record_count: usize,
    pub stale_record_count: usize,
}

pub fn scoring_record_hash(record: &StudentAnswerOcrRecord) -> String {
    stable_hash(&[
        &record.submission_id,
        &record.question_id,
        &record.question_number.to_string(),
        &record.answer_text,
        record.teacher_corrected_text.as_deref().unwrap_or_default(),
        &record.confidence.unwrap_or_default().to_string(),
        &serde_json::to_string(&record.uncertain_spans).unwrap_or_default(),
        &serde_json::to_string(&record.suggested_corrections).unwrap_or_default(),
        &serde_json::to_string(&record.critical_term_warnings).unwrap_or_default(),
        &record.ocr_semantic_warnings.join("|"),
        &record.critical_keyword_uncertain.to_string(),
        &format!("{:?}", record.status),
        &record.needs_review.to_string(),
        &record.review_reasons.join("|"),
        &record.warnings.join("|"),
        &record
            .structured_answer
            .as_ref()
            .map(|answer| serde_json::to_string(answer).unwrap_or_default())
            .unwrap_or_default(),
    ])
}

pub fn scoring_source_hash(project: &Project) -> String {
    project
        .exam_package_freeze
        .as_ref()
        .map(|freeze| freeze.source_hash.clone())
        .unwrap_or_else(|| stable_hash(&[&project.id, &project.root_path]))
}

pub fn scoring_question_text_hash(project: &Project) -> String {
    stable_hash(
        &project
            .questions
            .iter()
            .flat_map(|question| {
                [
                    question.id.clone(),
                    question.number.to_string(),
                    question.question_text.value.clone(),
                    format!("{:?}", question.question_text.status),
                    format!("{:?}", question.answer_type),
                ]
            })
            .collect::<Vec<_>>()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
}

pub fn scoring_rubric_hash(project: &Project) -> String {
    stable_hash(
        &project
            .questions
            .iter()
            .flat_map(|question| {
                let rubric = &question.rubric;
                let mut parts = vec![
                    question.id.clone(),
                    question.number.to_string(),
                    rubric.max_score.unwrap_or_default().to_string(),
                    rubric.expected_answer.clone().unwrap_or_default(),
                    format!("{:?}", rubric.status),
                    rubric
                        .criteria
                        .iter()
                        .map(|criterion| {
                            format!(
                                "{}:{}:{}:{}:{}",
                                criterion.id,
                                criterion.label,
                                criterion.description,
                                criterion.points,
                                serde_json::to_string(&criterion.levels).unwrap_or_default()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("|"),
                    rubric.partial_credit_hints.join("|"),
                    rubric.zero_score_conditions.join("|"),
                    rubric.common_mistakes.join("|"),
                ];
                if let Some(source) = rubric.source.as_ref() {
                    parts.push(format!("{:?}", source));
                }
                parts
            })
            .collect::<Vec<_>>()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
}

pub fn scoring_package_hash(project: &Project) -> String {
    let mut parts = vec![
        scoring_source_hash(project),
        scoring_question_text_hash(project),
        scoring_rubric_hash(project),
    ];

    for student in &project.students {
        parts.push(format!(
            "{}:{}:{}",
            student.id,
            student.display_name.clone().unwrap_or_default(),
            student.number.clone().unwrap_or_default()
        ));
    }

    for submission in &project.student_submissions {
        parts.push(format!(
            "{}:{}:{}:{:?}:{:?}",
            submission.id,
            submission.student_id,
            submission.document_id,
            submission.page_numbers,
            submission.status
        ));
    }

    for record in &project.student_answer_ocr_records {
        parts.push(format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            record.submission_id,
            record.question_id,
            record.question_number,
            record.answer_text,
            record.teacher_corrected_text.clone().unwrap_or_default(),
            serde_json::to_string(&record.uncertain_spans).unwrap_or_default(),
            serde_json::to_string(&record.suggested_corrections).unwrap_or_default(),
            serde_json::to_string(&record.critical_term_warnings).unwrap_or_default(),
            record.ocr_semantic_warnings.join("|"),
            record.critical_keyword_uncertain,
            record.needs_review,
            record.review_reasons.join("|"),
            record.warnings.join("|")
        ));
        parts.push(scoring_record_hash(record));
    }

    stable_hash(&parts.iter().map(String::as_str).collect::<Vec<_>>())
}

pub fn scoring_active_run_id(project: &Project) -> Option<String> {
    if let Some(run_id) = project
        .latest_scoring_run_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return Some(run_id.to_string());
    }

    scoring_latest_run_id_from_records(&project.scoring_records)
}

pub fn scoring_active_records(project: &Project) -> Vec<&ScoringRecord> {
    let records: Vec<&ScoringRecord> = if let Some(run_id) = scoring_active_run_id(project) {
        project
            .scoring_records
            .iter()
            .filter(|record| record.run_id == run_id)
            .collect()
    } else {
        project.scoring_records.iter().collect()
    };

    dedupe_scoring_records(records)
}

pub fn scoring_active_record_count(project: &Project) -> usize {
    scoring_active_records(project).len()
}

pub fn scoring_total_history_count(project: &Project) -> usize {
    project.scoring_records.len()
}

pub fn scoring_duplicate_result_count(project: &Project) -> usize {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for record in &project.scoring_records {
        *counts
            .entry((record.submission_id.clone(), record.question_id.clone()))
            .or_insert(0) += 1;
    }

    counts
        .into_values()
        .filter(|count| *count > 1)
        .map(|count| count - 1)
        .sum()
}

fn scoring_latest_run_id_from_records(records: &[ScoringRecord]) -> Option<String> {
    let mut best: Option<&ScoringRecord> = None;
    for record in records {
        if record.run_id.trim().is_empty() {
            continue;
        }
        match best {
            Some(current) if scoring_record_is_newer(record, current) => best = Some(record),
            None => best = Some(record),
            _ => {}
        }
    }

    best.map(|record| record.run_id.clone())
}

fn dedupe_scoring_records(records: Vec<&ScoringRecord>) -> Vec<&ScoringRecord> {
    let mut latest_by_key: BTreeMap<(String, String), &ScoringRecord> = BTreeMap::new();
    for record in records {
        let key = (record.submission_id.clone(), record.question_id.clone());
        match latest_by_key.get(&key) {
            Some(existing) if scoring_record_is_newer(record, existing) => {
                latest_by_key.insert(key, record);
            }
            None => {
                latest_by_key.insert(key, record);
            }
            _ => {}
        }
    }

    latest_by_key.into_values().collect()
}

fn scoring_record_is_newer(candidate: &ScoringRecord, current: &ScoringRecord) -> bool {
    match candidate.updated_at.cmp(&current.updated_at) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match candidate.created_at.cmp(&current.created_at) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => match candidate.run_id.cmp(&current.run_id) {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => candidate.id > current.id,
            },
        },
    }
}

pub fn scoring_readiness(project: &Project) -> ScoringReadiness {
    let expected_records = project.student_submissions.len() * project.questions.len();
    let active_records = scoring_active_records(project);
    let scoring_record_count = active_records.len();
    let approved_record_count = active_records
        .iter()
        .filter(|record| {
            matches!(
                record.teacher_review_status,
                ScoringReviewStatus::Approved | ScoringReviewStatus::Edited
            )
        })
        .count();
    let needs_review_record_count = active_records
        .iter()
        .filter(|record| record.needs_review)
        .count();
    let invalidated_record_count = active_records
        .iter()
        .filter(|record| {
            matches!(
                record.teacher_review_status,
                ScoringReviewStatus::Invalidated
            )
        })
        .count();
    let package_hash = scoring_package_hash(project);
    let stale_record_count = active_records
        .iter()
        .filter(|record| record.package_hash != package_hash)
        .count();

    let freeze_ready = project
        .exam_package_freeze
        .as_ref()
        .is_some_and(|freeze| freeze.freeze_status == ExamPackageFreezeStatus::Frozen);
    let questions_ready = !project.questions.is_empty()
        && project.questions.iter().all(|question| {
            is_question_text_ready(&question.question_text)
                && validate_rubric_state(&question.rubric, Some(&question.answer_type)).valid
        });
    let students_ready = !project.student_submissions.is_empty()
        && project
            .student_submissions
            .iter()
            .all(|submission| !submission.page_numbers.is_empty())
        && project
            .students
            .iter()
            .all(|student| !student_identity_is_missing(student));
    let ocr_ready = expected_records > 0
        && project.student_answer_ocr_records.len() == expected_records
        && project.student_answer_ocr_records.iter().all(|record| {
            record.status == super::student::StudentAnswerOcrStatus::TeacherApproved
                && !record.needs_review
        });

    let mut blockers = Vec::new();
    if !freeze_ready {
        blockers.push("QEP_NOT_FROZEN".to_string());
    }
    if !questions_ready {
        if project
            .questions
            .iter()
            .any(|question| !is_question_text_ready(&question.question_text))
        {
            blockers.push("QUESTION_TEXT_MISSING".to_string());
        }
        if project.questions.iter().any(|question| {
            !validate_rubric_state(&question.rubric, Some(&question.answer_type)).valid
        }) {
            blockers.push("RUBRIC_NOT_READY".to_string());
        }
    }
    if !students_ready {
        if project.student_submissions.is_empty() {
            blockers.push("STUDENT_GROUPING_NOT_READY".to_string());
        }
        if project.students.iter().any(student_identity_is_missing) {
            blockers.push("STUDENT_IDENTITY_INVALID".to_string());
        }
    }
    if !ocr_ready {
        blockers.push("STUDENT_ANSWER_OCR_NOT_READY".to_string());
    }
    if stale_record_count > 0 {
        blockers.push("SCORING_RERUN_REQUIRED".to_string());
    }

    blockers.sort();
    blockers.dedup();

    ScoringReadiness {
        ready: blockers.is_empty(),
        blockers,
        expected_records,
        scoring_record_count,
        approved_record_count,
        needs_review_record_count,
        invalidated_record_count,
        stale_record_count,
    }
}

pub fn scoring_record_effective_score(record: &ScoringRecord) -> Option<f32> {
    record.teacher_manual_score.or(record.awarded_score)
}

pub fn scoring_record_is_final(record: &ScoringRecord) -> bool {
    record.decision_state == ScoringDecisionState::TeacherApproved
        && record.scoring_applied
        && !record.needs_review
        && scoring_record_effective_score(record).is_some()
        && matches!(
            record.teacher_review_status,
            ScoringReviewStatus::Approved | ScoringReviewStatus::Edited
        )
        && !matches!(
            record.teacher_review_status,
            ScoringReviewStatus::Invalidated
        )
}

pub fn scoring_decision_transition_allowed(
    from: Option<ScoringDecisionState>,
    to: ScoringDecisionState,
    teacher_action: bool,
) -> bool {
    match from {
        None => matches!(
            to,
            ScoringDecisionState::ModelCandidate
                | ScoringDecisionState::DeterministicAccepted
                | ScoringDecisionState::Provisional
                | ScoringDecisionState::Failed
        ),
        Some(ScoringDecisionState::ModelCandidate) => {
            matches!(
                to,
                ScoringDecisionState::Provisional
                    | ScoringDecisionState::AutoAccepted
                    | ScoringDecisionState::Rejected
                    | ScoringDecisionState::Failed
            ) || (to == ScoringDecisionState::TeacherApproved && teacher_action)
        }
        Some(ScoringDecisionState::DeterministicAccepted)
        | Some(ScoringDecisionState::AutoAccepted)
        | Some(ScoringDecisionState::Provisional) => {
            matches!(
                to,
                ScoringDecisionState::Provisional | ScoringDecisionState::Rejected
            ) || (to == ScoringDecisionState::TeacherApproved && teacher_action)
        }
        Some(ScoringDecisionState::Failed) => {
            to == ScoringDecisionState::Rejected
                || (to == ScoringDecisionState::TeacherApproved && teacher_action)
        }
        Some(ScoringDecisionState::TeacherApproved) => {
            to == ScoringDecisionState::TeacherApproved
                || (to == ScoringDecisionState::Rejected && teacher_action)
        }
        Some(ScoringDecisionState::Rejected) => {
            to == ScoringDecisionState::Rejected
                || (to == ScoringDecisionState::TeacherApproved && teacher_action)
        }
    }
}

pub fn next_scoring_decision_version(current: &str) -> String {
    let number = current
        .strip_prefix('v')
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .saturating_add(1);
    format!("v{number}")
}

pub fn scoring_record_is_accepted(record: &ScoringRecord) -> bool {
    matches!(
        record.decision_state,
        ScoringDecisionState::DeterministicAccepted
            | ScoringDecisionState::AutoAccepted
            | ScoringDecisionState::TeacherApproved
    ) && record.scoring_applied
        && !record.needs_review
        && !matches!(
            record.teacher_review_status,
            ScoringReviewStatus::Invalidated
        )
        && scoring_record_effective_score(record).is_some()
}

pub fn scoring_record_is_provisional(record: &ScoringRecord) -> bool {
    matches!(
        record.decision_state,
        ScoringDecisionState::ModelCandidate | ScoringDecisionState::Provisional
    ) && record.scoring_applied
        && !matches!(
            record.teacher_review_status,
            ScoringReviewStatus::Invalidated
        )
        && scoring_record_effective_score(record).is_some()
}

fn scoring_summary_score(record: &ScoringRecord) -> Option<f32> {
    scoring_record_effective_score(record)
        .map(|score| score.max(0.0).min(record.max_score.max(0.0)))
}

pub fn scoring_summary(project: &Project) -> ScoringSummaryDto {
    let active_records = scoring_active_records(project);
    let mut submission_summaries = Vec::with_capacity(project.student_submissions.len());

    for submission in &project.student_submissions {
        let records: Vec<&ScoringRecord> = active_records
            .iter()
            .copied()
            .filter(|record| record.submission_id == submission.id)
            .collect();
        let accepted_records: Vec<&ScoringRecord> = records
            .iter()
            .copied()
            .filter(|record| {
                scoring_record_is_accepted(record) && scoring_summary_score(record).is_some()
            })
            .collect();
        let provisional_records: Vec<&ScoringRecord> = records
            .iter()
            .copied()
            .filter(|record| {
                scoring_record_is_provisional(record) && scoring_summary_score(record).is_some()
            })
            .collect();
        let accepted_score = accepted_records
            .iter()
            .filter_map(|record| scoring_summary_score(record))
            .sum::<f32>();
        let provisional_score = accepted_score
            + provisional_records
                .iter()
                .filter_map(|record| scoring_summary_score(record))
                .sum::<f32>();
        let max_score = project
            .questions
            .iter()
            .map(|question| {
                question
                    .rubric
                    .max_score
                    .unwrap_or(question.max_score)
                    .max(0.0)
            })
            .sum::<f32>();
        let expected_record_count = project.questions.len() as u32;
        let final_records: Vec<&ScoringRecord> = records
            .iter()
            .copied()
            .filter(|record| scoring_record_is_final(record))
            .collect();
        let final_score = final_records
            .iter()
            .filter_map(|record| scoring_summary_score(record))
            .sum::<f32>();
        let is_complete = expected_record_count > 0
            && project.questions.iter().all(|question| {
                final_records.iter().any(|record| {
                    record.question_id == question.id && scoring_summary_score(record).is_some()
                })
            });
        let review_required_count = records
            .iter()
            .filter(|record| {
                record.needs_review
                    || matches!(
                        record.decision_state,
                        ScoringDecisionState::Provisional | ScoringDecisionState::Failed
                    )
            })
            .count() as u32;

        submission_summaries.push(ScoringSubmissionSummaryDto {
            submission_id: submission.id.clone(),
            provisional_score,
            accepted_score,
            final_score: is_complete.then_some(final_score),
            max_score,
            is_complete,
            expected_record_count,
            accepted_record_count: accepted_records.len() as u32,
            provisional_record_count: provisional_records.len() as u32,
            review_required_count,
        });
    }

    let provisional_score = submission_summaries
        .iter()
        .map(|summary| summary.provisional_score)
        .sum::<f32>();
    let accepted_score = submission_summaries
        .iter()
        .map(|summary| summary.accepted_score)
        .sum::<f32>();
    let is_complete = !submission_summaries.is_empty()
        && submission_summaries
            .iter()
            .all(|summary| summary.is_complete);

    ScoringSummaryDto {
        provisional_score,
        accepted_score,
        final_score: is_complete.then_some(
            submission_summaries
                .iter()
                .filter_map(|summary| summary.final_score)
                .sum(),
        ),
        max_score: submission_summaries
            .iter()
            .map(|summary| summary.max_score)
            .sum(),
        is_complete,
        expected_record_count: submission_summaries
            .iter()
            .map(|summary| summary.expected_record_count)
            .sum(),
        accepted_record_count: submission_summaries
            .iter()
            .map(|summary| summary.accepted_record_count)
            .sum(),
        provisional_record_count: submission_summaries
            .iter()
            .map(|summary| summary.provisional_record_count)
            .sum(),
        review_required_count: submission_summaries
            .iter()
            .map(|summary| summary.review_required_count)
            .sum(),
        submissions: submission_summaries,
    }
}

pub fn scoring_record_is_current(record: &ScoringRecord, project: &Project) -> bool {
    record.package_hash == scoring_package_hash(project)
}

pub fn scoring_record_kind(record: &ScoringRecord) -> &'static str {
    match record.teacher_review_status {
        ScoringReviewStatus::PendingReview => "pending_review",
        ScoringReviewStatus::Approved => "approved",
        ScoringReviewStatus::Edited => "edited",
        ScoringReviewStatus::Invalidated => "invalidated",
    }
}

pub fn scoring_criterion_seed(
    question: &crate::domain::question::Question,
) -> Vec<ScoringCriterionScore> {
    question
        .rubric
        .criteria
        .iter()
        .map(|criterion| ScoringCriterionScore {
            criterion_id: criterion.id.clone(),
            criterion_title: criterion.label.clone(),
            criterion_max_score: criterion.points,
            awarded_score: 0.0,
            rationale: String::new(),
            evidence_quote: None,
        })
        .collect()
}

pub fn reconcile_scoring_award(
    model_awarded_score: f32,
    criterion_scores: &[ScoringCriterionScore],
    question_max_score: f32,
    initial_needs_review: bool,
    initial_warnings: Vec<String>,
) -> ScoringReconciliationOutcome {
    const SCORE_EPSILON: f32 = 0.01;

    let mut warnings = initial_warnings;
    let mut notes = Vec::new();
    let mut needs_review = initial_needs_review;
    let mut corrected_awarded_score = model_awarded_score.clamp(0.0, question_max_score);

    let criterion_summary = if criterion_scores.is_empty() {
        None
    } else {
        let criterion_sum = criterion_scores
            .iter()
            .map(|criterion| criterion.awarded_score)
            .sum::<f32>();
        let criterion_max_sum = criterion_scores
            .iter()
            .map(|criterion| criterion.criterion_max_score)
            .sum::<f32>();
        Some((criterion_sum, criterion_max_sum))
    };

    if let Some((criterion_sum, criterion_max_sum)) = criterion_summary {
        let criterion_sum_clamped = criterion_sum.clamp(0.0, question_max_score);
        if (criterion_max_sum - question_max_score).abs() > SCORE_EPSILON {
            warnings.push("criterion_max_sum_mismatch".to_string());
            needs_review = true;
            notes.push(format!(
                "Kriter puanları toplamı soru maksimum puanıyla uyuşmuyor; kriter max toplamı {:.2}, soru max {:.2}.",
                criterion_max_sum, question_max_score
            ));
        }
        if (model_awarded_score - criterion_sum).abs() > SCORE_EPSILON {
            warnings.push("model_score_mismatch_corrected".to_string());
            needs_review = true;
            notes.push(
                "Model üst puanı ile kriter toplamı uyuşmadı; puan kriter toplamına göre düzeltildi."
                    .to_string(),
            );
        }
        if criterion_sum > question_max_score + SCORE_EPSILON {
            warnings.push("criterion_sum_exceeds_question_max".to_string());
            needs_review = true;
            notes.push(
                "Kriter toplamı soru maksimum puanını aştı; puan üst sınıra çekildi.".to_string(),
            );
        }
        corrected_awarded_score = criterion_sum_clamped;
    }

    warnings.sort();
    warnings.dedup();

    ScoringReconciliationOutcome {
        awarded_score: corrected_awarded_score,
        needs_review,
        warnings: warnings.clone(),
        diagnostics: ScoringReconciliationDiagnostics {
            model_awarded_score,
            criterion_sum: criterion_summary.map(|(criterion_sum, _)| criterion_sum),
            criterion_max_sum: criterion_summary.map(|(_, criterion_max_sum)| criterion_max_sum),
            question_max_score,
            corrected_awarded_score,
            needs_review,
            warnings,
            notes,
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoringReconciliationOutcome {
    pub awarded_score: f32,
    pub needs_review: bool,
    pub warnings: Vec<String>,
    pub diagnostics: ScoringReconciliationDiagnostics,
}

pub fn scoring_rubric_criteria(
    question: &crate::domain::question::Question,
) -> Vec<RubricCriterion> {
    question.rubric.criteria.clone()
}

fn stable_hash(parts: &[&str]) -> String {
    let mut hash: u128 = 0xcbf29ce484222325cbf29ce484222325;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u128::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:032x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{Document, PdfPreviewState, PdfPreviewStatus};
    use crate::domain::project::ExamPackageFreeze;
    use crate::domain::question::{AnswerType, Question, TextFieldSource, TextFieldState};
    use crate::domain::rubric::RubricStatus;
    use crate::domain::student::{
        Student, StudentAnswerOcrStatus, StudentSubmission, StudentSubmissionStatus,
    };
    use uuid::Uuid;

    fn base_project() -> Project {
        Project {
            id: "project-1".into(),
            name: "Project".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            root_path: "/tmp/project".into(),
            storage_revision: 0,
            academic_year_id: None,
            course_id: None,
            course_name: None,
            sections: vec![],
            students: vec![Student {
                id: "student-1".into(),
                display_name: Some("Ali".into()),
                number: Some("1".into()),
                class_name: Some("A".into()),
                warnings: vec![],
                identity_ocr: None,
            }],
            school_classes: vec![],
            teaching_assignments: vec![],
            assessment_activities: vec![],
            student_scan_batches: vec![],
            student_submissions: vec![StudentSubmission {
                id: "submission-1".into(),
                student_id: "student-1".into(),
                document_id: "doc-1".into(),
                class_id: None,
                scan_batch_id: None,
                class_membership_source: None,
                page_numbers: vec![1, 2],
                status: StudentSubmissionStatus::Grouped,
                answer_slots: vec![],
                warnings: vec![],
                updated_at: None,
            }],
            student_answer_ocr_records: vec![crate::domain::student::StudentAnswerOcrRecord {
                id: "ocr-1".into(),
                submission_id: "submission-1".into(),
                question_id: "q-1".into(),
                question_number: 1,
                source_page_numbers: vec![1],
                source_image_refs: vec![],
                crop_refs: vec![],
                full_page_preview_refs: vec![],
                answer_text: "cevap".into(),
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
                review_policy: None,
                model_provenance: None,
                model_name: None,
                prompt_version: "v1".into(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                teacher_corrected_text: None,
                teacher_reviewed_at: Some(chrono::Utc::now()),
                parse_diagnostics: None,
                render_diagnostics: None,
                ..Default::default()
            }],
            student_answer_ocr_generations: vec![],
            student_answer_crop_template: Default::default(),
            student_identity_crop_template: None,
            student_scan_document_id: Some("doc-2".into()),
            student_grouping_mode: None,
            student_pages_per_student: Some(2),
            student_grouping_complete_at: Some("now".into()),
            expected_question_count: Some(1),
            exam_package_freeze: Some(ExamPackageFreeze {
                exam_package_version: 1,
                freeze_status: ExamPackageFreezeStatus::Frozen,
                frozen_at: "now".into(),
                frozen_by: None,
                source_hash: "source".into(),
                rubric_hash: "rubric".into(),
                question_text_hash: "question".into(),
                invalidated_at: None,
                invalidation_reason: None,
            }),
            documents: vec![Document {
                id: "doc-1".into(),
                role: crate::domain::document::DocumentRole::StudentScan,
                file_name: "scan.pdf".into(),
                stored_path: "scan.pdf".into(),
                page_count: 2,
                added_at: "now".into(),
                checksum: None,
                preview: Some(PdfPreviewState {
                    status: PdfPreviewStatus::Ready,
                    rendered_at: None,
                    page_count: Some(2),
                    job_id: None,
                    error_message: None,
                    active_generation_id: None,
                    pending_generation_id: None,
                    source_fingerprint: None,
                }),
            }],
            questions: vec![Question {
                id: "q-1".into(),
                number: 1,
                max_score: 10.0,
                answer_type: AnswerType::GeneralText,
                question_text: TextFieldState {
                    value: "Soru".into(),
                    source: TextFieldSource::Manual,
                    status: crate::domain::question::TextFieldStatus::Confirmed,
                    confidence: None,
                    warnings: vec![],
                    updated_at: None,
                },
                rubric: crate::domain::rubric::RubricState {
                    status: RubricStatus::Confirmed,
                    source: None,
                    max_score: Some(10.0),
                    expected_answer: Some("Cevap".into()),
                    key_concepts: vec![],
                    criteria: vec![RubricCriterion {
                        id: "c-1".into(),
                        label: "Kriter".into(),
                        description: "Açıklama".into(),
                        points: 10.0,
                        levels: vec![],
                    }],
                    partial_credit_hints: vec![],
                    zero_score_conditions: vec![],
                    common_mistakes: vec![],
                    warnings: vec![],
                    updated_at: None,
                },
                crop_template: None,
            }],
            latest_scoring_run_id: None,
            workflow: crate::domain::workflow::WorkflowSnapshot {
                current_stage: crate::domain::workflow::WorkflowStage::ScoringReady,
                current_stage_label: "Puanlama Hazır".into(),
                blocking_reasons: vec![],
                next_actions: vec![],
                summary: crate::domain::workflow::WorkflowSummary::default(),
            },
            scoring_records: vec![],
            scoring_anchors: vec![],
            speaking_exams: vec![],
        }
    }

    #[test]
    fn scoring_readiness_is_ready_for_minimal_project() {
        let project = base_project();
        let readiness = scoring_readiness(&project);
        assert!(readiness.ready);
        assert_eq!(readiness.blockers, Vec::<String>::new());
        assert_eq!(readiness.expected_records, 1);
    }

    #[test]
    fn reviewable_score_is_provisional_until_teacher_approval() {
        let mut project = base_project();
        let now = chrono::Utc::now();
        project.scoring_records = vec![ScoringRecord {
            id: "record-1".into(),
            run_id: "run-1".into(),
            submission_id: "submission-1".into(),
            student_id: "student-1".into(),
            student_display_name: Some("Ali".into()),
            student_number: Some("1".into()),
            student_class_name: Some("A".into()),
            question_id: "q-1".into(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(7.0),
            scoring_applied: true,
            decision_state: ScoringDecisionState::Provisional,
            decision_version: "v1".into(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "Kontrol gerektiren puan.".into(),
            confidence: 0.7,
            needs_review: true,
            review_reasons: vec!["low_scoring_confidence".into()],
            warnings: vec![],
            raw_model_output: "{}".into(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
            execution_diagnostics: None,
            cache_provenance: None,
            reuse_provenance: None,
            consistency_review: None,
            scoring_fingerprint: String::new(),
            policy_version: String::new(),
            answer_normalized_hash: String::new(),
            answer_raw_hash: String::new(),
            ocr_generation: String::new(),
            source_hash: "source".into(),
            package_hash: "package".into(),
            ocr_record_hash: "ocr".into(),
            question_text_hash: "question".into(),
            rubric_hash: "rubric".into(),
            teacher_review_status: ScoringReviewStatus::PendingReview,
            teacher_manual_score: None,
            teacher_reviewed_at: None,
            teacher_notes: None,
            invalidated_at: None,
            invalidation_reason: None,
            created_at: now,
            updated_at: now,
        }];

        let provisional = scoring_summary(&project);
        assert_eq!(provisional.provisional_score, 7.0);
        assert_eq!(provisional.accepted_score, 0.0);
        assert_eq!(provisional.final_score, None);
        assert!(!provisional.is_complete);

        let record = &mut project.scoring_records[0];
        record.decision_state = ScoringDecisionState::TeacherApproved;
        record.needs_review = false;
        record.teacher_review_status = ScoringReviewStatus::Approved;
        let accepted = scoring_summary(&project);
        assert_eq!(accepted.provisional_score, 7.0);
        assert_eq!(accepted.accepted_score, 7.0);
        assert_eq!(accepted.final_score, Some(7.0));
        assert!(accepted.is_complete);
    }

    #[test]
    fn scoring_lifecycle_requires_teacher_action_for_final_state() {
        assert!(scoring_decision_transition_allowed(
            Some(ScoringDecisionState::ModelCandidate),
            ScoringDecisionState::Provisional,
            false
        ));
        assert!(scoring_decision_transition_allowed(
            Some(ScoringDecisionState::ModelCandidate),
            ScoringDecisionState::TeacherApproved,
            true
        ));
        assert!(!scoring_decision_transition_allowed(
            Some(ScoringDecisionState::ModelCandidate),
            ScoringDecisionState::TeacherApproved,
            false
        ));
        assert!(scoring_decision_transition_allowed(
            Some(ScoringDecisionState::DeterministicAccepted),
            ScoringDecisionState::TeacherApproved,
            true
        ));
    }

    #[test]
    fn scoring_signature_changes_when_ocr_changes() {
        let mut project = base_project();
        let first = scoring_package_hash(&project);
        project.student_answer_ocr_records[0].answer_text = "değişti".into();
        let second = scoring_package_hash(&project);
        assert_ne!(first, second);
    }

    #[test]
    fn scoring_signature_ignores_class_organization_changes() {
        let mut project = base_project();
        let first = scoring_package_hash(&project);
        project.students[0].class_name = Some("11-B".into());
        let second = scoring_package_hash(&project);
        assert_eq!(first, second);
    }

    #[test]
    fn scoring_record_currentness_detects_stale_record() {
        let project = base_project();
        let mut record = ScoringRecord {
            id: Uuid::new_v4().to_string(),
            run_id: "run-1".into(),
            submission_id: "submission-1".into(),
            student_id: "student-1".into(),
            student_display_name: Some("Ali".into()),
            student_number: Some("1".into()),
            student_class_name: Some("A".into()),
            question_id: "q-1".into(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(8.0),
            scoring_applied: true,
            decision_state: ScoringDecisionState::AutoAccepted,
            decision_version: "v1".into(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "İyi".into(),
            confidence: 0.8,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: "{}".into(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
            execution_diagnostics: None,
            cache_provenance: None,
            reuse_provenance: None,
            consistency_review: None,
            scoring_fingerprint: String::new(),
            policy_version: String::new(),
            answer_normalized_hash: String::new(),
            answer_raw_hash: String::new(),
            ocr_generation: String::new(),
            source_hash: scoring_source_hash(&project),
            package_hash: scoring_package_hash(&project),
            ocr_record_hash: scoring_record_hash(&project.student_answer_ocr_records[0]),
            question_text_hash: scoring_question_text_hash(&project),
            rubric_hash: scoring_rubric_hash(&project),
            teacher_review_status: ScoringReviewStatus::PendingReview,
            teacher_manual_score: None,
            teacher_reviewed_at: None,
            teacher_notes: None,
            invalidated_at: None,
            invalidation_reason: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(scoring_record_is_current(&record, &project));
        record.awarded_score = Some(9.0);
        assert!(scoring_record_is_current(&record, &project));
        record.package_hash = "other".into();
        assert!(!scoring_record_is_current(&record, &project));
    }

    #[test]
    fn proof_38_scoring_rerun_preserves_teacher_override() {
        let project = base_project();
        let mut record = ScoringRecord {
            id: Uuid::new_v4().to_string(),
            run_id: "run-1".into(),
            submission_id: "submission-1".into(),
            student_id: "student-1".into(),
            student_display_name: Some("Ali".into()),
            student_number: Some("1".into()),
            student_class_name: Some("A".into()),
            question_id: "q-1".into(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(8.0),
            scoring_applied: true,
            decision_state: ScoringDecisionState::TeacherApproved,
            decision_version: "v1".into(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "İyi".into(),
            confidence: 0.8,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: "{}".into(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
            execution_diagnostics: None,
            cache_provenance: None,
            reuse_provenance: None,
            consistency_review: None,
            scoring_fingerprint: String::new(),
            policy_version: String::new(),
            answer_normalized_hash: String::new(),
            answer_raw_hash: String::new(),
            ocr_generation: String::new(),
            source_hash: scoring_source_hash(&project),
            package_hash: scoring_package_hash(&project),
            ocr_record_hash: scoring_record_hash(&project.student_answer_ocr_records[0]),
            question_text_hash: scoring_question_text_hash(&project),
            rubric_hash: scoring_rubric_hash(&project),
            teacher_review_status: ScoringReviewStatus::Edited,
            teacher_manual_score: Some(6.0),
            teacher_reviewed_at: Some(chrono::Utc::now()),
            teacher_notes: Some("Öğretmen düzeltmesi".into()),
            invalidated_at: None,
            invalidation_reason: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(scoring_record_effective_score(&record), Some(6.0));
        record.awarded_score = Some(1.0);
        assert_eq!(scoring_record_effective_score(&record), Some(6.0));
    }

    #[test]
    fn scoring_active_records_prefers_latest_run_and_dedupes_duplicates() {
        let mut project = base_project();
        project.latest_scoring_run_id = Some("run-new".into());
        let now = chrono::Utc::now();
        project.scoring_records = vec![
            ScoringRecord {
                id: "old-1".into(),
                run_id: "run-old".into(),
                submission_id: "submission-1".into(),
                student_id: "student-1".into(),
                student_display_name: Some("Ali".into()),
                student_number: Some("1".into()),
                student_class_name: Some("A".into()),
                question_id: "q-1".into(),
                question_number: 1,
                max_score: 10.0,
                awarded_score: Some(4.0),
                scoring_applied: true,
                decision_state: ScoringDecisionState::AutoAccepted,
                decision_version: "v1".into(),
                criterion_scores: vec![],
                semantic_decisions: vec![],
                rationale: "old".into(),
                confidence: 0.5,
                needs_review: false,
                review_reasons: vec![],
                warnings: vec![],
                raw_model_output: "{}".into(),
                parse_diagnostics: None,
                reconciliation_diagnostics: None,
                execution_diagnostics: None,
                cache_provenance: None,
                reuse_provenance: None,
                consistency_review: None,
                scoring_fingerprint: String::new(),
                policy_version: String::new(),
                answer_normalized_hash: String::new(),
                answer_raw_hash: String::new(),
                ocr_generation: String::new(),
                source_hash: "s".into(),
                package_hash: "p".into(),
                ocr_record_hash: "o".into(),
                question_text_hash: "q".into(),
                rubric_hash: "r".into(),
                teacher_review_status: ScoringReviewStatus::PendingReview,
                teacher_manual_score: None,
                teacher_reviewed_at: None,
                teacher_notes: None,
                invalidated_at: None,
                invalidation_reason: None,
                created_at: now - chrono::Duration::minutes(2),
                updated_at: now - chrono::Duration::minutes(2),
            },
            ScoringRecord {
                id: "new-1".into(),
                run_id: "run-new".into(),
                submission_id: "submission-1".into(),
                student_id: "student-1".into(),
                student_display_name: Some("Ali".into()),
                student_number: Some("1".into()),
                student_class_name: Some("A".into()),
                question_id: "q-1".into(),
                question_number: 1,
                max_score: 10.0,
                awarded_score: Some(8.0),
                scoring_applied: true,
                decision_state: ScoringDecisionState::AutoAccepted,
                decision_version: "v1".into(),
                criterion_scores: vec![],
                semantic_decisions: vec![],
                rationale: "new".into(),
                confidence: 0.9,
                needs_review: false,
                review_reasons: vec![],
                warnings: vec![],
                raw_model_output: "{}".into(),
                parse_diagnostics: None,
                reconciliation_diagnostics: None,
                execution_diagnostics: None,
                cache_provenance: None,
                reuse_provenance: None,
                consistency_review: None,
                scoring_fingerprint: String::new(),
                policy_version: String::new(),
                answer_normalized_hash: String::new(),
                answer_raw_hash: String::new(),
                ocr_generation: String::new(),
                source_hash: "s".into(),
                package_hash: "p".into(),
                ocr_record_hash: "o".into(),
                question_text_hash: "q".into(),
                rubric_hash: "r".into(),
                teacher_review_status: ScoringReviewStatus::PendingReview,
                teacher_manual_score: None,
                teacher_reviewed_at: None,
                teacher_notes: None,
                invalidated_at: None,
                invalidation_reason: None,
                created_at: now,
                updated_at: now,
            },
        ];

        let active_records = scoring_active_records(&project);
        assert_eq!(active_records.len(), 1);
        assert_eq!(active_records[0].run_id, "run-new");
        assert_eq!(scoring_active_run_id(&project).as_deref(), Some("run-new"));
        assert_eq!(scoring_duplicate_result_count(&project), 1);
    }

    #[test]
    fn scoring_active_records_respects_explicit_latest_run_even_before_results_arrive() {
        let mut project = base_project();
        project.latest_scoring_run_id = Some("run-new".into());
        project.scoring_records = vec![ScoringRecord {
            id: "old-1".into(),
            run_id: "run-old".into(),
            submission_id: "submission-1".into(),
            student_id: "student-1".into(),
            student_display_name: Some("Ali".into()),
            student_number: Some("1".into()),
            student_class_name: Some("A".into()),
            question_id: "q-1".into(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(4.0),
            scoring_applied: true,
            decision_state: ScoringDecisionState::AutoAccepted,
            decision_version: "v1".into(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "old".into(),
            confidence: 0.5,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: "{}".into(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
            execution_diagnostics: None,
            cache_provenance: None,
            reuse_provenance: None,
            consistency_review: None,
            scoring_fingerprint: String::new(),
            policy_version: String::new(),
            answer_normalized_hash: String::new(),
            answer_raw_hash: String::new(),
            ocr_generation: String::new(),
            source_hash: "s".into(),
            package_hash: "p".into(),
            ocr_record_hash: "o".into(),
            question_text_hash: "q".into(),
            rubric_hash: "r".into(),
            teacher_review_status: ScoringReviewStatus::PendingReview,
            teacher_manual_score: None,
            teacher_reviewed_at: None,
            teacher_notes: None,
            invalidated_at: None,
            invalidation_reason: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }];

        assert_eq!(scoring_active_run_id(&project).as_deref(), Some("run-new"));
        assert!(scoring_active_records(&project).is_empty());
    }

    #[test]
    fn scoring_active_records_falls_back_to_latest_non_empty_run_when_explicit_run_missing() {
        let mut project = base_project();
        project.latest_scoring_run_id = None;
        let now = chrono::Utc::now();
        project.scoring_records = vec![
            ScoringRecord {
                id: "legacy-1".into(),
                run_id: String::new(),
                submission_id: "submission-1".into(),
                student_id: "student-1".into(),
                student_display_name: Some("Ali".into()),
                student_number: Some("1".into()),
                student_class_name: Some("A".into()),
                question_id: "q-1".into(),
                question_number: 1,
                max_score: 10.0,
                awarded_score: Some(4.0),
                scoring_applied: true,
                decision_state: ScoringDecisionState::AutoAccepted,
                decision_version: "v1".into(),
                criterion_scores: vec![],
                semantic_decisions: vec![],
                rationale: "legacy".into(),
                confidence: 0.5,
                needs_review: false,
                review_reasons: vec![],
                warnings: vec![],
                raw_model_output: "{}".into(),
                parse_diagnostics: None,
                reconciliation_diagnostics: None,
                execution_diagnostics: None,
                cache_provenance: None,
                reuse_provenance: None,
                consistency_review: None,
                scoring_fingerprint: String::new(),
                policy_version: String::new(),
                answer_normalized_hash: String::new(),
                answer_raw_hash: String::new(),
                ocr_generation: String::new(),
                source_hash: "s".into(),
                package_hash: "p".into(),
                ocr_record_hash: "o".into(),
                question_text_hash: "q".into(),
                rubric_hash: "r".into(),
                teacher_review_status: ScoringReviewStatus::PendingReview,
                teacher_manual_score: None,
                teacher_reviewed_at: None,
                teacher_notes: None,
                invalidated_at: None,
                invalidation_reason: None,
                created_at: now - chrono::Duration::minutes(2),
                updated_at: now - chrono::Duration::minutes(2),
            },
            ScoringRecord {
                id: "new-1".into(),
                run_id: "run-new".into(),
                submission_id: "submission-1".into(),
                student_id: "student-1".into(),
                student_display_name: Some("Ali".into()),
                student_number: Some("1".into()),
                student_class_name: Some("A".into()),
                question_id: "q-1".into(),
                question_number: 1,
                max_score: 10.0,
                awarded_score: Some(8.0),
                scoring_applied: true,
                decision_state: ScoringDecisionState::AutoAccepted,
                decision_version: "v1".into(),
                criterion_scores: vec![],
                semantic_decisions: vec![],
                rationale: "new".into(),
                confidence: 0.9,
                needs_review: false,
                review_reasons: vec![],
                warnings: vec![],
                raw_model_output: "{}".into(),
                parse_diagnostics: None,
                reconciliation_diagnostics: None,
                execution_diagnostics: None,
                cache_provenance: None,
                reuse_provenance: None,
                consistency_review: None,
                scoring_fingerprint: String::new(),
                policy_version: String::new(),
                answer_normalized_hash: String::new(),
                answer_raw_hash: String::new(),
                ocr_generation: String::new(),
                source_hash: "s".into(),
                package_hash: "p".into(),
                ocr_record_hash: "o".into(),
                question_text_hash: "q".into(),
                rubric_hash: "r".into(),
                teacher_review_status: ScoringReviewStatus::PendingReview,
                teacher_manual_score: None,
                teacher_reviewed_at: None,
                teacher_notes: None,
                invalidated_at: None,
                invalidation_reason: None,
                created_at: now,
                updated_at: now,
            },
        ];

        let active_records = scoring_active_records(&project);
        assert_eq!(active_records.len(), 1);
        assert_eq!(active_records[0].run_id, "run-new");
    }

    #[test]
    fn reconcile_scoring_award_prefers_criterion_sum() {
        let outcome = reconcile_scoring_award(
            10.0,
            &[
                ScoringCriterionScore {
                    criterion_id: "c1".into(),
                    criterion_title: "A".into(),
                    criterion_max_score: 5.0,
                    awarded_score: 5.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
                ScoringCriterionScore {
                    criterion_id: "c2".into(),
                    criterion_title: "B".into(),
                    criterion_max_score: 5.0,
                    awarded_score: 5.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
                ScoringCriterionScore {
                    criterion_id: "c3".into(),
                    criterion_title: "C".into(),
                    criterion_max_score: 5.0,
                    awarded_score: 5.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
                ScoringCriterionScore {
                    criterion_id: "c4".into(),
                    criterion_title: "D".into(),
                    criterion_max_score: 5.0,
                    awarded_score: 5.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
            ],
            20.0,
            false,
            vec![],
        );

        assert_eq!(outcome.awarded_score, 20.0);
        assert!(outcome.needs_review);
        assert!(outcome
            .warnings
            .contains(&"model_score_mismatch_corrected".to_string()));
        assert_eq!(
            outcome.diagnostics.notes,
            vec![
                "Model üst puanı ile kriter toplamı uyuşmadı; puan kriter toplamına göre düzeltildi."
                    .to_string()
            ]
        );
    }

    #[test]
    fn reconcile_scoring_award_clamps_over_max_total() {
        let outcome = reconcile_scoring_award(
            30.0,
            &[
                ScoringCriterionScore {
                    criterion_id: "c1".into(),
                    criterion_title: "A".into(),
                    criterion_max_score: 10.0,
                    awarded_score: 10.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
                ScoringCriterionScore {
                    criterion_id: "c2".into(),
                    criterion_title: "B".into(),
                    criterion_max_score: 10.0,
                    awarded_score: 10.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
                ScoringCriterionScore {
                    criterion_id: "c3".into(),
                    criterion_title: "C".into(),
                    criterion_max_score: 10.0,
                    awarded_score: 10.0,
                    rationale: String::new(),
                    evidence_quote: None,
                },
            ],
            20.0,
            false,
            vec![],
        );

        assert_eq!(outcome.awarded_score, 20.0);
        assert!(outcome.needs_review);
        assert!(outcome
            .warnings
            .contains(&"criterion_sum_exceeds_question_max".to_string()));
    }
}

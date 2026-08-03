use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::document::PdfPagePreview;
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::scoring::{
    scoring_active_records, scoring_record_effective_score, scoring_record_is_accepted,
    ScoringRecord,
};
use crate::domain::student::{
    QuestionAnswerTemplate, Student, StudentAnswerOcrCropBBox, StudentSubmission,
};
use crate::services::pdf_preview_service::PdfPreviewService;
use crate::services::project_store::ProjectStore;

const BADGE_WIDTH: f32 = 0.135;
const BADGE_HEIGHT: f32 = 0.055;
const BADGE_GAP: f32 = 0.012;

#[derive(Clone)]
pub struct GradedExamReviewService {
    project_store: ProjectStore,
    pdf_preview_service: Arc<PdfPreviewService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GradedExamAnnotationStatus {
    ModelScore,
    NeedsReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GradedExamPlacement {
    RightOfAnswer,
    InsideTopRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradedExamScorePart {
    pub title: String,
    pub awarded_score: f32,
    pub max_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradedExamAnnotation {
    pub record_id: String,
    pub question_id: String,
    pub question_number: u32,
    pub model_score: Option<f32>,
    pub max_score: f32,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub placement: GradedExamPlacement,
    pub status: GradedExamAnnotationStatus,
    pub needs_review: bool,
    pub score_parts: Vec<GradedExamScorePart>,
    pub review_guidance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradedExamUnplacedScore {
    pub record_id: String,
    pub question_id: String,
    pub question_number: u32,
    pub model_score: Option<f32>,
    pub max_score: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradedExamPage {
    pub page_number: u32,
    pub image_path: String,
    pub width: u32,
    pub height: u32,
    pub annotations: Vec<GradedExamAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradedExamReview {
    pub project_id: String,
    pub submission_id: String,
    pub document_id: String,
    pub student_display_name: String,
    pub student_number: Option<String>,
    pub student_class_name: Option<String>,
    pub scoring_run_id: Option<String>,
    pub model_total_score: Option<f32>,
    pub max_total_score: f32,
    pub needs_review_count: u32,
    pub pages: Vec<GradedExamPage>,
    pub unplaced_scores: Vec<GradedExamUnplacedScore>,
}

impl GradedExamReviewService {
    pub fn new(project_store: ProjectStore, pdf_preview_service: Arc<PdfPreviewService>) -> Self {
        Self {
            project_store,
            pdf_preview_service,
        }
    }

    pub fn get_review(
        &self,
        project_id: &str,
        submission_id: &str,
    ) -> Result<GradedExamReview, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let submission = project
            .student_submissions
            .iter()
            .find(|submission| submission.id == submission_id)
            .ok_or_else(|| {
                app_error(
                    AppErrorCode::StudentSubmissionNotFound,
                    "İncelenecek öğrenci sınavı bulunamadı.",
                    Some(format!("submission_id={submission_id}")),
                    "Öğrenci gruplamasını kontrol edin.",
                )
            })?;
        let records = scoring_active_records(&project)
            .into_iter()
            .filter(|record| record.submission_id == submission.id)
            .collect::<Vec<_>>();
        if records.is_empty() {
            return Err(app_error(
                AppErrorCode::ScoringNotReady,
                "Bu öğrenci için gösterilecek model puanı henüz yok.",
                Some(format!("submission_id={submission_id}")),
                "Önce notlandırmayı tamamlayın.",
            ));
        }

        let previews = self
            .pdf_preview_service
            .require_ready_page_previews(&project.id, &submission.document_id)?;
        let student = project
            .students
            .iter()
            .find(|student| student.id == submission.student_id);

        build_review(
            &project.id,
            project.latest_scoring_run_id.clone(),
            submission,
            student,
            &records,
            &project.student_answer_crop_template.templates,
            &previews,
        )
    }
}

fn build_review(
    project_id: &str,
    scoring_run_id: Option<String>,
    submission: &StudentSubmission,
    student: Option<&Student>,
    records: &[&ScoringRecord],
    templates: &[QuestionAnswerTemplate],
    previews: &[PdfPagePreview],
) -> Result<GradedExamReview, AppError> {
    let previews_by_page = previews
        .iter()
        .map(|preview| (preview.page_number, preview))
        .collect::<BTreeMap<_, _>>();
    let mut pages = Vec::with_capacity(submission.page_numbers.len());
    for page_number in &submission.page_numbers {
        let preview = previews_by_page.get(page_number).ok_or_else(|| {
            app_error(
                AppErrorCode::PdfPreviewNotReady,
                "Öğrenci sınavının bir sayfa önizlemesi eksik.",
                Some(format!(
                    "submission_id={}; page_number={page_number}",
                    submission.id
                )),
                "Öğrenci PDF önizlemelerini yeniden oluşturun.",
            )
        })?;
        pages.push(GradedExamPage {
            page_number: *page_number,
            image_path: preview.image_path.clone(),
            width: preview.width,
            height: preview.height,
            annotations: vec![],
        });
    }

    let mut unplaced_scores = Vec::new();
    let mut model_total_score = 0.0_f32;
    let mut model_score_count = 0_u32;
    let mut max_total_score = 0.0_f32;
    let mut needs_review_count = 0_u32;

    for record in records {
        max_total_score += record.max_score;
        if record.needs_review || !record.scoring_applied || record.awarded_score.is_none() {
            needs_review_count += 1;
        }
        if scoring_record_is_accepted(record) {
            if let Some(score) = scoring_record_effective_score(record) {
                model_total_score += score;
                model_score_count += 1;
            }
        }

        let Some(template) = templates
            .iter()
            .find(|item| item.question_id == record.question_id)
        else {
            unplaced_scores.push(unplaced_score(record, "Soru konumu tanımlı değil."));
            continue;
        };
        let sorted_regions = template.sorted_regions();
        let Some(region) = sorted_regions.first() else {
            unplaced_scores.push(unplaced_score(record, "Soru cevap bölgesi tanımlı değil."));
            continue;
        };
        let Some(page_number) = submission
            .page_numbers
            .get(region.page_offset as usize)
            .copied()
        else {
            unplaced_scores.push(unplaced_score(
                record,
                "Soru sayfası öğrenci sınavında bulunamadı.",
            ));
            continue;
        };
        let Some(page) = pages
            .iter_mut()
            .find(|page| page.page_number == page_number)
        else {
            unplaced_scores.push(unplaced_score(record, "Soru sayfası önizlenemiyor."));
            continue;
        };

        let bbox = StudentAnswerOcrCropBBox {
            x: region.normalized_bbox.x,
            y: region.normalized_bbox.y,
            width: region.normalized_bbox.width,
            height: region.normalized_bbox.height,
            page_index: region.page_offset,
        };
        let (x, y, placement) = annotation_position(&bbox, &page.annotations);
        let model_score = record
            .scoring_applied
            .then_some(record.awarded_score)
            .flatten();
        let needs_review = record.needs_review || model_score.is_none();
        let score_parts = record
            .criterion_scores
            .iter()
            .map(|criterion| GradedExamScorePart {
                title: criterion.criterion_title.clone(),
                awarded_score: criterion.awarded_score,
                max_score: criterion.criterion_max_score,
            })
            .collect();
        page.annotations.push(GradedExamAnnotation {
            record_id: record.id.clone(),
            question_id: record.question_id.clone(),
            question_number: record.question_number,
            model_score,
            max_score: record.max_score,
            label: score_label(model_score, record.max_score),
            x,
            y,
            width: BADGE_WIDTH,
            height: BADGE_HEIGHT,
            placement,
            status: if model_score.is_some() {
                GradedExamAnnotationStatus::ModelScore
            } else {
                GradedExamAnnotationStatus::NeedsReview
            },
            needs_review,
            score_parts,
            review_guidance: review_guidance(record),
        });
    }

    for page in &mut pages {
        page.annotations
            .sort_by_key(|annotation| annotation.question_number);
    }
    unplaced_scores.sort_by_key(|score| score.question_number);

    Ok(GradedExamReview {
        project_id: project_id.to_string(),
        submission_id: submission.id.clone(),
        document_id: submission.document_id.clone(),
        student_display_name: student
            .and_then(|student| student.display_name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "İsimsiz öğrenci".to_string()),
        student_number: student.and_then(|student| student.number.clone()),
        student_class_name: student.and_then(|student| student.class_name.clone()),
        scoring_run_id,
        model_total_score: (model_score_count > 0).then_some(model_total_score),
        max_total_score,
        needs_review_count,
        pages,
        unplaced_scores,
    })
}

fn unplaced_score(record: &ScoringRecord, reason: &str) -> GradedExamUnplacedScore {
    GradedExamUnplacedScore {
        record_id: record.id.clone(),
        question_id: record.question_id.clone(),
        question_number: record.question_number,
        model_score: record
            .scoring_applied
            .then_some(record.awarded_score)
            .flatten(),
        max_score: record.max_score,
        reason: reason.to_string(),
    }
}

fn annotation_position(
    bbox: &StudentAnswerOcrCropBBox,
    existing: &[GradedExamAnnotation],
) -> (f32, f32, GradedExamPlacement) {
    let answer_right = (bbox.x + bbox.width).clamp(0.0, 1.0);
    let (x, placement) = if answer_right + BADGE_GAP + BADGE_WIDTH <= 0.985 {
        (answer_right + BADGE_GAP, GradedExamPlacement::RightOfAnswer)
    } else {
        (
            (answer_right - BADGE_WIDTH - BADGE_GAP).clamp(0.015, 0.985 - BADGE_WIDTH),
            GradedExamPlacement::InsideTopRight,
        )
    };
    let mut y = bbox.y.clamp(0.015, 0.985 - BADGE_HEIGHT);
    while existing.iter().any(|item| rectangles_overlap(x, y, item)) {
        y += BADGE_HEIGHT + 0.008;
        if y + BADGE_HEIGHT > 0.985 {
            y = (bbox.y - BADGE_HEIGHT - 0.008).clamp(0.015, 0.985 - BADGE_HEIGHT);
            break;
        }
    }
    (x, y, placement)
}

fn rectangles_overlap(x: f32, y: f32, existing: &GradedExamAnnotation) -> bool {
    x < existing.x + existing.width
        && x + BADGE_WIDTH > existing.x
        && y < existing.y + existing.height
        && y + BADGE_HEIGHT > existing.y
}

fn score_label(score: Option<f32>, max_score: f32) -> String {
    match score {
        Some(score) => format!("{}/{}", compact_score(score), compact_score(max_score)),
        None => "Kontrol".to_string(),
    }
}

fn review_guidance(record: &ScoringRecord) -> Vec<String> {
    let mut codes = record.review_reasons.clone();
    codes.extend(record.warnings.iter().cloned());
    codes.sort();
    codes.dedup();
    let mut guidance = codes
        .iter()
        .map(|code| match code.as_str() {
            "critical_keyword_ocr_uncertain" | "ocr_critical_keyword_uncertain" => {
                "El yazısındaki kritik terimi OCR okumasıyla karşılaştırın."
            }
            "ocr_parse_failed" => {
                "Öğrenci cevabının OCR tarafından doğru okunup okunmadığını kontrol edin."
            }
            "low_scoring_confidence" => {
                "Model güveni düşük; cevabın rubrikle uyumunu kontrol edin."
            }
            "scoring_json_parse_failed" | "MODEL_RESPONSE_INVALID_JSON" => {
                "Model güvenilir puan üretemedi; cevabı manuel puanlayın."
            }
            "scoring_criteria_incomplete" | "criterion_rationale_incomplete" => {
                "Rubrik ölçütlerinin tamamının ayrı ayrı değerlendirildiğini kontrol edin."
            }
            "scoring_rationale_too_short" => {
                "Puan gerekçesi yetersiz; kriter puanlarını kontrol edin."
            }
            "scoring_criterion_max_mismatch" | "scoring_criterion_score_out_of_range" => {
                "Kriter puanlarının rubrik üst sınırlarına uyduğunu kontrol edin."
            }
            "model_score_mismatch_corrected"
            | "criterion_sum_exceeds_question_max"
            | "criterion_max_sum_mismatch" => {
                "Kriter toplamı ile soru toplam puanını karşılaştırın."
            }
            "MODEL_SERVER_NOT_RUNNING"
            | "MODEL_TIMEOUT"
            | "MODEL_RESPONSE_EMPTY"
            | "ModelServerNotRunning"
            | "ModelTimeout"
            | "ModelResponseEmpty"
            | "ModelResponseInvalidJson" => "Model puanı tamamlanamadı; cevabı manuel puanlayın.",
            "ocr_record_missing" | "ocr_not_approved" => {
                "Onaylı öğrenci cevabını kontrol edip manuel puanlayın."
            }
            _ => "Cevabı ve kriter puanlarını öğretmen gözüyle kontrol edin.",
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    guidance.sort();
    guidance.dedup();
    if record.needs_review && guidance.is_empty() {
        guidance.push("Cevabı ve kriter puanlarını öğretmen gözüyle kontrol edin.".to_string());
    }
    guidance
}

fn compact_score(value: f32) -> String {
    if value.fract().abs() < f32::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn app_error(
    code: AppErrorCode,
    message: &str,
    technical_details: Option<String>,
    suggested_action: &str,
) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(suggested_action.to_string()),
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::scoring::{ScoringDecisionState, ScoringReviewStatus};
    use crate::domain::student::{
        AnswerRegionRole, ContinuationPolicy, NormalizedBBox, QuestionAnswerRegion,
        QuestionAnswerTemplate, StudentSubmissionStatus,
    };

    fn scoring_record() -> ScoringRecord {
        let now = chrono::Utc::now();
        ScoringRecord {
            id: "record-1".to_string(),
            run_id: "run-1".to_string(),
            submission_id: "submission-1".to_string(),
            student_id: "student-1".to_string(),
            student_display_name: Some("Ada Öğrenci".to_string()),
            student_number: Some("42".to_string()),
            student_class_name: Some("10-A".to_string()),
            question_id: "question-1".to_string(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(7.5),
            scoring_applied: true,
            decision_state: ScoringDecisionState::AutoAccepted,
            decision_version: "v1".to_string(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "Yanıt büyük ölçüde doğru.".to_string(),
            confidence: 0.9,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: "{}".to_string(),
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
            source_hash: "source".to_string(),
            package_hash: "package".to_string(),
            ocr_record_hash: "ocr".to_string(),
            question_text_hash: "question".to_string(),
            rubric_hash: "rubric".to_string(),
            teacher_review_status: ScoringReviewStatus::PendingReview,
            teacher_manual_score: None,
            teacher_reviewed_at: None,
            teacher_notes: None,
            invalidated_at: None,
            invalidation_reason: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn submission() -> StudentSubmission {
        StudentSubmission {
            id: "submission-1".to_string(),
            student_id: "student-1".to_string(),
            document_id: "document-1".to_string(),
            class_id: None,
            scan_batch_id: None,
            class_membership_source: None,
            page_numbers: vec![3],
            status: StudentSubmissionStatus::Grouped,
            answer_slots: vec![],
            warnings: vec![],
            updated_at: None,
        }
    }

    fn template() -> QuestionAnswerTemplate {
        QuestionAnswerTemplate {
            question_id: "question-1".to_string(),
            regions: vec![QuestionAnswerRegion {
                region_id: "question-1-region-0".to_string(),
                page_offset: 0,
                order: 0,
                normalized_bbox: NormalizedBBox {
                    x: 0.1,
                    y: 0.2,
                    width: 0.5,
                    height: 0.2,
                },
                region_role: AnswerRegionRole::Primary,
                continuation_policy: ContinuationPolicy::Independent,
                label: None,
                note: None,
            }],
        }
    }

    fn preview() -> PdfPagePreview {
        PdfPagePreview {
            document_id: "document-1".to_string(),
            page_number: 3,
            image_path: "/tmp/page-3.png".to_string(),
            width: 1200,
            height: 1800,
            rendered_at: "2026-07-20T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn builds_read_only_review_from_score_crop_and_preview() {
        let submission = submission();
        let mut record = scoring_record();
        record.criterion_scores = vec![
            crate::domain::model::ScoringCriterionScore {
                criterion_id: "c1".to_string(),
                criterion_title: "Ana düşünce".to_string(),
                criterion_max_score: 4.0,
                awarded_score: 4.0,
                rationale: "Doğru".to_string(),
                evidence_quote: Some("Ana düşünce".to_string()),
            },
            crate::domain::model::ScoringCriterionScore {
                criterion_id: "c2".to_string(),
                criterion_title: "Açıklama".to_string(),
                criterion_max_score: 6.0,
                awarded_score: 3.5,
                rationale: "Kısmen doğru".to_string(),
                evidence_quote: Some("Açıklama".to_string()),
            },
        ];
        let template = template();
        let preview = preview();
        let review = build_review(
            "project-1",
            Some("run-1".to_string()),
            &submission,
            None,
            &[&record],
            &[template],
            &[preview],
        )
        .expect("valid review inputs should build");

        assert_eq!(review.model_total_score, Some(7.5));
        assert_eq!(review.pages.len(), 1);
        assert_eq!(review.pages[0].annotations[0].label, "7.5/10");
        assert_eq!(review.pages[0].annotations[0].score_parts.len(), 2);
        assert!(review.unplaced_scores.is_empty());
    }

    #[test]
    fn converts_technical_review_codes_into_teacher_guidance() {
        let mut record = scoring_record();
        record.needs_review = true;
        record.review_reasons = vec![
            "low_scoring_confidence".to_string(),
            "critical_keyword_ocr_uncertain".to_string(),
        ];

        let guidance = review_guidance(&record);
        assert!(guidance
            .iter()
            .any(|item| item.contains("Model güveni düşük")));
        assert!(guidance.iter().any(|item| item.contains("kritik terimi")));
        assert!(!guidance.iter().any(|item| item.contains('_')));
    }

    #[test]
    fn reports_missing_submission_page_preview_as_structured_error() {
        let submission = submission();
        let record = scoring_record();
        let error = build_review(
            "project-1",
            Some("run-1".to_string()),
            &submission,
            None,
            &[&record],
            &[template()],
            &[],
        )
        .expect_err("missing page preview must block review");

        assert_eq!(error.code, AppErrorCode::PdfPreviewNotReady);
        assert!(error.recoverable);
    }

    #[test]
    fn places_score_to_right_when_page_has_room() {
        let bbox = StudentAnswerOcrCropBBox {
            x: 0.1,
            y: 0.2,
            width: 0.5,
            height: 0.2,
            page_index: 0,
        };
        let (x, y, placement) = annotation_position(&bbox, &[]);
        assert_eq!(placement, GradedExamPlacement::RightOfAnswer);
        assert!(x > 0.6);
        assert_eq!(y, 0.2);
    }

    #[test]
    fn keeps_score_inside_page_when_answer_reaches_right_edge() {
        let bbox = StudentAnswerOcrCropBBox {
            x: 0.55,
            y: 0.9,
            width: 0.44,
            height: 0.09,
            page_index: 0,
        };
        let (x, y, placement) = annotation_position(&bbox, &[]);
        assert_eq!(placement, GradedExamPlacement::InsideTopRight);
        assert!(x + BADGE_WIDTH <= 0.985);
        assert!(y + BADGE_HEIGHT <= 0.985);
    }

    #[test]
    fn score_label_never_turns_missing_model_score_into_zero() {
        assert_eq!(score_label(None, 10.0), "Kontrol");
        assert_eq!(score_label(Some(7.5), 10.0), "7.5/10");
    }
}

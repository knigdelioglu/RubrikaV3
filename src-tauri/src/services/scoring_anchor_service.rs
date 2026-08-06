use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::project::{ExamPackageFreezeStatus, Project};
use crate::domain::question::Question;
use crate::domain::scoring::{
    scoring_package_hash, scoring_question_text_hash, scoring_record_effective_score,
    scoring_record_is_final, scoring_rubric_hash, ScoringAnchor, ScoringAnchorAction,
    ScoringAnchorActionKind, ScoringAnchorDto, ScoringAnchorEligibility, ScoringAnchorEvidence,
    ScoringAnchorStatus, SCORING_ANCHOR_CALIBRATION_VERSION,
};
use crate::domain::student::{StudentAnswerOcrRecord, StudentAnswerOcrStatus};
use crate::services::audit_service::{AuditEntryInput, AuditService};
use crate::services::deterministic_scoring_service::{
    DeterministicScoringService, DETERMINISTIC_SCORING_POLICY_VERSION,
};
use crate::services::project_store::{MutationOptions, ProjectStore};
use crate::services::semantic_scoring_service::SEMANTIC_SCORING_POLICY_VERSION;

const ANCHOR_CREATE_OPERATION: &str = "create_scoring_anchor";
const ANCHOR_REVOKE_OPERATION: &str = "revoke_scoring_anchor";

#[derive(Clone)]
pub struct ScoringAnchorService {
    project_store: ProjectStore,
    audit_service: Arc<AuditService>,
}

impl ScoringAnchorService {
    pub fn new(project_store: ProjectStore, audit_service: Arc<AuditService>) -> Self {
        Self {
            project_store,
            audit_service,
        }
    }

    pub fn list(&self, project_id: &str) -> Result<Vec<ScoringAnchorDto>, AppError> {
        let project = self
            .project_store
            .get_project_snapshot(project_id.to_string())?;
        let mut anchors = project
            .scoring_anchors
            .iter()
            .cloned()
            .map(|anchor| anchor_dto(&project, anchor))
            .collect::<Vec<_>>();
        anchors.sort_by(|left, right| {
            left.anchor
                .question_number
                .cmp(&right.anchor.question_number)
                .then(left.anchor.created_at.cmp(&right.anchor.created_at))
                .then(left.anchor.id.cmp(&right.anchor.id))
        });
        Ok(anchors)
    }

    pub fn create(
        &self,
        project_id: &str,
        source_record_id: &str,
    ) -> Result<ScoringAnchorDto, AppError> {
        let correlation_id = Uuid::new_v4().to_string();
        let output = self.project_store.mutate(
            project_id,
            MutationOptions {
                expected_revision: None,
                expected_fingerprint: None,
                operation: ANCHOR_CREATE_OPERATION.to_string(),
                correlation_id: correlation_id.clone(),
            },
            |project, _context| {
                let anchor = build_anchor(project, source_record_id)?;
                if project.scoring_anchors.iter().any(|existing| {
                    existing.source_record_id == source_record_id
                        && existing.status == ScoringAnchorStatus::Active
                }) {
                    return Err(anchor_error(
                        AppErrorCode::ScoringAnchorAlreadyExists,
                        "Bu öğretmen onaylı karar zaten anchor olarak kayıtlı.",
                        Some(format!("source_record_id={source_record_id}")),
                        "Mevcut anchor kaydını kullanın veya önce statüsünü kaldırın.",
                    ));
                }
                project.scoring_anchors.push(anchor.clone());
                Ok(anchor)
            },
        )?;

        let anchor = output.result;
        self.append_audit(
            &output.snapshot.project,
            &correlation_id,
            ANCHOR_CREATE_OPERATION,
            &anchor,
            None,
            json!({
                "sourceRecordId": anchor.source_record_id,
                "anchorVersion": anchor.version,
                "calibrationVersion": anchor.calibration_version,
                "qepFingerprint": anchor.qep_fingerprint,
            }),
        )?;

        Ok(anchor_dto(&output.snapshot.project, anchor))
    }

    pub fn revoke(
        &self,
        project_id: &str,
        anchor_id: &str,
        reason: Option<String>,
    ) -> Result<ScoringAnchorDto, AppError> {
        let correlation_id = Uuid::new_v4().to_string();
        let reason = reason
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let output = self.project_store.mutate(
            project_id,
            MutationOptions {
                expected_revision: None,
                expected_fingerprint: None,
                operation: ANCHOR_REVOKE_OPERATION.to_string(),
                correlation_id: correlation_id.clone(),
            },
            |project, _context| {
                let anchor = project
                    .scoring_anchors
                    .iter_mut()
                    .find(|anchor| anchor.id == anchor_id)
                    .ok_or_else(|| {
                        anchor_error(
                            AppErrorCode::ScoringAnchorNotFound,
                            "Anchor kaydı bulunamadı.",
                            Some(format!("anchor_id={anchor_id}")),
                            "Güncel notlandırma görünümünü yenileyip tekrar deneyin.",
                        )
                    })?;
                if anchor.status == ScoringAnchorStatus::Revoked {
                    return Err(anchor_error(
                        AppErrorCode::ScoringAnchorAlreadyRevoked,
                        "Bu anchorın statüsü zaten kaldırılmış.",
                        Some(format!("anchor_id={anchor_id}")),
                        "Güncel anchor listesini kontrol edin.",
                    ));
                }
                let now = chrono::Utc::now().to_rfc3339();
                anchor.status = ScoringAnchorStatus::Revoked;
                anchor.revoked_at = Some(now.clone());
                anchor.revoked_reason = reason.clone();
                anchor.actions.push(ScoringAnchorAction {
                    action: ScoringAnchorActionKind::Revoked,
                    actor_kind: "teacher".to_string(),
                    occurred_at: now,
                    reason: reason.clone(),
                });
                Ok(anchor.clone())
            },
        )?;

        let anchor = output.result;
        self.append_audit(
            &output.snapshot.project,
            &correlation_id,
            ANCHOR_REVOKE_OPERATION,
            &anchor,
            reason.as_deref(),
            json!({
                "anchorVersion": anchor.version,
                "sourceRecordId": anchor.source_record_id,
                "reason": reason,
            }),
        )?;

        Ok(anchor_dto(&output.snapshot.project, anchor))
    }

    fn append_audit(
        &self,
        project: &Project,
        correlation_id: &str,
        operation: &str,
        anchor: &ScoringAnchor,
        reason: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<(), AppError> {
        let summary = match operation {
            ANCHOR_CREATE_OPERATION => "Öğretmen onaylı karar anchor olarak kaydedildi.",
            ANCHOR_REVOKE_OPERATION => "Anchor statüsü öğretmen tarafından kaldırıldı.",
            _ => "Scoring anchor işlemi kaydedildi.",
        };
        let safe_metadata = json!({
            "anchorId": anchor.id,
            "anchorVersion": anchor.version,
            "sourceRecordId": anchor.source_record_id,
            "reason": reason,
            "details": metadata,
        });
        self.audit_service.append_transactionally(
            Path::new(&project.root_path),
            AuditEntryInput::new(operation, summary)
                .project(&project.id)
                .entity("scoring_anchor", &anchor.id)
                .correlation(correlation_id)
                .metadata(safe_metadata),
            project.storage_revision.checked_sub(1),
            Some(project.storage_revision),
        )?;
        Ok(())
    }
}

fn build_anchor(project: &Project, source_record_id: &str) -> Result<ScoringAnchor, AppError> {
    let (record, question, ocr_record) = validate_anchor_source(project, source_record_id)?;
    let final_score = scoring_record_effective_score(&record).ok_or_else(|| {
        anchor_error(
            AppErrorCode::ScoringAnchorNotEligible,
            "Final puan bulunmayan karar anchor olamaz.",
            Some(format!("source_record_id={source_record_id}")),
            "Öğretmen puanını kaydedip kararı yeniden onaylayın.",
        )
    })?;
    let now = chrono::Utc::now().to_rfc3339();
    let next_version = project
        .scoring_anchors
        .iter()
        .filter(|anchor| anchor.source_record_id == source_record_id)
        .filter_map(|anchor| anchor.version.strip_prefix('v'))
        .filter_map(|value| value.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let anchor = ScoringAnchor {
        id: Uuid::new_v4().to_string(),
        version: format!("v{next_version}"),
        source_record_id: record.id.clone(),
        question_id: question.id.clone(),
        question_number: question.number,
        qep_fingerprint: scoring_package_hash(project),
        question_text_hash: scoring_question_text_hash(project),
        rubric_hash: scoring_rubric_hash(project),
        policy_version: record.policy_version.clone(),
        scoring_fingerprint: record.scoring_fingerprint.clone(),
        calibration_version: SCORING_ANCHOR_CALIBRATION_VERSION.to_string(),
        final_score,
        max_score: record.max_score,
        evidence: ScoringAnchorEvidence {
            answer_normalized_hash: record.answer_normalized_hash.clone(),
            answer_raw_hash: record.answer_raw_hash.clone(),
            ocr_record_hash: record.ocr_record_hash.clone(),
            awarded_score: final_score,
            max_score: record.max_score,
            rationale: record.rationale.clone(),
            criterion_scores: record.criterion_scores.clone(),
            teacher_notes: record.teacher_notes.clone(),
        },
        status: ScoringAnchorStatus::Active,
        actions: vec![ScoringAnchorAction {
            action: ScoringAnchorActionKind::Created,
            actor_kind: "teacher".to_string(),
            occurred_at: now.clone(),
            reason: None,
        }],
        created_at: now,
        revoked_at: None,
        revoked_reason: None,
    };

    // Keep the source record and OCR validation in the creation path even
    // though the evidence stores hashes and criterion evidence only. This
    // prevents a placeholder/model-only proposal from becoming an anchor.
    let _ = ocr_record;
    Ok(anchor)
}

fn validate_anchor_source(
    project: &Project,
    source_record_id: &str,
) -> Result<
    (
        crate::domain::scoring::ScoringRecord,
        Question,
        StudentAnswerOcrRecord,
    ),
    AppError,
> {
    let freeze_is_current = project
        .exam_package_freeze
        .as_ref()
        .is_some_and(|freeze| freeze.freeze_status == ExamPackageFreezeStatus::Frozen);
    if !freeze_is_current {
        return Err(anchor_error(
            AppErrorCode::QepNotFrozen,
            "Sınav paketi dondurulmadan anchor oluşturulamaz.",
            Some("exam_package_freeze!=frozen".to_string()),
            "QEP hazırlığını tamamlayıp sınav paketini dondurun.",
        ));
    }

    let record = project
        .scoring_records
        .iter()
        .find(|record| record.id == source_record_id)
        .cloned()
        .ok_or_else(|| {
            anchor_error(
                AppErrorCode::ScoringAnchorNotFound,
                "Anchor yapılacak notlandırma kaydı bulunamadı.",
                Some(format!("source_record_id={source_record_id}")),
                "Notlandırma sonuçlarını yenileyip tekrar deneyin.",
            )
        })?;
    if !scoring_record_is_final(&record) {
        return Err(anchor_error(
            AppErrorCode::ScoringAnchorNotEligible,
            "Yalnız öğretmen tarafından onaylanmış final kararlar anchor olabilir.",
            Some(format!(
                "decision_state={:?}; review_status={:?}",
                record.decision_state, record.teacher_review_status
            )),
            "Kaydı öğretmen olarak inceleyip final puanı onaylayın.",
        ));
    }

    let current_package_hash = scoring_package_hash(project);
    if record.package_hash != current_package_hash {
        return Err(anchor_error(
            AppErrorCode::ScoringAnchorNotEligible,
            "Notlandırma kararı güncel sınav paketiyle eşleşmiyor.",
            Some(format!(
                "record_package_hash={}; current_package_hash={current_package_hash}",
                record.package_hash
            )),
            "Sınav paketini kontrol edip notlandırmayı güncel verilerle yeniden çalıştırın.",
        ));
    }

    let question = project
        .questions
        .iter()
        .find(|question| question.id == record.question_id)
        .cloned()
        .ok_or_else(|| {
            anchor_error(
                AppErrorCode::ScoringAnchorNotEligible,
                "Anchor kararı için soru bağlamı bulunamadı.",
                Some(format!("question_id={}", record.question_id)),
                "Soru ve rubrik hazırlığını kontrol edin.",
            )
        })?;
    let expected_policy = current_policy_version(&question);
    if record.policy_version != expected_policy {
        return Err(anchor_error(
            AppErrorCode::ScoringAnchorNotEligible,
            "Notlandırma politikası güncel değil; bu karar anchor olamaz.",
            Some(format!(
                "record_policy={}; current_policy={expected_policy}",
                record.policy_version
            )),
            "Notlandırmayı güncel politika ile yeniden çalıştırıp kararı onaylayın.",
        ));
    }

    let ocr_record = project
        .student_answer_ocr_records
        .iter()
        .find(|ocr| {
            ocr.id == record.ocr_record_hash
                || (ocr.submission_id == record.submission_id
                    && ocr.question_id == record.question_id)
        })
        .cloned()
        .ok_or_else(|| {
            anchor_error(
                AppErrorCode::ScoringAnchorNotEligible,
                "Final kararın onaylı OCR kaydı bulunamadı.",
                Some(format!("ocr_record_hash={}", record.ocr_record_hash)),
                "Öğrenci cevabını ve OCR onayını yeniden kontrol edin.",
            )
        })?;
    if ocr_record.status != StudentAnswerOcrStatus::TeacherApproved || ocr_record.needs_review {
        return Err(anchor_error(
            AppErrorCode::ScoringAnchorNotEligible,
            "Onaylı ve kontrol gerektirmeyen OCR kaydı olmadan anchor oluşturulamaz.",
            Some(format!(
                "ocr_status={:?}; needs_review={}",
                ocr_record.status, ocr_record.needs_review
            )),
            "Öğrenci cevabını öğretmen olarak kontrol edip OCR kaydını onaylayın.",
        ));
    }
    if ocr_answer_is_placeholder(&ocr_record) {
        return Err(anchor_error(
            AppErrorCode::ScoringAnchorNotEligible,
            "Placeholder veya boş cevap anchor olarak kaydedilemez.",
            Some(format!("ocr_record_id={}", ocr_record.id)),
            "Gerçek öğrenci cevabını doğrulayıp tekrar deneyin.",
        ));
    }

    Ok((record, question, ocr_record))
}

fn anchor_dto(project: &Project, anchor: ScoringAnchor) -> ScoringAnchorDto {
    let (eligibility, eligibility_reasons) = anchor_eligibility(project, &anchor);
    ScoringAnchorDto {
        anchor,
        eligibility,
        eligibility_reasons,
    }
}

fn anchor_eligibility(
    project: &Project,
    anchor: &ScoringAnchor,
) -> (ScoringAnchorEligibility, Vec<String>) {
    if anchor.status == ScoringAnchorStatus::Revoked {
        return (
            ScoringAnchorEligibility::Revoked,
            vec!["Anchor statüsü kaldırıldı.".to_string()],
        );
    }

    let mut stale_reasons = Vec::new();
    let mut ineligible_reasons = Vec::new();
    let current_package_hash = scoring_package_hash(project);
    if project.exam_package_freeze.as_ref().map_or(true, |freeze| {
        freeze.freeze_status != ExamPackageFreezeStatus::Frozen
    }) {
        stale_reasons.push("Sınav paketi artık dondurulmuş durumda değil.".to_string());
    }
    if anchor.qep_fingerprint != current_package_hash {
        stale_reasons.push("Sınav paketi veya öğrenci cevap bağlamı değişti.".to_string());
    }
    if anchor.question_text_hash != scoring_question_text_hash(project) {
        stale_reasons.push("Soru metni değişti.".to_string());
    }
    if anchor.rubric_hash != scoring_rubric_hash(project) {
        stale_reasons.push("Rubrik değişti.".to_string());
    }
    if anchor.calibration_version != SCORING_ANCHOR_CALIBRATION_VERSION {
        stale_reasons.push("Kalibrasyon politikası sürümü değişti.".to_string());
    }

    let record = project
        .scoring_records
        .iter()
        .find(|record| record.id == anchor.source_record_id);
    let question = project
        .questions
        .iter()
        .find(|question| question.id == anchor.question_id);
    match (record, question) {
        (Some(record), Some(question)) => {
            if !scoring_record_is_final(record) {
                ineligible_reasons
                    .push("Kaynak karar artık öğretmen onaylı final değil.".to_string());
            }
            if record.package_hash != anchor.qep_fingerprint
                || record.question_text_hash != anchor.question_text_hash
                || record.rubric_hash != anchor.rubric_hash
            {
                stale_reasons.push(
                    "Kaynak kararın QEP, soru metni veya rubrik fingerprint’i değişti.".to_string(),
                );
            }
            if record.policy_version != current_policy_version(question) {
                stale_reasons.push("Notlandırma politikası sürümü değişti.".to_string());
            }
            if !record.scoring_fingerprint.is_empty()
                && !anchor.scoring_fingerprint.is_empty()
                && record.scoring_fingerprint != anchor.scoring_fingerprint
            {
                stale_reasons.push("Notlandırma fingerprint’i değişti.".to_string());
            }
            if record.answer_normalized_hash != anchor.evidence.answer_normalized_hash
                || record.answer_raw_hash != anchor.evidence.answer_raw_hash
                || record.ocr_record_hash != anchor.evidence.ocr_record_hash
            {
                stale_reasons
                    .push("Kaynak öğrenci cevabı veya OCR fingerprint’i değişti.".to_string());
            }
            if scoring_record_effective_score(record) != Some(anchor.final_score) {
                ineligible_reasons
                    .push("Kaynak final puanı anchor kanıtıyla artık eşleşmiyor.".to_string());
            }
            match project.student_answer_ocr_records.iter().find(|ocr| {
                ocr.id == record.ocr_record_hash
                    || (ocr.submission_id == record.submission_id
                        && ocr.question_id == record.question_id)
            }) {
                Some(ocr)
                    if ocr.status == StudentAnswerOcrStatus::TeacherApproved
                        && !ocr.needs_review =>
                {
                    if ocr_answer_is_placeholder(ocr) {
                        ineligible_reasons
                            .push("Kaynak cevap boş veya placeholder içeriyor.".to_string());
                    }
                }
                Some(_) => ineligible_reasons
                    .push("Kaynak OCR kaydı artık öğretmen onaylı değil.".to_string()),
                None => ineligible_reasons.push("Kaynak OCR kaydı bulunamadı.".to_string()),
            }
        }
        (None, _) => ineligible_reasons.push("Kaynak notlandırma kaydı bulunamadı.".to_string()),
        (_, None) => ineligible_reasons.push("Kaynak soru bulunamadı.".to_string()),
    }

    if !stale_reasons.is_empty() {
        (ScoringAnchorEligibility::Stale, stale_reasons)
    } else if !ineligible_reasons.is_empty() {
        (ScoringAnchorEligibility::Ineligible, ineligible_reasons)
    } else {
        (ScoringAnchorEligibility::Eligible, vec![])
    }
}

fn current_policy_version(question: &Question) -> &'static str {
    if DeterministicScoringService::supports(&question.answer_type) {
        DETERMINISTIC_SCORING_POLICY_VERSION
    } else {
        SEMANTIC_SCORING_POLICY_VERSION
    }
}

fn ocr_answer_is_placeholder(record: &StudentAnswerOcrRecord) -> bool {
    let effective = record
        .teacher_corrected_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| record.answer_text.trim());
    if let Some(structured_answer) = record.structured_answer.as_ref() {
        if let Ok(value) = serde_json::to_value(structured_answer) {
            if json_contains_placeholder(&value) {
                return true;
            }
            if effective.is_empty() {
                return !json_contains_content(&value);
            }
        }
    }
    effective.is_empty() || crate::domain::rubric::is_placeholder_text(effective)
}

fn json_contains_placeholder(value: &Value) -> bool {
    match value {
        Value::String(text) => crate::domain::rubric::is_placeholder_text(text),
        Value::Array(items) => items.iter().any(json_contains_placeholder),
        Value::Object(fields) => fields.values().any(json_contains_placeholder),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn json_contains_content(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => items.iter().any(json_contains_content),
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            !matches!(key.as_str(), "type" | "variant") && json_contains_content(value)
        }),
        Value::Null => false,
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn anchor_error(
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

    fn test_anchor(source_record_id: &str) -> ScoringAnchor {
        ScoringAnchor {
            id: "anchor-1".to_string(),
            version: "v1".to_string(),
            source_record_id: source_record_id.to_string(),
            question_id: "question-1".to_string(),
            question_number: 1,
            qep_fingerprint: "qep-v1".to_string(),
            question_text_hash: "question-hash".to_string(),
            rubric_hash: "rubric-hash".to_string(),
            policy_version: "policy-v1".to_string(),
            scoring_fingerprint: "scoring-v1".to_string(),
            calibration_version: SCORING_ANCHOR_CALIBRATION_VERSION.to_string(),
            final_score: 4.0,
            max_score: 5.0,
            evidence: ScoringAnchorEvidence {
                answer_normalized_hash: "answer-normalized".to_string(),
                answer_raw_hash: "answer-raw".to_string(),
                ocr_record_hash: "ocr-1".to_string(),
                awarded_score: 4.0,
                max_score: 5.0,
                rationale: "Öğretmen gerekçesi".to_string(),
                criterion_scores: vec![],
                teacher_notes: Some("Kontrol edildi".to_string()),
            },
            status: ScoringAnchorStatus::Active,
            actions: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            revoked_at: None,
            revoked_reason: None,
        }
    }

    #[test]
    fn placeholder_answer_is_never_anchor_eligible() {
        let mut record = StudentAnswerOcrRecord {
            answer_text: "Anahtar kavramları girin...".to_string(),
            ..StudentAnswerOcrRecord::default()
        };
        assert!(ocr_answer_is_placeholder(&record));

        record.answer_text = "Gerçek öğrenci cevabı".to_string();
        assert!(!ocr_answer_is_placeholder(&record));
    }

    #[test]
    fn structured_answer_can_supply_non_empty_content_when_text_projection_is_empty() {
        let record = StudentAnswerOcrRecord {
            structured_answer: Some(
                crate::domain::structured_answer::StructuredAnswer::OpenText {
                    text: "Gerçek cevap".to_string(),
                },
            ),
            ..StudentAnswerOcrRecord::default()
        };
        assert!(!ocr_answer_is_placeholder(&record));
    }

    #[test]
    fn structured_placeholder_or_empty_answer_is_not_anchor_eligible() {
        let mut record = StudentAnswerOcrRecord {
            structured_answer: Some(
                crate::domain::structured_answer::StructuredAnswer::OpenText {
                    text: "Anahtar kavramları girin...".to_string(),
                },
            ),
            ..StudentAnswerOcrRecord::default()
        };
        assert!(ocr_answer_is_placeholder(&record));

        record.structured_answer = Some(
            crate::domain::structured_answer::StructuredAnswer::OpenText {
                text: String::new(),
            },
        );
        assert!(ocr_answer_is_placeholder(&record));
    }

    #[test]
    fn anchor_creation_preserves_the_qep_frozen_gate() {
        let root = std::env::temp_dir().join(format!("rubrika-anchor-gate-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Anchor gate".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("project for anchor gate");
        let service = ScoringAnchorService::new(store.clone(), Arc::new(AuditService::new()));

        let error = service
            .create(&project.id, "record-missing")
            .expect_err("anchor creation must require frozen QEP");
        assert_eq!(error.code, AppErrorCode::QepNotFrozen);
        assert!(store
            .get_project_snapshot(project.id.clone())
            .expect("project remains readable")
            .scoring_anchors
            .is_empty());
    }

    #[test]
    fn revoke_is_atomic_and_is_written_to_the_audit_log() {
        let root = std::env::temp_dir().join(format!("rubrika-anchor-revoke-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Anchor revoke".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("project for anchor revoke");
        let anchor = test_anchor("record-1");
        store
            .mutate(
                &project.id,
                MutationOptions::new("seed_test_anchor"),
                |project, _context| {
                    project.scoring_anchors.push(anchor.clone());
                    Ok::<_, AppError>(())
                },
            )
            .expect("seed anchor");

        let service = ScoringAnchorService::new(store.clone(), Arc::new(AuditService::new()));
        let revoked = service
            .revoke(
                &project.id,
                &anchor.id,
                Some("Rubrik güncellendi".to_string()),
            )
            .expect("revoke anchor");

        assert_eq!(revoked.anchor.status, ScoringAnchorStatus::Revoked);
        assert_eq!(revoked.eligibility, ScoringAnchorEligibility::Revoked);
        assert_eq!(revoked.anchor.actions.len(), 1);
        assert_eq!(
            revoked.anchor.actions[0].reason.as_deref(),
            Some("Rubrik güncellendi")
        );
        let saved = store
            .get_project_snapshot(project.id.clone())
            .expect("saved project");
        assert_eq!(
            saved.scoring_anchors[0].status,
            ScoringAnchorStatus::Revoked
        );
        let audit =
            std::fs::read_to_string(AuditService::audit_path(Path::new(&project.root_path)))
                .expect("anchor audit log");
        assert!(audit.contains("revoke_scoring_anchor"));
        assert!(audit.contains("anchor-1"));
    }

    #[test]
    fn active_anchor_reports_stale_after_qep_is_not_frozen() {
        let root = std::env::temp_dir().join(format!("rubrika-anchor-stale-{}", Uuid::new_v4()));
        let store = ProjectStore::new();
        let project = store
            .create_project(
                "Anchor stale".to_string(),
                root.to_string_lossy().to_string(),
            )
            .expect("project for anchor stale");

        let (eligibility, reasons) = anchor_eligibility(&project, &test_anchor("record-1"));

        assert_eq!(eligibility, ScoringAnchorEligibility::Stale);
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("dondurulmuş durumda değil")));
    }
}

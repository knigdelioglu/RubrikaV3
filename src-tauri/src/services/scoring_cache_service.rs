use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::SemanticCriterionDecision;
use crate::domain::question::AnswerType;
use crate::domain::scoring::{
    ScoringCacheProvenance, ScoringCriterionScore, ScoringFingerprint, ScoringRecord,
};
use crate::services::text_normalization::normalize_for_comparison;

pub const CANDIDATE_CACHE_SCHEMA_VERSION: &str = "scoring_candidate_cache_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringCandidateProposal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awarded_score: Option<f32>,
    #[serde(default)]
    pub criterion_scores: Vec<ScoringCriterionScore>,
    #[serde(default)]
    pub semantic_decisions: Vec<SemanticCriterionDecision>,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub review_reasons: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub raw_model_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoringCandidateArtifact {
    pub schema_version: String,
    pub fingerprint: ScoringFingerprint,
    pub proposal: ScoringCandidateProposal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoringCacheHit {
    pub proposal: ScoringCandidateProposal,
    pub provenance: ScoringCacheProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExactDuplicateInput {
    pub question_id: String,
    pub qep_fingerprint: String,
    pub rubric_hash: String,
    pub policy_version: String,
    pub ocr_generation: String,
    pub answer_type: AnswerType,
    pub answer_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnswerHashes {
    pub normalized_hash: String,
    pub raw_hash: String,
    pub match_key: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScoringCacheService;

impl ScoringCacheService {
    pub fn new() -> Self {
        Self
    }

    pub fn lookup_candidate(
        &self,
        project_root: &Path,
        fingerprint: &ScoringFingerprint,
    ) -> Result<Option<ScoringCacheHit>, AppError> {
        let path = self.artifact_path(project_root, fingerprint);
        if !path.exists() {
            return Ok(None);
        }
        let bytes =
            std::fs::read(&path).map_err(|error| cache_error("Aday cache okunamadı.", error))?;
        let artifact = match serde_json::from_slice::<ScoringCandidateArtifact>(&bytes) {
            Ok(artifact) => artifact,
            Err(_) => return Ok(None),
        };
        if artifact.schema_version != CANDIDATE_CACHE_SCHEMA_VERSION
            || artifact.fingerprint.value != fingerprint.value
            || serde_json::to_vec(&artifact.fingerprint.components).ok()
                != serde_json::to_vec(&fingerprint.components).ok()
        {
            return Ok(None);
        }
        Ok(Some(ScoringCacheHit {
            proposal: artifact.proposal,
            provenance: ScoringCacheProvenance {
                fingerprint: fingerprint.value.clone(),
                artifact_schema_version: CANDIDATE_CACHE_SCHEMA_VERSION.to_string(),
                cache_hit: true,
                source: "candidate_cache".to_string(),
                artifact_path: Some(path.to_string_lossy().to_string()),
            },
        }))
    }

    pub fn write_candidate(
        &self,
        project_root: &Path,
        fingerprint: &ScoringFingerprint,
        proposal: ScoringCandidateProposal,
    ) -> Result<ScoringCacheProvenance, AppError> {
        let directory = project_root.join("cache").join("scoring_candidates");
        std::fs::create_dir_all(&directory)
            .map_err(|error| cache_error("Aday cache klasörü oluşturulamadı.", error))?;
        let path = self.artifact_path(project_root, fingerprint);
        let temporary = directory.join(format!(
            ".{}.{}.tmp",
            fingerprint.value,
            uuid::Uuid::new_v4()
        ));
        let artifact = ScoringCandidateArtifact {
            schema_version: CANDIDATE_CACHE_SCHEMA_VERSION.to_string(),
            fingerprint: fingerprint.clone(),
            proposal,
        };
        let bytes = serde_json::to_vec_pretty(&artifact)
            .map_err(|error| cache_error("Aday cache hazırlanamadı.", error))?;
        std::fs::write(&temporary, bytes)
            .map_err(|error| cache_error("Aday cache geçici dosyaya yazılamadı.", error))?;
        if let Ok(file) = std::fs::OpenOptions::new().read(true).open(&temporary) {
            let _ = file.sync_all();
        }
        std::fs::rename(&temporary, &path)
            .map_err(|error| cache_error("Aday cache atomik olarak kaydedilemedi.", error))?;
        Ok(ScoringCacheProvenance {
            fingerprint: fingerprint.value.clone(),
            artifact_schema_version: CANDIDATE_CACHE_SCHEMA_VERSION.to_string(),
            cache_hit: false,
            source: "candidate_cache".to_string(),
            artifact_path: Some(path.to_string_lossy().to_string()),
        })
    }

    pub fn artifact_path(&self, project_root: &Path, fingerprint: &ScoringFingerprint) -> PathBuf {
        project_root
            .join("cache")
            .join("scoring_candidates")
            .join(format!("{}.json", fingerprint.value))
    }

    pub fn answer_hashes(&self, answer_type: &AnswerType, answer_text: &str) -> AnswerHashes {
        answer_hashes(answer_type, answer_text)
    }

    pub fn exact_duplicate_source<'a>(
        &self,
        records: &'a [ScoringRecord],
        input: &ExactDuplicateInput,
    ) -> Option<&'a ScoringRecord> {
        let hashes = answer_hashes(&input.answer_type, &input.answer_text);
        records.iter().find(|record| {
            record.question_id == input.question_id
                && record.package_hash == input.qep_fingerprint
                && record.rubric_hash == input.rubric_hash
                && record.policy_version == input.policy_version
                && record.ocr_generation == input.ocr_generation
                && record.answer_normalized_hash == hashes.normalized_hash
                && record.answer_raw_hash == hashes.raw_hash
                && record.scoring_applied
                && record.awarded_score.is_some()
                && !record.needs_review
                && matches!(
                    record.decision_state,
                    crate::domain::scoring::ScoringDecisionState::DeterministicAccepted
                        | crate::domain::scoring::ScoringDecisionState::TeacherApproved
                )
                && !matches!(
                    record.teacher_review_status,
                    crate::domain::scoring::ScoringReviewStatus::Invalidated
                )
        })
    }
}

pub fn answer_hashes(answer_type: &AnswerType, answer_text: &str) -> AnswerHashes {
    let normalized = normalize_for_comparison(answer_text);
    let normalized_for_hash = match answer_type {
        AnswerType::Numeric => format!(
            "numeric::{normalized}::{}",
            numeric_punctuation(answer_text)
        ),
        _ => normalized.clone(),
    };
    let normalized_hash = sha256_hex(normalized_for_hash.as_bytes());
    let raw_hash = sha256_hex(answer_text.as_bytes());
    let match_key =
        sha256_hex(format!("{:?}|{}|{}", answer_type, normalized_hash, raw_hash).as_bytes());
    AnswerHashes {
        normalized_hash,
        raw_hash,
        match_key,
    }
}

fn numeric_punctuation(answer_text: &str) -> String {
    answer_text
        .chars()
        .filter(|character| {
            character.is_ascii_digit() || matches!(character, ',' | '.' | '-' | '+')
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn cache_error<T: std::fmt::Display>(message: &str, error: T) -> AppError {
    AppError {
        code: AppErrorCode::ProjectSaveFailed,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some("Aday cache temizlenip işlemi yeniden deneyin.".to_string()),
        technical_details: Some(error.to_string()),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::SamplingParameters;
    use crate::domain::scoring::{ScoringFingerprintComponents, ScoringReviewStatus};

    fn fingerprint(seed: &str) -> ScoringFingerprint {
        ScoringFingerprint::from_components(ScoringFingerprintComponents {
            qep_fingerprint: format!("qep-{seed}"),
            question_id: "q1".to_string(),
            answer_hash: "answer".to_string(),
            ocr_generation: "ocr-1".to_string(),
            prompt_version: "prompt-v1".to_string(),
            schema_version: "schema-v1".to_string(),
            policy_version: "policy-v1".to_string(),
            policy_fingerprint: "policy-fp".to_string(),
            model_file_fingerprint: "model-file".to_string(),
            runtime_fingerprint: "runtime".to_string(),
            sampling_parameters: SamplingParameters {
                temperature: 0.0,
                top_k: None,
                top_p: None,
                seed: Some(1),
                max_tokens: 128,
            },
            calibration_version: "none".to_string(),
            anchor_version: "none".to_string(),
        })
    }

    #[test]
    fn fingerprint_round_trip_requires_exact_components() {
        let root =
            std::env::temp_dir().join(format!("rubrika-score-cache-{}", uuid::Uuid::new_v4()));
        let service = ScoringCacheService::new();
        let first = fingerprint("same");
        let proposal = ScoringCandidateProposal {
            awarded_score: Some(3.0),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "aday".to_string(),
            confidence: 0.8,
            needs_review: true,
            review_reasons: vec!["consistency_review".to_string()],
            warnings: vec![],
            raw_model_output: "{}".to_string(),
        };
        service
            .write_candidate(&root, &first, proposal.clone())
            .unwrap();
        let hit = service.lookup_candidate(&root, &first).unwrap().unwrap();
        assert_eq!(hit.proposal, proposal);
        assert!(hit.provenance.cache_hit);
        assert!(service
            .lookup_candidate(&root, &fingerprint("different"))
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_candidate_is_a_cache_miss_and_not_a_score() {
        let root =
            std::env::temp_dir().join(format!("rubrika-score-cache-{}", uuid::Uuid::new_v4()));
        let service = ScoringCacheService::new();
        let fingerprint = fingerprint("corrupt");
        let path = service.artifact_path(&root, &fingerprint);
        std::fs::create_dir_all(path.parent().expect("cache parent")).unwrap();
        std::fs::write(path, b"broken").unwrap();
        assert!(service
            .lookup_candidate(&root, &fingerprint)
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn turkish_whitespace_matches_but_numeric_unit_and_negation_remain_in_key() {
        let service = ScoringCacheService::new();
        let first = service.answer_hashes(&AnswerType::GeneralText, " İyi   cevap ");
        let equivalent = service.answer_hashes(&AnswerType::GeneralText, "iyi cevap");
        assert_eq!(first.normalized_hash, equivalent.normalized_hash);
        let kilogram = service.answer_hashes(&AnswerType::Numeric, "1,5 kg");
        let meter = service.answer_hashes(&AnswerType::Numeric, "1,5 m");
        let negated = service.answer_hashes(&AnswerType::GeneralText, "değil");
        let positive = service.answer_hashes(&AnswerType::GeneralText, "doğru");
        assert_ne!(kilogram.normalized_hash, meter.normalized_hash);
        assert_ne!(negated.normalized_hash, positive.normalized_hash);
    }

    #[test]
    fn exact_duplicate_requires_normalized_and_raw_hashes() {
        let service = ScoringCacheService::new();
        let hashes = service.answer_hashes(&AnswerType::GeneralText, "İyi cevap");
        let now = chrono::Utc::now();
        let record = ScoringRecord {
            id: "source".to_string(),
            run_id: "run".to_string(),
            submission_id: "submission".to_string(),
            student_id: "student".to_string(),
            student_display_name: None,
            student_number: None,
            student_class_name: None,
            question_id: "q1".to_string(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(5.0),
            scoring_applied: true,
            decision_state: crate::domain::scoring::ScoringDecisionState::DeterministicAccepted,
            decision_version: "v1".to_string(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: String::new(),
            confidence: 1.0,
            needs_review: false,
            review_reasons: vec![],
            warnings: vec![],
            raw_model_output: String::new(),
            parse_diagnostics: None,
            reconciliation_diagnostics: None,
            execution_diagnostics: None,
            cache_provenance: None,
            reuse_provenance: None,
            consistency_review: None,
            scoring_fingerprint: "fingerprint".to_string(),
            policy_version: "policy".to_string(),
            answer_normalized_hash: hashes.normalized_hash,
            answer_raw_hash: hashes.raw_hash,
            ocr_generation: "ocr".to_string(),
            source_hash: "source".to_string(),
            package_hash: "qep".to_string(),
            ocr_record_hash: "ocr".to_string(),
            question_text_hash: "question".to_string(),
            rubric_hash: "rubric".to_string(),
            teacher_review_status: crate::domain::scoring::ScoringReviewStatus::PendingReview,
            teacher_manual_score: None,
            teacher_reviewed_at: None,
            teacher_notes: None,
            invalidated_at: None,
            invalidation_reason: None,
            created_at: now,
            updated_at: now,
        };
        let exact = ExactDuplicateInput {
            question_id: "q1".to_string(),
            qep_fingerprint: "qep".to_string(),
            rubric_hash: "rubric".to_string(),
            policy_version: "policy".to_string(),
            ocr_generation: "ocr".to_string(),
            answer_type: AnswerType::GeneralText,
            answer_text: "İyi cevap".to_string(),
        };
        assert!(service
            .exact_duplicate_source(&[record.clone()], &exact)
            .is_some());

        let mut changed_raw = record;
        changed_raw.answer_raw_hash = service
            .answer_hashes(&AnswerType::GeneralText, "İyi  cevap")
            .raw_hash;
        assert!(service
            .exact_duplicate_source(&[changed_raw], &exact)
            .is_none());
    }

    #[test]
    fn candidate_hit_keeps_review_state() {
        let proposal = ScoringCandidateProposal {
            awarded_score: Some(0.0),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: String::new(),
            confidence: 0.0,
            needs_review: true,
            review_reasons: vec!["invalid_model_output".to_string()],
            warnings: vec![],
            raw_model_output: "bad".to_string(),
        };
        assert!(proposal.needs_review);
        assert_eq!(
            ScoringReviewStatus::PendingReview,
            ScoringReviewStatus::PendingReview
        );
    }
}

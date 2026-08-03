use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::domain::scoring::{
    ScoringConsistencyReview, ScoringDecisionState, ScoringRecord, ScoringReviewStatus,
};
use crate::services::text_normalization::normalize_for_comparison;

pub const CONSISTENCY_REVIEW_REASON: &str = "consistency_review";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyFinding {
    pub reason_code: String,
    pub teacher_message: String,
    pub cluster_key: String,
    pub conflicting_record_ids: Vec<String>,
    pub blocks_auto_accept: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyCandidate {
    pub record_id: String,
    pub question_id: String,
    pub qep_fingerprint: String,
    pub policy_version: String,
    pub normalized_answer: String,
    pub outcome_signature: String,
    pub score: Option<f32>,
    pub decision_state: ScoringDecisionState,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoringConsistencyService;

impl ScoringConsistencyService {
    pub fn new() -> Self {
        Self
    }

    pub fn review(&self, records: &[ScoringRecord]) -> Vec<ConsistencyFinding> {
        let mut groups: BTreeMap<String, Vec<ConsistencyCandidate>> = BTreeMap::new();
        for record in records {
            if record.answer_normalized_hash.trim().is_empty()
                || matches!(
                    record.teacher_review_status,
                    ScoringReviewStatus::Invalidated
                )
            {
                continue;
            }
            let cluster_key = format!(
                "{}|{}|{}|{}",
                record.question_id,
                record.package_hash,
                record.rubric_hash,
                record.answer_normalized_hash
            );
            groups
                .entry(cluster_key)
                .or_default()
                .push(ConsistencyCandidate {
                    record_id: record.id.clone(),
                    question_id: record.question_id.clone(),
                    qep_fingerprint: record.package_hash.clone(),
                    policy_version: record.policy_version.clone(),
                    normalized_answer: record.answer_normalized_hash.clone(),
                    outcome_signature: outcome_signature(record),
                    score: record.teacher_manual_score.or(record.awarded_score),
                    decision_state: record.decision_state,
                });
        }
        self.findings_from_groups(groups)
    }

    /// Reviews exact and near-exact answer clusters when the current OCR text
    /// is available. The persisted record remains the source of outcome and
    /// decision state; answer text is used only to form a deterministic
    /// comparison cluster.
    pub fn review_with_answers(
        &self,
        records: &[ScoringRecord],
        answers: &HashMap<(String, String), String>,
    ) -> Vec<ConsistencyFinding> {
        let mut groups: BTreeMap<String, Vec<ConsistencyCandidate>> = BTreeMap::new();
        for record in records {
            if matches!(
                record.teacher_review_status,
                ScoringReviewStatus::Invalidated
            ) {
                continue;
            }
            let fallback = record.answer_normalized_hash.clone();
            let normalized_answer = answers
                .get(&(record.submission_id.clone(), record.question_id.clone()))
                .map(|answer| normalize_for_comparison(answer))
                .filter(|answer| !answer.is_empty())
                .unwrap_or(fallback);
            if normalized_answer.is_empty() {
                continue;
            }
            let group_key = format!(
                "{}|{}|{}|{}",
                record.question_id, record.package_hash, record.rubric_hash, record.policy_version
            );
            groups
                .entry(group_key)
                .or_default()
                .push(candidate_from_record(record, normalized_answer));
        }

        groups
            .into_iter()
            .flat_map(|(base_key, candidates)| {
                let clusters = near_exact_clusters(candidates);
                clusters.into_iter().enumerate().filter_map(move |(index, cluster)| {
                    let signatures = cluster
                        .iter()
                        .map(outcome_signature_candidate)
                        .collect::<HashSet<_>>();
                    (cluster.len() > 1 && signatures.len() > 1).then(|| ConsistencyFinding {
                        reason_code: CONSISTENCY_REVIEW_REASON.to_string(),
                        teacher_message: "Aynı veya çok benzer cevaplarda farklı puan ya da seviye görüldü; karşılaştırıp öğretmen onayı verin.".to_string(),
                        cluster_key: format!("{base_key}|near-cluster-{index}"),
                        conflicting_record_ids: cluster
                            .into_iter()
                            .map(|candidate| candidate.record_id)
                            .collect(),
                        blocks_auto_accept: true,
                    })
                })
            })
            .collect()
    }

    /// Applies only the review flag and explanation. It never copies a score
    /// from one record to another and never edits teacher-approved values.
    pub fn apply(&self, records: &mut [ScoringRecord]) -> Vec<ConsistencyFinding> {
        let findings = self.review(records);
        self.apply_findings(records, findings)
    }

    pub fn apply_with_answers(
        &self,
        records: &mut [ScoringRecord],
        answers: &HashMap<(String, String), String>,
    ) -> Vec<ConsistencyFinding> {
        let findings = self.review_with_answers(records, answers);
        self.apply_findings(records, findings)
    }

    fn apply_findings(
        &self,
        records: &mut [ScoringRecord],
        findings: Vec<ConsistencyFinding>,
    ) -> Vec<ConsistencyFinding> {
        for finding in &findings {
            for record in records.iter_mut().filter(|record| {
                finding
                    .conflicting_record_ids
                    .iter()
                    .any(|id| id == &record.id)
            }) {
                record.needs_review = true;
                if !record
                    .review_reasons
                    .iter()
                    .any(|reason| reason == CONSISTENCY_REVIEW_REASON)
                {
                    record
                        .review_reasons
                        .push(CONSISTENCY_REVIEW_REASON.to_string());
                }
                record.warnings.push(CONSISTENCY_REVIEW_REASON.to_string());
                record.consistency_review = Some(ScoringConsistencyReview {
                    reason_code: CONSISTENCY_REVIEW_REASON.to_string(),
                    teacher_message: finding.teacher_message.clone(),
                    cluster_key: finding.cluster_key.clone(),
                    conflicting_record_ids: finding.conflicting_record_ids.clone(),
                });
                if matches!(
                    record.decision_state,
                    ScoringDecisionState::AutoAccepted
                        | ScoringDecisionState::DeterministicAccepted
                ) && !matches!(
                    record.teacher_review_status,
                    ScoringReviewStatus::Approved | ScoringReviewStatus::Edited
                ) {
                    record.decision_state = ScoringDecisionState::Provisional;
                }
                record.review_reasons.sort();
                record.review_reasons.dedup();
                record.warnings.sort();
                record.warnings.dedup();
            }
        }
        findings
    }

    fn findings_from_groups(
        &self,
        groups: BTreeMap<String, Vec<ConsistencyCandidate>>,
    ) -> Vec<ConsistencyFinding> {
        groups
            .into_iter()
            .filter_map(|(cluster_key, candidates)| {
                let signatures = candidates
                    .iter()
                        .map(outcome_signature_candidate)
                    .collect::<std::collections::BTreeSet<_>>();
                (candidates.len() > 1 && signatures.len() > 1).then(|| ConsistencyFinding {
                    reason_code: CONSISTENCY_REVIEW_REASON.to_string(),
                    teacher_message: "Aynı cevap kümesinde farklı puan veya seviye görüldü; karşılaştırıp öğretmen onayı verin.".to_string(),
                    cluster_key,
                    conflicting_record_ids: candidates
                        .into_iter()
                        .map(|candidate| candidate.record_id)
                        .collect(),
                    blocks_auto_accept: true,
                })
            })
            .collect()
    }
}

impl Default for ScoringConsistencyService {
    fn default() -> Self {
        Self::new()
    }
}

fn outcome_signature(record: &ScoringRecord) -> String {
    let mut levels = record
        .semantic_decisions
        .iter()
        .map(|decision| format!("{}:{}", decision.criterion_id, decision.level_id))
        .collect::<Vec<_>>();
    levels.sort();
    format!(
        "levels={}|score={:?}",
        levels.join(","),
        record.teacher_manual_score.or(record.awarded_score)
    )
}

fn candidate_from_record(
    record: &ScoringRecord,
    normalized_answer: String,
) -> ConsistencyCandidate {
    ConsistencyCandidate {
        record_id: record.id.clone(),
        question_id: record.question_id.clone(),
        qep_fingerprint: record.package_hash.clone(),
        policy_version: record.policy_version.clone(),
        normalized_answer,
        outcome_signature: outcome_signature(record),
        score: record.teacher_manual_score.or(record.awarded_score),
        decision_state: record.decision_state,
    }
}

fn near_exact_clusters(candidates: Vec<ConsistencyCandidate>) -> Vec<Vec<ConsistencyCandidate>> {
    let mut clusters: Vec<Vec<ConsistencyCandidate>> = Vec::new();
    for candidate in candidates {
        if let Some(cluster) = clusters.iter_mut().find(|cluster| {
            cluster.iter().any(|other| {
                answers_are_near_exact(&candidate.normalized_answer, &other.normalized_answer)
            })
        }) {
            cluster.push(candidate);
        } else {
            clusters.push(vec![candidate]);
        }
    }
    clusters
}

fn answers_are_near_exact(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let left_length = left.chars().count();
    let right_length = right.chars().count();
    if left_length < 8 || right_length < 8 {
        return false;
    }
    let max_length = left_length.max(right_length);
    let distance = levenshtein_distance(left, right);
    distance <= 2 || (distance as f32 / max_length as f32) <= 0.08
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != *right_char);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current.push(substitution.min(insertion).min(deletion));
        }
        previous = current;
    }
    previous[right_chars.len()]
}

fn outcome_signature_candidate(candidate: &ConsistencyCandidate) -> String {
    format!(
        "{}|{:?}|{:?}",
        candidate.outcome_signature, candidate.score, candidate.decision_state
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::scoring::{ScoringRecord, ScoringReviewStatus};

    fn record(id: &str, score: f32, state: ScoringDecisionState) -> ScoringRecord {
        let now = chrono::Utc::now();
        ScoringRecord {
            id: id.to_string(),
            run_id: "run".to_string(),
            submission_id: id.to_string(),
            student_id: id.to_string(),
            student_display_name: None,
            student_number: None,
            student_class_name: None,
            question_id: "q1".to_string(),
            question_number: 1,
            max_score: 10.0,
            awarded_score: Some(score),
            scoring_applied: true,
            decision_state: state,
            decision_version: "v1".to_string(),
            criterion_scores: vec![],
            semantic_decisions: vec![],
            rationale: "aday".to_string(),
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
            scoring_fingerprint: String::new(),
            policy_version: "policy".to_string(),
            answer_normalized_hash: "same-answer".to_string(),
            answer_raw_hash: id.to_string(),
            ocr_generation: "ocr".to_string(),
            source_hash: "source".to_string(),
            package_hash: "qep".to_string(),
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

    #[test]
    fn different_scores_in_exact_cluster_require_review_without_copying() {
        let mut records = vec![
            record("a", 7.0, ScoringDecisionState::AutoAccepted),
            record("b", 5.0, ScoringDecisionState::AutoAccepted),
        ];
        let findings = ScoringConsistencyService::new().apply(&mut records);
        assert_eq!(findings.len(), 1);
        assert!(records.iter().all(|record| record.needs_review));
        assert!(records.iter().all(|record| record
            .review_reasons
            .contains(&CONSISTENCY_REVIEW_REASON.to_string())));
        assert_eq!(records[0].awarded_score, Some(7.0));
        assert_eq!(records[1].awarded_score, Some(5.0));
        assert_eq!(records[0].decision_state, ScoringDecisionState::Provisional);
    }

    #[test]
    fn same_score_does_not_create_consistency_review() {
        let records = vec![
            record("a", 7.0, ScoringDecisionState::TeacherApproved),
            record("b", 7.0, ScoringDecisionState::TeacherApproved),
        ];
        assert!(ScoringConsistencyService::new().review(&records).is_empty());
    }

    #[test]
    fn near_exact_answers_require_review_without_score_copying() {
        let mut records = vec![
            record("a", 7.0, ScoringDecisionState::AutoAccepted),
            record("b", 5.0, ScoringDecisionState::AutoAccepted),
        ];
        let answers = HashMap::from([
            (
                ("a".to_string(), "q1".to_string()),
                "Aynı doğru cevap".to_string(),
            ),
            (
                ("b".to_string(), "q1".to_string()),
                "Aynı doğru cevap.".to_string(),
            ),
        ]);

        let findings = ScoringConsistencyService::new().apply_with_answers(&mut records, &answers);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].blocks_auto_accept);
        assert_eq!(records[0].awarded_score, Some(7.0));
        assert_eq!(records[1].awarded_score, Some(5.0));
    }
}

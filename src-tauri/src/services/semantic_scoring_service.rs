use std::collections::HashSet;

use crate::domain::model::{ScoringOutput, SemanticCriterionDecision};
use crate::domain::question::Question;
use crate::domain::scoring::ScoringCriterionScore;

pub const SEMANTIC_SCORING_POLICY_VERSION: &str = "semantic_scoring_policy_v1";

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticScoringEvaluation {
    pub awarded_score: Option<f32>,
    pub criterion_scores: Vec<ScoringCriterionScore>,
    pub semantic_decisions: Vec<SemanticCriterionDecision>,
    pub rationale: String,
    pub confidence: f32,
    pub scoring_applied: bool,
    pub needs_review: bool,
    pub review_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

/// Maps model-selected rubric levels to canonical scores. The model never
/// supplies a score: it only supplies a criterion, a level, and evidence.
pub fn evaluate_semantic_output(
    question: &Question,
    answer_text: &str,
    output: &ScoringOutput,
    parse_error: Option<&str>,
) -> SemanticScoringEvaluation {
    let mut reasons = Vec::new();
    let mut warnings = output.warnings.clone();
    let mut hard_failure = parse_error.is_some();
    let mut soft_review = output.needs_review || output.confidence < 0.65;

    if parse_error.is_some() {
        reasons.push("semantic_scoring_parse_failed".to_string());
    }
    if output.direct_score_rejected || !output.direct_score_fields.is_empty() {
        reasons.push("model_direct_score_ignored".to_string());
        warnings.push("model_direct_score_ignored".to_string());
        hard_failure = true;
    }
    if output.criterion_decisions.is_empty() {
        reasons.push("semantic_criterion_decisions_missing".to_string());
        hard_failure = true;
    }

    let mut seen_criteria = HashSet::new();
    let mut canonical_scores = Vec::new();
    let mut total_score = 0.0f32;

    for decision in &output.criterion_decisions {
        let criterion_id = decision.criterion_id.trim();
        let Some(criterion) = question
            .rubric
            .criteria
            .iter()
            .find(|criterion| criterion.id == criterion_id)
        else {
            reasons.push("semantic_criterion_id_unknown".to_string());
            hard_failure = true;
            continue;
        };
        if !seen_criteria.insert(criterion_id.to_string()) {
            reasons.push("semantic_criterion_id_duplicate".to_string());
            hard_failure = true;
            continue;
        }
        if criterion.levels.is_empty() {
            reasons.push("rubric_levels_missing".to_string());
            hard_failure = true;
            continue;
        }
        let Some(level) = criterion
            .levels
            .iter()
            .find(|level| level.id == decision.level_id.trim())
        else {
            reasons.push("semantic_level_id_unknown".to_string());
            hard_failure = true;
            continue;
        };
        if !level.score.is_finite() || level.score < 0.0 || level.score > criterion.points {
            reasons.push("semantic_level_score_invalid".to_string());
            hard_failure = true;
            continue;
        }

        let evidence = decision
            .exact_evidence
            .as_deref()
            .map(str::trim)
            .filter(|evidence| !evidence.is_empty());
        if level.evidence_required && evidence.is_none() {
            reasons.push("semantic_evidence_missing".to_string());
            hard_failure = true;
        }
        if let Some(evidence) = evidence {
            if !answer_text.contains(evidence) {
                reasons.push("semantic_evidence_not_in_answer".to_string());
                hard_failure = true;
            }
        }
        if !decision.missing_requirements.is_empty() {
            reasons.push("semantic_missing_requirement".to_string());
            soft_review = true;
        }
        if decision.contradiction {
            reasons.push("semantic_contradiction".to_string());
            soft_review = true;
        }

        total_score += level.score;
        canonical_scores.push(ScoringCriterionScore {
            criterion_id: criterion.id.clone(),
            criterion_title: criterion.label.clone(),
            criterion_max_score: criterion.points,
            awarded_score: level.score,
            rationale: if decision.rationale.trim().is_empty() {
                format!("Rubrik seviyesi seçildi: {}.", level.title)
            } else {
                decision.rationale.clone()
            },
            evidence_quote: evidence.map(str::to_string),
        });
    }

    if question
        .rubric
        .criteria
        .iter()
        .any(|criterion| !seen_criteria.contains(&criterion.id))
    {
        reasons.push("semantic_criterion_id_missing".to_string());
        hard_failure = true;
    }

    let max_score = question
        .rubric
        .max_score
        .unwrap_or(question.max_score)
        .max(0.0);
    let awarded_score = (!hard_failure).then_some(total_score.min(max_score));
    if output.confidence < 0.65 {
        reasons.push("low_scoring_confidence".to_string());
        soft_review = true;
    }
    if output.rationale.trim().is_empty() {
        reasons.push("semantic_rationale_missing".to_string());
        soft_review = true;
    }

    reasons.sort();
    reasons.dedup();
    warnings.sort();
    warnings.dedup();

    SemanticScoringEvaluation {
        awarded_score,
        criterion_scores: canonical_scores,
        semantic_decisions: output.criterion_decisions.clone(),
        rationale: output.rationale.clone(),
        confidence: output.confidence,
        scoring_applied: awarded_score.is_some(),
        needs_review: hard_failure || soft_review || !reasons.is_empty(),
        review_reasons: reasons,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::SemanticCriterionDecision;
    use crate::domain::question::default_question;
    use crate::domain::rubric::{RubricCriterion, RubricLevel, RubricStatus};

    fn question_with_levels() -> Question {
        let mut question = default_question(1);
        question.rubric.status = RubricStatus::Confirmed;
        question.rubric.max_score = Some(4.0);
        question.rubric.expected_answer = Some("Doğru cevap".to_string());
        question.rubric.criteria = vec![RubricCriterion {
            id: "c1".to_string(),
            label: "Doğruluk".to_string(),
            description: "Cevabın doğruluğu".to_string(),
            points: 4.0,
            levels: vec![],
        }];
        question.rubric.criteria[0].levels = vec![RubricLevel {
            id: "full".to_string(),
            title: "Tam".to_string(),
            required_conditions: vec!["Doğru cevap".to_string()],
            disqualifying_conditions: vec![],
            score: 4.0,
            evidence_required: true,
            version: "rubric_level_v1".to_string(),
        }];
        question
    }

    fn output(decision: SemanticCriterionDecision) -> ScoringOutput {
        ScoringOutput {
            awarded_score: 999.0,
            confidence: 0.95,
            rationale: "Kanonik evidence ile seviye seçildi.".to_string(),
            teacher_visible_explanation: String::new(),
            needs_review: false,
            warnings: vec![],
            criterion_scores: vec![],
            criterion_decisions: vec![decision],
            direct_score_fields: vec![],
            direct_score_rejected: false,
        }
    }

    #[test]
    fn rust_maps_level_score_and_ignores_model_numeric_score() {
        let question = question_with_levels();
        let result = evaluate_semantic_output(
            &question,
            "Doğru cevap",
            &output(SemanticCriterionDecision {
                criterion_id: question.rubric.criteria[0].id.clone(),
                level_id: "full".to_string(),
                exact_evidence: Some("Doğru cevap".to_string()),
                missing_requirements: vec![],
                contradiction: false,
                rationale: "Tam karşılıyor.".to_string(),
            }),
            None,
        );
        assert_eq!(result.awarded_score, Some(4.0));
        assert!(!result
            .review_reasons
            .contains(&"model_direct_score_ignored".to_string()));
    }

    #[test]
    fn invalid_level_or_evidence_is_reviewable_and_not_zero() {
        let question = question_with_levels();
        let result = evaluate_semantic_output(
            &question,
            "Başka cevap",
            &output(SemanticCriterionDecision {
                criterion_id: question.rubric.criteria[0].id.clone(),
                level_id: "unknown".to_string(),
                exact_evidence: Some("Başka cevap".to_string()),
                missing_requirements: vec![],
                contradiction: false,
                rationale: String::new(),
            }),
            None,
        );
        assert_eq!(result.awarded_score, None);
        assert!(result.needs_review);
    }

    #[test]
    fn direct_numeric_model_field_is_recorded_as_review() {
        let question = question_with_levels();
        let mut model_output = output(SemanticCriterionDecision {
            criterion_id: question.rubric.criteria[0].id.clone(),
            level_id: "full".to_string(),
            exact_evidence: Some("Doğru cevap".to_string()),
            missing_requirements: vec![],
            contradiction: false,
            rationale: "Tam karşılıyor.".to_string(),
        });
        model_output.direct_score_fields = vec!["awardedScore".to_string()];
        model_output.direct_score_rejected = true;
        let result = evaluate_semantic_output(&question, "Doğru cevap", &model_output, None);
        assert_eq!(result.awarded_score, None);
        assert!(result
            .review_reasons
            .contains(&"model_direct_score_ignored".to_string()));
    }
}

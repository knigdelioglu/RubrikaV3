use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::question::AnswerType;
use crate::domain::rubric::{RubricState, RubricStatus};
use crate::domain::scoring::ScoringCriterionScore;
use crate::domain::structured_answer::{
    CorrectionTableRow, MatchingPair, MultipleChoiceSelection, OrderedSlot, StructuredAnswer,
    StructuredTableRow,
};
use crate::services::text_normalization::normalize_for_comparison;

pub const DETERMINISTIC_SCORING_POLICY_VERSION: &str = "deterministic_scoring_policy_v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NumericScoringPolicy {
    pub absolute_tolerance: f64,
    pub relative_tolerance: f64,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub expected_unit: Option<String>,
    #[serde(default)]
    pub accepted_unit_aliases: Vec<String>,
}

impl Default for NumericScoringPolicy {
    fn default() -> Self {
        Self {
            absolute_tolerance: 0.0,
            relative_tolerance: 0.0,
            locale: "tr-TR".to_string(),
            expected_unit: None,
            accepted_unit_aliases: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicScoringPolicy {
    pub version: String,
    pub qep_frozen: bool,
    pub rubric_confirmed: bool,
    pub allow_partial_credit: bool,
    #[serde(default)]
    pub numeric: NumericScoringPolicy,
}

impl Default for DeterministicScoringPolicy {
    fn default() -> Self {
        Self {
            version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
            qep_frozen: false,
            rubric_confirmed: false,
            allow_partial_credit: false,
            numeric: NumericScoringPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicScoringInput {
    pub answer_type: AnswerType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_answer: Option<StructuredAnswer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_answer_text: Option<String>,
    pub student_answer: StructuredAnswer,
    pub rubric: RubricState,
    pub policy: DeterministicScoringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicScoreProposal {
    pub awarded_score: f32,
    pub criterion_scores: Vec<ScoringCriterionScore>,
    pub rationale: String,
    pub evidence: Vec<String>,
    pub policy_version: String,
    pub model_called: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicScoringFailure {
    pub code: String,
    pub message: String,
    pub policy_version: String,
    pub model_called: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeterministicScoringResult {
    Applied(DeterministicScoreProposal),
    Reviewable(DeterministicScoringFailure),
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparisonOutcome {
    Exact,
    Partial,
    Incorrect,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicScoringService;

impl DeterministicScoringService {
    pub fn new() -> Self {
        Self
    }

    pub fn supports(answer_type: &AnswerType) -> bool {
        matches!(
            answer_type,
            AnswerType::MultipleChoice
                | AnswerType::TrueFalse
                | AnswerType::Matching
                | AnswerType::Ordering
                | AnswerType::FillBlank
                | AnswerType::Numeric
                | AnswerType::Table
                | AnswerType::CorrectionTable
        )
    }

    pub fn score(&self, input: DeterministicScoringInput) -> DeterministicScoringResult {
        if !Self::supports(&input.answer_type) {
            return DeterministicScoringResult::Unsupported;
        }
        if !input.policy.qep_frozen
            || !input.policy.rubric_confirmed
            || input.rubric.status != RubricStatus::Confirmed
        {
            return DeterministicScoringResult::Reviewable(failure(
                "deterministic_policy_not_frozen",
                "Deterministik puanlama için onaylı ve donmuş rubrik politikası gerekli.",
                &input.policy,
            ));
        }
        let canonical = match canonical_answer(&input) {
            Ok(answer) => answer,
            Err(error) => return DeterministicScoringResult::Reviewable(error),
        };
        if let Err(error) = crate::domain::structured_answer::validate_for_answer_type(
            &input.answer_type,
            &input.student_answer,
        ) {
            return DeterministicScoringResult::Reviewable(failure(
                "structured_answer_invalid",
                &format!(
                    "Öğrenci cevabının yapılandırılmış biçimi doğrulanamadı: {}",
                    error.message
                ),
                &input.policy,
            ));
        }

        let comparison = match compare_answers(
            &input.answer_type,
            &canonical,
            &input.student_answer,
            &input.policy,
        ) {
            Ok(result) => result,
            Err(error) => return DeterministicScoringResult::Reviewable(error),
        };
        let ratio = match comparison.0 {
            ComparisonOutcome::Exact => 1.0,
            ComparisonOutcome::Partial if input.policy.allow_partial_credit => comparison.1,
            ComparisonOutcome::Partial => 0.0,
            ComparisonOutcome::Incorrect => 0.0,
        };
        let evidence = comparison.2;
        let criterion_scores = input
            .rubric
            .criteria
            .iter()
            .map(|criterion| ScoringCriterionScore {
                criterion_id: criterion.id.clone(),
                criterion_title: criterion.label.clone(),
                criterion_max_score: criterion.points,
                awarded_score: criterion.points * ratio as f32,
                rationale: match comparison.0 {
                    ComparisonOutcome::Exact => "Kanonik cevapla tam eşleşti.".to_string(),
                    ComparisonOutcome::Partial if input.policy.allow_partial_credit => {
                        "Kanonik cevabın izin verilen kısmıyla eşleşti.".to_string()
                    }
                    _ => "Kanonik cevapla eşleşmedi.".to_string(),
                },
                evidence_quote: evidence.first().cloned(),
            })
            .collect::<Vec<_>>();
        let awarded_score = criterion_scores
            .iter()
            .map(|criterion| criterion.awarded_score)
            .sum::<f32>()
            .min(input.rubric.max_score.unwrap_or_default().max(0.0));

        DeterministicScoringResult::Applied(DeterministicScoreProposal {
            awarded_score,
            criterion_scores,
            rationale: "Sonuç backend deterministic scorer tarafından üretildi.".to_string(),
            evidence,
            policy_version: input.policy.version,
            model_called: false,
        })
    }
}

fn failure(
    code: &str,
    message: &str,
    policy: &DeterministicScoringPolicy,
) -> DeterministicScoringFailure {
    DeterministicScoringFailure {
        code: code.to_string(),
        message: message.to_string(),
        policy_version: policy.version.clone(),
        model_called: false,
    }
}

fn canonical_answer(
    input: &DeterministicScoringInput,
) -> Result<StructuredAnswer, DeterministicScoringFailure> {
    if let Some(answer) = input.canonical_answer.clone() {
        return crate::domain::structured_answer::validate_for_answer_type(
            &input.answer_type,
            &answer,
        )
        .map(|_| answer)
        .map_err(|error| failure("canonical_answer_invalid", &error.message, &input.policy));
    }
    let Some(text) = input.canonical_answer_text.as_deref() else {
        return Err(failure(
            "canonical_answer_missing",
            "Deterministik puanlama için kanonik cevap eksik.",
            &input.policy,
        ));
    };
    parse_answer_text(&input.answer_type, text).and_then(|answer| {
        crate::domain::structured_answer::validate_for_answer_type(&input.answer_type, &answer)
            .map(|_| answer)
            .map_err(|error| failure("canonical_answer_invalid", &error.message, &input.policy))
    })
}

fn parse_answer_text(
    answer_type: &AnswerType,
    text: &str,
) -> Result<StructuredAnswer, DeterministicScoringFailure> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if let Ok(answer) =
            crate::domain::structured_answer::parse_for_answer_type(answer_type, value)
        {
            return Ok(answer);
        }
    }
    let parts = split_items(text);
    let answer = match answer_type {
        AnswerType::MultipleChoice | AnswerType::TrueFalse => StructuredAnswer::MultipleChoice {
            selections: parts
                .into_iter()
                .map(|option| MultipleChoiceSelection {
                    option,
                    selected: true,
                })
                .collect(),
        },
        AnswerType::Matching => StructuredAnswer::Matching {
            pairs: parts
                .into_iter()
                .filter_map(|part| {
                    let (left, right) = part.split_once(':').or_else(|| part.split_once('='))?;
                    Some(MatchingPair {
                        left: left.trim().to_string(),
                        right: right.trim().to_string(),
                    })
                })
                .collect(),
        },
        AnswerType::Ordering | AnswerType::FillBlank => StructuredAnswer::OrderedSlots {
            slots: parts
                .into_iter()
                .enumerate()
                .map(|(index, value)| OrderedSlot {
                    index: index as u32,
                    value,
                })
                .collect(),
        },
        AnswerType::Numeric => {
            let mut tokens = text.split_whitespace();
            let value = tokens.next().map(ToString::to_string);
            let unit = tokens.next().map(ToString::to_string);
            StructuredAnswer::Numeric { value, unit }
        }
        AnswerType::Table => StructuredAnswer::Table {
            rows: text
                .split(';')
                .enumerate()
                .map(|(index, row)| StructuredTableRow {
                    index: index as u32,
                    cells: row.split('|').map(|cell| cell.trim().to_string()).collect(),
                })
                .collect(),
        },
        AnswerType::CorrectionTable => StructuredAnswer::CorrectionTable { rows: vec![] },
        _ => {
            return Err(DeterministicScoringFailure {
                code: "canonical_answer_unsupported".to_string(),
                message: "Kanonik cevap türü deterministic scorer tarafından desteklenmiyor."
                    .to_string(),
                policy_version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
                model_called: false,
            })
        }
    };
    Ok(answer)
}

fn split_items(text: &str) -> Vec<String> {
    text.split([',', ';', '|', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn compare_answers(
    answer_type: &AnswerType,
    canonical: &StructuredAnswer,
    student: &StructuredAnswer,
    policy: &DeterministicScoringPolicy,
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    match (answer_type, canonical, student) {
        (
            AnswerType::MultipleChoice | AnswerType::TrueFalse,
            StructuredAnswer::MultipleChoice {
                selections: expected,
            },
            StructuredAnswer::MultipleChoice { selections: actual },
        ) => compare_selections(expected, actual),
        (
            AnswerType::Matching,
            StructuredAnswer::Matching { pairs: expected },
            StructuredAnswer::Matching { pairs: actual },
        ) => compare_matching(expected, actual),
        (
            AnswerType::Ordering | AnswerType::FillBlank,
            StructuredAnswer::OrderedSlots { slots: expected },
            StructuredAnswer::OrderedSlots { slots: actual },
        ) => compare_ordered(expected, actual),
        (
            AnswerType::Numeric,
            StructuredAnswer::Numeric {
                value: expected,
                unit: expected_unit,
            },
            StructuredAnswer::Numeric {
                value: actual,
                unit: actual_unit,
            },
        ) => compare_numeric(
            expected.as_deref(),
            expected_unit.as_deref(),
            actual.as_deref(),
            actual_unit.as_deref(),
            policy,
        ),
        (
            AnswerType::Table,
            StructuredAnswer::Table { rows: expected },
            StructuredAnswer::Table { rows: actual },
        ) => compare_tables(expected, actual),
        (
            AnswerType::CorrectionTable,
            StructuredAnswer::CorrectionTable { rows: expected },
            StructuredAnswer::CorrectionTable { rows: actual },
        ) => compare_correction_tables(expected, actual),
        _ => Err(failure(
            "structured_answer_mismatched_answer_type",
            "Kanonik ve öğrenci cevaplarının typed biçimleri eşleşmiyor.",
            policy,
        )),
    }
}

fn compare_selections(
    expected: &[MultipleChoiceSelection],
    actual: &[MultipleChoiceSelection],
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    let expected = selected_set(expected);
    let actual = selected_set(actual);
    if expected.is_empty() || actual.is_empty() {
        return Err(DeterministicScoringFailure {
            code: "multiple_choice_selection_missing".to_string(),
            message: "Seçenek cevabı boş veya malformed olduğu için inceleme gerekiyor."
                .to_string(),
            policy_version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
            model_called: false,
        });
    }
    let matched = expected.intersection(&actual).count();
    let ratio = matched as f64 / expected.len().max(actual.len()) as f64;
    let outcome = if expected == actual {
        ComparisonOutcome::Exact
    } else if matched > 0 {
        ComparisonOutcome::Partial
    } else {
        ComparisonOutcome::Incorrect
    };
    Ok((outcome, ratio, actual.into_iter().collect()))
}

fn selected_set(selections: &[MultipleChoiceSelection]) -> HashSet<String> {
    selections
        .iter()
        .filter(|selection| selection.selected)
        .map(|selection| normalize_for_comparison(&selection.option))
        .filter(|option| !option.is_empty())
        .collect()
}

fn compare_matching(
    expected: &[MatchingPair],
    actual: &[MatchingPair],
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    if expected.is_empty()
        || expected
            .iter()
            .any(|pair| pair.left.trim().is_empty() || pair.right.trim().is_empty())
        || actual
            .iter()
            .any(|pair| pair.left.trim().is_empty() || pair.right.trim().is_empty())
        || has_duplicate_matching_left(expected)
        || has_duplicate_matching_left(actual)
    {
        return Err(DeterministicScoringFailure {
            code: "matching_pair_malformed".to_string(),
            message: "Eşleştirme cevabında eksik eş bulunduğu için inceleme gerekiyor.".to_string(),
            policy_version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
            model_called: false,
        });
    }
    let expected_map = pair_map(expected);
    let actual_map = pair_map(actual);
    let matched = expected_map
        .iter()
        .filter(|(left, right)| actual_map.get(*left) == Some(*right))
        .count();
    let ratio = matched as f64 / expected_map.len() as f64;
    let outcome = if expected_map == actual_map {
        ComparisonOutcome::Exact
    } else if matched > 0 {
        ComparisonOutcome::Partial
    } else {
        ComparisonOutcome::Incorrect
    };
    Ok((
        outcome,
        ratio,
        actual
            .iter()
            .map(|pair| format!("{}: {}", pair.left, pair.right))
            .collect(),
    ))
}

fn pair_map(pairs: &[MatchingPair]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|pair| {
            (
                normalize_for_comparison(&pair.left),
                normalize_for_comparison(&pair.right),
            )
        })
        .collect()
}

fn has_duplicate_matching_left(pairs: &[MatchingPair]) -> bool {
    let mut lefts = HashSet::new();
    pairs
        .iter()
        .any(|pair| !lefts.insert(normalize_for_comparison(&pair.left)))
}

fn compare_ordered(
    expected: &[OrderedSlot],
    actual: &[OrderedSlot],
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    if expected.is_empty()
        || actual.len() != expected.len()
        || actual.iter().any(|slot| slot.value.trim().is_empty())
    {
        return Err(DeterministicScoringFailure {
            code: "ordered_slots_malformed".to_string(),
            message: "Sıralama veya boşluk doldurma cevaplarında eksik alan bulunduğu için inceleme gerekiyor.".to_string(),
            policy_version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
            model_called: false,
        });
    }
    let matched = expected
        .iter()
        .zip(actual)
        .filter(|(left, right)| {
            normalize_for_comparison(&left.value) == normalize_for_comparison(&right.value)
        })
        .count();
    let ratio = matched as f64 / expected.len() as f64;
    let outcome = if matched == expected.len() {
        ComparisonOutcome::Exact
    } else if matched > 0 {
        ComparisonOutcome::Partial
    } else {
        ComparisonOutcome::Incorrect
    };
    Ok((
        outcome,
        ratio,
        actual.iter().map(|slot| slot.value.clone()).collect(),
    ))
}

fn compare_numeric(
    expected: Option<&str>,
    expected_unit: Option<&str>,
    actual: Option<&str>,
    actual_unit: Option<&str>,
    policy: &DeterministicScoringPolicy,
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    let expected = expected
        .and_then(|value| parse_locale_number(value, &policy.numeric.locale))
        .ok_or_else(|| {
            failure(
                "numeric_expected_malformed",
                "Kanonik numeric cevap çözümlenemedi.",
                policy,
            )
        })?;
    let actual = actual
        .and_then(|value| parse_locale_number(value, &policy.numeric.locale))
        .ok_or_else(|| {
            failure(
                "numeric_student_malformed",
                "Öğrenci numeric cevabı çözümlenemedi.",
                policy,
            )
        })?;
    let expected_unit = expected_unit.or(policy.numeric.expected_unit.as_deref());
    if !units_compatible(
        expected_unit,
        actual_unit,
        &policy.numeric.accepted_unit_aliases,
    ) {
        return Err(failure(
            "numeric_unit_mismatch",
            "Numeric cevap birimi kanonik rubrikle eşleşmiyor; öğretmen incelemesi gerekiyor.",
            policy,
        ));
    }
    let difference = (expected - actual).abs();
    let tolerance =
        policy.numeric.absolute_tolerance + expected.abs() * policy.numeric.relative_tolerance;
    let outcome = if difference <= tolerance {
        ComparisonOutcome::Exact
    } else {
        ComparisonOutcome::Incorrect
    };
    Ok((
        outcome,
        f64::from((difference <= tolerance) as u8),
        vec![actual.to_string()],
    ))
}

fn parse_locale_number(value: &str, locale: &str) -> Option<f64> {
    let mut normalized = value.trim().replace(' ', "");
    if locale.to_ascii_lowercase().starts_with("tr") {
        normalized = normalized.replace('.', "").replace(',', ".");
    } else if normalized.contains(',') && normalized.contains('.') {
        normalized = normalized.replace(',', "");
    } else if normalized.matches(',').count() == 1 && !normalized.contains('.') {
        normalized = normalized.replace(',', ".");
    }
    normalized
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn units_compatible(expected: Option<&str>, actual: Option<&str>, aliases: &[String]) -> bool {
    let Some(expected) = expected.map(normalize_for_comparison) else {
        return actual.map_or(true, |value| value.trim().is_empty());
    };
    let Some(actual) = actual.map(normalize_for_comparison) else {
        return false;
    };
    expected == actual
        || aliases
            .iter()
            .map(|alias| normalize_for_comparison(alias))
            .any(|alias| alias == actual || alias == expected)
}

fn compare_tables(
    expected: &[StructuredTableRow],
    actual: &[StructuredTableRow],
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    if expected.is_empty()
        || actual.len() != expected.len()
        || actual.iter().any(|row| row.cells.is_empty())
        || expected
            .iter()
            .any(|row| row.cells.len() != expected[0].cells.len())
        || actual
            .iter()
            .any(|row| row.cells.len() != expected[0].cells.len())
    {
        return Err(DeterministicScoringFailure {
            code: "structured_table_malformed".to_string(),
            message: "Tablo cevabında eksik satır veya hücre bulunduğu için inceleme gerekiyor."
                .to_string(),
            policy_version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
            model_called: false,
        });
    }
    let matched = expected
        .iter()
        .zip(actual)
        .filter(|(left, right)| normalized_cells(&left.cells) == normalized_cells(&right.cells))
        .count();
    let ratio = matched as f64 / expected.len() as f64;
    let outcome = if matched == expected.len() {
        ComparisonOutcome::Exact
    } else if matched > 0 {
        ComparisonOutcome::Partial
    } else {
        ComparisonOutcome::Incorrect
    };
    Ok((
        outcome,
        ratio,
        actual.iter().flat_map(|row| row.cells.clone()).collect(),
    ))
}

fn compare_correction_tables(
    expected: &[CorrectionTableRow],
    actual: &[CorrectionTableRow],
) -> Result<(ComparisonOutcome, f64, Vec<String>), DeterministicScoringFailure> {
    if expected.is_empty()
        || actual.len() != expected.len()
        || actual
            .iter()
            .any(|row| row.original.trim().is_empty() || row.correction.trim().is_empty())
    {
        return Err(DeterministicScoringFailure {
            code: "correction_table_malformed".to_string(),
            message: "Düzeltme tablosunda eksik alan bulunduğu için inceleme gerekiyor."
                .to_string(),
            policy_version: DETERMINISTIC_SCORING_POLICY_VERSION.to_string(),
            model_called: false,
        });
    }
    let matched = expected
        .iter()
        .zip(actual)
        .filter(|(left, right)| {
            normalize_for_comparison(&left.original) == normalize_for_comparison(&right.original)
                && normalize_for_comparison(&left.correction)
                    == normalize_for_comparison(&right.correction)
        })
        .count();
    let ratio = matched as f64 / expected.len() as f64;
    let outcome = if matched == expected.len() {
        ComparisonOutcome::Exact
    } else if matched > 0 {
        ComparisonOutcome::Partial
    } else {
        ComparisonOutcome::Incorrect
    };
    Ok((
        outcome,
        ratio,
        actual.iter().map(|row| row.correction.clone()).collect(),
    ))
}

fn normalized_cells(cells: &[String]) -> Vec<String> {
    cells
        .iter()
        .map(|cell| normalize_for_comparison(cell))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::rubric::{RubricCriterion, RubricStatus};
    use crate::domain::structured_answer::{MultipleChoiceSelection, StructuredAnswer};

    fn rubric(answer: &str) -> RubricState {
        RubricState {
            status: RubricStatus::Confirmed,
            source: None,
            max_score: Some(10.0),
            expected_answer: Some(answer.to_string()),
            key_concepts: vec![],
            criteria: vec![RubricCriterion {
                id: "criterion-1".to_string(),
                label: "Doğruluk".to_string(),
                description: "Doğru cevap".to_string(),
                points: 10.0,
                levels: vec![],
            }],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        }
    }

    fn policy() -> DeterministicScoringPolicy {
        DeterministicScoringPolicy {
            qep_frozen: true,
            rubric_confirmed: true,
            allow_partial_credit: true,
            ..Default::default()
        }
    }

    #[test]
    fn multiple_choice_is_scored_without_model_and_is_turkish_normalized() {
        let result = DeterministicScoringService::new().score(DeterministicScoringInput {
            answer_type: AnswerType::MultipleChoice,
            canonical_answer: Some(StructuredAnswer::MultipleChoice {
                selections: vec![MultipleChoiceSelection {
                    option: "İstanbul".into(),
                    selected: true,
                }],
            }),
            canonical_answer_text: None,
            student_answer: StructuredAnswer::MultipleChoice {
                selections: vec![MultipleChoiceSelection {
                    option: "istanbul".into(),
                    selected: true,
                }],
            },
            rubric: rubric("İstanbul"),
            policy: policy(),
        });
        let DeterministicScoringResult::Applied(proposal) = result else {
            panic!("expected deterministic result")
        };
        assert_eq!(proposal.awarded_score, 10.0);
        assert!(!proposal.model_called);
    }

    #[test]
    fn supported_structured_types_dispatch_to_rust_without_model() {
        let cases = vec![
            (
                AnswerType::TrueFalse,
                StructuredAnswer::MultipleChoice {
                    selections: vec![MultipleChoiceSelection {
                        option: "Doğru".into(),
                        selected: true,
                    }],
                },
                StructuredAnswer::MultipleChoice {
                    selections: vec![MultipleChoiceSelection {
                        option: "doğru".into(),
                        selected: true,
                    }],
                },
            ),
            (
                AnswerType::Matching,
                StructuredAnswer::Matching {
                    pairs: vec![MatchingPair {
                        left: "A".into(),
                        right: "1".into(),
                    }],
                },
                StructuredAnswer::Matching {
                    pairs: vec![MatchingPair {
                        left: "a".into(),
                        right: "1".into(),
                    }],
                },
            ),
            (
                AnswerType::Ordering,
                StructuredAnswer::OrderedSlots {
                    slots: vec![OrderedSlot {
                        index: 0,
                        value: "Birinci".into(),
                    }],
                },
                StructuredAnswer::OrderedSlots {
                    slots: vec![OrderedSlot {
                        index: 0,
                        value: "birinci".into(),
                    }],
                },
            ),
            (
                AnswerType::FillBlank,
                StructuredAnswer::OrderedSlots {
                    slots: vec![OrderedSlot {
                        index: 0,
                        value: "İstanbul".into(),
                    }],
                },
                StructuredAnswer::OrderedSlots {
                    slots: vec![OrderedSlot {
                        index: 0,
                        value: "istanbul".into(),
                    }],
                },
            ),
            (
                AnswerType::Table,
                StructuredAnswer::Table {
                    rows: vec![StructuredTableRow {
                        index: 0,
                        cells: vec!["A".into(), "B".into()],
                    }],
                },
                StructuredAnswer::Table {
                    rows: vec![StructuredTableRow {
                        index: 0,
                        cells: vec!["a".into(), "b".into()],
                    }],
                },
            ),
            (
                AnswerType::CorrectionTable,
                StructuredAnswer::CorrectionTable {
                    rows: vec![CorrectionTableRow {
                        index: 0,
                        original: "yanlış".into(),
                        correction: "doğru".into(),
                        explanation: None,
                    }],
                },
                StructuredAnswer::CorrectionTable {
                    rows: vec![CorrectionTableRow {
                        index: 0,
                        original: "YANLIŞ".into(),
                        correction: "DOĞRU".into(),
                        explanation: None,
                    }],
                },
            ),
        ];

        for (answer_type, canonical, student) in cases {
            let result = DeterministicScoringService::new().score(DeterministicScoringInput {
                answer_type,
                canonical_answer: Some(canonical),
                canonical_answer_text: None,
                student_answer: student,
                rubric: rubric("unused"),
                policy: policy(),
            });
            let DeterministicScoringResult::Applied(proposal) = result else {
                panic!("supported structured answer should be deterministic");
            };
            assert_eq!(proposal.awarded_score, 10.0);
            assert!(!proposal.model_called);
        }
    }

    #[test]
    fn numeric_turkish_decimal_and_tolerance_are_backend_policy() {
        let numeric = NumericScoringPolicy {
            absolute_tolerance: 0.01,
            ..NumericScoringPolicy::default()
        };
        let mut policy = policy();
        policy.numeric = numeric;
        let result = DeterministicScoringService::new().score(DeterministicScoringInput {
            answer_type: AnswerType::Numeric,
            canonical_answer: Some(StructuredAnswer::Numeric {
                value: Some("1,50".into()),
                unit: Some("kg".into()),
            }),
            canonical_answer_text: None,
            student_answer: StructuredAnswer::Numeric {
                value: Some("1,505".into()),
                unit: Some("kg".into()),
            },
            rubric: rubric("1,50 kg"),
            policy,
        });
        assert!(matches!(result, DeterministicScoringResult::Applied(_)));
    }

    #[test]
    fn malformed_or_unit_mismatch_is_reviewable_not_zero() {
        let result = DeterministicScoringService::new().score(DeterministicScoringInput {
            answer_type: AnswerType::Numeric,
            canonical_answer: Some(StructuredAnswer::Numeric {
                value: Some("2".into()),
                unit: Some("kg".into()),
            }),
            canonical_answer_text: None,
            student_answer: StructuredAnswer::Numeric {
                value: Some("2".into()),
                unit: Some("m".into()),
            },
            rubric: rubric("2 kg"),
            policy: policy(),
        });
        let DeterministicScoringResult::Reviewable(failure) = result else {
            panic!("expected review")
        };
        assert_eq!(failure.code, "numeric_unit_mismatch");
        assert!(!failure.model_called);
    }

    #[test]
    fn unfrozen_policy_blocks_deterministic_acceptance() {
        let result = DeterministicScoringService::new().score(DeterministicScoringInput {
            answer_type: AnswerType::MultipleChoice,
            canonical_answer: Some(StructuredAnswer::MultipleChoice { selections: vec![] }),
            canonical_answer_text: None,
            student_answer: StructuredAnswer::MultipleChoice { selections: vec![] },
            rubric: rubric("A"),
            policy: DeterministicScoringPolicy::default(),
        });
        assert!(matches!(result, DeterministicScoringResult::Reviewable(_)));
    }
}

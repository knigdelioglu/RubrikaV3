use serde::{Deserialize, Serialize};

use crate::domain::question::AnswerType;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RubricSource {
    Manual,
    Json,
    AnswerKeyPdf,
    Generated,
    #[serde(alias = "rubric_pdf")]
    RubricPdf,
    #[serde(alias = "gemma_draft", alias = "imported_template", alias = "unknown")]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RubricStatus {
    #[default]
    Missing,
    Suggested,
    Imported,
    Manual,
    Confirmed,
    Invalid,
    #[serde(alias = "edited", alias = "failed")]
    Legacy,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RubricCriterion {
    pub id: String,
    pub label: String,
    pub description: String,
    pub points: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RubricState {
    #[serde(default)]
    pub status: RubricStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<RubricSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_answer: Option<String>,
    #[serde(default)]
    pub criteria: Vec<RubricCriterion>,
    #[serde(default)]
    pub partial_credit_hints: Vec<String>,
    #[serde(default)]
    pub zero_score_conditions: Vec<String>,
    #[serde(default)]
    pub common_mistakes: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RubricValidationIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RubricValidationResult {
    pub valid: bool,
    pub confirmable: bool,
    pub warnings: Vec<String>,
    pub issues: Vec<RubricValidationIssue>,
    pub total_points: Option<f32>,
}

pub fn is_placeholder_text(text: &str) -> bool {
    let normalized = normalize_text(text).to_lowercase();
    if normalized.is_empty() {
        return true;
    }

    let placeholders = [
        "kelime1, kelime2, kelime3",
        "kısmi puan kriterleri",
        "kismi puan kriterleri",
        "anahtar kavramları girin",
        "anahtar kavramlari girin",
        "beklenen cevabı yazın",
        "beklenen cevabi yazin",
        "örnek cevap",
        "ornek cevap",
        "örnek cevap...",
        "ornek cevap...",
    ];

    placeholders
        .iter()
        .any(|placeholder| normalized.contains(placeholder))
}

pub fn normalize_text(text: &str) -> String {
    text.trim().replace('\u{a0}', " ")
}

pub fn is_technical_warning(warning: &str) -> bool {
    warning.contains("_alias:") || warning == "rubric_empty_content"
}

pub fn teacher_facing_warnings(warnings: &[String]) -> Vec<String> {
    warnings
        .iter()
        .filter(|warning| !is_technical_warning(warning))
        .cloned()
        .collect()
}

pub fn has_meaningful_rubric_content(rubric: &RubricState) -> bool {
    rubric.max_score.is_some_and(|score| score > 0.0)
        || rubric
            .expected_answer
            .as_ref()
            .is_some_and(|text| !normalize_text(text).is_empty())
        || !rubric.criteria.is_empty()
}

pub fn requires_expected_answer(answer_type: &AnswerType) -> bool {
    !matches!(answer_type, AnswerType::Table | AnswerType::CorrectionTable)
}

pub fn validate_rubric_state(
    rubric: &RubricState,
    _answer_type: Option<&AnswerType>,
) -> RubricValidationResult {
    let warnings = teacher_facing_warnings(&rubric.warnings);
    let mut issues = Vec::new();

    match rubric.status {
        RubricStatus::Missing => {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_NOT_READY".to_string(),
                message: "Rubrik eksik.".to_string(),
            });
        }
        RubricStatus::Invalid => {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_NOT_READY".to_string(),
                message: "Rubrik geçersiz.".to_string(),
            });
        }
        RubricStatus::Legacy => {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_NOT_READY".to_string(),
                message: "Rubrik eski biçimde işaretlenmiş.".to_string(),
            });
        }
        RubricStatus::Suggested
        | RubricStatus::Imported
        | RubricStatus::Manual
        | RubricStatus::Confirmed => {}
    }

    if rubric.status == RubricStatus::Imported && !has_meaningful_rubric_content(rubric) {
        issues.push(RubricValidationIssue {
            code: "RUBRIC_EMPTY_CONTENT".to_string(),
            message: "İçe aktarılan rubrik boş.".to_string(),
        });
    }

    if !rubric.max_score.is_some_and(|score| score > 0.0) {
        issues.push(RubricValidationIssue {
            code: "RUBRIC_MAX_SCORE_MISSING".to_string(),
            message: "Max puan belirtilmemiş.".to_string(),
        });
    }

    if let Some(expected_answer) = rubric.expected_answer.as_ref() {
        if is_placeholder_text(expected_answer) {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_PLACEHOLDER_DETECTED".to_string(),
                message: "Beklenen cevap alanında placeholder metin var.".to_string(),
            });
        }
    } else {
        issues.push(RubricValidationIssue {
            code: "RUBRIC_EXPECTED_ANSWER_MISSING".to_string(),
            message: "Beklenen cevap boş.".to_string(),
        });
    }

    if rubric.criteria.is_empty() {
        issues.push(RubricValidationIssue {
            code: "RUBRIC_CRITERIA_MISSING".to_string(),
            message: "En az bir kriter gerekli.".to_string(),
        });
    }

    for criterion in &rubric.criteria {
        if is_placeholder_text(&criterion.label) || is_placeholder_text(&criterion.description) {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_PLACEHOLDER_DETECTED".to_string(),
                message: format!("Kriter placeholder içeriyor: {}", criterion.label),
            });
        }
    }

    for hint in &rubric.partial_credit_hints {
        if is_placeholder_text(hint) {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_PLACEHOLDER_DETECTED".to_string(),
                message: "Kısmi puan ipuçlarında placeholder metin var.".to_string(),
            });
        }
    }

    for condition in &rubric.zero_score_conditions {
        if is_placeholder_text(condition) {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_PLACEHOLDER_DETECTED".to_string(),
                message: "Sıfır puan koşullarında placeholder metin var.".to_string(),
            });
        }
    }

    for mistake in &rubric.common_mistakes {
        if is_placeholder_text(mistake) {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_PLACEHOLDER_DETECTED".to_string(),
                message: "Yaygın yanlışlar alanında placeholder metin var.".to_string(),
            });
        }
    }

    let total_points = if rubric.criteria.is_empty() {
        Some(0.0)
    } else {
        Some(
            rubric
                .criteria
                .iter()
                .map(|criterion| criterion.points)
                .sum(),
        )
    };

    if let (Some(max_score), Some(total_points)) = (rubric.max_score, total_points) {
        if (max_score - total_points).abs() > 0.01 {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_POINTS_TOTAL_MISMATCH".to_string(),
                message: format!(
                    "Kriter puanları toplamı ({total_points:.2}) max puan ({max_score:.2}) ile uyuşmuyor."
                ),
            });
        }
    }

    let valid = issues.is_empty();
    let confirmable = valid && rubric.status != RubricStatus::Missing;

    RubricValidationResult {
        valid,
        confirmable,
        warnings,
        issues,
        total_points,
    }
}

pub fn is_rubric_confirmed(rubric: &RubricState, answer_type: Option<&AnswerType>) -> bool {
    rubric.status == RubricStatus::Confirmed && validate_rubric_state(rubric, answer_type).valid
}

pub fn rubric_status_label(status: &RubricStatus) -> &'static str {
    match status {
        RubricStatus::Missing => "missing",
        RubricStatus::Suggested => "suggested",
        RubricStatus::Imported => "imported",
        RubricStatus::Manual => "manual",
        RubricStatus::Confirmed => "confirmed",
        RubricStatus::Invalid => "invalid",
        RubricStatus::Legacy => "legacy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rubric() -> RubricState {
        RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: Some(10.0),
            expected_answer: Some("Real expected answer".to_string()),
            criteria: vec![
                RubricCriterion {
                    id: "c1".to_string(),
                    label: "Konuya uygunluk".to_string(),
                    description: "Yanıtın konuya uyumu".to_string(),
                    points: 4.0,
                },
                RubricCriterion {
                    id: "c2".to_string(),
                    label: "Açıklık".to_string(),
                    description: "Cevabın açıklığı".to_string(),
                    points: 6.0,
                },
            ],
            partial_credit_hints: vec!["İkinci adım doğruysa yarım puan".to_string()],
            zero_score_conditions: vec!["Yanıt boşsa".to_string()],
            common_mistakes: vec!["Kavramları karıştırma".to_string()],
            warnings: vec![],
            updated_at: None,
        }
    }

    #[test]
    fn placeholder_detection_catches_common_turkish_phrases() {
        assert!(is_placeholder_text("Beklenen cevabı yazın..."));
        assert!(is_placeholder_text(
            "Kısmi puan kriterleri veya ek değerlendirme notları..."
        ));
        assert!(is_placeholder_text("örnek cevap"));
    }

    #[test]
    fn validation_accepts_complete_rubric() {
        let rubric = sample_rubric();
        let result = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(result.valid);
        assert!(result.confirmable);
        assert_eq!(result.total_points, Some(10.0));
    }

    #[test]
    fn validation_flags_missing_max_score_and_placeholders() {
        let mut rubric = sample_rubric();
        rubric.max_score = None;
        rubric.expected_answer = Some("örnek cevap".to_string());
        let result = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_MAX_SCORE_MISSING"));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_PLACEHOLDER_DETECTED"));
    }

    #[test]
    fn validation_flags_score_mismatch() {
        let mut rubric = sample_rubric();
        rubric.criteria[0].points = 3.0;
        let result = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_POINTS_TOTAL_MISMATCH"));
    }

    #[test]
    fn validation_rejects_empty_imported_rubric() {
        let rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: None,
            expected_answer: None,
            criteria: vec![],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec!["maxPoints_alias:max_points".to_string()],
            updated_at: None,
        };

        let result = validate_rubric_state(&rubric, Some(&AnswerType::Essay));
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_EMPTY_CONTENT"));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_EXPECTED_ANSWER_MISSING"));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_CRITERIA_MISSING"));
        assert!(!result
            .warnings
            .iter()
            .any(|warning| warning.contains("_alias:")));
    }

    #[test]
    fn validation_allows_empty_later_guidance_fields() {
        let mut rubric = sample_rubric();
        rubric.partial_credit_hints = vec![];
        rubric.zero_score_conditions = vec![];
        rubric.common_mistakes = vec![];

        let result = validate_rubric_state(&rubric, Some(&AnswerType::Essay));

        assert!(result.valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn validation_flags_freeze_v1_required_fields() {
        let rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: Some(0.0),
            expected_answer: None,
            criteria: vec![],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        };

        let result = validate_rubric_state(&rubric, Some(&AnswerType::Essay));

        assert!(!result.valid);
        for code in [
            "RUBRIC_MAX_SCORE_MISSING",
            "RUBRIC_EXPECTED_ANSWER_MISSING",
            "RUBRIC_CRITERIA_MISSING",
        ] {
            assert!(result.issues.iter().any(|issue| issue.code == code));
        }
    }

    #[test]
    fn teacher_facing_warnings_hide_technical_alias_codes() {
        let warnings = teacher_facing_warnings(&[
            "maxPoints_alias:max_points".to_string(),
            "rubric_empty_content".to_string(),
            "Rubrik boş geldi.".to_string(),
        ]);

        assert_eq!(warnings, vec!["Rubrik boş geldi.".to_string()]);
    }
}

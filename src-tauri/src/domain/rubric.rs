use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    GemmaDraft,
    #[serde(alias = "imported_template", alias = "unknown")]
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
pub struct RubricLevel {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub required_conditions: Vec<String>,
    #[serde(default)]
    pub disqualifying_conditions: Vec<String>,
    pub score: f32,
    #[serde(default)]
    pub evidence_required: bool,
    #[serde(default = "default_rubric_level_version")]
    pub version: String,
}

pub const RUBRIC_LEVEL_SCHEMA_VERSION: &str = "rubric_level_v1";

fn default_rubric_level_version() -> String {
    RUBRIC_LEVEL_SCHEMA_VERSION.to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RubricCriterion {
    pub id: String,
    pub label: String,
    pub description: String,
    pub points: f32,
    #[serde(default)]
    pub levels: Vec<RubricLevel>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub key_concepts: Vec<String>,
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

pub const RUBRIC_EXTRACTION_SCHEMA_VERSION: &str = "rubric_extraction_contract_v2";

/// Returns the canonical rubric data fields from the persisted DTO.  The
/// extraction contract is built from this list so source/status are kept as
/// backend-owned boundaries instead of becoming model-authored truth.
pub fn canonical_rubric_data_field_names() -> Vec<String> {
    let value = serde_json::to_value(RubricState {
        status: RubricStatus::Missing,
        source: None,
        // These non-authoritative sample values make fields with
        // skip_serializing_if appear in the reflected DTO shape. Their
        // contents are never sent to the model.
        max_score: Some(0.0),
        expected_answer: Some(String::new()),
        key_concepts: vec![],
        criteria: vec![],
        partial_credit_hints: vec![],
        zero_score_conditions: vec![],
        common_mistakes: vec![],
        warnings: vec![],
        updated_at: None,
    })
    .unwrap_or_else(|_| json!({}));
    value
        .as_object()
        .map(|object| {
            object
                .keys()
                .filter(|key| !matches!(key.as_str(), "status" | "source" | "updatedAt"))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn canonical_rubric_extraction_schema() -> Value {
    let mut rubric_properties = serde_json::Map::new();
    for field in canonical_rubric_data_field_names() {
        let schema = match field.as_str() {
            "maxScore" => json!({"type": ["number", "null"], "minimum": 0}),
            "expectedAnswer" => json!({"type": ["string", "null"]}),
            "keyConcepts"
            | "partialCreditHints"
            | "zeroScoreConditions"
            | "commonMistakes"
            | "warnings" => json!({"type": "array", "items": {"type": "string"}}),
            "criteria" => json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": ["string", "null"]},
                        "label": {"type": "string"},
                        "description": {"type": "string"},
                        "points": {"type": "number", "minimum": 0},
                        "levels": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": {"type": "string"},
                                    "title": {"type": "string"},
                                    "requiredConditions": {"type": "array", "items": {"type": "string"}},
                                    "disqualifyingConditions": {"type": "array", "items": {"type": "string"}},
                                    "score": {"type": "number", "minimum": 0},
                                    "evidenceRequired": {"type": "boolean"},
                                    "version": {"type": "string"}
                                },
                                "required": ["id", "title", "score", "version"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["label", "description", "points"],
                    "additionalProperties": false
                }
            }),
            _ => json!({}),
        };
        rubric_properties.insert(field, schema);
    }

    let mut question_properties = serde_json::Map::new();
    question_properties.insert(
        "questionNumber".to_string(),
        json!({"type": "integer", "minimum": 1}),
    );
    question_properties.extend(rubric_properties);

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "RubricExtractionSuggestion",
        "version": RUBRIC_EXTRACTION_SCHEMA_VERSION,
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "questions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": question_properties,
                    "required": ["questionNumber", "criteria"]
                }
            },
            "documentWarnings": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["questions", "documentWarnings"]
    })
}

pub fn canonical_rubric_extraction_prompt() -> String {
    let fields = canonical_rubric_data_field_names().join(", ");
    format!(
        "Rubrik çıkarma sözleşmesi {RUBRIC_EXTRACTION_SCHEMA_VERSION}. Yalnızca JSON döndür. Her soru için questionNumber ve canonical RubricState veri alanlarını kullan: {fields}. source, status ve updatedAt alanlarını model üretmez; bunlar backend tarafından suggestion olarak atanır. Placeholder metin kullanma."
    )
}

/// A rubric with no level data is an old numeric/max-only rubric. It can be
/// opened and reviewed for backward compatibility, but it is not a semantic
/// level policy until the explicit migration assistant is accepted.
pub fn rubric_requires_level_migration(rubric: &RubricState) -> bool {
    !rubric.criteria.is_empty()
        && rubric
            .criteria
            .iter()
            .any(|criterion| criterion.levels.is_empty())
}

pub fn validate_rubric_levels(rubric: &RubricState) -> RubricValidationResult {
    let mut issues = Vec::new();
    let mut total_points = 0.0_f32;
    let mut has_levels = false;

    for criterion in &rubric.criteria {
        if criterion.levels.is_empty() {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_LEVELS_MISSING".to_string(),
                message: format!(
                    "{} kriteri için performans seviyeleri eksik.",
                    criterion.label
                ),
            });
            continue;
        }
        has_levels = true;
        let mut ids = std::collections::HashSet::new();
        for level in &criterion.levels {
            if !ids.insert(level.id.trim().to_string()) {
                issues.push(RubricValidationIssue {
                    code: "RUBRIC_LEVEL_ID_DUPLICATE".to_string(),
                    message: format!(
                        "{} kriterinde seviye kimliği tekrarlanıyor.",
                        criterion.label
                    ),
                });
            }
            if is_placeholder_text(&level.id)
                || is_placeholder_text(&level.title)
                || level
                    .required_conditions
                    .iter()
                    .chain(level.disqualifying_conditions.iter())
                    .any(|condition| is_placeholder_text(condition))
            {
                issues.push(RubricValidationIssue {
                    code: "RUBRIC_LEVEL_PLACEHOLDER_DETECTED".to_string(),
                    message: format!(
                        "{} kriterinde seviye placeholder içeriyor.",
                        criterion.label
                    ),
                });
            }
            if level.version.trim().is_empty() {
                issues.push(RubricValidationIssue {
                    code: "RUBRIC_LEVEL_VERSION_MISSING".to_string(),
                    message: format!("{} kriterinde seviye sürümü eksik.", criterion.label),
                });
            }
            if !level.score.is_finite()
                || level.score < 0.0
                || level.score > criterion.points + 0.01
            {
                issues.push(RubricValidationIssue {
                    code: "RUBRIC_LEVEL_SCORE_INVALID".to_string(),
                    message: format!("{} kriterinde seviye puanı geçersiz.", criterion.label),
                });
            }
            if level.evidence_required
                && level.required_conditions.is_empty()
                && level.disqualifying_conditions.is_empty()
            {
                issues.push(RubricValidationIssue {
                    code: "RUBRIC_LEVEL_EVIDENCE_POLICY_MISSING".to_string(),
                    message: format!("{} kriterinde kanıt koşulları eksik.", criterion.label),
                });
            }
            total_points += level.score;
        }
    }

    if !has_levels && !rubric.criteria.is_empty() {
        issues.push(RubricValidationIssue {
            code: "RUBRIC_LEVEL_MIGRATION_REQUIRED".to_string(),
            message: "Eski numeric rubrik için öğretmen seviyeleri oluşturulmalı.".to_string(),
        });
    }

    RubricValidationResult {
        valid: !issues.iter().any(|issue| {
            issue.code != "RUBRIC_LEVELS_MISSING" && issue.code != "RUBRIC_LEVEL_MIGRATION_REQUIRED"
        }),
        confirmable: false,
        warnings: if rubric_requires_level_migration(rubric) {
            vec!["Rubrik seviyeleri öğretmen onayı bekliyor.".to_string()]
        } else {
            vec![]
        },
        issues,
        total_points: Some(total_points),
    }
}

/// Builds a non-authoritative migration suggestion from the existing
/// criterion label/description. It never invents a model answer or marks the
/// rubric confirmed; the caller must persist it as `suggested` and request
/// teacher confirmation separately.
pub fn migrate_legacy_rubric_levels(rubric: &RubricState) -> RubricState {
    let mut migrated = rubric.clone();
    for criterion in &mut migrated.criteria {
        if criterion.levels.is_empty() {
            criterion.levels = vec![RubricLevel {
                id: format!("{}-legacy-level", criterion.id),
                title: criterion.label.clone(),
                required_conditions: vec![criterion.description.clone()],
                disqualifying_conditions: vec![],
                score: criterion.points,
                evidence_required: true,
                version: format!("{RUBRIC_LEVEL_SCHEMA_VERSION}:migration"),
            }];
        }
    }
    migrated.status = RubricStatus::Suggested;
    migrated
        .warnings
        .push("Rubrik seviyeleri öğretmen onayı bekliyor.".to_string());
    migrated
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

    for concept in &rubric.key_concepts {
        if is_placeholder_text(concept) {
            issues.push(RubricValidationIssue {
                code: "RUBRIC_PLACEHOLDER_DETECTED".to_string(),
                message: "Anahtar kavramlarda placeholder metin var.".to_string(),
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
            key_concepts: vec!["ana kavram".to_string()],
            criteria: vec![
                RubricCriterion {
                    id: "c1".to_string(),
                    label: "Konuya uygunluk".to_string(),
                    description: "Yanıtın konuya uyumu".to_string(),
                    points: 4.0,
                    levels: vec![],
                },
                RubricCriterion {
                    id: "c2".to_string(),
                    label: "Açıklık".to_string(),
                    description: "Cevabın açıklığı".to_string(),
                    points: 6.0,
                    levels: vec![],
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
            key_concepts: vec![],
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
    fn legacy_numeric_rubric_migration_is_suggested_and_preserves_points() {
        let legacy = sample_rubric();
        assert!(rubric_requires_level_migration(&legacy));

        let migrated = migrate_legacy_rubric_levels(&legacy);
        assert_eq!(migrated.status, RubricStatus::Suggested);
        assert_eq!(migrated.criteria.len(), legacy.criteria.len());
        assert!(migrated
            .criteria
            .iter()
            .zip(legacy.criteria.iter())
            .all(|(migrated, legacy)| {
                migrated.levels.len() == 1
                    && migrated.levels[0].score == legacy.points
                    && migrated.levels[0]
                        .version
                        .starts_with(RUBRIC_LEVEL_SCHEMA_VERSION)
            }));
        assert!(validate_rubric_levels(&migrated).issues.is_empty());
    }

    #[test]
    fn rubric_level_placeholders_are_rejected() {
        let mut rubric = sample_rubric();
        rubric.criteria[0].levels = vec![RubricLevel {
            id: "level-1".to_string(),
            title: "Beklenen cevabı yazın...".to_string(),
            required_conditions: vec!["Anahtar kavramı doğru kullanır.".to_string()],
            disqualifying_conditions: vec![],
            score: 4.0,
            evidence_required: true,
            version: RUBRIC_LEVEL_SCHEMA_VERSION.to_string(),
        }];

        let result = validate_rubric_levels(&rubric);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "RUBRIC_LEVEL_PLACEHOLDER_DETECTED"));
        assert!(!result.valid);
    }

    #[test]
    fn validation_flags_freeze_v1_required_fields() {
        let rubric = RubricState {
            status: RubricStatus::Imported,
            source: Some(RubricSource::Json),
            max_score: Some(0.0),
            expected_answer: None,
            key_concepts: vec![],
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

    #[test]
    fn extraction_contract_reflects_canonical_fields_and_backend_boundaries() {
        let fields = canonical_rubric_data_field_names();
        for field in [
            "maxScore",
            "expectedAnswer",
            "keyConcepts",
            "criteria",
            "partialCreditHints",
            "zeroScoreConditions",
            "commonMistakes",
            "warnings",
        ] {
            assert!(
                fields.iter().any(|candidate| candidate == field),
                "missing {field}"
            );
        }
        for backend_field in ["status", "source", "updatedAt"] {
            assert!(!fields.iter().any(|candidate| candidate == backend_field));
        }

        let schema = canonical_rubric_extraction_schema();
        let question_properties = schema
            .pointer("/properties/questions/items/properties")
            .and_then(Value::as_object)
            .expect("question properties");
        assert!(question_properties.contains_key("expectedAnswer"));
        assert!(question_properties.contains_key("keyConcepts"));
        assert!(question_properties.contains_key("partialCreditHints"));
        assert!(!question_properties.contains_key("status"));
        assert!(!question_properties.contains_key("source"));
        assert!(canonical_rubric_extraction_prompt().contains(RUBRIC_EXTRACTION_SCHEMA_VERSION));
    }
}

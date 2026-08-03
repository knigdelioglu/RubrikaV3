use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::domain::question::AnswerType;

/// The persisted/transport-safe answer representation.  The `kind` tag is
/// deliberately part of the contract so an OCR result cannot silently change
/// shape between OCR, review, and scoring.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StructuredAnswer {
    MultipleChoice {
        #[serde(default)]
        selections: Vec<MultipleChoiceSelection>,
    },
    Matching {
        #[serde(default)]
        pairs: Vec<MatchingPair>,
    },
    OrderedSlots {
        #[serde(default)]
        slots: Vec<OrderedSlot>,
    },
    Numeric {
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        unit: Option<String>,
    },
    Table {
        #[serde(default)]
        rows: Vec<StructuredTableRow>,
    },
    CorrectionTable {
        #[serde(default)]
        rows: Vec<CorrectionTableRow>,
    },
    SentenceAnnotation {
        #[serde(default)]
        annotations: Vec<SentenceAnnotationItem>,
    },
    GrammarAnalysis {
        #[serde(default)]
        items: Vec<GrammarAnalysisItem>,
    },
    #[serde(rename = "open_text", alias = "text")]
    OpenText {
        #[serde(default)]
        text: String,
    },
    /// Compatibility-only salvage for old arbitrary JSON records.  This is
    /// never accepted as a scoring-ready structured answer, but the original
    /// value remains available for review and migration diagnostics.
    LegacyUnparsed { raw: Value, reason: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MultipleChoiceSelection {
    #[serde(default)]
    pub option: String,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchingPair {
    #[serde(default)]
    pub left: String,
    #[serde(default)]
    pub right: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderedSlot {
    pub index: u32,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StructuredTableRow {
    pub index: u32,
    #[serde(default)]
    pub cells: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionTableRow {
    pub index: u32,
    #[serde(default)]
    pub original: String,
    #[serde(default)]
    pub correction: String,
    #[serde(default)]
    pub explanation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SentenceAnnotationItem {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub annotation: String,
    #[serde(default)]
    pub start: Option<usize>,
    #[serde(default)]
    pub end: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrammarAnalysisItem {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuredAnswerParseError {
    pub message: String,
}

impl std::fmt::Display for StructuredAnswerParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StructuredAnswerParseError {}

/// Deserialize old records without dropping arbitrary JSON.  Valid legacy
/// shapes are normalized into the tagged enum; unknown shapes become the
/// review-only `LegacyUnparsed` variant.
pub fn deserialize_compat<'de, D>(deserializer: D) -> Result<Option<StructuredAnswer>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.map(|value| match parse_legacy_value(&value) {
        Ok(answer) => answer,
        Err(error) => StructuredAnswer::LegacyUnparsed {
            raw: value,
            reason: error.message,
        },
    }))
}

pub fn parse_for_answer_type(
    answer_type: &AnswerType,
    value: Value,
) -> Result<StructuredAnswer, StructuredAnswerParseError> {
    let answer = parse_legacy_value(&value)?;
    validate_for_answer_type(answer_type, &answer)?;
    Ok(answer)
}

/// Parses a legacy/model answer for review without applying the answer-type
/// or placeholder gates.  This is used only to preserve invalid data for
/// teacher review; callers must keep it out of scoring readiness.
pub fn parse_legacy_for_review(
    value: Value,
) -> Result<StructuredAnswer, StructuredAnswerParseError> {
    parse_legacy_value(&value)
}

pub fn validate_for_answer_type(
    answer_type: &AnswerType,
    answer: &StructuredAnswer,
) -> Result<(), StructuredAnswerParseError> {
    if matches!(answer, StructuredAnswer::LegacyUnparsed { .. }) {
        return Err(StructuredAnswerParseError {
            message: "structured_answer_legacy_unparsed".to_string(),
        });
    }

    let valid = match answer_type {
        AnswerType::GeneralText | AnswerType::ShortText | AnswerType::Essay => {
            matches!(answer, StructuredAnswer::OpenText { .. })
        }
        AnswerType::Table | AnswerType::DiagramLabeling => {
            matches!(answer, StructuredAnswer::Table { .. })
        }
        AnswerType::CorrectionTable => matches!(answer, StructuredAnswer::CorrectionTable { .. }),
        AnswerType::FillBlank | AnswerType::Ordering => {
            matches!(answer, StructuredAnswer::OrderedSlots { .. })
        }
        AnswerType::Matching => matches!(answer, StructuredAnswer::Matching { .. }),
        AnswerType::MultipleChoice | AnswerType::TrueFalse => {
            matches!(answer, StructuredAnswer::MultipleChoice { .. })
        }
        AnswerType::Numeric => matches!(answer, StructuredAnswer::Numeric { .. }),
        AnswerType::SentenceAnnotation => {
            matches!(answer, StructuredAnswer::SentenceAnnotation { .. })
        }
        AnswerType::GrammarAnalysis => matches!(answer, StructuredAnswer::GrammarAnalysis { .. }),
    };
    if !valid {
        return Err(StructuredAnswerParseError {
            message: format!(
                "structured_answer_mismatched_answer_type:{}",
                answer_type_label(answer_type)
            ),
        });
    }

    if answer_contains_placeholder(answer) {
        return Err(StructuredAnswerParseError {
            message: "structured_answer_placeholder_detected".to_string(),
        });
    }
    Ok(())
}

pub fn answer_type_label(answer_type: &AnswerType) -> &'static str {
    match answer_type {
        AnswerType::GeneralText => "general_text",
        AnswerType::ShortText => "short_text",
        AnswerType::Essay => "essay",
        AnswerType::Table => "table",
        AnswerType::CorrectionTable => "correction_table",
        AnswerType::FillBlank => "fill_blank",
        AnswerType::Matching => "matching",
        AnswerType::MultipleChoice => "multiple_choice",
        AnswerType::TrueFalse => "true_false",
        AnswerType::Ordering => "ordering",
        AnswerType::Numeric => "numeric",
        AnswerType::DiagramLabeling => "diagram_labeling",
        AnswerType::SentenceAnnotation => "sentence_annotation",
        AnswerType::GrammarAnalysis => "grammar_analysis",
    }
}

fn parse_legacy_value(value: &Value) -> Result<StructuredAnswer, StructuredAnswerParseError> {
    if let Ok(answer) = serde_json::from_value::<StructuredAnswer>(value.clone()) {
        return Ok(answer);
    }

    let object = value
        .as_object()
        .ok_or_else(|| StructuredAnswerParseError {
            message: "structured_answer_not_an_object".to_string(),
        })?;
    let kind = string_field(object, &["kind", "type", "answerType", "answer_type"])
        .unwrap_or_default()
        .to_ascii_lowercase();

    match kind.as_str() {
        "text" | "open_text" | "open-text" | "general_text" | "short_text" | "essay" => {
            Ok(StructuredAnswer::OpenText {
                text: string_field(
                    object,
                    &["text", "value", "answerText", "answer_text", "answer"],
                )
                .unwrap_or_default(),
            })
        }
        "numeric" | "number" => Ok(StructuredAnswer::Numeric {
            value: string_field(object, &["value", "number", "answer"]),
            unit: string_field(object, &["unit"]),
        }),
        "multiple_choice" | "multiple-choice" | "true_false" => {
            let selections = array_field(object, &["selections", "options", "choices"])
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    if let Some(text) = item.as_str() {
                        return Some(MultipleChoiceSelection {
                            option: text.to_string(),
                            selected: true,
                        });
                    }
                    let item = item.as_object()?;
                    Some(MultipleChoiceSelection {
                        option: string_field(item, &["option", "label", "value"])
                            .unwrap_or_else(|| (index + 1).to_string()),
                        selected: item
                            .get("selected")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    })
                })
                .collect();
            Ok(StructuredAnswer::MultipleChoice { selections })
        }
        "matching" => Ok(StructuredAnswer::Matching {
            pairs: array_field(object, &["pairs", "matches", "items"])
                .iter()
                .filter_map(|item| {
                    let item = item.as_object()?;
                    Some(MatchingPair {
                        left: string_field(item, &["left", "prompt", "from"]).unwrap_or_default(),
                        right: string_field(item, &["right", "answer", "to"]).unwrap_or_default(),
                    })
                })
                .collect(),
        }),
        "ordered_slots" | "ordering" | "fill_blank" | "fill_blanks" => {
            Ok(StructuredAnswer::OrderedSlots {
                slots: array_field(object, &["slots", "items", "answers"])
                    .iter()
                    .enumerate()
                    .filter_map(|(index, item)| {
                        let value = item.as_str().map(ToString::to_string).or_else(|| {
                            item.as_object()
                                .and_then(|item| string_field(item, &["value", "answer", "text"]))
                        })?;
                        let item_index =
                            item.as_object()
                                .and_then(|item| item.get("index"))
                                .and_then(Value::as_u64)
                                .unwrap_or(index as u64) as u32;
                        Some(OrderedSlot {
                            index: item_index,
                            value,
                        })
                    })
                    .collect(),
            })
        }
        "table" | "diagram_labeling" => Ok(StructuredAnswer::Table {
            rows: array_field(object, &["rows", "items"])
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    let item = item.as_object()?;
                    let cells = array_field(item, &["cells", "values"])
                        .iter()
                        .filter_map(|cell| cell.as_str().map(ToString::to_string))
                        .collect();
                    Some(StructuredTableRow {
                        index: item
                            .get("index")
                            .and_then(Value::as_u64)
                            .unwrap_or(index as u64) as u32,
                        cells,
                    })
                })
                .collect(),
        }),
        "correction_table" | "corrections" => Ok(StructuredAnswer::CorrectionTable {
            rows: array_field(object, &["rows", "items"])
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    let item = item.as_object()?;
                    Some(CorrectionTableRow {
                        index: item
                            .get("index")
                            .and_then(Value::as_u64)
                            .unwrap_or(index as u64) as u32,
                        original: string_field(item, &["original", "source", "text"])
                            .unwrap_or_default(),
                        correction: string_field(item, &["correction", "corrected", "answer"])
                            .unwrap_or_default(),
                        explanation: string_field(item, &["explanation", "reason"]),
                    })
                })
                .collect(),
        }),
        "sentence_annotation" | "annotation" => Ok(StructuredAnswer::SentenceAnnotation {
            annotations: array_field(object, &["annotations", "items"])
                .iter()
                .filter_map(|item| {
                    let item = item.as_object()?;
                    Some(SentenceAnnotationItem {
                        text: string_field(item, &["text", "span"]).unwrap_or_default(),
                        annotation: string_field(item, &["annotation", "label", "type"])
                            .unwrap_or_default(),
                        start: item
                            .get("start")
                            .and_then(Value::as_u64)
                            .map(|v| v as usize),
                        end: item.get("end").and_then(Value::as_u64).map(|v| v as usize),
                    })
                })
                .collect(),
        }),
        "grammar_analysis" | "grammar" => Ok(StructuredAnswer::GrammarAnalysis {
            items: array_field(object, &["items", "analyses", "rows"])
                .iter()
                .filter_map(|item| {
                    let item = item.as_object()?;
                    Some(GrammarAnalysisItem {
                        text: string_field(item, &["text", "token"]).unwrap_or_default(),
                        label: string_field(item, &["label", "category", "type"])
                            .unwrap_or_default(),
                        explanation: string_field(item, &["explanation", "reason"]),
                    })
                })
                .collect(),
        }),
        _ => Err(StructuredAnswerParseError {
            message: "structured_answer_kind_unknown".to_string(),
        }),
    }
}

fn string_field(object: &Map<String, Value>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(Value::as_str))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn array_field<'a>(object: &'a Map<String, Value>, names: &[&str]) -> &'a [Value] {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(Value::as_array))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn answer_contains_placeholder(answer: &StructuredAnswer) -> bool {
    let values = match answer {
        StructuredAnswer::MultipleChoice { selections } => selections
            .iter()
            .map(|item| item.option.as_str())
            .collect::<Vec<_>>(),
        StructuredAnswer::Matching { pairs } => pairs
            .iter()
            .flat_map(|item| [item.left.as_str(), item.right.as_str()])
            .collect::<Vec<_>>(),
        StructuredAnswer::OrderedSlots { slots } => slots
            .iter()
            .map(|item| item.value.as_str())
            .collect::<Vec<_>>(),
        StructuredAnswer::Numeric { value, unit } => value
            .iter()
            .chain(unit.iter())
            .map(String::as_str)
            .collect::<Vec<_>>(),
        StructuredAnswer::Table { rows } => rows
            .iter()
            .flat_map(|row| row.cells.iter().map(String::as_str))
            .collect::<Vec<_>>(),
        StructuredAnswer::CorrectionTable { rows } => rows
            .iter()
            .flat_map(|row| {
                [row.original.as_str(), row.correction.as_str()]
                    .into_iter()
                    .chain(row.explanation.as_deref())
            })
            .collect::<Vec<_>>(),
        StructuredAnswer::SentenceAnnotation { annotations } => annotations
            .iter()
            .flat_map(|item| [item.text.as_str(), item.annotation.as_str()])
            .collect::<Vec<_>>(),
        StructuredAnswer::GrammarAnalysis { items } => items
            .iter()
            .flat_map(|item| [item.text.as_str(), item.label.as_str()])
            .collect::<Vec<_>>(),
        StructuredAnswer::OpenText { text } => vec![text.as_str()],
        StructuredAnswer::LegacyUnparsed { .. } => return true,
    };
    values
        .iter()
        .any(|value| crate::domain::rubric::is_placeholder_text(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::question::AnswerType;

    #[test]
    fn parses_and_validates_tagged_open_text() {
        let value = serde_json::json!({"kind":"open_text","text":"cevap"});
        let parsed = parse_for_answer_type(&AnswerType::GeneralText, value).unwrap();
        assert!(matches!(parsed, StructuredAnswer::OpenText { .. }));
    }

    #[test]
    fn rejects_mismatched_schema_without_turning_it_into_a_score() {
        let value = serde_json::json!({"kind":"numeric","value":"4"});
        let error = parse_for_answer_type(&AnswerType::Essay, value).unwrap_err();
        assert!(error.message.contains("mismatched"));
    }

    #[test]
    fn salvages_unknown_legacy_json_without_data_loss() {
        let value = serde_json::json!({"oldField":"keep me"});
        let answer = deserialize_compat_value(value.clone());
        assert!(matches!(answer, StructuredAnswer::LegacyUnparsed { raw, .. } if raw == value));
    }

    fn deserialize_compat_value(value: Value) -> StructuredAnswer {
        parse_legacy_value(&value).unwrap_or_else(|error| StructuredAnswer::LegacyUnparsed {
            raw: value,
            reason: error.message,
        })
    }
}

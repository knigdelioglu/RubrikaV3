use super::rubric::RubricState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnswerType {
    GeneralText,
    ShortText,
    Essay,
    Table,
    CorrectionTable,
    FillBlank,
    Matching,
    MultipleChoice,
    TrueFalse,
    Ordering,
    Numeric,
    DiagramLabeling,
    SentenceAnnotation,
    GrammarAnalysis,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TextFieldSource {
    Manual,
    ExamPdf,
    StudentPdf,
    ImportedTemplate,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TextFieldStatus {
    Missing,
    Suggested,
    Confirmed,
    Edited,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextFieldState {
    pub value: String,
    pub source: TextFieldSource,
    pub status: TextFieldStatus,
    pub confidence: Option<f32>,
    pub warnings: Vec<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CropTemplate {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub page_index: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub number: u32,
    pub max_score: f32,
    pub answer_type: AnswerType,
    pub question_text: TextFieldState,
    pub rubric: RubricState,
    pub crop_template: Option<CropTemplate>,
}

pub fn is_placeholder_text(text: &str) -> bool {
    super::rubric::is_placeholder_text(text)
}

pub fn is_question_text_ready(state: &TextFieldState) -> bool {
    if state.status == TextFieldStatus::Missing
        || state.status == TextFieldStatus::Failed
        || state.status == TextFieldStatus::Suggested
    {
        return false;
    }
    if is_placeholder_text(&state.value) {
        return false;
    }
    true
}

pub fn default_question(number: u32) -> Question {
    Question {
        id: Uuid::new_v4().to_string(),
        number,
        max_score: 0.0,
        answer_type: AnswerType::GeneralText,
        question_text: TextFieldState {
            value: String::new(),
            source: TextFieldSource::Unknown,
            status: TextFieldStatus::Missing,
            confidence: None,
            warnings: vec![],
            updated_at: None,
        },
        rubric: RubricState {
            status: super::rubric::RubricStatus::Missing,
            source: None,
            max_score: None,
            expected_answer: None,
            criteria: vec![],
            partial_credit_hints: vec![],
            zero_score_conditions: vec![],
            common_mistakes: vec![],
            warnings: vec![],
            updated_at: None,
        },
        crop_template: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_text_detection() {
        assert!(is_placeholder_text(""));
        assert!(is_placeholder_text("   "));
        assert!(is_placeholder_text("Anahtar kavramları girin..."));
        assert!(is_placeholder_text("kelime1, kelime2, kelime3"));
        assert!(!is_placeholder_text("This is a real question text"));
    }

    #[test]
    fn test_question_text_readiness() {
        let mut state = TextFieldState {
            value: "Some real text".to_string(),
            source: TextFieldSource::Manual,
            status: TextFieldStatus::Confirmed,
            confidence: None,
            warnings: vec![],
            updated_at: None,
        };
        assert!(is_question_text_ready(&state));

        state.status = TextFieldStatus::Suggested;
        assert!(!is_question_text_ready(&state));

        state.status = TextFieldStatus::Confirmed;
        state.value = "Anahtar kavramları girin...".to_string();
        assert!(!is_question_text_ready(&state));
    }
}

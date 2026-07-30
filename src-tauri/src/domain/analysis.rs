use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentKind {
    Written,
    Speaking,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Generating,
    Ready,
    Partial,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisCriterionSummary {
    pub id: String,
    pub label: String,
    pub average_score: f32,
    pub max_score: f32,
    pub percentage: f32,
    pub sample_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStudentSummary {
    pub student_id: String,
    pub display_name: String,
    pub score: f32,
    pub max_score: f32,
    pub percentage: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisScoreBand {
    pub label: String,
    pub minimum: f32,
    pub maximum: f32,
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentAnalysis {
    pub id: String,
    pub project_id: String,
    pub kind: AssessmentKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_id: Option<String>,
    pub status: AnalysisStatus,
    pub student_count: u32,
    pub criteria: Vec<AnalysisCriterionSummary>,
    pub students: Vec<AnalysisStudentSummary>,
    pub score_bands: Vec<AnalysisScoreBand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_report: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_report_error: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

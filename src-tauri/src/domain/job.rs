use crate::domain::errors::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    QuestionTextExtraction,
    PdfPreviewRender,
    RubricPdfImport,
    ExamPackageBuild,
    StudentAnswerOcr,
    StudentIdentityOcr,
    Scoring,
    SpeakingEvaluation,
    AssessmentAnalysis,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Partial,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub current: u32,
    pub total: u32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub id: String,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root_path: Option<String>,
    pub kind: JobKind,
    pub status: JobStatus,
    pub progress: JobProgress,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    pub result: Option<serde_json::Value>,
    pub error: Option<AppError>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobStartedEvent {
    pub job_id: String,
    pub kind: JobKind,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressEvent {
    pub job_id: String,
    pub current: u32,
    pub total: u32,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobSucceededEvent {
    pub job_id: String,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobFailedEvent {
    pub job_id: String,
    pub error: AppError,
}

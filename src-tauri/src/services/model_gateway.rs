use async_trait::async_trait;

use crate::domain::errors::AppError;
use crate::domain::model::{
    AnalysisReportRequest, AnalysisReportResult, ModelStatus, QuestionTextExtractionRequest,
    QuestionTextExtractionResult, RubricExtractionRequest, RubricExtractionResult, ScoringRequest,
    ScoringResult, SpeakingTranscriptCleanupRequest, SpeakingTranscriptCleanupResult,
    StudentAnswerOcrIssueCorrectionRequest, StudentAnswerOcrIssueCorrectionResult,
    StudentAnswerOcrRequest, StudentAnswerOcrResult, StudentIdentityOcrRequest,
    StudentIdentityOcrResult,
};

#[async_trait]
pub trait ModelGateway: Send + Sync {
    async fn get_status(&self) -> Result<ModelStatus, AppError>;
    async fn probe_server(&self) -> Result<ModelStatus, AppError>;
    async fn health_status(&self, base_url: &str) -> Result<ModelStatus, AppError>;
    async fn probe_status(&self, base_url: &str) -> Result<ModelStatus, AppError>;
    async fn extract_question_text(
        &self,
        input: QuestionTextExtractionRequest,
    ) -> Result<QuestionTextExtractionResult, AppError>;
    async fn draft_rubric(
        &self,
        input: RubricExtractionRequest,
    ) -> Result<RubricExtractionResult, AppError>;
    async fn extract_student_answer_ocr(
        &self,
        input: StudentAnswerOcrRequest,
    ) -> Result<StudentAnswerOcrResult, AppError>;
    async fn suggest_student_answer_issue_correction(
        &self,
        input: StudentAnswerOcrIssueCorrectionRequest,
    ) -> Result<StudentAnswerOcrIssueCorrectionResult, AppError>;
    async fn extract_student_identity_ocr(
        &self,
        input: StudentIdentityOcrRequest,
    ) -> Result<StudentIdentityOcrResult, AppError>;
    async fn cleanup_speaking_transcript(
        &self,
        input: SpeakingTranscriptCleanupRequest,
    ) -> Result<SpeakingTranscriptCleanupResult, AppError>;
    async fn generate_analysis_report(
        &self,
        input: AnalysisReportRequest,
    ) -> Result<AnalysisReportResult, AppError>;
    async fn score_answer(&self, input: ScoringRequest) -> Result<ScoringResult, AppError>;
}

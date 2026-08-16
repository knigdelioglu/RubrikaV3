use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{
    AnalysisReportRequest, AnalysisReportResult, ModelStatus, QuestionTextExtractionRequest,
    QuestionTextExtractionResult, RubricExtractionRequest, RubricExtractionResult, ScoringRequest,
    ScoringResult, SpeakingTranscriptCleanupRequest, SpeakingTranscriptCleanupResult,
    StudentAnswerOcrIssueCorrectionRequest, StudentAnswerOcrIssueCorrectionResult,
    StudentAnswerOcrRequest, StudentAnswerOcrResult, StudentIdentityOcrRequest,
    StudentIdentityOcrResult,
};
use crate::jobs::job_manager::JobManager;
use crate::services::cancellable_wait::await_with_cancellation;
use crate::services::model_gateway::ModelGateway;

/// Production gateway decorator that makes long student-answer OCR model calls
/// observe the owning job's cancellation token while the HTTP future is still
/// in flight. Other model operations retain their existing behavior.
#[derive(Clone)]
pub struct CancellableModelGateway {
    inner: Arc<dyn ModelGateway>,
    job_manager: Arc<JobManager>,
}

impl CancellableModelGateway {
    pub fn new(inner: Arc<dyn ModelGateway>, job_manager: Arc<JobManager>) -> Self {
        Self { inner, job_manager }
    }
}

#[async_trait]
impl ModelGateway for CancellableModelGateway {
    async fn get_status(&self) -> Result<ModelStatus, AppError> {
        self.inner.get_status().await
    }

    async fn probe_server(&self) -> Result<ModelStatus, AppError> {
        self.inner.probe_server().await
    }

    async fn health_status(&self, base_url: &str) -> Result<ModelStatus, AppError> {
        self.inner.health_status(base_url).await
    }

    async fn probe_status(&self, base_url: &str) -> Result<ModelStatus, AppError> {
        self.inner.probe_status(base_url).await
    }

    async fn extract_question_text(
        &self,
        input: QuestionTextExtractionRequest,
    ) -> Result<QuestionTextExtractionResult, AppError> {
        self.inner.extract_question_text(input).await
    }

    async fn draft_rubric(
        &self,
        input: RubricExtractionRequest,
    ) -> Result<RubricExtractionResult, AppError> {
        self.inner.draft_rubric(input).await
    }

    async fn extract_student_answer_ocr(
        &self,
        input: StudentAnswerOcrRequest,
    ) -> Result<StudentAnswerOcrResult, AppError> {
        let job_id = input.job_id.clone();
        let token = job_id
            .as_deref()
            .and_then(|job_id| self.job_manager.get_cancellation_token(job_id));

        match await_with_cancellation(token.as_ref(), self.inner.extract_student_answer_ocr(input))
            .await
        {
            Some(result) => result,
            None => Err(AppError {
                // StudentAnswerOcrService treats this family as a soft model
                // result and immediately reaches its existing cancellation
                // checkpoint, which transitions the job to Cancelled without
                // committing the dropped model result.
                code: AppErrorCode::ModelResponseEmpty,
                message: "OCR model çağrısı kullanıcı iptali nedeniyle durduruldu.".to_string(),
                recoverable: true,
                suggested_action: None,
                technical_details: Some("student_answer_ocr_inflight_cancelled".to_string()),
                correlation_id: job_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            }),
        }
    }

    async fn suggest_student_answer_issue_correction(
        &self,
        input: StudentAnswerOcrIssueCorrectionRequest,
    ) -> Result<StudentAnswerOcrIssueCorrectionResult, AppError> {
        self.inner
            .suggest_student_answer_issue_correction(input)
            .await
    }

    async fn extract_student_identity_ocr(
        &self,
        input: StudentIdentityOcrRequest,
    ) -> Result<StudentIdentityOcrResult, AppError> {
        self.inner.extract_student_identity_ocr(input).await
    }

    async fn cleanup_speaking_transcript(
        &self,
        input: SpeakingTranscriptCleanupRequest,
    ) -> Result<SpeakingTranscriptCleanupResult, AppError> {
        self.inner.cleanup_speaking_transcript(input).await
    }

    async fn generate_analysis_report(
        &self,
        input: AnalysisReportRequest,
    ) -> Result<AnalysisReportResult, AppError> {
        self.inner.generate_analysis_report(input).await
    }

    async fn score_answer(&self, input: ScoringRequest) -> Result<ScoringResult, AppError> {
        self.inner.score_answer(input).await
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppErrorCode {
    UnknownError,
    ProjectNotFound,
    ProjectLoadFailed,
    ProjectSaveFailed,
    DocumentImportFailed,
    DocumentNotFound,
    PdfDocumentNotFound,
    PdfPageCountFailed,
    PdfRenderFailed,
    PdfPreviewNotFound,
    PdfPreviewNotReady,
    PdfPreviewJobFailed,
    PdfUnsupportedFormat,
    PdfRendererNotFound,
    PdfRendererStartFailed,
    PdfRendererPermissionDenied,
    PdfRendererOutputMissing,
    PdfRendererUnsupported,
    FileReadFailed,
    FileWriteFailed,
    CropRegionMissing,
    WorkflowBlocked,
    ModelServerNotRunning,
    ModelConfigMissing,
    ModelBinaryMissing,
    ModelFileMissing,
    ModelMmprojMissing,
    ModelPortBlocked,
    ModelProfileNotFound,
    ModelProfileNotManaged,
    ModelServerPathMissing,
    ModelModelPathMissing,
    ModelMmprojPathMissing,
    ModelPortAlreadyInUse,
    ModelStartFailed,
    ModelStartTimeout,
    ModelServerStartFailed,
    ModelServerReadyTimeout,
    ModelServerStopFailed,
    ModelServerUnsupportedFlags,
    ModelServerNotStartedByApp,
    ModelStateAccessFailed,
    ModelHealthFailed,
    ModelPrivacyBlocked,
    ModelExternalConsentRequired,
    ModelRedirectRejected,
    ModelTimeout,
    ModelResponseEmpty,
    ModelResponseInvalidJson,
    ModelResponseInvalidSchema,
    ModelResponseReasoningOnly,
    ModelResponseTooLarge,
    ModelResponseTruncated,
    ModelResponseInvalidContentType,
    ModelRequestTooLarge,
    OcrFailed,
    ScoringFailed,
    QepNotFrozen,
    RubricMissing,
    QuestionTextMissing,
    QuestionTextExtractionFailed,
    QuestionTextSuggestionNotFound,
    QuestionTextConfirmFailed,
    QuestionTextPartialSuccess,
    JobStaleInterrupted,
    RubricJsonInvalid,
    RubricJsonParseFailed,
    RubricJsonSchemaUnsupported,
    RubricSchemaValidationFailed,
    RubricQuestionNotFound,
    RubricPlaceholderDetected,
    RubricEmptyContent,
    RubricImportEmpty,
    RubricMaxScoreMissing,
    RubricCriteriaScoreMismatch,
    RubricConfirmFailed,
    RubricNotReady,
    ExamSourcePdfMissing,
    RubricDocumentMissing,
    QuestionCountMissing,
    ExamPackageBuildPrecheckFailed,
    QuestionCoverageIncomplete,
    QuestionLastItemMissing,
    StudentScanNotFound,
    StudentScanPreviewNotReady,
    StudentGroupingNotReady,
    StudentGroupingInvalid,
    StudentSubmissionNotFound,
    StudentSubmissionInUse,
    SubmissionDeleteConflict,
    StudentIdentityInvalid,
    SchoolClassNotFound,
    SchoolClassNameInvalid,
    SchoolClassAlreadyExists,
    SchoolClassArchived,
    TeachingAssignmentNotFound,
    TeachingAssignmentAlreadyExists,
    TeachingAssignmentInvalid,
    StudentScanBatchNotFound,
    StudentScanBatchAlreadyExists,
    StudentScanBatchInUse,
    OcrNotReady,
    ScoringNotReady,
    ScoringAnchorNotEligible,
    ScoringAnchorNotFound,
    ScoringAnchorAlreadyExists,
    ScoringAnchorAlreadyRevoked,
    ScoringRerunRequired,
    JobNotFound,
    JobAlreadyTerminal,
    JobAlreadyRunning,
    JobNotCancellable,
    JobCannotBeCancelledNow,
    JobCancellationAlreadyRequested,
    JobCancelled,
    JobPersistenceFailed,
    JobStateTransitionInvalid,
    JobExecutionOwnerLost,
    JobDuplicateConflict,
    JobRetryRejected,
    JobInputStale,
    JobPersistenceCorrupt,
    AuditWriteFailed,
    AuditChainInvalid,
    BackupFailed,
    BackupArchiveInvalid,
    BackupCancelled,
    RestoreFailed,
    RestoreDestinationConflict,
    RestoreCancelled,
    PermissionDenied,
    AppAlreadyRunning,
    FeatureNotImplemented,
    ModelRequestTimeout,
    ModelOutputRetryFailed,
    ModelServerLostDuringRequest,
    ModelServerCrashedDuringRequest,
    ModelConnectionReset,
    ModelProcessIdentityMismatch,
    ModelProcessUnverified,
    ModelRuntimeInUse,
    ModelRuntimeDraining,
    ModelRuntimeProfileBusy,
    ModelRuntimeStartFailed,
    ModelRuntimeReadinessTimeout,
    ModelRuntimeExited,
    ModelRuntimeLeaseInvalid,
    ModelRuntimeLeaseAlreadyReleased,
    ModelRuntimePortOccupied,
    SpeakingEngineNotFound,
    SpeakingEngineLaunchFailed,
    SpeakingEngineUnsupported,
    SpeakingAttemptNotFound,
    SpeakingCaptureBusy,
    SpeakingTranscriptNotReady,
    SpeakingAudioMissing,
    SpeakingEvaluationFailed,
    SpeakingReviewIncomplete,
    AnalysisNotReady,
    AnalysisFailed,
    StudentNotFound,
    AssessmentActivityAlreadyExists,
    AssessmentActivityNotFound,
    AssessmentActivityInUse,
    AssessmentClassApplicationNotFound,
    AssessmentClassApplicationAlreadyExists,
    AssessmentClassApplicationInUse,
    AssessmentClassLevelMismatch,
    AssessmentClassNotEligible,
    AssessmentDocumentNotFound,
    AssessmentInvalidInput,
    ProjectRootMismatch,
    ProjectAlreadyExists,
    ProjectAlreadyOpen,
    ProjectWriteLeaseMissing,
    ProjectDirectoryNotEmpty,
    UnsafeManagedPath,
    ManagedPathOutsideProject,
    ManagedPathSymlinkEscape,
    LegacyDocumentPathUnresolved,
    ProjectRevisionConflict,
    ProjectExternallyModified,
    ProjectMutationConflict,
    ProjectEntityStale,
    ProjectEntityNotFound,
    ProjectMutationRejected,
    ProjectMigrationRequired,
    CommitDurabilityUncertain,
    CommitDurabilityUnsupported,
    AuditCommitIncomplete,
    IncompleteTransaction,
    VerifiedBackupRequired,
    OcrRerunCandidateFailed,
    OcrGenerationStale,
    OcrGenerationConflict,
    PreviewGenerationFailed,
    PreviewGenerationStale,
    PreviewActiveGenerationMissing,
    GenerationStillReferenced,
    GenerationGcFailed,
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub recoverable: bool,
    pub suggested_action: Option<String>,
    pub technical_details: Option<String>,
    pub correlation_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppErrorInternal {
    code: AppErrorCode,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    safe_message: Option<String>,
    #[serde(default)]
    recoverable: Option<bool>,
    #[serde(default)]
    retryable: Option<bool>,
    #[serde(default)]
    suggested_action: Option<String>,
    #[serde(default)]
    recovery_action: Option<String>,
    #[serde(default)]
    technical_details: Option<String>,
    #[serde(default)]
    correlation_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    details_available: Option<bool>,
}

impl<'de> Deserialize<'de> for AppError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let internal = AppErrorInternal::deserialize(deserializer)?;
        Ok(AppError {
            code: internal.code,
            message: internal
                .safe_message
                .or(internal.message)
                .unwrap_or_else(|| "Bilinmeyen hata.".to_string()),
            recoverable: internal.retryable.or(internal.recoverable).unwrap_or(false),
            suggested_action: internal.recovery_action.or(internal.suggested_action),
            technical_details: internal.technical_details,
            correlation_id: internal.correlation_id.unwrap_or_default(),
        })
    }
}

/// The only error shape that may cross the Tauri command boundary.
///
/// Internal technical details, raw paths, SQL/serde/HTTP payloads and
/// student content are never serialized here. `details_available` only
/// records that redacted technical context exists in the diagnostics
/// layer for the given correlation id.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublicErrorDto {
    pub code: AppErrorCode,
    pub safe_message: String,
    pub recovery_action: Option<String>,
    pub correlation_id: String,
    pub retryable: bool,
    pub details_available: bool,
}

impl AppError {
    pub fn to_public(&self) -> PublicErrorDto {
        PublicErrorDto {
            code: self.code.clone(),
            safe_message: self.message.clone(),
            recovery_action: self.suggested_action.clone(),
            correlation_id: self.correlation_id.clone(),
            retryable: self.recoverable,
            details_available: self.technical_details.is_some(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_public().serialize(serializer)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_serialization_never_leaks_internal_details() {
        let error = AppError {
            code: AppErrorCode::ProjectNotFound,
            message: "Proje bulunamadı.".to_string(),
            recoverable: false,
            suggested_action: None,
            technical_details: Some(
                "/Users/kadir/Documents/secret-project/project.json: serde error".to_string(),
            ),
            correlation_id: "test-id".to_string(),
        };
        let serialized = serde_json::to_string(&error).unwrap();
        assert!(serialized.contains("PROJECT_NOT_FOUND"));
        assert!(serialized.contains("Proje bulunamadı."));
        assert!(serialized.contains("test-id"));
        // Internal payload must never cross the boundary.
        assert!(!serialized.contains("/Users/kadir"));
        assert!(!serialized.contains("serde error"));
        assert!(!serialized.contains("technicalDetails"));
        assert!(serialized.contains("detailsAvailable"));
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["detailsAvailable"], serde_json::Value::Bool(true));
    }

    #[test]
    fn test_app_error_public_dto_shape_is_stable() {
        let error = AppError {
            code: AppErrorCode::ModelTimeout,
            message: "Model yanıt vermedi.".to_string(),
            recoverable: true,
            suggested_action: Some("Tekrar deneyin.".to_string()),
            technical_details: Some("raw=http://127.0.0.1:8080/v1/chat/completions".to_string()),
            correlation_id: "corr-1".to_string(),
        };
        let public = error.to_public();
        assert_eq!(public.code, AppErrorCode::ModelTimeout);
        assert_eq!(public.safe_message, "Model yanıt vermedi.");
        assert_eq!(public.recovery_action.as_deref(), Some("Tekrar deneyin."));
        assert_eq!(public.correlation_id, "corr-1");
        assert!(public.retryable);
        assert!(public.details_available);
    }

    #[test]
    fn test_app_error_deserializes_public_shape_after_roundtrip() {
        let error = AppError {
            code: AppErrorCode::ProjectLoadFailed,
            message: "Proje yüklenemedi.".to_string(),
            recoverable: true,
            suggested_action: None,
            technical_details: Some("internal".to_string()),
            correlation_id: "corr-2".to_string(),
        };
        let serialized = serde_json::to_string(&error).unwrap();
        let restored: AppError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.code, error.code);
        assert_eq!(restored.message, error.message);
        assert_eq!(restored.correlation_id, "corr-2");
        assert!(restored.technical_details.is_none());
    }
}

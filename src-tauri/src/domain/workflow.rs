use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStage {
    DocumentsMissing,
    PdfPreviewMissing,
    PdfPreviewReady,
    PdfPreviewReadyQuestionTextMissing,
    ExamPackageBuildReady,
    ExamPackageBuildRunning,
    ExamPackageReviewNeeded,
    ExamPackageIncomplete,
    ExamPackageReadyForQep,
    QuestionTextMissing,
    QuestionTextExtractionRunning,
    QuestionTextSuggested,
    QuestionTextConfirmed,
    RubricMissing,
    RubricSuggested,
    RubricImportedNeedsReview,
    RubricInvalid,
    RubricConfirmed,
    StudentScansMissing,
    StudentScanPreviewMissing,
    StudentGroupingMissing,
    StudentGroupingReady,
    CropMissing,
    OcrReady,
    OcrRunning,
    ReviewRequired,
    StudentAnswerOcrRunning,
    StudentAnswerOcrReviewNeeded,
    StudentAnswerOcrReadyForScoring,
    QepMissing,
    QepReady,
    QepFrozen,
    ScoringReady,
    ScoringRunning,
    ScoringDone,
    AnalysisReady,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockingReason {
    ExamSourceMissing,
    ExamSourcePdfMissing,
    RubricDocumentMissing,
    QuestionCountMissing,
    ExamPackageBuildPrecheckFailed,
    PdfPreviewMissing,
    QuestionTextMissing,
    RubricMissing,
    RubricInvalid,
    CropMissing,
    PlaceholderDataDetected,
    ReviewRequired,
    QepNotFrozen,
    StudentScanNotFound,
    StudentScanPreviewNotReady,
    StudentGroupingNotReady,
    StudentGroupingInvalid,
    StudentSubmissionNotFound,
    StudentIdentityInvalid,
    OcrNotReady,
    ModelServerNotRunning,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAction {
    pub code: String,
    pub label: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStep {
    pub code: String,
    pub label: String,
    pub status: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowReadiness {
    pub exam_package_freeze: bool,
    pub student_intake: bool,
    pub scoring: bool,
}

#[derive(Debug, Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub steps: Vec<WorkflowStep>,
    pub readiness: WorkflowReadiness,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSnapshot {
    pub current_stage: WorkflowStage,
    pub current_stage_label: String,
    pub blocking_reasons: Vec<BlockingReason>,
    pub next_actions: Vec<WorkflowAction>,
    pub summary: WorkflowSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowSummaryFields {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    steps: Option<Vec<WorkflowStep>>,
    #[serde(default)]
    readiness: Option<WorkflowReadiness>,
}

impl From<WorkflowSummaryFields> for WorkflowSummary {
    fn from(value: WorkflowSummaryFields) -> Self {
        Self {
            text: value.text,
            steps: value.steps.unwrap_or_default(),
            readiness: value.readiness.unwrap_or_default(),
        }
    }
}

impl<'de> Deserialize<'de> for WorkflowSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::String(text) => Ok(Self {
                text: Some(text),
                ..Self::default()
            }),
            serde_json::Value::Object(_) => {
                let fields: WorkflowSummaryFields =
                    serde_json::from_value(value).map_err(|error| {
                        D::Error::custom(format!("path=workflow.summary; serde_error={error}"))
                    })?;
                Ok(fields.into())
            }
            _ => Err(D::Error::custom(
                "path=workflow.summary; serde_error=summary must be null, string, or object",
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowSnapshotFields {
    current_stage: WorkflowStage,
    #[serde(default)]
    current_stage_label: Option<String>,
    #[serde(default)]
    blocking_reasons: Vec<BlockingReason>,
    #[serde(default)]
    next_actions: Vec<WorkflowAction>,
    #[serde(default)]
    summary: WorkflowSummary,
}

impl<'de> Deserialize<'de> for WorkflowSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = WorkflowSnapshotFields::deserialize(deserializer)?;
        let current_stage = fields.current_stage;
        Ok(Self {
            current_stage_label: fields
                .current_stage_label
                .filter(|label| !label.trim().is_empty())
                .unwrap_or_else(|| workflow_stage_label(&current_stage)),
            current_stage,
            blocking_reasons: fields.blocking_reasons,
            next_actions: fields.next_actions,
            summary: fields.summary,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppStatus {
    pub app_version: String,
    pub platform: String,
    pub tauri_ready: bool,
    pub rust_backend_ready: bool,
}

fn workflow_stage_label(stage: &WorkflowStage) -> String {
    match stage {
        WorkflowStage::DocumentsMissing => "Belgeler Eksik",
        WorkflowStage::PdfPreviewMissing => "PDF Önizleme Eksik",
        WorkflowStage::PdfPreviewReady => "PDF Önizleme Hazır",
        WorkflowStage::PdfPreviewReadyQuestionTextMissing => "Soru Metni Eksik",
        WorkflowStage::ExamPackageBuildReady => "Sınav Paketi Hazır",
        WorkflowStage::ExamPackageBuildRunning => "Sınav Paketi Oluşturuluyor",
        WorkflowStage::ExamPackageReviewNeeded => "Sınav Paketi İnceleme Gerekiyor",
        WorkflowStage::ExamPackageIncomplete => "Sınav Paketi Eksik",
        WorkflowStage::ExamPackageReadyForQep => "Sınav Paketi QEP İçin Hazır",
        WorkflowStage::QuestionTextMissing => "Soru Metni Eksik",
        WorkflowStage::QuestionTextExtractionRunning => "Soru Metni Çıkarılıyor",
        WorkflowStage::QuestionTextSuggested => "Soru Metni Önerildi",
        WorkflowStage::QuestionTextConfirmed => "Soru Metni Onaylandı",
        WorkflowStage::RubricMissing => "Rubrik Eksik",
        WorkflowStage::RubricSuggested => "Rubrik Önerildi",
        WorkflowStage::RubricImportedNeedsReview => "Rubrik İnceleme Bekliyor",
        WorkflowStage::RubricInvalid => "Rubrik Geçersiz",
        WorkflowStage::RubricConfirmed => "Rubrik Onaylandı",
        WorkflowStage::StudentScansMissing => "Öğrenci Kağıtları Eksik",
        WorkflowStage::StudentScanPreviewMissing => "Öğrenci Kağıtları Önizleme Eksik",
        WorkflowStage::StudentGroupingMissing => "Öğrenci Gruplama Eksik",
        WorkflowStage::StudentGroupingReady => "Öğrenci Gruplama Hazır",
        WorkflowStage::CropMissing => "Kırpma Eksik",
        WorkflowStage::OcrReady => "OCR Hazır",
        WorkflowStage::OcrRunning => "OCR Çalışıyor",
        WorkflowStage::ReviewRequired => "İnceleme Gerekiyor",
        WorkflowStage::StudentAnswerOcrRunning => "Öğrenci Cevap OCR’ı Çalışıyor",
        WorkflowStage::StudentAnswerOcrReviewNeeded => "Öğrenci Cevap OCR’ı Kontrol Bekliyor",
        WorkflowStage::StudentAnswerOcrReadyForScoring => "Öğrenci Cevap OCR’ı Onaylandı",
        WorkflowStage::QepMissing => "QEP Eksik",
        WorkflowStage::QepReady => "QEP Hazır",
        WorkflowStage::QepFrozen => "QEP Donduruldu",
        WorkflowStage::ScoringReady => "Puanlama Hazır",
        WorkflowStage::ScoringRunning => "Puanlama Çalışıyor",
        WorkflowStage::ScoringDone => "Puanlama Tamamlandı",
        WorkflowStage::AnalysisReady => "Analiz Hazır",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_snapshot_default() {
        let snapshot = WorkflowSnapshot {
            current_stage: WorkflowStage::DocumentsMissing,
            current_stage_label: "Belgeler Eksik".to_string(),
            blocking_reasons: vec![BlockingReason::ExamSourceMissing],
            next_actions: vec![WorkflowAction {
                code: "import_exam_source_pdf".to_string(),
                label: "Orijinal sınav PDF'i yükle".to_string(),
                enabled: true,
                disabled_reason: None,
                command: Some("import_exam_source_pdf".to_string()),
                requires: None,
            }],
            summary: WorkflowSummary {
                text: None,
                steps: vec![],
                readiness: WorkflowReadiness {
                    exam_package_freeze: false,
                    student_intake: false,
                    scoring: false,
                },
            },
        };
        assert_eq!(snapshot.blocking_reasons.len(), 1);
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(serialized.contains("documents_missing"));
        assert!(serialized.contains("EXAM_SOURCE_MISSING"));
        assert!(serialized.contains("import_exam_source_pdf"));
    }

    #[test]
    fn test_workflow_summary_accepts_legacy_null_and_string() {
        let null_summary: WorkflowSummary = serde_json::from_str("null").unwrap();
        assert_eq!(null_summary, WorkflowSummary::default());

        let string_summary: WorkflowSummary =
            serde_json::from_str(r#""Soru metni çıkarımı çalışıyor.""#).unwrap();
        assert_eq!(
            string_summary.text.as_deref(),
            Some("Soru metni çıkarımı çalışıyor.")
        );
        assert!(string_summary.steps.is_empty());
        assert_eq!(string_summary.readiness, WorkflowReadiness::default());
    }

    #[test]
    fn test_workflow_snapshot_defaults_missing_label_and_summary_fields() {
        let snapshot: WorkflowSnapshot = serde_json::from_str(
            r#"{
                "currentStage": "question_text_missing",
                "blockingReasons": [],
                "nextActions": [],
                "summary": {
                    "text": "Soru metni çıkarımı çalışıyor."
                }
            }"#,
        )
        .unwrap();

        assert_eq!(snapshot.current_stage_label, "Soru Metni Eksik");
        assert_eq!(
            snapshot.summary.text.as_deref(),
            Some("Soru metni çıkarımı çalışıyor.")
        );
        assert!(snapshot.summary.steps.is_empty());
        assert_eq!(snapshot.summary.readiness, WorkflowReadiness::default());
    }
}

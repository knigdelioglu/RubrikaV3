use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model_platform::BenchmarkResultSummary;
use crate::services::golden_ocr_metrics::{
    percentile_p50, percentile_p95, GoldenModelRuntimeStatus, GoldenOcrBenchmarkReport,
};
use crate::services::model_benchmark_service::{
    BenchmarkObservation, BenchmarkSubmission, ModelBenchmarkService,
};
use crate::services::model_platform_service::ModelPlatformService;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenOcrBenchmarkFileInput {
    pub task_profile_id: String,
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    pub report_path: String,
    pub baseline_report_path: String,
}

#[derive(Clone)]
pub struct ModelGoldenBenchmarkBridge {
    benchmark: ModelBenchmarkService,
}

impl ModelGoldenBenchmarkBridge {
    pub fn new(platform: ModelPlatformService) -> Self {
        Self {
            benchmark: ModelBenchmarkService::new(platform),
        }
    }

    pub fn evaluate_ocr_files(
        &self,
        input: GoldenOcrBenchmarkFileInput,
    ) -> Result<BenchmarkResultSummary, AppError> {
        let report = read_report(&input.report_path)?;
        let baseline = read_report(&input.baseline_report_path)?;
        require_measured_report(&report, "candidate")?;
        require_measured_report(&baseline, "baseline")?;
        if report.exam_id != baseline.exam_id {
            return Err(bridge_error(
                "Candidate ve baseline golden corpus kimliği eşleşmiyor.",
                Some(format!(
                    "candidate_exam_id={}; baseline_exam_id={}",
                    report.exam_id, baseline.exam_id
                )),
            ));
        }
        if report.corpus_manifest_sha256 != baseline.corpus_manifest_sha256 {
            return Err(bridge_error(
                "Candidate ve baseline farklı golden manifest üzerinde çalıştırılmış.",
                Some(format!(
                    "candidate_manifest={:?}; baseline_manifest={:?}",
                    report.corpus_manifest_sha256, baseline.corpus_manifest_sha256
                )),
            ));
        }

        let candidate = summarize(&report)?;
        let base = summarize(&baseline)?;
        self.benchmark
            .evaluate_verified_and_record(BenchmarkSubmission {
                task_profile_id: input.task_profile_id,
                model_definition_id: input.model_definition_id,
                runtime_definition_id: input.runtime_definition_id,
                observations: vec![
                    observation("critical_token_missing", candidate.critical_token_missing, None),
                    observation(
                        "printed_question_leakage",
                        candidate.printed_question_leakage,
                        None,
                    ),
                    observation(
                        "schema_failure_rate",
                        candidate.schema_failure_rate,
                        Some(base.schema_failure_rate),
                    ),
                    observation("cer", candidate.cer, Some(base.cer)),
                    observation("wer", candidate.wer, Some(base.wer)),
                    observation(
                        "latency_p50_ms",
                        candidate.latency_p50_ms,
                        Some(base.latency_p50_ms),
                    ),
                    observation(
                        "latency_p95_ms",
                        candidate.latency_p95_ms,
                        Some(base.latency_p95_ms),
                    ),
                    observation(
                        "image_token_count",
                        candidate.image_token_count,
                        Some(base.image_token_count),
                    ),
                    observation(
                        "model_call_count",
                        candidate.model_call_count,
                        Some(base.model_call_count),
                    ),
                    observation("retry_count", candidate.retry_count, Some(base.retry_count)),
                    observation(
                        "peak_memory_bytes",
                        candidate.peak_memory_bytes,
                        Some(base.peak_memory_bytes),
                    ),
                ],
                notes: vec![
                    format!("golden_exam_id={}", report.exam_id),
                    format!("candidate_report={}", input.report_path),
                    format!("baseline_report={}", input.baseline_report_path),
                    format!(
                        "corpus_manifest_sha256={}",
                        report.corpus_manifest_sha256.unwrap_or_default()
                    ),
                ],
            })
    }
}

#[derive(Debug, Clone, Copy)]
struct OcrSummary {
    cer: f64,
    wer: f64,
    critical_token_missing: f64,
    printed_question_leakage: f64,
    schema_failure_rate: f64,
    latency_p50_ms: f64,
    latency_p95_ms: f64,
    image_token_count: f64,
    model_call_count: f64,
    retry_count: f64,
    peak_memory_bytes: f64,
}

fn summarize(report: &GoldenOcrBenchmarkReport) -> Result<OcrSummary, AppError> {
    let cer = report
        .aggregate
        .cer_p50
        .ok_or_else(|| bridge_error("Golden raporda CER p50 yok.", None))?;
    let wer = report
        .aggregate
        .wer_p50
        .ok_or_else(|| bridge_error("Golden raporda WER p50 yok.", None))?;
    let critical_token_missing = report
        .per_question
        .iter()
        .map(|item| item.critical_token_missing.unwrap_or(0) as f64)
        .sum();
    let printed_question_leakage = report
        .per_question
        .iter()
        .filter(|item| item.printed_question_leakage == Some(true))
        .count() as f64;
    let structured = report
        .per_question
        .iter()
        .filter_map(|item| item.structured_exact_match)
        .collect::<Vec<_>>();
    let schema_failure_rate = if structured.is_empty() {
        0.0
    } else {
        structured.iter().filter(|value| !**value).count() as f64 / structured.len() as f64
    };
    let p50_samples = report
        .per_question
        .iter()
        .filter_map(|item| item.duration_ms_p50.map(|value| value as f64))
        .collect::<Vec<_>>();
    let p95_samples = report
        .per_question
        .iter()
        .filter_map(|item| item.duration_ms_p95.map(|value| value as f64))
        .collect::<Vec<_>>();
    let latency_p50_ms = percentile_p50(&p50_samples).unwrap_or(0.0);
    let latency_p95_ms = percentile_p95(&p95_samples).unwrap_or(0.0);
    Ok(OcrSummary {
        cer,
        wer,
        critical_token_missing,
        printed_question_leakage,
        schema_failure_rate,
        latency_p50_ms,
        latency_p95_ms,
        image_token_count: report.aggregate.total_image_tokens.unwrap_or(0) as f64,
        model_call_count: report.aggregate.total_model_calls.unwrap_or(0) as f64,
        retry_count: report.aggregate.total_retries.unwrap_or(0) as f64,
        peak_memory_bytes: report.aggregate.peak_memory_bytes.unwrap_or(0) as f64,
    })
}

fn read_report(path: &str) -> Result<GoldenOcrBenchmarkReport, AppError> {
    let path = Path::new(path);
    let content = std::fs::read_to_string(path).map_err(|error| AppError {
        code: AppErrorCode::FileReadFailed,
        message: "Golden benchmark raporu okunamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Geçerli benchmark_report.json dosyasını seçin.".to_string()),
        technical_details: Some(format!("path={}; error={error}", path.to_string_lossy())),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    serde_json::from_str(&content).map_err(|error| AppError {
        code: AppErrorCode::ModelBenchmarkFailed,
        message: "Golden benchmark raporu geçerli Rubrika benchmark JSON'u değil.".to_string(),
        recoverable: true,
        suggested_action: Some("Golden benchmark runner çıktısını seçin.".to_string()),
        technical_details: Some(format!("path={}; error={error}", path.to_string_lossy())),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

fn require_measured_report(report: &GoldenOcrBenchmarkReport, label: &str) -> Result<(), AppError> {
    if report.model_runtime != GoldenModelRuntimeStatus::Available {
        return Err(bridge_error(
            "Model runtime olmadan üretilmiş preview raporu promotion gate için kullanılamaz.",
            Some(format!(
                "report={label}; model_runtime={:?}",
                report.model_runtime
            )),
        ));
    }
    Ok(())
}

fn observation(key: &str, value: f64, baseline_value: Option<f64>) -> BenchmarkObservation {
    BenchmarkObservation {
        key: key.to_string(),
        value,
        baseline_value,
    }
}

fn bridge_error(message: &str, technical_details: Option<String>) -> AppError {
    AppError {
        code: AppErrorCode::ModelBenchmarkFailed,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Golden benchmark raporlarını yeniden seçin veya benchmark'ı yeniden çalıştırın."
                .to_string(),
        ),
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

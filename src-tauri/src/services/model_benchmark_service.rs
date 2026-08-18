use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model_platform::{
    fingerprint_runtime_definition, BenchmarkGateState, BenchmarkMetricValue,
    BenchmarkResultSummary, ModelLifecycleState, BENCHMARK_POLICY_VERSION,
};
use crate::services::model_platform_service::{new_benchmark_result_id, ModelPlatformService};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkObservation {
    pub key: String,
    pub value: f64,
    #[serde(default)]
    pub baseline_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkSubmission {
    pub task_profile_id: String,
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    pub observations: Vec<BenchmarkObservation>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum Compare {
    LessOrEqual,
    GreaterOrEqual,
    Equal,
    RegressionLessOrEqual,
}

#[derive(Debug, Clone)]
struct MetricRule {
    key: &'static str,
    compare: Compare,
    threshold: f64,
    required: bool,
}

#[derive(Clone)]
pub struct ModelBenchmarkService {
    platform: ModelPlatformService,
}

impl ModelBenchmarkService {
    pub fn new(platform: ModelPlatformService) -> Self {
        Self { platform }
    }

    /// Records observations entered by a user or another untrusted caller.
    /// These results are useful for side-by-side diagnostics, but can never
    /// satisfy the Production promotion gate even if every metric is within
    /// policy. Promotion evidence must come from a trusted measured bridge.
    pub fn evaluate_and_record(
        &self,
        submission: BenchmarkSubmission,
    ) -> Result<BenchmarkResultSummary, AppError> {
        self.evaluate_and_record_with_trust(submission, false)
    }

    /// Records a measured benchmark produced by an application-owned bridge
    /// (for example the golden OCR report importer). Only this path may emit a
    /// PASS result or advance a model to BenchmarkVerified.
    pub fn evaluate_verified_and_record(
        &self,
        submission: BenchmarkSubmission,
    ) -> Result<BenchmarkResultSummary, AppError> {
        self.evaluate_and_record_with_trust(submission, true)
    }

    fn evaluate_and_record_with_trust(
        &self,
        submission: BenchmarkSubmission,
        promotion_eligible: bool,
    ) -> Result<BenchmarkResultSummary, AppError> {
        let snapshot = self.platform.snapshot()?;
        let task = snapshot
            .task_profiles
            .iter()
            .find(|item| item.id == submission.task_profile_id)
            .ok_or_else(|| benchmark_error(
                AppErrorCode::ModelRegistryEntryNotFound,
                "Benchmark task profile bulunamadı.",
                Some(format!("task_profile_id={}", submission.task_profile_id)),
                None,
            ))?;
        let model = snapshot
            .models
            .iter()
            .find(|item| item.id == submission.model_definition_id)
            .ok_or_else(|| benchmark_error(
                AppErrorCode::ModelRegistryEntryNotFound,
                "Benchmark modeli registry'de bulunamadı.",
                Some(format!("model_definition_id={}", submission.model_definition_id)),
                None,
            ))?;
        let runtime = snapshot
            .runtimes
            .iter()
            .find(|item| item.id == submission.runtime_definition_id)
            .ok_or_else(|| benchmark_error(
                AppErrorCode::ModelRegistryEntryNotFound,
                "Benchmark runtime'ı registry'de bulunamadı.",
                Some(format!("runtime_definition_id={}", submission.runtime_definition_id)),
                None,
            ))?;

        let observed: BTreeMap<&str, &BenchmarkObservation> = submission
            .observations
            .iter()
            .map(|item| (item.key.as_str(), item))
            .collect();
        let rules = policy_for_task(&task.id);
        let policy_keys: BTreeSet<&str> = rules.iter().map(|rule| rule.key).collect();
        let mut metrics = Vec::with_capacity(rules.len() + submission.observations.len());
        let mut notes = submission.notes;
        let mut all_pass = true;

        for rule in rules {
            match observed.get(rule.key) {
                Some(observation) => {
                    let pass = evaluate_rule(&rule, observation);
                    if !pass {
                        all_pass = false;
                    }
                    metrics.push(BenchmarkMetricValue {
                        key: rule.key.to_string(),
                        value: observation.value,
                        baseline_value: observation.baseline_value,
                        pass,
                    });
                }
                None if rule.required => {
                    all_pass = false;
                    notes.push(format!("required benchmark metric missing: {}", rule.key));
                    metrics.push(BenchmarkMetricValue {
                        key: rule.key.to_string(),
                        value: 0.0,
                        baseline_value: None,
                        pass: false,
                    });
                }
                None => {}
            }
        }

        // Performance/diagnostic observations are retained for side-by-side
        // comparison but never silently become a quality gate. Only the
        // versioned policy rules above can change `all_pass`.
        for observation in &submission.observations {
            if policy_keys.contains(observation.key.as_str()) {
                continue;
            }
            metrics.push(BenchmarkMetricValue {
                key: observation.key.clone(),
                value: observation.value,
                baseline_value: observation.baseline_value,
                pass: observation.value.is_finite()
                    && observation
                        .baseline_value
                        .map(|value| value.is_finite())
                        .unwrap_or(true),
            });
        }

        let policy_pass = all_pass;
        if !promotion_eligible {
            notes.push(
                "diagnostic_only: manual/untrusted benchmark observations cannot satisfy Production promotion"
                    .to_string(),
            );
        }
        let base_id = new_benchmark_result_id(&task.id, &model.id);
        let result = BenchmarkResultSummary {
            id: format!(
                "{}-{}",
                if promotion_eligible { "verified" } else { "diagnostic" },
                base_id
            ),
            task_profile_id: task.id.clone(),
            model_definition_id: model.id.clone(),
            runtime_definition_id: runtime.id.clone(),
            model_fingerprint: model.model_fingerprint.clone(),
            runtime_fingerprint: fingerprint_runtime_definition(runtime),
            policy_version: BENCHMARK_POLICY_VERSION.to_string(),
            state: if promotion_eligible && policy_pass {
                BenchmarkGateState::Pass
            } else {
                BenchmarkGateState::Fail
            },
            generated_at: Utc::now().to_rfc3339(),
            metrics,
            notes,
        };

        self.platform.record_benchmark_result(result.clone())?;
        if promotion_eligible {
            if policy_pass
                && matches!(
                    model.lifecycle_state,
                    ModelLifecycleState::Experimental | ModelLifecycleState::Compatible
                )
            {
                let _ = self
                    .platform
                    .set_model_lifecycle(&model.id, ModelLifecycleState::BenchmarkVerified);
            } else if !policy_pass && model.lifecycle_state != ModelLifecycleState::Production {
                let _ = self
                    .platform
                    .set_model_lifecycle(&model.id, ModelLifecycleState::BenchmarkFailed);
            }
        }
        Ok(result)
    }
}

fn evaluate_rule(rule: &MetricRule, observation: &BenchmarkObservation) -> bool {
    if !observation.value.is_finite() {
        return false;
    }
    match rule.compare {
        Compare::LessOrEqual => observation.value <= rule.threshold,
        Compare::GreaterOrEqual => observation.value >= rule.threshold,
        Compare::Equal => (observation.value - rule.threshold).abs() <= f64::EPSILON,
        Compare::RegressionLessOrEqual => observation
            .baseline_value
            .filter(|value| value.is_finite())
            .map(|baseline| observation.value - baseline <= rule.threshold)
            .unwrap_or(false),
    }
}

fn policy_for_task(task_profile_id: &str) -> Vec<MetricRule> {
    match task_profile_id {
        "student_answer_ocr"
        | "student_answer_ocr_issue_correction"
        | "question_text_extraction"
        | "rubric_extraction" => vec![
            rule("critical_token_missing", Compare::Equal, 0.0),
            rule("printed_question_leakage", Compare::Equal, 0.0),
            regression_rule("schema_failure_rate", 0.0),
            regression_rule("cer", 0.03),
            regression_rule("wer", 0.05),
        ],
        "semantic_scoring" | "speaking_evaluation" => vec![
            rule("unknown_criterion_id", Compare::Equal, 0.0),
            rule("invalid_canonical_level_id", Compare::Equal, 0.0),
            rule("positive_score_without_exact_evidence", Compare::Equal, 0.0),
            regression_rule("schema_failure_rate", 0.0),
            rule("golden_agreement", Compare::GreaterOrEqual, 0.95),
        ],
        "speaking_transcript_cleanup" => vec![
            rule("segment_coverage", Compare::GreaterOrEqual, 1.0),
            rule("semantic_change_rate", Compare::LessOrEqual, 0.0),
            regression_rule("schema_failure_rate", 0.0),
        ],
        _ => vec![regression_rule("schema_failure_rate", 0.0)],
    }
}

fn rule(key: &'static str, compare: Compare, threshold: f64) -> MetricRule {
    MetricRule {
        key,
        compare,
        threshold,
        required: true,
    }
}

fn regression_rule(key: &'static str, threshold: f64) -> MetricRule {
    MetricRule {
        key,
        compare: Compare::RegressionLessOrEqual,
        threshold,
        required: true,
    }
}

fn benchmark_error(
    code: AppErrorCode,
    message: &str,
    technical_details: Option<String>,
    suggested_action: Option<String>,
) -> AppError {
    AppError {
        code,
        message: message.to_string(),
        recoverable: true,
        suggested_action,
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

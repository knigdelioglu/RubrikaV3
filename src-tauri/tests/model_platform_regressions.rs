use app_lib::domain::model::default_model_profile;
use app_lib::domain::model_platform::{
    fingerprint_runtime_definition, migrate_legacy_profile, BenchmarkGateState, CapabilityManifest,
    CapabilityProbeResult, CapabilitySupport, ModelCapabilityKind, ModelLifecycleState,
    ModelPlatformConfig,
};
use app_lib::services::model_benchmark_service::{
    BenchmarkObservation, BenchmarkSubmission, ModelBenchmarkService,
};
use app_lib::services::model_platform_service::ModelPlatformService;
use std::path::PathBuf;
use uuid::Uuid;

fn seeded_platform() -> (ModelPlatformService, String, String, PathBuf) {
    let migration = migrate_legacy_profile(&default_model_profile());
    let mut model = migration.model;
    model.lifecycle_state = ModelLifecycleState::Experimental;
    let runtime = migration.runtime;
    let model_id = model.id.clone();
    let runtime_id = runtime.id.clone();
    let config_path = std::env::temp_dir().join(format!(
        "rubrika-model-platform-regression-{}.json",
        Uuid::new_v4()
    ));
    let service = ModelPlatformService::new_with_path(config_path.clone());
    let mut config = ModelPlatformConfig::default();
    config.models.push(model);
    config.runtimes.push(runtime);
    service
        .replace_config(config)
        .expect("seeded model platform config must persist");
    (service, model_id, runtime_id, config_path)
}

fn passing_ocr_submission(
    model_definition_id: &str,
    runtime_definition_id: &str,
) -> BenchmarkSubmission {
    BenchmarkSubmission {
        task_profile_id: "student_answer_ocr".to_string(),
        model_definition_id: model_definition_id.to_string(),
        runtime_definition_id: runtime_definition_id.to_string(),
        observations: vec![
            BenchmarkObservation {
                key: "critical_token_missing".to_string(),
                value: 0.0,
                baseline_value: None,
            },
            BenchmarkObservation {
                key: "printed_question_leakage".to_string(),
                value: 0.0,
                baseline_value: None,
            },
            BenchmarkObservation {
                key: "schema_failure_rate".to_string(),
                value: 0.0,
                baseline_value: Some(0.0),
            },
            BenchmarkObservation {
                key: "cer".to_string(),
                value: 0.0,
                baseline_value: Some(0.0),
            },
            BenchmarkObservation {
                key: "wer".to_string(),
                value: 0.0,
                baseline_value: Some(0.0),
            },
        ],
        notes: vec![],
    }
}

#[test]
fn optional_capability_failure_does_not_make_text_model_globally_unsupported() {
    let (service, model_id, runtime_id, config_path) = seeded_platform();
    let snapshot = service.snapshot().expect("platform snapshot");
    let model = snapshot
        .models
        .iter()
        .find(|model| model.id == model_id)
        .expect("seeded model");
    let runtime = snapshot
        .runtimes
        .iter()
        .find(|runtime| runtime.id == runtime_id)
        .expect("seeded runtime");
    let manifest = CapabilityManifest {
        model_definition_id: model_id.clone(),
        runtime_definition_id: runtime_id,
        model_fingerprint: model.model_fingerprint.clone(),
        runtime_fingerprint: fingerprint_runtime_definition(runtime),
        verified_at: chrono::Utc::now().to_rfc3339(),
        results: vec![
            CapabilityProbeResult {
                capability: ModelCapabilityKind::Text,
                support: CapabilitySupport::Pass,
                detail: None,
                duration_ms: None,
            },
            CapabilityProbeResult {
                capability: ModelCapabilityKind::Vision,
                support: CapabilitySupport::Fail,
                detail: Some("text-only model".to_string()),
                duration_ms: None,
            },
        ],
    };

    service
        .mark_probe_finished(&model_id, &manifest)
        .expect("probe lifecycle should update");
    let lifecycle = service
        .snapshot()
        .expect("updated snapshot")
        .models
        .into_iter()
        .find(|model| model.id == model_id)
        .expect("updated model")
        .lifecycle_state;
    assert_eq!(lifecycle, ModelLifecycleState::Compatible);
    let _ = std::fs::remove_file(config_path);
}

#[test]
fn manual_benchmark_is_diagnostic_and_cannot_emit_pass() {
    let (service, model_id, runtime_id, config_path) = seeded_platform();
    let benchmark = ModelBenchmarkService::new(service.clone());
    let result = benchmark
        .evaluate_and_record(passing_ocr_submission(&model_id, &runtime_id))
        .expect("diagnostic benchmark must be recorded");

    assert_eq!(result.state, BenchmarkGateState::Fail);
    assert!(result.id.starts_with("diagnostic-benchmark-"));
    assert!(result
        .notes
        .iter()
        .any(|note| note.contains("diagnostic_only")));
    let lifecycle = service
        .snapshot()
        .expect("snapshot after diagnostic benchmark")
        .models
        .into_iter()
        .find(|model| model.id == model_id)
        .expect("model after diagnostic benchmark")
        .lifecycle_state;
    assert_eq!(lifecycle, ModelLifecycleState::Experimental);
    let _ = std::fs::remove_file(config_path);
}

#[test]
fn trusted_measured_benchmark_can_emit_pass_and_advance_lifecycle() {
    let (service, model_id, runtime_id, config_path) = seeded_platform();
    let benchmark = ModelBenchmarkService::new(service.clone());
    let result = benchmark
        .evaluate_verified_and_record(passing_ocr_submission(&model_id, &runtime_id))
        .expect("trusted measured benchmark must be recorded");

    assert_eq!(result.state, BenchmarkGateState::Pass);
    assert!(result.id.starts_with("verified-benchmark-"));
    let lifecycle = service
        .snapshot()
        .expect("snapshot after trusted benchmark")
        .models
        .into_iter()
        .find(|model| model.id == model_id)
        .expect("model after trusted benchmark")
        .lifecycle_state;
    assert_eq!(lifecycle, ModelLifecycleState::BenchmarkVerified);
    let _ = std::fs::remove_file(config_path);
}

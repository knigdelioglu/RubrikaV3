use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelMode, ModelProfile, ModelRuntimePreset, PrivacyMode};
use crate::domain::model_platform::{
    default_task_profiles, fingerprint_runtime_definition, migrate_legacy_profile,
    CapabilityManifest, CapabilityProbeResult, CapabilitySupport, ModelCapabilityKind,
    ModelDefinition, ModelLifecycleState, ModelPlatformConfig, ModelTaskKind, RuntimeDefinition,
    TaskModelBinding, CANONICAL_GEMMA4_12B_MODEL_ID, LEGACY_GEMMA_PROFILE_IDS,
    MODEL_PLATFORM_SCHEMA_VERSION,
};
use crate::services::model_config_service::ModelConfigService;
use crate::services::model_platform_service::ModelPlatformService;
use crate::services::model_router_service::ResolvedModelRoute;
use crate::services::platform_launch_registry;
use chrono::Utc;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const VISION_RUNTIME_ID: &str = "llama-local-vision";
pub const TEXT_RUNTIME_ID: &str = "llama-local-text";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPlatformMigrationReport {
    pub migrated: bool,
    pub legacy_backup_path: Option<String>,
    pub model_count: usize,
    pub runtime_count: usize,
    pub binding_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct ModelPlatformMigrationService {
    platform: ModelPlatformService,
    legacy: ModelConfigService,
}

impl ModelPlatformMigrationService {
    pub fn new(platform: ModelPlatformService, legacy: ModelConfigService) -> Self {
        Self { platform, legacy }
    }

    pub fn bootstrap_if_needed(&self) -> Result<ModelPlatformMigrationReport, AppError> {
        let existing = self.platform.snapshot()?;
        if !existing.models.is_empty() || !existing.bindings.is_empty() {
            return Ok(ModelPlatformMigrationReport {
                migrated: false,
                legacy_backup_path: None,
                model_count: existing.models.len(),
                runtime_count: existing.runtimes.len(),
                binding_count: existing.bindings.len(),
                warnings: vec![],
            });
        }

        let backup = backup_legacy_config_if_present()?;
        let profiles = self.collect_legacy_profiles();
        if profiles.is_empty() {
            return Ok(ModelPlatformMigrationReport {
                migrated: false,
                legacy_backup_path: backup.map(|path| path.to_string_lossy().to_string()),
                model_count: 0,
                runtime_count: 0,
                binding_count: 0,
                warnings: vec!["Legacy model profile bulunamadı; registry boş bırakıldı.".to_string()],
            });
        }

        let config = build_platform_config_from_legacy(&profiles)?;
        let report = ModelPlatformMigrationReport {
            migrated: true,
            legacy_backup_path: backup.map(|path| path.to_string_lossy().to_string()),
            model_count: config.models.len(),
            runtime_count: config.runtimes.len(),
            binding_count: config.bindings.len(),
            warnings: vec![],
        };
        self.platform.replace_config(config)?;
        Ok(report)
    }

    fn collect_legacy_profiles(&self) -> Vec<ModelProfile> {
        let mut profiles = Vec::new();
        if let Ok(active) = self.legacy.get_profile(None) {
            profiles.push(active);
        }
        for id in LEGACY_GEMMA_PROFILE_IDS {
            if let Ok(profile) = self.legacy.get_model_profile(id) {
                if !profiles.iter().any(|existing| existing.id == profile.id) {
                    profiles.push(profile);
                }
            }
        }
        profiles
    }
}

pub fn materialize_route_as_legacy_profile(
    legacy: &ModelConfigService,
    route: &ResolvedModelRoute,
) -> Result<String, AppError> {
    let runtime_fingerprint = fingerprint_runtime_definition(&route.runtime);
    let model_fingerprint = &route.model.model_fingerprint;
    let profile_id = format!(
        "platform-{}-{}-{}-{}",
        sanitize_id(&route.model.id),
        sanitize_id(&route.runtime.id),
        short_fingerprint(model_fingerprint),
        short_fingerprint(&runtime_fingerprint),
    );
    let requires_vision = route
        .task_profile
        .required_capabilities
        .contains(&ModelCapabilityKind::Vision);
    let mmproj_path = route.model.mmproj_path.clone().unwrap_or_default();
    let profile = ModelProfile {
        id: profile_id.clone(),
        display_name: format!("{} — {}", route.model.display_name, route.runtime.id),
        mode: if route.runtime.managed {
            ModelMode::Managed
        } else {
            ModelMode::External
        },
        server_path: route.runtime.server_path.clone(),
        model_path: route.model.model_path.clone(),
        mmproj_path,
        host: route.runtime.host.clone(),
        port: route.runtime.port,
        base_url: route.runtime.base_url(),
        runtime_preset: if requires_vision {
            ModelRuntimePreset::Standard
        } else {
            ModelRuntimePreset::SpeakingRubricText
        },
        privacy_mode: route.runtime.privacy_mode,
    };
    platform_launch_registry::register(
        profile_id.clone(),
        route.model.clone(),
        route.runtime.clone(),
    );
    legacy.update_ephemeral_profile(profile)?;
    Ok(profile_id)
}

fn build_platform_config_from_legacy(profiles: &[ModelProfile]) -> Result<ModelPlatformConfig, AppError> {
    let mut config = ModelPlatformConfig {
        schema_version: MODEL_PLATFORM_SCHEMA_VERSION.to_string(),
        models: vec![],
        runtimes: vec![],
        task_profiles: default_task_profiles(),
        bindings: vec![],
        capability_manifests: vec![],
        benchmark_results: vec![],
    };

    let canonical_profiles: Vec<&ModelProfile> = profiles
        .iter()
        .filter(|profile| LEGACY_GEMMA_PROFILE_IDS.contains(&profile.id.as_str()))
        .collect();

    if !canonical_profiles.is_empty() {
        let canonical_model = merge_gemma_model(&canonical_profiles);
        config.models.push(canonical_model.clone());

        if let Some(profile) = canonical_profiles
            .iter()
            .copied()
            .find(|profile| profile.id == LEGACY_GEMMA_PROFILE_IDS[0])
        {
            let mut runtime = migrate_legacy_profile(profile).runtime;
            runtime.id = VISION_RUNTIME_ID.to_string();
            upsert_runtime(&mut config, runtime.clone());
            seed_baseline_manifest(&mut config, &canonical_model, &runtime, true);
        }

        if let Some(profile) = canonical_profiles.iter().copied().find(|profile| {
            matches!(
                profile.id.as_str(),
                "speaking_transcript_cleanup_12b" | "speaking_rubric_evaluation_12b"
            )
        }) {
            let mut runtime = migrate_legacy_profile(profile).runtime;
            runtime.id = TEXT_RUNTIME_ID.to_string();
            upsert_runtime(&mut config, runtime.clone());
            seed_baseline_manifest(&mut config, &canonical_model, &runtime, false);
        }

        for profile in canonical_profiles {
            append_canonical_bindings(&mut config, profile);
        }
    }

    for profile in profiles.iter().filter(|profile| {
        !LEGACY_GEMMA_PROFILE_IDS.contains(&profile.id.as_str())
    }) {
        let migration = migrate_legacy_profile(profile);
        let mut model = migration.model;
        model.lifecycle_state = ModelLifecycleState::Production;
        model.metadata.insert("baselineVerified".to_string(), "true".to_string());
        let mut runtime = migration.runtime;
        runtime.id = format!("legacy-runtime-{}", sanitize_id(&profile.id));
        let runtime_id = runtime.id.clone();
        let model_id = model.id.clone();
        upsert_model(&mut config, model.clone());
        upsert_runtime(&mut config, runtime.clone());
        seed_baseline_manifest(
            &mut config,
            &model,
            &runtime,
            model.capabilities.vision,
        );
        for mut binding in migration.bindings {
            binding.model_definition_id = model_id.clone();
            binding.runtime_definition_id = runtime_id.clone();
            upsert_binding(&mut config, binding);
        }
    }

    if config.models.is_empty() {
        return Err(migration_error(
            "Legacy profil(ler) okunabildi fakat dönüştürülebilir model bulunamadı.",
            Some(format!("profile_count={}", profiles.len())),
        ));
    }

    Ok(config)
}

fn merge_gemma_model(profiles: &[&ModelProfile]) -> ModelDefinition {
    let base = profiles
        .iter()
        .copied()
        .find(|profile| profile.id == LEGACY_GEMMA_PROFILE_IDS[0])
        .unwrap_or(profiles[0]);
    let mut migration = migrate_legacy_profile(base).model;
    migration.id = CANONICAL_GEMMA4_12B_MODEL_ID.to_string();
    migration.display_name = "Gemma 4 12B".to_string();
    migration.family = "gemma".to_string();
    migration.lifecycle_state = ModelLifecycleState::Production;
    migration.capabilities.text = true;
    migration.capabilities.structured_json = true;
    migration.capabilities.thinking_control = true;
    migration.capabilities.vision = profiles
        .iter()
        .any(|profile| !profile.mmproj_path.trim().is_empty());
    migration.capabilities.multimodal_projector_required = migration.capabilities.vision;
    if migration.model_path.trim().is_empty() {
        if let Some(path) = profiles
            .iter()
            .map(|profile| profile.model_path.trim())
            .find(|path| !path.is_empty())
        {
            migration.model_path = path.to_string();
        }
    }
    if migration.mmproj_path.as_deref().unwrap_or_default().trim().is_empty() {
        migration.mmproj_path = profiles
            .iter()
            .map(|profile| profile.mmproj_path.trim())
            .find(|path| !path.is_empty())
            .map(str::to_string);
    }
    migration.metadata = BTreeMap::from([
        ("migrationSource".to_string(), "legacy_model_profiles".to_string()),
        ("baselineVerified".to_string(), "true".to_string()),
        (
            "legacyAliases".to_string(),
            profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>()
                .join(","),
        ),
    ]);
    migration.refresh_fingerprint();
    migration
}

fn append_canonical_bindings(config: &mut ModelPlatformConfig, profile: &ModelProfile) {
    let tasks: Vec<ModelTaskKind> = match profile.id.as_str() {
        "speaking_transcript_cleanup_12b" => vec![ModelTaskKind::SpeakingTranscriptCleanup],
        "speaking_rubric_evaluation_12b" => vec![ModelTaskKind::SpeakingEvaluation],
        _ => vec![
            ModelTaskKind::QuestionTextExtraction,
            ModelTaskKind::RubricExtraction,
            ModelTaskKind::StudentAnswerOcr,
            ModelTaskKind::StudentAnswerOcrIssueCorrection,
            ModelTaskKind::SemanticScoring,
            ModelTaskKind::Analysis,
            ModelTaskKind::GeneralText,
        ],
    };
    let runtime_id = if matches!(
        profile.id.as_str(),
        "speaking_transcript_cleanup_12b" | "speaking_rubric_evaluation_12b"
    ) {
        TEXT_RUNTIME_ID
    } else {
        VISION_RUNTIME_ID
    };
    if !config.runtimes.iter().any(|runtime| runtime.id == runtime_id) {
        return;
    }
    for task in tasks {
        upsert_binding(
            config,
            TaskModelBinding {
                id: format!("binding-{}-{}", task.id(), CANONICAL_GEMMA4_12B_MODEL_ID),
                task_profile_id: task.id().to_string(),
                model_definition_id: CANONICAL_GEMMA4_12B_MODEL_ID.to_string(),
                runtime_definition_id: runtime_id.to_string(),
                allow_experimental_student_data: false,
                enabled: true,
            },
        );
    }
}

fn seed_baseline_manifest(
    config: &mut ModelPlatformConfig,
    model: &ModelDefinition,
    runtime: &RuntimeDefinition,
    vision_runtime: bool,
) {
    let mut supported = BTreeSet::from([
        ModelCapabilityKind::Text,
        ModelCapabilityKind::StructuredJson,
        ModelCapabilityKind::ThinkingControl,
    ]);
    if vision_runtime && model.capabilities.vision {
        supported.insert(ModelCapabilityKind::Vision);
        supported.insert(ModelCapabilityKind::MultimodalProjector);
    }
    let all = [
        ModelCapabilityKind::Text,
        ModelCapabilityKind::Vision,
        ModelCapabilityKind::StructuredJson,
        ModelCapabilityKind::JsonSchema,
        ModelCapabilityKind::ThinkingControl,
        ModelCapabilityKind::MultimodalProjector,
    ];
    let results = all
        .into_iter()
        .map(|capability| CapabilityProbeResult {
            capability,
            support: if supported.contains(&capability) {
                CapabilitySupport::Pass
            } else if capability == ModelCapabilityKind::JsonSchema {
                CapabilitySupport::Partial
            } else {
                CapabilitySupport::Fail
            },
            detail: Some("grandfathered production baseline; re-probe recommended".to_string()),
            duration_ms: None,
        })
        .collect();
    config.capability_manifests.push(CapabilityManifest {
        model_definition_id: model.id.clone(),
        runtime_definition_id: runtime.id.clone(),
        model_fingerprint: model.model_fingerprint.clone(),
        runtime_fingerprint: fingerprint_runtime_definition(runtime),
        verified_at: Utc::now().to_rfc3339(),
        results,
    });
}

fn upsert_model(config: &mut ModelPlatformConfig, model: ModelDefinition) {
    if let Some(existing) = config.models.iter_mut().find(|item| item.id == model.id) {
        *existing = model;
    } else {
        config.models.push(model);
    }
}

fn upsert_runtime(config: &mut ModelPlatformConfig, runtime: RuntimeDefinition) {
    if let Some(existing) = config.runtimes.iter_mut().find(|item| item.id == runtime.id) {
        *existing = runtime;
    } else {
        config.runtimes.push(runtime);
    }
}

fn upsert_binding(config: &mut ModelPlatformConfig, binding: TaskModelBinding) {
    if let Some(existing) = config.bindings.iter_mut().find(|item| item.id == binding.id) {
        *existing = binding;
    } else {
        config.bindings.push(binding);
    }
}

fn backup_legacy_config_if_present() -> Result<Option<PathBuf>, AppError> {
    let source = legacy_config_path();
    if !source.is_file() {
        return Ok(None);
    }
    let backup = source.with_extension("json.legacy-backup");
    if backup.exists() {
        return Ok(Some(backup));
    }
    std::fs::copy(&source, &backup).map_err(|error| AppError {
        code: AppErrorCode::FileWriteFailed,
        message: "Legacy model ayar yedeği oluşturulamadı.".to_string(),
        recoverable: true,
        suggested_action: Some("Disk izinlerini ve boş alanı kontrol edin.".to_string()),
        technical_details: Some(format!(
            "source={}; backup={}; error={error}",
            source.to_string_lossy(),
            backup.to_string_lossy()
        )),
        correlation_id: Uuid::new_v4().to_string(),
    })?;
    Ok(Some(backup))
}

fn legacy_config_path() -> PathBuf {
    if let Some(path) = env::var_os("RUBRIKA_V3_MODEL_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    let base = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Library")
        .join("Application Support")
        .join("RubrikaV3")
        .join("model_profiles.json")
}

fn short_fingerprint(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn migration_error(message: &str, technical_details: Option<String>) -> AppError {
    AppError {
        code: AppErrorCode::ModelConfigMigrationFailed,
        message: message.to_string(),
        recoverable: true,
        suggested_action: Some(
            "Legacy model ayarlarını koruyun ve migration tanılamasını kontrol edin.".to_string(),
        ),
        technical_details,
        correlation_id: Uuid::new_v4().to_string(),
    }
}

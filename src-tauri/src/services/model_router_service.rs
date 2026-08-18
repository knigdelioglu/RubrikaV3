use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::PrivacyMode;
use crate::domain::model_platform::{
    fingerprint_runtime_definition, BenchmarkGateState, CapabilityManifest, ModelDefinition,
    ModelLifecycleState, ModelTaskKind, RuntimeDefinition, TaskModelBinding, TaskProfile,
};
use crate::services::model_platform_service::ModelPlatformService;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteUsageMode {
    Production,
    ExplicitExperiment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedModelRoute {
    pub task_profile: TaskProfile,
    pub binding: TaskModelBinding,
    pub model: ModelDefinition,
    pub runtime: RuntimeDefinition,
    pub capability_manifest: CapabilityManifest,
    pub benchmark_verified: bool,
    pub usage_mode: RouteUsageMode,
}

#[derive(Clone)]
pub struct ModelRouterService {
    platform: ModelPlatformService,
}

impl ModelRouterService {
    pub fn new(platform: ModelPlatformService) -> Self {
        Self { platform }
    }

    pub fn resolve(
        &self,
        task: ModelTaskKind,
        usage_mode: RouteUsageMode,
    ) -> Result<ResolvedModelRoute, AppError> {
        let snapshot = self.platform.snapshot()?;
        let task_profile = snapshot
            .task_profiles
            .iter()
            .find(|item| item.id == task.id())
            .cloned()
            .ok_or_else(|| route_error(
                AppErrorCode::ModelRegistryEntryNotFound,
                "Bu model görevi için TaskProfile bulunamadı.",
                Some(format!("task_profile_id={}", task.id())),
                Some("Model platform ayarlarını yeniden oluşturun.".to_string()),
            ))?;

        let binding = snapshot
            .bindings
            .iter()
            .find(|item| item.task_profile_id == task_profile.id && item.enabled)
            .cloned()
            .ok_or_else(|| route_error(
                AppErrorCode::ModelBindingNotFound,
                "Bu görev için etkin model ataması bulunamadı.",
                Some(format!("task_profile_id={}", task_profile.id)),
                Some("Yerel Modeller > Görev Atamaları bölümünden model seçin.".to_string()),
            ))?;

        let model = snapshot
            .models
            .iter()
            .find(|item| item.id == binding.model_definition_id)
            .cloned()
            .ok_or_else(|| route_error(
                AppErrorCode::ModelRegistryEntryNotFound,
                "Göreve atanmış model registry'de bulunamadı.",
                Some(format!("model_definition_id={}", binding.model_definition_id)),
                Some("Görev atamasını geçerli bir modele değiştirin.".to_string()),
            ))?;

        let runtime = snapshot
            .runtimes
            .iter()
            .find(|item| item.id == binding.runtime_definition_id)
            .cloned()
            .ok_or_else(|| route_error(
                AppErrorCode::ModelBindingUnavailable,
                "Göreve atanmış runtime registry'de bulunamadı.",
                Some(format!("runtime_definition_id={}", binding.runtime_definition_id)),
                Some("Görev atamasını geçerli bir runtime ile güncelleyin.".to_string()),
            ))?;

        if task.contains_student_data()
            && runtime.privacy_mode == PrivacyMode::StrictLocal
            && !is_loopback_host(&runtime.host)
        {
            return Err(route_error(
                AppErrorCode::ModelPrivacyBlocked,
                "Öğrenci verisi Strict Local sınırı dışındaki runtime'a gönderilemez.",
                Some(format!("host={}", runtime.host)),
                Some("Loopback yerel runtime seçin.".to_string()),
            ));
        }

        // The caller requests the safe Production route by default. A binding
        // can explicitly opt a non-production model into an experiment. This
        // is not a fallback: the persisted binding is the user's explicit
        // authorization and the resolved route records ExplicitExperiment.
        let effective_usage_mode = if usage_mode == RouteUsageMode::Production
            && binding.allow_experimental_student_data
            && model.lifecycle_state != ModelLifecycleState::Production
        {
            RouteUsageMode::ExplicitExperiment
        } else {
            usage_mode
        };

        enforce_lifecycle(&model, &binding, task, effective_usage_mode)?;

        let runtime_fingerprint = fingerprint_runtime_definition(&runtime);
        let manifest = snapshot
            .capability_manifests
            .iter()
            .find(|manifest| {
                manifest.model_definition_id == model.id
                    && manifest.runtime_definition_id == runtime.id
                    && manifest.model_fingerprint == model.model_fingerprint
                    && manifest.runtime_fingerprint == runtime_fingerprint
            })
            .cloned()
            .ok_or_else(|| route_error(
                AppErrorCode::ModelCapabilityUnverified,
                "Seçili model/runtime için güncel capability doğrulaması yok.",
                Some(format!(
                    "model_definition_id={}; runtime_definition_id={}",
                    model.id, runtime.id
                )),
                Some("Capability probe'u yeniden çalıştırın.".to_string()),
            ))?;

        if !manifest.satisfies(&task_profile.required_capabilities) {
            return Err(route_error(
                AppErrorCode::ModelCapabilityMismatch,
                "Model bu görevin gerekli capability'lerini karşılamıyor.",
                Some(format!("task_profile_id={}", task_profile.id)),
                Some("Uyumlu bir model seçin veya capability probe'u yenileyin.".to_string()),
            ));
        }

        let benchmark_verified = snapshot.benchmark_results.iter().any(|result| {
            result.task_profile_id == task_profile.id
                && result.model_definition_id == model.id
                && result.runtime_definition_id == runtime.id
                && result.model_fingerprint == model.model_fingerprint
                && result.runtime_fingerprint == runtime_fingerprint
                && result.state == BenchmarkGateState::Pass
        });
        let grandfathered = model
            .metadata
            .get("baselineVerified")
            .map(|value| value == "true")
            .unwrap_or(false);
        if effective_usage_mode == RouteUsageMode::Production
            && !benchmark_verified
            && !grandfathered
        {
            return Err(route_error(
                AppErrorCode::ModelBenchmarkRequired,
                "Seçili model bu görev için benchmark promotion gate'ini geçmemiş.",
                Some(format!("task_profile_id={}", task_profile.id)),
                Some("Golden benchmark çalıştırın veya production modeli seçin.".to_string()),
            ));
        }

        Ok(ResolvedModelRoute {
            task_profile,
            binding,
            model,
            runtime,
            capability_manifest: manifest,
            benchmark_verified: benchmark_verified || grandfathered,
            usage_mode: effective_usage_mode,
        })
    }
}

fn enforce_lifecycle(
    model: &ModelDefinition,
    binding: &TaskModelBinding,
    task: ModelTaskKind,
    usage_mode: RouteUsageMode,
) -> Result<(), AppError> {
    match usage_mode {
        RouteUsageMode::Production => {
            if !model.lifecycle_state.may_receive_production_student_data()
                && task.contains_student_data()
            {
                return Err(route_error(
                    AppErrorCode::ModelNotProductionApproved,
                    "Production öğrenci verisi yalnız Production modeline gönderilebilir.",
                    Some(format!(
                        "model_definition_id={}; lifecycle={:?}",
                        model.id, model.lifecycle_state
                    )),
                    Some("Production onaylı bir model seçin.".to_string()),
                ));
            }
            if model.lifecycle_state != ModelLifecycleState::Production {
                return Err(route_error(
                    AppErrorCode::ModelNotProductionApproved,
                    "Production akışı production onaylı model gerektiriyor.",
                    Some(format!("lifecycle={:?}", model.lifecycle_state)),
                    Some("Modeli benchmark gate sonrası Production'a yükseltin.".to_string()),
                ));
            }
        }
        RouteUsageMode::ExplicitExperiment => {
            if task.contains_student_data() {
                if !binding.allow_experimental_student_data {
                    return Err(route_error(
                        AppErrorCode::ModelBindingUnavailable,
                        "Experimental model için öğrenci verisi kullanımı açıkça onaylanmamış.",
                        Some(format!("binding_id={}", binding.id)),
                        Some("Görev atamasında güvenli deney kullanımını açıkça etkinleştirin.".to_string()),
                    ));
                }
                if !model.lifecycle_state.may_receive_explicit_experiment_student_data() {
                    return Err(route_error(
                        AppErrorCode::ModelNotProductionApproved,
                        "Bu model lifecycle durumunda öğrenci verisi alamaz.",
                        Some(format!("lifecycle={:?}", model.lifecycle_state)),
                        Some("Önce capability probe'u tamamlayıp modeli Experimental yapın.".to_string()),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    let normalized = host
        .trim()
        .trim_matches(|character| character == '[' || character == ']')
        .to_ascii_lowercase();
    matches!(normalized.as_str(), "127.0.0.1" | "::1" | "localhost")
}

fn route_error(
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

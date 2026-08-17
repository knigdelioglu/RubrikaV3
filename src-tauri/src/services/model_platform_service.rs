use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model_platform::{
    fingerprint_model_definition, fingerprint_runtime_definition, BenchmarkGateState,
    BenchmarkResultSummary, CapabilityManifest, CapabilitySupport, ModelCapabilityKind,
    ModelDefinition, ModelLifecycleState, ModelPlatformConfig, ModelTaskKind, RuntimeDefinition,
    TaskModelBinding, TaskProfile, MODEL_PLATFORM_SCHEMA_VERSION,
};
use crate::platform::file_access::atomic_write;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportModelInput {
    pub id: String,
    pub family: String,
    pub display_name: String,
    pub model_path: String,
    #[serde(default)]
    pub mmproj_path: Option<String>,
    #[serde(default)]
    pub quantization: Option<String>,
    #[serde(default)]
    pub context_limit: Option<u32>,
    #[serde(default)]
    pub declared_text: bool,
    #[serde(default)]
    pub declared_vision: bool,
    #[serde(default)]
    pub declared_structured_json: bool,
    #[serde(default)]
    pub declared_json_schema: bool,
    #[serde(default)]
    pub declared_thinking_control: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindTaskInput {
    pub task_profile_id: String,
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    #[serde(default)]
    pub allow_experimental_student_data: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionDecision {
    pub allowed: bool,
    pub model_definition_id: String,
    pub checked_task_profiles: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Clone)]
pub struct ModelPlatformService {
    config_path: PathBuf,
    store: Arc<Mutex<ModelPlatformConfig>>,
}

impl ModelPlatformService {
    pub fn new() -> Self {
        Self::new_with_path(model_platform_config_path())
    }

    pub fn new_with_path(config_path: PathBuf) -> Self {
        let store = load_store(&config_path).unwrap_or_default();
        Self {
            config_path,
            store: Arc::new(Mutex::new(store)),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn snapshot(&self) -> Result<ModelPlatformConfig, AppError> {
        Ok(self.lock()?.clone())
    }

    pub fn replace_config(&self, config: ModelPlatformConfig) -> Result<(), AppError> {
        if config.schema_version != MODEL_PLATFORM_SCHEMA_VERSION {
            return Err(platform_error(
                AppErrorCode::ModelConfigMissing,
                "Model platform config sürümü desteklenmiyor.",
                Some(format!("schema_version={}", config.schema_version)),
                Some("Model platform ayarlarını yeniden oluşturun.".to_string()),
            ));
        }
        let mut store = self.lock()?;
        *store = config;
        self.save_locked(&store)
    }

    pub fn import_model(&self, input: ImportModelInput) -> Result<ModelDefinition, AppError> {
        let model_path = PathBuf::from(input.model_path.trim());
        if !model_path.is_file() {
            return Err(platform_error(
                AppErrorCode::ModelFileMissing,
                "Seçilen model dosyası bulunamadı.",
                Some(format!("model_path={}", model_path.to_string_lossy())),
                Some("Geçerli bir GGUF model dosyası seçin.".to_string()),
            ));
        }
        let mmproj_path = input
            .mmproj_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if input.declared_vision {
            if let Some(path) = mmproj_path.as_deref() {
                if !Path::new(path).is_file() {
                    return Err(platform_error(
                        AppErrorCode::ModelMmprojMissing,
                        "Seçilen multimodal projector dosyası bulunamadı.",
                        Some(format!("mmproj_path={path}")),
                        Some("Doğru mmproj dosyasını seçin.".to_string()),
                    ));
                }
            }
        }

        let id = normalize_id(&input.id);
        if id.is_empty() {
            return Err(platform_error(
                AppErrorCode::ModelConfigMissing,
                "Model kimliği boş olamaz.",
                None,
                Some("Model için kısa ve benzersiz bir kimlik girin.".to_string()),
            ));
        }

        let mut model = ModelDefinition {
            id: id.clone(),
            family: normalize_id(&input.family),
            display_name: input.display_name.trim().to_string(),
            model_path: model_path.to_string_lossy().to_string(),
            mmproj_path,
            format: crate::domain::model_platform::ModelFormat::Gguf,
            quantization: input.quantization,
            capabilities: crate::domain::model_platform::ModelCapabilities {
                text: input.declared_text || true,
                vision: input.declared_vision,
                structured_json: input.declared_structured_json,
                json_schema: input.declared_json_schema,
                thinking_control: input.declared_thinking_control,
                multimodal_projector_required: input.declared_vision,
            },
            context_limit: input.context_limit,
            metadata: std::collections::BTreeMap::new(),
            model_fingerprint: fingerprint_file(&model_path)?,
            lifecycle_state: ModelLifecycleState::Imported,
        };
        if model.display_name.is_empty() {
            model.display_name = id.clone();
        }
        if model.family.is_empty() {
            model.family = "unknown".to_string();
        }
        model.metadata.insert(
            "definitionFingerprint".to_string(),
            fingerprint_model_definition(&model),
        );

        let mut store = self.lock()?;
        if let Some(existing) = store.models.iter_mut().find(|item| item.id == id) {
            *existing = model.clone();
            store
                .capability_manifests
                .retain(|manifest| manifest.model_definition_id != id);
            store
                .benchmark_results
                .retain(|result| result.model_definition_id != id);
        } else {
            store.models.push(model.clone());
        }
        self.save_locked(&store)?;
        Ok(model)
    }

    pub fn upsert_model_definition(&self, mut model: ModelDefinition) -> Result<(), AppError> {
        model.metadata.insert(
            "definitionFingerprint".to_string(),
            fingerprint_model_definition(&model),
        );
        let mut store = self.lock()?;
        if let Some(existing) = store.models.iter_mut().find(|item| item.id == model.id) {
            *existing = model;
        } else {
            store.models.push(model);
        }
        self.save_locked(&store)
    }

    pub fn upsert_runtime_definition(&self, runtime: RuntimeDefinition) -> Result<(), AppError> {
        if runtime.id.trim().is_empty() {
            return Err(platform_error(
                AppErrorCode::ModelConfigMissing,
                "Runtime kimliği boş olamaz.",
                None,
                Some("Runtime için benzersiz bir kimlik girin.".to_string()),
            ));
        }
        let mut store = self.lock()?;
        if let Some(existing) = store.runtimes.iter_mut().find(|item| item.id == runtime.id) {
            *existing = runtime;
        } else {
            store.runtimes.push(runtime);
        }
        self.save_locked(&store)
    }

    pub fn bind_task(&self, input: BindTaskInput) -> Result<TaskModelBinding, AppError> {
        let mut store = self.lock()?;
        require_task_profile(&store, &input.task_profile_id)?;
        require_model(&store, &input.model_definition_id)?;
        require_runtime(&store, &input.runtime_definition_id)?;

        let id = format!(
            "binding-{}-{}-{}",
            normalize_id(&input.task_profile_id),
            normalize_id(&input.model_definition_id),
            normalize_id(&input.runtime_definition_id)
        );
        let binding = TaskModelBinding {
            id,
            task_profile_id: input.task_profile_id.clone(),
            model_definition_id: input.model_definition_id.clone(),
            runtime_definition_id: input.runtime_definition_id.clone(),
            allow_experimental_student_data: input.allow_experimental_student_data,
            enabled: true,
        };

        for existing in store
            .bindings
            .iter_mut()
            .filter(|item| item.task_profile_id == input.task_profile_id)
        {
            existing.enabled = false;
        }
        if let Some(existing) = store.bindings.iter_mut().find(|item| item.id == binding.id) {
            *existing = binding.clone();
        } else {
            store.bindings.push(binding.clone());
        }
        self.save_locked(&store)?;
        Ok(binding)
    }

    pub fn disable_binding(&self, binding_id: &str) -> Result<(), AppError> {
        let mut store = self.lock()?;
        let binding = store
            .bindings
            .iter_mut()
            .find(|item| item.id == binding_id)
            .ok_or_else(|| platform_error(
                AppErrorCode::ModelProfileNotFound,
                "Model görev ataması bulunamadı.",
                Some(format!("binding_id={binding_id}")),
                Some("Görev atamalarını yenileyin.".to_string()),
            ))?;
        binding.enabled = false;
        self.save_locked(&store)
    }

    pub fn record_capability_manifest(&self, manifest: CapabilityManifest) -> Result<(), AppError> {
        let mut store = self.lock()?;
        let model = require_model(&store, &manifest.model_definition_id)?;
        let runtime = require_runtime(&store, &manifest.runtime_definition_id)?;
        if model.model_fingerprint != manifest.model_fingerprint
            || fingerprint_runtime_definition(runtime) != manifest.runtime_fingerprint
        {
            return Err(platform_error(
                AppErrorCode::ModelConfigMissing,
                "Capability sonucu güncel model/runtime fingerprint'i ile eşleşmiyor.",
                Some(format!("model_definition_id={}", manifest.model_definition_id)),
                Some("Capability probe'u yeniden çalıştırın.".to_string()),
            ));
        }
        store.capability_manifests.retain(|item| {
            !(item.model_definition_id == manifest.model_definition_id
                && item.runtime_definition_id == manifest.runtime_definition_id)
        });
        store.capability_manifests.push(manifest);
        self.save_locked(&store)
    }

    pub fn record_benchmark_result(&self, result: BenchmarkResultSummary) -> Result<(), AppError> {
        let mut store = self.lock()?;
        require_task_profile(&store, &result.task_profile_id)?;
        let model = require_model(&store, &result.model_definition_id)?;
        let runtime = require_runtime(&store, &result.runtime_definition_id)?;
        if model.model_fingerprint != result.model_fingerprint
            || fingerprint_runtime_definition(runtime) != result.runtime_fingerprint
        {
            return Err(platform_error(
                AppErrorCode::ModelConfigMissing,
                "Benchmark sonucu güncel fingerprint ile eşleşmiyor.",
                Some(format!("benchmark_id={}", result.id)),
                Some("Benchmark'ı güncel model/runtime ile yeniden çalıştırın.".to_string()),
            ));
        }
        store.benchmark_results.retain(|item| {
            !(item.task_profile_id == result.task_profile_id
                && item.model_definition_id == result.model_definition_id
                && item.runtime_definition_id == result.runtime_definition_id)
        });
        store.benchmark_results.push(result);
        self.save_locked(&store)
    }

    pub fn set_model_lifecycle(
        &self,
        model_definition_id: &str,
        target: ModelLifecycleState,
    ) -> Result<ModelDefinition, AppError> {
        if target == ModelLifecycleState::Production {
            let decision = self.production_promotion_decision(model_definition_id)?;
            if !decision.allowed {
                return Err(platform_error(
                    AppErrorCode::ModelConfigMissing,
                    "Model production promotion gate'ini geçemedi.",
                    Some(decision.reasons.join("; ")),
                    Some("Eksik capability probe veya benchmark adımlarını tamamlayın.".to_string()),
                ));
            }
        }

        let mut store = self.lock()?;
        let model = store
            .models
            .iter_mut()
            .find(|item| item.id == model_definition_id)
            .ok_or_else(|| platform_error(
                AppErrorCode::ModelProfileNotFound,
                "Model registry kaydı bulunamadı.",
                Some(format!("model_definition_id={model_definition_id}")),
                Some("Modeli yeniden ekleyin.".to_string()),
            ))?;
        model.lifecycle_state = target;
        let output = model.clone();
        self.save_locked(&store)?;
        Ok(output)
    }

    pub fn production_promotion_decision(
        &self,
        model_definition_id: &str,
    ) -> Result<PromotionDecision, AppError> {
        let store = self.lock()?;
        let model = require_model(&store, model_definition_id)?;
        let enabled_bindings: Vec<_> = store
            .bindings
            .iter()
            .filter(|binding| binding.enabled && binding.model_definition_id == model_definition_id)
            .collect();
        let mut reasons = vec![];
        let mut checked = vec![];

        if enabled_bindings.is_empty() {
            reasons.push("Model herhangi bir task'a atanmış değil.".to_string());
        }

        for binding in enabled_bindings {
            let task = require_task_profile(&store, &binding.task_profile_id)?;
            let runtime = require_runtime(&store, &binding.runtime_definition_id)?;
            checked.push(task.id.clone());
            let runtime_fingerprint = fingerprint_runtime_definition(runtime);
            let manifest = store.capability_manifests.iter().find(|manifest| {
                manifest.model_definition_id == model.id
                    && manifest.runtime_definition_id == runtime.id
                    && manifest.model_fingerprint == model.model_fingerprint
                    && manifest.runtime_fingerprint == runtime_fingerprint
            });
            match manifest {
                Some(manifest) if manifest.satisfies(&task.required_capabilities) => {}
                Some(_) => reasons.push(format!(
                    "{} için required capability sonucu PASS değil.",
                    task.id
                )),
                None => reasons.push(format!("{} için güncel capability probe yok.", task.id)),
            }

            let benchmark = store.benchmark_results.iter().find(|result| {
                result.task_profile_id == task.id
                    && result.model_definition_id == model.id
                    && result.runtime_definition_id == runtime.id
                    && result.model_fingerprint == model.model_fingerprint
                    && result.runtime_fingerprint == runtime_fingerprint
                    && result.state == BenchmarkGateState::Pass
            });
            if benchmark.is_none() && !is_grandfathered_baseline(model) {
                reasons.push(format!("{} için PASS benchmark sonucu yok.", task.id));
            }
        }

        Ok(PromotionDecision {
            allowed: reasons.is_empty(),
            model_definition_id: model_definition_id.to_string(),
            checked_task_profiles: checked,
            reasons,
        })
    }

    pub fn mark_probe_started(&self, model_definition_id: &str) -> Result<(), AppError> {
        self.force_lifecycle(model_definition_id, ModelLifecycleState::Probing)
    }

    pub fn mark_probe_finished(
        &self,
        model_definition_id: &str,
        manifest: &CapabilityManifest,
    ) -> Result<(), AppError> {
        let any_fail = manifest
            .results
            .iter()
            .any(|item| item.support == CapabilitySupport::Fail);
        let target = if any_fail {
            ModelLifecycleState::Unsupported
        } else {
            ModelLifecycleState::Compatible
        };
        self.force_lifecycle(model_definition_id, target)
    }

    pub fn mark_probe_failed(&self, model_definition_id: &str) -> Result<(), AppError> {
        self.force_lifecycle(model_definition_id, ModelLifecycleState::ProbeFailed)
    }

    fn force_lifecycle(
        &self,
        model_definition_id: &str,
        target: ModelLifecycleState,
    ) -> Result<(), AppError> {
        let mut store = self.lock()?;
        let model = store
            .models
            .iter_mut()
            .find(|item| item.id == model_definition_id)
            .ok_or_else(|| platform_error(
                AppErrorCode::ModelProfileNotFound,
                "Model registry kaydı bulunamadı.",
                Some(format!("model_definition_id={model_definition_id}")),
                Some("Modeli yeniden ekleyin.".to_string()),
            ))?;
        model.lifecycle_state = target;
        self.save_locked(&store)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ModelPlatformConfig>, AppError> {
        self.store.lock().map_err(|error| platform_error(
            AppErrorCode::ModelStateAccessFailed,
            "Model platform durumuna erişilemedi.",
            Some(error.to_string()),
            Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
        ))
    }

    fn save_locked(&self, config: &ModelPlatformConfig) -> Result<(), AppError> {
        let content = serde_json::to_string_pretty(config).map_err(|error| platform_error(
            AppErrorCode::FileWriteFailed,
            "Model platform ayarları serileştirilemedi.",
            Some(error.to_string()),
            Some("Ayarları yeniden kaydetmeyi deneyin.".to_string()),
        ))?;
        atomic_write(&self.config_path, &content).map_err(|error| platform_error(
            AppErrorCode::FileWriteFailed,
            "Model platform ayarları kaydedilemedi.",
            Some(error.to_string()),
            Some("Disk izinlerini ve boş alanı kontrol edin.".to_string()),
        ))
    }
}

impl Default for ModelPlatformService {
    fn default() -> Self {
        Self::new()
    }
}

fn load_store(path: &Path) -> Result<ModelPlatformConfig, AppError> {
    if !path.exists() {
        return Ok(ModelPlatformConfig::default());
    }
    let content = std::fs::read_to_string(path).map_err(|error| platform_error(
        AppErrorCode::FileReadFailed,
        "Model platform ayarları okunamadı.",
        Some(error.to_string()),
        Some("Ayar dosyasını kontrol edin.".to_string()),
    ))?;
    let config: ModelPlatformConfig = serde_json::from_str(&content).map_err(|error| platform_error(
        AppErrorCode::ModelConfigMissing,
        "Model platform ayar dosyası bozuk.",
        Some(error.to_string()),
        Some("Yedekten geri dönün veya model platform ayarlarını yeniden oluşturun.".to_string()),
    ))?;
    if config.schema_version != MODEL_PLATFORM_SCHEMA_VERSION {
        return Err(platform_error(
            AppErrorCode::ModelConfigMissing,
            "Model platform config sürümü desteklenmiyor.",
            Some(format!("schema_version={}", config.schema_version)),
            Some("Model platform migration'ını çalıştırın.".to_string()),
        ));
    }
    Ok(config)
}

fn model_platform_config_path() -> PathBuf {
    if let Some(path) = env::var_os("RUBRIKA_V3_MODEL_PLATFORM_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    let base = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Library")
        .join("Application Support")
        .join("RubrikaV3")
        .join("model_platform.json")
}

fn require_task_profile<'a>(
    store: &'a ModelPlatformConfig,
    id: &str,
) -> Result<&'a TaskProfile, AppError> {
    store.task_profiles.iter().find(|item| item.id == id).ok_or_else(|| platform_error(
        AppErrorCode::ModelProfileNotFound,
        "Task profile bulunamadı.",
        Some(format!("task_profile_id={id}")),
        Some("Model platform ayarlarını yenileyin.".to_string()),
    ))
}

fn require_model<'a>(
    store: &'a ModelPlatformConfig,
    id: &str,
) -> Result<&'a ModelDefinition, AppError> {
    store.models.iter().find(|item| item.id == id).ok_or_else(|| platform_error(
        AppErrorCode::ModelProfileNotFound,
        "Model registry kaydı bulunamadı.",
        Some(format!("model_definition_id={id}")),
        Some("Modeli yeniden ekleyin.".to_string()),
    ))
}

fn require_runtime<'a>(
    store: &'a ModelPlatformConfig,
    id: &str,
) -> Result<&'a RuntimeDefinition, AppError> {
    store.runtimes.iter().find(|item| item.id == id).ok_or_else(|| platform_error(
        AppErrorCode::ModelProfileNotFound,
        "Runtime registry kaydı bulunamadı.",
        Some(format!("runtime_definition_id={id}")),
        Some("Runtime ayarını yeniden oluşturun.".to_string()),
    ))
}

fn is_grandfathered_baseline(model: &ModelDefinition) -> bool {
    model
        .metadata
        .get("baselineVerified")
        .map(|value| value == "true")
        .unwrap_or(false)
}

pub fn task_kind_for_id(id: &str) -> Option<ModelTaskKind> {
    [
        ModelTaskKind::QuestionTextExtraction,
        ModelTaskKind::RubricExtraction,
        ModelTaskKind::StudentAnswerOcr,
        ModelTaskKind::StudentAnswerOcrIssueCorrection,
        ModelTaskKind::SemanticScoring,
        ModelTaskKind::SpeakingTranscriptCleanup,
        ModelTaskKind::SpeakingEvaluation,
        ModelTaskKind::Analysis,
        ModelTaskKind::GeneralText,
    ]
    .into_iter()
    .find(|kind| kind.id() == id)
}

pub fn required_capabilities_for_task(task: &TaskProfile) -> BTreeSet<ModelCapabilityKind> {
    task.required_capabilities.clone()
}

fn fingerprint_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|error| platform_error(
        AppErrorCode::FileReadFailed,
        "Model dosyası fingerprint için açılamadı.",
        Some(error.to_string()),
        Some("Model dosyası izinlerini kontrol edin.".to_string()),
    ))?;
    let metadata = file.metadata().map_err(|error| platform_error(
        AppErrorCode::FileReadFailed,
        "Model dosyası bilgileri okunamadı.",
        Some(error.to_string()),
        Some("Model dosyası izinlerini kontrol edin.".to_string()),
    ))?;

    // Multi-gigabyte GGUF files are fingerprinted from stable identity signals
    // plus the first/last 1 MiB. This detects normal replacement/update cases
    // without hashing the entire model every time the settings screen opens.
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
            hasher.update(duration.as_nanos().to_le_bytes());
        }
    }
    let sample = 1024 * 1024usize;
    let mut buffer = vec![0u8; sample.min(metadata.len() as usize)];
    if !buffer.is_empty() {
        file.read_exact(&mut buffer).map_err(|error| platform_error(
            AppErrorCode::FileReadFailed,
            "Model fingerprint başlangıç örneği okunamadı.",
            Some(error.to_string()),
            None,
        ))?;
        hasher.update(&buffer);
    }
    if metadata.len() > sample as u64 {
        let tail_len = sample.min(metadata.len() as usize);
        file.seek(SeekFrom::End(-(tail_len as i64))).map_err(|error| platform_error(
            AppErrorCode::FileReadFailed,
            "Model fingerprint son örneğine erişilemedi.",
            Some(error.to_string()),
            None,
        ))?;
        let mut tail = vec![0u8; tail_len];
        file.read_exact(&mut tail).map_err(|error| platform_error(
            AppErrorCode::FileReadFailed,
            "Model fingerprint son örneği okunamadı.",
            Some(error.to_string()),
            None,
        ))?;
        hasher.update(&tail);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn normalize_id(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_matches('-').to_string()
}

fn platform_error(
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

pub fn new_benchmark_result_id(task_profile_id: &str, model_definition_id: &str) -> String {
    format!(
        "benchmark-{}-{}-{}",
        normalize_id(task_profile_id),
        normalize_id(model_definition_id),
        Utc::now().timestamp_millis()
    )
}

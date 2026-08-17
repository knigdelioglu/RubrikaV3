use crate::domain::model::{
    ModelMode, ModelProfile, ModelResponseFormat, PrivacyMode, SamplingParameters,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MODEL_PLATFORM_SCHEMA_VERSION: &str = "rubrika.model-platform.v2";
pub const DEFAULT_LLAMA_RUNTIME_ID: &str = "llama-local-default";
pub const CANONICAL_GEMMA4_12B_MODEL_ID: &str = "gemma4-12b";
pub const BENCHMARK_POLICY_VERSION: &str = "model_benchmark_policy_v1";

pub const LEGACY_GEMMA_PROFILE_IDS: [&str; 3] = [
    "gemma4-ocr-q8",
    "speaking_transcript_cleanup_12b",
    "speaking_rubric_evaluation_12b",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapabilityKind {
    Text,
    Vision,
    StructuredJson,
    JsonSchema,
    ThinkingControl,
    MultimodalProjector,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Unverified,
    Pass,
    Partial,
    Fail,
}

impl CapabilitySupport {
    pub fn is_production_ready(self) -> bool {
        matches!(self, Self::Pass)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub text: bool,
    pub vision: bool,
    pub structured_json: bool,
    pub json_schema: bool,
    pub thinking_control: bool,
    pub multimodal_projector_required: bool,
}

impl ModelCapabilities {
    pub fn legacy_multimodal() -> Self {
        Self {
            text: true,
            vision: true,
            structured_json: true,
            json_schema: false,
            thinking_control: true,
            multimodal_projector_required: true,
        }
    }

    pub fn legacy_text_only() -> Self {
        Self {
            text: true,
            vision: false,
            structured_json: true,
            json_schema: false,
            thinking_control: true,
            multimodal_projector_required: false,
        }
    }

    pub fn supports(&self, capability: ModelCapabilityKind) -> bool {
        match capability {
            ModelCapabilityKind::Text => self.text,
            ModelCapabilityKind::Vision => self.vision,
            ModelCapabilityKind::StructuredJson => self.structured_json,
            ModelCapabilityKind::JsonSchema => self.json_schema,
            ModelCapabilityKind::ThinkingControl => self.thinking_control,
            ModelCapabilityKind::MultimodalProjector => self.multimodal_projector_required,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Gguf,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelLifecycleState {
    Imported,
    Probing,
    Compatible,
    Experimental,
    BenchmarkVerified,
    Production,
    Unsupported,
    ProbeFailed,
    BenchmarkFailed,
    Disabled,
}

impl ModelLifecycleState {
    pub fn may_receive_production_student_data(self) -> bool {
        matches!(self, Self::Production)
    }

    pub fn may_receive_explicit_experiment_student_data(self) -> bool {
        matches!(self, Self::Experimental | Self::BenchmarkVerified | Self::Production)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelDefinition {
    pub id: String,
    pub family: String,
    pub display_name: String,
    pub model_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mmproj_path: Option<String>,
    pub format: ModelFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    pub capabilities: ModelCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_limit: Option<u32>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub model_fingerprint: String,
    pub lifecycle_state: ModelLifecycleState,
}

impl ModelDefinition {
    pub fn refresh_fingerprint(&mut self) {
        self.model_fingerprint = fingerprint_model_definition(self);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEngine {
    LlamaCpp,
    Mlx,
    ExternalOpenAiCompatible,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    Off,
    On,
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlashAttentionMode {
    Off,
    On,
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalProjectorMode {
    Enabled,
    Disabled,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDefinition {
    pub id: String,
    pub engine: RuntimeEngine,
    pub server_path: String,
    pub host: String,
    pub port: u16,
    pub context_size: u32,
    pub gpu_layers: i32,
    pub flash_attention: FlashAttentionMode,
    pub parallel: u32,
    pub batch_size: u32,
    pub ubatch_size: u32,
    pub kv_cache_type_k: String,
    pub kv_cache_type_v: String,
    pub reasoning_mode: ReasoningMode,
    #[serde(default)]
    pub multimodal_projector_mode: MultimodalProjectorMode,
    #[serde(default)]
    pub image_min_tokens: Option<u32>,
    #[serde(default)]
    pub image_max_tokens: Option<u32>,
    #[serde(default)]
    pub cache_ram_megabytes: Option<u32>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    pub privacy_mode: PrivacyMode,
    pub managed: bool,
}

impl RuntimeDefinition {
    pub fn legacy_standard(profile: &ModelProfile) -> Self {
        Self {
            id: DEFAULT_LLAMA_RUNTIME_ID.to_string(),
            engine: RuntimeEngine::LlamaCpp,
            server_path: profile.server_path.clone(),
            host: profile.host.clone(),
            port: profile.port,
            context_size: 4096,
            gpu_layers: 99,
            flash_attention: FlashAttentionMode::On,
            parallel: 1,
            batch_size: 1536,
            ubatch_size: 1280,
            kv_cache_type_k: "q8_0".to_string(),
            kv_cache_type_v: "q8_0".to_string(),
            reasoning_mode: ReasoningMode::Off,
            multimodal_projector_mode: MultimodalProjectorMode::Enabled,
            image_min_tokens: Some(1120),
            image_max_tokens: Some(1120),
            cache_ram_megabytes: Some(0),
            extra_args: vec![
                "--temp".to_string(),
                "0".to_string(),
                "--top-p".to_string(),
                "1".to_string(),
                "--top-k".to_string(),
                "1".to_string(),
                "--repeat-penalty".to_string(),
                "1.0".to_string(),
                "-n".to_string(),
                "768".to_string(),
            ],
            privacy_mode: profile.privacy_mode,
            managed: matches!(profile.mode, ModelMode::Managed),
        }
    }

    pub fn legacy_text_only(profile: &ModelProfile) -> Self {
        Self {
            id: DEFAULT_LLAMA_RUNTIME_ID.to_string(),
            engine: RuntimeEngine::LlamaCpp,
            server_path: profile.server_path.clone(),
            host: profile.host.clone(),
            port: profile.port,
            context_size: 8192,
            gpu_layers: 99,
            flash_attention: FlashAttentionMode::Auto,
            parallel: 1,
            batch_size: 512,
            ubatch_size: 256,
            kv_cache_type_k: "turbo3".to_string(),
            kv_cache_type_v: "turbo3".to_string(),
            reasoning_mode: ReasoningMode::Off,
            multimodal_projector_mode: MultimodalProjectorMode::Disabled,
            image_min_tokens: None,
            image_max_tokens: None,
            cache_ram_megabytes: None,
            extra_args: vec![
                "--repeat-penalty".to_string(),
                "1.05".to_string(),
                "--no-cache-prompt".to_string(),
                "-cram".to_string(),
                "0".to_string(),
                "-ctxcp".to_string(),
                "0".to_string(),
            ],
            privacy_mode: profile.privacy_mode,
            managed: matches!(profile.mode, ModelMode::Managed),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    pub fn uses_multimodal_projector(&self, model: &ModelDefinition) -> bool {
        match self.multimodal_projector_mode {
            MultimodalProjectorMode::Enabled => true,
            MultimodalProjectorMode::Disabled => false,
            MultimodalProjectorMode::Auto => {
                model.capabilities.multimodal_projector_required
                    && (self.image_min_tokens.is_some() || self.image_max_tokens.is_some())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModelTaskKind {
    QuestionTextExtraction,
    RubricExtraction,
    StudentAnswerOcr,
    StudentAnswerOcrIssueCorrection,
    SemanticScoring,
    SpeakingTranscriptCleanup,
    SpeakingEvaluation,
    Analysis,
    GeneralText,
}

impl ModelTaskKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::QuestionTextExtraction => "question_text_extraction",
            Self::RubricExtraction => "rubric_extraction",
            Self::StudentAnswerOcr => "student_answer_ocr",
            Self::StudentAnswerOcrIssueCorrection => "student_answer_ocr_issue_correction",
            Self::SemanticScoring => "semantic_scoring",
            Self::SpeakingTranscriptCleanup => "speaking_transcript_cleanup",
            Self::SpeakingEvaluation => "speaking_evaluation",
            Self::Analysis => "analysis",
            Self::GeneralText => "general_text",
        }
    }

    pub fn contains_student_data(self) -> bool {
        matches!(
            self,
            Self::StudentAnswerOcr
                | Self::StudentAnswerOcrIssueCorrection
                | Self::SemanticScoring
                | Self::SpeakingTranscriptCleanup
                | Self::SpeakingEvaluation
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskProfile {
    pub id: String,
    pub use_case: ModelTaskKind,
    pub required_capabilities: BTreeSet<ModelCapabilityKind>,
    pub prompt_version: String,
    pub schema_version: String,
    pub policy_version: String,
    pub sampling_parameters: SamplingParameters,
    pub timeout_seconds: u64,
    pub response_format: Option<ModelResponseFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskModelBinding {
    pub id: String,
    pub task_profile_id: String,
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    #[serde(default)]
    pub allow_experimental_student_data: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProbeResult {
    pub capability: ModelCapabilityKind,
    pub support: CapabilitySupport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityManifest {
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    pub model_fingerprint: String,
    pub runtime_fingerprint: String,
    pub verified_at: String,
    pub results: Vec<CapabilityProbeResult>,
}

impl CapabilityManifest {
    pub fn result_for(&self, capability: ModelCapabilityKind) -> CapabilitySupport {
        self.results
            .iter()
            .find(|item| item.capability == capability)
            .map(|item| item.support)
            .unwrap_or(CapabilitySupport::Unverified)
    }

    pub fn satisfies(&self, required: &BTreeSet<ModelCapabilityKind>) -> bool {
        required
            .iter()
            .all(|capability| self.result_for(*capability).is_production_ready())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkGateState {
    NotRun,
    Running,
    Pass,
    Fail,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkMetricValue {
    pub key: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_value: Option<f64>,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResultSummary {
    pub id: String,
    pub task_profile_id: String,
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    pub model_fingerprint: String,
    pub runtime_fingerprint: String,
    pub policy_version: String,
    pub state: BenchmarkGateState,
    pub generated_at: String,
    #[serde(default)]
    pub metrics: Vec<BenchmarkMetricValue>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelPlatformConfig {
    pub schema_version: String,
    #[serde(default)]
    pub models: Vec<ModelDefinition>,
    #[serde(default)]
    pub runtimes: Vec<RuntimeDefinition>,
    #[serde(default)]
    pub task_profiles: Vec<TaskProfile>,
    #[serde(default)]
    pub bindings: Vec<TaskModelBinding>,
    #[serde(default)]
    pub capability_manifests: Vec<CapabilityManifest>,
    #[serde(default)]
    pub benchmark_results: Vec<BenchmarkResultSummary>,
}

impl Default for ModelPlatformConfig {
    fn default() -> Self {
        Self {
            schema_version: MODEL_PLATFORM_SCHEMA_VERSION.to_string(),
            models: vec![],
            runtimes: vec![],
            task_profiles: default_task_profiles(),
            bindings: vec![],
            capability_manifests: vec![],
            benchmark_results: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyProfileMigration {
    pub model: ModelDefinition,
    pub runtime: RuntimeDefinition,
    pub bindings: Vec<TaskModelBinding>,
}

pub fn migrate_legacy_profile(profile: &ModelProfile) -> LegacyProfileMigration {
    let is_canonical_gemma = LEGACY_GEMMA_PROFILE_IDS.contains(&profile.id.as_str());
    let canonical_model_id = if is_canonical_gemma {
        CANONICAL_GEMMA4_12B_MODEL_ID.to_string()
    } else {
        format!("legacy-{}", sanitize_id(&profile.id))
    };
    let is_text_only = matches!(
        profile.id.as_str(),
        "speaking_transcript_cleanup_12b" | "speaking_rubric_evaluation_12b"
    );

    let capabilities = if is_text_only {
        ModelCapabilities::legacy_text_only()
    } else {
        ModelCapabilities::legacy_multimodal()
    };
    let mut model = ModelDefinition {
        id: canonical_model_id.clone(),
        family: if is_canonical_gemma {
            "gemma".to_string()
        } else {
            "legacy".to_string()
        },
        display_name: if is_canonical_gemma {
            "Gemma 4 12B".to_string()
        } else {
            profile.display_name.clone()
        },
        model_path: profile.model_path.clone(),
        mmproj_path: non_empty(&profile.mmproj_path),
        format: if profile.model_path.to_ascii_lowercase().ends_with(".gguf") {
            ModelFormat::Gguf
        } else {
            ModelFormat::Unknown
        },
        quantization: infer_quantization(&profile.model_path),
        capabilities,
        context_limit: None,
        metadata: BTreeMap::from([
            ("migrationSource".to_string(), "legacy_model_profile".to_string()),
            ("legacyProfileId".to_string(), profile.id.clone()),
        ]),
        model_fingerprint: String::new(),
        lifecycle_state: ModelLifecycleState::Production,
    };
    model.refresh_fingerprint();

    let runtime = if is_text_only {
        RuntimeDefinition::legacy_text_only(profile)
    } else {
        RuntimeDefinition::legacy_standard(profile)
    };

    let binding_tasks = match profile.id.as_str() {
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

    let bindings = binding_tasks
        .into_iter()
        .map(|task| TaskModelBinding {
            id: format!("binding-{}-{}", task.id(), canonical_model_id),
            task_profile_id: task.id().to_string(),
            model_definition_id: canonical_model_id.clone(),
            runtime_definition_id: runtime.id.clone(),
            allow_experimental_student_data: false,
            enabled: true,
        })
        .collect();

    LegacyProfileMigration {
        model,
        runtime,
        bindings,
    }
}

pub fn default_task_profiles() -> Vec<TaskProfile> {
    vec![
        task_profile(
            ModelTaskKind::QuestionTextExtraction,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::Vision, ModelCapabilityKind::StructuredJson],
            "question_text_extraction_typed_user_data",
            "question_text_extraction_v1",
            "model_policy_v1",
            0,
            1,
            1024,
            180,
        ),
        task_profile(
            ModelTaskKind::RubricExtraction,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::StructuredJson],
            "rubric_extraction_typed_user_data",
            "rubric_extraction_v1",
            "model_policy_v1",
            0,
            1,
            4096,
            300,
        ),
        task_profile(
            ModelTaskKind::StudentAnswerOcr,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::Vision, ModelCapabilityKind::StructuredJson],
            "student_answer_ocr_v4_typed_user_data",
            "student_answer_ocr_v4",
            "ocr_review_policy_v1",
            0,
            1,
            4096,
            300,
        ),
        task_profile(
            ModelTaskKind::StudentAnswerOcrIssueCorrection,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::Vision, ModelCapabilityKind::StructuredJson],
            "student_answer_ocr_issue_correction_typed_user_data",
            "student_answer_ocr_issue_correction_v1",
            "ocr_review_policy_v1",
            0,
            1,
            1536,
            180,
        ),
        task_profile(
            ModelTaskKind::SemanticScoring,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::StructuredJson],
            "scoring_v4_typed_user_data",
            "semantic_scoring_v4",
            "semantic_scoring_policy_v1",
            0,
            1,
            3072,
            300,
        ),
        task_profile(
            ModelTaskKind::SpeakingTranscriptCleanup,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::StructuredJson],
            "speaking_transcript_cleanup_typed_user_data",
            "speaking_transcript_cleanup_v1",
            "speaking_cleanup_policy_v1",
            0,
            1,
            3072,
            300,
        ),
        task_profile(
            ModelTaskKind::SpeakingEvaluation,
            &[ModelCapabilityKind::Text, ModelCapabilityKind::StructuredJson],
            "speaking_rubric_evidence_tr_v5_typed_user_data",
            "speaking_rubric_v5",
            "speaking_scoring_policy_v2",
            0,
            1,
            3072,
            300,
        ),
        task_profile(
            ModelTaskKind::Analysis,
            &[ModelCapabilityKind::Text],
            "analysis_typed_user_data",
            "analysis_v1",
            "model_policy_v1",
            0,
            1,
            4096,
            300,
        ),
        task_profile(
            ModelTaskKind::GeneralText,
            &[ModelCapabilityKind::Text],
            "general_text_typed_user_data",
            "general_text_v1",
            "model_policy_v1",
            0,
            1,
            2048,
            180,
        ),
    ]
}

fn task_profile(
    use_case: ModelTaskKind,
    required: &[ModelCapabilityKind],
    prompt_version: &str,
    schema_version: &str,
    policy_version: &str,
    temperature_milli: u32,
    top_k: u32,
    max_tokens: u32,
    timeout_seconds: u64,
) -> TaskProfile {
    TaskProfile {
        id: use_case.id().to_string(),
        use_case,
        required_capabilities: required.iter().copied().collect(),
        prompt_version: prompt_version.to_string(),
        schema_version: schema_version.to_string(),
        policy_version: policy_version.to_string(),
        sampling_parameters: SamplingParameters {
            temperature: temperature_milli as f32 / 1000.0,
            top_k: Some(top_k),
            top_p: Some(1.0),
            seed: Some(42),
            max_tokens,
        },
        timeout_seconds,
        response_format: if required.contains(&ModelCapabilityKind::StructuredJson) {
            Some(ModelResponseFormat::JsonObject)
        } else {
            None
        },
    }
}

pub fn fingerprint_runtime_definition(runtime: &RuntimeDefinition) -> String {
    let payload = serde_json::to_vec(runtime).unwrap_or_default();
    sha256_hex(&payload)
}

pub fn fingerprint_model_definition(model: &ModelDefinition) -> String {
    let payload = serde_json::json!({
        "id": model.id,
        "family": model.family,
        "modelPath": model.model_path,
        "mmprojPath": model.mmproj_path,
        "format": model.format,
        "quantization": model.quantization,
        "capabilities": model.capabilities,
        "contextLimit": model.context_limit,
    });
    sha256_hex(&serde_json::to_vec(&payload).unwrap_or_default())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn sanitize_id(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_matches('-').to_string()
}

fn infer_quantization(path: &str) -> Option<String> {
    let upper = path.to_ascii_uppercase();
    [
        "Q2_K", "Q3_K", "Q4_K_XL", "Q4_K_M", "Q4_K_S", "Q5_K_M", "Q5_K_S", "Q6_K", "Q8_0",
    ]
    .iter()
    .find(|candidate| upper.contains(**candidate))
    .map(|candidate| candidate.to_string())
}

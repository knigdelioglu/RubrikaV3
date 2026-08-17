use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::SupportFlags;
use crate::domain::model_platform::{
    fingerprint_runtime_definition, FlashAttentionMode, ModelDefinition, ReasoningMode,
    RuntimeDefinition, RuntimeEngine,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLaunchSpec {
    pub engine: RuntimeEngine,
    pub command: String,
    pub args: Vec<String>,
    pub base_url: String,
    pub runtime_fingerprint: String,
    pub requires_mmproj: bool,
}

pub trait InferenceRuntimeAdapter: Send + Sync {
    fn engine(&self) -> RuntimeEngine;
    fn build_launch_spec(
        &self,
        runtime: &RuntimeDefinition,
        model: &ModelDefinition,
        support_flags: &SupportFlags,
    ) -> Result<RuntimeLaunchSpec, AppError>;
}

#[derive(Debug, Clone, Default)]
pub struct LlamaCppRuntimeAdapter;

impl LlamaCppRuntimeAdapter {
    pub fn build_args(
        &self,
        runtime: &RuntimeDefinition,
        model: &ModelDefinition,
        support_flags: &SupportFlags,
    ) -> Result<Vec<String>, AppError> {
        self.validate(runtime, model)?;

        let mut args = vec![
            "-m".to_string(),
            model.model_path.clone(),
            "-ngl".to_string(),
            runtime.gpu_layers.to_string(),
            "-c".to_string(),
            runtime.context_size.to_string(),
            "-fa".to_string(),
            flash_attention_value(runtime.flash_attention).to_string(),
            "--jinja".to_string(),
            "--parallel".to_string(),
            runtime.parallel.to_string(),
            "--batch-size".to_string(),
            runtime.batch_size.to_string(),
            "--ubatch-size".to_string(),
            runtime.ubatch_size.to_string(),
            "--host".to_string(),
            runtime.host.clone(),
            "--port".to_string(),
            runtime.port.to_string(),
        ];

        let requires_mmproj = runtime.uses_multimodal_projector(model);
        if requires_mmproj {
            if !model.capabilities.vision {
                return Err(model_error(
                    AppErrorCode::ModelCapabilityMismatch,
                    "Runtime multimodal projector istiyor ancak model vision capability bildirmiyor.",
                    Some(format!("model_definition_id={}", model.id)),
                    Some("Vision uyumlu bir model seçin veya projector kullanımını kapatın.".to_string()),
                ));
            }
            let mmproj = model
                .mmproj_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| model_error(
                    AppErrorCode::ModelMmprojMissing,
                    "Bu runtime vision kullanımı için multimodal projector gerektiriyor.",
                    Some(format!("model_definition_id={}", model.id)),
                    Some("Model için doğru mmproj dosyasını seçin.".to_string()),
                ))?;
            args.push("--mmproj".to_string());
            args.push(mmproj.to_string());
            if support_flags.mmproj_offload {
                args.push("--mmproj-offload".to_string());
            }
        }

        append_kv_cache_args(&mut args, runtime, support_flags)?;

        if let Some(cache_ram) = runtime.cache_ram_megabytes {
            if support_flags.cache_ram {
                args.push("--cache-ram".to_string());
                args.push(cache_ram.to_string());
            }
        }

        if requires_mmproj {
            if let Some(value) = runtime.image_min_tokens {
                if support_flags.image_min_tokens {
                    args.push("--image-min-tokens".to_string());
                    args.push(value.to_string());
                }
            }
            if let Some(value) = runtime.image_max_tokens {
                if support_flags.image_max_tokens {
                    args.push("--image-max-tokens".to_string());
                    args.push(value.to_string());
                }
            }
        }

        if support_flags.reasoning_off {
            match runtime.reasoning_mode {
                ReasoningMode::Off => args.extend(["--reasoning".to_string(), "off".to_string()]),
                ReasoningMode::On => args.extend(["--reasoning".to_string(), "on".to_string()]),
                ReasoningMode::Auto => {}
            }
        } else if matches!(runtime.reasoning_mode, ReasoningMode::Off | ReasoningMode::On) {
            return Err(model_error(
                AppErrorCode::ModelRuntimeAdapterUnsupported,
                "Seçili llama-server reasoning kontrolünü desteklemiyor.",
                Some("missing --reasoning support".to_string()),
                Some("Güncel llama-server binary kullanın veya reasoning ayarını Auto yapın.".to_string()),
            ));
        }

        args.extend(validate_and_normalize_extra_args(&runtime.extra_args)?);
        Ok(args)
    }

    pub fn validate(
        &self,
        runtime: &RuntimeDefinition,
        model: &ModelDefinition,
    ) -> Result<(), AppError> {
        if runtime.engine != RuntimeEngine::LlamaCpp {
            return Err(model_error(
                AppErrorCode::ModelRuntimeAdapterUnsupported,
                "Runtime adapter bu engine'i desteklemiyor.",
                Some(format!("runtime_engine={:?}", runtime.engine)),
                Some("llama.cpp runtime seçin.".to_string()),
            ));
        }
        if runtime.server_path.trim().is_empty() {
            return Err(model_error(
                AppErrorCode::ModelBinaryMissing,
                "llama-server binary yolu eksik.",
                Some(format!("runtime_definition_id={}", runtime.id)),
                Some("Runtime için llama-server binary dosyasını seçin.".to_string()),
            ));
        }
        if model.model_path.trim().is_empty() {
            return Err(model_error(
                AppErrorCode::ModelFileMissing,
                "Model dosyası yolu eksik.",
                Some(format!("model_definition_id={}", model.id)),
                Some("GGUF model dosyasını seçin.".to_string()),
            ));
        }
        if runtime.privacy_mode == crate::domain::model::PrivacyMode::StrictLocal
            && !is_loopback_host(&runtime.host)
        {
            return Err(model_error(
                AppErrorCode::ModelPrivacyBlocked,
                "Strict Local runtime yalnız loopback host üzerinde çalışabilir.",
                Some(format!("host={}", runtime.host)),
                Some("Host değerini 127.0.0.1, ::1 veya localhost yapın.".to_string()),
            ));
        }
        if runtime.parallel == 0 || runtime.batch_size == 0 || runtime.ubatch_size == 0 {
            return Err(model_error(
                AppErrorCode::ModelRuntimeAdapterUnsupported,
                "Runtime batch/parallel değerleri sıfır olamaz.",
                Some(format!(
                    "parallel={}; batch={}; ubatch={}",
                    runtime.parallel, runtime.batch_size, runtime.ubatch_size
                )),
                Some("Runtime ayarlarını güvenli varsayılanlara döndürün.".to_string()),
            ));
        }
        if runtime.ubatch_size > runtime.batch_size {
            return Err(model_error(
                AppErrorCode::ModelRuntimeAdapterUnsupported,
                "Ubatch değeri batch değerinden büyük olamaz.",
                Some(format!("batch={}; ubatch={}", runtime.batch_size, runtime.ubatch_size)),
                Some("Ubatch değerini batch değerinden küçük veya eşit yapın.".to_string()),
            ));
        }
        validate_and_normalize_extra_args(&runtime.extra_args)?;
        Ok(())
    }
}

impl InferenceRuntimeAdapter for LlamaCppRuntimeAdapter {
    fn engine(&self) -> RuntimeEngine {
        RuntimeEngine::LlamaCpp
    }

    fn build_launch_spec(
        &self,
        runtime: &RuntimeDefinition,
        model: &ModelDefinition,
        support_flags: &SupportFlags,
    ) -> Result<RuntimeLaunchSpec, AppError> {
        let args = self.build_args(runtime, model, support_flags)?;
        Ok(RuntimeLaunchSpec {
            engine: RuntimeEngine::LlamaCpp,
            command: runtime.server_path.clone(),
            args,
            base_url: runtime.base_url(),
            runtime_fingerprint: fingerprint_runtime_definition(runtime),
            requires_mmproj: runtime.uses_multimodal_projector(model),
        })
    }
}

fn append_kv_cache_args(
    args: &mut Vec<String>,
    runtime: &RuntimeDefinition,
    support_flags: &SupportFlags,
) -> Result<(), AppError> {
    if support_flags.cache_short_flags {
        args.extend([
            "-ctk".to_string(), runtime.kv_cache_type_k.clone(),
            "-ctv".to_string(), runtime.kv_cache_type_v.clone(),
        ]);
        return Ok(());
    }
    if support_flags.cache_type_flags {
        args.extend([
            "--cache-type-k".to_string(), runtime.kv_cache_type_k.clone(),
            "--cache-type-v".to_string(), runtime.kv_cache_type_v.clone(),
        ]);
        return Ok(());
    }
    Err(model_error(
        AppErrorCode::ModelRuntimeAdapterUnsupported,
        "Bu llama-server sürümü KV cache bayraklarını desteklemiyor.",
        Some("missing KV cache flags".to_string()),
        Some("KV cache desteği olan güncel llama-server binary kullanın.".to_string()),
    ))
}

fn flash_attention_value(value: FlashAttentionMode) -> &'static str {
    match value {
        FlashAttentionMode::Off => "off",
        FlashAttentionMode::On => "on",
        FlashAttentionMode::Auto => "auto",
    }
}

fn validate_and_normalize_extra_args(extra_args: &[String]) -> Result<Vec<String>, AppError> {
    let allowed_flags: BTreeSet<&'static str> = [
        "--threads", "--threads-batch", "--mlock", "--no-mmap", "--cont-batching",
        "--no-cont-batching", "--prio", "--poll", "--temp", "--top-p", "--top-k",
        "--repeat-penalty", "-n", "--no-cache-prompt", "-cram", "-ctxcp",
    ]
    .into_iter()
    .collect();
    let forbidden_flags: BTreeSet<&'static str> = [
        "-m", "--model", "--mmproj", "--host", "--port", "--api-key", "--api-key-file",
        "--ssl-key-file", "--ssl-cert-file", "--chat-template-file", "--rpc",
    ]
    .into_iter()
    .collect();

    for token in extra_args {
        if token.contains('\n') || token.contains('\r') || token.contains('\0') {
            return Err(unsafe_extra_arg(token));
        }
        if token.starts_with('-')
            && (forbidden_flags.contains(token.as_str()) || !allowed_flags.contains(token.as_str()))
        {
            return Err(unsafe_extra_arg(token));
        }
    }
    Ok(extra_args.to_vec())
}

fn unsafe_extra_arg(token: &str) -> AppError {
    model_error(
        AppErrorCode::ModelRuntimeAdapterUnsupported,
        "Runtime extra arg güvenlik politikası tarafından reddedildi.",
        Some(format!("rejected_extra_arg={token}")),
        Some("Yalnız desteklenen gelişmiş runtime seçeneklerini kullanın.".to_string()),
    )
}

fn is_loopback_host(host: &str) -> bool {
    let normalized = host
        .trim()
        .trim_matches(|character| character == '[' || character == ']')
        .to_ascii_lowercase();
    matches!(normalized.as_str(), "127.0.0.1" | "::1" | "localhost")
}

fn model_error(
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

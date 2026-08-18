use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::SupportFlags;
use crate::domain::model_platform::{
    fingerprint_runtime_definition, CapabilityManifest, CapabilityProbeResult, CapabilitySupport,
    ModelCapabilityKind, RuntimeEngine,
};
use crate::services::llama_cpp_runtime_adapter::{InferenceRuntimeAdapter, LlamaCppRuntimeAdapter};
use crate::services::model_platform_service::ModelPlatformService;
use chrono::Utc;
use reqwest::redirect::Policy;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

#[derive(Clone)]
pub struct ModelCapabilityProbeService {
    platform: ModelPlatformService,
    client: reqwest::Client,
}

impl ModelCapabilityProbeService {
    pub fn new(platform: ModelPlatformService) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|error| {
                probe_error(
                    AppErrorCode::ModelHealthFailed,
                    "Capability probe HTTP istemcisi oluşturulamadı.",
                    Some(error.to_string()),
                    None,
                )
            })?;
        Ok(Self { platform, client })
    }

    pub async fn probe(
        &self,
        model_definition_id: &str,
        runtime_definition_id: &str,
    ) -> Result<CapabilityManifest, AppError> {
        self.platform.mark_probe_started(model_definition_id)?;
        match self
            .probe_inner(model_definition_id, runtime_definition_id)
            .await
        {
            Ok(manifest) => {
                self.platform.record_capability_manifest(manifest.clone())?;
                self.platform
                    .mark_probe_finished(model_definition_id, &manifest)?;
                Ok(manifest)
            }
            Err(error) => {
                let _ = self.platform.mark_probe_failed(model_definition_id);
                Err(error)
            }
        }
    }

    async fn probe_inner(
        &self,
        model_definition_id: &str,
        runtime_definition_id: &str,
    ) -> Result<CapabilityManifest, AppError> {
        let snapshot = self.platform.snapshot()?;
        let model = snapshot
            .models
            .iter()
            .find(|item| item.id == model_definition_id)
            .cloned()
            .ok_or_else(|| {
                probe_error(
                    AppErrorCode::ModelProfileNotFound,
                    "Capability probe için model bulunamadı.",
                    Some(format!("model_definition_id={model_definition_id}")),
                    Some("Modeli registry'ye yeniden ekleyin.".to_string()),
                )
            })?;
        let runtime = snapshot
            .runtimes
            .iter()
            .find(|item| item.id == runtime_definition_id)
            .cloned()
            .ok_or_else(|| {
                probe_error(
                    AppErrorCode::ModelProfileNotFound,
                    "Capability probe için runtime bulunamadı.",
                    Some(format!("runtime_definition_id={runtime_definition_id}")),
                    Some("Runtime ayarını yeniden oluşturun.".to_string()),
                )
            })?;

        if runtime.engine != RuntimeEngine::LlamaCpp {
            return Err(probe_error(
                AppErrorCode::ModelServerUnsupportedFlags,
                "Bu sürümde capability probe yalnız llama.cpp runtime için uygulanıyor.",
                Some(format!("engine={:?}", runtime.engine)),
                Some("llama.cpp runtime seçin.".to_string()),
            ));
        }

        let help_output = run_help(&runtime.server_path).await?;
        let support_flags = SupportFlags::from_help_output(&help_output);
        let adapter = LlamaCppRuntimeAdapter;
        let launch = adapter.build_launch_spec(&runtime, &model, &support_flags)?;

        let mut owned_child: Option<Child> = None;
        if !self.health_ok(&launch.base_url).await {
            if !runtime.managed {
                return Err(probe_error(
                    AppErrorCode::ModelServerNotRunning,
                    "Harici runtime erişilebilir değil.",
                    Some(format!("base_url={}", launch.base_url)),
                    Some("Model sunucusunu başlatın ve probe'u yeniden çalıştırın.".to_string()),
                ));
            }
            let mut command = Command::new(&launch.command);
            command
                .args(&launch.args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            #[cfg(unix)]
            command.process_group(0);
            let child = command.spawn().map_err(|error| {
                probe_error(
                    AppErrorCode::ModelServerStartFailed,
                    "Capability probe için llama-server başlatılamadı.",
                    Some(error.to_string()),
                    Some("Runtime binary/model yollarını kontrol edin.".to_string()),
                )
            })?;
            owned_child = Some(child);
            if let Err(error) = self.wait_for_health(&launch.base_url).await {
                stop_owned_child(&mut owned_child).await;
                return Err(error);
            }
        }

        let mut results = Vec::new();
        let text = self.text_probe(&launch.base_url).await;
        results.push(text.clone());

        if text.support == CapabilitySupport::Pass {
            results.push(self.structured_json_probe(&launch.base_url).await);
            results.push(self.json_schema_probe(&launch.base_url).await);
        } else {
            results.push(skipped(
                ModelCapabilityKind::StructuredJson,
                "text completion probe başarısız olduğu için çalıştırılmadı",
            ));
            results.push(skipped(
                ModelCapabilityKind::JsonSchema,
                "text completion probe başarısız olduğu için çalıştırılmadı",
            ));
        }

        if model.capabilities.vision {
            results.push(self.vision_probe(&launch.base_url).await);
            if model.capabilities.multimodal_projector_required {
                results.push(CapabilityProbeResult {
                    capability: ModelCapabilityKind::MultimodalProjector,
                    support: if model
                        .mmproj_path
                        .as_deref()
                        .map(|path| !path.trim().is_empty())
                        .unwrap_or(false)
                    {
                        CapabilitySupport::Pass
                    } else {
                        CapabilitySupport::Fail
                    },
                    detail: Some("vision model projector requirement".to_string()),
                    duration_ms: None,
                });
            }
        } else {
            results.push(CapabilityProbeResult {
                capability: ModelCapabilityKind::Vision,
                support: CapabilitySupport::Fail,
                detail: Some("model definition vision=false".to_string()),
                duration_ms: None,
            });
        }

        results.push(CapabilityProbeResult {
            capability: ModelCapabilityKind::ThinkingControl,
            support: if support_flags.reasoning_off {
                CapabilitySupport::Pass
            } else if model.capabilities.thinking_control {
                CapabilitySupport::Partial
            } else {
                CapabilitySupport::Unverified
            },
            detail: Some(if support_flags.reasoning_off {
                "llama-server --reasoning control available".to_string()
            } else {
                "llama-server reasoning flag not detected".to_string()
            }),
            duration_ms: None,
        });

        stop_owned_child(&mut owned_child).await;

        Ok(CapabilityManifest {
            model_definition_id: model.id,
            runtime_definition_id: runtime.id,
            model_fingerprint: model.model_fingerprint,
            runtime_fingerprint: fingerprint_runtime_definition(&runtime),
            verified_at: Utc::now().to_rfc3339(),
            results,
        })
    }

    async fn health_ok(&self, base_url: &str) -> bool {
        let url = format!("{}/health", base_url.trim_end_matches('/'));
        match self.client.get(url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn wait_for_health(&self, base_url: &str) -> Result<(), AppError> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(180) {
            if self.health_ok(base_url).await {
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }
        Err(probe_error(
            AppErrorCode::ModelServerReadyTimeout,
            "Capability probe runtime readiness süresini aştı.",
            Some(format!("base_url={base_url}")),
            Some("Model/runtime uyumluluğunu ve llama-server loglarını kontrol edin.".to_string()),
        ))
    }

    async fn text_probe(&self, base_url: &str) -> CapabilityProbeResult {
        let started = Instant::now();
        let body = json!({
            "messages": [
                {"role": "system", "content": "You are a local compatibility probe. Follow the user instruction exactly."},
                {"role": "user", "content": "Reply with exactly: RUBRIKA_PROBE_OK"}
            ],
            "temperature": 0,
            "top_k": 1,
            "max_tokens": 32,
            "stream": false
        });
        match self.chat_completion(base_url, body).await {
            Ok(content) => CapabilityProbeResult {
                capability: ModelCapabilityKind::Text,
                support: if content.contains("RUBRIKA_PROBE_OK") {
                    CapabilitySupport::Pass
                } else {
                    CapabilitySupport::Partial
                },
                detail: Some("text completion probe".to_string()),
                duration_ms: Some(started.elapsed().as_millis() as u64),
            },
            Err(error) => failed(ModelCapabilityKind::Text, error, started),
        }
    }

    async fn structured_json_probe(&self, base_url: &str) -> CapabilityProbeResult {
        let started = Instant::now();
        let body = json!({
            "messages": [
                {"role": "system", "content": "Return only valid JSON."},
                {"role": "user", "content": "Return an object with ok=true and value=7."}
            ],
            "temperature": 0,
            "top_k": 1,
            "max_tokens": 64,
            "stream": false,
            "response_format": {"type": "json_object"}
        });
        match self.chat_completion(base_url, body).await {
            Ok(content) => {
                let parsed = parse_json_content(&content);
                CapabilityProbeResult {
                    capability: ModelCapabilityKind::StructuredJson,
                    support: match parsed {
                        Some(value)
                            if value.get("ok").and_then(Value::as_bool) == Some(true)
                                && value.get("value").and_then(Value::as_i64) == Some(7) =>
                        {
                            CapabilitySupport::Pass
                        }
                        Some(_) => CapabilitySupport::Partial,
                        None => CapabilitySupport::Fail,
                    },
                    detail: Some("response_format=json_object".to_string()),
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                }
            }
            Err(error) => failed(ModelCapabilityKind::StructuredJson, error, started),
        }
    }

    async fn json_schema_probe(&self, base_url: &str) -> CapabilityProbeResult {
        let started = Instant::now();
        let body = json!({
            "messages": [
                {"role": "system", "content": "Return only data matching the provided schema."},
                {"role": "user", "content": "Return ok=true and label=rubrika."}
            ],
            "temperature": 0,
            "top_k": 1,
            "max_tokens": 64,
            "stream": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "rubrika_capability_probe",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "ok": {"type": "boolean", "const": true},
                            "label": {"type": "string", "const": "rubrika"}
                        },
                        "required": ["ok", "label"],
                        "additionalProperties": false
                    }
                }
            }
        });
        match self.chat_completion(base_url, body).await {
            Ok(content) => {
                let parsed = parse_json_content(&content);
                CapabilityProbeResult {
                    capability: ModelCapabilityKind::JsonSchema,
                    support: match parsed {
                        Some(value)
                            if value.get("ok").and_then(Value::as_bool) == Some(true)
                                && value.get("label").and_then(Value::as_str)
                                    == Some("rubrika") =>
                        {
                            CapabilitySupport::Pass
                        }
                        Some(_) => CapabilitySupport::Partial,
                        None => CapabilitySupport::Fail,
                    },
                    detail: Some("response_format=json_schema".to_string()),
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                }
            }
            Err(_) => CapabilityProbeResult {
                capability: ModelCapabilityKind::JsonSchema,
                support: CapabilitySupport::Partial,
                detail: Some("runtime json_schema probe rejected or unsupported".to_string()),
                duration_ms: Some(started.elapsed().as_millis() as u64),
            },
        }
    }

    async fn vision_probe(&self, base_url: &str) -> CapabilityProbeResult {
        let started = Instant::now();
        // 1x1 transparent PNG; synthetic probe only, never student data.
        const PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "This is a synthetic capability probe. Return JSON {\"vision\":true}."},
                    {"type": "image_url", "image_url": {"url": PNG_DATA_URL}}
                ]
            }],
            "temperature": 0,
            "top_k": 1,
            "max_tokens": 64,
            "stream": false,
            "response_format": {"type": "json_object"}
        });
        match self.chat_completion(base_url, body).await {
            Ok(content) => {
                let parsed = parse_json_content(&content);
                CapabilityProbeResult {
                    capability: ModelCapabilityKind::Vision,
                    support: if parsed
                        .as_ref()
                        .and_then(|value| value.get("vision"))
                        .and_then(Value::as_bool)
                        == Some(true)
                    {
                        CapabilitySupport::Pass
                    } else {
                        CapabilitySupport::Partial
                    },
                    detail: Some("synthetic 1x1 image probe".to_string()),
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                }
            }
            Err(error) => failed(ModelCapabilityKind::Vision, error, started),
        }
    }

    async fn chat_completion(&self, base_url: &str, body: Value) -> Result<String, AppError> {
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let response = timeout(
            Duration::from_secs(45),
            self.client.post(url).json(&body).send(),
        )
        .await
        .map_err(|_| {
            probe_error(
                AppErrorCode::ModelTimeout,
                "Capability probe model yanıt süresini aştı.",
                None,
                Some("Model/runtime ayarlarını kontrol edip yeniden deneyin.".to_string()),
            )
        })?
        .map_err(|error| {
            probe_error(
                AppErrorCode::ModelHealthFailed,
                "Capability probe model isteği başarısız oldu.",
                Some(error.to_string()),
                Some("Model sunucusu durumunu kontrol edin.".to_string()),
            )
        })?;
        let status = response.status();
        let payload: Value = response.json().await.map_err(|error| {
            probe_error(
                AppErrorCode::ModelResponseInvalidJson,
                "Capability probe yanıtı JSON değil.",
                Some(error.to_string()),
                None,
            )
        })?;
        if !status.is_success() {
            return Err(probe_error(
                AppErrorCode::ModelHealthFailed,
                "Capability probe model sunucusu isteği reddetti.",
                Some(format!("http_status={}", status.as_u16())),
                Some("Runtime/model uyumluluğunu kontrol edin.".to_string()),
            ));
        }
        payload
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                probe_error(
                    AppErrorCode::ModelResponseEmpty,
                    "Capability probe model yanıtı boş.",
                    None,
                    None,
                )
            })
    }
}

async fn run_help(server_path: &str) -> Result<String, AppError> {
    if server_path.trim().is_empty() {
        return Err(probe_error(
            AppErrorCode::ModelServerPathMissing,
            "Capability probe için llama-server yolu eksik.",
            None,
            Some("Runtime binary yolunu seçin.".to_string()),
        ));
    }
    let output = timeout(
        Duration::from_secs(10),
        Command::new(server_path).arg("--help").output(),
    )
    .await
    .map_err(|_| {
        probe_error(
            AppErrorCode::ModelTimeout,
            "llama-server --help zaman aşımına uğradı.",
            None,
            None,
        )
    })?
    .map_err(|error| {
        probe_error(
            AppErrorCode::ModelServerStartFailed,
            "llama-server capability bilgisi okunamadı.",
            Some(error.to_string()),
            Some("Runtime binary yolunu ve çalıştırma iznini kontrol edin.".to_string()),
        )
    })?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}

async fn stop_owned_child(child: &mut Option<Child>) {
    if let Some(child) = child.as_mut() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    *child = None;
}

fn parse_json_content(content: &str) -> Option<Value> {
    serde_json::from_str(content.trim()).ok().or_else(|| {
        let start = content.find('{')?;
        let end = content.rfind('}')?;
        serde_json::from_str(&content[start..=end]).ok()
    })
}

fn failed(
    capability: ModelCapabilityKind,
    error: AppError,
    started: Instant,
) -> CapabilityProbeResult {
    CapabilityProbeResult {
        capability,
        support: CapabilitySupport::Fail,
        detail: Some(error.message),
        duration_ms: Some(started.elapsed().as_millis() as u64),
    }
}

fn skipped(capability: ModelCapabilityKind, detail: &str) -> CapabilityProbeResult {
    CapabilityProbeResult {
        capability,
        support: CapabilitySupport::Unverified,
        detail: Some(detail.to_string()),
        duration_ms: None,
    }
}

fn probe_error(
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

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::timeout;
use url::Url;
use uuid::Uuid;

use crate::domain::analysis::AnalysisModelOutput;
use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{
    AnalysisReportRequest, AnalysisReportResult, ExtractedQuestionCandidate,
    ExtractedRubricCandidate, ModelDiagnostics, ModelProvenance, ModelRequestKind,
    ModelRequestPayloadSummary, ModelResponseFormat, ModelStatus, PrivacyMode, PromptContract,
    QuestionTextExtractionOutput, QuestionTextExtractionRequest, QuestionTextExtractionResult,
    RubricExtractionOutput, RubricExtractionRequest, RubricExtractionResult, ScoringCriterionScore,
    ScoringOutput, ScoringRequest, ScoringResult, SemanticCriterionDecision,
    SpeakingTranscriptCleanupOutputSegment, SpeakingTranscriptCleanupRequest,
    SpeakingTranscriptCleanupResult, StudentAnswerOcrIssueCorrectionDecision,
    StudentAnswerOcrIssueCorrectionOutput, StudentAnswerOcrIssueCorrectionRequest,
    StudentAnswerOcrIssueCorrectionResult, StudentAnswerOcrIssueCorrectionScope,
    StudentAnswerOcrRequest, StudentAnswerOcrResult, StudentIdentityOcrOutput,
    StudentIdentityOcrRequest, StudentIdentityOcrResult,
};
use crate::domain::question::AnswerType;
use crate::domain::student::{
    default_ocr_review_policy, OcrCriticalTermWarning, OcrImagePreprocessMode,
    OcrSuggestedCorrection, OcrUncertainSpan,
};
use crate::platform::project_paths::TrustedProjectRoot;
use crate::services::model_gateway::ModelGateway;
use crate::services::prompt_contract::{
    invocation_metadata, legacy_prompt_contract_with_data, response_format_value, user_data_message,
};

const QUESTION_TEXT_MAX_TOKENS: u32 = 4096;
const RUBRIC_MAX_TOKENS: u32 = 8192;
const STUDENT_ANSWER_OCR_MAX_TOKENS: u32 = 4096;
const STUDENT_ANSWER_OCR_ISSUE_CORRECTION_MAX_TOKENS: u32 = 512;
const STUDENT_IDENTITY_OCR_MAX_TOKENS: u32 = 1024;
const CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING: &str = "critical_keyword_ocr_uncertain";

/// Default upper bound for a single model HTTP response body (bytes).
///
/// The body is read with a streaming, bounded reader; a larger body is
/// rejected with `ModelResponseTooLarge` and never parsed or committed.
pub const DEFAULT_MAX_RESPONSE_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// Default upper bound for a single model HTTP request body (bytes).
///
/// A request that exceeds this limit is rejected before it is sent.
pub const DEFAULT_MAX_REQUEST_BODY_BYTES: u64 = 128 * 1024 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(30);
const IDLE_CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

fn request_contract(
    provided: Option<PromptContract>,
    use_case: ModelRequestKind,
    prompt: &str,
    user_data: serde_json::Value,
    max_tokens: u32,
    response_format: Option<ModelResponseFormat>,
) -> PromptContract {
    provided.unwrap_or_else(|| {
        legacy_prompt_contract_with_data(
            use_case.clone(),
            prompt,
            user_data,
            max_tokens,
            response_format,
        )
    })
}

/// Bounded transport limits for model gateway HTTP calls.
#[derive(Debug, Clone, Copy)]
pub struct GatewayLimits {
    pub max_response_body_bytes: u64,
    pub max_request_body_bytes: u64,
    pub connect_timeout: Duration,
    pub first_byte_timeout: Duration,
    pub idle_chunk_timeout: Duration,
}

impl Default for GatewayLimits {
    fn default() -> Self {
        Self {
            max_response_body_bytes: DEFAULT_MAX_RESPONSE_BODY_BYTES,
            max_request_body_bytes: DEFAULT_MAX_REQUEST_BODY_BYTES,
            connect_timeout: CONNECT_TIMEOUT,
            first_byte_timeout: FIRST_BYTE_TIMEOUT,
            idle_chunk_timeout: IDLE_CHUNK_TIMEOUT,
        }
    }
}

#[derive(Clone)]
pub struct LlamaServerGateway {
    client: Client,
    base_url: String,
    limits: GatewayLimits,
    privacy_mode: Arc<RwLock<PrivacyMode>>,
}

impl LlamaServerGateway {
    pub fn new(base_url: String) -> Self {
        Self::new_with_limits_and_privacy(
            base_url,
            GatewayLimits::default(),
            PrivacyMode::StrictLocal,
        )
    }

    pub fn new_with_limits(base_url: String, limits: GatewayLimits) -> Self {
        Self::new_with_limits_and_privacy(base_url, limits, PrivacyMode::StrictLocal)
    }

    pub fn new_with_privacy(base_url: String, privacy_mode: PrivacyMode) -> Self {
        Self::new_with_limits_and_privacy(base_url, GatewayLimits::default(), privacy_mode)
    }

    fn new_with_limits_and_privacy(
        base_url: String,
        limits: GatewayLimits,
        privacy_mode: PrivacyMode,
    ) -> Self {
        let client = Client::builder()
            .connect_timeout(limits.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .unwrap_or_else(|_| {
                Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .unwrap_or_else(|_| Client::new())
            });
        Self {
            client,
            base_url,
            limits,
            privacy_mode: Arc::new(RwLock::new(privacy_mode)),
        }
    }

    pub fn limits(&self) -> GatewayLimits {
        self.limits
    }

    pub fn configure_privacy(&self, privacy_mode: PrivacyMode) -> Result<(), AppError> {
        let mut current = self.privacy_mode.write().map_err(|error| {
            app_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model gizlilik ayarı okunamadı.",
                Some(format!("privacy mode lock failed: {error}")),
                Some("Uygulamayı yeniden başlatıp tekrar deneyin.".to_string()),
            )
        })?;
        *current = privacy_mode;
        Ok(())
    }

    fn privacy_mode(&self) -> Result<PrivacyMode, AppError> {
        self.privacy_mode.read().map(|mode| *mode).map_err(|error| {
            app_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model gizlilik ayarı okunamadı.",
                Some(format!("privacy mode lock failed: {error}")),
                Some("Uygulamayı yeniden başlatıp tekrar deneyin.".to_string()),
            )
        })
    }

    fn client_for_url(&self, base_url: &str) -> Result<Client, AppError> {
        let privacy_mode = self.privacy_mode()?;
        if privacy_mode == PrivacyMode::ExplicitExternal {
            return Ok(self.client.clone());
        }
        let parsed = Url::parse(base_url).map_err(|error| {
            app_error(
                AppErrorCode::ModelPrivacyBlocked,
                "Model adresi güvenli bir URL değil.",
                Some(format!("model URL parse failed while pinning DNS: {error}")),
                Some("Model adresini güvenli bir yerel adresle güncelleyin.".to_string()),
            )
        })?;
        let host = parsed.host_str().unwrap_or_default();
        if host.parse::<IpAddr>().is_ok() {
            return Ok(self.client.clone());
        }
        let normalized_host = host.trim_start_matches('[').trim_end_matches(']');
        let port = parsed.port_or_known_default().unwrap_or(80);
        let addresses: Vec<SocketAddr> = (normalized_host, port)
            .to_socket_addrs()
            .map_err(|error| {
                app_error(
                    AppErrorCode::ModelPrivacyBlocked,
                    "Strict local model adresi çözümlenemedi.",
                    Some(format!("localhost resolution failed: {error}")),
                    Some("127.0.0.1 veya [::1] model adresini seçin.".to_string()),
                )
            })?
            .filter(|address| address.ip().is_loopback())
            .collect();
        if addresses.is_empty() {
            return Err(app_error(
                AppErrorCode::ModelPrivacyBlocked,
                "Strict local model adresi loopback olarak doğrulanamadı.",
                Some("localhost resolution produced no loopback address".to_string()),
                Some("127.0.0.1 veya [::1] model adresini seçin.".to_string()),
            ));
        }
        Client::builder()
            .connect_timeout(self.limits.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .resolve_to_addrs(normalized_host, &addresses)
            .build()
            .map_err(|error| {
                app_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model bağlantısı hazırlanamadı.",
                    Some(format!("pinned localhost client build failed: {error}")),
                    Some("Uygulamayı yeniden başlatıp tekrar deneyin.".to_string()),
                )
            })
    }

    fn health_url(&self, base_url: &str) -> String {
        format!("{}/health", base_url.trim_end_matches('/'))
    }

    fn chat_url(&self, base_url: &str) -> String {
        format!("{}/v1/chat/completions", base_url.trim_end_matches('/'))
    }

    async fn send_chat_request(
        &self,
        base_url: &str,
        body: serde_json::Value,
        timeout_seconds: u64,
        request_kind: &str,
    ) -> Result<(u16, String, u64), AppError> {
        let url = self.chat_url(base_url);
        let body_bytes = serde_json::to_vec(&body).map_err(|error| {
            app_error(
                AppErrorCode::ModelRequestTooLarge,
                "Model isteği hazırlanamadı.",
                Some(format!("request serialization failed: {error}")),
                Some("İsteği küçültüp yeniden deneyin.".to_string()),
            )
        })?;
        if (body_bytes.len() as u64) > self.limits.max_request_body_bytes {
            return Err(app_error(
                AppErrorCode::ModelRequestTooLarge,
                "Model isteği çok büyük.",
                Some(format!(
                    "request_bytes={} limit_bytes={}",
                    body_bytes.len(),
                    self.limits.max_request_body_bytes
                )),
                Some("Daha az sayıda belge veya görselle yeniden deneyin.".to_string()),
            ));
        }
        // Validate immediately before the network operation. Local request
        // validation (including size limits) remains deterministic even when
        // a legacy profile has an invalid external endpoint.
        validate_base_url_for_privacy(base_url, self.privacy_mode()?)?;
        let client = self.client_for_url(base_url)?;

        let start = std::time::Instant::now();
        let response = timeout(
            std::time::Duration::from_secs(timeout_seconds),
            client.post(&url).body(body_bytes).send(),
        )
        .await
        .map_err(|_| {
            app_error(
                AppErrorCode::ModelTimeout,
                "Model isteği zaman aşımına uğradı.",
                Some(format!("Endpoint: {}\nTimeout: {}s\nRequest Kind: {}", url.clone(), timeout_seconds, request_kind)),
                Some("Model server çalışıyor ancak zamanında yanıt vermedi. Tekrar deneyebilir veya model loglarını kontrol edebilirsiniz.".to_string()),
            )
        })?
        .map_err(|error| {
            if error.is_redirect() {
                app_error(
                    AppErrorCode::ModelRedirectRejected,
                    "Model sunucusu güvenli olmayan bir yönlendirme yaptı.",
                    Some(format!("redirect rejected for request_kind={request_kind}")),
                    Some("Model adresini ve sunucu yönlendirme ayarlarını kontrol edin.".to_string()),
                )
            } else {
                map_transport_error(error, &url)
            }
        })?;

        let status = response.status().as_u16();

        if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
            let value = content_type
                .to_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let safe = value.contains("json")
                || value.contains("text/plain")
                || value.contains("application/octet-stream")
                || value.contains("*/*")
                || value.is_empty();
            if !safe {
                return Err(app_error(
                    AppErrorCode::ModelResponseInvalidContentType,
                    "Model yanıtı beklenen biçimde değil.",
                    Some(format!("status={status} content_type={value}")),
                    Some("Model sürümünü kontrol edip yeniden deneyin.".to_string()),
                ));
            }
        }

        let body_text = read_bounded_body(
            response,
            self.limits.max_response_body_bytes,
            self.limits.first_byte_timeout,
            self.limits.idle_chunk_timeout,
            &url,
        )
        .await?;

        Ok((status, body_text, start.elapsed().as_millis() as u64))
    }
}

/// Validates a model endpoint before any network operation. Strict local mode
/// accepts only literal loopback addresses or `localhost`, and every resolved
/// localhost address must itself be loopback. Redirects are disabled on the
/// client, so a public/DNS-rebinding hop cannot silently widen this boundary.
pub fn validate_base_url_for_privacy(
    base_url: &str,
    privacy_mode: PrivacyMode,
) -> Result<(), AppError> {
    let parsed = Url::parse(base_url).map_err(|error| {
        app_error(
            AppErrorCode::ModelPrivacyBlocked,
            "Model adresi güvenli bir URL değil.",
            Some(format!("model URL parse failed: {error}")),
            Some("Model adresini http://127.0.0.1:8080 biçiminde kontrol edin.".to_string()),
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(app_error(
            AppErrorCode::ModelPrivacyBlocked,
            "Model adresi güvenli bir URL değil.",
            Some(
                "model URL must use http/https without credentials, query, or fragment".to_string(),
            ),
            Some("Model adresini güvenli bir yerel adresle güncelleyin.".to_string()),
        ));
    }
    if privacy_mode == PrivacyMode::ExplicitExternal {
        return Ok(());
    }

    let host = parsed.host_str().unwrap_or_default();
    let normalized_host = host.trim_start_matches('[').trim_end_matches(']');
    let port = parsed.port_or_known_default().unwrap_or(80);
    let loopback = if let Ok(address) = normalized_host.parse::<IpAddr>() {
        address.is_loopback()
    } else if normalized_host.eq_ignore_ascii_case("localhost") {
        match (normalized_host, port).to_socket_addrs() {
            Ok(addresses) => {
                let addresses: Vec<_> = addresses.collect();
                !addresses.is_empty() && addresses.iter().all(|address| address.ip().is_loopback())
            }
            Err(_) => false,
        }
    } else {
        false
    };
    if !loopback {
        return Err(app_error(
            AppErrorCode::ModelPrivacyBlocked,
            "Strict local gizlilik politikası yalnızca loopback model sunucusuna izin veriyor.",
            Some("non_loopback_model_endpoint_rejected".to_string()),
            Some("127.0.0.1 veya [::1] üzerindeki yönetilen model profilini seçin.".to_string()),
        ));
    }
    Ok(())
}

fn build_scoring_request_body(contract: &PromptContract) -> serde_json::Value {
    json!({
        "model": "gemma",
        "messages": [
            { "role": "system", "content": contract.system_policy.clone() },
            { "role": "user", "content": user_data_message(contract) }
        ],
        "chat_template_kwargs": { "enable_thinking": false },
        "temperature": contract.invocation.sampling_parameters.temperature,
        "top_k": contract.invocation.sampling_parameters.top_k,
        "top_p": contract.invocation.sampling_parameters.top_p,
        "seed": contract.invocation.sampling_parameters.seed,
        "max_tokens": contract.invocation.sampling_parameters.max_tokens,
        "response_format": response_format_value(contract),
        "stream": false
    })
}

impl Default for LlamaServerGateway {
    fn default() -> Self {
        Self::new("http://127.0.0.1:8080".to_string())
    }
}

#[async_trait]
impl ModelGateway for LlamaServerGateway {
    async fn get_status(&self) -> Result<ModelStatus, AppError> {
        self.health_status(&self.base_url).await
    }

    async fn probe_server(&self) -> Result<ModelStatus, AppError> {
        self.probe_status(&self.base_url).await
    }

    async fn health_status(&self, base_url: &str) -> Result<ModelStatus, AppError> {
        validate_base_url_for_privacy(base_url, self.privacy_mode()?)?;
        let client = self.client_for_url(base_url)?;
        let mut status = ModelStatus {
            base_url: base_url.to_string(),
            ..Default::default()
        };

        match timeout(
            std::time::Duration::from_secs(5),
            client.get(self.health_url(base_url)).send(),
        )
        .await
        {
            Ok(Ok(response)) => {
                status.server_running = true;
                status.health_ok = response.status().is_success();
                if status.health_ok {
                    status.health_verified_at = Some(Utc::now());
                }
                if !status.health_ok {
                    status.last_error = Some(app_error(
                        AppErrorCode::ModelHealthFailed,
                        "Health endpoint başarılı dönmedi.",
                        Some(format!("HTTP {}", response.status())),
                        Some("Health endpoint çıktısını kontrol edin.".to_string()),
                    ));
                }
            }
            Ok(Err(error)) => {
                status.last_error = Some(if error.is_redirect() {
                    app_error(
                        AppErrorCode::ModelRedirectRejected,
                        "Model sunucusu güvenli olmayan bir yönlendirme yaptı.",
                        Some("redirect rejected for health request".to_string()),
                        Some(
                            "Model adresini ve sunucu yönlendirme ayarlarını kontrol edin."
                                .to_string(),
                        ),
                    )
                } else {
                    map_transport_error(error, &self.health_url(base_url))
                });
            }
            Err(_) => {
                status.last_error = Some(app_error(
                    AppErrorCode::ModelTimeout,
                    "Health check zaman aşımına uğradı.",
                    Some(self.health_url(base_url)),
                    Some("llama-server sürecini kontrol edin.".to_string()),
                ));
            }
        }

        Ok(status)
    }

    async fn probe_status(&self, base_url: &str) -> Result<ModelStatus, AppError> {
        let mut status = self.health_status(base_url).await?;
        if !status.server_running || !status.health_ok {
            return Ok(status);
        }

        let probe_body = json!({
            "model": "probe",
            "messages": [
                {"role": "user", "content": "Reply with exactly one word: ok"}
            ],
            "temperature": 0.0,
            "stream": false
        });

        match self
            .send_chat_request(base_url, probe_body, 30, "HealthProbe")
            .await
        {
            Ok((http_status, body_text, _duration_ms)) => {
                if (200..300).contains(&http_status) {
                    match extract_assistant_content(&body_text) {
                        Ok(content) => {
                            status.completion_probe_ok = !content.trim().is_empty();
                            if status.completion_probe_ok {
                                status.completion_probe_verified_at = Some(Utc::now());
                            }
                            if !status.completion_probe_ok {
                                status.last_error = Some(app_error(
                                    AppErrorCode::ModelResponseEmpty,
                                    "Completion probe boş yanıt verdi.",
                                    Some(body_text),
                                    Some("Model probe yanıtını kontrol edin.".to_string()),
                                ));
                            }
                        }
                        Err(error) => {
                            status.last_error = Some(error);
                        }
                    }
                } else {
                    status.last_error = Some(app_error(
                        AppErrorCode::ModelHealthFailed,
                        "Completion probe başarılı HTTP yanıtı vermedi.",
                        Some(format!("HTTP {}", http_status)),
                        Some("Model sunucusunun chat endpointini kontrol edin.".to_string()),
                    ));
                }
            }
            Err(error) => {
                status.last_error = Some(error);
            }
        }

        Ok(status)
    }

    async fn extract_question_text(
        &self,
        input: QuestionTextExtractionRequest,
    ) -> Result<QuestionTextExtractionResult, AppError> {
        let status = self.get_status().await?;
        if !status.server_running {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Gemma model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Model sunucusunu başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let images = request_images(
            &input.model_input_images,
            Some(&input.image_path),
            "İşlenmiş PDF sayfası okunamadı.",
        )?;

        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::QuestionText,
            &input.prompt,
            json!({
                "targetQuestionNumber": input.target_question_number,
                "pageIndex": input.page_index,
                "pageCount": input.page_count,
            }),
            QUESTION_TEXT_MAX_TOKENS,
            Some(ModelResponseFormat::JsonObject),
        );

        let request_body = json!({
            "model": "gemma",
            "messages": [
                {
                    "role": "system",
                    "content": contract.system_policy.clone()
                },
                {
                    "role": "user",
                    "content": build_image_content(user_data_message(&contract), &images)
                }
            ],
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });

        let (http_status, response_body, duration_ms) = self
            .send_chat_request(&self.base_url, request_body, 600, "QuestionTextExtraction")
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusu başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Model sunucusunun loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let finish_reason = extract_finish_reason(&response_body);
        let reasoning_length = extract_reasoning_length(&response_body);
        let cleaned = strip_reasoning_and_fences(&assistant_content);
        if cleaned.trim().is_empty() {
            return Err(app_error(
                if assistant_content.contains("<think>") {
                    AppErrorCode::ModelResponseReasoningOnly
                } else {
                    AppErrorCode::ModelResponseEmpty
                },
                "Model final cevap içeriği üretmedi.",
                Some(assistant_content),
                Some("Promptu veya model çıktısını kontrol edin.".to_string()),
            ));
        }

        let payload_summary = build_payload_summary(
            contract.system_policy.len() as u32,
            600,
            Some(&input.model_input_images),
            images.first().map(|image| image.bytes_len),
            Some(QUESTION_TEXT_MAX_TOKENS),
        );
        let output = parse_question_text_output(&cleaned)?;
        Ok(QuestionTextExtractionResult {
            page_index: input.page_index,
            page_count: input.page_count,
            output,
            raw_response: assistant_content.clone(),
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::QuestionText,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(payload_summary.prompt_length),
                image_count: Some(payload_summary.image_count),
                image_total_bytes: Some(payload_summary.image_total_bytes),
                base64_approx_total_bytes: Some(payload_summary.base64_approx_total_bytes),
                model_input_images: payload_summary.model_input_images,
                timeout_seconds: Some(payload_summary.timeout_seconds),
                max_tokens: payload_summary.max_tokens,
                finish_reason: finish_reason.clone(),
                content_length: Some(assistant_content.len() as u32),
                reasoning_content_length: reasoning_length,
                raw_text_stored_path: None,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
        })
    }

    async fn draft_rubric(
        &self,
        input: RubricExtractionRequest,
    ) -> Result<RubricExtractionResult, AppError> {
        let status = self.get_status().await?;
        if !status.server_running {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Gemma model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Model sunucusunu başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::RubricDraft,
            &input.prompt,
            json!({
                "rawText": input.raw_text,
                "targetQuestionNumber": input.target_question_number,
                "strictJsonOnly": input.strict_json_only,
                "attempt": input.attempt,
            }),
            RUBRIC_MAX_TOKENS,
            Some(ModelResponseFormat::JsonSchema {
                name: "rubric_extraction_suggestion".to_string(),
                schema: crate::domain::rubric::canonical_rubric_extraction_schema(),
            }),
        );
        let prompt = contract.system_policy.clone();

        let messages = if input.raw_text.is_some() {
            json!([
                {
                    "role": "system",
                    "content": prompt.clone()
                },
                {
                    "role": "user",
                    "content": user_data_message(&contract)
                }
            ])
        } else if input.image_path.is_some() || !input.model_input_images.is_empty() {
            let images = request_images(
                &input.model_input_images,
                input.image_path.as_deref(),
                "Rubrik sayfası okunamadı.",
            )?;
            json!([
                {
                    "role": "system",
                    "content": prompt.clone()
                },
                {
                    "role": "user",
                    "content": build_image_content(user_data_message(&contract), &images)
                }
            ])
        } else {
            return Err(app_error(
                AppErrorCode::UnknownError,
                "Request must have raw_text or image_path",
                None,
                None,
            ));
        };

        let request_body = json!({
            "model": "gemma",
            "messages": messages,
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });

        let (http_status, response_body, duration_ms) = self
            .send_chat_request(&self.base_url, request_body, 600, "RubricExtraction")
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusu başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Model sunucusunun loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let finish_reason = extract_finish_reason(&response_body);
        let reasoning_length = extract_reasoning_length(&response_body);
        let cleaned = strip_reasoning_and_fences(&assistant_content);
        let extraction_method = if input.raw_text.is_some() {
            "pdftotext_text_only"
        } else {
            "vision_fallback"
        };
        let prompt_length = prompt.len();
        let raw_text_length = input.raw_text.as_ref().map(|text| text.len()).unwrap_or(0);
        let payload_summary = build_payload_summary(
            prompt_length as u32,
            600,
            Some(&input.model_input_images),
            input
                .image_path
                .as_ref()
                .and_then(|path| std::fs::metadata(path).ok().map(|m| m.len())),
            Some(RUBRIC_MAX_TOKENS),
        );
        let raw_response_path = save_rubric_import_artifacts(
            &input,
            extraction_method,
            raw_text_length,
            prompt_length,
            &assistant_content,
            &cleaned,
        )?;
        if cleaned.trim().is_empty() {
            let parse_error = app_error(
                if assistant_content.contains("<think>") {
                    AppErrorCode::ModelResponseReasoningOnly
                } else {
                    AppErrorCode::ModelResponseEmpty
                },
                "Model final cevap içeriği üretmedi.",
                Some(assistant_content.clone()),
                Some("Promptu veya model çıktısını kontrol edin.".to_string()),
            );
            let _ = save_rubric_parse_error(&input, &parse_error, &assistant_content, &cleaned);
            let mut parse_error = parse_error;
            if let Some(dir) = rubric_artifact_dir(&input)? {
                parse_error.technical_details = Some(format!(
                    "retry_attempted={}\nraw_response_path={}\nextracted_json_path={}\nparse_error_path={}\nschema_expected=canonical rubric questions schema\n{}",
                    input.strict_json_only,
                    dir.join("response_raw.txt").display(),
                    dir.join("response_extracted_json.txt").display(),
                    dir.join("parse_error.json").display(),
                    parse_error
                        .technical_details
                        .clone()
                        .unwrap_or_default()
                ));
            }
            return Err(parse_error);
        }

        let parsed = match parse_rubric_model_response(&cleaned) {
            Ok(payload) => payload,
            Err(error) => {
                let _ = save_rubric_parse_error(&input, &error, &assistant_content, &cleaned);
                let mut error = error;
                if let Some(dir) = rubric_artifact_dir(&input)? {
                    error.technical_details = Some(format!(
                        "retry_attempted={}\nraw_response_path={}\nextracted_json_path={}\nparse_error_path={}\nschema_expected=canonical rubric questions schema\n{}",
                        input.strict_json_only,
                        dir.join("response_raw.txt").display(),
                        dir.join("response_extracted_json.txt").display(),
                        dir.join("parse_error.json").display(),
                        error
                            .technical_details
                            .clone()
                            .unwrap_or_default()
                    ));
                }
                return Err(error);
            }
        };

        let output = rubric_payload_to_output(parsed);
        Ok(RubricExtractionResult {
            output,
            raw_response: assistant_content.clone(),
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::RubricDraft,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(payload_summary.prompt_length),
                image_count: Some(payload_summary.image_count),
                image_total_bytes: Some(payload_summary.image_total_bytes),
                base64_approx_total_bytes: Some(payload_summary.base64_approx_total_bytes),
                model_input_images: payload_summary.model_input_images,
                timeout_seconds: Some(payload_summary.timeout_seconds),
                max_tokens: payload_summary.max_tokens,
                finish_reason: finish_reason.clone(),
                content_length: Some(assistant_content.len() as u32),
                reasoning_content_length: reasoning_length,
                raw_text_stored_path: raw_response_path,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
        })
    }

    async fn extract_student_answer_ocr(
        &self,
        input: StudentAnswerOcrRequest,
    ) -> Result<StudentAnswerOcrResult, AppError> {
        let preprocess_mode = input.preprocess_mode;
        let preprocess_version = input.preprocess_version.clone();
        let model_input_crop_ref = input.model_input_crop_ref.clone();
        let status = self.get_status().await?;
        if !status.server_running {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Gemma model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Model sunucusunu başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let images = request_images(
            &input.model_input_images,
            None,
            "Öğrenci cevap görselleri okunamadı.",
        )?;

        let review_policy = default_ocr_review_policy();
        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::Ocr,
            &input.prompt,
            json!({
                "submissionId": input.submission_id,
                "questionId": input.question_id,
                "questionNumber": input.question_number,
                "questionText": input.question_text,
                "answerType": input.answer_type,
                "preprocessMode": preprocess_mode,
                "preprocessVersion": preprocess_version,
                "modelInputCropRef": model_input_crop_ref,
                "sourcePageNumbers": input.source_page_numbers,
                "regionIds": input.region_ids,
                "regionOrders": input.region_orders,
                "regionPageOffsets": input.region_page_offsets,
            }),
            STUDENT_ANSWER_OCR_MAX_TOKENS,
            Some(ModelResponseFormat::JsonObject),
        );

        let request_body = json!({
            "model": "gemma",
            "messages": [
                {
                    "role": "system",
                    "content": contract.system_policy.clone()
                },
                {
                    "role": "user",
                    "content": build_image_content(user_data_message(&contract), &images)
                }
            ],
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });

        let (http_status, response_body, duration_ms) = self
            .send_chat_request(&self.base_url, request_body, 600, "StudentAnswerOcr")
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusu başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Model sunucusunun loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let finish_reason = extract_finish_reason(&response_body);
        let reasoning_length = extract_reasoning_length(&response_body);
        let cleaned = strip_reasoning_and_fences(&assistant_content);
        if cleaned.trim().is_empty() {
            return Err(app_error(
                if assistant_content.contains("<think>") {
                    AppErrorCode::ModelResponseReasoningOnly
                } else {
                    AppErrorCode::ModelResponseEmpty
                },
                "Model final cevap içeriği üretmedi.",
                Some(assistant_content),
                Some("Promptu veya model çıktısını kontrol edin.".to_string()),
            ));
        }

        let payload_summary = build_payload_summary(
            contract.system_policy.len() as u32,
            600,
            Some(&input.model_input_images),
            images.first().map(|image| image.bytes_len),
            Some(STUDENT_ANSWER_OCR_MAX_TOKENS),
        );
        let parse_outcome = parse_student_answer_ocr_output_with_policy(
            &assistant_content,
            &input.question_text,
            &input.answer_type,
            &review_policy,
        );
        let StudentAnswerOcrParseOutcome {
            output,
            parsed_json,
            parse_error,
            salvaged_answer_text,
            parse_strategy,
            printed_text_mixed,
            printed_question_leak_detected,
        } = parse_outcome;
        let raw_response_path =
            save_student_answer_ocr_artifacts(&input, &assistant_content, &cleaned)?;
        Ok(StudentAnswerOcrResult {
            output,
            raw_response: assistant_content.clone(),
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::Ocr,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(payload_summary.prompt_length),
                image_count: Some(payload_summary.image_count),
                image_total_bytes: Some(payload_summary.image_total_bytes),
                base64_approx_total_bytes: Some(payload_summary.base64_approx_total_bytes),
                model_input_images: payload_summary.model_input_images,
                timeout_seconds: Some(payload_summary.timeout_seconds),
                max_tokens: payload_summary.max_tokens,
                finish_reason: finish_reason.clone(),
                content_length: Some(assistant_content.len() as u32),
                reasoning_content_length: reasoning_length,
                raw_text_stored_path: raw_response_path,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
            parse_error,
            parsed_json,
            salvaged_answer_text,
            parse_strategy,
            model_request_metadata: Some(json!({
                "requestKind": "student_answer_ocr",
                "submissionId": input.submission_id,
                "questionId": input.question_id,
                "questionNumber": input.question_number,
                "questionTextLength": input.question_text.len(),
                "answerType": input.answer_type,
                "sourcePageNumbers": input.source_page_numbers,
                "preprocessMode": preprocess_mode,
                "preprocessModeLabel": preprocess_mode.as_ref().map(preprocess_mode_label),
                "preprocessVersion": preprocess_version,
                "modelInputCropRef": model_input_crop_ref,
                "promptLength": payload_summary.prompt_length,
                "imageCount": payload_summary.image_count,
                "imageTotalBytes": payload_summary.image_total_bytes,
                "base64ApproxTotalBytes": payload_summary.base64_approx_total_bytes,
                "timeoutSeconds": payload_summary.timeout_seconds,
                "maxTokens": payload_summary.max_tokens,
                "diagnostics": {
                    "endpoint": self.chat_url(&self.base_url),
                    "httpStatus": http_status,
                    "durationMs": duration_ms,
                    "finishReason": finish_reason.clone(),
                    "contentLength": assistant_content.len(),
                    "reasoningContentLength": reasoning_length,
                }
            })),
            printed_text_mixed,
            printed_question_leak_detected,
        })
    }

    async fn suggest_student_answer_issue_correction(
        &self,
        input: StudentAnswerOcrIssueCorrectionRequest,
    ) -> Result<StudentAnswerOcrIssueCorrectionResult, AppError> {
        let status = self.get_status().await?;
        if !status.server_running {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Gemma model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Model sunucusunu başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let images = request_images(
            &input.model_input_images,
            None,
            "OCR sorun görseli okunamadı.",
        )?;
        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::OcrIssueCorrection,
            &input.prompt,
            json!({
                "observedText": input.observed_text,
                "highlightRegion": input.highlight_region,
                "modelInputCropRef": input.model_input_crop_ref,
                "sourceImageRef": input.source_image_ref,
                "imageQuality": {
                    "hasHighlightRegion": input.highlight_region.is_some(),
                    "imageCount": input.model_input_images.len(),
                },
            }),
            STUDENT_ANSWER_OCR_ISSUE_CORRECTION_MAX_TOKENS,
            Some(ModelResponseFormat::JsonObject),
        );
        let request_body = json!({
            "model": "gemma",
            "messages": [
                {
                    "role": "system",
                    "content": contract.system_policy.clone()
                },
                {
                    "role": "user",
                    "content": build_image_content(user_data_message(&contract), &images)
                }
            ],
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });

        let (http_status, response_body, duration_ms) = self
            .send_chat_request(
                &self.base_url,
                request_body,
                300,
                "StudentAnswerOcrIssueCorrection",
            )
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusu başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Model sunucusunun loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let finish_reason = extract_finish_reason(&response_body);
        let reasoning_length = extract_reasoning_length(&response_body);
        let cleaned = strip_reasoning_and_fences(&assistant_content);
        if cleaned.trim().is_empty() {
            return Err(app_error(
                if assistant_content.contains("<think>") {
                    AppErrorCode::ModelResponseReasoningOnly
                } else {
                    AppErrorCode::ModelResponseEmpty
                },
                "Model final cevap içeriği üretmedi.",
                Some(assistant_content),
                Some("Promptu veya model çıktısını kontrol edin.".to_string()),
            ));
        }

        let correction_policy = default_ocr_review_policy();
        let parsed =
            parse_student_answer_issue_correction_output_with_policy(&cleaned, &correction_policy)?;
        let payload_summary = build_payload_summary(
            contract.system_policy.len() as u32,
            300,
            Some(&input.model_input_images),
            images.first().map(|image| image.bytes_len),
            Some(STUDENT_ANSWER_OCR_ISSUE_CORRECTION_MAX_TOKENS),
        );
        let raw_response_path =
            save_student_answer_issue_correction_artifacts(&input, &assistant_content, &cleaned)?;

        Ok(StudentAnswerOcrIssueCorrectionResult {
            output: parsed.output,
            raw_response: assistant_content.clone(),
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::OcrIssueCorrection,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(payload_summary.prompt_length),
                image_count: Some(payload_summary.image_count),
                image_total_bytes: Some(payload_summary.image_total_bytes),
                base64_approx_total_bytes: Some(payload_summary.base64_approx_total_bytes),
                model_input_images: payload_summary.model_input_images,
                timeout_seconds: Some(payload_summary.timeout_seconds),
                max_tokens: payload_summary.max_tokens,
                finish_reason: finish_reason.clone(),
                content_length: Some(assistant_content.len() as u32),
                reasoning_content_length: reasoning_length,
                raw_text_stored_path: raw_response_path,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
            parse_error: parsed.parse_error,
            parsed_json: parsed.parsed_json,
            model_request_metadata: Some(json!({
                "requestKind": "student_answer_ocr_issue_correction",
                "ocrRecordId": input.ocr_record_id,
                "issueId": input.issue_id,
                "observedText": input.observed_text,
                "questionNumber": input.question_number,
                "promptContract": invocation_metadata(&contract),
                "promptLength": payload_summary.prompt_length,
                "imageCount": payload_summary.image_count,
                "imageTotalBytes": payload_summary.image_total_bytes,
                "base64ApproxTotalBytes": payload_summary.base64_approx_total_bytes,
                "timeoutSeconds": payload_summary.timeout_seconds,
                "maxTokens": payload_summary.max_tokens,
                "diagnostics": {
                    "endpoint": self.chat_url(&self.base_url),
                    "httpStatus": http_status,
                    "durationMs": duration_ms,
                    "finishReason": finish_reason,
                    "contentLength": assistant_content.len(),
                    "reasoningContentLength": reasoning_length,
                }
            })),
        })
    }

    async fn extract_student_identity_ocr(
        &self,
        input: StudentIdentityOcrRequest,
    ) -> Result<StudentIdentityOcrResult, AppError> {
        let preprocess_mode = input.preprocess_mode;
        let preprocess_version = input.preprocess_version.clone();
        let model_input_crop_ref = input.model_input_crop_ref.clone();
        let status = self.get_status().await?;
        if !status.server_running {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Gemma model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Model sunucusunu başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let images = request_images(
            &input.model_input_images,
            None,
            "Öğrenci kimlik crop görseli okunamadı.",
        )?;
        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::Ocr,
            &input.prompt,
            json!({
                "submissionId": input.submission_id,
                "preprocessMode": preprocess_mode,
                "preprocessVersion": preprocess_version,
                "modelInputCropRef": model_input_crop_ref,
                "sourcePageNumbers": input.source_page_numbers,
            }),
            STUDENT_IDENTITY_OCR_MAX_TOKENS,
            Some(ModelResponseFormat::JsonObject),
        );
        let request_body = json!({
            "model": "gemma",
            "messages": [
                { "role": "system", "content": contract.system_policy.clone() },
                {
                    "role": "user",
                    "content": build_image_content(user_data_message(&contract), &images)
                }
            ],
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });
        let (http_status, response_body, duration_ms) = self
            .send_chat_request(&self.base_url, request_body, 300, "StudentIdentityOcr")
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusu başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Model sunucusunun loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let finish_reason = extract_finish_reason(&response_body);
        let reasoning_length = extract_reasoning_length(&response_body);
        let cleaned = strip_reasoning_and_fences(&assistant_content);
        if cleaned.trim().is_empty() {
            return Err(app_error(
                if assistant_content.contains("<think>") {
                    AppErrorCode::ModelResponseReasoningOnly
                } else {
                    AppErrorCode::ModelResponseEmpty
                },
                "Model final cevap içeriği üretmedi.",
                Some(assistant_content),
                Some("Promptu veya model çıktısını kontrol edin.".to_string()),
            ));
        }

        let payload_summary = build_payload_summary(
            contract.system_policy.len() as u32,
            300,
            Some(&input.model_input_images),
            images.first().map(|image| image.bytes_len),
            Some(STUDENT_IDENTITY_OCR_MAX_TOKENS),
        );
        let parse_outcome = parse_student_identity_ocr_output(&assistant_content);
        let raw_response_path =
            save_student_identity_ocr_artifacts(&input, &assistant_content, &cleaned)?;
        Ok(StudentIdentityOcrResult {
            output: parse_outcome.output,
            raw_response: assistant_content.clone(),
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::Ocr,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(payload_summary.prompt_length),
                image_count: Some(payload_summary.image_count),
                image_total_bytes: Some(payload_summary.image_total_bytes),
                base64_approx_total_bytes: Some(payload_summary.base64_approx_total_bytes),
                model_input_images: payload_summary.model_input_images,
                timeout_seconds: Some(payload_summary.timeout_seconds),
                max_tokens: payload_summary.max_tokens,
                finish_reason,
                content_length: Some(assistant_content.len() as u32),
                reasoning_content_length: reasoning_length,
                raw_text_stored_path: raw_response_path,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
            parse_error: parse_outcome.parse_error,
            parsed_json: parse_outcome.parsed_json,
            parse_strategy: parse_outcome.parse_strategy,
            model_request_metadata: Some(json!({
                "requestKind": "student_identity_ocr",
                "submissionId": input.submission_id,
                "sourcePageNumbers": input.source_page_numbers,
                "preprocessMode": preprocess_mode,
                "preprocessModeLabel": preprocess_mode.as_ref().map(preprocess_mode_label),
                "preprocessVersion": preprocess_version,
                "modelInputCropRef": model_input_crop_ref,
                "promptLength": payload_summary.prompt_length,
                "imageCount": payload_summary.image_count,
                "timeoutSeconds": payload_summary.timeout_seconds,
                "maxTokens": payload_summary.max_tokens,
            })),
        })
    }

    async fn cleanup_speaking_transcript(
        &self,
        input: SpeakingTranscriptCleanupRequest,
    ) -> Result<SpeakingTranscriptCleanupResult, AppError> {
        // The speaking job already completed a full readiness probe before
        // entering cleanup. Avoid spending another model completion on the
        // same probe; health is sufficient at this boundary.
        let status = self.health_status(&self.base_url).await?;
        if !status.server_running || !status.health_ok {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "ASR düzeltme model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Gemma 4 12B modelini başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::SpeakingTranscriptCleanup,
            &input.prompt,
            json!({
                "rawTranscript": input.raw_transcript,
                "segments": input.segments,
            }),
            input.max_tokens,
            Some(ModelResponseFormat::JsonObject),
        );
        let request_body = json!({
            "model": "gemma-4-12b",
            "messages": [
                { "role": "system", "content": contract.system_policy.clone() },
                { "role": "user", "content": user_data_message(&contract) }
            ],
            "chat_template_kwargs": { "enable_thinking": false },
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });
        let (http_status, response_body, duration_ms) = self
            .send_chat_request(
                &self.base_url,
                request_body,
                input.timeout_seconds,
                "SpeakingTranscriptCleanup",
            )
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "ASR düzeltme modeli başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Gemma 4 12B model loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let segments = parse_speaking_transcript_cleanup_output(&assistant_content)?;
        let cleaned_transcript = segments
            .iter()
            .map(|segment| segment.cleaned_text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        Ok(SpeakingTranscriptCleanupResult {
            cleaned_transcript: cleaned_transcript.clone(),
            segments,
            raw_response: assistant_content,
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::SpeakingTranscriptCleanup,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(contract.system_policy.len() as u32),
                image_count: Some(0),
                image_total_bytes: Some(0),
                base64_approx_total_bytes: Some(0),
                model_input_images: vec![],
                timeout_seconds: Some(input.timeout_seconds),
                max_tokens: Some(input.max_tokens),
                finish_reason: extract_finish_reason(&response_body),
                content_length: Some(cleaned_transcript.len() as u32),
                reasoning_content_length: extract_reasoning_length(&response_body),
                raw_text_stored_path: None,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
        })
    }

    async fn generate_analysis_report(
        &self,
        input: AnalysisReportRequest,
    ) -> Result<AnalysisReportResult, AppError> {
        let status = self.get_status().await?;
        if !status.server_running || !status.health_ok {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Analiz raporu modeli çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Gemma 4 12B modelini başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::AnalysisReport,
            &input.prompt,
            json!({ "analysisData": input.prompt }),
            900,
            Some(ModelResponseFormat::JsonSchema {
                name: "analysis_report_v1".to_string(),
                schema: analysis_report_json_schema(),
            }),
        );
        let request_body = json!({
            "model": "gemma-4-12b",
            "messages": [
                { "role": "system", "content": contract.system_policy.clone() },
                {
                    "role": "user",
                    "content": user_data_message(&contract)
                }
            ],
            "chat_template_kwargs": { "enable_thinking": false },
            "temperature": contract.invocation.sampling_parameters.temperature,
            "top_k": contract.invocation.sampling_parameters.top_k,
            "top_p": contract.invocation.sampling_parameters.top_p,
            "seed": contract.invocation.sampling_parameters.seed,
            "max_tokens": contract.invocation.sampling_parameters.max_tokens,
            "response_format": response_format_value(&contract),
            "stream": false
        });
        let (http_status, response_body, duration_ms) = self
            .send_chat_request(&self.base_url, request_body, 120, "AnalysisReport")
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::AnalysisFailed,
                "Gemma analiz raporu üretemedi.",
                Some(format!("HTTP {http_status}\n{response_body}")),
                Some("Grafikler korunmuştur; raporu yeniden oluşturabilirsiniz.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let report = strip_reasoning_and_fences(&assistant_content);
        let parsed = parse_analysis_model_output(&report)?;

        Ok(AnalysisReportResult {
            report: report.clone(),
            claims: parsed.claims,
            raw_response: assistant_content,
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::AnalysisReport,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(contract.system_policy.len() as u32),
                image_count: Some(0),
                image_total_bytes: Some(0),
                base64_approx_total_bytes: Some(0),
                model_input_images: vec![],
                timeout_seconds: Some(120),
                max_tokens: Some(900),
                finish_reason: extract_finish_reason(&response_body),
                content_length: Some(report.len() as u32),
                reasoning_content_length: extract_reasoning_length(&response_body),
                raw_text_stored_path: None,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
        })
    }

    async fn score_answer(&self, input: ScoringRequest) -> Result<ScoringResult, AppError> {
        let speaking_request = input.answer_type == "speaking";
        let status = self.health_status(&self.base_url).await?;
        if !status.server_running {
            return Err(app_error(
                AppErrorCode::ModelServerNotRunning,
                "Gemma model sunucusu çalışmıyor.",
                status.last_error.as_ref().map(|error| format!("{error:?}")),
                Some("Model sunucusunu başlatıp tekrar deneyin.".to_string()),
            ));
        }

        let max_tokens = if speaking_request { 3072 } else { 2048 };
        let timeout_seconds = if speaking_request { 300 } else { 600 };
        let contract = request_contract(
            input.prompt_contract.clone(),
            ModelRequestKind::Scoring,
            &input.prompt,
            json!({
                "submissionId": input.submission_id,
                "questionId": input.question_id,
                "questionNumber": input.question_number,
                "studentDisplayName": input.student_display_name,
                "studentNumber": input.student_number,
                "studentClassName": input.student_class_name,
                "questionText": input.question_text,
                "expectedAnswer": input.expected_answer,
                "answerType": input.answer_type,
                "answerText": input.answer_text,
                "rubric": input.rubric_json,
                "criterionScoresSeed": input.criterion_scores_seed,
                "partialCreditHints": input.partial_credit_hints,
                "zeroScoreConditions": input.zero_score_conditions,
                "commonMistakes": input.common_mistakes,
                "maxScore": input.max_score,
                "sourceHash": input.source_hash,
                "packageHash": input.package_hash,
                "ocrRecordHash": input.ocr_record_hash,
            }),
            max_tokens,
            Some(ModelResponseFormat::JsonObject),
        );
        let request_body = build_scoring_request_body(&contract);

        let (http_status, response_body, duration_ms) = self
            .send_chat_request(&self.base_url, request_body, timeout_seconds, "Scoring")
            .await?;
        if !(200..300).contains(&http_status) {
            return Err(app_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusu başarılı bir yanıt döndürmedi.",
                Some(response_body),
                Some("Model sunucusunun loglarını kontrol edin.".to_string()),
            ));
        }

        let assistant_content = extract_assistant_content(&response_body)?;
        let finish_reason = extract_finish_reason(&response_body);
        let reasoning_length = extract_reasoning_length(&response_body);
        let cleaned = strip_reasoning_and_fences(&assistant_content);
        if cleaned.trim().is_empty() {
            return Err(app_error(
                if assistant_content.contains("<think>") {
                    AppErrorCode::ModelResponseReasoningOnly
                } else {
                    AppErrorCode::ModelResponseEmpty
                },
                "Model final puanlama içeriği üretmedi.",
                Some(assistant_content),
                Some("Promptu veya model çıktısını kontrol edin.".to_string()),
            ));
        }

        let payload_summary = build_payload_summary(
            contract.system_policy.len() as u32,
            timeout_seconds,
            None,
            None,
            Some(max_tokens),
        );
        let parse_outcome = parse_scoring_output(&assistant_content, input.max_score);
        Ok(ScoringResult {
            output: parse_outcome.output,
            raw_response: assistant_content.clone(),
            diagnostics: ModelDiagnostics {
                endpoint: self.chat_url(&self.base_url),
                request_kind: ModelRequestKind::Scoring,
                http_status: Some(http_status),
                duration_ms,
                prompt_length: Some(payload_summary.prompt_length),
                image_count: Some(payload_summary.image_count),
                image_total_bytes: Some(payload_summary.image_total_bytes),
                base64_approx_total_bytes: Some(payload_summary.base64_approx_total_bytes),
                model_input_images: payload_summary.model_input_images,
                timeout_seconds: Some(payload_summary.timeout_seconds),
                max_tokens: payload_summary.max_tokens,
                finish_reason,
                content_length: Some(assistant_content.len() as u32),
                reasoning_content_length: reasoning_length,
                raw_text_stored_path: None,
                error_code: None,
                provenance: Some(ModelProvenance::from_invocation(&contract.invocation)),
            },
            parse_error: parse_outcome.parse_error,
            parsed_json: parse_outcome.parsed_json,
            salvaged_rationale: parse_outcome.salvaged_rationale,
            parse_strategy: parse_outcome.parse_strategy,
            model_request_metadata: Some(json!({
                "requestKind": "scoring",
                "submissionId": input.submission_id,
                "questionId": input.question_id,
                "questionNumber": input.question_number,
                "studentDisplayName": input.student_display_name,
                "studentNumber": input.student_number,
                "studentClassName": input.student_class_name,
                "promptLength": payload_summary.prompt_length,
                "timeoutSeconds": payload_summary.timeout_seconds,
                "maxTokens": payload_summary.max_tokens,
                "sourceHash": input.source_hash,
                "packageHash": input.package_hash,
                "ocrRecordHash": input.ocr_record_hash,
                "promptContract": invocation_metadata(&contract),
            })),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: serde_json::Value,
}

fn extract_assistant_content(body_text: &str) -> Result<String, AppError> {
    if body_text.trim().is_empty() {
        return Err(app_error(
            AppErrorCode::ModelResponseEmpty,
            "Model response was empty.",
            None,
            Some("Model sunucusunu kontrol edin.".to_string()),
        ));
    }

    let response: ChatResponse = serde_json::from_str(body_text).map_err(|error| {
        app_error(
            AppErrorCode::ModelResponseInvalidJson,
            "Model yanıtı JSON olarak çözülemedi.",
            Some(error.to_string()),
            Some("Model response schema uyumsuz olabilir.".to_string()),
        )
    })?;

    let content = response
        .choices
        .first()
        .map(|choice| content_value_to_text(&choice.message.content))
        .unwrap_or_default();

    Ok(content)
}

fn content_value_to_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .map(content_value_to_text)
            .collect::<Vec<_>>()
            .join(""),
        serde_json::Value::Object(object) => {
            for key in ["text", "content", "value"] {
                if let Some(value) = object.get(key) {
                    return content_value_to_text(value);
                }
            }
            value.to_string()
        }
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn extract_finish_reason(body_text: &str) -> Option<String> {
    serde_json::from_str::<ChatResponse>(body_text)
        .ok()
        .and_then(|response| {
            response
                .choices
                .first()
                .and_then(|choice| choice.finish_reason.clone())
        })
}

fn extract_reasoning_length(body_text: &str) -> Option<u32> {
    let content = serde_json::from_str::<ChatResponse>(body_text)
        .ok()
        .and_then(|response| {
            response
                .choices
                .first()
                .map(|choice| choice.message.content.to_string())
        })?;
    let start = content.find("<think>")?;
    let after_start = &content[start + "<think>".len()..];
    let end = after_start.find("</think>")?;
    Some(after_start[..end].chars().count() as u32)
}

fn strip_reasoning_and_fences(text: &str) -> String {
    let without_think = remove_think_blocks(text);
    let trimmed = without_think.trim();

    // 1. Try to find ```json ... ``` code block anywhere in the text
    if let Some(start_idx) = trimmed.find("```json") {
        let content_after = &trimmed[start_idx + "```json".len()..];
        if let Some(end_idx) = content_after.find("```") {
            return content_after[..end_idx].trim().to_string();
        }
    }

    // 2. Try to find ``` ... ``` code block anywhere in the text
    if let Some(start_idx) = trimmed.find("```") {
        let content_after = &trimmed[start_idx + "```".len()..];
        if let Some(end_idx) = content_after.find("```") {
            return content_after[..end_idx].trim().to_string();
        }
    }

    // 3. Extract a complete object/array from prose or wrapper tokens. If the
    // outer JSON is truncated, keep it intact so callers can attempt recovery
    // instead of accidentally selecting one complete nested item.
    if let Some((start_index, _)) = trimmed
        .char_indices()
        .find(|(_, ch)| *ch == '{' || *ch == '[')
    {
        if let Some(candidate) = extract_balanced_json_from_start(trimmed, start_index) {
            return candidate;
        }
    }
    if let (Some(first_brace), Some(last_brace)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if first_brace < last_brace {
            return trimmed[first_brace..=last_brace].trim().to_string();
        }
    }

    trimmed.to_string()
}

struct RequestImage {
    data_url: String,
    bytes_len: u64,
}

fn request_images(
    model_input_images: &[crate::domain::model::ModelInputImage],
    fallback_path: Option<&str>,
    read_message: &str,
) -> Result<Vec<RequestImage>, AppError> {
    let paths = if model_input_images.is_empty() {
        fallback_path
            .map(|path| vec![path.to_string()])
            .unwrap_or_default()
    } else {
        model_input_images
            .iter()
            .map(|image| image.output_image_path.clone())
            .collect()
    };

    let mut images = Vec::new();
    for path in paths {
        let image_bytes = std::fs::read(&path).map_err(|error| {
            app_error(
                AppErrorCode::PdfRenderFailed,
                read_message,
                Some(error.to_string()),
                Some("PDF render çıktısını kontrol edin.".to_string()),
            )
        })?;
        images.push(RequestImage {
            data_url: format!(
                "data:image/jpeg;base64,{}",
                general_purpose::STANDARD.encode(&image_bytes)
            ),
            bytes_len: image_bytes.len() as u64,
        });
    }

    Ok(images)
}

fn build_image_content(text: String, images: &[RequestImage]) -> Vec<serde_json::Value> {
    let mut content = vec![json!({"type": "text", "text": text})];
    content.extend(
        images
            .iter()
            .map(|image| json!({"type": "image_url", "image_url": {"url": image.data_url, "detail": "high"}})),
    );
    content
}

fn preprocess_mode_label(mode: &OcrImagePreprocessMode) -> &'static str {
    match mode {
        OcrImagePreprocessMode::Original => "orijinal",
        OcrImagePreprocessMode::CleanGrayscale => "temiz gri ton",
        OcrImagePreprocessMode::HandwritingEnhanced => "el yazısı güçlendirildi",
        OcrImagePreprocessMode::HighContrast => "yüksek kontrast",
        OcrImagePreprocessMode::HighContrastBw => "yüksek kontrast siyah-beyaz",
    }
}

fn extract_first_balanced_json_candidate(text: &str) -> Option<String> {
    let trimmed = text.trim();
    trimmed
        .char_indices()
        .filter(|(_, ch)| *ch == '{' || *ch == '[')
        .find_map(|(start_index, _)| extract_balanced_json_from_start(trimmed, start_index))
}

fn extract_balanced_json_from_start(text: &str, start_index: usize) -> Option<String> {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escape = false;

    for (relative_index, ch) in text[start_index..].char_indices() {
        let index = start_index + relative_index;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                if stack.pop() != Some(ch) {
                    return None;
                }
                if stack.is_empty() {
                    return Some(text[start_index..index + ch.len_utf8()].trim().to_string());
                }
            }
            _ => {}
        }
    }

    None
}

fn normalize_alias_warning(field: &str, alias: &str) -> String {
    format!("{field}_alias:{alias}")
}

fn sanitize_json_control_chars(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escape = false;

    for ch in text.chars() {
        if in_string {
            if escape {
                escape = false;
                result.push(ch);
                continue;
            }
            match ch {
                '\\' => {
                    escape = true;
                    result.push(ch);
                }
                '"' => {
                    in_string = false;
                    result.push(ch);
                }
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                c if c.is_control() => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
                _ => result.push(ch),
            }
        } else {
            match ch {
                '"' => {
                    in_string = true;
                    result.push(ch);
                }
                _ => result.push(ch),
            }
        }
    }
    result
}

fn extract_speaking_cleanup_segments_from_value(
    value: &serde_json::Value,
) -> Option<Vec<SpeakingTranscriptCleanupOutputSegment>> {
    let array = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(obj) = value.as_object() {
        for key in ["result", "output", "response", "payload"] {
            if let Some(nested) = obj.get(key) {
                if let Some(segments) = extract_speaking_cleanup_segments_from_value(nested) {
                    return Some(segments);
                }
            }
        }
        [
            "segments",
            "transcriptSegments",
            "transcript_segments",
            "cleanedSegments",
            "cleaned_segments",
            "results",
            "outputs",
            "data",
            "items",
            "transcript",
        ]
        .iter()
        .find_map(|key| obj.get(*key))
        .and_then(serde_json::Value::as_array)?
    } else {
        return None;
    };

    if array.is_empty() {
        return Some(vec![]);
    }

    let mut segments = Vec::with_capacity(array.len());
    for item in array {
        if let Ok(segment) =
            serde_json::from_value::<SpeakingTranscriptCleanupOutputSegment>(item.clone())
        {
            segments.push(segment);
        } else if let Some(obj) = item.as_object() {
            let segment_id = ["segment_id", "segmentId", "id", "index"]
                .iter()
                .find_map(|key| obj.get(*key).and_then(json_text))?;
            let cleaned_text = [
                "cleaned_text",
                "cleanedText",
                "text",
                "transcript",
                "cleaned_transcript",
                "cleanedTranscript",
                "content",
            ]
            .iter()
            .find_map(|key| obj.get(*key).and_then(json_text))?;
            let changes = obj
                .get("changes")
                .or_else(|| obj.get("modifications"))
                .or_else(|| obj.get("edits"))
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let semantic_change_detected = obj
                .get("semantic_change_detected")
                .or_else(|| obj.get("semanticChangeDetected"))
                .or_else(|| obj.get("semantic_change"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let needs_review = obj
                .get("needs_review")
                .or_else(|| obj.get("needsReview"))
                .or_else(|| obj.get("review"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            segments.push(SpeakingTranscriptCleanupOutputSegment {
                segment_id,
                cleaned_text,
                changes,
                semantic_change_detected,
                needs_review,
            });
        } else {
            return None;
        }
    }

    Some(segments)
}

fn parse_speaking_transcript_cleanup_output(
    raw_content: &str,
) -> Result<Vec<SpeakingTranscriptCleanupOutputSegment>, AppError> {
    let cleaned_response = strip_reasoning_and_fences(raw_content);
    if cleaned_response.trim().is_empty() {
        return Err(app_error(
            if raw_content.contains("<think>") {
                AppErrorCode::ModelResponseReasoningOnly
            } else {
                AppErrorCode::ModelResponseEmpty
            },
            "ASR düzeltme modeli geçerli bir transkript üretmedi.",
            Some(raw_content.to_string()),
            Some("Kayıt öğretmen incelemesine bırakıldı.".to_string()),
        ));
    }

    let raw_trimmed = raw_content.trim();
    let fenced_candidate = extract_fenced_json_candidate(raw_content);
    let balanced_candidate = extract_first_balanced_json_candidate(&cleaned_response);

    let mut candidates = vec![
        raw_trimmed.to_string(),
        cleaned_response.trim().to_string(),
        fenced_candidate.unwrap_or_default(),
        balanced_candidate.unwrap_or_default(),
    ];
    for (start_index, ch) in cleaned_response.char_indices() {
        if (ch == '{' || ch == '[')
            && extract_balanced_json_from_start(&cleaned_response, start_index).is_some_and(
                |candidate| {
                    if !candidates.iter().any(|existing| existing == &candidate) {
                        candidates.push(candidate);
                    }
                    true
                },
            )
        {
            // Candidate collection is intentionally tolerant: schema validation below
            // decides which balanced JSON value is the cleanup payload.
        }
    }

    let mut last_parse_error: Option<String> = None;
    let mut saw_valid_json = false;

    for candidate in &candidates {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(value) => {
                saw_valid_json = true;
                if let Some(segments) = extract_speaking_cleanup_segments_from_value(&value) {
                    return Ok(segments);
                }
            }
            Err(err) => {
                last_parse_error = Some(err.to_string());
            }
        }

        let repaired = sanitize_json_control_chars(trimmed);
        if repaired != trimmed {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&repaired) {
                saw_valid_json = true;
                if let Some(segments) = extract_speaking_cleanup_segments_from_value(&value) {
                    return Ok(segments);
                }
            }
        }
    }

    Err(app_error(
        if saw_valid_json {
            AppErrorCode::ModelResponseInvalidSchema
        } else {
            AppErrorCode::ModelResponseInvalidJson
        },
        if saw_valid_json {
            "Konuşma transkript temizleme çıktısı beklenen segment şemasında değil."
        } else {
            "Konuşma transkript temizleme çıktısı geçerli JSON değil."
        },
        Some(format!(
            "parse_error={}; valid_json_seen={}; raw_model_output={}",
            last_parse_error.unwrap_or_else(|| "schema_not_found".to_string()),
            saw_valid_json,
            raw_content.chars().take(12000).collect::<String>()
        )),
        Some(
            "Temizlemeyi yeniden çalıştırın veya ham transkripti öğretmen onayına gönderin."
                .to_string(),
        ),
    ))
}

fn analysis_report_json_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["claims"],
        "properties": {
            "claims": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["claim", "metricRefs", "recommendation"],
                    "properties": {
                        "claim": { "type": "string", "minLength": 1 },
                        "metricRefs": {
                            "type": "array",
                            "items": {
                                "oneOf": [
                                    { "type": "string", "minLength": 1 },
                                    {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["metricId"],
                                        "properties": {
                                            "metricId": { "type": "string", "minLength": 1 },
                                            "value": { "type": ["number", "null"] }
                                        }
                                    }
                                ]
                            }
                        },
                        "recommendation": { "type": "string" }
                    }
                }
            }
        }
    })
}

fn parse_analysis_model_output(report: &str) -> Result<AnalysisModelOutput, AppError> {
    if report.trim().is_empty() {
        return Err(app_error(
            AppErrorCode::ModelResponseEmpty,
            "Gemma boş bir analiz raporu döndürdü.",
            None,
            Some("Grafikler korunmuştur; analizi yeniden oluşturabilirsiniz.".to_string()),
        ));
    }
    let candidate =
        extract_first_balanced_json_candidate(report).unwrap_or_else(|| report.to_string());
    let parsed: AnalysisModelOutput = serde_json::from_str(candidate.trim()).map_err(|error| {
        app_error(
            AppErrorCode::ModelResponseInvalidJson,
            "Gemma analiz raporu beklenen yapılandırılmış biçimde değil.",
            Some(format!(
                "analysis_report_parse_error={error}; raw_model_output={report}"
            )),
            Some("Grafikler korunmuştur; analizi yeniden oluşturabilirsiniz.".to_string()),
        )
    })?;
    if parsed.claims.is_empty()
        || parsed
            .claims
            .iter()
            .any(|claim| claim.claim.trim().is_empty())
    {
        return Err(app_error(
            AppErrorCode::ModelResponseInvalidSchema,
            "Gemma analiz raporu geçerli bir iddia listesi içermiyor.",
            Some(format!("claim_count={}", parsed.claims.len())),
            Some("Grafikler korunmuştur; analizi yeniden oluşturabilirsiniz.".to_string()),
        ));
    }
    Ok(parsed)
}

fn request_metadata(
    request: &RubricExtractionRequest,
    extraction_method: &str,
    raw_text_length: usize,
    prompt_length: usize,
) -> serde_json::Value {
    let image_count = request.model_input_images.len();
    let image_total_bytes: u64 = request
        .model_input_images
        .iter()
        .map(|image| image.output_bytes)
        .sum();
    let base64_approx_total_bytes: u64 = request
        .model_input_images
        .iter()
        .map(|image| image.base64_approx_bytes)
        .sum();

    json!({
        "requestKind": "rubric_draft",
        "attempt": request.attempt,
        "strictJsonOnly": request.strict_json_only,
        "extractionMethod": extraction_method,
        "promptLength": prompt_length,
        "rawTextLength": raw_text_length,
        "imageCount": image_count,
        "imageBytes": image_total_bytes,
        "base64ApproxBytes": base64_approx_total_bytes,
        "projectRootPath": request.project_root_path,
        "jobId": request.job_id,
    })
}

fn project_artifact_dir(
    project_root: Option<&String>,
    relative: &str,
) -> Result<Option<std::path::PathBuf>, AppError> {
    let Some(project_root) = project_root else {
        return Ok(None);
    };
    let trusted_root =
        TrustedProjectRoot::from_canonical_root(std::path::PathBuf::from(project_root), false)?;
    let managed = trusted_root.managed(relative)?;
    let directory = trusted_root.root().join(managed.as_path());
    trusted_root.ensure_managed_directory(&directory)?;
    Ok(Some(directory))
}

fn write_project_artifact(
    project_root: Option<&String>,
    path: &std::path::Path,
    content: &str,
    message: &str,
) -> Result<(), AppError> {
    let Some(project_root) = project_root else {
        return Ok(());
    };
    let trusted_root =
        TrustedProjectRoot::from_canonical_root(std::path::PathBuf::from(project_root), false)?;
    let managed = trusted_root.managed_for_path(path)?;
    trusted_root
        .atomic_write(&managed, content)
        .map_err(|error| {
            app_error(
                AppErrorCode::FileWriteFailed,
                message,
                Some(error.message),
                Some("Proje logs klasörünü kontrol edin.".to_string()),
            )
        })
}

fn rubric_artifact_dir(
    request: &RubricExtractionRequest,
) -> Result<Option<std::path::PathBuf>, AppError> {
    let Some(job_id) = request.job_id.as_ref() else {
        return Ok(None);
    };
    let base_job_id = if let Some(idx) = job_id.find("_q") {
        &job_id[..idx]
    } else {
        job_id
    };
    project_artifact_dir(
        request.project_root_path.as_ref(),
        &format!(
            "logs/model_responses/rubric_import/{base_job_id}/question_{}/attempt_{}",
            request.target_question_number,
            request.attempt.max(1)
        ),
    )
}

fn save_rubric_import_artifacts(
    request: &RubricExtractionRequest,
    extraction_method: &str,
    raw_text_length: usize,
    prompt_length: usize,
    raw_response: &str,
    extracted_json: &str,
) -> Result<Option<String>, AppError> {
    let Some(dir) = rubric_artifact_dir(request)? else {
        return Ok(None);
    };

    let request_path = dir.join("request.json");
    let raw_path = dir.join("response_raw.txt");
    let extracted_path = dir.join("response_extracted_json.txt");

    let request_body = request_metadata(request, extraction_method, raw_text_length, prompt_length);
    write_project_artifact(
        request.project_root_path.as_ref(),
        &request_path,
        &serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| request_body.to_string()),
        "Rubrik request artifact kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &raw_path,
        raw_response,
        "Rubrik raw response kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &extracted_path,
        extracted_json,
        "Rubrik extracted JSON kaydedilemedi.",
    )?;

    Ok(Some(raw_path.to_string_lossy().to_string()))
}

fn save_rubric_parse_error(
    request: &RubricExtractionRequest,
    parse_error: &AppError,
    raw_response: &str,
    extracted_json: &str,
) -> Result<(), AppError> {
    let Some(dir) = rubric_artifact_dir(request)? else {
        return Ok(());
    };
    let parse_error_path = dir.join("parse_error.json");
    let payload = json!({
        "attempt": request.attempt,
        "strictJsonOnly": request.strict_json_only,
        "error": parse_error,
        "rawResponseLength": raw_response.len(),
        "extractedJsonLength": extracted_json.len(),
    });
    write_project_artifact(
        request.project_root_path.as_ref(),
        &parse_error_path,
        &serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string()),
        "Rubrik parse error artifact kaydedilemedi.",
    )?;
    Ok(())
}

fn student_answer_ocr_artifact_dir(
    request: &StudentAnswerOcrRequest,
) -> Result<Option<std::path::PathBuf>, AppError> {
    let Some(job_id) = request.job_id.as_ref() else {
        return Ok(None);
    };
    project_artifact_dir(
        request.project_root_path.as_ref(),
        &format!(
            "logs/model_responses/student_answer_ocr/{job_id}/submission_{}/question_{}",
            request.submission_id, request.question_number
        ),
    )
}

fn save_student_answer_ocr_artifacts(
    request: &StudentAnswerOcrRequest,
    raw_response: &str,
    extracted_json: &str,
) -> Result<Option<String>, AppError> {
    let Some(dir) = student_answer_ocr_artifact_dir(request)? else {
        return Ok(None);
    };

    let request_path = dir.join("request.json");
    let raw_path = dir.join("response_raw.txt");
    let extracted_path = dir.join("response_extracted_json.txt");
    let request_body = json!({
        "submissionId": request.submission_id,
        "questionId": request.question_id,
        "questionNumber": request.question_number,
        "questionTextLength": request.question_text.len(),
        "answerType": request.answer_type,
        "preprocessMode": request.preprocess_mode,
        "preprocessVersion": request.preprocess_version,
        "modelInputCropRef": request.model_input_crop_ref,
        "pageNumbers": request.source_page_numbers,
        "regionIds": request.region_ids,
        "regionOrders": request.region_orders,
        "regionPageOffsets": request.region_page_offsets,
        "promptLength": request.prompt.len(),
        "imageCount": request.model_input_images.len(),
        "projectRootPath": request.project_root_path,
        "jobId": request.job_id,
    });

    write_project_artifact(
        request.project_root_path.as_ref(),
        &request_path,
        &serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| request_body.to_string()),
        "Student OCR request artifact kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &raw_path,
        raw_response,
        "Student OCR raw response kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &extracted_path,
        extracted_json,
        "Student OCR extracted JSON kaydedilemedi.",
    )?;

    Ok(Some(raw_path.to_string_lossy().to_string()))
}

fn student_answer_issue_correction_artifact_dir(
    request: &StudentAnswerOcrIssueCorrectionRequest,
) -> Result<Option<std::path::PathBuf>, AppError> {
    let Some(job_id) = request.job_id.as_ref() else {
        return Ok(None);
    };
    project_artifact_dir(
        request.project_root_path.as_ref(),
        &format!(
            "logs/model_responses/student_answer_ocr_issue_correction/{job_id}/ocr_record_{}/question_{}",
            request.ocr_record_id, request.question_number
        ),
    )
}

fn save_student_answer_issue_correction_artifacts(
    request: &StudentAnswerOcrIssueCorrectionRequest,
    raw_response: &str,
    extracted_json: &str,
) -> Result<Option<String>, AppError> {
    let Some(dir) = student_answer_issue_correction_artifact_dir(request)? else {
        return Ok(None);
    };

    let request_path = dir.join("request.json");
    let raw_path = dir.join("response_raw.txt");
    let extracted_path = dir.join("response_extracted_json.txt");
    let request_body = json!({
        "ocrRecordId": request.ocr_record_id,
        "issueId": request.issue_id,
        "observedText": request.observed_text,
        "questionNumber": request.question_number,
        "highlightRegion": request.highlight_region,
        "modelInputCropRef": request.model_input_crop_ref,
        "sourceImageRef": request.source_image_ref,
        "promptLength": request.prompt.len(),
        "imageCount": request.model_input_images.len(),
        "projectRootPath": request.project_root_path,
        "jobId": request.job_id,
    });

    write_project_artifact(
        request.project_root_path.as_ref(),
        &request_path,
        &serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| request_body.to_string()),
        "OCR issue correction request artifact kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &raw_path,
        raw_response,
        "OCR issue correction raw response kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &extracted_path,
        extracted_json,
        "OCR issue correction extracted JSON kaydedilemedi.",
    )?;

    Ok(Some(raw_path.to_string_lossy().to_string()))
}

fn student_identity_ocr_artifact_dir(
    request: &StudentIdentityOcrRequest,
) -> Result<Option<std::path::PathBuf>, AppError> {
    let Some(job_id) = request.job_id.as_ref() else {
        return Ok(None);
    };
    project_artifact_dir(
        request.project_root_path.as_ref(),
        &format!(
            "logs/model_responses/student_identity_ocr/{job_id}/submission_{}",
            request.submission_id
        ),
    )
}

fn save_student_identity_ocr_artifacts(
    request: &StudentIdentityOcrRequest,
    raw_response: &str,
    extracted_json: &str,
) -> Result<Option<String>, AppError> {
    let Some(dir) = student_identity_ocr_artifact_dir(request)? else {
        return Ok(None);
    };
    let request_path = dir.join("request.json");
    let raw_path = dir.join("response_raw.txt");
    let extracted_path = dir.join("response_extracted_json.txt");
    let request_body = json!({
        "submissionId": request.submission_id,
        "preprocessMode": request.preprocess_mode,
        "preprocessVersion": request.preprocess_version,
        "modelInputCropRef": request.model_input_crop_ref,
        "pageNumbers": request.source_page_numbers,
        "promptLength": request.prompt.len(),
        "imageCount": request.model_input_images.len(),
        "projectRootPath": request.project_root_path,
        "jobId": request.job_id,
    });
    write_project_artifact(
        request.project_root_path.as_ref(),
        &request_path,
        &serde_json::to_string_pretty(&request_body).unwrap_or_else(|_| request_body.to_string()),
        "Student identity OCR request artifact kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &raw_path,
        raw_response,
        "Student identity OCR raw response kaydedilemedi.",
    )?;
    write_project_artifact(
        request.project_root_path.as_ref(),
        &extracted_path,
        extracted_json,
        "Student identity OCR extracted JSON kaydedilemedi.",
    )?;
    Ok(Some(raw_path.to_string_lossy().to_string()))
}

fn normalize_rubric_criteria(
    value: &serde_json::Value,
) -> (Vec<crate::domain::rubric::RubricCriterion>, Vec<String>) {
    let Some(array) = value.as_array() else {
        return (vec![], vec!["criteria_not_array".to_string()]);
    };

    let mut warnings = Vec::new();
    let criteria = array
        .iter()
        .filter_map(|criterion| {
            let object = criterion.as_object()?;
            let label = object
                .get("label")
                .or_else(|| object.get("name"))
                .or_else(|| object.get("kriter"))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let description = object
                .get("description")
                .or_else(|| object.get("açıklama"))
                .or_else(|| object.get("aciklama"))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let points = object
                .get("points")
                .or_else(|| object.get("puan"))
                .or_else(|| object.get("score"))
                .or_else(|| object.get("point"))
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0) as f32;
            if object.contains_key("name") {
                warnings.push(normalize_alias_warning("criterion.label", "name"));
            }
            if object.contains_key("kriter") {
                warnings.push(normalize_alias_warning("criterion.label", "kriter"));
            }
            if object.contains_key("puan") {
                warnings.push(normalize_alias_warning("criterion.points", "puan"));
            }
            if object.contains_key("score") {
                warnings.push(normalize_alias_warning("criterion.points", "score"));
            }
            Some(crate::domain::rubric::RubricCriterion {
                id: uuid::Uuid::new_v4().to_string(),
                label,
                description,
                points,
                levels: vec![],
            })
        })
        .collect();

    (criteria, warnings)
}

fn parse_rubric_model_response(
    raw: &str,
) -> Result<crate::domain::model::RubricImportPayload, AppError> {
    let cleaned = strip_reasoning_and_fences(raw);
    let candidate = cleaned
        .trim_start()
        .char_indices()
        .find(|(_, ch)| *ch == '{' || *ch == '[')
        .and_then(|(start_index, _)| {
            extract_balanced_json_from_start(cleaned.trim_start(), start_index)
        })
        .unwrap_or_else(|| cleaned.clone());
    let value: serde_json::Value = match serde_json::from_str(&candidate) {
        Ok(value) => value,
        Err(error) => {
            if let Some(payload) = parse_partial_rubric_questions(&candidate) {
                return Ok(payload);
            }
            return Err(app_error(
                AppErrorCode::RubricJsonParseFailed,
                "Rubrik JSON çıktısı çözülemedi.",
                Some(format!(
                    "JSON Parse Error: {}\nCandidate: {}\nRaw output received: {}",
                    error, candidate, raw
                )),
                Some("Model çıktısını strict JSON olarak tekrar isteyin.".to_string()),
            ));
        }
    };

    let questions_value = match value {
        serde_json::Value::Object(mut object) => object.remove("questions").ok_or_else(|| {
            app_error(
                AppErrorCode::RubricSchemaValidationFailed,
                "Rubrik JSON şeması beklenen alanları içermiyor.",
                Some("Missing `questions` field".to_string()),
                Some("Model output schema is incomplete.".to_string()),
            )
        })?,
        serde_json::Value::Array(array) => serde_json::Value::Array(array),
        other => {
            return Err(app_error(
                AppErrorCode::RubricSchemaValidationFailed,
                "Rubrik JSON kök nesnesi beklenen biçimde değil.",
                Some(format!("root={other}")),
                Some("Model output schema is invalid.".to_string()),
            ))
        }
    };

    let items = questions_value.as_array().ok_or_else(|| {
        app_error(
            AppErrorCode::RubricSchemaValidationFailed,
            "`questions` alanı bir liste olmalıdır.",
            Some(format!("questions={questions_value}")),
            Some("Model output schema is invalid.".to_string()),
        )
    })?;

    let mut questions = Vec::new();
    for item in items {
        questions.push(parse_rubric_question_item(item, &[])?);
    }

    if questions.is_empty() {
        return Err(app_error(
            AppErrorCode::RubricSchemaValidationFailed,
            "Rubrik JSON çıktısında geçerli soru bulunamadı.",
            Some(candidate),
            Some("Model output schema is invalid.".to_string()),
        ));
    }

    Ok(crate::domain::model::RubricImportPayload { questions })
}

struct StudentAnswerOcrParseOutcome {
    output: crate::domain::model::StudentAnswerOcrOutput,
    parsed_json: Option<serde_json::Value>,
    parse_error: Option<String>,
    salvaged_answer_text: Option<String>,
    parse_strategy: String,
    printed_text_mixed: bool,
    printed_question_leak_detected: bool,
}

struct StudentAnswerOcrIssueCorrectionParseOutcome {
    output: StudentAnswerOcrIssueCorrectionOutput,
    parsed_json: Option<serde_json::Value>,
    parse_error: Option<String>,
}

struct StudentIdentityOcrParseOutcome {
    output: StudentIdentityOcrOutput,
    parsed_json: Option<serde_json::Value>,
    parse_error: Option<String>,
    parse_strategy: String,
}

struct ScoringParseOutcome {
    output: ScoringOutput,
    parsed_json: Option<serde_json::Value>,
    parse_error: Option<String>,
    salvaged_rationale: Option<String>,
    parse_strategy: String,
}

fn parse_student_identity_ocr_output(raw: &str) -> StudentIdentityOcrParseOutcome {
    let cleaned = strip_reasoning_and_fences(raw);
    let candidate = extract_first_balanced_json_candidate(&cleaned).unwrap_or(cleaned);
    let parsed = serde_json::from_str::<serde_json::Value>(&candidate);
    match parsed {
        Ok(value) => {
            let display_name = optional_string_field(&value, "displayName");
            let number = optional_string_field(&value, "number");
            let class_name = optional_string_field(&value, "className");
            let confidence = value
                .get("confidence")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0) as f32;
            let mut warnings = string_array_field(&value, "warnings");
            if display_name.is_none() && number.is_none() {
                warnings.push("identity_name_or_number_missing".to_string());
            }
            let needs_review = value
                .get("needsReview")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
                || display_name.is_none() && number.is_none();
            StudentIdentityOcrParseOutcome {
                output: StudentIdentityOcrOutput {
                    display_name,
                    number,
                    class_name,
                    confidence,
                    needs_review,
                    warnings,
                },
                parsed_json: Some(value),
                parse_error: None,
                parse_strategy: "json".to_string(),
            }
        }
        Err(error) => StudentIdentityOcrParseOutcome {
            output: StudentIdentityOcrOutput {
                display_name: None,
                number: None,
                class_name: None,
                confidence: 0.0,
                needs_review: true,
                warnings: vec!["identity_ocr_json_parse_failed".to_string()],
            },
            parsed_json: None,
            parse_error: Some(error.to_string()),
            parse_strategy: "parse_failed".to_string(),
        },
    }
}

fn optional_string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn string_array_field(value: &serde_json::Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_ocr_warning_code(code: &str) -> String {
    match code {
        "ocr_critical_keyword_uncertain" | "critical_keyword_ocr_uncertain" => {
            CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string()
        }
        other => other.to_string(),
    }
}

fn uncertain_spans_field(value: &serde_json::Value) -> Vec<OcrUncertainSpan> {
    value
        .get("uncertainSpans")
        .or_else(|| value.get("uncertain_spans"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    let text = optional_string_field(item, "text")
                        .or_else(|| optional_string_field(item, "value"))
                        .unwrap_or_default();
                    if text.trim().is_empty() {
                        return None;
                    }
                    let alternatives = object
                        .get("alternatives")
                        .or_else(|| object.get("alternativeTexts"))
                        .or_else(|| object.get("alternative_texts"))
                        .and_then(|value| value.as_array())
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(|value| value.as_str().map(str::trim))
                                .filter(|value| !value.is_empty())
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let confidence = object
                        .get("confidence")
                        .and_then(|value| value.as_f64())
                        .map(|value| value.clamp(0.0, 1.0) as f32);
                    let reason = optional_string_field(item, "reason")
                        .or_else(|| optional_string_field(item, "explanation"))
                        .unwrap_or_else(|| "critical_term_uncertain".to_string());
                    let _warning_code = object
                        .get("warningCode")
                        .or_else(|| object.get("warning_code"))
                        .and_then(|value| value.as_str())
                        .map(normalize_ocr_warning_code)
                        .unwrap_or_else(|| CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string());
                    Some(OcrUncertainSpan {
                        text,
                        start: object
                            .get("start")
                            .or_else(|| object.get("startIndex"))
                            .or_else(|| object.get("start_index"))
                            .and_then(|value| value.as_u64())
                            .map(|value| value as usize),
                        end: object
                            .get("end")
                            .or_else(|| object.get("endIndex"))
                            .or_else(|| object.get("end_index"))
                            .and_then(|value| value.as_u64())
                            .map(|value| value as usize),
                        alternatives,
                        confidence,
                        reason,
                        highlight_region: highlight_region_field(object),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn suggested_corrections_field(value: &serde_json::Value) -> Vec<OcrSuggestedCorrection> {
    value
        .get("suggestedCorrections")
        .or_else(|| value.get("suggested_corrections"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    let original_text = optional_string_field(item, "originalText")
                        .or_else(|| optional_string_field(item, "original_text"))
                        .unwrap_or_default();
                    let suggested_text = optional_string_field(item, "suggestedText")
                        .or_else(|| optional_string_field(item, "suggested_text"))
                        .unwrap_or_default();
                    if original_text.trim().is_empty() || suggested_text.trim().is_empty() {
                        return None;
                    }
                    let reason = optional_string_field(item, "reason")
                        .or_else(|| optional_string_field(item, "explanation"))
                        .unwrap_or_else(|| "critical_term_suggestion".to_string());
                    let confidence = object
                        .get("confidence")
                        .and_then(|value| value.as_f64())
                        .map(|value| value.clamp(0.0, 1.0) as f32);
                    let applied = object
                        .get("applied")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false);
                    Some(OcrSuggestedCorrection {
                        original_text,
                        suggested_text,
                        reason,
                        confidence,
                        applied,
                        highlight_region: highlight_region_field(object),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn critical_term_warnings_field(value: &serde_json::Value) -> Vec<OcrCriticalTermWarning> {
    value
        .get("criticalTermWarnings")
        .or_else(|| value.get("critical_term_warnings"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    let observed_text = optional_string_field(item, "observedText")
                        .or_else(|| optional_string_field(item, "observed_text"))
                        .unwrap_or_default();
                    let expected_or_related_term =
                        optional_string_field(item, "expectedOrRelatedTerm")
                            .or_else(|| optional_string_field(item, "expected_or_related_term"))
                            .or_else(|| optional_string_field(item, "expectedTerm"))
                            .unwrap_or_default();
                    if observed_text.trim().is_empty() || expected_or_related_term.trim().is_empty()
                    {
                        return None;
                    }
                    let reason = optional_string_field(item, "reason")
                        .or_else(|| optional_string_field(item, "explanation"))
                        .unwrap_or_else(|| "critical_term_uncertain".to_string());
                    let warning_code = optional_string_field(item, "warningCode")
                        .or_else(|| optional_string_field(item, "warning_code"))
                        .map(|value| normalize_ocr_warning_code(&value))
                        .unwrap_or_else(|| CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string());
                    Some(OcrCriticalTermWarning {
                        observed_text,
                        expected_or_related_term,
                        reason,
                        warning_code,
                        highlight_region: highlight_region_field(object),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn highlight_region_field(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<crate::domain::student::StudentAnswerOcrCropBBox> {
    let region = object
        .get("highlightRegion")
        .or_else(|| object.get("highlight_region"))
        .or_else(|| object.get("bbox"))
        .or_else(|| object.get("normalizedBBox"))
        .or_else(|| object.get("normalized_bbox"))?;
    let region_object = region.as_object()?;
    let x = region_object.get("x")?.as_f64()?.clamp(0.0, 1.0) as f32;
    let y = region_object.get("y")?.as_f64()?.clamp(0.0, 1.0) as f32;
    let width = region_object.get("width")?.as_f64()?.clamp(0.0, 1.0) as f32;
    let height = region_object.get("height")?.as_f64()?.clamp(0.0, 1.0) as f32;
    let page_index = region_object
        .get("pageIndex")
        .or_else(|| region_object.get("pageIndexWithinSubmission"))
        .or_else(|| region_object.get("page_index_within_submission"))
        .or_else(|| region_object.get("page_index"))
        .and_then(|value| value.as_u64())? as u32;

    Some(crate::domain::student::StudentAnswerOcrCropBBox {
        x,
        y,
        width,
        height,
        page_index,
    })
}

#[cfg(test)]
fn parse_student_answer_ocr_output(raw: &str, question_text: &str) -> StudentAnswerOcrParseOutcome {
    let policy = default_ocr_review_policy();
    parse_student_answer_ocr_output_with_policy(raw, question_text, "general_text", &policy)
}

fn parse_student_answer_ocr_output_with_policy(
    raw: &str,
    question_text: &str,
    answer_type: &str,
    policy: &crate::domain::student::OcrReviewPolicy,
) -> StudentAnswerOcrParseOutcome {
    let raw_text = raw.trim().to_string();
    let cleaned = strip_reasoning_and_fences(raw);
    let fenced_candidate = extract_fenced_json_candidate(raw);
    let balanced_candidate = extract_first_balanced_json_candidate(raw);
    let attempts = [
        ("strict_json", raw_text.as_str()),
        ("fenced_json", fenced_candidate.as_deref().unwrap_or("")),
        ("trailing_prose_trim", cleaned.as_str()),
        ("balanced_json", balanced_candidate.as_deref().unwrap_or("")),
    ];

    for (strategy, candidate) in attempts {
        if candidate.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
            if let Some(output) = parse_student_answer_ocr_value(
                value.clone(),
                candidate,
                strategy,
                None,
                question_text,
                raw,
                answer_type,
                policy,
            ) {
                return output;
            }
        }
    }

    let salvaged_answer_text = salvage_student_answer_text(&cleaned, raw);
    let printed_question_leak_detected =
        detect_printed_question_leak(&salvaged_answer_text, question_text);
    let mut warnings = vec!["ocr_parse_failed".to_string()];
    if printed_question_leak_detected {
        warnings.push("printed_question_leak_detected".to_string());
    }
    StudentAnswerOcrParseOutcome {
        output: crate::domain::model::StudentAnswerOcrOutput {
            answer_text: salvaged_answer_text.clone(),
            structured_answer: None,
            confidence: 0.0,
            uncertain_spans: vec![],
            suggested_corrections: vec![],
            critical_term_warnings: vec![],
            ocr_semantic_warnings: vec!["ocr_parse_failed".to_string()],
            critical_keyword_uncertain: true,
            needs_review: true,
            review_reasons: vec!["parse_failed".to_string()],
            warnings,
            review_policy: Some(policy.clone()),
        },
        parsed_json: None,
        parse_error: Some("Öğrenci OCR JSON çıktısı çözülemedi.".to_string()),
        salvaged_answer_text: Some(salvaged_answer_text),
        parse_strategy: "raw_text_salvage".to_string(),
        printed_text_mixed: printed_question_leak_detected,
        printed_question_leak_detected,
    }
}

fn extract_fenced_json_candidate(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (start_idx, fence_len) = if let Some(start_idx) = trimmed.find("```json") {
        (start_idx, "```json".len())
    } else if let Some(start_idx) = trimmed.find("```") {
        (start_idx, "```".len())
    } else {
        return None;
    };
    let content_after = &trimmed[start_idx + fence_len..];
    let end_idx = content_after.find("```")?;
    Some(content_after[..end_idx].trim().to_string())
}

#[allow(clippy::too_many_arguments)]
fn parse_student_answer_ocr_value(
    value: serde_json::Value,
    candidate: &str,
    strategy: &str,
    parse_error: Option<String>,
    question_text: &str,
    raw: &str,
    answer_type: &str,
    policy: &crate::domain::student::OcrReviewPolicy,
) -> Option<StudentAnswerOcrParseOutcome> {
    let object = value.as_object()?;
    let has_answer_field = object.contains_key("answerText")
        || object.contains_key("answer_text")
        || object.contains_key("text")
        || object.contains_key("answer");
    let has_confidence_field = object.contains_key("confidence");
    let has_review_field =
        object.contains_key("needsReview") || object.contains_key("needs_review");

    let answer_text = object
        .get("answerText")
        .or_else(|| object.get("answer_text"))
        .or_else(|| object.get("text"))
        .or_else(|| object.get("answer"))
        .and_then(|value| value.as_str())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| salvage_student_answer_text(candidate, raw));
    let structured_value = object
        .get("structuredAnswer")
        .or_else(|| object.get("structured_answer"))
        .cloned()
        .and_then(|value| if value.is_null() { None } else { Some(value) });
    let (structured_answer, structured_answer_error) = match structured_value {
        Some(value) => match answer_type_from_label(answer_type) {
            None => (
                Some(
                    crate::domain::structured_answer::StructuredAnswer::LegacyUnparsed {
                        raw: value,
                        reason: "structured_answer_unknown_answer_type".to_string(),
                    },
                ),
                Some("structured_answer_unknown_answer_type".to_string()),
            ),
            Some(answer_type) => {
                match crate::domain::structured_answer::parse_for_answer_type(
                    &answer_type,
                    value.clone(),
                ) {
                    Ok(answer) => (Some(answer), None),
                    Err(error) => {
                        let review_answer =
                            crate::domain::structured_answer::parse_legacy_for_review(
                                value.clone(),
                            )
                            .unwrap_or_else(|parse_error| {
                                crate::domain::structured_answer::StructuredAnswer::LegacyUnparsed {
                                    raw: value,
                                    reason: parse_error.message,
                                }
                            });
                        (Some(review_answer), Some(error.message))
                    }
                }
            }
        },
        None => (None, None),
    };
    let confidence = object
        .get("confidence")
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let mut needs_review = object
        .get("needsReview")
        .or_else(|| object.get("needs_review"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let mut review_reasons = object
        .get("reviewReasons")
        .or_else(|| object.get("review_reasons"))
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(|text| text.trim().to_string()))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut warnings = object
        .get("warnings")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(|text| text.trim().to_string()))
                .filter(|text| !text.is_empty())
                .map(|text| normalize_ocr_warning_code(&text))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let uncertain_spans = uncertain_spans_field(&value);
    let suggested_corrections = suggested_corrections_field(&value);
    let critical_term_warnings = critical_term_warnings_field(&value);
    let mut ocr_semantic_warnings = string_array_field(&value, "ocrSemanticWarnings")
        .into_iter()
        .map(|warning| normalize_ocr_warning_code(&warning))
        .collect::<Vec<_>>();
    if ocr_semantic_warnings.is_empty() {
        ocr_semantic_warnings = string_array_field(&value, "ocr_semantic_warnings")
            .into_iter()
            .map(|warning| normalize_ocr_warning_code(&warning))
            .collect();
    }
    let critical_keyword_uncertain = object
        .get("criticalKeywordUncertain")
        .or_else(|| object.get("critical_keyword_uncertain"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || !uncertain_spans.is_empty()
        || !suggested_corrections.is_empty()
        || !critical_term_warnings.is_empty()
        || !ocr_semantic_warnings.is_empty();

    let scoring_fields_present = object.contains_key("score")
        || object.contains_key("points")
        || object.contains_key("criteria")
        || object.contains_key("awardedScore")
        || object.contains_key("criterionScores");
    if scoring_fields_present {
        warnings.push("ocr_scoring_fields_ignored".to_string());
        review_reasons.push("ocr_scoring_fields_present".to_string());
        needs_review = true;
    }
    if !has_answer_field || !has_confidence_field || !has_review_field {
        warnings.push("ocr_schema_incomplete".to_string());
        review_reasons.push("ocr_schema_incomplete".to_string());
        needs_review = true;
    }
    if structured_answer_error.is_some() {
        warnings.push("structured_answer_invalid".to_string());
        review_reasons.push("structured_answer_invalid".to_string());
        needs_review = true;
    }

    let printed_question_leak_detected = detect_printed_question_leak(&answer_text, question_text);
    let printed_text_mixed = printed_question_leak_detected || answer_text.trim().is_empty();
    if printed_question_leak_detected {
        review_reasons.push("printed_question_leak_detected".to_string());
        warnings.push("printed_question_leak_detected".to_string());
        needs_review = true;
    }
    if answer_text.trim().is_empty() {
        review_reasons.push("ocr_answer_empty".to_string());
        warnings.push("ocr_answer_empty".to_string());
        needs_review = true;
    }
    if normalize_for_similarity(&answer_text).contains("okunamadi") {
        review_reasons.push("ocr_unreadable_span".to_string());
        warnings.push("ocr_unreadable_span".to_string());
        needs_review = true;
    }
    if contains_ocr_commentary(&answer_text) {
        review_reasons.push("ocr_commentary_detected".to_string());
        warnings.push("ocr_commentary_detected".to_string());
        needs_review = true;
    }
    if critical_keyword_uncertain {
        review_reasons.push("critical_keyword_uncertain".to_string());
        warnings.push(CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string());
        if ocr_semantic_warnings.is_empty() {
            ocr_semantic_warnings.push(CRITICAL_KEYWORD_OCR_UNCERTAIN_WARNING.to_string());
        }
        needs_review = true;
    }
    if parse_error.is_some() {
        review_reasons.push("parse_failed".to_string());
        warnings.push("ocr_parse_failed".to_string());
        needs_review = true;
    }
    if policy.should_review_confidence(confidence) {
        review_reasons.push("ocr_low_confidence".to_string());
        warnings.push("ocr_low_confidence".to_string());
        needs_review = true;
    }

    review_reasons.sort();
    review_reasons.dedup();
    warnings.sort();
    warnings.dedup();

    Some(StudentAnswerOcrParseOutcome {
        output: crate::domain::model::StudentAnswerOcrOutput {
            answer_text,
            structured_answer,
            confidence,
            uncertain_spans,
            suggested_corrections,
            critical_term_warnings,
            ocr_semantic_warnings,
            critical_keyword_uncertain,
            needs_review,
            review_reasons,
            warnings,
            review_policy: Some(policy.clone()),
        },
        parsed_json: Some(value),
        parse_error,
        salvaged_answer_text: Some(salvage_student_answer_text(candidate, raw)),
        parse_strategy: strategy.to_string(),
        printed_text_mixed,
        printed_question_leak_detected,
    })
}

fn answer_type_from_label(label: &str) -> Option<AnswerType> {
    Some(match label.trim().to_ascii_lowercase().as_str() {
        "general_text" => AnswerType::GeneralText,
        "short_text" => AnswerType::ShortText,
        "essay" => AnswerType::Essay,
        "table" => AnswerType::Table,
        "correction_table" => AnswerType::CorrectionTable,
        "fill_blank" => AnswerType::FillBlank,
        "matching" => AnswerType::Matching,
        "multiple_choice" => AnswerType::MultipleChoice,
        "true_false" => AnswerType::TrueFalse,
        "ordering" => AnswerType::Ordering,
        "numeric" => AnswerType::Numeric,
        "diagram_labeling" => AnswerType::DiagramLabeling,
        "sentence_annotation" => AnswerType::SentenceAnnotation,
        "grammar_analysis" => AnswerType::GrammarAnalysis,
        _ => return None,
    })
}

fn contains_ocr_commentary(answer_text: &str) -> bool {
    let normalized = normalize_for_similarity(answer_text);
    [
        "ogrenci burada",
        "ogrenci sunu",
        "ogrencinin cevabi",
        "cevap su sekildedir",
        "cevap sudur",
        "gorselde ogrencinin",
        "buradan anlasilmaktadir",
        "demek istemistir",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
fn parse_student_answer_issue_correction_output(
    raw: &str,
) -> Result<StudentAnswerOcrIssueCorrectionParseOutcome, AppError> {
    let policy = default_ocr_review_policy();
    parse_student_answer_issue_correction_output_with_policy(raw, &policy)
}

fn parse_student_answer_issue_correction_output_with_policy(
    raw: &str,
    policy: &crate::domain::student::OcrReviewPolicy,
) -> Result<StudentAnswerOcrIssueCorrectionParseOutcome, AppError> {
    let cleaned = strip_reasoning_and_fences(raw);
    let candidate = extract_first_balanced_json_candidate(&cleaned).unwrap_or(cleaned.clone());
    let parsed_json: serde_json::Value = serde_json::from_str(&candidate).map_err(|error| {
        app_error(
            AppErrorCode::ModelResponseInvalidJson,
            "OCR sorun önerisi JSON çıktısı çözülemedi.",
            Some(format!(
                "JSON Parse Error: {}\nRaw output received: {}",
                error, raw
            )),
            Some("Prompt çıktısının strict JSON olduğundan emin olun.".to_string()),
        )
    })?;

    let mut output: StudentAnswerOcrIssueCorrectionOutput =
        serde_json::from_value(parsed_json.clone()).map_err(|error| {
            app_error(
                AppErrorCode::ModelResponseInvalidSchema,
                "OCR sorun önerisi JSON şeması beklenen biçimde değil.",
                Some(format!(
                    "Schema error: {}\nRaw value: {}",
                    error, parsed_json
                )),
                Some("Model output schema is invalid.".to_string()),
            )
        })?;

    output.original_text = output.original_text.trim().to_string();
    output.context_reason = output.context_reason.trim().to_string();
    output.visual_reading = output
        .visual_reading
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    output.suggested_text = output
        .suggested_text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    output.warnings.sort();
    output.warnings.dedup();

    if output.original_text.is_empty() {
        return Err(app_error(
            AppErrorCode::ModelResponseInvalidSchema,
            "`originalText` alanı boş olamaz.",
            Some(parsed_json.to_string()),
            Some("Model output schema is invalid.".to_string()),
        ));
    }
    if output.context_reason.is_empty() {
        return Err(app_error(
            AppErrorCode::ModelResponseInvalidSchema,
            "`contextReason` alanı boş olamaz.",
            Some(parsed_json.to_string()),
            Some("Model output schema is invalid.".to_string()),
        ));
    }

    output.requires_teacher_approval = true;

    let observed_scope = issue_scope_from_text(&output.original_text);
    if !scope_allows_text(&observed_scope, output.suggested_text.as_deref()) {
        output.warnings.push("scope_expansion_blocked".to_string());
        output.decision = StudentAnswerOcrIssueCorrectionDecision::NeedsTeacherReview;
        output.suggested_text = None;
    }

    if matches!(
        output.decision,
        StudentAnswerOcrIssueCorrectionDecision::NoChange
    ) {
        output.suggested_text = None;
    }

    if matches!(
        output.decision,
        StudentAnswerOcrIssueCorrectionDecision::SuggestCorrection
    ) && output
        .suggested_text
        .as_ref()
        .map(|text| text.trim().is_empty())
        .unwrap_or(true)
    {
        output.warnings.push("issue_context_missing".to_string());
        output.decision = StudentAnswerOcrIssueCorrectionDecision::NeedsTeacherReview;
    }

    if output.confidence < policy.critical_confidence_threshold {
        output
            .warnings
            .push("suggestion_confidence_low".to_string());
    }
    output.warnings.sort();
    output.warnings.dedup();

    Ok(StudentAnswerOcrIssueCorrectionParseOutcome {
        output,
        parsed_json: Some(parsed_json),
        parse_error: None,
    })
}

fn salvage_student_answer_text(candidate: &str, raw: &str) -> String {
    let cleaned = strip_reasoning_and_fences(candidate);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return raw.trim().to_string();
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(text) = value
            .get("answerText")
            .or_else(|| value.get("answer_text"))
            .or_else(|| value.get("text"))
            .or_else(|| value.get("answer"))
            .and_then(|value| value.as_str())
        {
            return text.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn detect_printed_question_leak(answer_text: &str, question_text: &str) -> bool {
    let answer = normalize_for_similarity(answer_text);
    let question = normalize_for_similarity(question_text);
    if answer.is_empty() || question.is_empty() {
        return false;
    }
    if answer == question {
        return true;
    }
    if answer.contains(&question) || question.contains(&answer) {
        return true;
    }
    let answer_tokens = token_set(&answer);
    let question_tokens = token_set(&question);
    if answer_tokens.is_empty() || question_tokens.is_empty() {
        return false;
    }
    let common = answer_tokens.intersection(&question_tokens).count() as f32;
    common / question_tokens.len().max(answer_tokens.len()) as f32 >= 0.75
}

fn normalize_for_similarity(text: &str) -> String {
    crate::services::text_normalization::normalize_for_comparison(text)
}

fn token_set(text: &str) -> std::collections::BTreeSet<String> {
    text.split_whitespace()
        .map(|token| token.to_string())
        .collect()
}

fn issue_scope_from_text(text: &str) -> StudentAnswerOcrIssueCorrectionScope {
    if text.split_whitespace().count() <= 1 {
        StudentAnswerOcrIssueCorrectionScope::SingleWord
    } else {
        StudentAnswerOcrIssueCorrectionScope::ShortPhrase
    }
}

fn scope_allows_text(
    scope: &StudentAnswerOcrIssueCorrectionScope,
    suggested_text: Option<&str>,
) -> bool {
    let Some(text) = suggested_text else {
        return true;
    };
    let token_count = text
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .count();
    match scope {
        StudentAnswerOcrIssueCorrectionScope::SingleWord => token_count <= 1,
        StudentAnswerOcrIssueCorrectionScope::ShortPhrase => token_count <= 2,
    }
}

fn parse_rubric_question_item(
    item: &serde_json::Value,
    extra_warnings: &[&str],
) -> Result<crate::domain::model::RubricImportQuestion, AppError> {
    let Some(object) = item.as_object() else {
        return Err(app_error(
            AppErrorCode::RubricSchemaValidationFailed,
            "Rubrik soru girdisi nesne olmalıdır.",
            Some(format!("item={item}")),
            Some("Model output schema is invalid.".to_string()),
        ));
    };

    let mut warnings = extra_warnings
        .iter()
        .map(|warning| warning.to_string())
        .collect::<Vec<_>>();
    let number_value = object
        .get("questionNumber")
        .or_else(|| object.get("question_number"))
        .or_else(|| object.get("question_no"))
        .or_else(|| object.get("questionNo"))
        .or_else(|| object.get("number"))
        .or_else(|| object.get("soru_no"));
    if object.contains_key("question_no") {
        warnings.push(normalize_alias_warning("questionNumber", "question_no"));
    }
    if object.contains_key("question_number") {
        warnings.push(normalize_alias_warning("questionNumber", "question_number"));
    }
    if object.contains_key("soru_no") {
        warnings.push(normalize_alias_warning("questionNumber", "soru_no"));
    }
    let question_number = number_value
        .and_then(|value| value.as_u64())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            app_error(
                AppErrorCode::RubricSchemaValidationFailed,
                "`questionNumber` alanı eksik veya geçersiz.",
                Some(format!("item={item}")),
                Some("Model output schema is invalid.".to_string()),
            )
        })? as u32;

    let max_points_value = object
        .get("maxScore")
        .or_else(|| object.get("maxPoints"))
        .or_else(|| object.get("max_points"))
        .or_else(|| object.get("maxPoint"))
        .or_else(|| object.get("max_score"))
        .or_else(|| object.get("puan"))
        .or_else(|| object.get("points"))
        .or_else(|| object.get("score"));
    if object.contains_key("max_points") {
        warnings.push(normalize_alias_warning("maxPoints", "max_points"));
    }
    if object.contains_key("maxPoint") {
        warnings.push(normalize_alias_warning("maxPoints", "maxPoint"));
    }
    if object.contains_key("max_score") {
        warnings.push(normalize_alias_warning("maxPoints", "max_score"));
    }
    if object.contains_key("puan") {
        warnings.push(normalize_alias_warning("maxPoints", "puan"));
    }
    if object.contains_key("points") {
        warnings.push(normalize_alias_warning("maxPoints", "points"));
    }
    if object.contains_key("score") {
        warnings.push(normalize_alias_warning("maxPoints", "score"));
    }
    let max_points = max_points_value
        .and_then(|value| value.as_f64())
        .map(|value| value as f32);

    let expected_answer = object
        .get("expectedAnswer")
        .or_else(|| object.get("expected_answer"))
        .or_else(|| object.get("answer"))
        .or_else(|| object.get("beklenen_cevap"))
        .or_else(|| object.get("model_answer"))
        .or_else(|| object.get("expected"))
        .and_then(|value| value.as_str())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    if object.contains_key("expected_answer") {
        warnings.push(normalize_alias_warning("expectedAnswer", "expected_answer"));
    }
    if object.contains_key("answer") {
        warnings.push(normalize_alias_warning("expectedAnswer", "answer"));
    }
    if object.contains_key("beklenen_cevap") {
        warnings.push(normalize_alias_warning("expectedAnswer", "beklenen_cevap"));
    }
    if object.contains_key("model_answer") {
        warnings.push(normalize_alias_warning("expectedAnswer", "model_answer"));
    }

    let key_concepts = rubric_string_array(object, &["keyConcepts", "key_concepts", "keywords"]);
    let partial_credit_hints = rubric_string_array(
        object,
        &[
            "partialCreditHints",
            "partial_credit_hints",
            "partialCreditNotes",
        ],
    );
    let zero_score_conditions =
        rubric_string_array(object, &["zeroScoreConditions", "zero_score_conditions"]);
    let common_mistakes = rubric_string_array(object, &["commonMistakes", "common_mistakes"]);
    if object.contains_key("rubric") {
        warnings.push(normalize_alias_warning("criteria", "rubric"));
    }
    if object.contains_key("scoring_criteria") {
        warnings.push(normalize_alias_warning("criteria", "scoring_criteria"));
    }
    if object.contains_key("kriterler") {
        warnings.push(normalize_alias_warning("criteria", "kriterler"));
    }

    let criteria_value = object
        .get("criteria")
        .or_else(|| object.get("rubric"))
        .or_else(|| object.get("scoring_criteria"))
        .or_else(|| object.get("kriterler"))
        .or_else(|| object.get("criteria_list"));
    let (criteria, criterion_warnings) = if let Some(value) = criteria_value {
        normalize_rubric_criteria(value)
    } else {
        (vec![], vec!["criteria_missing".to_string()])
    };
    warnings.extend(criterion_warnings);
    warnings.extend(
        object
            .get("warnings")
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|warning| warning.as_str().map(|text| text.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );

    let has_meaningful_content = max_points.is_some_and(|points| points > 0.0)
        || expected_answer
            .as_ref()
            .is_some_and(|text| !text.trim().is_empty())
        || !key_concepts.is_empty()
        || !criteria.is_empty();
    if !has_meaningful_content {
        warnings.push("rubric_empty_content".to_string());
    }

    Ok(crate::domain::model::RubricImportQuestion {
        question_number,
        max_points,
        expected_answer,
        key_concepts,
        criteria: criteria
            .into_iter()
            .map(|criterion| crate::domain::model::RubricImportCriterion {
                label: criterion.label,
                points: criterion.points,
                description: criterion.description,
            })
            .collect(),
        partial_credit_hints,
        zero_score_conditions,
        common_mistakes,
        warnings,
    })
}

fn rubric_string_array(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Vec<String> {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(|value| value.as_array()))
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_partial_rubric_questions(raw: &str) -> Option<crate::domain::model::RubricImportPayload> {
    let questions_key = raw.find("\"questions\"")?;
    let array_start = raw[questions_key..].find('[')? + questions_key;
    let mut questions = Vec::new();
    let mut object_start = None;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for (relative_index, ch) in raw[array_start + 1..].char_indices() {
        let index = array_start + 1 + relative_index;
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    object_start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = object_start.take() {
                        if let Ok(value) =
                            serde_json::from_str::<serde_json::Value>(&raw[start..=index])
                        {
                            if let Ok(question) = parse_rubric_question_item(
                                &value,
                                &[
                                    "partial_model_json_recovered",
                                    "model_response_truncated_or_incomplete_json",
                                ],
                            ) {
                                questions.push(question);
                            }
                        }
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    if questions.is_empty() {
        None
    } else {
        Some(crate::domain::model::RubricImportPayload { questions })
    }
}

fn rubric_payload_to_output(
    payload: crate::domain::model::RubricImportPayload,
) -> RubricExtractionOutput {
    let questions = payload
        .questions
        .into_iter()
        .map(|question| {
            let criteria = question
                .criteria
                .into_iter()
                .map(|criterion| crate::domain::rubric::RubricCriterion {
                    id: uuid::Uuid::new_v4().to_string(),
                    label: criterion.label,
                    description: criterion.description,
                    points: criterion.points,
                    levels: vec![],
                })
                .collect();

            ExtractedRubricCandidate {
                number: question.question_number,
                max_points: question.max_points,
                expected_answer: question.expected_answer,
                key_concepts: question.key_concepts,
                criteria,
                partial_credit_hints: question.partial_credit_hints,
                zero_score_conditions: question.zero_score_conditions,
                common_mistakes: question.common_mistakes,
                confidence: 1.0,
                warnings: question.warnings,
            }
        })
        .collect();

    RubricExtractionOutput {
        questions,
        document_warnings: vec![],
    }
}

fn remove_think_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("<think>") {
        result.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<think>".len()..];
        if let Some(end) = after_start.find("</think>") {
            remaining = &after_start[end + "</think>".len()..];
        } else {
            remaining = "";
            break;
        }
    }
    result.push_str(remaining);
    result
}

fn parse_question_text_output(text: &str) -> Result<QuestionTextExtractionOutput, AppError> {
    let cleaned = strip_reasoning_and_fences(text);
    let candidate =
        extract_first_balanced_json_candidate(&cleaned).unwrap_or_else(|| cleaned.clone());
    let value: serde_json::Value = serde_json::from_str(&candidate).map_err(|error| {
        app_error(
            AppErrorCode::ModelResponseInvalidJson,
            "Soru metni JSON çıktısı çözülemedi.",
            Some(format!(
                "JSON Parse Error: {}\nRaw output received: {}",
                error, text
            )),
            Some("Prompt çıktısının strict JSON olduğundan emin olun.".to_string()),
        )
    })?;

    let questions_value = match &value {
        serde_json::Value::Object(object) => object.get("questions").ok_or_else(|| {
            app_error(
                AppErrorCode::ModelResponseInvalidSchema,
                "Soru metni JSON şeması beklenen alanları içermiyor.",
                Some("Missing `questions` field".to_string()),
                Some("Model output schema is incomplete.".to_string()),
            )
        })?,
        serde_json::Value::Array(_) => &value,
        other => {
            return Err(app_error(
                AppErrorCode::ModelResponseInvalidSchema,
                "Soru metni JSON kök nesnesi beklenen biçimde değil.",
                Some(format!("root={other}")),
                Some("Model output schema is invalid.".to_string()),
            ))
        }
    };
    let questions_array = questions_value.as_array().ok_or_else(|| {
        app_error(
            AppErrorCode::ModelResponseInvalidSchema,
            "`questions` alanı bir liste olmalıdır.",
            Some(format!("questions={questions_value}")),
            Some("Model output schema is invalid.".to_string()),
        )
    })?;

    let mut questions = Vec::new();
    for item in questions_array {
        let item_object = item.as_object().ok_or_else(|| {
            app_error(
                AppErrorCode::ModelResponseInvalidSchema,
                "Soru girdisi nesne olmalıdır.",
                Some(format!("item={item}")),
                Some("Model output schema is invalid.".to_string()),
            )
        })?;

        let number = item_object
            .get("number")
            .or_else(|| item_object.get("questionNumber"))
            .or_else(|| item_object.get("question_number"))
            .or_else(|| item_object.get("soru_no"))
            .and_then(|value| value.as_u64())
            .ok_or_else(|| {
                app_error(
                    AppErrorCode::ModelResponseInvalidSchema,
                    "`number` alanı eksik veya geçersiz.",
                    Some(format!("item={item}")),
                    Some("Model output schema is invalid.".to_string()),
                )
            })? as u32;
        let question_text = item_object
            .get("question_text")
            .or_else(|| item_object.get("questionText"))
            .or_else(|| item_object.get("text"))
            .or_else(|| item_object.get("soru"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                app_error(
                    AppErrorCode::ModelResponseInvalidSchema,
                    "`question_text` alanı eksik veya geçersiz.",
                    Some(format!("item={item}")),
                    Some("Model output schema is invalid.".to_string()),
                )
            })?;
        if number == 0 || question_text.trim().is_empty() {
            return Err(app_error(
                AppErrorCode::ModelResponseInvalidSchema,
                "Soru girdisi boş veya geçersiz.",
                Some(format!("item={item}")),
                Some("Model output schema is invalid.".to_string()),
            ));
        }

        let confidence = item_object
            .get("confidence")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.5) as f32;
        let warnings = item_object
            .get("warnings")
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|warning| warning.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        questions.push(ExtractedQuestionCandidate {
            number,
            question_text: question_text.to_string(),
            confidence,
            warnings,
        });
    }

    if questions.is_empty() {
        return Err(app_error(
            AppErrorCode::ModelResponseInvalidSchema,
            "Soru metni çıktısında geçerli soru bulunamadı.",
            Some(text.to_string()),
            Some("Model output schema is invalid.".to_string()),
        ));
    }

    let page_warnings = value
        .as_object()
        .and_then(|object| object.get("page_warnings"))
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|warning| warning.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(QuestionTextExtractionOutput {
        questions,
        page_warnings,
    })
}

fn map_transport_error(error: reqwest::Error, url: &str) -> AppError {
    let err_str = error.to_string();
    let code = if error.is_timeout() {
        AppErrorCode::ModelTimeout
    } else if error.is_connect() {
        AppErrorCode::ModelServerNotRunning
    } else if err_str.contains("connection reset") || err_str.contains("Connection reset") {
        AppErrorCode::ModelConnectionReset
    } else {
        AppErrorCode::ModelHealthFailed
    };

    app_error(
        code,
        "Model sunucusuna erişilemedi.",
        Some(format!("{url}: {error}")),
        Some("llama-server durumunu kontrol edin.".to_string()),
    )
}

/// Reads a model HTTP response body with a streaming byte cap.
///
/// The full body is never accumulated past `max_bytes`. A chunked or
/// streamed response that exceeds the cap in total is rejected with
/// `ModelResponseTooLarge`; no partial body is parsed or committed.
async fn read_bounded_body(
    mut response: reqwest::Response,
    max_bytes: u64,
    first_byte_timeout: Duration,
    idle_chunk_timeout: Duration,
    url: &str,
) -> Result<String, AppError> {
    let mut body: Vec<u8> = Vec::new();
    let mut first = true;
    loop {
        let chunk_future = response.chunk();
        let chunk = if first {
            timeout(first_byte_timeout, chunk_future)
                .await
                .map_err(|_| {
                    app_error(
                        AppErrorCode::ModelTimeout,
                        "Model ilk yanıt baytını gönderemedi.",
                        Some(format!(
                            "endpoint={url} first_byte_timeout={:?}",
                            first_byte_timeout
                        )),
                        Some("Model server durumunu kontrol edip yeniden deneyin.".to_string()),
                    )
                })?
        } else {
            timeout(idle_chunk_timeout, chunk_future)
                .await
                .map_err(|_| {
                    app_error(
                        AppErrorCode::ModelTimeout,
                        "Model yanıtı gönderilirken durdu.",
                        Some(format!(
                            "endpoint={url} idle_chunk_timeout={:?}",
                            idle_chunk_timeout
                        )),
                        Some("Ağı veya model server durumunu kontrol edin.".to_string()),
                    )
                })?
        }
        .map_err(|error| map_transport_error(error, url))?;
        first = false;
        match chunk {
            Some(bytes) => {
                let new_len = body.len() as u64 + bytes.len() as u64;
                if new_len > max_bytes {
                    return Err(app_error(
                        AppErrorCode::ModelResponseTooLarge,
                        "Model yanıtı çok büyük.",
                        Some(format!("received_bytes={new_len} limit_bytes={max_bytes}")),
                        Some("Model çıktı ayarlarını küçültüp yeniden deneyin.".to_string()),
                    ));
                }
                body.extend_from_slice(&bytes);
            }
            None => break,
        }
    }
    String::from_utf8(body).map_err(|_| {
        app_error(
            AppErrorCode::ModelResponseInvalidJson,
            "Model yanıtı geçerli metin değil.",
            Some("model response is not valid UTF-8".to_string()),
            Some("Model çıktı biçimini kontrol edin.".to_string()),
        )
    })
}

fn build_payload_summary(
    prompt_length: u32,
    timeout_seconds: u64,
    model_input_images: Option<&[crate::domain::model::ModelInputImage]>,
    fallback_image_bytes: Option<u64>,
    max_tokens: Option<u32>,
) -> ModelRequestPayloadSummary {
    let images: Vec<crate::domain::model::ModelInputImage> =
        model_input_images.unwrap_or_default().to_vec();
    let image_total_bytes = if images.is_empty() {
        fallback_image_bytes.unwrap_or_default()
    } else {
        images.iter().map(|image| image.output_bytes).sum()
    };
    let base64_approx_total_bytes = if images.is_empty() {
        if let Some(bytes) = fallback_image_bytes {
            bytes.div_ceil(3) * 4
        } else {
            0
        }
    } else {
        images.iter().map(|image| image.base64_approx_bytes).sum()
    };
    let image_count = if images.is_empty() {
        if fallback_image_bytes.is_some() {
            1
        } else {
            0
        }
    } else {
        images.len() as u32
    };
    ModelRequestPayloadSummary {
        prompt_length,
        image_count,
        image_total_bytes,
        base64_approx_total_bytes,
        model_input_images: images,
        timeout_seconds,
        max_tokens,
    }
}

fn app_error(
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

fn json_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn json_number(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.trim().parse::<f64>().ok())
    })
}

fn object_text(object: &serde_json::Map<String, serde_json::Value>, names: &[&str]) -> String {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(json_text))
        .unwrap_or_default()
}

fn object_number(object: &serde_json::Map<String, serde_json::Value>, names: &[&str]) -> f32 {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(json_number))
        .unwrap_or(0.0) as f32
}

fn object_evidence(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    for name in ["evidenceQuote", "evidence_quote", "quote", "evidence"] {
        let Some(value) = object.get(name) else {
            continue;
        };
        if let Some(text) = json_text(value) {
            return Some(text);
        }
        if let Some(item) = value.as_object() {
            let text = object_text(item, &["quote", "text", "evidence"]);
            if !text.is_empty() {
                return Some(text);
            }
        }
        if let Some(items) = value.as_array() {
            if let Some(text) = items.iter().find_map(json_text) {
                return Some(text);
            }
            if let Some(text) = items.iter().find_map(|item| {
                item.as_object()
                    .map(|item| object_text(item, &["quote", "text", "evidence"]))
                    .filter(|text| !text.is_empty())
            }) {
                return Some(text);
            }
        }
    }
    None
}

fn parse_scoring_output(raw: &str, max_score: f32) -> ScoringParseOutcome {
    let cleaned = strip_reasoning_and_fences(raw);
    let candidate = extract_first_balanced_json_candidate(&cleaned).unwrap_or(cleaned.clone());
    match serde_json::from_str::<serde_json::Value>(&candidate) {
        Ok(value) => {
            let awarded_score = value
                .as_object()
                .map(|object| object_number(object, &["awardedScore", "awarded_score", "score"]))
                .unwrap_or(0.0);
            let direct_score_fields = direct_scoring_fields(&value);
            let criterion_decisions = parse_semantic_criterion_decisions(&value);
            let confidence = value
                .get("confidence")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0) as f32;
            let criterion_scores = value
                .get("criterionScores")
                .or_else(|| value.get("criteria"))
                .or_else(|| value.get("scoringCriteria"))
                .or_else(|| value.get("scores"))
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            let object = item.as_object()?;
                            let criterion_id =
                                object_text(object, &["criterionId", "criterion_id"]);
                            let criterion_title = object_text(
                                object,
                                &[
                                    "criterionTitle",
                                    "criterion_title",
                                    "title",
                                    "label",
                                    "criterion",
                                    "criterionName",
                                    "criterion_name",
                                    "name",
                                ],
                            );
                            let criterion_max_score = object_number(
                                object,
                                &[
                                    "criterionMaxScore",
                                    "criterion_max_score",
                                    "maxScore",
                                    "max_score",
                                    "points",
                                ],
                            );
                            let criterion_awarded_score = object_number(
                                object,
                                &["awardedScore", "awarded_score", "score", "points"],
                            );
                            let rationale = object_text(
                                object,
                                &["rationale", "reason", "explanation", "feedback"],
                            );
                            let evidence_quote = object_evidence(object);
                            Some(ScoringCriterionScore {
                                criterion_id,
                                criterion_title,
                                criterion_max_score,
                                awarded_score: criterion_awarded_score,
                                rationale,
                                evidence_quote,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let rationale = value
                .get("rationale")
                .or_else(|| value.get("feedback"))
                .or_else(|| value.get("explanation"))
                .or_else(|| value.get("reason"))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let warnings = string_array_field(&value, "warnings");
            let parse_strategy = if !criterion_decisions.is_empty() {
                "semantic_criterion_levels"
            } else if value.get("criterionScores").is_some() {
                "criterion_scores"
            } else if value.get("criteria").is_some() {
                "criteria"
            } else if value.get("scores").is_some() {
                "scores"
            } else {
                "fallback"
            }
            .to_string();

            let direct_score_rejected =
                !criterion_decisions.is_empty() && !direct_score_fields.is_empty();
            let schema_error = if !criterion_decisions.is_empty() {
                semantic_scoring_schema_error(&value)
            } else {
                scoring_schema_error(&value, max_score)
            };
            ScoringParseOutcome {
                output: ScoringOutput {
                    awarded_score,
                    confidence,
                    rationale: rationale.clone(),
                    teacher_visible_explanation: rationale.clone(),
                    needs_review: value
                        .get("needsReview")
                        .or_else(|| value.get("needs_review"))
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false)
                        || rationale.is_empty()
                        || (criterion_scores.is_empty() && criterion_decisions.is_empty())
                        || direct_score_rejected
                        || schema_error.is_some(),
                    warnings,
                    criterion_scores,
                    criterion_decisions,
                    direct_score_fields,
                    direct_score_rejected,
                },
                parsed_json: Some(value),
                parse_error: schema_error,
                salvaged_rationale: if rationale.is_empty() {
                    None
                } else {
                    Some(rationale)
                },
                parse_strategy,
            }
        }
        Err(error) => ScoringParseOutcome {
            output: ScoringOutput {
                awarded_score: 0.0,
                confidence: 0.0,
                rationale: String::new(),
                teacher_visible_explanation: String::new(),
                needs_review: true,
                warnings: vec!["scoring_json_parse_failed".to_string()],
                criterion_scores: vec![],
                criterion_decisions: vec![],
                direct_score_fields: vec![],
                direct_score_rejected: false,
            },
            parsed_json: None,
            parse_error: Some(error.to_string()),
            salvaged_rationale: None,
            parse_strategy: "parse_failed".to_string(),
        },
    }
}

fn scoring_schema_error(value: &serde_json::Value, max_score: f32) -> Option<String> {
    let Some(object) = value.as_object() else {
        return Some("Notlandırma çıktısı JSON nesnesi değil.".to_string());
    };
    let mut missing = Vec::new();
    for (canonical_name, aliases) in [
        (
            "awardedScore",
            &["awardedScore", "awarded_score", "score"][..],
        ),
        ("confidence", &["confidence"][..]),
        (
            "rationale",
            &["rationale", "feedback", "explanation", "reason"][..],
        ),
        (
            "criterionScores",
            &["criterionScores", "criteria", "scoringCriteria", "scores"][..],
        ),
    ] {
        if !aliases.iter().any(|alias| object.contains_key(*alias)) {
            missing.push(canonical_name);
        }
    }
    if !missing.is_empty() {
        return Some(format!(
            "Notlandırma şeması eksik alan içeriyor: {}",
            missing.join(",")
        ));
    }
    let awarded_score = object
        .get("awardedScore")
        .or_else(|| object.get("awarded_score"))
        .or_else(|| object.get("score"))
        .and_then(json_number);
    if awarded_score.map_or(true, |score| {
        !score.is_finite() || score < 0.0 || score > max_score as f64
    }) {
        return Some("Notlandırma toplam puanı geçerli aralıkta değil.".to_string());
    }
    let confidence = object.get("confidence").and_then(|value| value.as_f64());
    if confidence.map_or(true, |value| {
        !value.is_finite() || !(0.0..=1.0).contains(&value)
    }) {
        return Some("Notlandırma güven değeri 0..1 aralığında değil.".to_string());
    }
    if let Some(nr) = object
        .get("needsReview")
        .or_else(|| object.get("needs_review"))
    {
        if !nr.is_boolean() {
            return Some("Notlandırma needsReview alanı boolean değil.".to_string());
        }
    }
    let criteria_value = object
        .get("criterionScores")
        .or_else(|| object.get("criteria"))
        .or_else(|| object.get("scoringCriteria"))
        .or_else(|| object.get("scores"));
    if !criteria_value.is_some_and(|value| value.is_array()) {
        return Some("Notlandırma criterionScores alanı dizi değil.".to_string());
    }
    if let Some(criteria) = criteria_value.and_then(|value| value.as_array()) {
        for (index, criterion) in criteria.iter().enumerate() {
            let Some(criterion) = criterion.as_object() else {
                return Some(format!("Notlandırma kriteri {index} JSON nesnesi değil."));
            };
            for (field, aliases) in [
                ("criterionId", &["criterionId", "criterion_id"][..]),
                (
                    "criterionTitle",
                    &[
                        "criterionTitle",
                        "criterion_title",
                        "title",
                        "label",
                        "criterion",
                        "criterionName",
                        "criterion_name",
                        "name",
                    ][..],
                ),
                (
                    "criterionMaxScore",
                    &[
                        "criterionMaxScore",
                        "criterion_max_score",
                        "maxScore",
                        "max_score",
                        "points",
                    ][..],
                ),
                (
                    "awardedScore",
                    &["awardedScore", "awarded_score", "score", "points"][..],
                ),
                (
                    "rationale",
                    &["rationale", "reason", "explanation", "feedback"][..],
                ),
                (
                    "evidenceQuote",
                    &["evidenceQuote", "evidence_quote", "quote", "evidence"][..],
                ),
            ] {
                if !aliases.iter().any(|alias| criterion.contains_key(*alias)) {
                    return Some(format!(
                        "Notlandırma kriteri {index} eksik alan içeriyor: {field}"
                    ));
                }
            }
        }
    }
    None
}

fn direct_scoring_fields(value: &serde_json::Value) -> Vec<String> {
    let mut fields = Vec::new();
    if let Some(object) = value.as_object() {
        for field in [
            "awardedScore",
            "awarded_score",
            "score",
            "totalScore",
            "total_score",
        ] {
            if object.contains_key(field) {
                fields.push(field.to_string());
            }
        }
        if let Some(criteria) = object
            .get("criterionDecisions")
            .or_else(|| object.get("criteria"))
            .or_else(|| object.get("decisions"))
            .or_else(|| object.get("criterionScores"))
            .and_then(|value| value.as_array())
        {
            for criterion in criteria.iter().filter_map(|value| value.as_object()) {
                for field in ["awardedScore", "awarded_score", "score", "points"] {
                    if criterion.contains_key(field) {
                        fields.push(format!("criterion.{field}"));
                    }
                }
            }
        }
    }
    fields.sort();
    fields.dedup();
    fields
}

fn parse_semantic_criterion_decisions(value: &serde_json::Value) -> Vec<SemanticCriterionDecision> {
    let Some(items) = value
        .get("criterionDecisions")
        .or_else(|| value.get("decisions"))
        .or_else(|| value.get("criteria"))
        .and_then(|value| value.as_array())
    else {
        return vec![];
    };
    items
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let criterion_id = object_text(object, &["criterionId", "criterion_id"]);
            let level_id = object_text(
                object,
                &[
                    "levelId",
                    "level_id",
                    "selectedLevelId",
                    "selected_level_id",
                ],
            );
            if criterion_id.is_empty() || level_id.is_empty() {
                return None;
            }
            let exact_evidence = object_text(
                object,
                &[
                    "exactEvidence",
                    "exact_evidence",
                    "evidenceQuote",
                    "evidence_quote",
                    "quote",
                ],
            );
            let missing_requirements = object
                .get("missingRequirements")
                .or_else(|| object.get("missing_requirements"))
                .and_then(|value| value.as_array())
                .map(|items| items.iter().filter_map(json_text).collect())
                .unwrap_or_default();
            Some(SemanticCriterionDecision {
                criterion_id,
                level_id,
                exact_evidence: (!exact_evidence.is_empty()).then_some(exact_evidence),
                missing_requirements,
                contradiction: object
                    .get("contradiction")
                    .or_else(|| object.get("hasContradiction"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                rationale: object_text(object, &["rationale", "reason", "explanation"]),
            })
        })
        .collect()
}

fn semantic_scoring_schema_error(value: &serde_json::Value) -> Option<String> {
    let Some(object) = value.as_object() else {
        return Some("Semantik notlandırma çıktısı JSON nesnesi değil.".to_string());
    };
    let confidence = object.get("confidence").and_then(|value| value.as_f64());
    if confidence.map_or(true, |value| {
        !value.is_finite() || !(0.0..=1.0).contains(&value)
    }) {
        return Some("Semantik notlandırma güven değeri 0..1 aralığında değil.".to_string());
    }
    if !object
        .get("criterionDecisions")
        .or_else(|| object.get("decisions"))
        .or_else(|| object.get("criteria"))
        .is_some_and(|value| value.is_array())
    {
        return Some("Semantik notlandırma criterionDecisions alanı dizi değil.".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaking_scoring_request_is_text_only() {
        let contract = legacy_prompt_contract_with_data(
            ModelRequestKind::Scoring,
            "speaking policy",
            json!({
                "answerType": "speaking",
                "transcriptForScoring": "Öğrenci metni"
            }),
            3072,
            Some(ModelResponseFormat::JsonObject),
        );
        let body = build_scoring_request_body(&contract);
        let messages = body["messages"]
            .as_array()
            .expect("scoring request must have messages");
        assert!(messages[0]["content"].is_string());
        assert!(messages[1]["content"].is_string());
        assert!(!body.to_string().contains("image_url"));
    }

    #[test]
    fn analysis_output_parser_rejects_invalid_json_as_a_structured_model_error() {
        let error = parse_analysis_model_output("Bu bir serbest metin raporudur.")
            .expect_err("free text must not pass the structured analysis gate");
        assert_eq!(error.code, AppErrorCode::ModelResponseInvalidJson);
        assert!(error.message.contains("yapılandırılmış"));
        assert!(error.suggested_action.is_some());
    }

    #[test]
    fn analysis_schema_requires_claim_metric_references() {
        let schema = analysis_report_json_schema();
        assert_eq!(schema["required"], json!(["claims"]));
        assert_eq!(
            schema["properties"]["claims"]["items"]["required"],
            json!(["claim", "metricRefs", "recommendation"])
        );
    }

    #[test]
    fn strict_local_rejects_public_endpoints_but_accepts_ipv4_and_ipv6_loopback() {
        assert!(
            validate_base_url_for_privacy("http://127.0.0.1:8080", PrivacyMode::StrictLocal)
                .is_ok()
        );
        assert!(
            validate_base_url_for_privacy("http://[::1]:8080", PrivacyMode::StrictLocal).is_ok()
        );
        assert!(validate_base_url_for_privacy(
            "https://model.example.test:443",
            PrivacyMode::StrictLocal
        )
        .is_err());
        assert!(validate_base_url_for_privacy(
            "https://model.example.test:443",
            PrivacyMode::ExplicitExternal
        )
        .is_ok());
    }
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn spawn_test_server(
        response_health: &'static str,
        response_completion: &'static str,
    ) -> Option<String> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(error) => panic!("failed to bind test server: {error}"),
        };
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(_) => break,
                };
                handle_stream(&mut stream, response_health, response_completion);
            }
        });
        Some(format!("http://{}", addr))
    }

    fn handle_stream(stream: &mut TcpStream, health: &str, completion: &str) {
        let mut buffer = [0u8; 2048];
        let _ = stream.read(&mut buffer);
        let request = String::from_utf8_lossy(&buffer);
        let body = if request.contains("/health") {
            health
        } else {
            completion
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }

    #[test]
    fn test_server_unavailable() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = LlamaServerGateway::new("http://127.0.0.1:59999".to_string());
            let status = gateway.probe_server().await.unwrap();
            assert!(!status.server_running);
            assert!(!status.health_ok);
        });
    }

    #[test]
    fn test_health_and_completion_probe() {
        let base_url = match spawn_test_server(
            r#"{"status":"ok"}"#,
            r#"{"choices":[{"message":{"content":"OK"}}]}"#,
        ) {
            Some(base_url) => base_url,
            None => return,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = LlamaServerGateway::new(base_url);
            let health_only = gateway.get_status().await.unwrap();
            assert!(health_only.server_running);
            assert!(health_only.health_ok);
            assert!(!health_only.completion_probe_ok);
            assert!(health_only.health_verified_at.is_some());
            assert!(health_only.completion_probe_verified_at.is_none());
            let status = gateway.probe_server().await.unwrap();
            assert!(status.server_running);
            assert!(status.health_ok);
            assert!(status.completion_probe_ok);
            assert!(status.health_verified_at.is_some());
            assert!(status.completion_probe_verified_at.is_some());
        });
    }

    #[test]
    fn test_parse_question_text_output_requires_schema() {
        let err = parse_question_text_output(r#"{"page_warnings":[]}"#).unwrap_err();
        assert_eq!(err.code, AppErrorCode::ModelResponseInvalidSchema);
    }

    #[test]
    fn test_parse_question_text_output_handles_valid_payload() {
        let output = parse_question_text_output(
            r#"{"questions":[{"number":1,"question_text":"Question 1","confidence":0.8,"warnings":["low_confidence"]}],"page_warnings":["page_1"]}"#,
        )
        .expect("output");
        assert_eq!(output.questions.len(), 1);
        assert_eq!(output.questions[0].number, 1);
        assert_eq!(output.page_warnings, vec!["page_1".to_string()]);
    }

    #[test]
    fn test_parse_question_text_output_handles_fenced_alias_payload() {
        let output = parse_question_text_output(
            "Açıklama\n```json\n[{\"soru_no\":6,\"questionText\":\"S6. Son soru\",\"confidence\":0.9,\"warnings\":[]}]\n```",
        )
        .expect("output");

        assert_eq!(output.questions.len(), 1);
        assert_eq!(output.questions[0].number, 6);
        assert_eq!(output.questions[0].question_text, "S6. Son soru");
    }

    #[test]
    fn test_parse_student_answer_ocr_output_ignores_scoring_fields() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"Cevap","structuredAnswer":{"kind":"text"},"confidence":0.84,"needsReview":true,"reviewReasons":["handwriting"],"warnings":["layout"],"score":10,"points":12,"criteria":[{"label":"x"}]}"#,
            "Soru metni",
        );
        let output = outcome.output;

        assert_eq!(output.answer_text, "Cevap");
        assert_eq!(output.confidence, 0.84);
        assert!(output.structured_answer.is_some());
        assert!(output
            .warnings
            .iter()
            .any(|warning| warning == "ocr_scoring_fields_ignored"));
        assert!(output.needs_review);
        assert!(output
            .review_reasons
            .iter()
            .any(|reason| reason == "ocr_scoring_fields_present"));
    }

    #[test]
    fn test_parse_student_answer_ocr_output_flags_commentary_instead_of_accepting_it() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"Öğrencinin cevabı burada enerji dönüşümünü anlatmaktadır.","confidence":0.95,"needsReview":false,"reviewReasons":[],"warnings":[]}"#,
            "Enerji dönüşümünü açıklayınız.",
        );
        assert!(outcome.output.needs_review);
        assert!(outcome
            .output
            .review_reasons
            .contains(&"ocr_commentary_detected".to_string()));
    }

    #[test]
    fn test_parse_student_answer_ocr_output_flags_incomplete_contract() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"yalnızca görülen metin"}"#,
            "Soru metni",
        );
        assert!(outcome.output.needs_review);
        assert!(outcome
            .output
            .review_reasons
            .contains(&"ocr_schema_incomplete".to_string()));
    }

    #[test]
    fn test_parse_scoring_output_normalizes_numeric_ids_and_common_aliases() {
        let outcome = parse_scoring_output(
            r#"{
              "scores": [
                {
                  "criterionId": 1,
                  "label": "Konuya uygunluk, içerik ve ana düşünce",
                  "maxScore": "20",
                  "score": "18",
                  "feedback": "Konu açıkça ele alınmış.",
                  "evidence": [{"quote": "Bugün bu konu hakkında düşünüyorum."}]
                }
              ],
              "score": "18",
              "confidence": 0.9,
              "rationale": "Kanıtlı değerlendirme."
            }"#,
            50.0,
        );
        assert!(outcome.parse_error.is_none());
        assert_eq!(outcome.output.criterion_scores.len(), 1);
        assert_eq!(outcome.output.criterion_scores[0].criterion_id, "1");
        assert_eq!(
            outcome.output.criterion_scores[0].criterion_title,
            "Konuya uygunluk, içerik ve ana düşünce"
        );
        assert_eq!(outcome.output.criterion_scores[0].awarded_score, 18.0);
        assert_eq!(
            outcome.output.criterion_scores[0].evidence_quote.as_deref(),
            Some("Bugün bu konu hakkında düşünüyorum.")
        );
    }

    #[test]
    fn test_parse_scoring_output_preserves_evidence_quote() {
        let outcome = parse_scoring_output(
            r#"{"awardedScore":2,"confidence":0.88,"needsReview":false,"rationale":"Kanıt kriteri destekliyor.","criterionScores":[{"criterionId":"c1","criterionTitle":"Kavram","criterionMaxScore":2,"awardedScore":2,"rationale":"Kavram açıkça yazılmış.","evidenceQuote":"ısı enerjisine dönüşür"}],"warnings":[]}"#,
            2.0,
        );
        assert!(outcome.parse_error.is_none());
        assert_eq!(
            outcome.output.criterion_scores[0].evidence_quote.as_deref(),
            Some("ısı enerjisine dönüşür")
        );
    }

    #[test]
    fn test_parse_scoring_output_rejects_missing_required_schema_fields() {
        let outcome = parse_scoring_output(r#"{"awardedScore":0,"criterionScores":[]}"#, 10.0);
        assert!(outcome.parse_error.is_some());
        assert!(outcome.output.needs_review);
    }

    #[test]
    fn test_parse_scoring_output_rejects_title_as_criterion_id() {
        let outcome = parse_scoring_output(
            r#"{"awardedScore":1,"confidence":0.8,"needsReview":false,"rationale":"Yeterli gerekçe.","criterionScores":[{"criterionTitle":"Kriter","criterionMaxScore":2,"awardedScore":1,"rationale":"Kanıt var.","evidenceQuote":"kanıt"}]}"#,
            2.0,
        );
        assert!(outcome.parse_error.is_some());
        assert_eq!(outcome.output.criterion_scores[0].criterion_id, "");
        assert!(outcome.output.needs_review);
    }

    #[test]
    fn test_parse_student_answer_ocr_output_parses_uncertainty_metadata() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"çelişen sözcük kullanımı","confidence":0.73,"uncertainSpans":[{"text":"çelişen","start":0,"end":8,"alternatives":["gelişen"],"confidence":0.41,"reason":"handwriting_ambiguity","highlightRegion":{"x":0.1,"y":0.2,"width":0.3,"height":0.1,"pageIndex":0}}],"suggestedCorrections":[{"originalText":"çelişen","suggestedText":"gelişen","reason":"near_match","confidence":0.41,"applied":false,"highlightRegion":{"x":0.2,"y":0.3,"width":0.2,"height":0.1,"pageIndex":0}}],"criticalTermWarnings":[{"observedText":"çelişen sözcük kullanımı","expectedOrRelatedTerm":"gelişen sözcük kullanımı","reason":"semantic_confusion","warningCode":"critical_keyword_ocr_uncertain","highlightRegion":{"x":0.15,"y":0.35,"width":0.4,"height":0.12,"pageIndex":0}}],"ocrSemanticWarnings":["critical_keyword_ocr_uncertain"],"criticalKeywordUncertain":true,"reviewReasons":[],"warnings":[]}"#,
            "Soru metni",
        );

        assert!(outcome.output.critical_keyword_uncertain);
        assert_eq!(outcome.output.uncertain_spans.len(), 1);
        assert_eq!(outcome.output.suggested_corrections.len(), 1);
        assert_eq!(outcome.output.critical_term_warnings.len(), 1);
        assert!(outcome.output.uncertain_spans[0].highlight_region.is_some());
        assert!(outcome.output.suggested_corrections[0]
            .highlight_region
            .is_some());
        assert!(outcome.output.critical_term_warnings[0]
            .highlight_region
            .is_some());
        assert!(outcome
            .output
            .review_reasons
            .iter()
            .any(|reason| reason == "critical_keyword_uncertain"));
        assert!(outcome.output.warnings.iter().any(|warning| {
            warning == "critical_keyword_ocr_uncertain"
                || warning == "ocr_critical_keyword_uncertain"
        }));
    }

    #[test]
    fn test_parse_student_answer_ocr_output_forces_review_below_confidence_threshold() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"Belirsiz el yazısı","confidence":0.65,"needsReview":false,"reviewReasons":[],"warnings":[]}"#,
            "Soru metni",
        );

        assert!(outcome.output.needs_review);
        assert!(outcome
            .output
            .review_reasons
            .iter()
            .any(|reason| reason == "ocr_low_confidence"));
    }

    #[test]
    fn test_parse_student_answer_ocr_output_handles_fenced_json() {
        let outcome = parse_student_answer_ocr_output(
            "Açıklama\n```json\n{\"answerText\":\"Cevap\",\"reviewReasons\":[],\"warnings\":[]}\n```",
            "Soru metni",
        );

        assert_eq!(outcome.output.answer_text, "Cevap");
        assert_eq!(outcome.parse_strategy, "fenced_json");
        assert!(outcome.parse_error.is_none());
    }

    #[test]
    fn test_parse_student_answer_ocr_output_handles_trailing_prose() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"Cevap","reviewReasons":[],"warnings":[]} açıklama fazlası"#,
            "Soru metni",
        );

        assert_eq!(outcome.output.answer_text, "Cevap");
        assert_eq!(outcome.parse_strategy, "trailing_prose_trim");
        assert!(outcome.parse_error.is_none());
    }

    #[test]
    fn test_parse_student_answer_ocr_output_salvages_raw_text_on_malformed_json() {
        let outcome = parse_student_answer_ocr_output("```json\n{broken\n```", "Soru metni");

        assert_eq!(outcome.parse_strategy, "raw_text_salvage");
        assert!(outcome.parse_error.is_some());
        assert_eq!(outcome.salvaged_answer_text.as_deref(), Some("{broken"));
        assert_eq!(outcome.output.answer_text, "{broken");
        assert!(outcome.output.needs_review);
    }

    #[test]
    fn test_parse_student_answer_ocr_output_flags_printed_question_leak() {
        let outcome = parse_student_answer_ocr_output(
            r#"{"answerText":"Soru metni","reviewReasons":[],"warnings":[]}"#,
            "Soru metni",
        );

        assert!(outcome.printed_question_leak_detected);
        assert!(outcome.printed_text_mixed);
        assert!(outcome
            .output
            .review_reasons
            .iter()
            .any(|reason| reason == "printed_question_leak_detected"));
    }

    #[test]
    fn test_parse_student_answer_issue_correction_output_accepts_strict_json() {
        let outcome = parse_student_answer_issue_correction_output(
            r#"{"decision":"suggest_correction","originalText":"gelşeqiz","suggestedText":"çelişen","scope":"single_word","visualReading":"gelşeqiz","contextReason":"Visual and OCR hint align","confidence":0.91,"requiresTeacherApproval":true,"warnings":[]}"#,
        )
        .expect("issue correction output");

        assert!(matches!(
            outcome.output.decision,
            StudentAnswerOcrIssueCorrectionDecision::SuggestCorrection
        ));
        assert_eq!(outcome.output.original_text, "gelşeqiz");
        assert_eq!(outcome.output.suggested_text.as_deref(), Some("çelişen"));
        assert!(outcome.output.requires_teacher_approval);
    }

    #[test]
    fn test_parse_student_answer_issue_correction_output_blocks_scope_expansion() {
        let outcome = parse_student_answer_issue_correction_output(
            r#"{"decision":"suggest_correction","originalText":"gelşeqiz","suggestedText":"çelişen sözcüklerin bir arada kullanılması","scope":"single_word","visualReading":"gelşeqiz","contextReason":"Too broad","confidence":0.42,"requiresTeacherApproval":true,"warnings":[]}"#,
        )
        .expect("issue correction output");

        assert!(matches!(
            outcome.output.decision,
            StudentAnswerOcrIssueCorrectionDecision::NeedsTeacherReview
        ));
        assert!(outcome.output.suggested_text.is_none());
        assert!(outcome
            .output
            .warnings
            .iter()
            .any(|warning| warning == "scope_expansion_blocked"));
    }

    #[test]
    fn test_parse_rubric_model_response_handles_plain_json() {
        let payload = parse_rubric_model_response(
            r#"{"questions":[{"questionNumber":1,"maxPoints":10,"expectedAnswer":"A","criteria":[{"label":"L","points":10,"description":"D"}],"warnings":[]}]}"#,
        )
        .expect("payload");
        assert_eq!(payload.questions.len(), 1);
        assert_eq!(payload.questions[0].question_number, 1);
        assert_eq!(payload.questions[0].max_points, Some(10.0));
    }

    #[test]
    fn test_parse_rubric_model_response_accepts_canonical_rubric_fields() {
        let payload = parse_rubric_model_response(
            r#"{"questions":[{"questionNumber":1,"maxScore":10,"expectedAnswer":"A","keyConcepts":["A"],"criteria":[{"label":"L","points":10,"description":"D"}],"partialCreditHints":["Kısmi"],"zeroScoreConditions":["Boş"],"commonMistakes":["Hata"],"warnings":[]}],"documentWarnings":[]}"#,
        )
        .expect("payload");
        let question = &payload.questions[0];
        assert_eq!(question.max_points, Some(10.0));
        assert_eq!(question.key_concepts, vec!["A"]);
        assert_eq!(question.partial_credit_hints, vec!["Kısmi"]);
        assert_eq!(question.zero_score_conditions, vec!["Boş"]);
        assert_eq!(question.common_mistakes, vec!["Hata"]);
    }

    #[test]
    fn test_parse_rubric_model_response_normalizes_aliases() {
        let payload = parse_rubric_model_response(
            r#"{"questions":[{"question_no":2,"maxPoint":12,"expected_answer":"B","scoring_criteria":[{"kriter":"Ölçüt","score":12,"aciklama":"D"}],"warnings":[]}]}"#,
        )
        .expect("payload");

        let question = &payload.questions[0];
        assert_eq!(question.question_number, 2);
        assert_eq!(question.max_points, Some(12.0));
        assert_eq!(question.expected_answer.as_deref(), Some("B"));
        assert_eq!(question.criteria.len(), 1);
        assert_eq!(question.criteria[0].label, "Ölçüt");
        assert_eq!(question.criteria[0].points, 12.0);
        assert!(question
            .warnings
            .iter()
            .any(|warning| warning == "questionNumber_alias:question_no"));
        assert!(question
            .warnings
            .iter()
            .any(|warning| warning == "maxPoints_alias:maxPoint"));
        assert!(question
            .warnings
            .iter()
            .any(|warning| warning == "expectedAnswer_alias:expected_answer"));
        assert!(question
            .warnings
            .iter()
            .any(|warning| warning == "criteria_alias:scoring_criteria"));
    }

    #[test]
    fn test_parse_rubric_model_response_marks_empty_payload() {
        let payload = parse_rubric_model_response(
            r#"{"questions":[{"questionNumber":1,"maxPoints":null,"expectedAnswer":"","criteria":[],"warnings":[]}]}"#,
        )
        .expect("payload");
        assert!(payload.questions[0]
            .warnings
            .iter()
            .any(|warning| warning == "rubric_empty_content"));
    }

    #[test]
    fn test_parse_rubric_model_response_handles_fenced_and_explanatory_json() {
        let payload = parse_rubric_model_response(
            "Açıklama\n```json\n{\"questions\":[{\"soru_no\":6,\"max_score\":8,\"expected_answer\":\"B\",\"rubric\":[{\"name\":\"Nokta\",\"puan\":4,\"description\":\"D\"}],\"warnings\":[\"note\"]}]}\n```",
        )
        .expect("payload");
        assert_eq!(payload.questions[0].question_number, 6);
        assert!(payload.questions[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("soru_no")));
        assert!(payload.questions[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("max_score")));
    }

    #[test]
    fn test_parse_rubric_model_response_invalid_json_errors() {
        let err = parse_rubric_model_response("```json\n{broken\n```").unwrap_err();
        assert_eq!(err.code, AppErrorCode::RubricJsonParseFailed);
    }

    #[test]
    fn test_parse_rubric_model_response_recovers_complete_items_from_truncated_json() {
        let payload = parse_rubric_model_response(
            r#"{"questions":[{"number":1,"max_points":10,"expected_answer":"A","criteria":[{"label":"L","points":10,"description":"D"}],"warnings":[]},{"number":2,"max_points":5"#,
        )
        .expect("partial payload");

        assert_eq!(payload.questions.len(), 1);
        assert_eq!(payload.questions[0].question_number, 1);
        assert!(payload.questions[0]
            .warnings
            .contains(&"partial_model_json_recovered".to_string()));
    }

    #[test]
    fn test_parse_scoring_output_accepts_snake_case_keys_and_criteria_array() {
        let outcome = parse_scoring_output(
            r#"{
              "criteria": [
                {
                  "criterion_id": "content_main_idea",
                  "criterion_title": "Konuya uygunluk, içerik ve ana düşünce",
                  "criterion_max_score": 20.0,
                  "awarded_score": 10.0,
                  "rationale": "Gerekçe açıklaması.",
                  "evidence_quote": "Aras'ı bu ara rahat bırak."
                }
              ],
              "awarded_score": 10.0,
              "confidence": 0.9,
              "rationale": "Genel açıklama."
            }"#,
            20.0,
        );
        assert!(outcome.parse_error.is_none());
        assert_eq!(outcome.output.criterion_scores.len(), 1);
        assert_eq!(
            outcome.output.criterion_scores[0].criterion_id,
            "content_main_idea"
        );
        assert_eq!(
            outcome.output.criterion_scores[0].criterion_title,
            "Konuya uygunluk, içerik ve ana düşünce"
        );
        assert_eq!(outcome.output.criterion_scores[0].awarded_score, 10.0);
        assert_eq!(
            outcome.output.criterion_scores[0].evidence_quote.as_deref(),
            Some("Aras'ı bu ara rahat bırak.")
        );
        assert_eq!(outcome.output.awarded_score, 10.0);
    }

    #[test]
    fn test_parse_semantic_scoring_output_extracts_level_evidence_and_rejects_direct_score() {
        let outcome = parse_scoring_output(
            r#"{
              "criterionDecisions": [{
                "criterionId": "c1",
                "levelId": "full",
                "exactEvidence": "Doğru cevap",
                "missingRequirements": [],
                "contradiction": false,
                "rationale": "Kriter karşılandı.",
                "awardedScore": 99
              }],
              "awardedScore": 99,
              "confidence": 0.94,
              "needsReview": false,
              "rationale": "Seviye kanıtla seçildi."
            }"#,
            10.0,
        );

        assert_eq!(outcome.output.criterion_decisions.len(), 1);
        assert_eq!(outcome.output.criterion_decisions[0].level_id, "full");
        assert_eq!(
            outcome.output.criterion_decisions[0]
                .exact_evidence
                .as_deref(),
            Some("Doğru cevap")
        );
        assert!(outcome.output.direct_score_rejected);
        assert!(outcome
            .output
            .direct_score_fields
            .contains(&"awardedScore".to_string()));
        assert!(outcome.output.needs_review);
    }

    #[test]
    fn test_parse_speaking_transcript_cleanup_output_snake_case_and_camel_case() {
        let snake_json = r#"{"segments":[{"segment_id":"seg-1","cleaned_text":"Merhaba dunya","changes":[],"semantic_change_detected":false,"needs_review":false}]}"#;
        let res = parse_speaking_transcript_cleanup_output(snake_json).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].segment_id, "seg-1");
        assert_eq!(res[0].cleaned_text, "Merhaba dunya");

        let camel_json = r#"{"segments":[{"segmentId":"seg-2","cleanedText":"Test mesaji","changes":[],"semanticChangeDetected":true,"needsReview":true}]}"#;
        let res2 = parse_speaking_transcript_cleanup_output(camel_json).unwrap();
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].segment_id, "seg-2");
        assert_eq!(res2[0].cleaned_text, "Test mesaji");
        assert!(res2[0].semantic_change_detected);
        assert!(res2[0].needs_review);
    }

    #[test]
    fn test_parse_speaking_transcript_cleanup_output_fenced_and_root_array() {
        let fenced = "<think>Düşünüyorum...</think>\n```json\n[{\"id\":\"seg-10\",\"text\":\"Satir 1\"}]\n```";
        let res = parse_speaking_transcript_cleanup_output(fenced).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].segment_id, "seg-10");
        assert_eq!(res[0].cleaned_text, "Satir 1");
    }

    #[test]
    fn test_parse_speaking_transcript_cleanup_output_unescaped_newlines() {
        let unescaped =
            "{\"segments\":[{\"segment_id\":\"seg-1\",\"cleaned_text\":\"Satir 1\nSatir 2\"}]}";
        let res = parse_speaking_transcript_cleanup_output(unescaped).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].segment_id, "seg-1");
        assert_eq!(res[0].cleaned_text, "Satir 1\nSatir 2");
    }

    #[test]
    fn test_parse_speaking_transcript_cleanup_output_with_wrapper_prose_and_nested_array() {
        let wrapped = r#"Model cevabı:
{"segments":[{"segment_id":"seg-1","cleaned_text":"Birinci cümle"},{"segment_id":"seg-2","cleaned_text":"İkinci cümle"}]}
İşlem tamamlandı."#;
        let result = parse_speaking_transcript_cleanup_output(wrapped).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].segment_id, "seg-2");

        let root_array = r#"İşte çıktı: [{"id":"seg-1","text":"Birinci cümle"},{"id":"seg-2","text":"İkinci cümle"}]"#;
        let result = parse_speaking_transcript_cleanup_output(root_array).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].cleaned_text, "Birinci cümle");
    }

    #[test]
    fn test_parse_speaking_transcript_cleanup_output_accepts_common_root_aliases() {
        let output = r#"{"cleanedSegments":[{"index":0,"content":"Temiz metin"}]}"#;
        let result = parse_speaking_transcript_cleanup_output(output).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].segment_id, "0");
        assert_eq!(result[0].cleaned_text, "Temiz metin");

        let nested =
            r#"{"result":{"output":{"segments":[{"segmentId":"seg-2","text":"İç içe çıktı"}]}}}"#;
        let result = parse_speaking_transcript_cleanup_output(nested).unwrap();
        assert_eq!(result[0].cleaned_text, "İç içe çıktı");
    }

    #[test]
    fn test_parse_speaking_transcript_cleanup_output_distinguishes_schema_error() {
        let error = parse_speaking_transcript_cleanup_output(
            r#"{"cleanedTranscript":"Sadece metin döndü"}"#,
        )
        .unwrap_err();
        assert_eq!(error.code, AppErrorCode::ModelResponseInvalidSchema);
        assert!(error.message.contains("beklenen segment şemasında"));
    }

    #[test]
    fn test_extract_assistant_content_accepts_openai_content_parts() {
        let body =
            r#"{"choices":[{"message":{"content":[{"type":"text","text":"{\"segments\":[]}"}]}}]}"#;
        assert_eq!(
            extract_assistant_content(body).unwrap(),
            "{\"segments\":[]}"
        );
    }

    fn spawn_raw_server(
        response_head: &'static str,
        body_chunks: Vec<&'static [u8]>,
        chunked: bool,
    ) -> Option<String> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(error) => panic!("failed to bind test server: {error}"),
        };
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = stream.read(&mut buffer);
                let mut head = response_head.to_string();
                if chunked {
                    head.push_str("\r\n");
                    for chunk in body_chunks.iter() {
                        head.push_str(&format!("{:x}\r\n", chunk.len()));
                        head.push_str(&String::from_utf8_lossy(chunk));
                        head.push_str("\r\n");
                    }
                    head.push_str("0\r\n\r\n");
                } else {
                    let total: usize = body_chunks.iter().map(|chunk| chunk.len()).sum();
                    head.push_str(&format!("Content-Length: {total}\r\n"));
                    head.push_str("\r\n");
                    let _ = stream.write_all(head.as_bytes());
                    for chunk in body_chunks.iter() {
                        let _ = stream.write_all(chunk);
                    }
                    return;
                }
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.flush();
            }
        });
        Some(format!("http://{}", addr))
    }

    fn minimal_limits(max_response: u64, max_request: u64) -> GatewayLimits {
        GatewayLimits {
            max_response_body_bytes: max_response,
            max_request_body_bytes: max_request,
            connect_timeout: Duration::from_secs(5),
            first_byte_timeout: Duration::from_secs(5),
            idle_chunk_timeout: Duration::from_secs(5),
        }
    }

    #[test]
    fn test_response_one_byte_below_limit_is_accepted() {
        let payload = br#"{"choices":[{"message":{"content":"OK"}}]}"#;
        let base_url = match spawn_raw_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
            vec![payload.as_slice()],
            false,
        ) {
            Some(url) => url,
            None => return,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let base = base_url.clone();
            let gateway = LlamaServerGateway::new_with_limits(
                base_url,
                minimal_limits((payload.len() + 1) as u64, 1024 * 1024),
            );
            let result = gateway
                .send_chat_request(&base, json!({"x": 1}), 10, "test")
                .await
                .expect("response should be accepted");
            assert_eq!(result.0, 200);
            assert!(result.1.contains("OK"));
        });
    }

    #[test]
    fn test_response_one_byte_above_limit_is_rejected_without_raw_body() {
        let payload = b"{\"choices\":[{\"message\":{\"content\":\"MODEL_SECRET_47bf\"}}]}";
        let base_url = match spawn_raw_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
            vec![payload.as_slice()],
            false,
        ) {
            Some(url) => url,
            None => return,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let base = base_url.clone();
            let gateway = LlamaServerGateway::new_with_limits(
                base_url,
                minimal_limits((payload.len() - 1) as u64, 1024 * 1024),
            );
            let error = gateway
                .send_chat_request(&base, json!({"x": 1}), 10, "test")
                .await
                .expect_err("response must be rejected");
            assert_eq!(error.code, AppErrorCode::ModelResponseTooLarge);
            let serialized = format!("{error:?}");
            assert!(!serialized.contains("MODEL_SECRET_47bf"));
        });
    }

    #[test]
    fn test_chunked_response_exceeding_total_limit_is_stopped() {
        let base_url = match spawn_raw_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n",
            vec![b"aaaaaaaaaa".as_slice(), b"bbbbbbbbbb".as_slice()],
            true,
        ) {
            Some(url) => url,
            None => return,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let base = base_url.clone();
            let gateway =
                LlamaServerGateway::new_with_limits(base_url, minimal_limits(12, 1024 * 1024));
            let error = gateway
                .send_chat_request(&base, json!({"x": 1}), 10, "test")
                .await
                .expect_err("chunked body must be rejected once the total cap is crossed");
            assert_eq!(error.code, AppErrorCode::ModelResponseTooLarge);
        });
    }

    #[test]
    fn test_request_body_over_limit_is_rejected_before_send() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let gateway = LlamaServerGateway::new_with_limits(
                "http://127.0.0.1:1".to_string(),
                minimal_limits(1024 * 1024, 8),
            );
            let big_body = json!({"prompt": "x".repeat(64)});
            let error = gateway
                .send_chat_request("http://placeholder", big_body, 5, "test")
                .await
                .expect_err("oversized request must be rejected before any network IO");
            assert_eq!(error.code, AppErrorCode::ModelRequestTooLarge);
        });
    }

    #[test]
    fn test_oversized_response_never_becomes_ocr_result() {
        let payload = b"{\"answerText\":\"OCR_SECRET_17ce\",\"confidence\":0.99}";
        let base_url = match spawn_raw_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
            vec![payload.as_slice()],
            false,
        ) {
            Some(url) => url,
            None => return,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let base = base_url.clone();
            let gateway = LlamaServerGateway::new_with_limits(
                base_url,
                minimal_limits((payload.len() - 1) as u64, 1024 * 1024),
            );
            let error = gateway
                .send_chat_request(&base, json!({"x": 1}), 10, "test")
                .await
                .expect_err("oversized body must be rejected");
            assert_eq!(error.code, AppErrorCode::ModelResponseTooLarge);
        });
    }

    #[test]
    fn test_non_json_content_type_is_rejected() {
        let base_url = match spawn_raw_server(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n",
            vec![b"<html>MODEL_SECRET_47bf</html>".as_slice()],
            false,
        ) {
            Some(url) => url,
            None => return,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let base = base_url.clone();
            let gateway = LlamaServerGateway::new_with_limits(
                base_url,
                minimal_limits(1024 * 1024, 1024 * 1024),
            );
            let error = gateway
                .send_chat_request(&base, json!({"x": 1}), 10, "test")
                .await
                .expect_err("text/html must be rejected");
            assert_eq!(error.code, AppErrorCode::ModelResponseInvalidContentType);
            let serialized = format!("{error:?}");
            assert!(!serialized.contains("MODEL_SECRET_47bf"));
        });
    }
}

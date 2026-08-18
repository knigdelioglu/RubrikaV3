use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelMode, ModelProfile, ModelStatus, PrivacyMode};
use crate::domain::model_platform::{fingerprint_runtime_definition, ModelTaskKind};
use crate::services::llama_server_gateway::validate_base_url_for_privacy;
use crate::services::model_config_service::ModelConfigService;
use crate::services::model_platform_migration_service::materialize_route_as_legacy_profile;
use crate::services::model_platform_service::ModelPlatformService;
use crate::services::model_process_manager::{ModelProcessManager, RuntimeLeaseGrant};
use crate::services::model_router_service::{
    ModelRouterService, ResolvedModelRoute, RouteUsageMode,
};
use crate::services::platform_launch_registry;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelUseCase {
    QuestionTextExtraction,
    RubricPdfImport,
    StudentAnswerOcr,
    StudentAnswerOcrIssueCorrection,
    Scoring,
    SpeakingTranscriptCleanup,
    SpeakingEvaluation,
    Analysis,
    GeneralText,
}

impl ModelUseCase {
    fn step_name(&self) -> &'static str {
        match self {
            Self::QuestionTextExtraction => "question_text_extraction",
            Self::RubricPdfImport => "rubric_extraction",
            Self::StudentAnswerOcr => "student_answer_ocr",
            Self::StudentAnswerOcrIssueCorrection => "student_answer_ocr_issue_correction",
            Self::Scoring => "scoring",
            Self::SpeakingTranscriptCleanup => "speaking_transcript_cleanup",
            Self::SpeakingEvaluation => "speaking_evaluation",
            Self::Analysis => "analysis",
            Self::GeneralText => "general_text",
        }
    }

    fn contains_student_data(&self) -> bool {
        matches!(
            self,
            Self::StudentAnswerOcr
                | Self::StudentAnswerOcrIssueCorrection
                | Self::Scoring
                | Self::SpeakingTranscriptCleanup
                | Self::SpeakingEvaluation
        )
    }

    fn platform_task(&self, requested_profile_id: Option<&str>) -> ModelTaskKind {
        // One-release read compatibility for callers that still pass the old
        // speaking profile ids. New callers should use the explicit use case.
        if requested_profile_id == Some("speaking_transcript_cleanup_12b") {
            return ModelTaskKind::SpeakingTranscriptCleanup;
        }
        if requested_profile_id == Some("speaking_rubric_evaluation_12b") {
            return ModelTaskKind::SpeakingEvaluation;
        }
        match self {
            Self::QuestionTextExtraction => ModelTaskKind::QuestionTextExtraction,
            Self::RubricPdfImport => ModelTaskKind::RubricExtraction,
            Self::StudentAnswerOcr => ModelTaskKind::StudentAnswerOcr,
            Self::StudentAnswerOcrIssueCorrection => ModelTaskKind::StudentAnswerOcrIssueCorrection,
            Self::Scoring => ModelTaskKind::SemanticScoring,
            Self::SpeakingTranscriptCleanup => ModelTaskKind::SpeakingTranscriptCleanup,
            Self::SpeakingEvaluation => ModelTaskKind::SpeakingEvaluation,
            Self::Analysis => ModelTaskKind::Analysis,
            Self::GeneralText => ModelTaskKind::GeneralText,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Text,
    Vision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuntimeRequest {
    pub use_case: ModelUseCase,
    pub capability: ModelCapability,
    pub requires_mmproj: bool,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuntimeStatus {
    pub health_ok: bool,
    pub state: ModelRuntimeState,
    pub managed_pid: Option<u32>,
    pub host: String,
    pub port: u16,
    pub port_listening: bool,
    pub port_health_ok: bool,
    pub llama_server_binary_exists: bool,
    pub model_file_exists: bool,
    pub mmproj_file_exists: bool,
    pub config_complete: bool,
    pub autostart_available: bool,
    pub message: String,
    pub active_lease_count: usize,
    pub draining: bool,
    pub oldest_lease_age_seconds: Option<i64>,
    pub lease_operation_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelRuntimeState {
    Stopped,
    Starting,
    Loading,
    Healthy,
    Unhealthy,
    Failed,
    ConfigMissing,
    PortBlocked,
    Draining,
    Stopping,
    Unverified,
}

#[derive(Debug, Clone)]
pub struct ModelRouteLeaseMetadata {
    pub task_profile_id: String,
    pub binding_id: String,
    pub model_definition_id: String,
    pub runtime_definition_id: String,
    pub model_fingerprint: String,
    pub runtime_fingerprint: String,
}

/// Resolved model/runtime identity without starting the inference process.
///
/// Production services use this when cache keys or provenance need to be
/// computed before a runtime lease is acquired. Platform-backed identities
/// carry model/runtime fingerprints; legacy profiles intentionally leave them
/// empty so callers can use their existing compatibility fallback.
#[derive(Debug, Clone)]
pub struct ModelRuntimeIdentity {
    pub profile_id: String,
    pub base_url: String,
    pub model_path: String,
    pub model_display_name: String,
    pub model_family: String,
    pub model_fingerprint: Option<String>,
    pub runtime_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformRuntimeSelection {
    profile_id: String,
    model_fingerprint: String,
    runtime_fingerprint: String,
}

pub struct ModelRuntimeLease {
    manager: ModelProcessManager,
    grant: RuntimeLeaseGrant,
    released: Arc<AtomicBool>,
    route_metadata: Option<ModelRouteLeaseMetadata>,
}

impl ModelRuntimeLease {
    pub fn lease_id(&self) -> &str {
        &self.grant.lease_id
    }

    pub fn runtime_instance_id(&self) -> &str {
        &self.grant.runtime_instance_id
    }

    pub fn base_url(&self) -> &str {
        &self.grant.base_url
    }

    pub fn profile_id(&self) -> &str {
        &self.grant.profile_id
    }

    pub fn profile_fingerprint(&self) -> &str {
        &self.grant.profile_fingerprint
    }

    pub fn model_fingerprint(&self) -> Option<&str> {
        self.route_metadata
            .as_ref()
            .map(|metadata| metadata.model_fingerprint.as_str())
            .or_else(|| self.grant.model_fingerprint.as_deref())
    }

    pub fn runtime_fingerprint(&self) -> Option<&str> {
        self.route_metadata
            .as_ref()
            .map(|metadata| metadata.runtime_fingerprint.as_str())
    }

    pub fn task_profile_id(&self) -> Option<&str> {
        self.route_metadata
            .as_ref()
            .map(|metadata| metadata.task_profile_id.as_str())
    }

    pub fn binding_id(&self) -> Option<&str> {
        self.route_metadata
            .as_ref()
            .map(|metadata| metadata.binding_id.as_str())
    }

    pub fn model_definition_id(&self) -> Option<&str> {
        self.route_metadata
            .as_ref()
            .map(|metadata| metadata.model_definition_id.as_str())
    }

    pub fn runtime_definition_id(&self) -> Option<&str> {
        self.route_metadata
            .as_ref()
            .map(|metadata| metadata.runtime_definition_id.as_str())
    }

    pub fn correlation_id(&self) -> &str {
        &self.grant.correlation_id
    }

    pub fn active_lease_count(&self) -> usize {
        self.grant.active_lease_count
    }

    pub async fn release(&self) -> Result<(), AppError> {
        if self.released.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.manager
            .release_lease(&self.grant.lease_id, &self.grant.runtime_instance_id)
            .await
    }
}

impl Drop for ModelRuntimeLease {
    fn drop(&mut self) {
        if self.released.swap(true, Ordering::AcqRel) {
            return;
        }
        let manager = self.manager.clone();
        let lease_id = self.grant.lease_id.clone();
        let runtime_instance_id = self.grant.runtime_instance_id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = manager.release_lease(&lease_id, &runtime_instance_id).await;
            });
        }
    }
}

#[derive(Clone)]
pub struct ModelRuntimeService {
    config_service: ModelConfigService,
    process_manager: ModelProcessManager,
    platform_service: Option<ModelPlatformService>,
    platform_runtime_selection: Arc<Mutex<Option<PlatformRuntimeSelection>>>,
}

impl ModelRuntimeService {
    pub fn new(config_service: ModelConfigService, process_manager: ModelProcessManager) -> Self {
        Self {
            config_service,
            process_manager,
            platform_service: None,
            platform_runtime_selection: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_model_platform(mut self, platform_service: ModelPlatformService) -> Self {
        self.platform_service = Some(platform_service);
        self
    }

    pub fn resolve_runtime_identity(
        &self,
        profile_id: Option<&str>,
        operation: &ModelRuntimeRequest,
        correlation_id: &str,
    ) -> Result<ModelRuntimeIdentity, AppError> {
        let (effective_profile_id, profile, route) =
            self.resolve_effective_profile(profile_id, operation, correlation_id)?;
        if let Some(route) = route {
            return Ok(ModelRuntimeIdentity {
                profile_id: effective_profile_id.unwrap_or_else(|| profile.id.clone()),
                base_url: profile.base_url,
                model_path: route.model.model_path.clone(),
                model_display_name: route.model.display_name.clone(),
                model_family: route.model.family.clone(),
                model_fingerprint: Some(route.model.model_fingerprint.clone()),
                runtime_fingerprint: Some(fingerprint_runtime_definition(&route.runtime)),
            });
        }
        Ok(ModelRuntimeIdentity {
            profile_id: effective_profile_id.unwrap_or_else(|| profile.id.clone()),
            base_url: profile.base_url,
            model_path: profile.model_path,
            model_display_name: profile.display_name,
            model_family: "legacy".to_string(),
            model_fingerprint: None,
            runtime_fingerprint: None,
        })
    }

    pub async fn acquire_ready_runtime_lease(
        &self,
        profile_id: Option<&str>,
        consumer_id: &str,
        operation: ModelRuntimeRequest,
        correlation_id: &str,
    ) -> Result<ModelRuntimeLease, AppError> {
        let (effective_profile_id, profile, route) =
            self.resolve_effective_profile(profile_id, &operation, correlation_id)?;
        if let (Some(materialized_id), Some(resolved_route)) =
            (effective_profile_id.as_deref(), route.as_ref())
        {
            self.prepare_platform_runtime(materialized_id, resolved_route, correlation_id)
                .await?;
        }
        if profile.server_path.trim().is_empty()
            && profile.model_path.trim().is_empty()
            && profile.mmproj_path.trim().is_empty()
        {
            return Err(AppError {
                code: AppErrorCode::ModelConfigMissing,
                message: "Model yapılandırması eksik.".to_string(),
                recoverable: true,
                suggested_action: Some("Model profilini yapılandırın.".to_string()),
                technical_details: Some(format!(
                    "step={}; profile_id={}; server_path_empty=true; model_path_empty=true; mmproj_path_empty=true",
                    operation.use_case.step_name(),
                    profile.id
                )),
                correlation_id: correlation_id.to_string(),
            });
        }
        validate_base_url_for_privacy(&profile.base_url, profile.privacy_mode)?;
        if profile.privacy_mode == PrivacyMode::StrictLocal
            && profile.mode == ModelMode::External
            && operation.use_case.contains_student_data()
        {
            return Err(AppError {
                code: AppErrorCode::ModelPrivacyBlocked,
                message: "Öğrenci verisi taşıyan model işlemi yalnızca güvenli yerel profilde çalıştırılabilir.".to_string(),
                recoverable: true,
                suggested_action: Some("Yönetilen yerel model profilini seçin veya harici model kullanımını Ayarlar'dan açıkça onaylayın.".to_string()),
                technical_details: Some(format!(
                    "strict_local_external_student_data_blocked; profile_id={}; use_case={:?}",
                    profile.id, operation.use_case
                )),
                correlation_id: correlation_id.to_string(),
            });
        }
        let grant = self
            .process_manager
            .acquire_lease(
                effective_profile_id.as_deref(),
                operation.requires_mmproj,
                operation.timeout_seconds,
                consumer_id,
                Some(correlation_id),
                operation.use_case.step_name(),
                correlation_id,
            )
            .await
            .map_err(|mut error| {
                error.correlation_id = correlation_id.to_string();
                normalize_model_error(error)
            })?;
        if let (Some(materialized_id), Some(resolved_route)) =
            (effective_profile_id.as_deref(), route.as_ref())
        {
            self.remember_platform_runtime(materialized_id, resolved_route)?;
        }
        Ok(ModelRuntimeLease {
            manager: self.process_manager.clone(),
            grant,
            released: Arc::new(AtomicBool::new(false)),
            route_metadata: route.as_ref().map(route_lease_metadata),
        })
    }

    fn resolve_effective_profile(
        &self,
        requested_profile_id: Option<&str>,
        operation: &ModelRuntimeRequest,
        correlation_id: &str,
    ) -> Result<(Option<String>, ModelProfile, Option<ResolvedModelRoute>), AppError> {
        let Some(platform) = self.platform_service.as_ref() else {
            let profile = self.config_service.get_profile(requested_profile_id)?;
            return Ok((requested_profile_id.map(str::to_string), profile, None));
        };
        let snapshot = platform.snapshot().map_err(|mut error| {
            error.correlation_id = correlation_id.to_string();
            error
        })?;
        if snapshot.models.is_empty() && snapshot.bindings.is_empty() {
            let profile = self.config_service.get_profile(requested_profile_id)?;
            return Ok((requested_profile_id.map(str::to_string), profile, None));
        }

        let task = operation.use_case.platform_task(requested_profile_id);
        let route = ModelRouterService::new(platform.clone())
            .resolve(task, RouteUsageMode::Production)
            .map_err(|mut error| {
                error.correlation_id = correlation_id.to_string();
                error
            })?;
        let materialized_id = materialize_route_as_legacy_profile(&self.config_service, &route)
            .map_err(|mut error| {
                error.correlation_id = correlation_id.to_string();
                error
            })?;
        let profile = self.config_service.get_profile(Some(&materialized_id))?;
        Ok((Some(materialized_id), profile, Some(route)))
    }

    async fn prepare_platform_runtime(
        &self,
        profile_id: &str,
        route: &ResolvedModelRoute,
        correlation_id: &str,
    ) -> Result<(), AppError> {
        let desired = platform_selection(profile_id, route);
        let current = self
            .platform_runtime_selection
            .lock()
            .map_err(|error| runtime_state_error(error.to_string(), correlation_id))?
            .clone();

        if let Some(current) = current.filter(|current| current != &desired) {
            let active_leases = self.process_manager.active_lease_count()?;
            if active_leases > 0 {
                return Err(AppError {
                    code: AppErrorCode::ModelRuntimeProfileBusy,
                    message: "Etkin model işlemi sürerken model/runtime değiştirilemez.".to_string(),
                    recoverable: true,
                    suggested_action: Some("Etkin model işlemlerinin tamamlanmasını bekleyin.".to_string()),
                    technical_details: Some(format!(
                        "active_leases={active_leases}; current_profile={}; requested_profile={profile_id}",
                        current.profile_id
                    )),
                    correlation_id: correlation_id.to_string(),
                });
            }
            self.process_manager
                .stop_server(Some(&current.profile_id))
                .await
                .map_err(|mut error| {
                    error.correlation_id = correlation_id.to_string();
                    error
                })?;
            self.config_service
                .remove_ephemeral_profile(&current.profile_id)?;
            platform_launch_registry::remove(&current.profile_id);
            let mut guard = self
                .platform_runtime_selection
                .lock()
                .map_err(|error| runtime_state_error(error.to_string(), correlation_id))?;
            if guard.as_ref() == Some(&current) {
                *guard = None;
            }
        }

        let status = self
            .process_manager
            .get_model_status(Some(profile_id))
            .await
            .map_err(|mut error| {
                error.correlation_id = correlation_id.to_string();
                error
            })?;
        if route.runtime.managed && status.server_running && !status.started_by_app {
            return Err(AppError {
                code: AppErrorCode::ModelRuntimePortOccupied,
                message: "Seçili managed runtime portu doğrulanmamış başka bir süreç tarafından kullanılıyor."
                    .to_string(),
                recoverable: true,
                suggested_action: Some(
                    "Çakışan süreci kapatın veya model runtime için başka bir port seçin.".to_string(),
                ),
                technical_details: Some(format!(
                    "profile_id={profile_id}; base_url={}; managed=true; started_by_app=false",
                    status.base_url
                )),
                correlation_id: correlation_id.to_string(),
            });
        }
        Ok(())
    }

    fn remember_platform_runtime(
        &self,
        profile_id: &str,
        route: &ResolvedModelRoute,
    ) -> Result<(), AppError> {
        let selection = platform_selection(profile_id, route);
        let mut guard = self
            .platform_runtime_selection
            .lock()
            .map_err(|error| AppError {
                code: AppErrorCode::ModelStateAccessFailed,
                message: "Model platform runtime seçimi kaydedilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatın.".to_string()),
                technical_details: Some(error.to_string()),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })?;
        *guard = Some(selection);
        Ok(())
    }

    pub async fn get_runtime_status(
        &self,
        profile_id: Option<&str>,
        request: &ModelRuntimeRequest,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let status = self.process_manager.get_model_status(profile_id).await?;
        self.get_runtime_status_with_status(profile_id, request, status)
            .await
    }

    pub async fn get_model_status(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelStatus, AppError> {
        self.process_manager.get_model_status(profile_id).await
    }

    pub fn get_profile(&self, profile_id: &str) -> Result<ModelProfile, AppError> {
        let use_case = match profile_id {
            "speaking_transcript_cleanup_12b" => Some(ModelUseCase::SpeakingTranscriptCleanup),
            "speaking_rubric_evaluation_12b" => Some(ModelUseCase::SpeakingEvaluation),
            _ => None,
        };
        if let Some(use_case) = use_case {
            let request = ModelRuntimeRequest {
                use_case,
                capability: ModelCapability::Text,
                requires_mmproj: false,
                timeout_seconds: 60,
            };
            let correlation_id = uuid::Uuid::new_v4().to_string();
            let (_, profile, _) =
                self.resolve_effective_profile(Some(profile_id), &request, &correlation_id)?;
            return Ok(profile);
        }
        self.config_service.get_model_profile(profile_id)
    }

    pub async fn probe_model_status(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelStatus, AppError> {
        self.process_manager.probe_model_server(profile_id).await
    }

    pub async fn start_server(
        &self,
        profile_id: Option<&str>,
    ) -> Result<crate::services::model_process_manager::StartModelServerOutput, AppError> {
        self.process_manager.start_server(profile_id).await
    }

    pub async fn stop_server(
        &self,
        profile_id: Option<&str>,
    ) -> Result<crate::services::model_process_manager::StopModelServerOutput, AppError> {
        self.process_manager.stop_server(profile_id).await
    }

    pub async fn set_mode(
        &self,
        profile_id: Option<&str>,
        mode: ModelMode,
    ) -> Result<ModelStatus, AppError> {
        if mode == ModelMode::External {
            let profile = self.config_service.get_profile(profile_id)?;
            if profile.privacy_mode != PrivacyMode::ExplicitExternal {
                return Err(AppError {
                    code: AppErrorCode::ModelExternalConsentRequired,
                    message: "Harici model kullanımı için açık kullanıcı onayı gerekiyor."
                        .to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Ayarlar > Modeller bölümünde harici kullanımı açıkça onaylayın."
                            .to_string(),
                    ),
                    technical_details: Some(format!(
                        "external_mode_requires_explicit_external_privacy; profile_id={}",
                        profile.id
                    )),
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                });
            }
        }
        self.process_manager.set_mode(profile_id, mode).await
    }

    pub fn enable_external_profile(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelProfile, AppError> {
        self.config_service.enable_external_profile(profile_id)
    }

    pub async fn reset_profile(&self, profile_id: Option<&str>) -> Result<ModelStatus, AppError> {
        let _ = profile_id;
        self.process_manager.reset_profile().await
    }

    pub async fn preview_args(
        &self,
        profile_id: Option<&str>,
    ) -> Result<crate::domain::model::ModelServerArgsPreview, AppError> {
        self.process_manager.preview_args(profile_id).await
    }

    pub async fn get_log_tail(
        &self,
        profile_id: Option<&str>,
        lines: usize,
    ) -> Result<(String, Vec<String>), AppError> {
        let status = self.get_model_status(profile_id).await?;
        let path = status
            .log_path
            .clone()
            .unwrap_or_else(|| crate::platform::paths::model_server_log_path(&status.profile_id));
        Ok((
            path.to_string_lossy().to_string(),
            read_log_lines(&path, lines),
        ))
    }

    async fn get_runtime_status_with_status(
        &self,
        profile_id: Option<&str>,
        request: &ModelRuntimeRequest,
        status: ModelStatus,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        let mut runtime = build_runtime_status(request, &profile, status);
        runtime.active_lease_count = self.process_manager.active_lease_count()?;
        runtime.draining = self.process_manager.is_draining()?;
        let lease_diagnostics = self.process_manager.lease_diagnostics()?;
        runtime.oldest_lease_age_seconds = lease_diagnostics.oldest_lease_age_seconds;
        runtime.lease_operation_kinds = lease_diagnostics.operation_kinds;
        if runtime.draining {
            runtime.state = ModelRuntimeState::Draining;
            runtime.message = "Model işlemlerin bitmesi bekleniyor.".to_string();
        }
        Ok(runtime)
    }
}

fn platform_selection(profile_id: &str, route: &ResolvedModelRoute) -> PlatformRuntimeSelection {
    PlatformRuntimeSelection {
        profile_id: profile_id.to_string(),
        model_fingerprint: route.model.model_fingerprint.clone(),
        runtime_fingerprint: fingerprint_runtime_definition(&route.runtime),
    }
}

fn route_lease_metadata(route: &ResolvedModelRoute) -> ModelRouteLeaseMetadata {
    ModelRouteLeaseMetadata {
        task_profile_id: route.task_profile.id.clone(),
        binding_id: route.binding.id.clone(),
        model_definition_id: route.model.id.clone(),
        runtime_definition_id: route.runtime.id.clone(),
        model_fingerprint: route.model.model_fingerprint.clone(),
        runtime_fingerprint: fingerprint_runtime_definition(&route.runtime),
    }
}

fn runtime_state_error(details: String, correlation_id: &str) -> AppError {
    AppError {
        code: AppErrorCode::ModelStateAccessFailed,
        message: "Model platform runtime durumuna erişilemedi.".to_string(),
        recoverable: false,
        suggested_action: Some("Uygulamayı yeniden başlatın.".to_string()),
        technical_details: Some(details),
        correlation_id: correlation_id.to_string(),
    }
}

fn build_runtime_status(
    request: &ModelRuntimeRequest,
    profile: &ModelProfile,
    status: ModelStatus,
) -> ModelRuntimeStatus {
    let config_complete = status.server_path_exists
        && status.model_path_exists
        && (!request.requires_mmproj || status.mmproj_path_exists);
    let host = profile.host.clone();
    let port = profile.port;
    let autostart_available = status.can_start_from_app && config_complete;
    let state = if !config_complete {
        ModelRuntimeState::ConfigMissing
    } else if matches!(
        status.last_error.as_ref().map(|error| &error.code),
        Some(AppErrorCode::ModelProcessUnverified | AppErrorCode::ModelProcessIdentityMismatch)
    ) {
        ModelRuntimeState::Unverified
    } else if status.server_running && status.health_ok {
        ModelRuntimeState::Healthy
    } else if status.server_running && !status.health_ok && !status.started_by_app {
        ModelRuntimeState::PortBlocked
    } else if status.started_by_app && !status.health_ok {
        ModelRuntimeState::Starting
    } else if status.server_running && !status.health_ok {
        ModelRuntimeState::Unhealthy
    } else if matches!(
        status.last_error.as_ref().map(|err| &err.code),
        Some(AppErrorCode::ModelPortAlreadyInUse)
    ) {
        ModelRuntimeState::PortBlocked
    } else if status.can_start_from_app {
        ModelRuntimeState::Stopped
    } else {
        ModelRuntimeState::Failed
    };

    let mut message = if status.server_running && status.health_ok {
        "Model hazır.".to_string()
    } else if let Some(reason) = status.start_disabled_reason.clone() {
        reason
    } else if let Some(error) = status.last_error.as_ref() {
        error.message.clone()
    } else if !config_complete {
        "Model yapılandırması eksik.".to_string()
    } else {
        "Model durumu hazır değil.".to_string()
    };

    if matches!(request.capability, ModelCapability::Vision) && !request.requires_mmproj {
        message.push_str(" Vision için mmproj zorunlu değil olarak işaretlendi.");
    }

    ModelRuntimeStatus {
        health_ok: status.health_ok,
        state,
        managed_pid: status.managed_process_pid,
        host,
        port,
        port_listening: status.server_running,
        port_health_ok: status.health_ok,
        llama_server_binary_exists: status.server_path_exists,
        model_file_exists: status.model_path_exists,
        mmproj_file_exists: status.mmproj_path_exists,
        config_complete,
        autostart_available,
        message,
        active_lease_count: 0,
        draining: false,
        oldest_lease_age_seconds: None,
        lease_operation_kinds: Vec::new(),
    }
}

fn normalize_model_error(mut error: AppError) -> AppError {
    error.code = match error.code {
        AppErrorCode::ModelServerPathMissing => AppErrorCode::ModelBinaryMissing,
        AppErrorCode::ModelModelPathMissing => AppErrorCode::ModelFileMissing,
        AppErrorCode::ModelMmprojPathMissing => AppErrorCode::ModelMmprojMissing,
        AppErrorCode::ModelPortAlreadyInUse => AppErrorCode::ModelPortBlocked,
        AppErrorCode::ModelServerStartFailed => AppErrorCode::ModelStartFailed,
        AppErrorCode::ModelServerReadyTimeout => AppErrorCode::ModelStartTimeout,
        other => other,
    };
    error
}

fn read_log_lines(path: &std::path::Path, line_count: usize) -> Vec<String> {
    if !path.exists() {
        return vec![format!(
            "Log file does not exist at {}",
            path.to_string_lossy()
        )];
    }
    match std::fs::File::open(path) {
        Ok(file) => {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(file);
            let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
            let len = lines.len();
            if len <= line_count {
                lines
            } else {
                lines[len - line_count..].to_vec()
            }
        }
        Err(error) => vec![format!("Failed to open log file: {error}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{ModelMode, ModelProfile, ModelRuntimePreset, ModelStatus};

    fn sample_profile() -> ModelProfile {
        ModelProfile {
            id: "gemma".to_string(),
            display_name: "Gemma".to_string(),
            mode: ModelMode::Managed,
            server_path: "/tmp/llama-server".to_string(),
            model_path: "/tmp/model.gguf".to_string(),
            mmproj_path: "/tmp/model.mmproj".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            base_url: "http://127.0.0.1:8080".to_string(),
            runtime_preset: ModelRuntimePreset::Standard,
            privacy_mode: PrivacyMode::StrictLocal,
        }
    }

    #[test]
    fn step_name_matches_use_case() {
        assert_eq!(
            ModelUseCase::StudentAnswerOcr.step_name(),
            "student_answer_ocr"
        );
    }

    #[test]
    fn speaking_profiles_map_to_distinct_platform_tasks() {
        assert_eq!(
            ModelUseCase::GeneralText.platform_task(Some("speaking_transcript_cleanup_12b")),
            ModelTaskKind::SpeakingTranscriptCleanup
        );
        assert_eq!(
            ModelUseCase::SpeakingEvaluation.platform_task(Some("speaking_rubric_evaluation_12b")),
            ModelTaskKind::SpeakingEvaluation
        );
    }

    #[test]
    fn speaking_evaluation_is_classified_as_student_data() {
        assert!(ModelUseCase::SpeakingEvaluation.contains_student_data());
        assert!(ModelUseCase::SpeakingTranscriptCleanup.contains_student_data());
    }

    #[test]
    fn runtime_status_marks_missing_config() {
        let request = ModelRuntimeRequest {
            use_case: ModelUseCase::QuestionTextExtraction,
            capability: ModelCapability::Text,
            requires_mmproj: true,
            timeout_seconds: 30,
        };
        let status = ModelStatus {
            server_path_exists: false,
            model_path_exists: false,
            mmproj_path_exists: false,
            ..Default::default()
        };
        let runtime = build_runtime_status(&request, &sample_profile(), status);
        assert_eq!(runtime.state, ModelRuntimeState::ConfigMissing);
        assert!(!runtime.config_complete);
    }

    #[test]
    fn runtime_status_marks_healthy_when_server_is_ready() {
        let request = ModelRuntimeRequest {
            use_case: ModelUseCase::GeneralText,
            capability: ModelCapability::Text,
            requires_mmproj: false,
            timeout_seconds: 30,
        };
        let status = ModelStatus {
            server_path_exists: true,
            model_path_exists: true,
            mmproj_path_exists: true,
            server_running: true,
            health_ok: true,
            can_start_from_app: false,
            ..Default::default()
        };
        let runtime = build_runtime_status(&request, &sample_profile(), status);
        assert_eq!(runtime.state, ModelRuntimeState::Healthy);
        assert!(runtime.health_ok);
    }
}

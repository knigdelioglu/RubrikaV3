use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{
    build_model_server_args, preview_model_server_args, ManagedModelProcess,
    ManagedProcessIdentity, ModelMode, ModelProfile, ModelServerArgsPreview, ModelStatus,
    ModelSuggestedAction, SupportFlags,
};
use crate::domain::model_platform::fingerprint_runtime_definition;
use crate::platform::file_access::atomic_write;
use crate::platform::paths::{app_log_dir, model_server_log_path};
use crate::platform::process_inspector::{fingerprint, ProcessInspector, SystemProcessInspector};
use crate::services::llama_cpp_runtime_adapter::{
    InferenceRuntimeAdapter, LlamaCppRuntimeAdapter, RuntimeLaunchSpec,
};
use crate::services::llama_server_gateway::{validate_base_url_for_privacy, LlamaServerGateway};
use crate::services::model_config_service::ModelConfigService;
use crate::services::model_gateway::ModelGateway;
use crate::services::platform_launch_registry;
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{TcpListener, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedManagedProcess {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    metadata: Option<ManagedModelProcess>,
    #[serde(default)]
    processes: Vec<ManagedModelProcess>,
}

#[derive(Debug, Clone)]
struct LeaseRecord {
    runtime_instance_id: String,
    profile_id: String,
    consumer_id: String,
    job_id: Option<String>,
    operation_kind: String,
    acquired_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Default)]
struct LeaseRegistry {
    runtime_instance_id: Option<String>,
    profile_id: Option<String>,
    profile_fingerprint: Option<String>,
    leases: HashMap<String, LeaseRecord>,
    draining: bool,
    unexpected_exit_count: u64,
    idle_generation: u64,
}

#[derive(Clone)]
pub struct ModelProcessManager {
    config_service: ModelConfigService,
    gateway: Arc<LlamaServerGateway>,
    runtime: Arc<Mutex<Vec<ManagedModelProcess>>>,
    persisted_state_path: PathBuf,
    process_handle: Arc<tokio::sync::Mutex<Option<tokio::process::Child>>>,
    startup_lock: Arc<tokio::sync::Mutex<()>>,
    lease_registry: Arc<Mutex<LeaseRegistry>>,
    process_inspector: Arc<dyn ProcessInspector>,
    idle_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct RuntimeLeaseGrant {
    pub lease_id: String,
    pub runtime_instance_id: String,
    pub profile_id: String,
    pub profile_fingerprint: String,
    pub model_fingerprint: Option<String>,
    pub base_url: String,
    pub correlation_id: String,
    pub active_lease_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeLeaseDiagnostics {
    pub active_lease_count: usize,
    pub oldest_lease_age_seconds: Option<i64>,
    pub operation_kinds: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartModelServerOutput {
    pub started: bool,
    pub mode: ModelMode,
    pub pid: Option<u32>,
    pub base_url: String,
    pub log_path: String,
    pub health_ok: bool,
    pub message: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopModelServerOutput {
    pub stopped: bool,
    pub draining: bool,
    pub active_lease_count: usize,
    pub message: String,
}

impl ModelProcessManager {
    pub fn new(config_service: ModelConfigService, gateway: Arc<LlamaServerGateway>) -> Self {
        Self::new_with_state_path(config_service, gateway, model_process_state_path())
    }

    pub fn new_with_state_path(
        config_service: ModelConfigService,
        gateway: Arc<LlamaServerGateway>,
        persisted_state_path: PathBuf,
    ) -> Self {
        Self::new_with_inspector(
            config_service,
            gateway,
            persisted_state_path,
            Arc::new(SystemProcessInspector),
            Duration::from_secs(30),
        )
    }

    pub fn new_with_inspector(
        config_service: ModelConfigService,
        gateway: Arc<LlamaServerGateway>,
        persisted_state_path: PathBuf,
        process_inspector: Arc<dyn ProcessInspector>,
        idle_timeout: Duration,
    ) -> Self {
        let mut runtime = restore_runtime_state(&persisted_state_path);
        for process in &mut runtime {
            process.unverified = true;
        }
        Self {
            config_service,
            gateway,
            runtime: Arc::new(Mutex::new(runtime)),
            persisted_state_path,
            process_handle: Arc::new(tokio::sync::Mutex::new(None)),
            startup_lock: Arc::new(tokio::sync::Mutex::new(())),
            lease_registry: Arc::new(Mutex::new(LeaseRegistry::default())),
            process_inspector,
            idle_timeout,
        }
    }

    pub async fn get_model_status(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelStatus, AppError> {
        self.build_status(profile_id, false).await
    }

    pub async fn probe_model_server(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelStatus, AppError> {
        self.build_status(profile_id, true).await
    }

    pub async fn start_server(
        &self,
        profile_id: Option<&str>,
    ) -> Result<StartModelServerOutput, AppError> {
        self.start_server_with_timeout(profile_id, Duration::from_secs(180))
            .await
    }

    pub async fn start_server_with_timeout(
        &self,
        profile_id: Option<&str>,
        timeout: Duration,
    ) -> Result<StartModelServerOutput, AppError> {
        let _startup_guard = self.startup_lock.lock().await;
        self.start_server_with_timeout_locked(profile_id, timeout)
            .await
    }

    async fn start_server_with_timeout_locked(
        &self,
        profile_id: Option<&str>,
        timeout: Duration,
    ) -> Result<StartModelServerOutput, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        self.require_managed(&profile)?;
        validate_base_url_for_privacy(&profile.base_url, profile.privacy_mode)?;
        self.gateway.configure_privacy(profile.privacy_mode)?;
        self.recover_persisted_runtime(&profile).await?;
        if self.is_draining()? {
            return Err(model_error(
                AppErrorCode::ModelRuntimeDraining,
                "Yerel model işlemlerin bitmesi beklenirken yeniden başlatılamaz.",
                Some(format!("profile_id={}", profile.id)),
                Some("Mevcut işlemlerin bitmesini bekleyin.".to_string()),
            ));
        }

        if self.is_managed_profile_running(&profile.id)? {
            let status = self.build_status(Some(&profile.id), false).await?;
            return Ok(StartModelServerOutput {
                started: false,
                mode: ModelMode::Managed,
                pid: status.managed_process_pid,
                base_url: profile.base_url.clone(),
                log_path: model_server_log_path(&profile.id)
                    .to_string_lossy()
                    .to_string(),
                health_ok: status.health_ok,
                message: "Model sunucusu zaten RubrikaV3 tarafından başlatılmış.".to_string(),
            });
        }

        if let Some(running_profile) = self.running_profile_id()? {
            return Err(model_error(
                AppErrorCode::ModelPortAlreadyInUse,
                "Başka bir yerel model runtime'ı şu anda RubrikaV3 tarafından kullanılıyor.",
                Some(running_profile),
                Some("Etkin model işi tamamlandığında tekrar deneyin.".to_string()),
            ));
        }

        self.require_paths(&profile)?;

        if self.is_port_in_use(&profile.host, profile.port)? {
            let status = self.gateway.health_status(&profile.base_url).await?;
            if status.server_running && status.health_ok {
                return Ok(StartModelServerOutput {
                    started: false,
                    mode: ModelMode::Managed,
                    pid: None,
                    base_url: profile.base_url.clone(),
                    log_path: model_server_log_path(&profile.id)
                        .to_string_lossy()
                        .to_string(),
                    health_ok: true,
                    message: "Port kullanımda; mevcut model sunucusu harici olarak kullanılabilir."
                        .to_string(),
                });
            }

            return Err(model_error(
                AppErrorCode::ModelPortAlreadyInUse,
                &format!("{} portu başka bir süreç tarafından kullanılıyor.", profile.port),
                Some(profile.base_url.clone()),
                Some("RubrikaV3 başka bir süreci kapatmaz.".to_string()),
            ));
        }

        let help_output = self.run_help(&profile.server_path).await?;
        let support_flags = SupportFlags::from_help_output(&help_output);
        let launch_spec = launch_spec_for_profile(&profile, &support_flags)?;
        let args = launch_spec.args.clone();
        let log_path = model_server_log_path(&profile.id);
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                self.io_error(
                    AppErrorCode::ModelServerStartFailed,
                    "Log dizini oluşturulamadı.",
                    err,
                )
            })?;
        }

        let mut command = Command::new(&launch_spec.command);
        command
            .args(&args)
            .current_dir(
                PathBuf::from(&launch_spec.command)
                    .parent()
                    .map(|path| path.to_path_buf())
                    .unwrap_or_else(|| profile_workdir(&profile)),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env(
                "LLAMA_ARG_FLASH_ATTN",
                if launch_spec.requires_mmproj { "off" } else { "auto" },
            );
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|err| {
            model_error(
                AppErrorCode::ModelServerStartFailed,
                "llama-server başlatılamadı.",
                Some(err.to_string()),
                Some("Binary yolunu ve izinleri kontrol edin.".to_string()),
            )
        })?;

        let pid = child.id().ok_or_else(|| {
            model_error(
                AppErrorCode::ModelRuntimeStartFailed,
                "Model sürecinin kimliği alınamadı.",
                None,
                Some("Model sunucusunu yeniden başlatmayı deneyin.".to_string()),
            )
        })?;
        let runtime_instance_id = Uuid::new_v4().to_string();
        let runtime_profile_fingerprint = runtime_profile_fingerprint(&profile);
        let identity = match self.capture_process_identity(
            pid,
            &profile,
            &args,
            &runtime_profile_fingerprint,
            &runtime_instance_id,
        ) {
            Ok(identity) => identity,
            Err(error) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(error);
            }
        };
        let metadata = ManagedModelProcess {
            pid: Some(pid),
            started_by_app: true,
            profile_id: profile.id.clone(),
            base_url: profile.base_url.clone(),
            log_path: log_path.clone(),
            started_at: Some(Utc::now()),
            identity: Some(identity),
            runtime_instance_id: Some(runtime_instance_id.clone()),
            runtime_profile_fingerprint: Some(runtime_profile_fingerprint.clone()),
            unverified: false,
        };
        if let Err(err) = self.attach_log_forwarders(&mut child, &log_path).await {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(err);
        }

        {
            let mut runtime = self
                .runtime
                .lock()
                .map_err(|err| crate::domain::errors::AppError {
                    code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                    message: "Model durumuna erişilemedi.".to_string(),
                    recoverable: false,
                    suggested_action: Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
                    technical_details: Some(format!("Mutex poison error: {}", err)),
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                })?;
            runtime.push(metadata.clone());
        }
        if let Err(err) = self.persist_runtime_state() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            self.remove_runtime_profile(&profile.id)?;
            return Err(err);
        }

        *self.process_handle.lock().await = Some(child);
        self.spawn_exit_watcher(pid, runtime_instance_id.clone());

        match self.wait_until_ready(&profile, timeout).await {
            Ok(probe) => Ok(StartModelServerOutput {
                started: true,
                mode: ModelMode::Managed,
                pid: Some(pid),
                base_url: profile.base_url.clone(),
                log_path: log_path.to_string_lossy().to_string(),
                health_ok: probe.health_ok,
                message: "Model sunucusu başarıyla başlatıldı.".to_string(),
            }),
            Err(err) => {
                let _ = self.stop_startup_child().await;
                let _ = self.remove_runtime_profile(&profile.id);
                let _ = self.persist_runtime_state();
                let tail = read_log_tail(&log_path, 80).unwrap_or_default();
                Err(with_tail(err, tail))
            }
        }
    }

    pub async fn stop_server(
        &self,
        profile_id: Option<&str>,
    ) -> Result<StopModelServerOutput, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        let _startup_guard = self.startup_lock.lock().await;
        self.recover_persisted_runtime(&profile).await?;

        let active_lease_count = self.active_lease_count()?;
        if active_lease_count > 0 {
            self.set_draining(true)?;
            return Ok(StopModelServerOutput {
                stopped: false,
                draining: true,
                active_lease_count,
                message: format!(
                    "Yerel model şu anda {active_lease_count} işlem tarafından kullanılıyor. İşlemler bitince durdurulacak."
                ),
            });
        }

        let metadata = self.runtime_metadata(&profile.id)?;
        let Some(metadata) = metadata else {
            return Ok(StopModelServerOutput {
                stopped: false,
                draining: false,
                active_lease_count: 0,
                message: "Durdurulacak yönetilen model süreci bulunamadı.".to_string(),
            });
        };

        if !metadata.started_by_app || metadata.unverified {
            return Err(model_error(
                AppErrorCode::ModelProcessUnverified,
                "Yerel model süreci güvenli biçimde doğrulanamadı; kapatılmadı.",
                Some(format!("profile_id={}; pid={:?}", profile.id, metadata.pid)),
                Some("Tanılama ayrıntılarını kontrol edin ve süreci işletim sistemi üzerinden yönetin.".to_string()),
            ));
        }

        if metadata.identity.is_none() {
            return Ok(StopModelServerOutput {
                stopped: false,
                draining: false,
                active_lease_count: 0,
                message: "Model sürecinin güçlü kimliği bulunamadığı için kapatılmadı.".to_string(),
            });
        }

        self.stop_current_process(&metadata).await?;
        self.remove_runtime_profile(&profile.id)?;
        self.persist_runtime_state()?;
        self.reset_lease_registry()?;
        Ok(StopModelServerOutput {
            stopped: true,
            draining: false,
            active_lease_count: 0,
            message: "RubrikaV3 tarafından başlatılan doğrulanmış süreç güvenle durduruldu."
                .to_string(),
        })
    }

    pub async fn set_mode(
        &self,
        profile_id: Option<&str>,
        mode: ModelMode,
    ) -> Result<ModelStatus, AppError> {
        self.config_service.set_mode(profile_id, mode)?;
        self.build_status(profile_id, false).await
    }

    pub async fn reset_profile(&self) -> Result<ModelStatus, AppError> {
        let profile = self.config_service.reset_active_profile()?;
        self.build_status(Some(&profile.id), false).await
    }

    pub async fn preview_args(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelServerArgsPreview, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        let help_output = self.run_help(&profile.server_path).await?;
        if platform_launch_registry::get(&profile.id).is_some() {
            let support_flags = SupportFlags::from_help_output(&help_output);
            let launch_spec = launch_spec_for_profile(&profile, &support_flags)?;
            return Ok(ModelServerArgsPreview {
                profile_id: profile.id.clone(),
                display_name: profile.display_name.clone(),
                mode: profile.mode,
                base_url: launch_spec.base_url,
                command: launch_spec.command,
                args: launch_spec.args,
                supported_flags: vec![],
                unsupported_flags: vec![],
                log_path: model_server_log_path(&profile.id),
            });
        }
        let mut preview = preview_model_server_args(&profile, &help_output)?;
        preview.log_path = model_server_log_path(&profile.id);
        Ok(preview)
    }

    async fn build_status(
        &self,
        profile_id: Option<&str>,
        probe_completion: bool,
    ) -> Result<ModelStatus, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        self.gateway.configure_privacy(profile.privacy_mode)?;
        self.recover_persisted_runtime(&profile).await?;
        let log_path = model_server_log_path(&profile.id);
        let server_path_exists = self.path_exists(&profile.server_path);
        let model_path_exists = self.path_exists(&profile.model_path);
        let mmproj_path_exists = !profile.requires_mmproj() || self.path_exists(&profile.mmproj_path);
        let runtime = self
            .runtime
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model durumuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
                technical_details: Some(format!("Mutex poison error: {}", err)),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })?
            .iter()
            .find(|metadata| metadata.profile_id == profile.id)
            .cloned();

        let identity_verified = runtime
            .as_ref()
            .map(|metadata| self.verify_process_identity(metadata, &profile).unwrap_or(false))
            .unwrap_or(false);
        let runtime_unverified = runtime
            .as_ref()
            .map(|metadata| metadata.unverified || !identity_verified)
            .unwrap_or(false);
        let mut status = ModelStatus {
            profile_id: profile.id.clone(),
            display_name: profile.display_name.clone(),
            mode: profile.mode.clone(),
            base_url: profile.base_url.clone(),
            server_path_exists,
            model_path_exists,
            mmproj_path_exists,
            server_running: false,
            health_ok: false,
            completion_probe_ok: false,
            health_verified_at: None,
            completion_probe_verified_at: None,
            privacy_mode: profile.privacy_mode,
            privacy_blocked: false,
            privacy_block_reason: None,
            model_fingerprint: model_file_fingerprint(&profile),
            managed_process_pid: runtime.as_ref().and_then(|metadata| metadata.pid),
            started_by_app: runtime
                .as_ref()
                .map(|metadata| metadata.started_by_app && identity_verified)
                .unwrap_or(false),
            active_lease_count: 0,
            draining: false,
            log_path: Some(log_path.clone()),
            last_error: None,
            warnings: vec![],
            can_start_from_app: false,
            can_stop_from_app: false,
            start_requires_mode_change: false,
            start_disabled_reason: None,
            suggested_actions: vec![],
        };
        status.active_lease_count = self.active_lease_count()?;
        status.draining = self.is_draining()?;

        if let Err(error) = validate_base_url_for_privacy(&profile.base_url, profile.privacy_mode) {
            status.privacy_blocked = true;
            status.privacy_block_reason = Some(error.message.clone());
            status.warnings.push(error.message.clone());
            status.last_error = Some(error);
            status.can_start_from_app = false;
            status.can_stop_from_app = false;
            status.start_disabled_reason =
                Some("Model adresi Strict Local gizlilik politikasını karşılamıyor.".to_string());
            status.suggested_actions.push(ModelSuggestedAction {
                code: "use_managed_local_model".to_string(),
                label: "Yönetilen yerel model profilini seç".to_string(),
            });
            return Ok(status);
        }

        if runtime_unverified {
            status.warnings.push(
                "Kayıtlı model sürecinin kimliği doğrulanamadı; RubrikaV3 bu sürece dokunmaz."
                    .to_string(),
            );
            status.last_error = Some(model_error(
                AppErrorCode::ModelProcessUnverified,
                "Yerel model süreci güvenli biçimde doğrulanamadı.",
                Some(format!("profile_id={}; identity_verified=false", profile.id)),
                Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
            ));
        }

        if status.started_by_app {
            status.warnings.push("RubrikaV3 tarafından başlatıldı.".to_string());
            status.server_running = true;
        }

        if self.is_port_in_use(&profile.host, profile.port)? {
            status.server_running = true;
            if !status.started_by_app {
                status.warnings.push(
                    "Port başka bir süreç tarafından kullanılıyor; bu süreç harici görünüyor."
                        .to_string(),
                );
            }
        }

        if server_path_exists && status.server_running {
            if probe_completion {
                let probe_status = self.gateway.probe_status(&profile.base_url).await?;
                status.server_running = probe_status.server_running || status.server_running;
                status.health_ok = probe_status.health_ok;
                status.completion_probe_ok = probe_status.completion_probe_ok;
                status.health_verified_at = probe_status.health_verified_at;
                status.completion_probe_verified_at = probe_status.completion_probe_verified_at;
                if let Some(error) = probe_status.last_error {
                    status.last_error = Some(error);
                }
            } else {
                let health_status = self.gateway.health_status(&profile.base_url).await?;
                status.server_running = health_status.server_running || status.server_running;
                status.health_ok = health_status.health_ok;
                status.health_verified_at = health_status.health_verified_at;
                if let Some(error) = health_status.last_error {
                    status.last_error = Some(error);
                }
            }
        }

        if !server_path_exists {
            status.last_error = Some(model_error(
                AppErrorCode::ModelServerPathMissing,
                "llama-server binary bulunamadı.",
                Some(profile.server_path.clone()),
                None,
            ));
        } else if !model_path_exists {
            status.last_error = Some(model_error(
                AppErrorCode::ModelModelPathMissing,
                "Model dosyası bulunamadı.",
                Some(profile.model_path.clone()),
                None,
            ));
        } else if profile.requires_mmproj() && !mmproj_path_exists {
            status.last_error = Some(model_error(
                AppErrorCode::ModelMmprojPathMissing,
                "MMProj dosyası bulunamadı.",
                Some(profile.mmproj_path.clone()),
                None,
            ));
        } else if status.server_running && !status.health_ok && status.last_error.is_none() {
            status.last_error = Some(model_error(
                AppErrorCode::ModelHealthFailed,
                "Model sunucusuna sağlık yanıtı alınamadı.",
                Some(profile.base_url.clone()),
                Some("Sunucu uyumlu değilse harici olarak kapatılmaz.".to_string()),
            ));
        }

        if !status.server_running {
            status.warnings.push("Model sunucusu kapalı görünüyor.".to_string());
        }

        status.can_stop_from_app = status.started_by_app;

        if status.server_running {
            status.can_start_from_app = false;
            status.start_disabled_reason = Some("Model sunucusu zaten çalışıyor.".to_string());
        } else if !server_path_exists
            || !model_path_exists
            || (profile.requires_mmproj() && !mmproj_path_exists)
        {
            status.can_start_from_app = false;
            status.start_disabled_reason = Some("Model, server veya mmproj dosyası eksik.".to_string());
            status.suggested_actions.push(ModelSuggestedAction {
                code: "open_model_status_page".to_string(),
                label: "Model durumunu aç".to_string(),
            });
        } else {
            status.can_start_from_app = true;
            if status.mode == ModelMode::External {
                status.start_requires_mode_change = true;
                status.suggested_actions.push(ModelSuggestedAction {
                    code: "switch_to_managed_and_start".to_string(),
                    label: "Yönetilen moda al ve modeli başlat".to_string(),
                });
                status.suggested_actions.push(ModelSuggestedAction {
                    code: "open_model_status_page".to_string(),
                    label: "Model durumunu aç".to_string(),
                });
            } else {
                status.start_requires_mode_change = false;
                status.suggested_actions.push(ModelSuggestedAction {
                    code: "start_model_server".to_string(),
                    label: "Model Server’ı Başlat".to_string(),
                });
            }
        }

        Ok(status)
    }

    async fn wait_until_ready(
        &self,
        profile: &ModelProfile,
        deadline: Duration,
    ) -> Result<ModelStatus, AppError> {
        let start = Instant::now();
        loop {
            if start.elapsed() >= deadline {
                return Err(model_error(
                    AppErrorCode::ModelServerReadyTimeout,
                    "Model sunucusu zamanında hazır olmadı.",
                    Some(format!("profile_id={}", profile.id)),
                    Some("Log dosyasını inceleyin.".to_string()),
                ));
            }

            let runtime_pid = self
                .runtime
                .lock()
                .map_err(|err| AppError {
                    code: AppErrorCode::ModelStateAccessFailed,
                    message: "Model çalışma zamanı durumuna erişilemedi.".to_string(),
                    recoverable: false,
                    suggested_action: Some("Uygulamayı yeniden başlatın.".to_string()),
                    technical_details: Some(format!("Mutex lock failed: {}", err)),
                    correlation_id: Uuid::new_v4().to_string(),
                })?
                .iter()
                .find(|metadata| metadata.profile_id == profile.id)
                .and_then(|metadata| metadata.pid);
            if runtime_pid.is_some() && !self.is_pid_running(runtime_pid)? {
                return Err(model_error(
                    AppErrorCode::ModelServerStartFailed,
                    "Model süreci hazır olmadan kapandı.",
                    Some(format!("profile_id={}", profile.id)),
                    Some("Log dosyasını kontrol edin.".to_string()),
                ));
            }

            let remaining = deadline.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(model_error(
                    AppErrorCode::ModelServerReadyTimeout,
                    "Model sunucusu zamanında hazır olmadı.",
                    Some(format!("profile_id={}", profile.id)),
                    Some("Log dosyasını inceleyin.".to_string()),
                ));
            }
            let health = match timeout(remaining, self.gateway.health_status(&profile.base_url)).await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(model_error(
                        AppErrorCode::ModelServerReadyTimeout,
                        "Model sunucusu zamanında hazır olmadı.",
                        Some(format!("profile_id={}", profile.id)),
                        Some("Log dosyasını inceleyin.".to_string()),
                    ));
                }
            };
            if health.server_running && health.health_ok {
                return Ok(health);
            }

            let elapsed = start.elapsed();
            let remaining = deadline.saturating_sub(elapsed);
            if remaining.is_zero() {
                return Err(model_error(
                    AppErrorCode::ModelServerReadyTimeout,
                    "Model sunucusu zamanında hazır olmadı.",
                    Some(format!("profile_id={}", profile.id)),
                    Some("Log dosyasını inceleyin.".to_string()),
                ));
            }
            let backoff = Duration::from_millis(
                50_u64.saturating_mul(1_u64 << ((elapsed.as_millis() / 250) as u32).min(4)),
            );
            sleep(backoff.min(remaining)).await;
        }
    }

    fn require_managed(&self, profile: &ModelProfile) -> Result<(), AppError> {
        if profile.mode == ModelMode::Managed {
            return Ok(());
        }
        Err(model_error(
            AppErrorCode::ModelProfileNotManaged,
            "Bu profil yönetilen modda değil.",
            Some(format!("profile_id={}", profile.id)),
            Some("Önce Model Laboratuvarı'ndan profili managed moduna alın.".to_string()),
        ))
    }

    fn require_paths(&self, profile: &ModelProfile) -> Result<(), AppError> {
        if !self.path_exists(&profile.server_path) {
            return Err(model_error(
                AppErrorCode::ModelServerPathMissing,
                "llama-server binary bulunamadı.",
                Some(profile.server_path.clone()),
                Some("Sunucu binary yolunu kontrol edin.".to_string()),
            ));
        }
        if !self.path_exists(&profile.model_path) {
            return Err(model_error(
                AppErrorCode::ModelModelPathMissing,
                "Model dosyası bulunamadı.",
                Some(profile.model_path.clone()),
                Some("GGUF model yolunu kontrol edin.".to_string()),
            ));
        }
        if profile.requires_mmproj() && !self.path_exists(&profile.mmproj_path) {
            return Err(model_error(
                AppErrorCode::ModelMmprojPathMissing,
                "MMProj dosyası bulunamadı.",
                Some(profile.mmproj_path.clone()),
                Some("MMProj dosya yolunu kontrol edin.".to_string()),
            ));
        }
        Ok(())
    }

    fn path_exists(&self, path: &str) -> bool {
        !path.trim().is_empty() && PathBuf::from(path).exists()
    }

    fn is_port_in_use(&self, host: &str, port: u16) -> Result<bool, AppError> {
        let mut addrs = (host, port).to_socket_addrs().map_err(|err| {
            self.io_error(
                AppErrorCode::ModelServerStartFailed,
                "Host adı çözümlenemedi.",
                err,
            )
        })?;
        let Some(addr) = addrs.next() else {
            return Err(model_error(
                AppErrorCode::ModelServerStartFailed,
                "Port adresi oluşturulamadı.",
                Some(format!("{}:{}", host, port)),
                None,
            ));
        };
        Ok(TcpListener::bind(addr).is_err())
    }

    fn is_pid_running(&self, pid: Option<u32>) -> Result<bool, AppError> {
        let Some(pid) = pid else {
            return Ok(false);
        };
        self.process_inspector
            .inspect(pid)
            .map(|snapshot| snapshot.is_some())
            .map_err(|error| {
                model_error(
                    AppErrorCode::ModelProcessUnverified,
                    "Model süreci güvenli biçimde incelenemedi.",
                    Some(error),
                    Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
                )
            })
    }

    fn is_managed_profile_running(&self, profile_id: &str) -> Result<bool, AppError> {
        let profile = self.config_service.get_profile(Some(profile_id))?;
        let Some(metadata) = self.runtime_metadata(profile_id)? else {
            return Ok(false);
        };
        Ok(metadata.started_by_app
            && !metadata.unverified
            && self.verify_process_identity(&metadata, &profile).unwrap_or(false))
    }

    fn running_profile_id(&self) -> Result<Option<String>, AppError> {
        let runtime = self.runtime.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
            )
        })?;
        for metadata in runtime.iter() {
            let Ok(profile) = self.config_service.get_profile(Some(&metadata.profile_id)) else {
                continue;
            };
            if metadata.started_by_app
                && !metadata.unverified
                && self.verify_process_identity(metadata, &profile).unwrap_or(false)
            {
                return Ok(Some(metadata.profile_id.clone()));
            }
        }
        Ok(None)
    }

    fn runtime_metadata(&self, profile_id: &str) -> Result<Option<ManagedModelProcess>, AppError> {
        self.runtime
            .lock()
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
                )
            })
            .map(|runtime| {
                runtime
                    .iter()
                    .find(|metadata| metadata.profile_id == profile_id)
                    .cloned()
            })
    }

    fn capture_process_identity(
        &self,
        pid: u32,
        profile: &ModelProfile,
        args: &[String],
        profile_fingerprint: &str,
        runtime_instance_id: &str,
    ) -> Result<ManagedProcessIdentity, AppError> {
        let snapshot = self
            .process_inspector
            .inspect(pid)
            .map_err(|error| {
                model_error(
                    AppErrorCode::ModelProcessUnverified,
                    "Model süreci başlatıldı ancak kimliği doğrulanamadı.",
                    Some(error),
                    Some("Model sunucusunu yeniden başlatmayı deneyin.".to_string()),
                )
            })?
            .ok_or_else(|| {
                model_error(
                    AppErrorCode::ModelProcessUnverified,
                    "Model süreci başlatıldı ancak kimliği doğrulanamadı.",
                    Some(format!("pid={pid}; process_not_found=true")),
                    Some("Model sunucusunu yeniden başlatmayı deneyin.".to_string()),
                )
            })?;
        if snapshot.owner_uid != self.process_inspector.current_uid() {
            return Err(model_error(
                AppErrorCode::ModelProcessUnverified,
                "Model süreci mevcut kullanıcıya ait olmadığı için doğrulanamadı.",
                Some(format!(
                    "pid={pid}; owner_uid={}; current_uid={}",
                    snapshot.owner_uid,
                    self.process_inspector.current_uid()
                )),
                Some("Model sunucusunu mevcut kullanıcıyla yeniden başlatmayı deneyin.".to_string()),
            ));
        }
        let executable_fingerprint = fingerprint(&[snapshot
            .canonical_executable_path
            .to_string_lossy()
            .to_string()]);
        Ok(ManagedProcessIdentity {
            pid,
            owner_uid: snapshot.owner_uid,
            process_start_time_unix_ms: snapshot.process_start_time_unix_ms,
            canonical_executable_path: snapshot.canonical_executable_path,
            executable_fingerprint,
            argv_fingerprint: snapshot
                .argv_fingerprint
                .unwrap_or_else(|| fingerprint(args)),
            expected_port: profile.port,
            runtime_profile_fingerprint: profile_fingerprint.to_string(),
            launch_instance_id: runtime_instance_id.to_string(),
            launched_at: Utc::now(),
        })
    }

    fn verify_process_identity(
        &self,
        metadata: &ManagedModelProcess,
        profile: &ModelProfile,
    ) -> Result<bool, String> {
        let Some(identity) = metadata.identity.as_ref() else {
            return Ok(false);
        };
        if identity.pid == 0 || identity.pid != metadata.pid.unwrap_or_default() {
            return Ok(false);
        }
        let Some(snapshot) = self
            .process_inspector
            .inspect(identity.pid)
            .map_err(|error| error.to_string())?
        else {
            return Ok(false);
        };
        let c_pid = snapshot.pid != identity.pid;
        let c_uid = snapshot.owner_uid != self.process_inspector.current_uid();
        let c_owner = snapshot.owner_uid != identity.owner_uid;
        let c_time = identity
            .process_start_time_unix_ms
            .abs_diff(snapshot.process_start_time_unix_ms)
            > 500;
        let is_wrapper_match = {
            let name_a = identity
                .canonical_executable_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let name_b = snapshot
                .canonical_executable_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let is_py_launcher = |name: &str| {
                name.contains("python")
                    || name.contains("Python")
                    || name == "env"
                    || name.ends_with(".py")
                    || name.ends_with(".sh")
            };
            is_py_launcher(name_a) && is_py_launcher(name_b)
        };
        let c_exe = snapshot.canonical_executable_path != identity.canonical_executable_path
            && !is_wrapper_match;
        let c_fp = fingerprint(&[snapshot
            .canonical_executable_path
            .to_string_lossy()
            .to_string()])
            != identity.executable_fingerprint
            && !is_wrapper_match;
        let c_port = identity.expected_port != profile.port;
        let c_prof = identity.runtime_profile_fingerprint != runtime_profile_fingerprint(profile);

        if c_pid || c_uid || c_owner || c_time || c_exe || c_fp || c_port || c_prof {
            return Ok(false);
        }
        if let Some(argv_fingerprint) = snapshot.argv_fingerprint {
            if argv_fingerprint != identity.argv_fingerprint && !is_wrapper_match {
                return Ok(false);
            }
        }
        self.process_inspector
            .process_owns_port(identity.pid, &profile.host, profile.port)
    }

    async fn recover_persisted_runtime(&self, profile: &ModelProfile) -> Result<(), AppError> {
        let Some(metadata) = self.runtime_metadata(&profile.id)? else {
            return Ok(());
        };
        let Some(identity) = metadata.identity.as_ref() else {
            self.mark_runtime_unverified(&profile.id)?;
            return Ok(());
        };
        let snapshot = match self.process_inspector.inspect(identity.pid) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.mark_runtime_unverified_with_detail(&profile.id, error)?;
                return Ok(());
            }
        };
        let Some(snapshot) = snapshot else {
            self.remove_runtime_profile(&profile.id)?;
            self.persist_runtime_state()?;
            return Ok(());
        };
        if self.verify_process_identity(&metadata, profile).unwrap_or(false) {
            self.mark_runtime_verified(&profile.id)?;
            return Ok(());
        }
        let port_owned = self
            .process_inspector
            .process_owns_port(identity.pid, &profile.host, profile.port)
            .unwrap_or(false);
        if port_owned {
            self.mark_runtime_unverified_with_detail(
                &profile.id,
                format!(
                    "persisted_identity_mismatch=true; pid={}; live_executable={}",
                    identity.pid,
                    snapshot.canonical_executable_path.display()
                ),
            )?;
        } else {
            self.remove_runtime_profile(&profile.id)?;
            self.persist_runtime_state()?;
        }
        Ok(())
    }

    fn mark_runtime_verified(&self, profile_id: &str) -> Result<(), AppError> {
        let mut runtime = self.runtime.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                None,
            )
        })?;
        if let Some(metadata) = runtime.iter_mut().find(|item| item.profile_id == profile_id) {
            metadata.unverified = false;
        }
        Ok(())
    }

    fn mark_runtime_unverified(&self, profile_id: &str) -> Result<(), AppError> {
        self.mark_runtime_unverified_with_detail(profile_id, "identity_unavailable".to_string())
    }

    fn mark_runtime_unverified_with_detail(
        &self,
        profile_id: &str,
        detail: String,
    ) -> Result<(), AppError> {
        let mut runtime = self.runtime.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model durumuna erişilemedi.".to_string(),
                Some(format!("Mutex poison error: {err}")),
                None,
            )
        })?;
        if let Some(metadata) = runtime.iter_mut().find(|item| item.profile_id == profile_id) {
            metadata.unverified = true;
        }
        drop(runtime);
        let _ = self.persist_runtime_state();
        let _ = detail;
        Ok(())
    }

    fn remove_runtime_profile(&self, profile_id: &str) -> Result<(), AppError> {
        let mut runtime = self.runtime.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
            )
        })?;
        runtime.retain(|metadata| metadata.profile_id != profile_id);
        Ok(())
    }

    async fn run_help(&self, server_path: &str) -> Result<String, AppError> {
        let output = Command::new(server_path)
            .arg("--help")
            .output()
            .await
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelServerStartFailed,
                    "llama-server yardım çıktısı alınamadı.",
                    Some(err.to_string()),
                    Some("Binary yolunu ve çalışma izinlerini kontrol edin.".to_string()),
                )
            })?;

        Ok(format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }

    async fn attach_log_forwarders(
        &self,
        child: &mut tokio::process::Child,
        log_path: &PathBuf,
    ) -> Result<(), AppError> {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .await
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelServerStartFailed,
                    "Log dosyası açılamadı.",
                    Some(err.to_string()),
                    Some("Log yolunu ve izinleri kontrol edin.".to_string()),
                )
            })?;
        let file = Arc::new(tokio::sync::Mutex::new(file));

        if let Some(stdout) = child.stdout.take() {
            spawn_log_reader(stdout, file.clone(), "stdout");
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_reader(stderr, file, "stderr");
        }

        Ok(())
    }

    async fn stop_current_process(&self, metadata: &ManagedModelProcess) -> Result<(), AppError> {
        let profile = self.config_service.get_profile(Some(&metadata.profile_id))?;
        let Some(identity) = metadata.identity.as_ref() else {
            return Err(model_error(
                AppErrorCode::ModelProcessUnverified,
                "Model sürecinin güçlü kimliği bulunamadığı için kapatılmadı.",
                Some(format!("profile_id={}", metadata.profile_id)),
                Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
            ));
        };
        if identity.pid == 0 || !self.verify_process_identity(metadata, &profile).unwrap_or(false) {
            return Err(model_error(
                AppErrorCode::ModelProcessIdentityMismatch,
                "Model süreci kimliği beklenen süreçle eşleşmiyor; sinyal gönderilmedi.",
                Some(format!("profile_id={}; pid={}", metadata.profile_id, identity.pid)),
                Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
            ));
        }

        let mut child_guard = self.process_handle.lock().await;
        if let Some(child) = child_guard.as_mut() {
            if child.id() != Some(identity.pid) {
                return Err(model_error(
                    AppErrorCode::ModelProcessIdentityMismatch,
                    "Model Child handle kimlik kaydıyla eşleşmiyor; sinyal gönderilmedi.",
                    Some(format!(
                        "expected_pid={}; actual_pid={:?}",
                        identity.pid,
                        child.id()
                    )),
                    Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
                ));
            }
            send_verified_signal(&self.process_inspector, metadata, &profile, libc::SIGTERM)?;
            let wait_result = timeout(Duration::from_secs(8), child.wait()).await;
            if wait_result.is_err() {
                if !self.verify_process_identity(metadata, &profile).unwrap_or(false) {
                    return Err(model_error(
                        AppErrorCode::ModelProcessIdentityMismatch,
                        "Force kill öncesi model süreci kimliği değişti; sinyal gönderilmedi.",
                        Some(format!("pid={}", identity.pid)),
                        Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
                    ));
                }
                send_verified_signal(&self.process_inspector, metadata, &profile, libc::SIGKILL)?;
                let _ = timeout(Duration::from_secs(2), child.wait()).await;
            }
            *child_guard = None;
            return Ok(());
        }
        drop(child_guard);

        send_verified_signal(&self.process_inspector, metadata, &profile, libc::SIGTERM)?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(8) {
            if self.process_inspector.inspect(identity.pid).ok().flatten().is_none() {
                return Ok(());
            }
            sleep(Duration::from_millis(250)).await;
        }
        if !self.verify_process_identity(metadata, &profile).unwrap_or(false) {
            return Err(model_error(
                AppErrorCode::ModelProcessIdentityMismatch,
                "Force kill öncesi model süreci kimliği değişti; sinyal gönderilmedi.",
                Some(format!("pid={}", identity.pid)),
                Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
            ));
        }
        send_verified_signal(&self.process_inspector, metadata, &profile, libc::SIGKILL)
    }

    async fn stop_startup_child(&self) -> Result<(), AppError> {
        let mut child_guard = self.process_handle.lock().await;
        if let Some(child) = child_guard.as_mut() {
            child.kill().await.map_err(|error| {
                model_error(
                    AppErrorCode::ModelRuntimeStartFailed,
                    "Başarısız model başlatma süreci temizlenemedi.",
                    Some(error.to_string()),
                    Some("Model sunucusunu yeniden başlatmayı deneyin.".to_string()),
                )
            })?;
            let _ = child.wait().await;
            *child_guard = None;
        }
        Ok(())
    }

    fn spawn_exit_watcher(&self, pid: u32, runtime_instance_id: String) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                let exited = {
                    let mut child_guard = manager.process_handle.lock().await;
                    let Some(child) = child_guard.as_mut() else {
                        return;
                    };
                    match child.try_wait() {
                        Ok(Some(_status)) => {
                            *child_guard = None;
                            true
                        }
                        Ok(None) => false,
                        Err(_) => true,
                    }
                };
                if exited {
                    let _ = manager.handle_runtime_exit(pid, &runtime_instance_id);
                    return;
                }
                sleep(Duration::from_millis(500)).await;
            }
        });
    }

    fn handle_runtime_exit(&self, pid: u32, runtime_instance_id: &str) -> Result<(), AppError> {
        {
            let mut runtime = self.runtime.lock().map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    None,
                )
            })?;
            runtime.retain(|metadata| {
                metadata.runtime_instance_id.as_deref() != Some(runtime_instance_id)
            });
        }
        let mut leases = self.lease_registry.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model lease durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                None,
            )
        })?;
        if leases.runtime_instance_id.as_deref() == Some(runtime_instance_id) {
            leases.unexpected_exit_count += 1;
            leases.leases.clear();
            leases.runtime_instance_id = None;
            leases.profile_id = None;
            leases.profile_fingerprint = None;
            leases.draining = false;
        }
        drop(leases);
        self.persist_runtime_state()?;
        let _ = pid;
        Ok(())
    }

    fn schedule_idle_shutdown(&self, runtime_instance_id: String, generation: u64) {
        let manager = self.clone();
        let delay = self
            .lease_registry
            .lock()
            .map(|registry| {
                if registry.draining {
                    Duration::from_millis(0)
                } else {
                    self.idle_timeout
                }
            })
            .unwrap_or(self.idle_timeout);
        tokio::spawn(async move {
            sleep(delay).await;
            let _startup_guard = manager.startup_lock.lock().await;
            let should_stop = manager
                .lease_registry
                .lock()
                .map(|registry| {
                    registry.runtime_instance_id.as_deref() == Some(&runtime_instance_id)
                        && registry.leases.is_empty()
                        && !registry.draining
                        && registry.idle_generation == generation
                })
                .unwrap_or(false);
            if !should_stop {
                return;
            }
            let metadata = manager.runtime.lock().ok().and_then(|runtime| {
                runtime
                    .iter()
                    .find(|item| item.runtime_instance_id.as_deref() == Some(&runtime_instance_id))
                    .cloned()
            });
            if let Some(metadata) = metadata {
                if manager.stop_current_process(&metadata).await.is_ok() {
                    let _ = manager.remove_runtime_profile(&metadata.profile_id);
                    let _ = manager.persist_runtime_state();
                }
            }
            let _ = manager.reset_lease_registry();
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn acquire_lease(
        &self,
        profile_id: Option<&str>,
        requires_mmproj: bool,
        timeout_seconds: u64,
        consumer_id: &str,
        job_id: Option<&str>,
        operation_kind: &str,
        correlation_id: &str,
    ) -> Result<RuntimeLeaseGrant, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        validate_base_url_for_privacy(&profile.base_url, profile.privacy_mode)?;
        self.gateway.configure_privacy(profile.privacy_mode)?;
        let model_fingerprint = model_file_fingerprint(&profile);
        if requires_mmproj && !profile.requires_mmproj() && profile.mmproj_path.trim().is_empty() {
            return Err(model_error(
                AppErrorCode::ModelMmprojMissing,
                "Bu model işlemi için mmproj dosyası gerekiyor.",
                Some(format!("profile_id={}", profile.id)),
                Some("Vision model profilini ve mmproj yolunu kontrol edin.".to_string()),
            ));
        }
        let profile_fingerprint = runtime_profile_fingerprint(&profile);
        let _startup_guard = self.startup_lock.lock().await;
        self.recover_persisted_runtime(&profile).await?;

        {
            let registry = self.lease_registry.lock().map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model lease durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    None,
                )
            })?;
            if registry.draining {
                return Err(model_error(
                    AppErrorCode::ModelRuntimeDraining,
                    "Yerel model işlemlerin bitmesi beklenirken yeni kullanım kabul etmiyor.",
                    Some(format!("active_leases={}", registry.leases.len())),
                    Some("Mevcut işlemlerin bitmesini bekleyin.".to_string()),
                ));
            }
            if registry
                .profile_fingerprint
                .as_deref()
                .is_some_and(|current| current != profile_fingerprint)
                && !registry.leases.is_empty()
            {
                return Err(model_error(
                    AppErrorCode::ModelRuntimeProfileBusy,
                    "Yerel model başka bir runtime profili tarafından kullanılıyor.",
                    Some(format!("requested_profile={}", profile.id)),
                    Some("Etkin model işlemi tamamlandıktan sonra tekrar deneyin.".to_string()),
                ));
            }
        }

        let current_runtime = self
            .lease_registry
            .lock()
            .ok()
            .and_then(|registry| registry.runtime_instance_id.clone());
        let current_profile = self
            .lease_registry
            .lock()
            .ok()
            .and_then(|registry| registry.profile_fingerprint.clone());
        if current_profile
            .as_deref()
            .is_some_and(|current| current != profile_fingerprint)
        {
            let metadata = self
                .runtime
                .lock()
                .ok()
                .and_then(|runtime| runtime.first().cloned());
            if let Some(metadata) = metadata {
                self.stop_current_process(&metadata).await?;
                self.remove_runtime_profile(&metadata.profile_id)?;
                self.persist_runtime_state()?;
            }
            self.reset_lease_registry()?;
        }

        let metadata = self.runtime_metadata(&profile.id)?;
        if metadata.as_ref().is_some_and(|item| item.unverified) {
            return Err(model_error(
                AppErrorCode::ModelProcessUnverified,
                "Kayıtlı model süreci güvenli biçimde doğrulanamadı.",
                Some(format!("profile_id={}", profile.id)),
                Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
            ));
        }

        let status = self.build_status(Some(&profile.id), false).await?;
        let usable_existing_runtime = status.health_ok
            && (metadata.is_none() || status.started_by_app || profile.mode == ModelMode::External);
        let mut ready_health_ok = status.health_ok;
        if !usable_existing_runtime {
            if profile.mode != ModelMode::Managed {
                return Err(model_error(
                    AppErrorCode::ModelServerNotRunning,
                    "Model sunucusu hazır değil.",
                    Some(format!(
                        "profile_id={}; consumer_id={consumer_id}",
                        profile.id
                    )),
                    Some("Model sunucusunu başlatın veya yönetilen moda geçin.".to_string()),
                ));
            }
            let start_result = self
                .start_server_with_timeout_locked(
                    Some(&profile.id),
                    Duration::from_secs(timeout_seconds),
                )
                .await
                .map_err(|mut error| {
                    if error.code == AppErrorCode::ModelServerReadyTimeout {
                        error.code = AppErrorCode::ModelRuntimeReadinessTimeout;
                    } else if error.code == AppErrorCode::ModelServerStartFailed {
                        error.code = AppErrorCode::ModelRuntimeStartFailed;
                    }
                    error
                })?;
            ready_health_ok = start_result.health_ok;
        }

        if !ready_health_ok {
            ready_health_ok = self
                .wait_until_ready(&profile, Duration::from_secs(timeout_seconds))
                .await?
                .health_ok;
        }
        if !ready_health_ok {
            return Err(model_error(
                AppErrorCode::ModelRuntimeReadinessTimeout,
                "Model sunucusu hazır olmadan lease verilemedi.",
                Some(format!(
                    "profile_id={}; runtime_instance={current_runtime:?}",
                    profile.id
                )),
                Some("Model durumunu ve sunucu loglarını kontrol edin.".to_string()),
            ));
        }

        let runtime_instance_id = self
            .runtime_metadata(&profile.id)?
            .and_then(|item| item.runtime_instance_id)
            .unwrap_or_else(|| format!("external-{}", Uuid::new_v4()));
        let lease_id = Uuid::new_v4().to_string();
        let active_lease_count = {
            let mut registry = self.lease_registry.lock().map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model lease durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    None,
                )
            })?;
            registry.runtime_instance_id = Some(runtime_instance_id.clone());
            registry.profile_id = Some(profile.id.clone());
            registry.profile_fingerprint = Some(profile_fingerprint.clone());
            registry.draining = false;
            registry.idle_generation = registry.idle_generation.wrapping_add(1);
            registry.leases.insert(
                lease_id.clone(),
                LeaseRecord {
                    runtime_instance_id: runtime_instance_id.clone(),
                    profile_id: profile.id.clone(),
                    consumer_id: consumer_id.to_string(),
                    job_id: job_id.map(str::to_string),
                    operation_kind: operation_kind.to_string(),
                    acquired_at: Utc::now(),
                },
            );
            registry.leases.len()
        };
        Ok(RuntimeLeaseGrant {
            lease_id,
            runtime_instance_id,
            profile_id: profile.id,
            profile_fingerprint,
            model_fingerprint,
            base_url: profile.base_url,
            correlation_id: correlation_id.to_string(),
            active_lease_count,
        })
    }

    pub async fn release_lease(
        &self,
        lease_id: &str,
        runtime_instance_id: &str,
    ) -> Result<(), AppError> {
        let (schedule, generation) = {
            let mut registry = self.lease_registry.lock().map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model lease durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    None,
                )
            })?;
            let Some(record) = registry.leases.get(lease_id) else {
                return Err(model_error(
                    AppErrorCode::ModelRuntimeLeaseAlreadyReleased,
                    "Model lease daha önce bırakılmış veya bulunamıyor.",
                    Some(format!("lease_id={lease_id}")),
                    None,
                ));
            };
            if record.runtime_instance_id != runtime_instance_id {
                return Err(model_error(
                    AppErrorCode::ModelRuntimeLeaseInvalid,
                    "Model lease farklı bir runtime örneğine uygulanamaz.",
                    Some(format!(
                        "lease_id={lease_id}; runtime_instance_id={runtime_instance_id}"
                    )),
                    None,
                ));
            }
            registry.leases.remove(lease_id);
            let schedule = registry.leases.is_empty();
            if schedule {
                registry.idle_generation = registry.idle_generation.wrapping_add(1);
            }
            (schedule, registry.idle_generation)
        };
        if schedule {
            self.schedule_idle_shutdown(runtime_instance_id.to_string(), generation);
        }
        Ok(())
    }

    pub fn active_lease_count(&self) -> Result<usize, AppError> {
        self.lease_registry
            .lock()
            .map(|registry| registry.leases.len())
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model lease durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    None,
                )
            })
    }

    pub fn lease_diagnostics(&self) -> Result<RuntimeLeaseDiagnostics, AppError> {
        let registry = self.lease_registry.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model lease durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                None,
            )
        })?;
        let now = Utc::now();
        let mut operation_kinds = Vec::new();
        let mut oldest_lease_age_seconds = None;
        for record in registry.leases.values() {
            let _ownership_context = (
                &record.profile_id,
                &record.consumer_id,
                &record.job_id,
                &record.runtime_instance_id,
            );
            if !operation_kinds.contains(&record.operation_kind) {
                operation_kinds.push(record.operation_kind.clone());
            }
            let age = now
                .signed_duration_since(record.acquired_at)
                .num_seconds()
                .max(0);
            oldest_lease_age_seconds =
                Some(oldest_lease_age_seconds.map_or(age, |current: i64| current.max(age)));
        }
        Ok(RuntimeLeaseDiagnostics {
            active_lease_count: registry.leases.len(),
            oldest_lease_age_seconds,
            operation_kinds,
        })
    }

    pub fn set_draining(&self, draining: bool) -> Result<(), AppError> {
        let mut registry = self.lease_registry.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model lease durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                None,
            )
        })?;
        registry.draining = draining;
        Ok(())
    }

    pub fn is_draining(&self) -> Result<bool, AppError> {
        self.lease_registry
            .lock()
            .map(|registry| registry.draining)
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model lease durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    None,
                )
            })
    }

    fn reset_lease_registry(&self) -> Result<(), AppError> {
        let mut registry = self.lease_registry.lock().map_err(|err| {
            model_error(
                AppErrorCode::ModelStateAccessFailed,
                "Model lease durumuna erişilemedi.",
                Some(format!("Mutex poison error: {err}")),
                None,
            )
        })?;
        let unexpected_exit_count = registry.unexpected_exit_count;
        *registry = LeaseRegistry {
            unexpected_exit_count,
            ..LeaseRegistry::default()
        };
        Ok(())
    }

    fn io_error(&self, code: AppErrorCode, message: &str, err: std::io::Error) -> AppError {
        model_error(code, message, Some(err.to_string()), None)
    }

    fn persist_runtime_state(&self) -> Result<(), AppError> {
        let processes = self
            .runtime
            .lock()
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelStateAccessFailed,
                    "Model durumuna erişilemedi.",
                    Some(format!("Mutex poison error: {err}")),
                    Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
                )
            })?
            .clone();
        if processes.is_empty() {
            return clear_persisted_state(&self.persisted_state_path);
        }
        let persisted = PersistedManagedProcess {
            schema_version: 2,
            metadata: None,
            processes,
        };
        let content = serde_json::to_string_pretty(&persisted).map_err(|err| {
            model_error(
                AppErrorCode::ModelServerStartFailed,
                "Managed süreç durumu kaydedilemedi.",
                Some(err.to_string()),
                None,
            )
        })?;
        atomic_write(&self.persisted_state_path, &content).map_err(|err| {
            model_error(
                AppErrorCode::ModelServerStartFailed,
                "Managed süreç durumu yazılamadı.",
                Some(err.to_string()),
                None,
            )
        })
    }

    pub async fn ensure_model_ready_for_model_step(
        &self,
        step_name: &str,
    ) -> Result<ModelStatus, AppError> {
        let status = self.get_model_status(None).await?;
        if status.server_running && status.health_ok {
            return Ok(status);
        }

        if !status.can_start_from_app {
            let mut error = status.last_error.clone().unwrap_or_else(|| {
                model_error(
                    AppErrorCode::ModelServerNotRunning,
                    "Yerel model sunucusu çalışmıyor.",
                    Some(format!(
                        "step={step_name}; profile_id={}",
                        status.profile_id
                    )),
                    Some("Model sunucusunu manuel başlatın veya managed moda geçin.".to_string()),
                )
            });
            error.technical_details = Some(model_step_details(step_name, &status, false, None, None));
            return Err(error);
        }

        if status.start_requires_mode_change {
            let _ = self.set_mode(None, ModelMode::Managed).await?;
        }

        let start_result = self.start_server(None).await;
        match start_result {
            Ok(_) => {
                let ready_status = self.get_model_status(None).await?;
                if ready_status.server_running && ready_status.health_ok {
                    Ok(ready_status)
                } else {
                    let mut error = ready_status.last_error.clone().unwrap_or_else(|| {
                        model_error(
                            AppErrorCode::ModelServerStartFailed,
                            "Yerel model sunucusu başlatılamadı.",
                            Some(format!(
                                "step={step_name}; profile_id={}",
                                ready_status.profile_id
                            )),
                            Some("Model ayarlarını kontrol edin veya modeli elle başlatın.".to_string()),
                        )
                    });
                    error.code = AppErrorCode::ModelServerStartFailed;
                    error.message = "Yerel model sunucusu başlatılamadı.".to_string();
                    error.suggested_action =
                        Some("Model ayarlarını kontrol edin veya modeli elle başlatın.".to_string());
                    error.technical_details = Some(model_step_details(
                        step_name,
                        &status,
                        true,
                        Some(&ready_status),
                        None,
                    ));
                    Err(error)
                }
            }
            Err(start_error) => {
                let start_error_details = start_error.technical_details.clone().unwrap_or_default();
                let step_details = model_step_details(step_name, &status, true, None, Some(&start_error));
                let mut error = AppError {
                    code: AppErrorCode::ModelServerStartFailed,
                    message: "Yerel model sunucusu başlatılamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Model ayarlarını kontrol edin veya modeli elle başlatın.".to_string(),
                    ),
                    technical_details: Some(step_details),
                    correlation_id: start_error.correlation_id.clone(),
                };
                if !start_error_details.is_empty() {
                    let mut merged_details = error.technical_details.unwrap_or_default();
                    if !merged_details.is_empty() {
                        merged_details.push_str("\n\n");
                    }
                    merged_details.push_str(&format!("start_error_details={start_error_details}"));
                    error.technical_details = Some(merged_details);
                }
                Err(error)
            }
        }
    }
}

fn launch_spec_for_profile(
    profile: &ModelProfile,
    support_flags: &SupportFlags,
) -> Result<RuntimeLaunchSpec, AppError> {
    if let Some(definition) = platform_launch_registry::get(&profile.id) {
        return LlamaCppRuntimeAdapter.build_launch_spec(
            &definition.runtime,
            &definition.model,
            support_flags,
        );
    }
    Ok(RuntimeLaunchSpec {
        engine: crate::domain::model_platform::RuntimeEngine::LlamaCpp,
        command: profile.server_path.clone(),
        args: build_model_server_args(profile, support_flags)?,
        base_url: profile.base_url.clone(),
        runtime_fingerprint: runtime_profile_fingerprint(profile),
        requires_mmproj: profile.requires_mmproj(),
    })
}

fn spawn_log_reader<R>(
    reader: R,
    file: Arc<tokio::sync::Mutex<tokio::fs::File>>,
    stream_name: &'static str,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut file = file.lock().await;
            let timestamp = Utc::now().to_rfc3339();
            let _ = file
                .write_all(format!("{} [{}] {}\n", timestamp, stream_name, line).as_bytes())
                .await;
            let _ = file.flush().await;
        }
    });
}

fn restore_runtime_state(path: &PathBuf) -> Vec<ManagedModelProcess> {
    let Ok(content) = fs::read_to_string(path) else {
        return vec![];
    };
    let Ok(persisted) = serde_json::from_str::<PersistedManagedProcess>(&content) else {
        return vec![];
    };
    if persisted.schema_version > 2 {
        return vec![];
    }
    let mut processes = persisted.processes;
    if let Some(metadata) = persisted.metadata {
        if !processes
            .iter()
            .any(|process| process.profile_id == metadata.profile_id)
        {
            processes.push(metadata);
        }
    }
    processes
}

fn clear_persisted_state(path: &PathBuf) -> Result<(), AppError> {
    if path.exists() {
        fs::remove_file(path).map_err(|err| {
            model_error(
                AppErrorCode::ModelServerStopFailed,
                "Managed süreç durumu silinemedi.",
                Some(err.to_string()),
                None,
            )
        })?;
    }
    Ok(())
}

fn read_log_tail(path: &PathBuf, line_count: usize) -> io::Result<String> {
    let content = fs::read_to_string(path)?;
    let mut lines: Vec<&str> = content.lines().collect();
    if lines.len() > line_count {
        lines = lines.split_off(lines.len() - line_count);
    }
    Ok(lines.join("\n"))
}

fn profile_workdir(profile: &ModelProfile) -> PathBuf {
    PathBuf::from(&profile.server_path)
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn model_process_state_path() -> PathBuf {
    app_log_dir().join("managed_model_process.json")
}

fn runtime_profile_fingerprint(profile: &ModelProfile) -> String {
    if let Some(definition) = platform_launch_registry::get(&profile.id) {
        return fingerprint(&[
            definition.model.model_fingerprint,
            fingerprint_runtime_definition(&definition.runtime),
        ]);
    }
    let canonical = |path: &str| {
        fs::canonicalize(path)
            .unwrap_or_else(|_| PathBuf::from(path))
            .to_string_lossy()
            .to_string()
    };
    fingerprint(&[
        canonical(&profile.server_path),
        canonical(&profile.model_path),
        canonical(&profile.mmproj_path),
        profile.host.clone(),
        profile.port.to_string(),
        format!("{:?}", profile.runtime_preset),
        format!("{:?}", profile.privacy_mode),
    ])
}

fn model_file_fingerprint(profile: &ModelProfile) -> Option<String> {
    if let Some(definition) = platform_launch_registry::get(&profile.id) {
        return Some(definition.model.model_fingerprint);
    }
    if profile.model_path.trim().is_empty() {
        return None;
    }
    let path = fs::canonicalize(&profile.model_path).ok()?;
    let metadata = fs::metadata(&path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_nanos().to_string())
        .unwrap_or_default();
    Some(fingerprint(&[
        path.to_string_lossy().to_string(),
        metadata.len().to_string(),
        modified,
    ]))
}

fn send_verified_signal(
    inspector: &Arc<dyn ProcessInspector>,
    metadata: &ManagedModelProcess,
    profile: &ModelProfile,
    signal: i32,
) -> Result<(), AppError> {
    let Some(identity) = metadata.identity.as_ref() else {
        return Err(model_error(
            AppErrorCode::ModelProcessUnverified,
            "Model süreci kimliği doğrulanamadı; sinyal gönderilmedi.",
            None,
            Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
        ));
    };
    if identity.pid == 0 || identity.pid > i32::MAX as u32 || identity.expected_port != profile.port {
        return Err(model_error(
            AppErrorCode::ModelProcessIdentityMismatch,
            "Geçersiz model süreç kimliği; sinyal gönderilmedi.",
            Some(format!("pid={}; port={}", identity.pid, identity.expected_port)),
            None,
        ));
    }
    let snapshot = inspector
        .inspect(identity.pid)
        .map_err(|error| {
            model_error(
                AppErrorCode::ModelProcessUnverified,
                "Model süreci sonlandırılmadan önce kimliği doğrulanamadı.",
                Some(error),
                Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
            )
        })?
        .ok_or_else(|| {
            model_error(
                AppErrorCode::ModelRuntimeExited,
                "Model süreci zaten çalışmıyor.",
                Some(format!("pid={}", identity.pid)),
                None,
            )
        })?;
    if snapshot.owner_uid != identity.owner_uid
        || snapshot.owner_uid != inspector.current_uid()
        || snapshot.process_start_time_unix_ms != identity.process_start_time_unix_ms
        || snapshot.canonical_executable_path != identity.canonical_executable_path
    {
        return Err(model_error(
            AppErrorCode::ModelProcessIdentityMismatch,
            "Model süreci kimliği değişti; sinyal gönderilmedi.",
            Some(format!("pid={}", identity.pid)),
            Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
        ));
    }
    if !inspector
        .process_owns_port(identity.pid, &profile.host, profile.port)
        .unwrap_or(false)
    {
        return Err(model_error(
            AppErrorCode::ModelProcessIdentityMismatch,
            "Model süreci beklenen portu sahiplenmiyor; sinyal gönderilmedi.",
            Some(format!("pid={}; port={}", identity.pid, profile.port)),
            Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
        ));
    }
    #[cfg(unix)]
    {
        if unsafe { libc::kill(-(identity.pid as i32), signal) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(model_error(
                    AppErrorCode::ModelServerStopFailed,
                    "Model süreci sonlandırılamadı.",
                    Some(error.to_string()),
                    Some("Tanılama ayrıntılarını kontrol edin.".to_string()),
                ));
            }
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (inspector, metadata, profile, signal);
        Err(model_error(
            AppErrorCode::ModelServerStopFailed,
            "Bu platformda güvenli süreç sonlandırma desteklenmiyor.",
            None,
            None,
        ))
    }
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

fn with_tail(mut error: AppError, tail: String) -> AppError {
    if tail.is_empty() {
        return error;
    }
    error.technical_details = Some(match error.technical_details {
        Some(existing) => format!("{}\n\nLog Tail:\n{}", existing, tail),
        None => format!("Log Tail:\n{}", tail),
    });
    error
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelStepDetails {
    step: String,
    model_required: bool,
    model_mode: ModelMode,
    health_ok_before_start: bool,
    attempted_managed_start: bool,
    health_ok_after_start: bool,
    model_log_path: Option<String>,
    error_code: Option<AppErrorCode>,
    error_message: Option<String>,
}

fn model_step_details(
    step_name: &str,
    before: &ModelStatus,
    attempted_managed_start: bool,
    after: Option<&ModelStatus>,
    error: Option<&AppError>,
) -> String {
    let details = ModelStepDetails {
        step: step_name.to_string(),
        model_required: true,
        model_mode: before.mode.clone(),
        health_ok_before_start: before.server_running && before.health_ok,
        attempted_managed_start,
        health_ok_after_start: after.is_some_and(|status| status.server_running && status.health_ok),
        model_log_path: before
            .log_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        error_code: error.map(|err| err.code.clone()),
        error_message: error.map(|err| err.message.clone()),
    };

    serde_json::to_string_pretty(&details).unwrap_or_else(|serialization_error| {
        format!(
            "step={step_name}; model_mode={:?}; serialization_error={serialization_error}",
            before.mode
        )
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::net::TcpListener;
    use tokio::sync::{Mutex, MutexGuard};

    static MODEL_RUNTIME_TEST_LOCK: Mutex<()> = Mutex::const_new(());

    pub(crate) async fn lock_model_runtime_test() -> MutexGuard<'static, ()> {
        MODEL_RUNTIME_TEST_LOCK.lock().await
    }

    pub(crate) fn blocking_lock_model_runtime_test() -> MutexGuard<'static, ()> {
        MODEL_RUNTIME_TEST_LOCK.blocking_lock()
    }

    pub(crate) fn available_loopback_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("model runtime test requires a loopback TCP listener");
        let port = listener
            .local_addr()
            .expect("model runtime test listener must have a local address")
            .port();
        drop(listener);
        port
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{default_model_profile, ModelMode};
    use crate::platform::process_inspector::{ProcessInspector, ProcessSnapshot};
    use crate::services::model_config_service::ModelConfigService;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command as StdCommand;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::thread;

    #[derive(Clone, Default)]
    struct FakeProcessInspector {
        processes: Arc<Mutex<HashMap<u32, ProcessSnapshot>>>,
        owned_ports: Arc<Mutex<Vec<(u32, u16)>>>,
        uid: u32,
    }

    impl ProcessInspector for FakeProcessInspector {
        fn inspect(&self, pid: u32) -> Result<Option<ProcessSnapshot>, String> {
            Ok(self
                .processes
                .lock()
                .map_err(|error| error.to_string())?
                .get(&pid)
                .cloned())
        }

        fn process_owns_port(&self, pid: u32, _host: &str, port: u16) -> Result<bool, String> {
            Ok(self
                .owned_ports
                .lock()
                .map_err(|error| error.to_string())?
                .contains(&(pid, port)))
        }

        fn current_uid(&self) -> u32 {
            self.uid
        }
    }

    fn test_service() -> ModelConfigService {
        let path = env::temp_dir().join(format!("rubrika-model-config-{}.json", Uuid::new_v4()));
        ModelConfigService::new_with_path(path)
    }

    type ModelProbeServer = (
        String,
        String,
        u16,
        Arc<AtomicUsize>,
        thread::JoinHandle<()>,
    );

    fn spawn_model_probe_server() -> Option<ModelProbeServer> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(error) => panic!("failed to bind probe test server: {error}"),
        };
        let addr = listener.local_addr().unwrap();
        let completion_count = Arc::new(AtomicUsize::new(0));
        let count_clone = completion_count.clone();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(_) => break,
                };
                let mut buffer = [0u8; 2048];
                let read_len = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read_len]);
                let (status_line, body) = if request.contains("/v1/chat/completions") {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                    (
                        "HTTP/1.1 200 OK",
                        r#"{"choices":[{"message":{"content":"OK"}}]}"#,
                    )
                } else {
                    ("HTTP/1.1 200 OK", r#"{"status":"ok"}"#)
                };
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Some((
            format!("http://{}", addr),
            addr.ip().to_string(),
            addr.port(),
            completion_count,
            handle,
        ))
    }

    fn write_mock_llama_server_script(mode: &str) -> PathBuf {
        let path = env::temp_dir().join(format!("rubrika-mock-llama-{}.py", Uuid::new_v4()));
        let script = r#"#!/usr/bin/env python3
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

args = sys.argv[1:]
if "--help" in args or "-h" in args:
    print("--cache-type-k\n--cache-type-v\n--mmproj-offload")
    sys.exit(0)

if "__MODE__" == "fail":
    sys.exit(1)

host = "127.0.0.1"
port = 8080
for i, arg in enumerate(args):
    if arg == "--host" and i + 1 < len(args):
        host = args[i + 1]
    elif arg == "--port" and i + 1 < len(args):
        port = int(args[i + 1])

class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        return

    def _write_json(self, body, status=200):
        payload = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self):
        if self.path == "/health":
            self._write_json({"status": "ok"})
            return
        self._write_json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path == "/v1/chat/completions":
            length = int(self.headers.get("Content-Length", "0") or "0")
            _ = self.rfile.read(length)
            self._write_json({"choices": [{"message": {"content": "OK"}}]})
            return
        self._write_json({"error": "not found"}, 404)

HTTPServer((host, port), Handler).serve_forever()
"#
        .replace("__MODE__", mode);
        fs::write(&path, script).expect("mock llama-server script should be writable");
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&path)
                .expect("mock llama-server metadata should be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)
                .expect("mock llama-server script should be executable");
        }
        path
    }

    #[test]
    fn default_profile_has_log_path() {
        let profile = default_model_profile();
        let path = model_server_log_path(&profile.id);
        assert!(path.to_string_lossy().contains("RubrikaV3"));
    }

    #[test]
    fn unsupported_kv_flags_error_from_preview_helper() {
        let profile = default_model_profile();
        let help = "--mmproj-offload";
        let err = build_model_server_args(&profile, &SupportFlags::from_help_output(help)).unwrap_err();
        assert_eq!(err.code, AppErrorCode::ModelServerUnsupportedFlags);
    }

    #[test]
    fn port_in_use_is_detected() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(error) => panic!("failed to bind test listener: {error}"),
        };
        let addr = listener.local_addr().unwrap();
        let service = ModelProcessManager::new_with_state_path(
            test_service(),
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        assert!(service.is_port_in_use("127.0.0.1", addr.port()).unwrap());
        drop(listener);
    }

    #[test]
    fn child_fixture_can_be_cleaned_without_raw_pid_control() {
        let mut child = StdCommand::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep should be available");
        child.kill().expect("test child should be killable");
        let _ = child.wait();
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn identity_mismatch_never_signals_unrelated_child() {
        let mut child = StdCommand::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep should be available");
        let pid = child.id();
        let inspector = FakeProcessInspector {
            processes: Arc::new(Mutex::new(HashMap::from([(
                pid,
                ProcessSnapshot {
                    pid,
                    owner_uid: 501,
                    process_start_time_unix_ms: 200,
                    canonical_executable_path: PathBuf::from("/usr/bin/unrelated-process"),
                    argv_fingerprint: Some("unrelated-argv".to_string()),
                },
            )]))),
            owned_ports: Arc::new(Mutex::new(vec![(pid, 8080)])),
            uid: 501,
        };
        let manager = ModelProcessManager::new_with_inspector(
            test_service(),
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
            Arc::new(inspector),
            Duration::from_millis(20),
        );
        let profile = default_model_profile();
        let metadata = ManagedModelProcess {
            pid: Some(pid),
            started_by_app: true,
            profile_id: profile.id.clone(),
            base_url: profile.base_url.clone(),
            log_path: PathBuf::from("/tmp/rubrika-test.log"),
            started_at: Some(Utc::now()),
            identity: Some(ManagedProcessIdentity {
                pid,
                owner_uid: 501,
                process_start_time_unix_ms: 100,
                canonical_executable_path: PathBuf::from("/usr/bin/rubrika-llama-server"),
                executable_fingerprint: fingerprint(&["/usr/bin/rubrika-llama-server".to_string()]),
                argv_fingerprint: "rubrika-argv".to_string(),
                expected_port: profile.port,
                runtime_profile_fingerprint: runtime_profile_fingerprint(&profile),
                launch_instance_id: Uuid::new_v4().to_string(),
                launched_at: Utc::now(),
            }),
            runtime_instance_id: Some(Uuid::new_v4().to_string()),
            runtime_profile_fingerprint: Some(runtime_profile_fingerprint(&profile)),
            unverified: false,
        };
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(manager.stop_current_process(&metadata));
        assert!(matches!(
            result,
            Err(AppError {
                code: AppErrorCode::ModelProcessIdentityMismatch,
                ..
            })
        ));
        assert!(child.try_wait().expect("child status").is_none());
        child.kill().expect("test child should be killable");
        let _ = child.wait();
    }

    #[test]
    #[ignore = "Requires loopback TCP bind; restricted sandboxes must run this with network permission."]
    fn releasing_one_lease_does_not_stop_runtime_used_by_another_job() {
        let Some((base_url, host, port, completion_count, _handle)) = spawn_model_probe_server() else {
            return;
        };
        let config = test_service();
        let mut profile = default_model_profile();
        profile.mode = ModelMode::External;
        profile.server_path = "/bin/echo".to_string();
        profile.model_path = "/bin/echo".to_string();
        profile.mmproj_path = "/bin/echo".to_string();
        profile.base_url = base_url;
        profile.host = host;
        profile.port = port;
        config.update_profile(profile).expect("profile update");
        let manager = ModelProcessManager::new_with_state_path(
            config,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let first = manager
                .acquire_lease(None, false, 2, "ocr", Some("job-a"), "ocr", "corr-a")
                .await
                .expect("first lease");
            assert_eq!(first.correlation_id, "corr-a");
            assert!(!first.profile_fingerprint.is_empty());
            let second = manager
                .acquire_lease(
                    None,
                    false,
                    2,
                    "scoring",
                    Some("job-b"),
                    "scoring",
                    "corr-b",
                )
                .await
                .expect("second lease");
            assert_eq!(manager.active_lease_count().expect("lease count"), 2);
            manager
                .release_lease(&first.lease_id, &first.runtime_instance_id)
                .await
                .expect("first release");
            assert_eq!(manager.active_lease_count().expect("lease count"), 1);
            let status = manager
                .probe_model_server(None)
                .await
                .expect("runtime remains probeable");
            assert!(status.health_ok);
            assert_eq!(completion_count.load(Ordering::SeqCst), 1);
            manager
                .release_lease(&second.lease_id, &second.runtime_instance_id)
                .await
                .expect("second release");
            tokio::time::sleep(Duration::from_millis(60)).await;
            assert_eq!(manager.active_lease_count().expect("lease count"), 0);
        });
    }

    #[test]
    fn releasing_one_registry_lease_preserves_another_without_network() {
        let manager = ModelProcessManager::new_with_state_path(
            test_service(),
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let runtime_instance_id = "runtime-test".to_string();
        let profile_id = default_model_profile().id;
        let profile_fingerprint = "profile-test".to_string();
        {
            let mut registry = manager.lease_registry.lock().expect("lease registry");
            registry.runtime_instance_id = Some(runtime_instance_id.clone());
            registry.profile_id = Some(profile_id.clone());
            registry.profile_fingerprint = Some(profile_fingerprint);
            for (lease_id, consumer_id) in [("lease-a", "ocr"), ("lease-b", "scoring")] {
                registry.leases.insert(
                    lease_id.to_string(),
                    LeaseRecord {
                        runtime_instance_id: runtime_instance_id.clone(),
                        profile_id: profile_id.clone(),
                        consumer_id: consumer_id.to_string(),
                        job_id: None,
                        operation_kind: consumer_id.to_string(),
                        acquired_at: Utc::now(),
                    },
                );
            }
        }
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            manager
                .release_lease("lease-a", &runtime_instance_id)
                .await
                .expect("first lease release");
            assert_eq!(manager.active_lease_count().expect("lease count"), 1);
            manager
                .release_lease("lease-b", &runtime_instance_id)
                .await
                .expect("second lease release");
            assert_eq!(manager.active_lease_count().expect("lease count"), 0);
        });
    }

    #[test]
    fn manual_start_is_blocked_while_runtime_is_draining() {
        let service = test_service();
        let _ = service.set_mode(None, ModelMode::Managed).expect("managed mode");
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        manager.set_draining(true).expect("draining state");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let error = runtime
            .block_on(manager.start_server_with_timeout(None, Duration::from_millis(10)))
            .expect_err("draining runtime must reject manual start");
        assert_eq!(error.code, AppErrorCode::ModelRuntimeDraining);
    }

    #[test]
    fn fifty_concurrent_lease_releases_leave_zero_owned_runtime() {
        let manager = ModelProcessManager::new_with_inspector(
            test_service(),
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
            Arc::new(FakeProcessInspector {
                processes: Arc::new(Mutex::new(HashMap::new())),
                owned_ports: Arc::new(Mutex::new(Vec::new())),
                uid: 501,
            }),
            Duration::from_secs(3600),
        );
        let runtime_instance_id = "runtime-stress".to_string();
        let profile_id = default_model_profile().id;
        {
            let mut registry = manager.lease_registry.lock().expect("lease registry");
            registry.runtime_instance_id = Some(runtime_instance_id.clone());
            registry.profile_id = Some(profile_id.clone());
            registry.profile_fingerprint = Some("profile-stress".to_string());
            for index in 0..50 {
                let lease_id = format!("lease-{index}");
                registry.leases.insert(
                    lease_id,
                    LeaseRecord {
                        runtime_instance_id: runtime_instance_id.clone(),
                        profile_id: profile_id.clone(),
                        consumer_id: format!("consumer-{index}"),
                        job_id: Some(format!("job-{index}")),
                        operation_kind: "stress".to_string(),
                        acquired_at: Utc::now(),
                    },
                );
            }
        }
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let mut tasks = Vec::new();
            for index in 0..50 {
                let manager = manager.clone();
                let runtime_instance_id = runtime_instance_id.clone();
                tasks.push(tokio::spawn(async move {
                    manager
                        .release_lease(&format!("lease-{index}"), &runtime_instance_id)
                        .await
                }));
            }
            for task in tasks {
                task.await.expect("lease release task").expect("lease release");
            }
            assert_eq!(manager.active_lease_count().expect("lease count"), 0);
        });
    }

    #[test]
    fn production_model_consumers_have_no_global_stop_call() {
        for file in [
            "student_answer_ocr_service.rs",
            "scoring_service.rs",
            "rubric_extraction_service.rs",
            "speaking_exam_service.rs",
            "analysis_service.rs",
            "question_text_service.rs",
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/services")
                .join(file);
            let source = std::fs::read_to_string(path).expect("service source");
            assert!(!source.contains("stop_server("), "{file} must not own model lifecycle");
            assert!(
                !source.contains("ensure_ready(") && !source.contains("acquire_runtime("),
                "{file} must use the single ready runtime lease contract"
            );
            assert!(
                source.contains("acquire_ready_runtime_lease"),
                "{file} must carry readiness into the domain operation"
            );
        }
    }

    #[test]
    fn status_reflects_mode_setting() {
        let service = test_service();
        let _ = service.set_mode(None, ModelMode::Managed).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let status = manager.get_model_status(None).await.unwrap();
            assert_eq!(status.mode, ModelMode::Managed);
        });
    }

    #[test]
    fn legacy_public_external_profile_returns_privacy_blocked_status() {
        let config_service = ModelConfigService::new_with_path(std::env::temp_dir().join(format!(
            "rubrika-model-status-privacy-{}.json",
            uuid::Uuid::new_v4()
        )));
        let mut profile = config_service.get_profile(None).unwrap();
        profile.mode = ModelMode::External;
        profile.base_url = "https://model.example.test".to_string();
        profile.host = "model.example.test".to_string();
        profile.privacy_mode = crate::domain::model::PrivacyMode::StrictLocal;
        config_service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            config_service,
            Arc::new(LlamaServerGateway::default()),
            std::env::temp_dir().join(format!(
                "rubrika-model-status-state-{}.json",
                uuid::Uuid::new_v4()
            )),
        );
        let status = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.get_model_status(None))
            .expect("status should explain the privacy block");
        assert!(status.privacy_blocked);
        assert_eq!(
            status.last_error.as_ref().map(|error| &error.code),
            Some(&AppErrorCode::ModelPrivacyBlocked)
        );
        assert!(status
            .suggested_actions
            .iter()
            .any(|action| action.code == "use_managed_local_model"));
    }

    #[test]
    #[ignore = "Requires loopback TCP bind; restricted sandboxes must run this with network permission."]
    fn get_model_status_avoids_completion_probe() {
        let Some((base_url, host, port, completion_count, _handle)) = spawn_model_probe_server() else {
            return;
        };
        let service = test_service();
        let mut profile = default_model_profile();
        profile.mode = ModelMode::External;
        profile.server_path = "/bin/echo".to_string();
        profile.model_path = "/bin/echo".to_string();
        profile.mmproj_path = "/bin/echo".to_string();
        profile.base_url = base_url;
        profile.host = host;
        profile.port = port;
        service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let status = manager.get_model_status(None).await.unwrap();
            assert!(status.server_running);
            assert!(status.health_ok);
            assert!(!status.completion_probe_ok);
            assert_eq!(completion_count.load(Ordering::SeqCst), 0);
            let probed = manager.probe_model_server(None).await.unwrap();
            assert!(probed.server_running);
            assert!(probed.health_ok);
            assert!(probed.completion_probe_ok);
            assert_eq!(completion_count.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    #[ignore = "Requires loopback TCP bind; restricted sandboxes must run this with network permission."]
    fn ensure_model_ready_skips_start_when_server_is_already_healthy() {
        let Some((base_url, host, port, _completion_count, _handle)) = spawn_model_probe_server() else {
            return;
        };
        let service = test_service();
        let mut profile = default_model_profile();
        profile.mode = ModelMode::External;
        profile.server_path = "/bin/echo".to_string();
        profile.model_path = "/bin/echo".to_string();
        profile.mmproj_path = "/bin/echo".to_string();
        profile.base_url = base_url;
        profile.host = host;
        profile.port = port;
        service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let status = manager
                .ensure_model_ready_for_model_step("rubric_extraction")
                .await
                .unwrap();
            assert!(status.server_running);
            assert!(status.health_ok);
            assert!(!status.started_by_app);
        });
    }

    #[test]
    fn ensure_model_ready_reports_start_failure() {
        let _model_runtime_guard = test_support::blocking_lock_model_runtime_test();
        let server_path = write_mock_llama_server_script("fail");
        let service = test_service();
        let mut profile = default_model_profile();
        profile.id = format!("managed-start-failure-{}", Uuid::new_v4());
        profile.display_name = "Managed start failure test".to_string();
        profile.mode = ModelMode::Managed;
        profile.server_path = server_path.to_string_lossy().to_string();
        profile.model_path = server_path.to_string_lossy().to_string();
        profile.mmproj_path = server_path.to_string_lossy().to_string();
        profile.host = "127.0.0.1".to_string();
        profile.port = test_support::available_loopback_port();
        profile.base_url = format!("http://127.0.0.1:{}", profile.port);
        service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = manager
                .ensure_model_ready_for_model_step("rubric_extraction")
                .await
                .unwrap_err();
            assert_eq!(
                err.code,
                AppErrorCode::ModelServerStartFailed,
                "unexpected readiness error: {err:#?}"
            );
            let details = err.technical_details.unwrap_or_default();
            assert!(details.contains("\"step\": \"rubric_extraction\""));
            assert!(details.contains("\"attemptedManagedStart\": true"));
        });
    }

    #[test]
    fn start_fails_on_missing_server_path() {
        let service = test_service();
        let mut profile = default_model_profile();
        profile.mode = ModelMode::Managed;
        profile.server_path = "/does/not/exist".to_string();
        profile.model_path = "/bin/echo".to_string();
        profile.mmproj_path = "/bin/echo".to_string();
        service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = manager.start_server(None).await.unwrap_err();
            assert_eq!(err.code, AppErrorCode::ModelServerPathMissing);
        });
    }

    #[test]
    fn start_fails_on_missing_model_path() {
        let service = test_service();
        let mut profile = default_model_profile();
        profile.mode = ModelMode::Managed;
        profile.server_path = "/bin/echo".to_string();
        profile.model_path = "/does/not/exist".to_string();
        profile.mmproj_path = "/bin/echo".to_string();
        service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = manager.start_server(None).await.unwrap_err();
            assert_eq!(err.code, AppErrorCode::ModelModelPathMissing);
        });
    }

    #[test]
    fn start_fails_on_missing_mmproj_path() {
        let service = test_service();
        let mut profile = default_model_profile();
        profile.mode = ModelMode::Managed;
        profile.server_path = "/bin/echo".to_string();
        profile.model_path = "/bin/echo".to_string();
        profile.mmproj_path = "/does/not/exist".to_string();
        service.update_profile(profile).unwrap();
        let manager = ModelProcessManager::new_with_state_path(
            service,
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = manager.start_server(None).await.unwrap_err();
            assert_eq!(err.code, AppErrorCode::ModelMmprojPathMissing);
        });
    }

    #[test]
    fn stop_external_process_is_noop() {
        let service = ModelProcessManager::new_with_state_path(
            test_service(),
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = service.stop_server(None).await.unwrap();
            assert!(!result.stopped);
        });
    }
}

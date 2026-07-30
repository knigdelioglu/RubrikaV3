use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{
    build_model_server_args, preview_model_server_args, ManagedModelProcess, ModelMode,
    ModelProfile, ModelServerArgsPreview, ModelStatus, ModelSuggestedAction, SupportFlags,
};
use crate::platform::file_access::atomic_write;
use crate::platform::paths::{app_log_dir, model_server_log_path};
use crate::services::llama_server_gateway::LlamaServerGateway;
use crate::services::model_config_service::ModelConfigService;
use crate::services::model_gateway::ModelGateway;
use chrono::Utc;
use std::fs;
use std::io;
use std::net::{TcpListener, ToSocketAddrs};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedManagedProcess {
    #[serde(default)]
    metadata: Option<ManagedModelProcess>,
    #[serde(default)]
    processes: Vec<ManagedModelProcess>,
}

#[derive(Clone)]
pub struct ModelProcessManager {
    config_service: ModelConfigService,
    gateway: Arc<LlamaServerGateway>,
    runtime: Arc<Mutex<Vec<ManagedModelProcess>>>,
    persisted_state_path: PathBuf,
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
        let runtime = restore_runtime_state(&persisted_state_path);
        Self {
            config_service,
            gateway,
            runtime: Arc::new(Mutex::new(runtime)),
            persisted_state_path,
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
        let profile = self.config_service.get_profile(profile_id)?;
        self.require_managed(&profile)?;

        if self.is_managed_profile_running(&profile.id)? {
            let status = self.build_status(Some(&profile.id), true).await?;
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
                "Başka bir Gemma modeli şu anda RubrikaV3 tarafından kullanılıyor.",
                Some(running_profile),
                Some("Etkin model işi tamamlandığında tekrar deneyin.".to_string()),
            ));
        }

        self.require_paths(&profile)?;

        if self.is_port_in_use(&profile.host, profile.port)? {
            let status = self.gateway.probe_status(&profile.base_url).await?;
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
                "8080 portu başka bir süreç tarafından kullanılıyor.",
                Some(profile.base_url.clone()),
                Some("RubrikaV3 başka bir süreci kapatmaz.".to_string()),
            ));
        }

        let help_output = self.run_help(&profile.server_path).await?;
        let support_flags = SupportFlags::from_help_output(&help_output);
        let args = build_model_server_args(&profile, &support_flags)?;
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

        let mut child = Command::new(&profile.server_path)
            .args(&args)
            .current_dir(profile_workdir(&profile))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env(
                "LLAMA_ARG_FLASH_ATTN",
                if profile.requires_mmproj() {
                    "off"
                } else {
                    "auto"
                },
            )
            .spawn()
            .map_err(|err| {
                model_error(
                    AppErrorCode::ModelServerStartFailed,
                    "llama-server başlatılamadı.",
                    Some(err.to_string()),
                    Some("Binary yolunu ve izinleri kontrol edin.".to_string()),
                )
            })?;

        let pid = child.id();
        let metadata = ManagedModelProcess {
            pid,
            started_by_app: true,
            profile_id: profile.id.clone(),
            base_url: profile.base_url.clone(),
            log_path: log_path.clone(),
            started_at: Some(Utc::now()),
        };
        if let Err(err) = self.attach_log_forwarders(&mut child, &log_path).await {
            if let Some(pid) = pid {
                let _ = self.stop_pid(pid);
            }
            return Err(err);
        }

        {
            let mut runtime =
                self.runtime
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
            if let Some(pid) = pid {
                let _ = self.stop_pid(pid);
            }
            self.remove_runtime_profile(&profile.id)?;
            return Err(err);
        }

        match self.wait_until_ready(&profile, timeout).await {
            Ok(probe) => Ok(StartModelServerOutput {
                started: true,
                mode: ModelMode::Managed,
                pid,
                base_url: profile.base_url.clone(),
                log_path: log_path.to_string_lossy().to_string(),
                health_ok: probe.health_ok,
                message: "Model sunucusu başarıyla başlatıldı.".to_string(),
            }),
            Err(err) => {
                if let Some(pid) = pid {
                    let _ = self.stop_pid(pid);
                }
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
        let metadata = self
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

        let Some(metadata) = metadata else {
            return Ok(StopModelServerOutput {
                stopped: false,
                message: "Durdurulacak yönetilen model süreci bulunamadı.".to_string(),
            });
        };

        if !metadata.started_by_app {
            return Ok(StopModelServerOutput {
                stopped: false,
                message: "Bu model sunucusu RubrikaV3 tarafından başlatılmadığı için kapatılmadı."
                    .to_string(),
            });
        }

        if let Some(pid) = metadata.pid {
            self.stop_pid(pid)?;
            self.remove_runtime_profile(&profile.id)?;
            self.persist_runtime_state()?;
            return Ok(StopModelServerOutput {
                stopped: true,
                message: format!(
                    "RubrikaV3 tarafından başlatılan süreç durduruldu (pid {}).",
                    pid
                ),
            });
        }

        let _ = profile;
        Err(model_error(
            AppErrorCode::ModelServerStopFailed,
            "Yönetilen süreç için PID bulunamadı.",
            None,
            None,
        ))
    }

    pub async fn set_mode(
        &self,
        profile_id: Option<&str>,
        mode: ModelMode,
    ) -> Result<ModelStatus, AppError> {
        self.config_service.set_mode(profile_id, mode)?;
        self.build_status(profile_id, true).await
    }

    pub async fn reset_profile(&self) -> Result<ModelStatus, AppError> {
        let profile = self.config_service.reset_active_profile()?;
        self.build_status(Some(&profile.id), true).await
    }

    pub async fn preview_args(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelServerArgsPreview, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        let help_output = self.run_help(&profile.server_path).await?;
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
        let log_path = model_server_log_path(&profile.id);
        let server_path_exists = self.path_exists(&profile.server_path);
        let model_path_exists = self.path_exists(&profile.model_path);
        let mmproj_path_exists =
            !profile.requires_mmproj() || self.path_exists(&profile.mmproj_path);
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
            managed_process_pid: runtime.as_ref().and_then(|metadata| metadata.pid),
            started_by_app: runtime
                .as_ref()
                .map(|metadata| {
                    metadata.started_by_app && self.is_pid_running(metadata.pid).unwrap_or(false)
                })
                .unwrap_or(false),
            log_path: Some(log_path.clone()),
            last_error: None,
            warnings: vec![],
            can_start_from_app: false,
            can_stop_from_app: false,
            start_requires_mode_change: false,
            start_disabled_reason: None,
            suggested_actions: vec![],
        };

        if status.started_by_app {
            status
                .warnings
                .push("RubrikaV3 tarafından başlatıldı.".to_string());
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
                if let Some(error) = probe_status.last_error {
                    status.last_error = Some(error);
                }
            } else {
                let health_status = self.gateway.health_status(&profile.base_url).await?;
                status.server_running = health_status.server_running || status.server_running;
                status.health_ok = health_status.health_ok;
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
            status
                .warnings
                .push("Model sunucusu kapalı görünüyor.".to_string());
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
            status.start_disabled_reason =
                Some("Model, server veya mmproj dosyası eksik.".to_string());
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
        timeout: Duration,
    ) -> Result<ModelStatus, AppError> {
        let start = Instant::now();
        loop {
            if start.elapsed() >= timeout {
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

            let probe = self.gateway.probe_status(&profile.base_url).await?;
            if probe.server_running && probe.health_ok && probe.completion_probe_ok {
                return Ok(probe);
            }

            sleep(Duration::from_secs(2)).await;
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
            Some("Önce Model Status ekranından profili managed moduna alın.".to_string()),
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
        PathBuf::from(path).exists()
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
        #[cfg(unix)]
        {
            let result = unsafe { libc::kill(pid as i32, 0) };
            if result == 0 {
                return Ok(true);
            }
            let err = io::Error::last_os_error();
            Ok(err.raw_os_error() == Some(libc::EPERM))
        }
        #[cfg(not(unix))]
        {
            Ok(pid > 0)
        }
    }

    fn is_managed_profile_running(&self, profile_id: &str) -> Result<bool, AppError> {
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
            .clone();
        Ok(runtime.iter().any(|metadata| {
            metadata.profile_id == profile_id
                && metadata
                    .pid
                    .map(|pid| self.is_pid_running(Some(pid)).unwrap_or(false))
                    .unwrap_or(false)
        }))
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
            if metadata
                .pid
                .map(|pid| self.is_pid_running(Some(pid)).unwrap_or(false))
                .unwrap_or(false)
            {
                return Ok(Some(metadata.profile_id.clone()));
            }
        }
        Ok(None)
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

    fn stop_pid(&self, pid: u32) -> Result<(), AppError> {
        #[cfg(unix)]
        {
            unsafe {
                if libc::kill(pid as i32, libc::SIGTERM) != 0 {
                    let err = io::Error::last_os_error();
                    if err.raw_os_error() != Some(libc::ESRCH) {
                        return Err(model_error(
                            AppErrorCode::ModelServerStopFailed,
                            "Model süreci sonlandırılamadı.",
                            Some(err.to_string()),
                            None,
                        ));
                    }
                }
            }
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(8) {
                if !self.is_pid_running(Some(pid))? {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            unsafe {
                if libc::kill(pid as i32, libc::SIGKILL) != 0 {
                    let err = io::Error::last_os_error();
                    if err.raw_os_error() != Some(libc::ESRCH) {
                        return Err(model_error(
                            AppErrorCode::ModelServerStopFailed,
                            "Model süreci zorla sonlandırılamadı.",
                            Some(err.to_string()),
                            None,
                        ));
                    }
                }
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            Err(model_error(
                AppErrorCode::ModelServerStopFailed,
                "Bu platformda süreç sonlandırma desteklenmiyor.",
                None,
                None,
            ))
        }
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
                    "Gemma model sunucusu çalışmıyor.",
                    Some(format!(
                        "step={step_name}; profile_id={}",
                        status.profile_id
                    )),
                    Some("Model sunucusunu manuel başlatın veya managed moda geçin.".to_string()),
                )
            });
            error.technical_details =
                Some(model_step_details(step_name, &status, false, None, None));
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
                            "Gemma model sunucusu başlatılamadı.",
                            Some(format!(
                                "step={step_name}; profile_id={}",
                                ready_status.profile_id
                            )),
                            Some(
                                "Model ayarlarını kontrol edin veya modeli elle başlatın."
                                    .to_string(),
                            ),
                        )
                    });
                    error.code = AppErrorCode::ModelServerStartFailed;
                    error.message = "Gemma model sunucusu başlatılamadı.".to_string();
                    error.suggested_action = Some(
                        "Model ayarlarını kontrol edin veya modeli elle başlatın.".to_string(),
                    );
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
                let step_details =
                    model_step_details(step_name, &status, true, None, Some(&start_error));
                let mut error = AppError {
                    code: AppErrorCode::ModelServerStartFailed,
                    message: "Gemma model sunucusu başlatılamadı.".to_string(),
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

impl Drop for ModelProcessManager {
    fn drop(&mut self) {
        let runtime = self
            .runtime
            .lock()
            .map(|runtime| runtime.clone())
            .unwrap_or_default();
        for metadata in runtime {
            if metadata.started_by_app {
                if let Some(pid) = metadata.pid {
                    let _ = self.stop_pid(pid);
                }
            }
        }
        let _ = clear_persisted_state(&self.persisted_state_path);
    }
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
        health_ok_after_start: after
            .is_some_and(|status| status.server_running && status.health_ok),
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
    use crate::services::model_config_service::ModelConfigService;
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
        let path = env::temp_dir().join(format!("rubrika-mock-llama-{}.sh", Uuid::new_v4()));
        let script = r#"#!/bin/sh
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  cat <<'EOF'
--cache-type-k
--cache-type-v
--mmproj-offload
EOF
  exit 0
fi
host=127.0.0.1
port=8080
while [ "$#" -gt 0 ]; do
  case "$1" in
    --host)
      host="$2"
      shift 2
      ;;
    --port)
      port="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
if [ "__MODE__" = "fail" ]; then
  exit 1
fi
exec python3 - "$host" "$port" <<'PY'
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

host = sys.argv[1]
port = int(sys.argv[2])

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
PY
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
        let err =
            build_model_server_args(&profile, &SupportFlags::from_help_output(help)).unwrap_err();
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
    fn stop_pid_terminates_process() {
        let mut child = StdCommand::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep should be available");
        let pid = child.id();
        let service = ModelProcessManager::new_with_state_path(
            test_service(),
            Arc::new(LlamaServerGateway::default()),
            env::temp_dir().join(format!("rubrika-state-{}.json", Uuid::new_v4())),
        );
        service.stop_pid(pid).expect("stop_pid should succeed");
        let _ = child.wait();
        assert!(child.try_wait().unwrap().is_some());
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
    fn get_model_status_avoids_completion_probe() {
        let Some((base_url, host, port, completion_count, _handle)) = spawn_model_probe_server()
        else {
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
    fn ensure_model_ready_skips_start_when_server_is_already_healthy() {
        let Some((base_url, host, port, _completion_count, _handle)) = spawn_model_probe_server()
        else {
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

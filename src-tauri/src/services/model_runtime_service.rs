use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{ModelMode, ModelProfile, ModelStatus};
use crate::services::model_config_service::ModelConfigService;
use crate::services::model_process_manager::ModelProcessManager;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelUseCase {
    QuestionTextExtraction,
    RubricPdfImport,
    StudentAnswerOcr,
    StudentAnswerOcrIssueCorrection,
    Scoring,
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
            Self::GeneralText => "general_text",
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
}

#[derive(Clone)]
pub struct ModelRuntimeService {
    config_service: ModelConfigService,
    process_manager: ModelProcessManager,
}

impl ModelRuntimeService {
    pub fn new(config_service: ModelConfigService, process_manager: ModelProcessManager) -> Self {
        Self {
            config_service,
            process_manager,
        }
    }

    pub async fn ensure_ready(
        &self,
        profile_id: Option<&str>,
        request: ModelRuntimeRequest,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let profile = self.config_service.get_profile(profile_id)?;
        let status = self.get_runtime_status(profile_id, &request).await?;
        if status.health_ok {
            return Ok(status);
        }

        let mut current_status = self.get_model_status(profile_id).await?;
        if let Some(error) = model_readiness_error(&profile, &current_status, &request) {
            return Err(error);
        }

        if current_status.start_requires_mode_change {
            let _ = self.set_mode(profile_id, ModelMode::Managed).await?;
        }

        let started = match self
            .process_manager
            .start_server_with_timeout(profile_id, Duration::from_secs(request.timeout_seconds))
            .await
        {
            Ok(output) => output,
            Err(error) => return Err(normalize_model_error(error)),
        };
        if !started.health_ok {
            current_status = self.get_model_status(profile_id).await?;
            let mut error = current_status
                .last_error
                .clone()
                .map(normalize_model_error)
                .unwrap_or_else(|| AppError {
                    code: AppErrorCode::ModelStartFailed,
                    message: "Gemma model sunucusu başlatılamadı.".to_string(),
                    recoverable: true,
                    suggested_action: Some(
                        "Model ayarlarını kontrol edin veya modeli elle başlatın.".to_string(),
                    ),
                    technical_details: Some(format!(
                        "step={}; profile_id={}",
                        request.use_case.step_name(),
                        current_status.profile_id
                    )),
                    correlation_id: uuid::Uuid::new_v4().to_string(),
                });
            if matches!(
                error.code,
                AppErrorCode::ModelServerReadyTimeout | AppErrorCode::ModelStartTimeout
            ) {
                error.code = AppErrorCode::ModelStartTimeout;
            } else {
                error.code = AppErrorCode::ModelStartFailed;
            }
            error.message = if matches!(error.code, AppErrorCode::ModelStartTimeout) {
                "Gemma model sunucusu zamanında hazır olmadı.".to_string()
            } else {
                "Gemma model sunucusu başlatılamadı.".to_string()
            };
            error.suggested_action =
                Some("Model ayarlarını kontrol edin veya modeli elle başlatın.".to_string());
            error.technical_details = Some(format!(
                "step={}; profile_id={}; started={:?}",
                request.use_case.step_name(),
                current_status.profile_id,
                started
            ));
            return Err(error);
        }

        self.get_runtime_status(profile_id, &request).await
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
        self.process_manager.set_mode(profile_id, mode).await
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
        Ok(build_runtime_status(request, &profile, status))
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
    }
}

fn model_readiness_error(
    profile: &ModelProfile,
    status: &ModelStatus,
    request: &ModelRuntimeRequest,
) -> Option<AppError> {
    if profile.server_path.trim().is_empty()
        && profile.model_path.trim().is_empty()
        && profile.mmproj_path.trim().is_empty()
    {
        return Some(AppError {
            code: AppErrorCode::ModelConfigMissing,
            message: "Model yapılandırması eksik.".to_string(),
            recoverable: true,
            suggested_action: Some("Model profilini yapılandırın.".to_string()),
            technical_details: Some(format!(
                "step={}; profile_id={}; server_path_empty=true; model_path_empty=true; mmproj_path_empty=true",
                request.use_case.step_name(),
                status.profile_id
            )),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    if !status.server_path_exists {
        return Some(AppError {
            code: AppErrorCode::ModelBinaryMissing,
            message: "llama-server binary bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("Llama-server binary yolunu kontrol edin.".to_string()),
            technical_details: Some(format!(
                "step={}; profile_id={}; server_path_exists=false",
                request.use_case.step_name(),
                status.profile_id
            )),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    if !status.model_path_exists {
        return Some(AppError {
            code: AppErrorCode::ModelFileMissing,
            message: "Model dosyası bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("GGUF model yolunu kontrol edin.".to_string()),
            technical_details: Some(format!(
                "step={}; profile_id={}; model_path_exists=false",
                request.use_case.step_name(),
                status.profile_id
            )),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    if request.requires_mmproj && !status.mmproj_path_exists {
        return Some(AppError {
            code: AppErrorCode::ModelMmprojMissing,
            message: "MMProj dosyası bulunamadı.".to_string(),
            recoverable: true,
            suggested_action: Some("MMProj yolunu kontrol edin.".to_string()),
            technical_details: Some(format!(
                "step={}; profile_id={}; mmproj_path_exists=false",
                request.use_case.step_name(),
                status.profile_id
            )),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    if status.server_running && !status.started_by_app && !status.health_ok {
        return Some(AppError {
            code: AppErrorCode::ModelPortBlocked,
            message: "Model portu başka bir süreç tarafından kullanılıyor.".to_string(),
            recoverable: true,
            suggested_action: Some("Mevcut model sürecini kontrol edin.".to_string()),
            technical_details: Some(format!(
                "step={}; profile_id={}; port={}; server_running=true; started_by_app=false",
                request.use_case.step_name(),
                status.profile_id,
                profile.port
            )),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }
    None
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
        Err(e) => vec![format!("Failed to open log file: {e}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{ModelMode, ModelProfile, ModelRuntimePreset, ModelStatus};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

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

    fn write_unhealthy_mock_llama_server_script() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rubrika-unhealthy-llama-{}.sh",
            uuid::Uuid::new_v4()
        ));
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
            self._write_json({"status": "booting"}, 503)
            return
        self._write_json({"error": "not found"}, 404)

HTTPServer((host, port), Handler).serve_forever()
PY
"#;
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn find_free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(0)
    }

    #[test]
    fn ensure_ready_times_out_with_short_timeout() {
        let config_service = ModelConfigService::new_with_path(std::env::temp_dir().join(format!(
            "rubrika-model-config-{}.json",
            uuid::Uuid::new_v4()
        )));
        let mut profile = config_service.get_profile(None).unwrap();
        profile.mode = ModelMode::Managed;
        profile.server_path = write_unhealthy_mock_llama_server_script()
            .to_string_lossy()
            .to_string();
        let model_path =
            std::env::temp_dir().join(format!("rubrika-model-{}.gguf", uuid::Uuid::new_v4()));
        let mmproj_path =
            std::env::temp_dir().join(format!("rubrika-mmproj-{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&model_path, b"dummy").unwrap();
        std::fs::write(&mmproj_path, b"dummy").unwrap();
        profile.model_path = model_path.to_string_lossy().to_string();
        profile.mmproj_path = mmproj_path.to_string_lossy().to_string();
        profile.port = find_free_port();
        profile.base_url = format!("http://127.0.0.1:{}", profile.port);
        config_service.update_profile(profile).unwrap();

        let manager = ModelProcessManager::new_with_state_path(
            config_service.clone(),
            Arc::new(crate::services::llama_server_gateway::LlamaServerGateway::default()),
            std::env::temp_dir().join(format!("rubrika-model-state-{}.json", uuid::Uuid::new_v4())),
        );
        let runtime = ModelRuntimeService::new(config_service, manager);
        let request = ModelRuntimeRequest {
            use_case: ModelUseCase::GeneralText,
            capability: ModelCapability::Text,
            requires_mmproj: true,
            timeout_seconds: 1,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let err = runtime.ensure_ready(None, request).await.unwrap_err();
            assert!(matches!(
                err.code,
                AppErrorCode::ModelStartTimeout | AppErrorCode::ModelPortBlocked
            ));
        });
    }
}

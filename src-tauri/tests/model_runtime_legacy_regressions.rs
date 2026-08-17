use std::sync::Arc;

use app_lib::domain::errors::AppErrorCode;
use app_lib::domain::model::{ModelMode, PrivacyMode};
use app_lib::services::llama_server_gateway::LlamaServerGateway;
use app_lib::services::model_config_service::ModelConfigService;
use app_lib::services::model_process_manager::ModelProcessManager;
use app_lib::services::model_runtime_service::{
    ModelCapability, ModelRuntimeRequest, ModelRuntimeService, ModelUseCase,
};

#[test]
fn strict_local_blocks_configured_public_external_student_runtime() {
    let config_service = ModelConfigService::new_with_path(std::env::temp_dir().join(format!(
        "rubrika-model-privacy-config-{}.json",
        uuid::Uuid::new_v4()
    )));
    let mut profile = config_service.get_profile(None).unwrap();
    profile.server_path = "/tmp/llama-server".to_string();
    profile.model_path = "/tmp/model.gguf".to_string();
    profile.base_url = "https://model.example.test".to_string();
    profile.host = "model.example.test".to_string();
    profile.mode = ModelMode::External;
    profile.privacy_mode = PrivacyMode::StrictLocal;
    config_service.update_profile(profile).unwrap();
    let manager = ModelProcessManager::new_with_state_path(
        config_service.clone(),
        Arc::new(LlamaServerGateway::default()),
        std::env::temp_dir().join(format!(
            "rubrika-model-privacy-state-{}.json",
            uuid::Uuid::new_v4()
        )),
    );
    let runtime = ModelRuntimeService::new(config_service, manager);
    let request = ModelRuntimeRequest {
        use_case: ModelUseCase::StudentAnswerOcr,
        capability: ModelCapability::Vision,
        requires_mmproj: false,
        timeout_seconds: 1,
    };
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(runtime.acquire_ready_runtime_lease(
            None,
            "privacy_test",
            request,
            "privacy-correlation",
        ));
    let error = match result {
        Ok(_) => panic!("public strict-local runtime must be blocked"),
        Err(error) => error,
    };
    assert_eq!(error.code, AppErrorCode::ModelPrivacyBlocked);
}

#[cfg(unix)]
fn write_unhealthy_mock_llama_server_script() -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

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

#[cfg(unix)]
fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .unwrap_or(0)
}

#[cfg(unix)]
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
        Arc::new(LlamaServerGateway::default()),
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
        let result = runtime
            .acquire_ready_runtime_lease(None, "runtime_test", request, "runtime-test")
            .await;
        let error = match result {
            Ok(_) => panic!("runtime should fail before acquiring a lease"),
            Err(error) => error,
        };
        assert!(matches!(
            error.code,
            AppErrorCode::ModelStartTimeout
                | AppErrorCode::ModelServerReadyTimeout
                | AppErrorCode::ModelRuntimeReadinessTimeout
                | AppErrorCode::ModelServerStartFailed
                | AppErrorCode::ModelPortBlocked
        ));
    });
}

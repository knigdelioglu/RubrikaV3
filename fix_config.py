f = "src-tauri/src/services/model_config_service.rs"
with open(f, "r") as file:
    content = file.read()

import re

# We will replace `.lock().unwrap()` with a map_err that returns an AppError
# using the AppErrorCode::ModelStateAccessFailed.
# This requires adding `uuid::Uuid` if not imported (it's likely imported since it's used nearby)

def replacer(match):
    return """lock().map_err(|err| AppError {
            code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
            message: "Model konfigürasyonuna erişilemedi.".to_string(),
            recoverable: false,
            suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
            technical_details: Some(format!("Mutex lock failed: {}", err)),
            correlation_id: Uuid::new_v4().to_string(),
        })?"""

# Replace .lock().unwrap() -> .lock().map_err(...)
new_content = content.replace("lock().unwrap()", """lock().map_err(|err| crate::domain::errors::AppError {
            code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
            message: "Model konfigürasyonuna erişilemedi.".to_string(),
            recoverable: false,
            suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
            technical_details: Some(format!("Mutex lock failed: {}", err)),
            correlation_id: Uuid::new_v4().to_string(),
        })?""")

# We also need to import `Uuid` if not there, but `Uuid` is already used on line 55.
# `AppError` is also imported.

with open(f, "w") as file:
    file.write(new_content)

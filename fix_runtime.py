f = "src-tauri/src/services/model_process_manager.rs"
with open(f, "r") as file:
    content = file.read()

import re

# Replace `self.runtime.lock().unwrap()`
# Wait, let's just use `unwrap_or_else(|e| e.into_inner())` like we did for active_profile_id,
# since there are no other unwraps that we need to return an error for, or we can use map_err
# map_err is already used in this file for `runtime` locks:
# `let mut runtime = self.runtime.lock().map_err(|e| ...)?`
# Let's replace `self.runtime.lock().unwrap()` with the same `map_err`.

pattern = r"self\s*\.runtime\s*\.lock\(\)\s*\.unwrap\(\)"
replacement = """self.runtime.lock().map_err(|err| AppError {
                    code: AppErrorCode::ModelStateAccessFailed,
                    message: "Model çalışma zamanı durumuna erişilemedi.".to_string(),
                    recoverable: false,
                    suggested_action: Some("Uygulamayı yeniden başlatın.".to_string()),
                    technical_details: Some(format!("Mutex lock failed: {}", err)),
                    correlation_id: Uuid::new_v4().to_string(),
                })?"""

new_content = re.sub(pattern, replacement, content)

with open(f, "w") as file:
    file.write(new_content)

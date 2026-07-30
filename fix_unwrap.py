f = "src-tauri/src/services/model_process_manager.rs"
with open(f, "r") as file:
    lines = file.readlines()

replacement = """.lock().map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model durumuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Lütfen uygulamayı yeniden başlatın.".to_string()),
                technical_details: Some(format!("Mutex poison error: {}", err)),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            })?"""

for i in [190, 222, 242, 297, 559]:  # 0-indexed line numbers
    lines[i] = lines[i].replace(".lock().unwrap()", replacement)

with open(f, "w") as file:
    file.writelines(lines)

use crate::domain::errors::AppError;
use crate::domain::workflow::AppStatus;

#[tauri::command]
pub async fn get_app_status() -> Result<AppStatus, AppError> {
    Ok(AppStatus {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        tauri_ready: true,
        rust_backend_ready: true,
    })
}

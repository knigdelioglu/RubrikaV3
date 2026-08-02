use std::path::Path;

use crate::diagnostics::{DataLossPreflightReport, DiagnosticsContext};
use crate::domain::errors::AppError;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataLossPreflightInput {
    pub project_path: String,
}

/// Read-only, backend-authoritative safety report used before opening a
/// project for writes or starting a destructive operation. This command does
/// not acquire the project writer lease and never repairs or migrates files.
#[tauri::command]
pub async fn get_data_loss_preflight(
    _state: State<'_, crate::AppState>,
    input: DataLossPreflightInput,
) -> Result<DataLossPreflightReport, AppError> {
    DiagnosticsContext::new().data_loss_preflight(Path::new(&input.project_path))
}

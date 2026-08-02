use serde::Deserialize;
use tauri::State;

use crate::domain::errors::AppError;
use crate::services::generation_gc_service::{self, GcReport};
use crate::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGenerationGcInput {
    pub project_id: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[tauri::command]
pub async fn run_generation_gc(
    state: State<'_, AppState>,
    input: RunGenerationGcInput,
) -> Result<GcReport, AppError> {
    let policy = generation_gc_service::GenerationGcPolicy::default();
    generation_gc_service::run_generation_gc_transaction(
        &state.project_store,
        &input.project_id,
        input.dry_run,
        &policy,
    )
}

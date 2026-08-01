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
    let project = state
        .project_store
        .get_project_snapshot(input.project_id.clone())?;
    let trusted_root = crate::platform::project_paths::TrustedProjectRoot::from_canonical_root(
        std::path::PathBuf::from(&project.root_path),
        false,
    )?;
    let report = generation_gc_service::run_generation_gc(
        &trusted_root,
        &project,
        input.dry_run,
        &generation_gc_service::GenerationGcPolicy::default(),
    )?;

    if !input.dry_run {
        let deletable = generation_gc_service::ocr_cleanup_plan(&project);
        if !deletable.is_empty() {
            let deleted_set: std::collections::HashSet<String> = deletable.into_iter().collect();
            let project_id = input.project_id.clone();
            let _ = state.project_store.mutate(
                &project_id,
                crate::services::project_store::MutationOptions::new("generation_gc"),
                move |current, _| {
                    current
                        .student_answer_ocr_generations
                        .retain(|generation| !deleted_set.contains(&generation.generation_id));
                    Ok(())
                },
            )?;
        }
    }

    Ok(report)
}

use crate::domain::errors::AppError;
use crate::services::exam_package_build_service::StartExamPackageBuildOutput;
use crate::AppState;
use tauri::{AppHandle, State};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartExamPackageBuildInput {
    pub project_id: String,
    pub expected_question_count: u32,
}

#[tauri::command]
pub async fn start_exam_package_build(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartExamPackageBuildInput,
) -> Result<StartExamPackageBuildOutput, AppError> {
    state
        .exam_package_build_service
        .start(app, input.project_id, input.expected_question_count)
        .await
}

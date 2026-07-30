use crate::domain::errors::AppError;
use crate::domain::project::Project;
use crate::domain::question::Question;
use crate::services::rubric_extraction_service::{StartJobOutput, StartRubricPdfImportInput};
use crate::services::rubric_service::{
    ImportRubricJsonInput, ImportRubricJsonOutput, RubricQuestionSnapshot, RubricStateSnapshot,
    RubricValidationReport, UpdateQuestionRubricInput,
};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn import_rubric_json(
    state: State<'_, AppState>,
    input: ImportRubricJsonInput,
) -> Result<ImportRubricJsonOutput, AppError> {
    state.rubric_service.import_rubric_json(input)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRubricStateInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn get_rubric_state(
    state: State<'_, AppState>,
    input: GetRubricStateInput,
) -> Result<RubricStateSnapshot, AppError> {
    state.rubric_service.get_rubric_state(&input.project_id)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRubricItemsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn list_rubric_items(
    state: State<'_, AppState>,
    input: ListRubricItemsInput,
) -> Result<Vec<RubricQuestionSnapshot>, AppError> {
    state.rubric_service.list_rubric_items(&input.project_id)
}

#[tauri::command]
pub async fn update_question_rubric(
    state: State<'_, AppState>,
    input: UpdateQuestionRubricInput,
) -> Result<Question, AppError> {
    state.rubric_service.update_question_rubric(input)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmQuestionRubricInput {
    pub project_id: String,
    pub question_id: String,
}

#[tauri::command]
pub async fn confirm_question_rubric(
    state: State<'_, AppState>,
    input: ConfirmQuestionRubricInput,
) -> Result<Question, AppError> {
    state
        .rubric_service
        .confirm_question_rubric(&input.project_id, &input.question_id)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmAllRubricsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn confirm_all_rubrics(
    state: State<'_, AppState>,
    input: ConfirmAllRubricsInput,
) -> Result<Project, AppError> {
    state.rubric_service.confirm_all_rubrics(&input.project_id)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRubricsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn validate_rubrics(
    state: State<'_, AppState>,
    input: ValidateRubricsInput,
) -> Result<RubricValidationReport, AppError> {
    state.rubric_service.validate_rubrics(&input.project_id)
}

#[tauri::command]
pub async fn start_rubric_pdf_import(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartRubricPdfImportInput,
) -> Result<StartJobOutput, AppError> {
    state
        .rubric_extraction_service
        .start_import(app, input)
        .await
}

use crate::domain::errors::AppError;
use crate::domain::job::JobSnapshot;
use crate::domain::model::{QuestionTextExtractionStatus, QuestionTextSuggestion};
use crate::domain::project::Project;
use crate::domain::question::Question;
use crate::services::question_text_service::QuestionTextSource;
use crate::AppState;
use tauri::{AppHandle, State};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartQuestionTextExtractionInput {
    pub project_id: String,
    pub document_id: Option<String>,
    pub source: QuestionTextSource,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartQuestionTextVisionFallbackInput {
    pub project_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTextProjectInput {
    pub project_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmQuestionTextInput {
    pub project_id: String,
    pub question_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditQuestionTextInput {
    pub project_id: String,
    pub question_id: String,
    pub text: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmAllQuestionTextsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn start_question_text_extraction(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartQuestionTextExtractionInput,
) -> Result<JobSnapshot, AppError> {
    state
        .question_text_service
        .start_extraction(app, input.project_id, input.document_id, input.source)
        .await
}

#[tauri::command]
pub async fn start_question_text_vision_fallback(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartQuestionTextVisionFallbackInput,
) -> Result<JobSnapshot, AppError> {
    state
        .question_text_service
        .start_vision_fallback(app, input.project_id)
        .await
}

#[tauri::command]
pub async fn get_question_text_extraction_status(
    state: State<'_, AppState>,
    input: QuestionTextProjectInput,
) -> Result<QuestionTextExtractionStatus, AppError> {
    state
        .question_text_service
        .get_extraction_status(&input.project_id)
        .await
}

#[tauri::command]
pub async fn list_question_text_suggestions(
    state: State<'_, AppState>,
    input: QuestionTextProjectInput,
) -> Result<Vec<QuestionTextSuggestion>, AppError> {
    state
        .question_text_service
        .list_suggestions(&input.project_id)
}

#[tauri::command]
pub async fn confirm_question_text(
    state: State<'_, AppState>,
    input: ConfirmQuestionTextInput,
) -> Result<Question, AppError> {
    let question = state
        .question_text_service
        .confirm_question_text(&input.project_id, &input.question_id)?;
    super::audit_critical(
        &state,
        &input.project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "question_text_confirmed",
            "Soru metni öğretmen tarafından onaylandı.",
        )
        .entity("question", &input.question_id),
    )?;
    Ok(question)
}

#[tauri::command]
pub async fn confirm_all_question_texts(
    state: State<'_, AppState>,
    input: ConfirmAllQuestionTextsInput,
) -> Result<Project, AppError> {
    let project = state
        .question_text_service
        .confirm_all_question_texts(&input.project_id)?;
    super::audit_critical(
        &state,
        &input.project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "question_text_confirmed_all",
            "Soru metinleri öğretmen tarafından onaylandı.",
        ),
    )?;
    Ok(project)
}

#[tauri::command]
pub async fn edit_question_text(
    state: State<'_, AppState>,
    input: EditQuestionTextInput,
) -> Result<Question, AppError> {
    state.question_text_service.edit_question_text(
        &input.project_id,
        &input.question_id,
        input.text,
    )
}

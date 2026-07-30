use crate::domain::document::{Document, DocumentRole};
use crate::domain::errors::AppError;
use crate::services::document_service;
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDocumentInput {
    pub project_id: String,
    pub source_path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveDocumentInput {
    pub project_id: String,
    pub document_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDocumentsInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn import_exam_source_pdf(
    state: State<'_, AppState>,
    input: ImportDocumentInput,
) -> Result<Document, AppError> {
    document_service::import_document(
        &state.project_store,
        state.pdf_service.as_ref(),
        &input.project_id,
        &input.source_path,
        DocumentRole::ExamSource,
    )
}

#[tauri::command]
pub async fn import_answer_key_pdf(
    state: State<'_, AppState>,
    input: ImportDocumentInput,
) -> Result<Document, AppError> {
    document_service::import_document(
        &state.project_store,
        state.pdf_service.as_ref(),
        &input.project_id,
        &input.source_path,
        DocumentRole::AnswerKey,
    )
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    input: ListDocumentsInput,
) -> Result<Vec<Document>, AppError> {
    let project = state.project_store.get_project_snapshot(input.project_id)?;
    Ok(project.documents)
}

#[tauri::command]
pub async fn remove_document(
    state: State<'_, AppState>,
    input: RemoveDocumentInput,
) -> Result<(), AppError> {
    document_service::remove_document(&state.project_store, &input.project_id, &input.document_id)
}

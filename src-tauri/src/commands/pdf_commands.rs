use crate::domain::document::PdfPagePreview;
use crate::domain::errors::AppError;
use crate::services::pdf_preview_service::{PdfPreviewStatusSnapshot, StartPdfPreviewRenderOutput};
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfDocumentInput {
    pub project_id: String,
    pub document_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPagePreviewInput {
    pub project_id: String,
    pub document_id: String,
    pub page_number: u32,
}

#[tauri::command]
pub async fn get_pdf_page_count(
    state: State<'_, AppState>,
    input: PdfDocumentInput,
) -> Result<u32, AppError> {
    state
        .pdf_preview_service
        .get_pdf_page_count(&input.project_id, &input.document_id)
}

#[tauri::command]
pub async fn start_pdf_preview_render(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: PdfDocumentInput,
) -> Result<StartPdfPreviewRenderOutput, AppError> {
    state
        .pdf_preview_service
        .start_render(app, input.project_id, input.document_id)
}

#[tauri::command]
pub async fn get_pdf_preview_status(
    state: State<'_, AppState>,
    input: PdfDocumentInput,
) -> Result<PdfPreviewStatusSnapshot, AppError> {
    state
        .pdf_preview_service
        .get_pdf_preview_status(&input.project_id, &input.document_id)
}

#[tauri::command]
pub async fn get_pdf_page_preview(
    state: State<'_, AppState>,
    input: PdfPagePreviewInput,
) -> Result<PdfPagePreview, AppError> {
    state.pdf_preview_service.get_pdf_page_preview(
        &input.project_id,
        &input.document_id,
        input.page_number,
    )
}

#[tauri::command]
pub async fn list_pdf_page_previews(
    state: State<'_, AppState>,
    input: PdfDocumentInput,
) -> Result<Vec<PdfPagePreview>, AppError> {
    state
        .pdf_preview_service
        .list_pdf_page_previews(&input.project_id, &input.document_id)
}

#[tauri::command]
pub async fn get_pdf_renderer_status(
    state: State<'_, AppState>,
) -> Result<crate::services::pdf_service::PdfRendererStatus, AppError> {
    state.pdf_service.get_renderer_status()
}

use crate::commands::document_commands::ImportDocumentInput;
use crate::domain::document::Document;
use crate::domain::errors::AppError;
use crate::domain::project::Project;
use crate::domain::student::{StudentScanReadinessSnapshot, StudentSubmission};
use crate::services::document_service;
use crate::services::pdf_preview_service::{PdfPreviewStatusSnapshot, StartPdfPreviewRenderOutput};
use crate::services::student_scan_service::{
    CreateStudentPageGroupsInput, CreateStudentPageGroupsOutput, DeleteStudentSubmissionInput,
    GetOcrReadinessInput, MarkStudentGroupingCompleteInput, UpdateStudentIdentityInput,
    UpdateSubmissionPagesInput,
};
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIdInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn import_student_scan_pdf(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ImportDocumentInput,
) -> Result<Document, AppError> {
    document_service::import_document_with_job(
        &state.project_store,
        state.pdf_service.as_ref(),
        &input.project_id,
        &input.source_path,
        crate::domain::document::DocumentRole::StudentScan,
        Some((state.job_manager.as_ref(), &app)),
        input.correlation_id,
    )
}

#[tauri::command]
pub async fn list_student_scan_documents(
    state: State<'_, AppState>,
    input: ProjectIdInput,
) -> Result<Vec<Document>, AppError> {
    state
        .student_scan_service
        .list_student_scan_documents(&input.project_id)
}

#[tauri::command]
pub async fn start_student_scan_preview_render(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: crate::commands::pdf_commands::PdfDocumentInput,
) -> Result<StartPdfPreviewRenderOutput, AppError> {
    state
        .pdf_preview_service
        .start_render(app, input.project_id, input.document_id)
}

#[tauri::command]
pub async fn get_student_scan_preview_status(
    state: State<'_, AppState>,
    input: crate::commands::pdf_commands::PdfDocumentInput,
) -> Result<PdfPreviewStatusSnapshot, AppError> {
    state
        .pdf_preview_service
        .get_pdf_preview_status(&input.project_id, &input.document_id)
}

#[tauri::command]
pub async fn create_student_page_groups(
    state: State<'_, AppState>,
    input: CreateStudentPageGroupsInput,
) -> Result<CreateStudentPageGroupsOutput, AppError> {
    state.student_scan_service.create_student_page_groups(input)
}

#[tauri::command]
pub async fn list_student_submissions(
    state: State<'_, AppState>,
    input: ProjectIdInput,
) -> Result<Vec<StudentSubmission>, AppError> {
    state
        .student_scan_service
        .list_student_submissions(&input.project_id)
}

#[tauri::command]
pub async fn update_student_identity(
    state: State<'_, AppState>,
    input: UpdateStudentIdentityInput,
) -> Result<StudentSubmission, AppError> {
    state.student_scan_service.update_student_identity(input)
}

#[tauri::command]
pub async fn update_submission_pages(
    state: State<'_, AppState>,
    input: UpdateSubmissionPagesInput,
) -> Result<StudentSubmission, AppError> {
    state.student_scan_service.update_submission_pages(input)
}

#[tauri::command]
pub async fn delete_student_submission(
    state: State<'_, AppState>,
    input: DeleteStudentSubmissionInput,
) -> Result<(), AppError> {
    state.student_scan_service.delete_student_submission(input)
}

#[tauri::command]
pub async fn mark_student_grouping_complete(
    state: State<'_, AppState>,
    input: MarkStudentGroupingCompleteInput,
) -> Result<Project, AppError> {
    state
        .student_scan_service
        .mark_student_grouping_complete(input)
}

#[tauri::command]
pub async fn get_ocr_readiness(
    state: State<'_, AppState>,
    input: GetOcrReadinessInput,
) -> Result<StudentScanReadinessSnapshot, AppError> {
    state.student_scan_service.get_ocr_readiness(&input)
}

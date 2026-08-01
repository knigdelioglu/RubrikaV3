use crate::domain::errors::AppError;
use crate::domain::project::Project;
use crate::domain::student::{OcrGeneration, OcrImagePreprocessMode};
use crate::domain::student::{
    StudentAnswerCropTemplateItem, StudentAnswerOcrRecord, StudentIdentityCropTemplate,
};
use crate::services::ocr_image_preprocess_service::OcrImagePreprocessResult;
use crate::services::student_answer_ocr_service::{
    RebuildStudentAnswerOcrIssuesOutput, StartStudentAnswerOcrOutput,
    StartStudentIdentityOcrOutput, SuggestStudentAnswerOcrIssueCorrectionWithModelOutput,
};
use crate::AppState;
use std::path::Path;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIdInput {
    pub project_id: String,
    #[serde(default)]
    pub force_rerun: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStudentAnswerOcrTextInput {
    pub project_id: String,
    pub submission_id: String,
    pub question_id: String,
    pub text: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkStudentAnswerOcrReviewedInput {
    pub project_id: String,
    pub submission_id: String,
    pub question_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrGenerationInput {
    pub project_id: String,
    pub generation_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStudentAnswerCropTemplateInput {
    pub project_id: String,
    pub items: Vec<StudentAnswerCropTemplateItem>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStudentIdentityCropTemplateInput {
    pub project_id: String,
    pub template: StudentIdentityCropTemplate,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreprocessOcrImageInput {
    pub project_id: String,
    pub image_path: String,
    #[serde(default)]
    pub mode: OcrImagePreprocessMode,
}

#[tauri::command]
pub async fn start_student_answer_ocr(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ProjectIdInput,
) -> Result<StartStudentAnswerOcrOutput, AppError> {
    state
        .student_answer_ocr_service
        .start(app, input.project_id, input.force_rerun)
        .await
}

#[tauri::command]
pub async fn start_student_identity_ocr(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ProjectIdInput,
) -> Result<StartStudentIdentityOcrOutput, AppError> {
    state
        .student_answer_ocr_service
        .start_identity_ocr(app, input.project_id)
        .await
}

#[tauri::command]
pub async fn update_student_answer_ocr_text(
    state: State<'_, AppState>,
    input: UpdateStudentAnswerOcrTextInput,
) -> Result<StudentAnswerOcrRecord, AppError> {
    state.student_answer_ocr_service.update_student_answer_text(
        &input.project_id,
        &input.submission_id,
        &input.question_id,
        input.text,
    )
}

#[tauri::command]
pub async fn mark_student_answer_ocr_reviewed(
    state: State<'_, AppState>,
    input: MarkStudentAnswerOcrReviewedInput,
) -> Result<StudentAnswerOcrRecord, AppError> {
    state
        .student_answer_ocr_service
        .mark_student_answer_reviewed(&input.project_id, &input.submission_id, &input.question_id)
}

#[tauri::command]
pub async fn mark_all_student_answer_ocr_reviewed(
    state: State<'_, AppState>,
    input: ProjectIdInput,
) -> Result<Project, AppError> {
    state
        .student_answer_ocr_service
        .mark_all_student_answers_reviewed(&input.project_id)
}

#[tauri::command]
pub async fn accept_student_answer_ocr_generation(
    state: State<'_, AppState>,
    input: OcrGenerationInput,
) -> Result<OcrGeneration, AppError> {
    let generation = state
        .student_answer_ocr_service
        .accept_student_answer_ocr_generation(&input.project_id, &input.generation_id)?;
    super::audit_critical(
        &state,
        &input.project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "ocr_generation_accepted",
            "OCR nesli öğretmen tarafından kabul edildi.",
        )
        .entity("ocr_generation", &input.generation_id),
    )?;
    Ok(generation)
}

#[tauri::command]
pub async fn reject_student_answer_ocr_generation(
    state: State<'_, AppState>,
    input: OcrGenerationInput,
) -> Result<OcrGeneration, AppError> {
    let generation = state
        .student_answer_ocr_service
        .reject_student_answer_ocr_generation(&input.project_id, &input.generation_id)?;
    super::audit_critical(
        &state,
        &input.project_id,
        crate::services::audit_service::AuditEntryInput::new(
            "ocr_generation_rejected",
            "OCR nesli öğretmen tarafından reddedildi.",
        )
        .entity("ocr_generation", &input.generation_id),
    )?;
    Ok(generation)
}

#[tauri::command]
pub async fn save_student_answer_crop_template(
    state: State<'_, AppState>,
    input: SaveStudentAnswerCropTemplateInput,
) -> Result<Project, AppError> {
    state
        .student_answer_crop_service
        .save_template(&input.project_id, input.items)
}

#[tauri::command]
pub async fn save_student_identity_crop_template(
    state: State<'_, AppState>,
    input: SaveStudentIdentityCropTemplateInput,
) -> Result<Project, AppError> {
    state
        .student_answer_crop_service
        .save_identity_template(&input.project_id, input.template)
}

#[tauri::command]
pub async fn preprocess_ocr_image(
    state: State<'_, AppState>,
    input: PreprocessOcrImageInput,
) -> Result<OcrImagePreprocessResult, AppError> {
    let project = state
        .project_store
        .get_project_snapshot(input.project_id.clone())?;
    state.ocr_image_preprocess_service.preprocess_image(
        Path::new(&project.root_path),
        Path::new(&input.image_path),
        input.mode,
    )
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildStudentAnswerOcrIssuesInput {
    pub project_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestOcrIssueCorrectionWithModelInput {
    pub project_path: String,
    pub ocr_record_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_id: Option<String>,
    pub observed_text: String,
    pub suggested_text_from_analyzer: String,
    pub question_number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_region: Option<crate::domain::student::StudentAnswerOcrCropBBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_input_crop_ref: Option<String>,
}

#[tauri::command]
pub async fn rebuild_student_answer_ocr_issues(
    state: State<'_, AppState>,
    input: RebuildStudentAnswerOcrIssuesInput,
) -> Result<RebuildStudentAnswerOcrIssuesOutput, AppError> {
    state
        .student_answer_ocr_service
        .rebuild_issues(&input.project_id)
}

#[tauri::command]
pub async fn suggest_ocr_issue_correction_with_model(
    state: State<'_, AppState>,
    input: SuggestOcrIssueCorrectionWithModelInput,
) -> Result<SuggestStudentAnswerOcrIssueCorrectionWithModelOutput, AppError> {
    state
        .student_answer_ocr_service
        .suggest_ocr_issue_correction_with_model(
            input.project_path,
            input.ocr_record_id,
            input.issue_id,
            input.observed_text,
            input.suggested_text_from_analyzer,
            input.question_number,
            input.highlight_region,
            input.crop_ref,
            input.model_input_crop_ref,
        )
        .await
}

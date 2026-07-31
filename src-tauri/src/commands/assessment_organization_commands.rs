use tauri::State;

use crate::domain::assessment::{
    AssessmentActivity, AssessmentClassApplication, TeachingAssignment,
};
use crate::domain::errors::AppError;
use crate::services::assessment_organization_service::{
    AddAssessmentClassApplicationInput, AssessmentActivityIdInput,
    AssessmentClassApplicationIdInput, AttachAssessmentDocumentInput, ClassApplicationIdInput,
    CreateAssessmentActivityInput, GetAssessmentSequenceOptionsInput,
    GetClassApplicationStudentsInput, ListAssessmentActivitiesInput, UpdateAssessmentActivityInput,
};
use crate::services::school_class_service::{
    CreateTeachingAssignmentInput, ListAssessmentClassesInput, ListTeachingAssignmentsInput,
};
use crate::{domain::school_class::SchoolClass, AppState};

#[tauri::command]
pub async fn list_assessment_activities(
    state: State<'_, AppState>,
    input: ListAssessmentActivitiesInput,
) -> Result<Vec<AssessmentActivity>, AppError> {
    state.assessment_organization_service.list_activities(input)
}

#[tauri::command]
pub async fn get_assessment_sequence_options(
    state: State<'_, AppState>,
    input: GetAssessmentSequenceOptionsInput,
) -> Result<crate::services::assessment_organization_service::AssessmentSequenceOptions, AppError> {
    state
        .assessment_organization_service
        .sequence_options(input)
}

#[tauri::command]
pub async fn list_assessment_classes(
    state: State<'_, AppState>,
    input: ListAssessmentClassesInput,
) -> Result<Vec<SchoolClass>, AppError> {
    state.school_class_service.list_assessment_classes(input)
}

#[tauri::command]
pub async fn get_assessment_activity(
    state: State<'_, AppState>,
    input: AssessmentActivityIdInput,
) -> Result<AssessmentActivity, AppError> {
    state.assessment_organization_service.get_activity(input)
}

#[tauri::command]
pub async fn get_assessment_class_applications(
    state: State<'_, AppState>,
    input: AssessmentActivityIdInput,
) -> Result<Vec<AssessmentClassApplication>, AppError> {
    state
        .assessment_organization_service
        .get_class_applications(input)
}

#[tauri::command]
pub async fn get_class_application_students(
    state: State<'_, AppState>,
    input: GetClassApplicationStudentsInput,
) -> Result<Vec<crate::domain::student::Student>, AppError> {
    state
        .assessment_organization_service
        .get_class_application_students(input)
}

#[tauri::command]
pub async fn update_assessment_activity(
    state: State<'_, AppState>,
    input: UpdateAssessmentActivityInput,
) -> Result<AssessmentActivity, AppError> {
    state.assessment_organization_service.update_activity(input)
}

#[tauri::command]
pub async fn create_assessment_activity(
    state: State<'_, AppState>,
    input: CreateAssessmentActivityInput,
) -> Result<AssessmentActivity, AppError> {
    state.assessment_organization_service.create_activity(input)
}

#[tauri::command]
pub async fn add_assessment_class_application(
    state: State<'_, AppState>,
    input: AddAssessmentClassApplicationInput,
) -> Result<AssessmentClassApplication, AppError> {
    state
        .assessment_organization_service
        .add_class_application(input)
}

#[tauri::command]
pub async fn archive_assessment_class_application(
    state: State<'_, AppState>,
    input: AssessmentClassApplicationIdInput,
) -> Result<AssessmentClassApplication, AppError> {
    state
        .assessment_organization_service
        .archive_class_application(input)
}

#[tauri::command]
pub async fn remove_assessment_class_application(
    state: State<'_, AppState>,
    input: ClassApplicationIdInput,
) -> Result<AssessmentClassApplication, AppError> {
    state
        .assessment_organization_service
        .remove_class_application(input)
}

#[tauri::command]
pub async fn attach_assessment_document(
    state: State<'_, AppState>,
    input: AttachAssessmentDocumentInput,
) -> Result<AssessmentActivity, AppError> {
    state.assessment_organization_service.attach_document(input)
}

#[tauri::command]
pub async fn list_teaching_assignments(
    state: State<'_, AppState>,
    input: ListTeachingAssignmentsInput,
) -> Result<Vec<TeachingAssignment>, AppError> {
    state.school_class_service.list_teaching_assignments(input)
}

#[tauri::command]
pub async fn create_teaching_assignment(
    state: State<'_, AppState>,
    input: CreateTeachingAssignmentInput,
) -> Result<TeachingAssignment, AppError> {
    state.school_class_service.create_teaching_assignment(input)
}

#[tauri::command]
pub async fn archive_teaching_assignment(
    state: State<'_, AppState>,
    input: crate::services::school_class_service::TeachingAssignmentIdInput,
) -> Result<TeachingAssignment, AppError> {
    state
        .school_class_service
        .archive_teaching_assignment(input)
}

#[tauri::command]
pub async fn batch_create_teaching_assignments(
    state: State<'_, AppState>,
    input: crate::services::school_class_service::BatchCreateTeachingAssignmentsInput,
) -> Result<Vec<TeachingAssignment>, AppError> {
    state
        .school_class_service
        .batch_create_teaching_assignments(input)
}

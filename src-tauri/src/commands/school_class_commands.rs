use tauri::State;

use crate::domain::errors::AppError;
use crate::domain::school_class::{SchoolClass, StudentScanBatch};
use crate::services::school_class_service::{
    CreateClassStudentInput, CreateSchoolClassInput, CreateStudentScanBatchInput,
    GetSchoolClassOverviewInput, ImportStudentScanBatchInput, ImportStudentScanBatchOutput,
    ListClassStudentsInput, ListSchoolClassesInput, ListStudentScanBatchesInput,
    MoveStudentScanBatchInput, RemoveStudentScanBatchInput, SchoolClassIdInput,
    SchoolClassOverviewSnapshot, UpdateClassStudentInput, UpdateSchoolClassInput,
};
use crate::AppState;

#[tauri::command]
pub async fn list_school_classes(
    state: State<'_, AppState>,
    input: ListSchoolClassesInput,
) -> Result<Vec<SchoolClass>, AppError> {
    state.school_class_service.list_school_classes(input)
}

#[tauri::command]
pub async fn get_school_class_overview(
    state: State<'_, AppState>,
    input: GetSchoolClassOverviewInput,
) -> Result<SchoolClassOverviewSnapshot, AppError> {
    state.school_class_service.get_school_class_overview(input)
}

#[tauri::command]
pub async fn list_class_students(
    state: State<'_, AppState>,
    input: ListClassStudentsInput,
) -> Result<Vec<crate::domain::student::Student>, AppError> {
    state.school_class_service.list_class_students(input)
}

#[tauri::command]
pub async fn create_class_student(
    state: State<'_, AppState>,
    input: CreateClassStudentInput,
) -> Result<crate::domain::student::Student, AppError> {
    state.school_class_service.create_class_student(input)
}

#[tauri::command]
pub async fn update_class_student(
    state: State<'_, AppState>,
    input: UpdateClassStudentInput,
) -> Result<crate::domain::student::Student, AppError> {
    state.school_class_service.update_class_student(input)
}

#[tauri::command]
pub async fn create_school_class(
    state: State<'_, AppState>,
    input: CreateSchoolClassInput,
) -> Result<SchoolClass, AppError> {
    state.school_class_service.create_school_class(input)
}

#[tauri::command]
pub async fn update_school_class(
    state: State<'_, AppState>,
    input: UpdateSchoolClassInput,
) -> Result<SchoolClass, AppError> {
    state.school_class_service.update_school_class(input)
}

#[tauri::command]
pub async fn archive_school_class(
    state: State<'_, AppState>,
    input: SchoolClassIdInput,
) -> Result<SchoolClass, AppError> {
    state.school_class_service.archive_school_class(input)
}

#[tauri::command]
pub async fn restore_school_class(
    state: State<'_, AppState>,
    input: SchoolClassIdInput,
) -> Result<SchoolClass, AppError> {
    state.school_class_service.restore_school_class(input)
}

#[tauri::command]
pub async fn import_student_scan_batch(
    state: State<'_, AppState>,
    input: ImportStudentScanBatchInput,
) -> Result<ImportStudentScanBatchOutput, AppError> {
    state.school_class_service.import_student_scan_batch(input)
}

#[tauri::command]
pub async fn create_student_scan_batch(
    state: State<'_, AppState>,
    input: CreateStudentScanBatchInput,
) -> Result<StudentScanBatch, AppError> {
    state.school_class_service.create_student_scan_batch(input)
}

#[tauri::command]
pub async fn list_student_scan_batches(
    state: State<'_, AppState>,
    input: ListStudentScanBatchesInput,
) -> Result<Vec<StudentScanBatch>, AppError> {
    state.school_class_service.list_student_scan_batches(input)
}

#[tauri::command]
pub async fn move_student_scan_batch(
    state: State<'_, AppState>,
    input: MoveStudentScanBatchInput,
) -> Result<StudentScanBatch, AppError> {
    state.school_class_service.move_student_scan_batch(input)
}

#[tauri::command]
pub async fn remove_student_scan_batch(
    state: State<'_, AppState>,
    input: RemoveStudentScanBatchInput,
) -> Result<StudentScanBatch, AppError> {
    state.school_class_service.remove_student_scan_batch(input)
}

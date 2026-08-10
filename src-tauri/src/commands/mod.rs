pub mod analysis_commands;
pub mod app_commands;
pub mod assessment_organization_commands;
pub mod backup_commands;
pub mod diagnostics_commands;
pub mod document_commands;
pub mod exam_package_commands;
pub mod generation_gc_commands;
pub mod graded_exam_review_commands;
pub mod job_commands;
pub mod mobile_connection_commands;
pub mod model_commands;
pub mod pdf_commands;
pub mod project_commands;
pub mod question_text_commands;
pub mod rubric_commands;
pub mod school_class_commands;
pub mod scoring_commands;
pub mod speaking_exam_commands;
pub mod student_answer_ocr_commands;
pub mod student_scan_commands;
pub mod workflow_commands;

use std::path::Path;

use crate::services::audit_service::AuditEntryInput;
use crate::AppState;

/// Appends an audit record for a critical teacher decision.
///
/// The audit write failure is propagated: critical decisions never report
/// fake success when the audit trail cannot be persisted.
pub(crate) fn audit_critical(
    state: &AppState,
    project_id: &str,
    input: AuditEntryInput,
) -> Result<(), crate::domain::errors::AppError> {
    let project = state
        .project_store
        .get_project_snapshot(project_id.to_string())?;
    state
        .audit_service
        .append(Path::new(&project.root_path), input.project(project_id))
        .map(|_| ())
}

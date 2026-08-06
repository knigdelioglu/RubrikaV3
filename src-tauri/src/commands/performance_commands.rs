use tauri::State;

use crate::domain::assessment::AssessmentActivity;
use crate::domain::errors::AppError;
use crate::domain::performance::{PerformanceAssessment, PerformanceRubric};
use crate::services::audit_service::AuditEntryInput;
use crate::services::performance_service::{
    ApprovePerformanceAssessmentInput, CreatePerformanceTaskInput, GetPerformanceReportInput,
    ListPerformanceAssessmentsInput, ListPerformanceTasksInput, PerformanceActivityIdInput,
    PerformanceReportDto, PerformanceStatusDto, PublishPerformanceRubricInput,
    SavePerformanceAssessmentInput, SetPerformanceAssessmentStatusInput,
    UpdatePerformanceTaskInput,
};
use crate::AppState;

#[tauri::command]
pub async fn create_performance_task(
    state: State<'_, AppState>,
    input: CreatePerformanceTaskInput,
) -> Result<AssessmentActivity, AppError> {
    let project_id = input.project_id.clone();
    let activity = state.performance_service.create_performance_task(input)?;
    super::audit_critical(
        &state,
        &project_id,
        AuditEntryInput::new("performance_task_created", "Performans görevi oluşturuldu.")
            .entity("assessment_activity", &activity.id),
    )?;
    Ok(activity)
}

#[tauri::command]
pub async fn update_performance_task(
    state: State<'_, AppState>,
    input: UpdatePerformanceTaskInput,
) -> Result<AssessmentActivity, AppError> {
    let project_id = input.project_id.clone();
    let activity_id = input.activity_id.clone();
    let activity = state.performance_service.update_performance_task(input)?;
    super::audit_critical(
        &state,
        &project_id,
        AuditEntryInput::new("performance_task_updated", "Performans görevi güncellendi.")
            .entity("assessment_activity", &activity_id),
    )?;
    Ok(activity)
}

#[tauri::command]
pub async fn list_performance_tasks(
    state: State<'_, AppState>,
    input: ListPerformanceTasksInput,
) -> Result<Vec<AssessmentActivity>, AppError> {
    state.performance_service.list_performance_tasks(input)
}

#[tauri::command]
pub async fn get_performance_task(
    state: State<'_, AppState>,
    input: PerformanceActivityIdInput,
) -> Result<AssessmentActivity, AppError> {
    state.performance_service.get_performance_task(input)
}

#[tauri::command]
pub async fn publish_performance_rubric(
    state: State<'_, AppState>,
    input: PublishPerformanceRubricInput,
) -> Result<PerformanceRubric, AppError> {
    let project_id = input.project_id.clone();
    let activity_id = input.activity_id.clone();
    let rubric = state
        .performance_service
        .publish_performance_rubric(input)?;
    super::audit_critical(
        &state,
        &project_id,
        AuditEntryInput::new(
            "performance_rubric_published",
            "Performans rubriği yayınlandı (yeni sürüm).",
        )
        .entity("assessment_activity", &activity_id),
    )?;
    Ok(rubric)
}

#[tauri::command]
pub async fn get_performance_rubric_history(
    state: State<'_, AppState>,
    input: PerformanceActivityIdInput,
) -> Result<Vec<PerformanceRubric>, AppError> {
    state
        .performance_service
        .get_performance_rubric_history(input)
}

#[tauri::command]
pub async fn save_performance_assessment(
    state: State<'_, AppState>,
    input: SavePerformanceAssessmentInput,
) -> Result<PerformanceAssessment, AppError> {
    state.performance_service.save_performance_assessment(input)
}

#[tauri::command]
pub async fn approve_performance_assessment(
    state: State<'_, AppState>,
    input: ApprovePerformanceAssessmentInput,
) -> Result<PerformanceAssessment, AppError> {
    let project_id = input.project_id.clone();
    let assessment_id = input.assessment_id.clone();
    let assessment = state
        .performance_service
        .approve_performance_assessment(input)?;
    super::audit_critical(
        &state,
        &project_id,
        AuditEntryInput::new(
            "performance_assessment_approved",
            "Performans değerlendirmesi onaylandı.",
        )
        .entity("performance_assessment", &assessment_id),
    )?;
    Ok(assessment)
}

#[tauri::command]
pub async fn set_performance_assessment_status(
    state: State<'_, AppState>,
    input: SetPerformanceAssessmentStatusInput,
) -> Result<PerformanceAssessment, AppError> {
    let project_id = input.project_id.clone();
    let status = input.status;
    let assessment = state
        .performance_service
        .set_performance_assessment_status(input)?;
    let summary = format!("Performans değerlendirme durumu güncellendi ({status:?}).");
    super::audit_critical(
        &state,
        &project_id,
        AuditEntryInput::new("performance_assessment_status_updated", summary.as_str())
            .entity("performance_assessment", &assessment.id),
    )?;
    Ok(assessment)
}

#[tauri::command]
pub async fn list_performance_assessments(
    state: State<'_, AppState>,
    input: ListPerformanceAssessmentsInput,
) -> Result<Vec<PerformanceAssessment>, AppError> {
    state
        .performance_service
        .list_performance_assessments(input)
}

#[tauri::command]
pub async fn get_performance_report(
    state: State<'_, AppState>,
    input: GetPerformanceReportInput,
) -> Result<PerformanceReportDto, AppError> {
    state.performance_service.get_performance_report(input)
}

#[tauri::command]
pub async fn get_performance_status(
    state: State<'_, AppState>,
    input: PerformanceActivityIdInput,
) -> Result<PerformanceStatusDto, AppError> {
    state.performance_service.get_performance_status(input)
}

#[cfg(test)]
mod tests {
    use super::{
        CreatePerformanceTaskInput, PublishPerformanceRubricInput, SavePerformanceAssessmentInput,
        SetPerformanceAssessmentStatusInput,
    };
    use crate::domain::performance::{CriterionRating, PerformanceAssessmentStatus};
    use crate::services::performance_service::PerformanceStatusDto;

    #[test]
    fn create_performance_task_uses_camel_case_contract() {
        let input: CreatePerformanceTaskInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "academicYearId": "2026-2027",
            "courseId": "tde",
            "courseName": "Türk Dili ve Edebiyatı",
            "gradeLevel": 9,
            "term": 1,
            "sequenceNumber": 1,
            "schoolClassIds": ["class-1"],
            "title": "1. Performans",
            "performanceDetails": {
                "theme": "Doğa ve insan",
                "learningOutcomes": ["Okuma çıktısı"],
                "skillArea": "writing",
                "taskInstruction": "Bir metin yazın.",
                "workMode": "individual",
                "evidenceTypes": ["Yazılı ürün"]
            },
            "initialRubric": {
                "id": "rubric-1",
                "name": "Yazılı Ürün",
                "version": 0,
                "criteria": [],
                "levels": [],
                "createdAt": "2026-01-01T00:00:00Z"
            }
        }))
        .expect("valid create input");
        assert_eq!(input.course_id, "tde");
        assert_eq!(
            input.performance_details.skill_area,
            crate::domain::performance::PerformanceSkillArea::Writing
        );
        assert_eq!(input.school_class_ids, vec!["class-1".to_string()]);
        assert_eq!(input.initial_rubric.unwrap().name, "Yazılı Ürün");
    }

    #[test]
    fn save_assessment_uses_camel_case_contract_with_ratings() {
        let input: SavePerformanceAssessmentInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "activityId": "activity-1",
            "applicationId": "app-1",
            "studentId": "student-1",
            "ratings": [
                { "criterionId": "c1", "levelId": "l1", "note": "güçlü" }
            ],
            "feedback": "Tebrikler"
        }))
        .expect("valid save input");
        assert_eq!(
            input.ratings,
            vec![CriterionRating {
                criterion_id: "c1".into(),
                level_id: "l1".into(),
                note: Some("güçlü".into()),
            }]
        );
        assert_eq!(input.feedback.as_deref(), Some("Tebrikler"));
    }

    #[test]
    fn set_status_uses_snake_case_status_contract() {
        let input: SetPerformanceAssessmentStatusInput =
            serde_json::from_value(serde_json::json!({
                "projectId": "project-1",
                "activityId": "activity-1",
                "applicationId": "app-1",
                "studentId": "student-1",
                "status": "not_performed"
            }))
            .expect("valid status input");
        assert_eq!(input.status, PerformanceAssessmentStatus::NotPerformed);
        assert_eq!(input.assessment_id, None);
    }

    #[test]
    fn publish_rubric_uses_camel_case_contract() {
        let input: PublishPerformanceRubricInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "activityId": "activity-1",
            "rubric": {
                "id": "rubric-1",
                "name": "Sözlü Performans",
                "version": 0,
                "criteria": [],
                "levels": [],
                "createdAt": "2026-01-01T00:00:00Z"
            }
        }))
        .expect("valid publish input");
        assert_eq!(input.rubric.name, "Sözlü Performans");
    }

    #[test]
    fn get_performance_status_uses_camel_case_contract() {
        let input: crate::services::performance_service::PerformanceActivityIdInput =
            serde_json::from_value(serde_json::json!({
                "projectId": "project-1",
                "activityId": "activity-1",
            }))
            .expect("valid status input");
        assert_eq!(input.project_id, "project-1");
        assert_eq!(input.activity_id, "activity-1");

        let dto = PerformanceStatusDto {
            has_published_rubric: true,
            published_rubric_version: Some(2),
            has_draft_rubric: false,
            has_task_details: true,
            total_students: 20,
            approved_count: 20,
            in_progress_count: 0,
            missing_count: 0,
            not_performed_count: 0,
            all_approved: true,
        };
        let value = serde_json::to_value(&dto).expect("status dto should serialize");
        assert_eq!(value["hasPublishedRubric"], true);
        assert_eq!(value["publishedRubricVersion"], 2);
        assert_eq!(value["totalStudents"], 20);
        assert_eq!(value["allApproved"], true);
        assert_eq!(value["approvedCount"], 20);
    }
}

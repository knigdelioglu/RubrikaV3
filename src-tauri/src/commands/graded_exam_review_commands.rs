use crate::domain::errors::AppError;
use crate::services::graded_exam_review_service::GradedExamReview;
use crate::AppState;
use tauri::State;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGradedExamReviewInput {
    pub project_id: String,
    pub submission_id: String,
}

#[tauri::command]
pub async fn get_graded_exam_review(
    state: State<'_, AppState>,
    input: GetGradedExamReviewInput,
) -> Result<GradedExamReview, AppError> {
    state
        .graded_exam_review_service
        .get_review(&input.project_id, &input.submission_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_input_uses_camel_case_contract() {
        let input: GetGradedExamReviewInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "submissionId": "submission-1"
        }))
        .expect("typed command input should deserialize");
        assert_eq!(input.project_id, "project-1");
        assert_eq!(input.submission_id, "submission-1");
    }
}

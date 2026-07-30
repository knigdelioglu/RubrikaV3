use crate::domain::analysis::{AssessmentAnalysis, AssessmentKind};
use crate::domain::errors::AppError;
use crate::services::analysis_service::FinishAssessmentOutput;
use crate::AppState;
use tauri::State;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishAssessmentInput {
    pub project_id: String,
    pub kind: AssessmentKind,
    pub source_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAssessmentAnalysisInput {
    pub project_id: String,
    pub analysis_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssessmentAnalysesInput {
    pub project_id: String,
}

#[tauri::command]
pub async fn finish_assessment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: FinishAssessmentInput,
) -> Result<FinishAssessmentOutput, AppError> {
    state
        .analysis_service
        .finish(app, input.project_id, input.kind, input.source_id)
        .await
}

#[tauri::command]
pub fn get_assessment_analysis(
    state: State<'_, AppState>,
    input: GetAssessmentAnalysisInput,
) -> Result<AssessmentAnalysis, AppError> {
    state
        .analysis_service
        .get(&input.project_id, &input.analysis_id)
}

#[tauri::command]
pub fn list_assessment_analyses(
    state: State<'_, AppState>,
    input: ListAssessmentAnalysesInput,
) -> Result<Vec<AssessmentAnalysis>, AppError> {
    state.analysis_service.list(&input.project_id)
}

#[cfg(test)]
mod tests {
    use super::FinishAssessmentInput;
    use crate::domain::analysis::AssessmentKind;

    #[test]
    fn finish_assessment_input_uses_camel_case_contract() {
        let input: FinishAssessmentInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "kind": "speaking",
            "sourceId": "exam-1"
        }))
        .expect("valid command input");

        assert_eq!(input.project_id, "project-1");
        assert_eq!(input.kind, AssessmentKind::Speaking);
        assert_eq!(input.source_id.as_deref(), Some("exam-1"));
    }
}

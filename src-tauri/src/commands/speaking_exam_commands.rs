use crate::domain::errors::AppError;
use crate::services::speaking_exam_service::{
    SpeakingAttemptSyncOutput, SpeakingCaptureRequest, SpeakingEngineRuntimeStatus,
    StartSpeakingExamOutput, ToggleSpeakingCaptureOutput,
};
use crate::AppState;
use tauri::State;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSpeakingExamInput {
    pub project_id: String,
    pub exam_name: String,
    pub exam_type: String,
    pub task_text: String,
    #[serde(default)]
    pub target_minutes: u32,
    #[serde(default)]
    pub minimum_minutes: u32,
    #[serde(default)]
    pub maximum_minutes: u32,
    #[serde(default)]
    pub target_seconds: Option<u32>,
    #[serde(default)]
    pub minimum_seconds: Option<u32>,
    #[serde(default)]
    pub maximum_seconds: Option<u32>,
    #[serde(default)]
    pub class_id: Option<String>,
    #[serde(default)]
    pub assigned_class_ids: Option<Vec<String>>,
    #[serde(default)]
    pub assessment_activity_id: Option<String>,
    #[serde(default)]
    pub exam_id: Option<String>,
    #[serde(default)]
    pub teacher_note: Option<String>,
    #[serde(default)]
    pub exam_date: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleSpeakingCaptureInput {
    pub project_id: String,
    pub exam_id: String,
    #[serde(default)]
    pub assessment_activity_id: Option<String>,
    #[serde(default)]
    pub class_application_id: Option<String>,
    pub student_id: String,
    pub action: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSpeakingAttemptInput {
    pub project_id: String,
    pub exam_id: String,
    pub attempt_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSpeakingExamInput {
    pub project_id: String,
    #[serde(default)]
    pub exam_id: String,
    #[serde(default)]
    pub assessment_activity_id: Option<String>,
    #[serde(default)]
    pub class_application_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectSpeakingExamClassInput {
    pub project_id: String,
    pub exam_id: String,
    #[serde(default)]
    pub assessment_activity_id: Option<String>,
    #[serde(default)]
    pub class_application_id: Option<String>,
    #[serde(default)]
    pub class_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectSpeakingExamStudentInput {
    pub project_id: String,
    pub exam_id: String,
    #[serde(default)]
    pub assessment_activity_id: Option<String>,
    #[serde(default)]
    pub class_application_id: Option<String>,
    pub student_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpeakingCriterionScoreInput {
    pub project_id: String,
    pub exam_id: String,
    pub attempt_id: String,
    pub criterion_id: String,
    pub score: f32,
    pub note: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpeakingCriterionLevelInput {
    pub project_id: String,
    pub exam_id: String,
    pub attempt_id: String,
    pub criterion_id: String,
    pub level: crate::domain::speaking::SpeakingPerformanceLevel,
    pub note: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveSpeakingAttemptInput {
    pub project_id: String,
    pub exam_id: String,
    pub attempt_id: String,
    pub teacher_note: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpeakingAttemptNoteInput {
    pub project_id: String,
    pub exam_id: String,
    pub attempt_id: String,
    pub teacher_note: Option<String>,
}

#[tauri::command]
pub fn list_speaking_exam_microphones(
    state: State<'_, AppState>,
) -> Result<Vec<speakoflow_types::MicrophoneDevice>, AppError> {
    state.speaking_exam_service.list_microphones()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectSpeakingExamMicrophoneInput {
    pub microphone_id: String,
}

#[tauri::command]
pub fn select_speaking_exam_microphone(
    state: State<'_, AppState>,
    input: SelectSpeakingExamMicrophoneInput,
) -> Result<(), AppError> {
    state
        .speaking_exam_service
        .select_microphone(&input.microphone_id)
}

#[tauri::command]
pub fn get_speaking_exam_runtime_status(state: State<'_, AppState>) -> SpeakingEngineRuntimeStatus {
    state.speaking_exam_service.runtime_status()
}

#[tauri::command]
pub async fn start_speaking_exam(
    state: State<'_, AppState>,
    input: StartSpeakingExamInput,
) -> Result<StartSpeakingExamOutput, AppError> {
    if input.exam_type != "prepared" && input.exam_type != "impromptu" {
        return Err(AppError {
            code: crate::domain::errors::AppErrorCode::SpeakingEngineLaunchFailed,
            message: "Konuşma türü geçersiz.".to_string(),
            recoverable: true,
            suggested_action: Some("Hazırlıklı veya hazırlıksız konuşma türünü seçin.".to_string()),
            technical_details: Some(format!("Unsupported exam_type: {}", input.exam_type)),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        });
    }

    let assigned_ids = if let Some(ids) = input.assigned_class_ids {
        ids
    } else if let Some(class_id) = input.class_id {
        vec![class_id]
    } else {
        vec![]
    };

    let target_sec = input
        .target_seconds
        .unwrap_or_else(|| input.target_minutes.saturating_mul(60));
    let min_sec = input
        .minimum_seconds
        .unwrap_or_else(|| input.minimum_minutes.saturating_mul(60));
    let max_sec = input
        .maximum_seconds
        .unwrap_or_else(|| input.maximum_minutes.saturating_mul(60));

    state
        .speaking_exam_service
        .start(
            &input.project_id,
            &input.exam_name,
            assigned_ids,
            input.assessment_activity_id,
            &input.exam_type,
            &input.task_text,
            target_sec,
            min_sec,
            max_sec,
            input.exam_id,
            input.teacher_note,
            input.exam_date,
        )
        .await
}

#[tauri::command]
pub async fn toggle_speaking_capture(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ToggleSpeakingCaptureInput,
) -> Result<ToggleSpeakingCaptureOutput, AppError> {
    state
        .speaking_exam_service
        .toggle_capture(
            app,
            SpeakingCaptureRequest {
                project_id: &input.project_id,
                exam_id: &input.exam_id,
                assessment_activity_id: input.assessment_activity_id.as_deref(),
                class_application_id: input.class_application_id.as_deref(),
                student_id: &input.student_id,
                action: &input.action,
            },
        )
        .await
}

#[tauri::command]
pub async fn start_speaking_exam_attempt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ToggleSpeakingCaptureInput,
) -> Result<ToggleSpeakingCaptureOutput, AppError> {
    state
        .speaking_exam_service
        .toggle_capture(
            app,
            SpeakingCaptureRequest {
                project_id: &input.project_id,
                exam_id: &input.exam_id,
                assessment_activity_id: input.assessment_activity_id.as_deref(),
                class_application_id: input.class_application_id.as_deref(),
                student_id: &input.student_id,
                action: "start",
            },
        )
        .await
}

#[tauri::command]
pub async fn stop_speaking_exam_attempt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ToggleSpeakingCaptureInput,
) -> Result<ToggleSpeakingCaptureOutput, AppError> {
    state
        .speaking_exam_service
        .toggle_capture(
            app,
            SpeakingCaptureRequest {
                project_id: &input.project_id,
                exam_id: &input.exam_id,
                assessment_activity_id: input.assessment_activity_id.as_deref(),
                class_application_id: input.class_application_id.as_deref(),
                student_id: &input.student_id,
                action: "stop",
            },
        )
        .await
}

#[tauri::command]
pub async fn pause_speaking_exam_attempt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ToggleSpeakingCaptureInput,
) -> Result<ToggleSpeakingCaptureOutput, AppError> {
    state
        .speaking_exam_service
        .toggle_capture(
            app,
            SpeakingCaptureRequest {
                project_id: &input.project_id,
                exam_id: &input.exam_id,
                assessment_activity_id: input.assessment_activity_id.as_deref(),
                class_application_id: input.class_application_id.as_deref(),
                student_id: &input.student_id,
                action: "pause",
            },
        )
        .await
}

#[tauri::command]
pub async fn resume_speaking_exam_attempt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ToggleSpeakingCaptureInput,
) -> Result<ToggleSpeakingCaptureOutput, AppError> {
    state
        .speaking_exam_service
        .toggle_capture(
            app,
            SpeakingCaptureRequest {
                project_id: &input.project_id,
                exam_id: &input.exam_id,
                assessment_activity_id: input.assessment_activity_id.as_deref(),
                class_application_id: input.class_application_id.as_deref(),
                student_id: &input.student_id,
                action: "resume",
            },
        )
        .await
}

#[tauri::command]
pub async fn cancel_speaking_exam_attempt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: ToggleSpeakingCaptureInput,
) -> Result<ToggleSpeakingCaptureOutput, AppError> {
    state
        .speaking_exam_service
        .toggle_capture(
            app,
            SpeakingCaptureRequest {
                project_id: &input.project_id,
                exam_id: &input.exam_id,
                assessment_activity_id: input.assessment_activity_id.as_deref(),
                class_application_id: input.class_application_id.as_deref(),
                student_id: &input.student_id,
                action: "cancel",
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{SelectSpeakingExamStudentInput, UpdateSpeakingCriterionLevelInput};
    use crate::domain::speaking::SpeakingPerformanceLevel;

    #[test]
    fn qualitative_level_command_uses_teacher_facing_contract() {
        let input: UpdateSpeakingCriterionLevelInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "examId": "exam-1",
            "attemptId": "attempt-1",
            "criterionId": "body-language",
            "level": "good"
        }))
        .expect("valid qualitative level input");

        assert_eq!(input.level, SpeakingPerformanceLevel::Good);
        assert_eq!(input.note, None);
    }

    #[test]
    fn active_student_selection_uses_camel_case_contract() {
        let input: SelectSpeakingExamStudentInput = serde_json::from_value(serde_json::json!({
            "projectId": "project-1",
            "examId": "exam-1",
            "studentId": "student-1"
        }))
        .expect("valid active student input");

        assert_eq!(input.student_id, "student-1");
    }
}

#[tauri::command]
pub async fn sync_speaking_attempt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: SyncSpeakingAttemptInput,
) -> Result<SpeakingAttemptSyncOutput, AppError> {
    state
        .speaking_exam_service
        .sync_attempt(app, &input.project_id, &input.exam_id, &input.attempt_id)
        .await
}

#[tauri::command]
pub async fn get_speaking_exam(
    state: State<'_, AppState>,
    input: GetSpeakingExamInput,
) -> Result<crate::domain::speaking::SpeakingExam, AppError> {
    state.speaking_exam_service.get_exam(
        &input.project_id,
        &input.exam_id,
        input.assessment_activity_id.as_deref(),
        input.class_application_id.as_deref(),
    )
}

#[tauri::command]
pub async fn select_speaking_exam_class(
    state: State<'_, AppState>,
    input: SelectSpeakingExamClassInput,
) -> Result<crate::domain::speaking::SpeakingExam, AppError> {
    state.speaking_exam_service.select_exam_class(
        &input.project_id,
        &input.exam_id,
        input.assessment_activity_id.as_deref(),
        input.class_application_id.as_deref(),
        input.class_id.as_deref(),
    )
}

#[tauri::command]
pub async fn select_speaking_exam_student(
    state: State<'_, AppState>,
    input: SelectSpeakingExamStudentInput,
) -> Result<crate::domain::speaking::SpeakingExam, AppError> {
    state.speaking_exam_service.select_exam_student(
        &input.project_id,
        &input.exam_id,
        input.assessment_activity_id.as_deref(),
        input.class_application_id.as_deref(),
        &input.student_id,
    )
}

#[tauri::command]
pub async fn update_speaking_criterion_score(
    state: State<'_, AppState>,
    input: UpdateSpeakingCriterionScoreInput,
) -> Result<crate::domain::speaking::SpeakingAttempt, AppError> {
    state.speaking_exam_service.update_criterion_score(
        &input.project_id,
        &input.exam_id,
        &input.attempt_id,
        &input.criterion_id,
        input.score,
        input.note,
    )
}

#[tauri::command]
pub async fn update_speaking_criterion_level(
    state: State<'_, AppState>,
    input: UpdateSpeakingCriterionLevelInput,
) -> Result<crate::domain::speaking::SpeakingAttempt, AppError> {
    state.speaking_exam_service.update_criterion_level(
        &input.project_id,
        &input.exam_id,
        &input.attempt_id,
        &input.criterion_id,
        input.level,
        input.note,
    )
}

#[tauri::command]
pub async fn approve_speaking_attempt(
    state: State<'_, AppState>,
    input: ApproveSpeakingAttemptInput,
) -> Result<crate::domain::speaking::SpeakingAttempt, AppError> {
    state.speaking_exam_service.approve_attempt(
        &input.project_id,
        &input.exam_id,
        &input.attempt_id,
        input.teacher_note,
    )
}

#[tauri::command]
pub async fn update_speaking_attempt_note(
    state: State<'_, AppState>,
    input: UpdateSpeakingAttemptNoteInput,
) -> Result<crate::domain::speaking::SpeakingAttempt, AppError> {
    state.speaking_exam_service.update_attempt_note(
        &input.project_id,
        &input.exam_id,
        &input.attempt_id,
        input.teacher_note,
    )
}

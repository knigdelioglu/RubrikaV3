use crate::domain::errors::AppError;
use crate::domain::project::Project;
use crate::services::project_store::ListProjectsOutput;
use crate::AppState;
use tauri::State;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub root_path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectInput {
    pub project_path: Option<String>,
    pub root_path: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectSnapshotInput {
    pub project_id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectOutput {
    pub project: Project,
    pub project_path: String,
    pub warnings: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectOutput {
    pub project: Project,
    pub project_path: String,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<ListProjectsOutput, AppError> {
    Ok(state.project_store.list_projects())
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<CreateProjectOutput, AppError> {
    let project = state
        .project_store
        .create_project(input.name, input.root_path)?;
    Ok(CreateProjectOutput {
        project_path: project.root_path.clone(),
        project,
        warnings: vec![],
    })
}

#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    input: OpenProjectInput,
) -> Result<OpenProjectOutput, AppError> {
    let project_path = input
        .project_path
        .filter(|value| !value.trim().is_empty())
        .or_else(|| input.root_path.filter(|value| !value.trim().is_empty()))
        .unwrap_or_default();
    let (project, warnings) = state
        .project_store
        .open_project_with_warnings(project_path.clone())?;
    Ok(OpenProjectOutput {
        project_path: project.root_path.clone(),
        project,
        warnings,
    })
}

#[tauri::command]
pub async fn get_project_snapshot(
    state: State<'_, AppState>,
    input: GetProjectSnapshotInput,
) -> Result<Project, AppError> {
    state.project_store.get_project_snapshot(input.project_id)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDefaultProjectPathInput {
    pub project_name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDefaultProjectPathOutput {
    pub path: String,
}

#[tauri::command]
pub async fn get_default_project_path(
    app: tauri::AppHandle,
    input: GetDefaultProjectPathInput,
) -> Result<GetDefaultProjectPathOutput, AppError> {
    let path = crate::platform::paths::generate_default_project_path(&app, &input.project_name)?;
    Ok(GetDefaultProjectPathOutput { path })
}

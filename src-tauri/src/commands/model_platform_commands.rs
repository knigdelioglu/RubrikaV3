use crate::domain::errors::AppError;
use crate::domain::model_platform::{
    CapabilityManifest, ModelDefinition, ModelLifecycleState, ModelPlatformConfig, ModelTaskKind,
    RuntimeDefinition, TaskModelBinding,
};
use crate::services::model_benchmark_service::{
    BenchmarkSubmission, ModelBenchmarkService,
};
use crate::services::model_capability_probe_service::ModelCapabilityProbeService;
use crate::services::model_platform_service::{
    BindTaskInput, ImportModelInput, ModelPlatformService, PromotionDecision,
};
use crate::services::model_router_service::{
    ModelRouterService, ResolvedModelRoute, RouteUsageMode,
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeModelInput {
    pub model_definition_id: String,
    pub runtime_definition_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetModelLifecycleInput {
    pub model_definition_id: String,
    pub lifecycle_state: ModelLifecycleState,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelIdInput {
    pub model_definition_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisableBindingInput {
    pub binding_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePreviewInput {
    pub task: ModelTaskKind,
    pub usage_mode: RouteUsageMode,
}

#[tauri::command]
pub fn get_model_platform_snapshot() -> Result<ModelPlatformConfig, AppError> {
    ModelPlatformService::new().snapshot()
}

#[tauri::command]
pub fn import_local_model(input: ImportModelInput) -> Result<ModelDefinition, AppError> {
    ModelPlatformService::new().import_model(input)
}

#[tauri::command]
pub fn upsert_model_runtime(input: RuntimeDefinition) -> Result<RuntimeDefinition, AppError> {
    let platform = ModelPlatformService::new();
    platform.upsert_runtime_definition(input.clone())?;
    Ok(input)
}

#[tauri::command]
pub async fn probe_local_model(input: ProbeModelInput) -> Result<CapabilityManifest, AppError> {
    let platform = ModelPlatformService::new();
    ModelCapabilityProbeService::new(platform)?
        .probe(&input.model_definition_id, &input.runtime_definition_id)
        .await
}

#[tauri::command]
pub fn bind_model_task(input: BindTaskInput) -> Result<TaskModelBinding, AppError> {
    ModelPlatformService::new().bind_task(input)
}

#[tauri::command]
pub fn disable_model_task_binding(input: DisableBindingInput) -> Result<(), AppError> {
    ModelPlatformService::new().disable_binding(&input.binding_id)
}

#[tauri::command]
pub fn set_model_lifecycle(input: SetModelLifecycleInput) -> Result<ModelDefinition, AppError> {
    ModelPlatformService::new()
        .set_model_lifecycle(&input.model_definition_id, input.lifecycle_state)
}

#[tauri::command]
pub fn submit_model_benchmark(
    input: BenchmarkSubmission,
) -> Result<crate::domain::model_platform::BenchmarkResultSummary, AppError> {
    let platform = ModelPlatformService::new();
    ModelBenchmarkService::new(platform).evaluate_and_record(input)
}

#[tauri::command]
pub fn get_model_promotion_decision(input: ModelIdInput) -> Result<PromotionDecision, AppError> {
    ModelPlatformService::new().production_promotion_decision(&input.model_definition_id)
}

#[tauri::command]
pub fn resolve_model_route_preview(input: RoutePreviewInput) -> Result<ResolvedModelRoute, AppError> {
    let platform = ModelPlatformService::new();
    ModelRouterService::new(platform).resolve(input.task, input.usage_mode)
}

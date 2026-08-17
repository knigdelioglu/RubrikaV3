use crate::domain::model_platform::{ModelDefinition, RuntimeDefinition};
use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone)]
pub struct PlatformLaunchDefinition {
    pub model: ModelDefinition,
    pub runtime: RuntimeDefinition,
}

fn registry() -> &'static RwLock<BTreeMap<String, PlatformLaunchDefinition>> {
    static REGISTRY: OnceLock<RwLock<BTreeMap<String, PlatformLaunchDefinition>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(BTreeMap::new()))
}

pub fn register(
    profile_id: impl Into<String>,
    model: ModelDefinition,
    runtime: RuntimeDefinition,
) {
    let mut guard = registry().write().unwrap_or_else(|error| error.into_inner());
    guard.insert(
        profile_id.into(),
        PlatformLaunchDefinition { model, runtime },
    );
}

pub fn get(profile_id: &str) -> Option<PlatformLaunchDefinition> {
    registry()
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .get(profile_id)
        .cloned()
}

pub fn remove(profile_id: &str) {
    registry()
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .remove(profile_id);
}

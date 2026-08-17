use crate::domain::errors::{AppError, AppErrorCode};
use crate::domain::model::{
    default_model_profile, speaking_asr_cleanup_model_profile, speaking_rubric_model_profile,
    ModelMode, ModelProfile, PrivacyMode,
};
use crate::platform::file_access::atomic_write;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelConfigStore {
    active_profile_id: String,
    profiles: Vec<ModelProfile>,
}

#[derive(Clone)]
pub struct ModelConfigService {
    store: Arc<Mutex<ModelConfigStore>>,
    config_path: PathBuf,
}

impl ModelConfigService {
    pub fn new() -> Self {
        let config_path = model_config_path();
        let store = load_or_default_store(&config_path);
        Self {
            store: Arc::new(Mutex::new(store)),
            config_path,
        }
    }

    pub fn new_with_path(config_path: PathBuf) -> Self {
        let store = load_or_default_store(&config_path);
        Self {
            store: Arc::new(Mutex::new(store)),
            config_path,
        }
    }

    pub fn get_profile(&self, profile_id: Option<&str>) -> Result<ModelProfile, AppError> {
        let store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {}", err)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let target_id = profile_id.unwrap_or(&store.active_profile_id);
        store
            .profiles
            .iter()
            .find(|profile| profile.id == target_id)
            .cloned()
            .ok_or_else(|| AppError {
                code: AppErrorCode::ModelProfileNotFound,
                message: "Model profili bulunamadı.".to_string(),
                recoverable: false,
                suggested_action: Some("Varsayılan modeli yeniden yükleyin.".to_string()),
                technical_details: Some(format!("profile_id={}", target_id)),
                correlation_id: Uuid::new_v4().to_string(),
            })
    }

    pub fn set_mode(
        &self,
        profile_id: Option<&str>,
        mode: ModelMode,
    ) -> Result<ModelProfile, AppError> {
        let mut store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {}", err)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let target_id = profile_id.unwrap_or(&store.active_profile_id).to_string();
        let profile = store
            .profiles
            .iter_mut()
            .find(|profile| profile.id == target_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::ModelProfileNotFound,
                message: "Model profili bulunamadı.".to_string(),
                recoverable: false,
                suggested_action: Some("Varsayılan profili açmayı deneyin.".to_string()),
                technical_details: Some(format!("profile_id={}", target_id)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;

        profile.mode = mode;
        let cloned = profile.clone();
        store.active_profile_id = cloned.id.clone();
        save_store(&self.config_path, &store)?;
        Ok(cloned)
    }

    pub fn reset_active_profile(&self) -> Result<ModelProfile, AppError> {
        let mut store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {}", err)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let default_profile = default_model_profile();
        store.active_profile_id = default_profile.id.clone();
        store.profiles = vec![
            default_profile.clone(),
            speaking_asr_cleanup_model_profile(),
            speaking_rubric_model_profile(),
        ];
        save_store(&self.config_path, &store)?;
        Ok(default_profile)
    }

    pub fn enable_external_profile(
        &self,
        profile_id: Option<&str>,
    ) -> Result<ModelProfile, AppError> {
        let mut store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {err}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        let target_id = profile_id.unwrap_or(&store.active_profile_id).to_string();
        let profile = store
            .profiles
            .iter_mut()
            .find(|profile| profile.id == target_id)
            .ok_or_else(|| AppError {
                code: AppErrorCode::ModelProfileNotFound,
                message: "Model profili bulunamadı.".to_string(),
                recoverable: false,
                suggested_action: Some("Geçerli bir model profili seçin.".to_string()),
                technical_details: Some(format!("profile_id={target_id}")),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        profile.privacy_mode = PrivacyMode::ExplicitExternal;
        profile.mode = ModelMode::External;
        let cloned = profile.clone();
        store.active_profile_id = cloned.id.clone();
        save_store(&self.config_path, &store)?;
        Ok(cloned)
    }

    pub fn active_profile_id(&self) -> String {
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        store.active_profile_id.clone()
    }

    pub fn get_model_profile(&self, profile_id: &str) -> Result<ModelProfile, AppError> {
        self.get_profile(Some(profile_id))
    }

    /// Adds or replaces a compatibility profile only in process memory.
    ///
    /// Model-platform routes use this seam so the legacy `model_profiles.json`
    /// remains a read/migration source and is never polluted with generated
    /// task/model/runtime profiles.
    pub fn update_ephemeral_profile(&self, profile: ModelProfile) -> Result<(), AppError> {
        let mut store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {}", err)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        if let Some(existing) = store.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            store.profiles.push(profile);
        }
        Ok(())
    }

    pub fn remove_ephemeral_profile(&self, profile_id: &str) -> Result<(), AppError> {
        let mut store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {}", err)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        store.profiles.retain(|profile| profile.id != profile_id);
        Ok(())
    }

    pub fn update_profile(&self, profile: ModelProfile) -> Result<(), AppError> {
        let mut store = self
            .store
            .lock()
            .map_err(|err| crate::domain::errors::AppError {
                code: crate::domain::errors::AppErrorCode::ModelStateAccessFailed,
                message: "Model konfigürasyonuna erişilemedi.".to_string(),
                recoverable: false,
                suggested_action: Some("Uygulamayı yeniden başlatmayı deneyin.".to_string()),
                technical_details: Some(format!("Mutex lock failed: {}", err)),
                correlation_id: Uuid::new_v4().to_string(),
            })?;
        if let Some(existing) = store.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile.clone();
        } else {
            store.profiles.push(profile.clone());
        }
        store.active_profile_id = profile.id.clone();
        save_store(&self.config_path, &store)
    }
}

impl Default for ModelConfigService {
    fn default() -> Self {
        Self::new()
    }
}

fn model_config_path() -> PathBuf {
    if let Some(path) = env::var_os("RUBRIKA_V3_MODEL_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    let base_dir = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base_dir
        .join("Library")
        .join("Application Support")
        .join("RubrikaV3")
        .join("model_profiles.json")
}

fn load_or_default_store(path: &PathBuf) -> ModelConfigStore {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(|mut store| {
                if bootstrap_local_model_profile(&mut store) {
                    let _ = save_store(path, &store);
                }
                store
            })
            .unwrap_or_else(|_| default_store()),
        Err(_) => default_store(),
    }
}

fn default_store() -> ModelConfigStore {
    let profile = default_model_profile();
    ModelConfigStore {
        active_profile_id: profile.id.clone(),
        profiles: vec![
            profile,
            speaking_asr_cleanup_model_profile(),
            speaking_rubric_model_profile(),
        ],
    }
}

fn bootstrap_local_model_profile(store: &mut ModelConfigStore) -> bool {
    let mut changed = false;
    if let Some(local_profile) = crate::domain::model::local_model_paths() {
        for profile in &mut store.profiles {
            if profile.id == local_profile.id {
                if profile.display_name != local_profile.display_name {
                    profile.display_name = local_profile.display_name.clone();
                    changed = true;
                }
                if profile.server_path != local_profile.server_path {
                    profile.server_path = local_profile.server_path.clone();
                    changed = true;
                }
                if profile.model_path != local_profile.model_path {
                    profile.model_path = local_profile.model_path.clone();
                    changed = true;
                }
                if profile.mmproj_path != local_profile.mmproj_path {
                    profile.mmproj_path = local_profile.mmproj_path.clone();
                    changed = true;
                }
                if profile.host != local_profile.host {
                    profile.host = local_profile.host.clone();
                    changed = true;
                }
                if profile.port != local_profile.port {
                    profile.port = local_profile.port;
                    changed = true;
                }
                if profile.base_url != local_profile.base_url {
                    profile.base_url = local_profile.base_url.clone();
                    changed = true;
                }
            }
        }
    }
    for speaking_profile in [
        speaking_asr_cleanup_model_profile(),
        speaking_rubric_model_profile(),
    ] {
        if !store
            .profiles
            .iter()
            .any(|profile| profile.id == speaking_profile.id)
        {
            store.profiles.push(speaking_profile);
            changed = true;
        }
    }

    changed
}

fn save_store(path: &PathBuf, store: &ModelConfigStore) -> Result<(), AppError> {
    let content = serde_json::to_string_pretty(store).map_err(|e| AppError {
        code: AppErrorCode::ModelServerStartFailed,
        message: "Model ayarları kaydedilemedi.".to_string(),
        recoverable: false,
        suggested_action: None,
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })?;

    atomic_write(path, &content).map_err(|e| AppError {
        code: AppErrorCode::ModelServerStartFailed,
        message: "Model ayar dosyası yazılamadı.".to_string(),
        recoverable: false,
        suggested_action: Some("Disk iznini ve boş alanı kontrol edin.".to_string()),
        technical_details: Some(e.to_string()),
        correlation_id: Uuid::new_v4().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn test_service() -> ModelConfigService {
        let path = env::temp_dir().join(format!("rubrika-model-config-{}.json", Uuid::new_v4()));
        ModelConfigService::new_with_path(path)
    }

    #[test]
    fn default_config_loads() {
        let service = test_service();
        let profile = service.get_profile(None).unwrap();
        assert_eq!(profile.id, "gemma4-ocr-q8");
    }

    #[test]
    fn default_config_includes_speaking_cleanup_and_rubric_profiles() {
        let service = test_service();
        assert_eq!(
            service
                .get_model_profile("speaking_transcript_cleanup_12b")
                .expect("cleanup profile should be available")
                .display_name,
            "Gemma 4 12B — Konuşma Transkript Temizleme"
        );
        assert_eq!(
            service
                .get_model_profile("speaking_rubric_evaluation_12b")
                .expect("rubric profile should be available")
                .display_name,
            "Gemma 4 12B — Konuşma Rubrik Değerlendirme"
        );
    }

    #[test]
    fn set_mode_updates_profile() {
        let service = test_service();
        let profile = service
            .set_mode(None, ModelMode::Managed)
            .expect("mode update should succeed");
        assert_eq!(profile.mode, ModelMode::Managed);
    }

    #[test]
    fn legacy_profile_without_privacy_mode_defaults_to_strict_local() {
        let path = env::temp_dir().join(format!("rubrika-model-legacy-{}.json", Uuid::new_v4()));
        let legacy = serde_json::json!({
            "activeProfileId": "legacy-external",
            "profiles": [{
                "id": "legacy-external",
                "displayName": "Legacy external",
                "mode": "external",
                "serverPath": "/tmp/llama-server",
                "modelPath": "/tmp/model.gguf",
                "mmprojPath": "",
                "host": "model.example.test",
                "port": 443,
                "baseUrl": "https://model.example.test"
            }]
        });
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let service = ModelConfigService::new_with_path(path.clone());
        let profile = service.get_profile(None).unwrap();
        assert_eq!(profile.privacy_mode, PrivacyMode::StrictLocal);
        assert_eq!(profile.mode, ModelMode::External);
        let _ = std::fs::remove_file(path);
    }
}

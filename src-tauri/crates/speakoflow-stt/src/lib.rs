use speakoflow_types::{EngineError, TranscriptSegment};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use transcribe_cpp::{Model as GgufModel, RunOptions, TimestampKind};
use transcribe_rs::whisper_cpp::{WhisperEngine, WhisperInferenceParams};

enum SttEngine {
    Gguf(GgufModel),
    LegacyWhisper(WhisperEngine),
}

pub struct WhisperStt {
    model_path: Option<PathBuf>,
    engine: Mutex<Option<SttEngine>>,
}

impl WhisperStt {
    pub fn from_env() -> Self {
        let model_path = whisper_model_candidates()
            .into_iter()
            .find(|path| path.is_file());
        Self {
            model_path,
            engine: Mutex::new(None),
        }
    }

    pub fn model_path(&self) -> Option<String> {
        self.model_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
    }

    pub fn is_configured(&self) -> bool {
        self.model_path.as_ref().is_some_and(|path| path.is_file())
    }

    pub fn is_loaded(&self) -> bool {
        self.engine
            .lock()
            .map(|engine| engine.is_some())
            .unwrap_or(false)
    }

    pub fn prepare(&self) -> Result<(), EngineError> {
        let model_path = self.model_path.as_ref().ok_or_else(|| {
            EngineError::Configuration(
                "Whisper modeli bulunamadı. Bir Whisper GGUF modeli seçin veya RUBRIKA_WHISPER_MODEL_PATH ayarlayın."
                    .to_string(),
            )
        })?;
        let mut guard = self
            .engine
            .lock()
            .map_err(|_| EngineError::Stt("Whisper motor kilidi alınamadı".to_string()))?;
        if guard.is_some() {
            return Ok(());
        }
        *guard = Some(load_engine(model_path)?);
        Ok(())
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
    ) -> Result<(String, Vec<TranscriptSegment>), EngineError> {
        self.prepare()?;
        let mut guard = self
            .engine
            .lock()
            .map_err(|_| EngineError::Stt("Whisper motor kilidi alınamadı".to_string()))?;
        match guard
            .as_mut()
            .ok_or_else(|| EngineError::Stt("Whisper motoru hazır değil".to_string()))?
        {
            SttEngine::Gguf(model) => transcribe_gguf(model, samples),
            SttEngine::LegacyWhisper(engine) => transcribe_legacy(engine, samples),
        }
    }

    pub fn unload(&self) -> Result<(), EngineError> {
        let mut guard = self
            .engine
            .lock()
            .map_err(|_| EngineError::Stt("Whisper motor kilidi alınamadı".to_string()))?;
        *guard = None;
        Ok(())
    }
}

fn load_engine(model_path: &Path) -> Result<SttEngine, EngineError> {
    if is_gguf_model(model_path) {
        initialize_gguf_backends()?;
        return GgufModel::load(model_path)
            .map(SttEngine::Gguf)
            .map_err(|error| {
                EngineError::Stt(format!(
                    "Whisper GGUF modeli yüklenemedi: {error}. Model dosyası transcribe.cpp ile uyumlu olmalı."
                ))
            });
    }
    WhisperEngine::load(model_path)
        .map(SttEngine::LegacyWhisper)
        .map_err(|error| EngineError::Stt(format!("Whisper modeli yüklenemedi: {error}")))
}

fn is_gguf_model(model_path: &Path) -> bool {
    model_path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
}

fn initialize_gguf_backends() -> Result<(), EngineError> {
    static BACKENDS: OnceLock<Result<(), String>> = OnceLock::new();
    BACKENDS
        .get_or_init(|| transcribe_cpp::init_backends_default().map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|error| EngineError::Stt(format!("GGUF ses motoru başlatılamadı: {error}")))
}

fn transcribe_gguf(
    model: &GgufModel,
    samples: &[f32],
) -> Result<(String, Vec<TranscriptSegment>), EngineError> {
    let mut session = model
        .session()
        .map_err(|error| EngineError::Stt(format!("Whisper GGUF oturumu açılamadı: {error}")))?;
    let result = session
        .run(
            samples,
            &RunOptions {
                language: Some("tr".to_string()),
                timestamps: TimestampKind::Segment,
                ..Default::default()
            },
        )
        .map_err(|error| EngineError::Stt(format!("Whisper transkripsiyonu başarısız: {error}")))?;
    let segments = result
        .segments
        .into_iter()
        .map(|segment| TranscriptSegment {
            start_ms: segment.t0_ms.max(0) as u64,
            end_ms: segment.t1_ms.max(0) as u64,
            text: segment.text.trim().to_string(),
            confidence: None,
        })
        .filter(|segment| !segment.text.is_empty())
        .collect();
    Ok((result.text.trim().to_string(), segments))
}

fn transcribe_legacy(
    engine: &mut WhisperEngine,
    samples: &[f32],
) -> Result<(String, Vec<TranscriptSegment>), EngineError> {
    let params = WhisperInferenceParams {
        language: Some("tr".to_string()),
        print_progress: false,
        print_realtime: false,
        print_timestamps: true,
        ..Default::default()
    };
    let result = engine
        .transcribe_with(samples, &params)
        .map_err(|error| EngineError::Stt(format!("Whisper transkripsiyonu başarısız: {error}")))?;
    let segments = result
        .segments
        .unwrap_or_default()
        .into_iter()
        .map(|segment| TranscriptSegment {
            start_ms: (segment.start.max(0.0) * 1_000.0) as u64,
            end_ms: (segment.end.max(0.0) * 1_000.0) as u64,
            text: segment.text.trim().to_string(),
            confidence: None,
        })
        .filter(|segment| !segment.text.is_empty())
        .collect();
    Ok((result.text.trim().to_string(), segments))
}

fn whisper_model_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("RUBRIKA_WHISPER_MODEL_PATH") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("RUBRIKA_V3_WHISPER_MODEL_PATH") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let model_dir = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.abhishekbarali.speakoflow")
            .join("models");
        for filename in [
            "whisper-medium-Q8_0.gguf",
            "whisper-medium.gguf",
            "ggml-medium.bin",
            "ggml-small.bin",
        ] {
            candidates.push(model_dir.join(filename));
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::is_gguf_model;
    use std::path::Path;

    #[test]
    fn gguf_models_use_the_transcribe_cpp_route() {
        assert!(is_gguf_model(Path::new("whisper-medium-Q8_0.gguf")));
        assert!(is_gguf_model(Path::new("WHISPER.GGUF")));
    }

    #[test]
    fn legacy_bin_models_do_not_use_the_gguf_route() {
        assert!(!is_gguf_model(Path::new("ggml-medium.bin")));
    }
}

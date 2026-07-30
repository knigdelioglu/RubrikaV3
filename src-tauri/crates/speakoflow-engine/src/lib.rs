use speakoflow_audio::{list_input_devices, AudioCapture, CapturedAudio};
use speakoflow_stt::WhisperStt;
use speakoflow_types::{EngineError, EngineResult, EngineState};
use std::sync::Mutex;
use std::time::Instant;
use uuid::Uuid;

pub struct SpeakoflowEngine {
    state: Mutex<EngineState>,
    session_id: Mutex<Option<String>>,
    microphone_id: Mutex<Option<String>>,
    started_at: Mutex<Option<Instant>>,
    capture: Mutex<AudioCapture>,
    stt: WhisperStt,
}

impl Default for SpeakoflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeakoflowEngine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(EngineState::Ready),
            session_id: Mutex::new(None),
            microphone_id: Mutex::new(None),
            started_at: Mutex::new(None),
            capture: Mutex::new(AudioCapture::default()),
            stt: WhisperStt::from_env(),
        }
    }

    pub fn state(&self) -> EngineState {
        self.state
            .lock()
            .map(|state| *state)
            .unwrap_or(EngineState::Failed)
    }

    pub fn stt_ready(&self) -> bool {
        self.stt.is_configured()
    }

    pub fn stt_loaded(&self) -> bool {
        self.stt.is_loaded()
    }

    pub fn whisper_model_path(&self) -> Option<String> {
        self.stt.model_path()
    }

    pub fn release_stt(&self) -> Result<(), EngineError> {
        self.stt.unload()
    }

    pub fn audio_level(&self) -> (f32, f32) {
        self.capture
            .lock()
            .map(|capture| capture.level())
            .unwrap_or((0.0, 0.0))
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.started_at
            .lock()
            .ok()
            .and_then(|started| started.map(|instant| instant.elapsed().as_millis() as u64))
            .unwrap_or(0)
    }

    pub fn select_microphone(&self, microphone_id: &str) -> Result<(), EngineError> {
        if !list_input_devices()?
            .iter()
            .any(|device| device.id == microphone_id)
        {
            return Err(EngineError::Configuration(
                "seçilen mikrofon bulunamadı".to_string(),
            ));
        }
        *self.microphone_id.lock().map_err(|_| {
            EngineError::InvalidTransition("microphone lock unavailable".to_string())
        })? = Some(microphone_id.to_string());
        Ok(())
    }

    pub fn start(&self, microphone_id: Option<&str>) -> Result<String, EngineError> {
        self.stt.prepare()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| EngineError::InvalidTransition("engine lock unavailable".to_string()))?;
        if *state == EngineState::Recording
            || *state == EngineState::Paused
            || *state == EngineState::Transcribing
        {
            return Err(EngineError::SessionBusy);
        }
        *state = EngineState::Starting;
        let session_id = Uuid::new_v4().to_string();
        let selected_device = microphone_id.map(str::to_string).or(self
            .microphone_id
            .lock()
            .map_err(|_| EngineError::InvalidTransition("microphone lock unavailable".to_string()))?
            .clone());
        self.capture
            .lock()
            .map_err(|_| EngineError::Audio("capture lock unavailable".to_string()))?
            .start(selected_device.as_deref())?;
        *self.session_id.lock().map_err(|_| {
            EngineError::InvalidTransition("session lock unavailable".to_string())
        })? = Some(session_id.clone());
        *self
            .started_at
            .lock()
            .map_err(|_| EngineError::InvalidTransition("timer lock unavailable".to_string()))? =
            Some(Instant::now());
        *state = EngineState::Recording;
        Ok(session_id)
    }

    pub fn stop_capture(&self, session_id: &str) -> Result<CapturedAudio, EngineError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| EngineError::InvalidTransition("engine lock unavailable".to_string()))?;
        let active = self
            .session_id
            .lock()
            .map_err(|_| EngineError::InvalidTransition("session lock unavailable".to_string()))?
            .clone();
        if active.as_deref() != Some(session_id) {
            return Err(EngineError::NoActiveSession);
        }
        if *state != EngineState::Recording && *state != EngineState::Paused {
            return Err(EngineError::InvalidTransition(format!(
                "stop is invalid in {:?}",
                *state
            )));
        }
        *state = EngineState::Stopping;
        let audio = self
            .capture
            .lock()
            .map_err(|_| EngineError::Audio("capture lock unavailable".to_string()))?
            .stop()?;
        *state = EngineState::Transcribing;
        Ok(audio)
    }

    pub fn pause(&self, session_id: &str) -> Result<(), EngineError> {
        self.transition_capture(
            session_id,
            EngineState::Recording,
            EngineState::Paused,
            |capture| capture.pause(),
        )
    }

    pub fn resume(&self, session_id: &str) -> Result<(), EngineError> {
        self.transition_capture(
            session_id,
            EngineState::Paused,
            EngineState::Recording,
            |capture| capture.resume(),
        )
    }

    fn transition_capture<F>(
        &self,
        session_id: &str,
        expected: EngineState,
        next: EngineState,
        operation: F,
    ) -> Result<(), EngineError>
    where
        F: FnOnce(&AudioCapture) -> Result<(), EngineError>,
    {
        let mut state = self
            .state
            .lock()
            .map_err(|_| EngineError::InvalidTransition("engine lock unavailable".to_string()))?;
        let active = self
            .session_id
            .lock()
            .map_err(|_| EngineError::InvalidTransition("session lock unavailable".to_string()))?
            .clone();
        if active.as_deref() != Some(session_id) {
            return Err(EngineError::NoActiveSession);
        }
        if *state != expected {
            return Err(EngineError::InvalidTransition(format!(
                "capture transition is invalid in {:?}",
                *state
            )));
        }
        let capture = self
            .capture
            .lock()
            .map_err(|_| EngineError::Audio("capture lock unavailable".to_string()))?;
        operation(&capture)?;
        *state = next;
        Ok(())
    }

    pub fn cancel(&self, session_id: &str) -> Result<(), EngineError> {
        let active = self
            .session_id
            .lock()
            .map_err(|_| EngineError::InvalidTransition("session lock unavailable".to_string()))?
            .clone();
        if active.as_deref() != Some(session_id) {
            return Err(EngineError::NoActiveSession);
        }
        self.capture
            .lock()
            .map_err(|_| EngineError::Audio("capture lock unavailable".to_string()))?
            .stop()
            .map(|_| ())?;
        if let Ok(mut state) = self.state.lock() {
            *state = EngineState::Cancelled;
        }
        if let Ok(mut session) = self.session_id.lock() {
            *session = None;
        }
        if let Ok(mut started_at) = self.started_at.lock() {
            *started_at = None;
        }
        Ok(())
    }

    pub fn transcribe(
        &self,
        session_id: &str,
        audio: CapturedAudio,
    ) -> Result<EngineResult, EngineError> {
        let active = self
            .session_id
            .lock()
            .map_err(|_| EngineError::InvalidTransition("session lock unavailable".to_string()))?
            .clone();
        if active.as_deref() != Some(session_id) {
            return Err(EngineError::NoActiveSession);
        }
        let (transcript, segments) = self.stt.transcribe(&audio.session_samples)?;
        let mut metrics = speakoflow_vad::analyze(&audio.session_samples, &transcript)?.metrics;
        if audio.dropped_chunks > 0 {
            metrics.warnings.push(format!(
                "{} ses parçası yoğunluk nedeniyle atlandı",
                audio.dropped_chunks
            ));
        }
        let result = EngineResult {
            session_id: session_id.to_string(),
            transcript,
            segments,
            metrics,
            sample_rate: audio.sample_rate,
            samples: audio.session_samples,
            peak: audio.peak,
            rms: audio.rms,
            diagnostics: vec![
                format!("whisper_configured={}", self.stt.is_configured()),
                format!("whisper_loaded={}", self.stt.is_loaded()),
            ],
        };
        *self
            .state
            .lock()
            .map_err(|_| EngineError::InvalidTransition("engine lock unavailable".to_string()))? =
            EngineState::Completed;
        *self.session_id.lock().map_err(|_| {
            EngineError::InvalidTransition("session lock unavailable".to_string())
        })? = None;
        *self
            .started_at
            .lock()
            .map_err(|_| EngineError::InvalidTransition("timer lock unavailable".to_string()))? =
            None;
        Ok(result)
    }

    pub fn fail(&self) {
        if let Ok(mut capture) = self.capture.lock() {
            let _ = capture.stop();
        }
        if let Ok(mut state) = self.state.lock() {
            *state = EngineState::Failed;
        }
        if let Ok(mut session) = self.session_id.lock() {
            *session = None;
        }
        if let Ok(mut started_at) = self.started_at.lock() {
            *started_at = None;
        }
    }
}

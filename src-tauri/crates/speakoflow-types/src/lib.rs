use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    Uninitialized,
    Preparing,
    Ready,
    Starting,
    Recording,
    Paused,
    Stopping,
    Transcribing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakingMetrics {
    pub recording_duration_ms: u64,
    pub speech_duration_ms: u64,
    pub silence_duration_ms: u64,
    pub speech_ratio: f32,
    pub word_count: u32,
    pub words_per_minute: f32,
    pub long_silence_count: u32,
    pub longest_silence_ms: u64,
    pub average_segment_ms: u64,
    pub filler_count: u32,
    pub repetition_count: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineResult {
    pub session_id: String,
    pub transcript: String,
    pub segments: Vec<TranscriptSegment>,
    pub metrics: SpeakingMetrics,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
    pub peak: f32,
    pub rms: f32,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("another speaking session is active")]
    SessionBusy,
    #[error("no active speaking session")]
    NoActiveSession,
    #[error("invalid engine state transition: {0}")]
    InvalidTransition(String),
    #[error("audio error: {0}")]
    Audio(String),
    #[error("speech-to-text error: {0}")]
    Stt(String),
    #[error("engine configuration error: {0}")]
    Configuration(String),
    #[error("engine cancelled")]
    Cancelled,
}

use speakoflow_types::{EngineError, SpeakingMetrics};

const FRAME_MS: u64 = 30;
const LONG_SILENCE_MS: u64 = 700;

#[derive(Debug, Clone)]
pub struct VadAnalysis {
    pub speech_ranges: Vec<(u64, u64)>,
    pub silence_ranges: Vec<(u64, u64)>,
    pub metrics: SpeakingMetrics,
}

pub fn analyze(samples: &[f32], transcript: &str) -> Result<VadAnalysis, EngineError> {
    if samples.is_empty() {
        return Ok(VadAnalysis {
            speech_ranges: vec![],
            silence_ranges: vec![],
            metrics: SpeakingMetrics::default(),
        });
    }
    let frame_samples = (16_000 * FRAME_MS / 1_000) as usize;
    let total_ms = (samples.len() as u64 * 1_000) / 16_000;
    let mut speech_ranges = Vec::new();
    let mut silence_ranges = Vec::new();
    let mut current_speech: Option<u64> = None;
    let mut current_silence: Option<u64> = None;
    for (index, frame) in samples.chunks(frame_samples).enumerate() {
        let rms =
            (frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len() as f32).sqrt();
        let is_speech = rms >= 0.012;
        let start = index as u64 * FRAME_MS;
        let end = (start + FRAME_MS).min(total_ms);
        if is_speech {
            if let Some(silence) = current_silence.take() {
                silence_ranges.push((silence, start));
            }
            current_speech.get_or_insert(start);
        } else {
            if let Some(speech) = current_speech.take() {
                speech_ranges.push((speech, start));
            }
            current_silence.get_or_insert(start);
        }
        let _ = end;
    }
    if let Some(speech) = current_speech {
        speech_ranges.push((speech, total_ms));
    }
    if let Some(silence) = current_silence {
        silence_ranges.push((silence, total_ms));
    }
    let speech_duration_ms = speech_ranges
        .iter()
        .map(|(start, end)| end.saturating_sub(*start))
        .sum::<u64>();
    let silence_duration_ms = total_ms.saturating_sub(speech_duration_ms);
    let longest_silence_ms = silence_ranges
        .iter()
        .map(|(start, end)| end.saturating_sub(*start))
        .max()
        .unwrap_or(0);
    let long_silence_count = silence_ranges
        .iter()
        .filter(|(start, end)| end.saturating_sub(*start) >= LONG_SILENCE_MS)
        .count() as u32;
    let words: Vec<&str> = transcript.split_whitespace().collect();
    let word_count = words.len() as u32;
    let words_per_minute = if speech_duration_ms == 0 {
        0.0
    } else {
        word_count as f32 * 60_000.0 / speech_duration_ms as f32
    };
    let fillers = ["ıı", "eee", "şey", "yani", "hani", "işte", "falan", "filan"];
    let filler_count = words
        .iter()
        .filter(|word| {
            fillers.iter().any(|filler| {
                word.trim_matches(|c: char| !c.is_alphabetic())
                    .eq_ignore_ascii_case(filler)
            })
        })
        .count() as u32;
    let repetition_count = words
        .windows(2)
        .filter(|pair| pair[0].eq_ignore_ascii_case(pair[1]))
        .count() as u32;
    let average_segment_ms = if speech_ranges.is_empty() {
        0
    } else {
        speech_duration_ms / speech_ranges.len() as u64
    };
    Ok(VadAnalysis {
        speech_ranges,
        silence_ranges,
        metrics: SpeakingMetrics {
            recording_duration_ms: total_ms,
            speech_duration_ms,
            silence_duration_ms,
            speech_ratio: speech_duration_ms as f32 / total_ms.max(1) as f32,
            word_count,
            words_per_minute,
            long_silence_count,
            longest_silence_ms,
            average_segment_ms,
            filler_count,
            repetition_count,
            warnings: vec![],
        },
    })
}

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, SupportedStreamConfig};
use speakoflow_types::{EngineError, MicrophoneDevice};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::time::Duration;

const OUTPUT_RATE: u32 = 16_000;
const QUEUE_CAPACITY: usize = 128;

#[derive(Default)]
struct AudioLevel {
    peak_bits: AtomicU32,
    rms_bits: AtomicU32,
}

impl AudioLevel {
    fn update(&self, samples: &[f32]) {
        let peak = samples
            .iter()
            .fold(0.0_f32, |value, sample| value.max(sample.abs()));
        let rms = if samples.is_empty() {
            0.0
        } else {
            (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32)
                .sqrt()
        };
        self.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
        self.rms_bits.store(rms.to_bits(), Ordering::Relaxed);
    }

    fn snapshot(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak_bits.load(Ordering::Relaxed)),
            f32::from_bits(self.rms_bits.load(Ordering::Relaxed)),
        )
    }
}

#[derive(Debug, Clone)]
pub struct CapturedAudio {
    pub session_samples: Vec<f32>,
    pub sample_rate: u32,
    pub peak: f32,
    pub rms: f32,
    pub dropped_chunks: u64,
}

pub fn list_input_devices() -> Result<Vec<MicrophoneDevice>, EngineError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let devices = host
        .input_devices()
        .map_err(|error| EngineError::Audio(format!("input device enumeration failed: {error}")))?;
    Ok(devices
        .enumerate()
        .map(|(index, device)| {
            let name = device
                .name()
                .unwrap_or_else(|_| "Bilinmeyen mikrofon".to_string());
            MicrophoneDevice {
                id: index.to_string(),
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
            }
        })
        .collect())
}

enum CaptureControl {
    Pause(SyncSender<Result<(), EngineError>>),
    Resume(SyncSender<Result<(), EngineError>>),
    Stop(SyncSender<Result<CapturedAudio, EngineError>>),
}

pub struct AudioCapture {
    control: Option<SyncSender<CaptureControl>>,
    worker: Option<std::thread::JoinHandle<()>>,
    level: Arc<AudioLevel>,
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self {
            control: None,
            worker: None,
            level: Arc::new(AudioLevel::default()),
        }
    }
}

impl AudioCapture {
    pub fn start(&mut self, selected_device_id: Option<&str>) -> Result<(), EngineError> {
        if self.control.is_some() {
            return Err(EngineError::SessionBusy);
        }
        let (control, control_rx) = sync_channel(1);
        let (ready_tx, ready_rx) = sync_channel(1);
        let selected_device_id = selected_device_id.map(str::to_string);
        let level = Arc::new(AudioLevel::default());
        let worker_level = level.clone();
        let worker = std::thread::spawn(move || {
            capture_worker(
                selected_device_id.as_deref(),
                control_rx,
                ready_tx,
                worker_level,
            );
        });
        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.control = Some(control);
                self.worker = Some(worker);
                self.level = level;
                Ok(())
            }
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(EngineError::Audio(
                    "mikrofon worker başlatılamadı".to_string(),
                ))
            }
        }
    }

    pub fn stop(&mut self) -> Result<CapturedAudio, EngineError> {
        let control = self.control.take().ok_or(EngineError::NoActiveSession)?;
        let worker = self.worker.take().ok_or(EngineError::NoActiveSession)?;
        let (result_tx, result_rx) = sync_channel(1);
        control
            .send(CaptureControl::Stop(result_tx))
            .map_err(|_| EngineError::Audio("mikrofon worker kapalı".to_string()))?;
        let result = result_rx
            .recv()
            .map_err(|_| EngineError::Audio("mikrofon sonucu alınamadı".to_string()))?;
        worker.join().map_err(|_| {
            EngineError::Audio("ses toplama iş parçacığı sonlandırılamadı".to_string())
        })?;
        result
    }

    pub fn pause(&self) -> Result<(), EngineError> {
        self.send_control(CaptureControl::Pause)
    }

    pub fn resume(&self) -> Result<(), EngineError> {
        self.send_control(CaptureControl::Resume)
    }

    pub fn level(&self) -> (f32, f32) {
        self.level.snapshot()
    }

    fn send_control<F>(&self, build: F) -> Result<(), EngineError>
    where
        F: FnOnce(SyncSender<Result<(), EngineError>>) -> CaptureControl,
    {
        let control = self.control.as_ref().ok_or(EngineError::NoActiveSession)?;
        let (reply_tx, reply_rx) = sync_channel(1);
        control
            .send(build(reply_tx))
            .map_err(|_| EngineError::Audio("mikrofon worker kapalı".to_string()))?;
        reply_rx
            .recv()
            .map_err(|_| EngineError::Audio("mikrofon worker yanıt vermedi".to_string()))?
    }
}

fn capture_worker(
    selected_device_id: Option<&str>,
    control_rx: Receiver<CaptureControl>,
    ready_tx: SyncSender<Result<(), EngineError>>,
    level: Arc<AudioLevel>,
) {
    let host = cpal::default_host();
    let devices: Vec<_> = match host.input_devices() {
        Ok(devices) => devices.collect(),
        Err(error) => {
            let _ = ready_tx.send(Err(EngineError::Audio(format!(
                "input device enumeration failed: {error}"
            ))));
            return;
        }
    };
    let device = match selected_device_id {
        Some(id) => match id
            .parse::<usize>()
            .ok()
            .and_then(|index| devices.get(index).cloned())
        {
            Some(device) => device,
            None => {
                let _ = ready_tx.send(Err(EngineError::Configuration(
                    "seçilen mikrofon bulunamadı".to_string(),
                )));
                return;
            }
        },
        None => match host.default_input_device() {
            Some(device) => device,
            None => {
                let _ = ready_tx.send(Err(EngineError::Audio("mikrofon bulunamadı".to_string())));
                return;
            }
        },
    };
    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            let _ = ready_tx.send(Err(EngineError::Audio(format!(
                "mikrofon yapılandırması okunamadı: {error}"
            ))));
            return;
        }
    };
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let (sender, receiver) = sync_channel::<Vec<f32>>(QUEUE_CAPACITY);
    let dropped = Arc::new(AtomicU64::new(0));
    let stream =
        match build_stream_for_config(&device, &config, sender, dropped.clone(), level, |error| {
            log::error!("Speakoflow mikrofon akışı hatası: {error}")
        }) {
            Ok(stream) => stream,
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };
    if let Err(error) = stream.play() {
        let _ = ready_tx.send(Err(EngineError::Audio(format!(
            "mikrofon akışı başlatılamadı: {error}"
        ))));
        return;
    }
    let _ = ready_tx.send(Ok(()));
    let mut raw = Vec::new();
    let mut paused = false;
    loop {
        loop {
            match receiver.try_recv() {
                Ok(chunk) if !paused => raw.extend(chunk),
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        match control_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(CaptureControl::Pause(reply)) => {
                let result = stream.pause().map_err(|error| {
                    EngineError::Audio(format!("mikrofon duraklatılamadı: {error}"))
                });
                if result.is_ok() {
                    paused = true;
                }
                let _ = reply.send(result);
            }
            Ok(CaptureControl::Resume(reply)) => {
                let result = stream.play().map_err(|error| {
                    EngineError::Audio(format!("mikrofon sürdürülemedi: {error}"))
                });
                if result.is_ok() {
                    paused = false;
                }
                let _ = reply.send(result);
            }
            Ok(CaptureControl::Stop(reply)) => {
                while let Ok(chunk) = receiver.try_recv() {
                    if !paused {
                        raw.extend(chunk);
                    }
                }
                let _ = reply.send(Ok(make_captured_audio(
                    &raw,
                    sample_rate,
                    channels,
                    dropped,
                )));
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
}

fn make_captured_audio(
    raw_samples: &[f32],
    input_rate: u32,
    channels: usize,
    dropped: Arc<AtomicU64>,
) -> CapturedAudio {
    let session_samples = resample_to_16k(raw_samples, input_rate);
    let peak = session_samples
        .iter()
        .fold(0.0_f32, |value, sample| value.max(sample.abs()));
    let rms = if session_samples.is_empty() {
        0.0
    } else {
        (session_samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / session_samples.len() as f32)
            .sqrt()
    };
    let _ = channels;
    CapturedAudio {
        session_samples,
        sample_rate: OUTPUT_RATE,
        peak,
        rms,
        dropped_chunks: dropped.load(Ordering::Relaxed),
    }
}

fn resample_to_16k(samples: &[f32], input_rate: u32) -> Vec<f32> {
    if input_rate == OUTPUT_RATE {
        return samples.to_vec();
    }
    if samples.is_empty() {
        return Vec::new();
    }
    let output_len = ((samples.len() as u64 * OUTPUT_RATE as u64) / input_rate as u64) as usize;
    (0..output_len)
        .map(|index| {
            let source = index as f64 * input_rate as f64 / OUTPUT_RATE as f64;
            let left = source.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = (source - left as f64) as f32;
            samples[left] * (1.0 - fraction) + samples[right] * fraction
        })
        .collect()
}

fn build_stream_for_config<E>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    sender: SyncSender<Vec<f32>>,
    dropped: Arc<AtomicU64>,
    level: Arc<AudioLevel>,
    error_callback: E,
) -> Result<Stream, EngineError>
where
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    match config.sample_format() {
        SampleFormat::F32 => {
            build_stream::<f32, E>(device, config, sender, dropped, level, error_callback)
        }
        SampleFormat::I16 => {
            build_stream::<i16, E>(device, config, sender, dropped, level, error_callback)
        }
        SampleFormat::U16 => {
            build_stream::<u16, E>(device, config, sender, dropped, level, error_callback)
        }
        SampleFormat::I8 => {
            build_stream::<i8, E>(device, config, sender, dropped, level, error_callback)
        }
        SampleFormat::U8 => {
            build_stream::<u8, E>(device, config, sender, dropped, level, error_callback)
        }
        SampleFormat::I32 => {
            build_stream::<i32, E>(device, config, sender, dropped, level, error_callback)
        }
        format => Err(EngineError::Audio(format!(
            "desteklenmeyen mikrofon örnek biçimi: {format:?}"
        ))),
    }
}

fn build_stream<T, E>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    sender: SyncSender<Vec<f32>>,
    dropped: Arc<AtomicU64>,
    level: Arc<AudioLevel>,
    error_callback: E,
) -> Result<Stream, EngineError>
where
    T: Sample + SizedSample + Send + 'static,
    f32: FromSample<T>,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let channels = config.channels() as usize;
    let callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let mono: Vec<f32> = if channels == 1 {
            data.iter()
                .map(|sample| sample.to_sample::<f32>())
                .collect()
        } else {
            data.chunks_exact(channels)
                .map(|frame| {
                    frame
                        .iter()
                        .map(|sample| sample.to_sample::<f32>())
                        .sum::<f32>()
                        / channels as f32
                })
                .collect()
        };
        level.update(&mono);
        match sender.try_send(mono) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    };
    device
        .build_input_stream(&config.clone().into(), callback, error_callback, None)
        .map_err(|error| EngineError::Audio(format!("mikrofon akışı oluşturulamadı: {error}")))
}

pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), EngineError> {
    let parent = path
        .parent()
        .ok_or_else(|| EngineError::Audio("WAV klasörü bulunamadı".to_string()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| EngineError::Audio(format!("WAV klasörü oluşturulamadı: {error}")))?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: OUTPUT_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|error| EngineError::Audio(format!("WAV oluşturulamadı: {error}")))?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(value)
            .map_err(|error| EngineError::Audio(format!("WAV yazılamadı: {error}")))?;
    }
    writer
        .finalize()
        .map_err(|error| EngineError::Audio(format!("WAV kapatılamadı: {error}")))
}

#[cfg(test)]
mod tests {
    use super::resample_to_16k;

    #[test]
    fn resampler_preserves_one_second_length() {
        assert_eq!(resample_to_16k(&vec![0.0; 48_000], 48_000).len(), 16_000);
    }
}

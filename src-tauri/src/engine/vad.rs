//! Voice Activity Detection (VAD) module using sherpa-onnx Silero VAD.
//!
//! Uses the official silero-vad ONNX model for precise speech segment detection.
//! The model file is expected at `{model_dir}/silero-vad/silero_vad.onnx`.

use crate::errors::{AppError, AppResult};
use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};

/// A detected speech segment with start (seconds), end (seconds).
#[derive(Debug, Clone)]
pub struct VadSegment {
    pub start: f64,
    pub end: f64,
}

/// Create a Silero VAD instance with the given model path.
pub fn create_silero_vad(model_path: &str) -> AppResult<VoiceActivityDetector> {
    let config = VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(model_path.to_string()),
            threshold: 0.5,
            min_silence_duration: 0.5,
            min_speech_duration: 0.25,
            window_size: 512,
            max_speech_duration: 30.0,
        },
        sample_rate: 16000,
        num_threads: 1,
        provider: None,
        debug: false,
        ..Default::default()
    };

    VoiceActivityDetector::create(&config, 120.0)
        .ok_or_else(|| AppError::Transcription("Failed to create Silero VAD detector".into()))
}

/// Detect speech segments using the sherpa-onnx Silero VAD.
///
/// Reads the WAV file, feeds samples to the VAD detector, and returns a list
/// of detected speech segments with their audio samples for per-segment ASR.
pub fn detect_speech_segments(
    wav_path: &str,
    model_path: &str,
) -> AppResult<(Vec<VadSegment>, f64)> {
    let audio = sherpa_onnx::Wave::read(wav_path)
        .ok_or_else(|| AppError::Transcription(format!("Failed to read WAV: {}", wav_path)))?;

    let sample_rate = audio.sample_rate();
    let total_duration = audio.samples().len() as f64 / sample_rate as f64;

    let vad = create_silero_vad(model_path)?;

    vad.accept_waveform(audio.samples());
    vad.flush();

    let mut segments: Vec<VadSegment> = Vec::new();

    while !vad.is_empty() {
        if let Some(seg) = vad.front() {
            let start_secs = seg.start() as f64 / sample_rate as f64;
            let end_secs = (seg.start() + seg.n()) as f64 / sample_rate as f64;

            segments.push(VadSegment {
                start: start_secs,
                end: end_secs,
            });

            vad.pop();
        } else {
            break;
        }
    }

    Ok((segments, total_duration))
}
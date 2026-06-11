//! Streaming transcription pipeline.
//!
//! Implements the core processing loop: packet-by-packet symphonia decode →
//! resample to 16 kHz → feed Silero VAD windows → recognize each speech segment
//! with the ASR model → push results into shared segments vec.
//!
//! Designed to match the sherpa-onnx non-streaming-speech-recognition-from-file
//! Tauri example architecture.

use crate::engine::audio_decoder;
use crate::errors::AppResult;
use serde::{Deserialize, Serialize};
use sherpa_onnx::{OfflineRecognizer, VoiceActivityDetector};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// A single recognition result segment with timestamp and text.
#[derive(Debug, Clone, Serialize)]
pub struct SegmentResult {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// VAD configuration that can be adjusted at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VadSettings {
    pub threshold: f32,
    pub min_silence_duration: f32,
    pub min_speech_duration: f32,
    pub max_speech_duration: f32,
    pub num_threads: i32,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            threshold: 0.2,
            min_silence_duration: 0.2,
            min_speech_duration: 0.2,
            max_speech_duration: 10.0,
            num_threads: 2,
        }
    }
}

/// Recognize a single VAD speech segment: skip if too short or punctuation-only.
fn recognize_segment(
    recognizer: &OfflineRecognizer,
    segment: &sherpa_onnx::SpeechSegment,
    segments: &Arc<Mutex<Vec<SegmentResult>>>,
) {
    let samples = segment.samples();
    let duration = samples.len() as f32 / 16000.0;
    if duration < 0.1 {
        return;
    }

    let start_time = segment.start() as f32 / 16000.0;
    let end_time = start_time + duration;

    let stream = recognizer.create_stream();
    stream.accept_waveform(16000, samples);
    recognizer.decode(&stream);

    if let Some(r) = stream.get_result() {
        let text = r.text.trim().to_string();
        if !text.is_empty()
            && !text
                .chars()
                .all(|c| c.is_ascii_punctuation() || c.is_ascii_whitespace())
        {
            if let Ok(mut segs) = segments.lock() {
                segs.push(SegmentResult {
                    start: start_time,
                    end: end_time,
                    text,
                });
            }
        }
    }
}

/// Stream audio through VAD + ASR without buffering the entire file.
///
/// Returns the audio duration in seconds on success.
///
/// # Arguments
/// * `path` - Path to the audio/video file.
/// * `recognizer` - Shared ASR recognizer (initialized by background thread).
/// * `vad` - Shared VAD detector (initialized by background thread).
/// * `cancelled` - Set to `true` to abort recognition.
/// * `progress` - Updated with percentage (0-99) during decoding.
/// * `segments` - Results are pushed here as VAD detects + ASR transcribes each segment.
pub fn run_recognition(
    path: &str,
    recognizer: &Arc<Mutex<Option<OfflineRecognizer>>>,
    vad: &Arc<Mutex<Option<VoiceActivityDetector>>>,
    cancelled: &AtomicBool,
    progress: &Arc<AtomicU32>,
    segments: &Arc<Mutex<Vec<SegmentResult>>>,
) -> AppResult<f32> {
    let (mut format_reader, mut decoder, track_id, num_channels_hint, native_rate_hint) =
        audio_decoder::open_audio_file(path)?;

    log::info!(
        "[run_recognition] file: {path}, native_rate_hint={native_rate_hint}, channels_hint={num_channels_hint}"
    );

    // The native_rate and num_channels from codec_params may be incorrect for
    // video containers (often default to 16000/1 when metadata is missing).
    // We'll determine the actual values from the first decoded frame and create
    // the resampler lazily.
    let mut resampler: Option<sherpa_onnx::LinearResampler> = None;
    let mut actual_native_rate: u32 = native_rate_hint;
    let mut actual_num_channels: usize = num_channels_hint;
    let mut first_frame_decoded = false;

    // Get approximate total samples for progress reporting.
    // If not available, progress will stay at 0% until completion.
    let total_samples: Option<usize> = format_reader
        .default_track()
        .and_then(|t| t.codec_params.n_frames)
        .map(|n| n as usize);

    let mut recognizer_guard = recognizer.lock().map_err(|e| crate::errors::AppError::Transcription(e.to_string()))?;
    let recognizer = recognizer_guard
        .as_mut()
        .ok_or(crate::errors::AppError::Transcription("Recognizer not initialized".into()))?;
    let mut vad_guard = vad.lock().map_err(|e| crate::errors::AppError::Transcription(e.to_string()))?;
    let vad = vad_guard.as_mut().ok_or(crate::errors::AppError::Transcription("VAD not initialized".into()))?;
    vad.reset();

    let window_size: usize = 512;
    let mut vad_buf: Vec<f32> = Vec::new();
    let mut decoded_count: usize = 0;
    let mut last_progress: u32 = 0;

    loop {
        if cancelled.load(Ordering::Relaxed) {
            vad.clear();
            break;
        }

        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(err) => {
                return Err(crate::errors::AppError::Transcription(format!(
                    "Read error: {err}"
                )))
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::IoError(_))
            | Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(err) => {
                return Err(crate::errors::AppError::Transcription(format!(
                    "Decode error: {err}"
                )))
            }
        };

        // On the first decoded frame, determine the actual sample rate and
        // channel count from the AudioBufferRef. Video containers (MP4/MKV)
        // often don't report sample_rate/channels in codec_params during probe,
        // causing the hint to be the default 16000/1 — which is wrong for
        // 44100/48000 Hz stereo audio.
        if !first_frame_decoded {
            let actual_rate = decoded.spec().rate;
            let actual_ch = decoded.spec().channels.count();
            log::info!(
                "[run_recognition] first frame: actual_rate={actual_rate}, actual_channels={actual_ch}, hint_rate={native_rate_hint}, hint_channels={num_channels_hint}"
            );
            actual_native_rate = actual_rate;
            actual_num_channels = actual_ch;
            first_frame_decoded = true;

            // Create resampler now that we know the real rate
            resampler = audio_decoder::create_resampler(actual_native_rate)?;
        }

        let mono = audio_decoder::decode_to_mono_f32(&decoded, actual_num_channels);
        decoded_count += mono.len();

        // Resample to 16 kHz if needed
        let pcm = if let Some(ref resamp) = resampler {
            resamp.resample(&mono, false)
        } else {
            mono
        };

        vad_buf.extend_from_slice(&pcm);

        // Feed 512-sample windows to VAD
        while vad_buf.len() >= window_size {
            if cancelled.load(Ordering::Relaxed) {
                vad.clear();
                return Ok(decoded_count as f32 / actual_native_rate as f32);
            }

            vad.accept_waveform(&vad_buf[..window_size]);
            vad_buf.drain(..window_size);

            while let Some(segment) = vad.front() {
                recognize_segment(recognizer, &segment, segments);
                vad.pop();
            }
        }

        // Report progress
        if let Some(total) = total_samples {
            if total > 0 {
                let percent = ((decoded_count as f32 / total as f32) * 100.0) as u32;
                let percent = percent.min(99); // 100% is set on completion
                if percent != last_progress {
                    last_progress = percent;
                    progress.store(percent, Ordering::Relaxed);
                }
            }
        }
    }

    // Flush remaining samples through VAD
    if !cancelled.load(Ordering::Relaxed) {
        if !vad_buf.is_empty() {
            vad_buf.resize(window_size, 0.0);
            vad.accept_waveform(&vad_buf[..window_size]);
        }
        vad.flush();
        while let Some(segment) = vad.front() {
            recognize_segment(recognizer, &segment, segments);
            vad.pop();
        }
    }

    let audio_duration = decoded_count as f32 / actual_native_rate as f32;
    let final_count = segments.lock().map(|s| s.len()).unwrap_or(0);
    log::info!(
        "[run_recognition] done, total segments: {final_count}, audio_duration: {audio_duration:.2}s, actual_rate: {actual_native_rate}"
    );

    Ok(audio_duration)
}
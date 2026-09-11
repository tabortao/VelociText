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

/// Reset the VAD state after this many seconds without detected speech,
/// but only while the audio is audible (RMS above `VAD_RESET_RMS_THRESHOLD`).
///
/// Silero VAD is stateful (LSTM). Music/singing drives its hidden state into
/// a region where subsequent *real* speech is no longer detected — silently
/// dropping the rest of the file. Resetting the state during long non-speech
/// *music* passages keeps the detector usable. Pure silence does not poison
/// the state, so resets are skipped there to preserve the detector's natural
/// behavior (segmentation of normal speech is unchanged).
const VAD_STALE_RESET_SECS: f32 = 2.0;

/// RMS threshold for "audible" audio (speech/music). Windows below this are
/// treated as silence.
const VAD_RESET_RMS_THRESHOLD: f32 = 0.01;

/// Gaps between detected speech longer than this are re-recognized with
/// fixed-size chunks ("gap backfill").
///
/// Silero VAD does not fire on singing/rap mixed with background music, so
/// song lyrics would be lost entirely. The backfill passes those gaps
/// through the ASR model directly.
const GAP_BACKFILL_MIN_SECS: f32 = 3.0;

/// Chunk length (seconds) used for gap backfill ASR.
/// Kept well below the models' max utterance length (~30s) and qwen3-asr's
/// max_total_len (~39s of audio).
const GAP_BACKFILL_CHUNK_SECS: f32 = 12.0;

/// Map a UI language code (e.g. "en") to the Qwen3-ASR prompt language name
/// (e.g. "English"). Qwen3-ASR accepts full language names in its
/// "language <name>" prompt instruction; sherpa-onnx exposes this through the
/// per-stream "language" option (read only by the Qwen3-ASR recognizer).
///
/// Returns `None` for "auto" or unsupported codes — no language hint is set
/// and the model auto-detects the language.
fn qwen3_language_name(code: &str) -> Option<&'static str> {
    match code {
        "zh" => Some("Chinese"),
        "en" => Some("English"),
        "yue" => Some("Cantonese"),
        "ja" => Some("Japanese"),
        "ko" => Some("Korean"),
        "de" => Some("German"),
        "fr" => Some("French"),
        "es" => Some("Spanish"),
        "ru" => Some("Russian"),
        "it" => Some("Italian"),
        "pt" => Some("Portuguese"),
        "th" => Some("Thai"),
        "vi" => Some("Vietnamese"),
        "id" => Some("Indonesian"),
        "ms" => Some("Malay"),
        "tr" => Some("Turkish"),
        "ar" => Some("Arabic"),
        "hi" => Some("Hindi"),
        "nl" => Some("Dutch"),
        _ => None,
    }
}

/// Recognize a single VAD speech segment: skip if too short or punctuation-only.
///
/// `sample_offset` compensates for VAD state resets: after `vad.reset()` the
/// segment start indices restart from zero, so the absolute position is
/// `sample_offset + segment.start()`.
///
/// `language` is the Qwen3-ASR prompt language name ("English", ...) or `None`
/// to let the model auto-detect. Other models ignore the option.
fn recognize_segment(
    recognizer: &OfflineRecognizer,
    segment: &sherpa_onnx::SpeechSegment,
    segments: &Arc<Mutex<Vec<SegmentResult>>>,
    sample_offset: usize,
    language: Option<&str>,
) {
    let samples = segment.samples();
    let duration = samples.len() as f32 / 16000.0;
    if duration < 0.1 {
        return;
    }

    let start_time = (sample_offset + segment.start().max(0) as usize) as f32 / 16000.0;
    let end_time = start_time + duration;

    let stream = recognizer.create_stream();
    if let Some(lang) = language {
        stream.set_option("language", lang);
    }
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

/// Re-recognize the gaps between detected speech segments with the ASR model
/// directly (fixed-size chunks).
///
/// VAD (Silero) does not detect singing/rap over background music, which
/// would silently drop those parts of the audio. Running the ASR model over
/// the gaps recovers the lyrics; chunks that yield no text (e.g. pure
/// instrumental passages) are discarded.
fn backfill_gaps(
    path: &str,
    recognizer: &OfflineRecognizer,
    audio_duration: f32,
    cancelled: &AtomicBool,
    segments: &Arc<Mutex<Vec<SegmentResult>>>,
    language: Option<&str>,
) {
    // Collect gaps (start, end) longer than GAP_BACKFILL_MIN_SECS.
    let gaps: Vec<(f32, f32)> = {
        let Ok(segs) = segments.lock() else {
            return;
        };
        let mut gaps = Vec::new();
        let mut prev_end = 0.0f32;
        for s in segs.iter() {
            if s.start - prev_end > GAP_BACKFILL_MIN_SECS {
                gaps.push((prev_end, s.start));
            }
            prev_end = prev_end.max(s.end);
        }
        if audio_duration - prev_end > GAP_BACKFILL_MIN_SECS {
            gaps.push((prev_end, audio_duration));
        }
        gaps
    };

    if gaps.is_empty() {
        return;
    }

    log::info!(
        "[backfill_gaps] {} gap(s) to re-recognize: {:?}",
        gaps.len(),
        gaps
    );

    let chunk_len = (GAP_BACKFILL_CHUNK_SECS * 16000.0) as usize;
    let mut filled = 0usize;

    for (gap_start, gap_end) in gaps {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let Ok(samples) = audio_decoder::decode_time_range(path, gap_start, gap_end) else {
            continue; // decode failure for one gap should not abort the rest
        };

        for (i, chunk) in samples.chunks(chunk_len).enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            let cs = gap_start + i as f32 * GAP_BACKFILL_CHUNK_SECS;
            let ce = cs + chunk.len() as f32 / 16000.0;

            let stream = recognizer.create_stream();
            if let Some(lang) = language {
                stream.set_option("language", lang);
            }
            stream.accept_waveform(16000, chunk);
            recognizer.decode(&stream);

            if let Some(r) = stream.get_result() {
                let text = r.text.trim().to_string();
                if !text.is_empty()
                    && !text
                        .chars()
                        .all(|c| c.is_ascii_punctuation() || c.is_ascii_whitespace())
                {
                    filled += 1;
                    if let Ok(mut segs) = segments.lock() {
                        segs.push(SegmentResult {
                            start: cs,
                            end: ce,
                            text,
                        });
                    }
                }
            }
        }
    }

    // Keep segments ordered by start time after merging backfilled results.
    if let Ok(mut segs) = segments.lock() {
        segs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    }
    log::info!("[backfill_gaps] backfilled {filled} segment(s)");
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
/// * `language_code` - Optional UI language code ("en", "zh", ...) to pin the
///   recognition language (effective for Qwen3-ASR models; ignored by others).
pub fn run_recognition(
    path: &str,
    recognizer: &Arc<Mutex<Option<OfflineRecognizer>>>,
    vad: &Arc<Mutex<Option<VoiceActivityDetector>>>,
    cancelled: &AtomicBool,
    progress: &Arc<AtomicU32>,
    segments: &Arc<Mutex<Vec<SegmentResult>>>,
    language_code: Option<&str>,
) -> AppResult<f32> {
    // Resolve the language code once; None = auto-detect.
    let language = language_code.and_then(qwen3_language_name);
    if language.is_some() {
        log::info!("[run_recognition] language pinned: {}", language.unwrap());
    }

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

    let mut recognizer_guard = recognizer
        .lock()
        .map_err(|e| crate::errors::AppError::Transcription(e.to_string()))?;
    let recognizer = recognizer_guard
        .as_mut()
        .ok_or(crate::errors::AppError::Transcription(
            "Recognizer not initialized".into(),
        ))?;
    let mut vad_guard = vad
        .lock()
        .map_err(|e| crate::errors::AppError::Transcription(e.to_string()))?;
    let vad = vad_guard
        .as_mut()
        .ok_or(crate::errors::AppError::Transcription(
            "VAD not initialized".into(),
        ))?;
    vad.reset();

    let window_size: usize = 512;
    let mut vad_buf: Vec<f32> = Vec::new();
    let mut decoded_count: usize = 0;
    let mut last_progress: u32 = 0;

    // VAD state-reset bookkeeping. `reset_offset` is the number of samples
    // fed to the VAD when it was last reset; segment start indices are
    // relative to that point, so the absolute position requires this offset.
    let mut total_fed: usize = 0;
    let mut reset_offset: usize = 0;
    let mut samples_since_speech: usize = 0;
    let stale_reset_samples = (VAD_STALE_RESET_SECS * 16000.0) as usize;

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

            let win = &vad_buf[..window_size];
            let rms = (win.iter().map(|&s| s * s).sum::<f32>() / window_size as f32).sqrt();
            vad.accept_waveform(win);
            vad_buf.drain(..window_size);
            total_fed += window_size;
            samples_since_speech += window_size;

            while let Some(segment) = vad.front() {
                recognize_segment(recognizer, &segment, segments, reset_offset, language);
                vad.pop();
                samples_since_speech = 0;
            }

            if vad.detected() {
                samples_since_speech = 0;
            } else if samples_since_speech >= stale_reset_samples && rms > VAD_RESET_RMS_THRESHOLD
            {
                // Audible audio (music) but no speech for a while: reset the
                // VAD to clear the LSTM state — music can poison it, making
                // later speech undetectable. Safe here because no segment is
                // pending (queue drained above) and no speech is in progress.
                vad.reset();
                reset_offset = total_fed;
                samples_since_speech = 0;
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
            recognize_segment(recognizer, &segment, segments, reset_offset, language);
            vad.pop();
        }
    }

    let audio_duration = decoded_count as f32 / actual_native_rate as f32;

    // Gap backfill: recover speech the VAD missed (singing/rap over music).
    if !cancelled.load(Ordering::Relaxed) && audio_duration > 0.0 {
        backfill_gaps(path, recognizer, audio_duration, cancelled, segments, language);
    }

    let final_count = segments.lock().map(|s| s.len()).unwrap_or(0);
    log::info!(
        "[run_recognition] done, total segments: {final_count}, audio_duration: {audio_duration:.2}s, actual_rate: {actual_native_rate}"
    );

    Ok(audio_duration)
}

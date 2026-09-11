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

// ────────────────────────────────────────────────────────────────────────────
// Repetition-loop collapse (post-processing for autoregressive ASR models)
// ────────────────────────────────────────────────────────────────────────────

/// Minimum consecutive occurrences of the same unit before a run is treated
/// as a decode loop and collapsed. Natural speech rarely repeats the same
/// word/character this many times ("对对对", "哈哈哈" stay untouched).
const REPEAT_UNIT_RUN_LIMIT: usize = 6;
/// Occurrences kept when collapsing a single-unit run.
const REPEAT_UNIT_KEEP: usize = 3;
/// Minimum repetitions of a multi-unit phrase before the tail is treated as
/// a decode loop ("然后然后然后...", "I want to I want to ...").
const REPEAT_PHRASE_MIN_REPS: usize = 3;
/// Phrase repetitions kept when collapsing.
const REPEAT_PHRASE_KEEP: usize = 2;
/// Maximum phrase length (units) considered for loop detection.
const REPEAT_PHRASE_MAX_UNITS: usize = 24;

/// A token of ASR output: a repeatable unit (CJK character or alphanumeric
/// word) or a separator run (whitespace / punctuation).
#[derive(Debug, Clone, PartialEq)]
enum AsrTok {
    Unit(String),
    Sep(String),
}

/// CJK ideographs and kana are treated as single-character units so that
/// Chinese/Japanese loops ("然后然后然后...") are detected as phrase repeats.
fn is_cjk_unit(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF   // Hiragana + Katakana
        | 0x3400..=0x4DBF // CJK Extension A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xAC00..=0xD7AF // Hangul syllables
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
    )
}

fn push_sep(sep: &mut String, toks: &mut Vec<AsrTok>) {
    if !sep.is_empty() {
        toks.push(AsrTok::Sep(std::mem::take(sep)));
    }
}

fn push_word(word: &mut String, toks: &mut Vec<AsrTok>) {
    if !word.is_empty() {
        toks.push(AsrTok::Unit(std::mem::take(word)));
    }
}

/// Split ASR output into units (CJK chars / alphanumeric words) and
/// separators (whitespace / punctuation runs).
fn tokenize_asr_units(text: &str) -> Vec<AsrTok> {
    let mut toks = Vec::new();
    let mut word = String::new();
    let mut sep = String::new();
    for ch in text.chars() {
        if is_cjk_unit(ch) {
            push_word(&mut word, &mut toks);
            push_sep(&mut sep, &mut toks);
            toks.push(AsrTok::Unit(ch.to_string()));
        } else if ch.is_alphanumeric() {
            push_sep(&mut sep, &mut toks);
            word.push(ch);
        } else {
            push_word(&mut word, &mut toks);
            sep.push(ch);
        }
    }
    push_word(&mut word, &mut toks);
    push_sep(&mut sep, &mut toks);
    toks
}

/// Collapse runs of ≥ `REPEAT_UNIT_RUN_LIMIT` identical consecutive units
/// (separated only by separators) down to `REPEAT_UNIT_KEEP` occurrences.
fn collapse_unit_runs(toks: &mut Vec<AsrTok>) {
    let mut i = 0;
    while i < toks.len() {
        let AsrTok::Unit(u) = &toks[i] else {
            i += 1;
            continue;
        };
        let u = u.clone();
        // Extend the run: identical units, arbitrary separators in between.
        let mut j = i + 1;
        let mut count = 1usize;
        while j < toks.len() {
            match &toks[j] {
                AsrTok::Sep(_) => j += 1,
                AsrTok::Unit(v) if v == &u => {
                    count += 1;
                    j += 1;
                }
                _ => break,
            }
        }
        if count >= REPEAT_UNIT_RUN_LIMIT {
            // Token index of the `REPEAT_UNIT_KEEP`-th unit occurrence.
            let keep_tok = {
                let mut seen = 0usize;
                toks[i..j]
                    .iter()
                    .enumerate()
                    .find_map(|(k, t)| match t {
                        AsrTok::Unit(v) if v == &u => {
                            seen += 1;
                            (seen == REPEAT_UNIT_KEEP).then_some(i + k)
                        }
                        _ => None,
                    })
                    .unwrap_or(i)
            };
            // Drop everything past the kept occurrence, but keep a following
            // separator when it carries punctuation ("word, word, word,").
            let mut drop_start = keep_tok + 1;
            if let Some(AsrTok::Sep(s)) = toks.get(drop_start) {
                if !s.chars().all(char::is_whitespace) {
                    drop_start += 1;
                }
            }
            toks.drain(drop_start..j);
            i = drop_start;
        } else {
            i = j;
        }
    }
}

/// Collapse a suffix of the unit sequence that consists of ≥
/// `REPEAT_PHRASE_MIN_REPS` repetitions of a multi-unit pattern down to
/// `REPEAT_PHRASE_KEEP` repetitions. Autoregressive loops run to the end of
/// the generated text, so checking the tail is sufficient.
fn collapse_phrase_loops(toks: &mut Vec<AsrTok>) {
    let unit_pos: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter_map(|(i, t)| match t {
            AsrTok::Unit(_) => Some(i),
            _ => None,
        })
        .collect();
    let n = unit_pos.len();
    if n < REPEAT_PHRASE_MIN_REPS * 2 {
        return;
    }

    let unit_at = |i: usize| -> &str {
        match &toks[unit_pos[i]] {
            AsrTok::Unit(s) => s,
            _ => unreachable!(),
        }
    };

    let max_l = (n / REPEAT_PHRASE_MIN_REPS).min(REPEAT_PHRASE_MAX_UNITS);
    for l in 2..=max_l {
        // Pattern = the last `l` units; the two preceding chunks must match.
        let chunk_eq = |start: usize| -> bool {
            (0..l).all(|k| unit_at(start + k) == unit_at(n - l + k))
        };
        if !chunk_eq(n - 2 * l) || !chunk_eq(n - 3 * l) {
            continue;
        }
        // Extend the repetition count leftward as far as possible.
        let mut reps = REPEAT_PHRASE_MIN_REPS;
        while n >= (reps + 1) * l && chunk_eq(n - (reps + 1) * l) {
            reps += 1;
        }
        // Keep the head plus REPEAT_PHRASE_KEEP repetitions of the pattern.
        let keep_units = n - (reps - REPEAT_PHRASE_KEEP) * l;
        let last_kept_tok = unit_pos[keep_units - 1];
        let mut drop_start = last_kept_tok + 1;
        if let Some(AsrTok::Sep(s)) = toks.get(drop_start) {
            if !s.chars().all(char::is_whitespace) {
                drop_start += 1;
            }
        }
        toks.truncate(drop_start);
        return;
    }
}

/// Collapse pathological repetition loops produced by autoregressive ASR
/// models (Qwen3-ASR): the same word or character repeated many times in a
/// row ("the the the ...", "的的的...") or a short phrase repeated
/// ("然后然后然后...", "I want to I want to ..."). Such loops can fill the
/// model's entire token budget with garbage — sherpa-onnx exposes no
/// repetition penalty for Qwen3-ASR, so this runs as a post-processing step
/// on every recognized segment.
///
/// Thresholds are conservative so natural repetitions pass through unchanged:
/// "对对对", "哈哈哈", "非常好非常好" (phrase twice) and "very very very"
/// are all kept as-is. Single units repeated ≥ 6 times are truncated to 3
/// occurrences; a phrase repeated ≥ 3 times at the tail is truncated to 2
/// repetitions. Non-autoregressive models (SenseVoice, Paraformer) cannot
/// loop and their output is unaffected.
pub fn collapse_repetition_loops(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() < 4 {
        return trimmed.to_string();
    }
    let mut toks = tokenize_asr_units(trimmed);
    collapse_unit_runs(&mut toks);
    collapse_phrase_loops(&mut toks);
    let collapsed: String = toks
        .iter()
        .map(|t| match t {
            AsrTok::Unit(s) | AsrTok::Sep(s) => s.as_str(),
        })
        .collect();
    let collapsed = collapsed.trim();
    if collapsed != trimmed {
        log::debug!(
            "[repetition-collapse] collapsed decode loop: {} chars -> {} chars",
            trimmed.chars().count(),
            collapsed.chars().count()
        );
    }
    collapsed.to_string()
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
        let text = collapse_repetition_loops(&r.text);
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
                let text = collapse_repetition_loops(&r.text);
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
    if let Some(lang) = language {
        log::info!("[run_recognition] language pinned: {lang}");
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

#[cfg(test)]
mod repetition_tests {
    use super::collapse_repetition_loops;

    #[test]
    fn test_collapse_english_word_loop() {
        assert_eq!(
            collapse_repetition_loops("okay the the the the the the the the the the"),
            "okay the the the"
        );
    }

    #[test]
    fn test_collapse_cjk_char_loop() {
        assert_eq!(
            collapse_repetition_loops("今天天气很的的的的的的的的的"),
            "今天天气很的的的"
        );
    }

    #[test]
    fn test_collapse_cjk_phrase_loop() {
        // "然后" repeated 6 times = [然, 后] × 6 → keep 2 repetitions.
        assert_eq!(
            collapse_repetition_loops("首先我们来看然后然后然后然后然后然后"),
            "首先我们来看然后然后"
        );
    }

    #[test]
    fn test_collapse_english_phrase_loop() {
        assert_eq!(
            collapse_repetition_loops(
                "This is a test I want to go I want to go I want to go I want to go"
            ),
            "This is a test I want to go I want to go"
        );
    }

    #[test]
    fn test_collapse_punctuated_word_loop() {
        assert_eq!(
            collapse_repetition_loops("yes, yes, yes, yes, yes, yes, yes"),
            "yes, yes, yes,"
        );
    }

    #[test]
    fn test_keeps_natural_repetition() {
        assert_eq!(collapse_repetition_loops("对对对，没错"), "对对对，没错");
        assert_eq!(collapse_repetition_loops("哈哈哈哈，太好笑了"), "哈哈哈哈，太好笑了");
        assert_eq!(collapse_repetition_loops("非常好非常好"), "非常好非常好");
        assert_eq!(
            collapse_repetition_loops("very very very interesting"),
            "very very very interesting"
        );
        assert_eq!(collapse_repetition_loops("Word. Word. Word."), "Word. Word. Word.");
    }

    #[test]
    fn test_normal_text_unchanged() {
        let t = "Hello, world! 2024 你好，世界。日本語テスト";
        assert_eq!(collapse_repetition_loops(t), t);
    }

    #[test]
    fn test_empty_and_short() {
        assert_eq!(collapse_repetition_loops(""), "");
        assert_eq!(collapse_repetition_loops("  hi  "), "hi");
        assert_eq!(collapse_repetition_loops("。。。"), "。。。");
    }
}

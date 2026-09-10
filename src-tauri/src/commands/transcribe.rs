use crate::engine::audio_decoder;
use crate::engine::export::ExportManager;
use crate::engine::transcription_pipeline::{run_recognition, SegmentResult, VadSettings};
use crate::models::task::TranscribeSegment;
use crate::AppState;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::State;

/// Idle time (seconds) before ASR models are actually released from memory
/// after `release_asr_models` is requested. This allows switching between
/// the Transcribe and Subtitle pages without reloading the shared models.
const MODEL_RELEASE_DELAY_SECS: u64 = 300;

/// Time (seconds) after recognition completes before the backend result
/// buffers (segments) are cleared. The frontend keeps its own copy.
const SEGMENT_CLEANUP_DELAY_SECS: u64 = 60;

/// Export transcription result and write directly to file
#[tauri::command]
pub async fn export_to_file(
    segments: Vec<TranscribeSegment>,
    format: String,
    save_path: String,
) -> Result<(), String> {
    let content = ExportManager::export(&segments, &format).map_err(|e| e.to_string())?;
    std::fs::write(&save_path, content).map_err(|e| format!("Failed to write file: {}", e))
}

/// Check if FFmpeg is available: the explicit path from the config first,
/// then the system PATH.
#[tauri::command]
pub async fn check_ffmpeg(state: State<'_, AppState>) -> Result<String, String> {
    let explicit = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.ffmpeg_path.clone()
    };
    if let Some(path) = explicit {
        if !path.is_empty() {
            if let Ok(version) = crate::engine::audio_extractor::check_ffmpeg_at(&path) {
                return Ok(version);
            }
        }
    }
    crate::engine::audio_extractor::check_ffmpeg().map_err(|e| e.to_string())
}

// ============================================================================
// Streaming recognition commands (new architecture)
// ============================================================================

/// Initialization status response.
#[derive(Debug, Clone, Serialize)]
pub struct InitStatus {
    status: u8, // 0 = pending, 1 = ready, 2 = error, 3 = released
    error: String,
    #[serde(rename = "numThreads")]
    num_threads: u32,
}

/// Processing state for polling.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingState {
    percent: u32,
    status: String,
    segments: Vec<SegmentResult>,
    #[serde(rename = "elapsedSecs")]
    elapsed_secs: f32,
    #[serde(rename = "audioDurationSecs")]
    audio_duration_secs: f32,
}

/// Return model initialization status.
/// Frontend should poll this on startup until status == 1 (ready).
#[tauri::command]
pub fn get_init_status(state: State<'_, AppState>) -> InitStatus {
    let status = state.init_status.load(Ordering::Relaxed);
    let error = state
        .init_error
        .lock()
        .map(|e| e.clone())
        .unwrap_or_default();
    let num_threads = state.num_threads.load(Ordering::Relaxed);
    InitStatus {
        status,
        error,
        num_threads,
    }
}

/// Ensure ASR models are loaded (lazy loading).
/// Called when the user navigates to the Transcribe / Subtitle page.
/// If models are already loaded (status == 1), returns immediately and
/// cancels any pending deferred release (so switching pages doesn't reload).
#[tauri::command]
pub async fn ensure_asr_models(state: State<'_, AppState>) -> Result<InitStatus, String> {
    // Invalidate any pending deferred release — the models are needed again.
    state.release_token.fetch_add(1, Ordering::SeqCst);

    let current_status = state.init_status.load(Ordering::Relaxed);

    // Already ready
    if current_status == 1 {
        return Ok(InitStatus {
            status: 1,
            error: String::new(),
            num_threads: state.num_threads.load(Ordering::Relaxed),
        });
    }

    // Currently loading — just return status, frontend will poll
    if current_status == 0 {
        return Ok(InitStatus {
            status: 0,
            error: String::new(),
            num_threads: 0,
        });
    }

    // Status == 2 (error) or models were released (status reset to 3)
    // Need to (re)load models
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };
    let active_model_name = state
        .active_model
        .lock()
        .map(|a| a.clone())
        .unwrap_or_default();
    let hotwords_file = state
        .hotwords_file_path
        .lock()
        .map(|h| h.clone())
        .unwrap_or(None);
    let settings = state
        .vad_settings
        .lock()
        .map(|s| s.clone())
        .map_err(|e| e.to_string())?;

    // Set status to loading
    state.init_status.store(0, Ordering::Relaxed);

    let recognizer_arc = Arc::clone(&state.recognizer);
    let vad_arc = Arc::clone(&state.vad_detector);
    let init_status_arc = Arc::clone(&state.init_status);
    let init_error_arc = Arc::clone(&state.init_error);
    let num_threads_arc = Arc::clone(&state.num_threads);
    let active_model_arc = Arc::clone(&state.active_model);

    tokio::task::spawn_blocking(move || {
        let preferred = if active_model_name.is_empty() {
            None
        } else {
            Some(active_model_name.as_str())
        };

        // Write crash marker
        let marker_path = std::path::Path::new(&model_path).join(".model_loading");
        if let Some(parent) = marker_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&marker_path, &active_model_name);

        match crate::build_models(&model_path, &settings, preferred, hotwords_file) {
            Ok((rec, vad, threads, model_name)) => {
                let _ = std::fs::remove_file(&marker_path);

                log::info!("[ensure_asr_models] models loaded, threads={threads}, model={model_name}");
                let _ = recognizer_arc.lock().map(|mut r| *r = Some(rec));
                let _ = vad_arc.lock().map(|mut v| *v = Some(vad));
                num_threads_arc.store(threads, Ordering::Relaxed);
                let _ = active_model_arc.lock().map(|mut a| *a = model_name.clone());

                // Persist actual model to config
                let mut cfg = crate::config::app_config::AppConfig::load();
                if cfg.active_model != model_name {
                    cfg.active_model = model_name;
                    let _ = crate::config::app_config::AppConfig::save(&cfg);
                }

                init_status_arc.store(1, Ordering::Relaxed);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&marker_path);
                log::error!("[ensure_asr_models] failed: {e}");
                let _ = init_error_arc.lock().map(|mut err| *err = e);
                init_status_arc.store(2, Ordering::Relaxed);
            }
        }
    })
    .await
    .map_err(|e| format!("Model loading task failed: {e}"))?;

    Ok(InitStatus {
        status: state.init_status.load(Ordering::Relaxed),
        error: state
            .init_error
            .lock()
            .map(|e| e.clone())
            .unwrap_or_default(),
        num_threads: state.num_threads.load(Ordering::Relaxed),
    })
}

/// Request release of ASR models from memory to reduce RAM usage.
///
/// The release is DEFERRED: models are actually freed after
/// `MODEL_RELEASE_DELAY_SECS` of inactivity. If the user switches between
/// the Transcribe and Subtitle pages (both share the same models), the
/// pending release is cancelled by `ensure_asr_models` and the models stay
/// in memory — no reload cost.
#[tauri::command]
pub fn release_asr_models(state: State<'_, AppState>) -> Result<(), String> {
    // Bump the token: invalidates any previously scheduled release.
    let my_token = state.release_token.fetch_add(1, Ordering::SeqCst) + 1;

    let recognizer = Arc::clone(&state.recognizer);
    let vad = Arc::clone(&state.vad_detector);
    let running = Arc::clone(&state.running);
    let init_status = Arc::clone(&state.init_status);
    let token = Arc::clone(&state.release_token);

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(MODEL_RELEASE_DELAY_SECS));

        // Cancelled by a new ensure/release request?
        if token.load(Ordering::SeqCst) != my_token {
            log::info!("[release_asr_models] deferred release cancelled (models in use again)");
            return;
        }
        // A recognition is running — keep models.
        if running.load(Ordering::SeqCst) {
            log::info!("[release_asr_models] deferred release skipped (recognition running)");
            return;
        }
        // Models are not currently loaded.
        if init_status.load(Ordering::Relaxed) != 1 {
            return;
        }

        // Drop recognizer and VAD
        if let Ok(mut rec) = recognizer.lock() {
            *rec = None;
        }
        if let Ok(mut v) = vad.lock() {
            *v = None;
        }

        // Reset init status so next ensure_asr_models will reload
        init_status.store(3, Ordering::Relaxed); // 3 = released

        log::info!("[release_asr_models] ASR models released from memory after idle timeout");
    });

    Ok(())
}

/// Start recognition in a background thread. Returns immediately.
/// Frontend should poll `get_recognition_progress` to track progress.
#[tauri::command]
pub fn recognize_file(path: String, state: State<'_, AppState>) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("Recognition is already running".to_string());
    }

    // Check initialization
    let init = state.init_status.load(Ordering::Relaxed);
    if init == 0 {
        state.running.store(false, Ordering::SeqCst);
        return Err("Models are still loading, please wait".to_string());
    }
    if init == 2 {
        state.running.store(false, Ordering::SeqCst);
        let err = state.init_error.lock().map_err(|e| e.to_string())?.clone();
        return Err(format!("Initialization failed: {err}"));
    }
    if init == 3 {
        state.running.store(false, Ordering::SeqCst);
        return Err("Models have been released. Please wait for them to reload.".to_string());
    }

    log::info!("[recognize_file] starting recognition for: {path}");

    // Reset state
    state.cancelled.store(false, Ordering::Relaxed);
    state.progress.store(0, Ordering::Relaxed);
    *state.status.lock().map_err(|e| e.to_string())? = "processing".to_string();
    state.segments.lock().map_err(|e| e.to_string())?.clear();
    *state.audio_path.lock().map_err(|e| e.to_string())? = path.clone();
    *state.elapsed_secs.lock().map_err(|e| e.to_string())? = 0.0;
    *state
        .audio_duration_secs
        .lock()
        .map_err(|e| e.to_string())? = 0.0;

    // Clone Arc handles for the worker thread
    let recognizer = Arc::clone(&state.recognizer);
    let vad = Arc::clone(&state.vad_detector);
    let running = Arc::clone(&state.running);
    let cancelled = Arc::clone(&state.cancelled);
    let progress = Arc::clone(&state.progress);
    let status = Arc::clone(&state.status);
    let segments = Arc::clone(&state.segments);
    let elapsed_secs = Arc::clone(&state.elapsed_secs);
    let audio_duration_secs = Arc::clone(&state.audio_duration_secs);

    std::thread::spawn(move || {
        let start_time = Instant::now();
        let result = run_recognition(&path, &recognizer, &vad, &cancelled, &progress, &segments);
        let elapsed = start_time.elapsed().as_secs_f32();

        if let Ok(mut e) = elapsed_secs.lock() {
            *e = elapsed;
        }

        let Ok(mut s) = status.lock() else {
            running.store(false, Ordering::SeqCst);
            return;
        };
        match result {
            Ok(audio_dur) => {
                if let Ok(mut d) = audio_duration_secs.lock() {
                    *d = audio_dur;
                }
                if cancelled.load(Ordering::Relaxed) {
                    *s = "cancelled".to_string();
                } else {
                    progress.store(100, Ordering::Relaxed);
                    *s = "done".to_string();
                }
            }
            Err(e) => {
                *s = format!("error: {e}");
            }
        }
        running.store(false, Ordering::SeqCst);

        // Schedule cleanup of the backend result buffers. The frontend
        // keeps its own copy of the results after polling completes.
        let cleanup_segments = Arc::clone(&segments);
        let cleanup_running = Arc::clone(&running);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(SEGMENT_CLEANUP_DELAY_SECS));
            // Skip if a new recognition started meanwhile
            if cleanup_running.load(Ordering::SeqCst) {
                return;
            }
            if let Ok(mut segs) = cleanup_segments.lock() {
                if !segs.is_empty() {
                    segs.clear();
                    segs.shrink_to_fit();
                    log::info!("[recognize_file] backend result buffers reclaimed");
                }
            }
        });
    });

    Ok(())
}

/// Poll this from the frontend to get current progress and results.
/// Frontend should call this every ~200ms while recognition is running.
#[tauri::command]
pub fn get_recognition_progress(state: State<'_, AppState>) -> Result<ProcessingState, String> {
    let percent = state.progress.load(Ordering::Relaxed);
    let status = state.status.lock().map_err(|e| e.to_string())?.clone();
    let raw_segments = state.segments.lock().map_err(|e| e.to_string())?.clone();
    let elapsed_secs = *state.elapsed_secs.lock().map_err(|e| e.to_string())?;
    let audio_duration_secs = *state
        .audio_duration_secs
        .lock()
        .map_err(|e| e.to_string())?;

    // Apply text replacements from dictionary config
    let segments: Vec<SegmentResult> = {
        let dict_config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
        if dict_config.replacements.is_empty() {
            raw_segments
        } else {
            raw_segments
                .into_iter()
                .map(|s| SegmentResult {
                    text: dict_config.apply_replacements(&s.text),
                    ..s
                })
                .collect()
        }
    };

    Ok(ProcessingState {
        percent,
        status,
        segments,
        elapsed_secs,
        audio_duration_secs,
    })
}

/// Cancel the current recognition.
#[tauri::command]
pub fn cancel_recognition(state: State<'_, AppState>) {
    state.cancelled.store(true, Ordering::Relaxed);
}

/// Save a single audio segment as a WAV file.
/// Re-decodes only the needed portion from the file (no full-file buffering).
#[tauri::command]
pub fn save_segment_as_wav(
    path: String,
    start: f32,
    end: f32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let audio_path = state.audio_path.lock().map_err(|e| e.to_string())?;
    if audio_path.is_empty() {
        return Err("No audio file has been processed".to_string());
    }

    let samples =
        audio_decoder::decode_time_range(&audio_path, start, end).map_err(|e| e.to_string())?;
    audio_decoder::write_wav(&path, &samples).map_err(|e| e.to_string())
}

/// Get current VAD settings.
#[tauri::command]
pub fn get_vad_settings(state: State<'_, AppState>) -> Result<VadSettings, String> {
    state
        .vad_settings
        .lock()
        .map(|s| s.clone())
        .map_err(|e| e.to_string())
}

/// Apply new VAD settings (reloads models in background).
#[tauri::command]
pub fn apply_vad_settings(
    new_settings: VadSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("Cannot change settings while recognition is running".to_string());
    }
    let init = state.init_status.load(Ordering::Relaxed);
    if init == 0 {
        return Err("Models are still loading, please wait".to_string());
    }

    // Validate settings
    if new_settings.threshold <= 0.0 || new_settings.threshold >= 1.0 {
        return Err("threshold must be between 0.0 and 1.0 (exclusive)".to_string());
    }
    if new_settings.min_silence_duration < 0.0 {
        return Err("min_silence_duration must be >= 0".to_string());
    }
    if new_settings.min_speech_duration < 0.0 {
        return Err("min_speech_duration must be >= 0".to_string());
    }
    if new_settings.max_speech_duration <= 0.0 {
        return Err("max_speech_duration must be > 0".to_string());
    }
    if new_settings.num_threads < 1 || new_settings.num_threads > 16 {
        return Err("num_threads must be between 1 and 16".to_string());
    }

    // Check if settings actually changed
    {
        let current = state.vad_settings.lock().map_err(|e| e.to_string())?;
        if current.threshold == new_settings.threshold
            && current.min_silence_duration == new_settings.min_silence_duration
            && current.min_speech_duration == new_settings.min_speech_duration
            && current.max_speech_duration == new_settings.max_speech_duration
            && current.num_threads == new_settings.num_threads
        {
            return Ok(());
        }
    }

    // Set init_status to 0 so the frontend shows "Loading models..."
    state.init_status.store(0, Ordering::Relaxed);

    // Store the new settings
    *state.vad_settings.lock().map_err(|e| e.to_string())? = new_settings.clone();

    // Rebuild models in background
    let recognizer_arc = Arc::clone(&state.recognizer);
    let vad_arc = Arc::clone(&state.vad_detector);
    let init_status = Arc::clone(&state.init_status);
    let init_error = Arc::clone(&state.init_error);
    let init_num_threads = Arc::clone(&state.num_threads);
    let active_model_arc = Arc::clone(&state.active_model);
    let hotwords_file = state
        .hotwords_file_path
        .lock()
        .map(|h| h.clone())
        .unwrap_or(None);
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    std::thread::spawn(move || {
        log::info!("[apply_settings] rebuilding models with new settings...");
        let active = active_model_arc
            .lock()
            .map(|a| a.clone())
            .unwrap_or_default();
        let preferred = if active.is_empty() {
            None
        } else {
            Some(active.as_str())
        };
        match crate::build_models(&model_path, &new_settings, preferred, hotwords_file) {
            Ok((rec, vad, threads, model_name)) => {
                log::info!("[apply_settings] models rebuilt, num_threads={threads}, active_model={model_name}");
                let r_ok = recognizer_arc
                    .lock()
                    .map(|mut r| {
                        *r = Some(rec);
                    })
                    .is_ok();
                let v_ok = vad_arc
                    .lock()
                    .map(|mut v| {
                        *v = Some(vad);
                    })
                    .is_ok();
                if r_ok && v_ok {
                    init_num_threads.store(threads, Ordering::Relaxed);
                    init_status.store(1, Ordering::Relaxed);
                } else {
                    log::error!("[apply_settings] mutex poisoned");
                    if let Ok(mut err) = init_error.lock() {
                        *err = "Internal error: mutex poisoned".to_string();
                    }
                    init_status.store(2, Ordering::Relaxed);
                }
            }
            Err(e) => {
                log::error!("[apply_settings] rebuild failed: {e}");
                if let Ok(mut err) = init_error.lock() {
                    *err = e;
                }
                init_status.store(2, Ordering::Relaxed);
            }
        }
    });

    Ok(())
}

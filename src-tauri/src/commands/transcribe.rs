use crate::engine::audio_decoder;
use crate::engine::export::{segment_text, ExportManager};
use crate::engine::model_manager::ModelManager;
use crate::engine::recognizer_factory::{ModelType, RecognizerConfig, RecognizerFactory};
use crate::engine::transcription_pipeline::{run_recognition, SegmentResult, VadSettings};
use crate::engine::vad::detect_speech_segments;
use crate::models::task::{BatchFileResult, BatchResult, TranscribeOptions, TranscribeResult, TranscribeSegment};
use crate::AppState;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tauri::{Emitter, State};

/// Progress event payload
#[derive(Debug, Clone, Serialize)]
struct ProgressPayload {
    percent: u32,
    stage: String,
    message: String,
}

/// Transcribe audio/video file
#[tauri::command]
pub async fn transcribe_file(
    options: TranscribeOptions,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<TranscribeResult, String> {
    let start_time = Instant::now();
    let file_name = std::path::Path::new(&options.file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    // Determine model type
    let model_type = options
        .model_type
        .as_deref()
        .map(|s| {
            match s {
                "paraformer" => ModelType::Paraformer,
                "zipformer-ctc" => ModelType::ZipformerCtc,
                "transducer" => ModelType::Transducer,
                _ => ModelType::SenseVoice,
            }
        })
        .unwrap_or(ModelType::SenseVoice);

    let model_type_display = model_type.display_name().to_string();
    let model_dir_name = model_type.dir_name().to_string();

    // Get model path
    let model_dir = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let manager = ModelManager::new(config.model_path.clone());
        manager
            .get_model_path(&model_dir_name)
            .map_err(|e| e.to_string())?
    };

    // Initial progress
    emit_progress(&app_handle, 5, "extracting", "extracting_audio");

    // Progress channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProgressPayload>(16);

    let file_path = options.file_path.clone();
    let model_dir_clone = model_dir.clone();
    let vad_model_dir = model_dir.clone();
    let hotwords_file = options.hotwords_file.clone();
    let use_vad = options.use_vad.unwrap_or(false);
    let emit_handle = app_handle.clone();

    // Execute transcription in spawn_blocking
    let handle = tokio::task::spawn_blocking(move || -> Result<(Vec<TranscribeSegment>, f64), String> {
        use crate::engine::audio_extractor::extract_audio;
        use tempfile::NamedTempFile;

        // 1. Extract audio
        tx.blocking_send(ProgressPayload {
            percent: 10,
            stage: "extracting".into(),
            message: "extracting_audio".into(),
        }).ok();

        let temp_wav = NamedTempFile::new().map_err(|e| e.to_string())?;
        let wav_path = temp_wav.path().to_string_lossy().to_string();
        let duration = extract_audio(&file_path, &wav_path).map_err(|e| e.to_string())?;

        // Create recognizer
        let factory_config = RecognizerConfig {
            model_dir: model_dir_clone,
            num_threads: 4,
            hotwords_file,
            hotwords_score: 1.5,
            use_itn: true,
        };

        let recognizer = RecognizerFactory::create(&model_type, &factory_config)
            .map_err(|e| e.to_string())?;

        // 2. Speech recognition — with or without VAD
        if use_vad {
            // Build silero-vad model path: {models_dir}/silero-vad/model.onnx
            let vad_model_path = std::path::Path::new(&vad_model_dir)
                .parent()
                .map(|p| p.join("silero-vad").join("model.onnx"))
                .unwrap_or_else(|| {
                    std::path::Path::new(&vad_model_dir).join("silero-vad").join("model.onnx")
                });
            let vad_model_path_str = vad_model_path.to_string_lossy().to_string();

            // Always do full audio ASR first (most reliable)
            tx.blocking_send(ProgressPayload {
                percent: 30,
                stage: "transcribing".into(),
                message: "transcribing_full".into(),
            }).ok();

            let audio = sherpa_onnx::Wave::read(&wav_path)
                .ok_or_else(|| format!("Failed to read WAV: {}", wav_path))?;
            let stream = recognizer.create_stream();
            stream.accept_waveform(audio.sample_rate(), audio.samples());
            recognizer.decode(&stream);
            let full_text = stream.get_result().map(|r| r.text).unwrap_or_default();

            // Try VAD for time-aligned segmentation
            tx.blocking_send(ProgressPayload {
                percent: 60,
                stage: "vad".into(),
                message: "vad_detecting".into(),
            }).ok();

            let vad_result = detect_speech_segments(&wav_path, &vad_model_path_str);

            match vad_result {
                Ok((vad_segments, _)) if !vad_segments.is_empty() => {
                    // Distribute full text across VAD segments proportionally
                    let total_chars = full_text.chars().count();
                    if total_chars > 0 {
                        let mut recognized_segments: Vec<TranscribeSegment> = Vec::new();
                        let mut char_offset = 0;

                        let total_vad_duration: f64 = vad_segments.iter()
                            .map(|s| s.end - s.start)
                            .sum();

                        for vad_seg in &vad_segments {
                            let seg_duration = vad_seg.end - vad_seg.start;
                            let ratio = if total_vad_duration > 0.0 { seg_duration / total_vad_duration } else { 0.0 };
                            let seg_chars = (total_chars as f64 * ratio).ceil() as usize;
                            let seg_chars = seg_chars.min(total_chars - char_offset);
                            if seg_chars == 0 {
                                break;
                            }
                            let seg_text: String = full_text.chars()
                                .skip(char_offset)
                                .take(seg_chars)
                                .collect();
                            char_offset += seg_chars;

                            // Apply smart sentence segmentation within this VAD segment
                            let sub_segments = segment_text(&seg_text, seg_duration);
                            for sub in sub_segments {
                                if !sub.text.trim().is_empty() {
                                    recognized_segments.push(TranscribeSegment {
                                        start: vad_seg.start + sub.start,
                                        end: vad_seg.start + sub.end,
                                        text: sub.text,
                                    });
                                }
                            }
                        }

                        tx.blocking_send(ProgressPayload {
                            percent: 95,
                            stage: "done".into(),
                            message: "transcribing_done".into(),
                        }).ok();

                        return Ok((recognized_segments, duration));
                    }
                }
                _ => {
                    // VAD failed or no segments, log and fall through
                    log::warn!("VAD failed or returned no segments, using text-based segmentation");
                }
            }

            // Fallback: use text-based segmentation
            tx.blocking_send(ProgressPayload {
                percent: 85,
                stage: "segmenting".into(),
                message: "segmenting_text".into(),
            }).ok();

            let segments = segment_text(&full_text, duration);
            Ok((segments, duration))
        } else {
            // Full audio recognition with smart text segmentation
            tx.blocking_send(ProgressPayload {
                percent: 40,
                stage: "transcribing".into(),
                message: "transcribing_full".into(),
            }).ok();

            let audio = sherpa_onnx::Wave::read(&wav_path)
                .ok_or_else(|| format!("Failed to read WAV: {}", wav_path))?;
            let stream = recognizer.create_stream();
            stream.accept_waveform(audio.sample_rate(), audio.samples());
            recognizer.decode(&stream);

            let text = stream.get_result().map(|r| r.text).unwrap_or_default();

            tx.blocking_send(ProgressPayload {
                percent: 85,
                stage: "segmenting".into(),
                message: "segmenting_text".into(),
            }).ok();

            // Smart segmentation
            let segments = segment_text(&text, duration);

            Ok((segments, duration))
        }
    });

    // Forward progress events to frontend
    let progress_task = tokio::spawn(async move {
        while let Some(payload) = rx.recv().await {
            emit_handle.emit("transcribe-progress", &payload).ok();
        }
    });

    // Wait for completion
    let (segments, audio_duration) = handle
        .await
        .map_err(|e| format!("Transcription task error: {}", e))?
        .map_err(|e| format!("Transcription failed: {}", e))?;

    progress_task.abort();

    let elapsed = start_time.elapsed();

    emit_progress(
        &app_handle,
        100,
        "done",
        "transcribing_done",
    );

    Ok(TranscribeResult {
        segments,
        audio_duration,
        elapsed_ms: elapsed.as_millis() as u64,
        file_name,
        model_type: model_type_display,
    })
}

fn emit_progress(app_handle: &tauri::AppHandle, percent: u32, stage: &str, message: &str) {
    let _ = app_handle.emit(
        "transcribe-progress",
        ProgressPayload {
            percent,
            stage: stage.into(),
            message: message.into(),
        },
    );
}

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

/// Open a file with the system's default application
#[tauri::command]
pub async fn open_file_with_system(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file: {}", e))
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file: {}", e))
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open file: {}", e))
    }
}

/// Export transcription result (returns content string, for legacy use)
#[tauri::command]
pub async fn export_result(
    segments: Vec<TranscribeSegment>,
    format: String,
) -> Result<String, String> {
    ExportManager::export(&segments, &format).map_err(|e| e.to_string())
}

/// Check if FFmpeg is available
#[tauri::command]
pub async fn check_ffmpeg() -> Result<String, String> {
    crate::engine::audio_extractor::check_ffmpeg().map_err(|e| e.to_string())
}

/// Batch transcribe multiple files sequentially
#[tauri::command]
pub async fn transcribe_batch(
    file_paths: Vec<String>,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<BatchResult, String> {
    let batch_start = Instant::now();
    let total = file_paths.len();

    if total == 0 {
        return Ok(BatchResult {
            results: vec![],
            total_files: 0,
            succeeded: 0,
            failed: 0,
            total_elapsed_ms: 0,
            total_audio_duration: 0.0,
        });
    }

    // Load model once for all files
    let model_type = ModelType::SenseVoice;
    let model_dir_name = model_type.dir_name().to_string();

    let model_dir = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        let manager = ModelManager::new(config.model_path.clone());
        manager
            .get_model_path(&model_dir_name)
            .map_err(|e| e.to_string())?
    };

    let factory_config = RecognizerConfig {
        model_dir,
        num_threads: 4,
        hotwords_file: None,
        hotwords_score: 1.5,
        use_itn: true,
    };

    // Process all files in a single blocking task to share the recognizer
    let emit_handle = app_handle.clone();
    let handle = tokio::task::spawn_blocking(move || -> Result<BatchResult, String> {
        use crate::engine::audio_extractor::extract_audio;
        use tempfile::NamedTempFile;

        let recognizer = RecognizerFactory::create(&model_type, &factory_config)
            .map_err(|e| e.to_string())?;

        let mut results = Vec::with_capacity(total);
        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut total_audio_duration = 0.0f64;

        for (i, file_path) in file_paths.iter().enumerate() {
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());

            let file_pct = ((i as f64 / total as f64) * 100.0) as u32;
            emit_handle.emit("transcribe-progress", ProgressPayload {
                percent: file_pct,
                stage: "batch".into(),
                message: "batch_processing".into(),
            }).ok();

            let file_start = Instant::now();

            match (|| -> Result<(Vec<TranscribeSegment>, f64), String> {
                let temp_wav = NamedTempFile::new().map_err(|e| e.to_string())?;
                let wav_path = temp_wav.path().to_string_lossy().to_string();
                let duration = extract_audio(file_path, &wav_path).map_err(|e| e.to_string())?;

                let audio = sherpa_onnx::Wave::read(&wav_path)
                    .ok_or_else(|| format!("Failed to read WAV: {}", wav_path))?;
                let stream = recognizer.create_stream();
                stream.accept_waveform(audio.sample_rate(), audio.samples());
                recognizer.decode(&stream);
                let text = stream.get_result().map(|r| r.text).unwrap_or_default();

                let segments = segment_text(&text, duration);
                Ok((segments, duration))
            })() {
                Ok((segments, duration)) => {
                    succeeded += 1;
                    total_audio_duration += duration;
                    results.push(BatchFileResult {
                        file_name: file_name.clone(),
                        file_path: file_path.clone(),
                        success: true,
                        segments,
                        audio_duration: duration,
                        elapsed_ms: file_start.elapsed().as_millis() as u64,
                        model_type: model_type.display_name().to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    failed += 1;
                    results.push(BatchFileResult {
                        file_name: file_name.clone(),
                        file_path: file_path.clone(),
                        success: false,
                        segments: vec![],
                        audio_duration: 0.0,
                        elapsed_ms: file_start.elapsed().as_millis() as u64,
                        model_type: model_type.display_name().to_string(),
                        error: Some(err),
                    });
                }
            }
        }

        emit_handle.emit("transcribe-progress", ProgressPayload {
            percent: 100,
            stage: "batch".into(),
            message: "batch_complete".into(),
        }).ok();

        Ok(BatchResult {
            results,
            total_files: total,
            succeeded,
            failed,
            total_elapsed_ms: batch_start.elapsed().as_millis() as u64,
            total_audio_duration,
        })
    });

    handle
        .await
        .map_err(|e| format!("Batch task error: {}", e))?
}

/// Check if file format is supported
#[tauri::command]
pub async fn check_file_format(file_path: String) -> Result<bool, String> {
    Ok(crate::engine::audio_extractor::is_supported_format(
        &file_path,
    ))
}

/// Get available model types
#[tauri::command]
pub async fn get_model_types(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let available = RecognizerFactory::list_available(&config.model_path);

    let all_types = vec![
        serde_json::json!({
            "id": "sense-voice",
            "name": "SenseVoice-Small",
            "available": available.contains(&"sense-voice-small".to_string()),
            "default": true
        }),
        serde_json::json!({
            "id": "paraformer-large",
            "name": "Paraformer-Large",
            "available": available.contains(&"paraformer-large".to_string()),
            "default": false
        }),
        serde_json::json!({
            "id": "paraformer",
            "name": "Paraformer",
            "available": available.contains(&"paraformer".to_string()),
            "default": false
        }),
        serde_json::json!({
            "id": "zipformer-ctc",
            "name": "Zipformer CTC",
            "available": available.contains(&"zipformer-ctc".to_string()),
            "default": false
        }),
        serde_json::json!({
            "id": "transducer",
            "name": "Transducer",
            "available": available.contains(&"transducer".to_string()),
            "default": false
        }),
    ];

    Ok(all_types)
}

// ============================================================================
// Streaming recognition commands (new architecture)
// ============================================================================

/// Initialization status response.
#[derive(Debug, Clone, Serialize)]
pub struct InitStatus {
    status: u8,        // 0 = pending, 1 = ready, 2 = error
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
    let error = state.init_error.lock().map(|e| e.clone()).unwrap_or_default();
    let num_threads = state.num_threads.load(Ordering::Relaxed);
    InitStatus {
        status,
        error,
        num_threads,
    }
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

    log::info!("[recognize_file] starting recognition for: {path}");

    // Reset state
    state.cancelled.store(false, Ordering::Relaxed);
    state.progress.store(0, Ordering::Relaxed);
    *state.status.lock().map_err(|e| e.to_string())? = "processing".to_string();
    state.segments.lock().map_err(|e| e.to_string())?.clear();
    *state.audio_path.lock().map_err(|e| e.to_string())? = path.clone();
    *state.elapsed_secs.lock().map_err(|e| e.to_string())? = 0.0;
    *state.audio_duration_secs.lock().map_err(|e| e.to_string())? = 0.0;

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
        let result = run_recognition(
            &path,
            &recognizer,
            &vad,
            &cancelled,
            &progress,
            &segments,
        );
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
    let audio_duration_secs = *state.audio_duration_secs.lock().map_err(|e| e.to_string())?;

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

    let samples = audio_decoder::decode_time_range(&audio_path, start, end)
        .map_err(|e| e.to_string())?;
    audio_decoder::write_wav(&path, &samples).map_err(|e| e.to_string())
}

/// Get current VAD settings.
#[tauri::command]
pub fn get_vad_settings(state: State<'_, AppState>) -> Result<VadSettings, String> {
    state.vad_settings.lock().map(|s| s.clone()).map_err(|e| e.to_string())
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
    let hotwords_file = state.hotwords_file_path.lock().map(|h| h.clone()).unwrap_or(None);
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    std::thread::spawn(move || {
        log::info!("[apply_settings] rebuilding models with new settings...");
        let active = active_model_arc.lock().map(|a| a.clone()).unwrap_or_default();
        let preferred = if active.is_empty() { None } else { Some(active.as_str()) };
        match crate::build_models(&model_path, &new_settings, preferred, hotwords_file) {
            Ok((rec, vad, threads, model_name)) => {
                log::info!("[apply_settings] models rebuilt, num_threads={threads}, active_model={model_name}");
                let r_ok = recognizer_arc.lock().map(|mut r| { *r = Some(rec); }).is_ok();
                let v_ok = vad_arc.lock().map(|mut v| { *v = Some(vad); }).is_ok();
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
mod commands;
mod config;
mod engine;
mod errors;
mod models;

#[cfg(test)]
mod tests;

use config::app_config::AppConfig;
use engine::recognizer_factory::RecognizerFactory;
use engine::transcriber::Transcriber;
use engine::transcription_pipeline::{SegmentResult, VadSettings};
use sherpa_onnx::OfflineRecognizer;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use tauri::webview::PageLoadEvent;
use tauri::{Emitter, WindowEvent};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub transcriber: Mutex<Transcriber>,

    // Streaming transcription state
    pub recognizer: Arc<Mutex<Option<OfflineRecognizer>>>,
    pub vad_detector: Arc<Mutex<Option<sherpa_onnx::VoiceActivityDetector>>>,
    pub running: Arc<AtomicBool>,
    pub cancelled: Arc<AtomicBool>,
    pub progress: Arc<AtomicU32>,
    pub status: Arc<Mutex<String>>, // "idle" | "processing" | "done" | "cancelled" | "error:..."
    pub segments: Arc<Mutex<Vec<SegmentResult>>>,
    pub audio_path: Arc<Mutex<String>>,
    pub init_status: Arc<AtomicU8>, // 0=pending, 1=ready, 2=error
    pub init_error: Arc<Mutex<String>>,
    pub num_threads: Arc<AtomicU32>,
    pub vad_settings: Arc<Mutex<VadSettings>>,
    pub elapsed_secs: Arc<Mutex<f32>>,
    pub audio_duration_secs: Arc<Mutex<f32>>,
}

/// Build the ASR recognizer and Silero VAD from the configured model path.
/// Returns (recognizer, vad, num_threads) on success.
fn build_models(
    model_path: &str,
    settings: &VadSettings,
) -> Result<(OfflineRecognizer, sherpa_onnx::VoiceActivityDetector, u32), String> {
    // Find available model
    let available = RecognizerFactory::list_available(model_path);

    // Prefer SenseVoice-Small, fallback to Paraformer-Large
    let model_type = if available.contains(&"sense-voice-small".to_string()) {
        engine::recognizer_factory::ModelType::SenseVoice
    } else if available.contains(&"paraformer-large".to_string()) {
        engine::recognizer_factory::ModelType::Paraformer
    } else {
        return Err(format!(
            "No model found in {model_path}. Available: {available:?}"
        ));
    };

    let model_dir = Path::new(model_path).join(model_type.dir_name());
    let model_dir_str = model_dir.to_string_lossy().to_string();

    let config = engine::recognizer_factory::RecognizerConfig {
        model_dir: model_dir_str,
        num_threads: settings.num_threads as u32,
        hotwords_file: None,
        hotwords_score: 1.5,
        use_itn: true,
    };

    log::info!(
        "[build_models] model_type={}, model_dir={:?}, num_threads={}",
        model_type.display_name(),
        model_dir,
        settings.num_threads
    );

    let recognizer = engine::recognizer_factory::RecognizerFactory::create(&model_type, &config)
        .map_err(|e| e.to_string())?;
    log::info!("[build_models] recognizer created");

    // Create Silero VAD
    let vad_model_path = Path::new(model_path)
        .join("silero-vad")
        .join("model.onnx");
    let vad_model_str = vad_model_path.to_string_lossy().to_string();

    if !vad_model_path.exists() {
        return Err(format!("VAD model not found at {vad_model_str}"));
    }

    // Create VAD with custom settings
    let vad = create_silero_vad_with_settings(&vad_model_str, settings)?;
    log::info!("[build_models] VAD created");

    Ok((recognizer, vad, settings.num_threads as u32))
}

/// Create Silero VAD with custom settings.
fn create_silero_vad_with_settings(
    model_path: &str,
    settings: &VadSettings,
) -> Result<sherpa_onnx::VoiceActivityDetector, String> {
    use sherpa_onnx::{SileroVadModelConfig, VadModelConfig};

    let config = VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(model_path.to_string()),
            threshold: settings.threshold,
            min_silence_duration: settings.min_silence_duration,
            min_speech_duration: settings.min_speech_duration,
            window_size: 512,
            max_speech_duration: settings.max_speech_duration,
            ..Default::default()
        },
        sample_rate: 16000,
        num_threads: 1,
        ..Default::default()
    };

    sherpa_onnx::VoiceActivityDetector::create(&config, 120.0)
        .ok_or_else(|| format!("Failed to create VAD from {model_path}"))
}

fn external_navigation_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("external-navigation")
        .on_navigation(|webview, url| {
            let is_internal_host = matches!(
                url.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("tauri.localhost") | Some("::1")
            );

            let is_internal = url.scheme() == "tauri" || is_internal_host;

            if is_internal {
                return true;
            }

            let is_external_link = matches!(url.scheme(), "http" | "https" | "mailto" | "tel");

            if is_external_link {
                log::info!("opening external link in system browser: {}", url);
                let _ = webview.opener().open_url(url.as_str(), None::<&str>);
                return false;
            }

            true
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Read initial config to get model path for background init
    let initial_config = AppConfig::default();

    // Build streaming state
    let recognizer = Arc::new(Mutex::new(None::<OfflineRecognizer>));
    let vad_detector = Arc::new(Mutex::new(None::<sherpa_onnx::VoiceActivityDetector>));
    let init_status = Arc::new(AtomicU8::new(0)); // 0 = pending
    let init_error = Arc::new(Mutex::new(String::new()));
    let num_threads = Arc::new(AtomicU32::new(0));
    let vad_settings = Arc::new(Mutex::new(VadSettings::default()));

    // Clone Arc handles for the init thread
    let init_recognizer = Arc::clone(&recognizer);
    let init_vad = Arc::clone(&vad_detector);
    let init_status_clone = Arc::clone(&init_status);
    let init_error_clone = Arc::clone(&init_error);
    let init_num_threads = Arc::clone(&num_threads);
    let init_vad_settings = Arc::clone(&vad_settings);
    let init_model_path = initial_config.model_path.clone();

    // Background thread: load models
    std::thread::spawn(move || {
        log::info!("[init] starting model initialization...");
        let settings = init_vad_settings.lock().unwrap().clone();
        match build_models(&init_model_path, &settings) {
            Ok((rec, vad, threads)) => {
                log::info!("[init] models ready, num_threads={threads}");
                let r_ok = init_recognizer.lock().map(|mut r| {
                    *r = Some(rec);
                }).is_ok();
                let v_ok = init_vad.lock().map(|mut v| {
                    *v = Some(vad);
                }).is_ok();
                if r_ok && v_ok {
                    init_num_threads.store(threads, Ordering::Relaxed);
                    init_status_clone.store(1, Ordering::Relaxed); // 1 = ready
                } else {
                    log::error!("[init] mutex poisoned, marking as error");
                    if let Ok(mut err) = init_error_clone.lock() {
                        *err = "Internal error: mutex poisoned".to_string();
                    }
                    init_status_clone.store(2, Ordering::Relaxed); // 2 = error
                }
            }
            Err(e) => {
                log::error!("[init] model initialization failed: {e}");
                if let Ok(mut err) = init_error_clone.lock() {
                    *err = e;
                }
                init_status_clone.store(2, Ordering::Relaxed);
            }
        }
    });

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(external_navigation_plugin())
        .manage(AppState {
            config: Mutex::new(initial_config),
            transcriber: Mutex::new(Transcriber::new()),

            // Streaming state
            recognizer,
            vad_detector,
            running: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicU32::new(0)),
            status: Arc::new(Mutex::new("idle".to_string())),
            segments: Arc::new(Mutex::new(Vec::new())),
            audio_path: Arc::new(Mutex::new(String::new())),
            init_status,
            init_error,
            num_threads,
            vad_settings,
            elapsed_secs: Arc::new(Mutex::new(0.0)),
            audio_duration_secs: Arc::new(Mutex::new(0.0)),
        })
        .invoke_handler(tauri::generate_handler![
            // 转录命令 (旧)
            commands::transcribe::transcribe_file,
            commands::transcribe::transcribe_batch,
            commands::transcribe::export_result,
            commands::transcribe::export_to_file,
            commands::transcribe::open_file_with_system,
            commands::transcribe::check_ffmpeg,
            commands::transcribe::check_file_format,
            commands::transcribe::get_model_types,
            // 转录命令 (新 — 流式)
            commands::transcribe::recognize_file,
            commands::transcribe::get_recognition_progress,
            commands::transcribe::cancel_recognition,
            commands::transcribe::save_segment_as_wav,
            commands::transcribe::get_vad_settings,
            commands::transcribe::apply_vad_settings,
            // App init
            commands::transcribe::get_init_status,
            // 模型命令
            commands::model::list_models,
            commands::model::get_model_path,
            commands::model::download_model,
            // commands::model::download_paraformer_large, // not yet implemented
            // 配置命令
            commands::config::get_app_config,
            commands::config::set_app_config,
            commands::config::get_languages,
            commands::config::download_ffmpeg,
            // 历史命令
            commands::history::get_history,
            commands::history::save_history,
            commands::history::delete_history,
            commands::history::clear_history,
        ])
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                log::info!("VelociText main webview loaded");
                let _ = webview.window().show();
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(taura_drop_event) = event {
                use tauri::DragDropEvent;
                match taura_drop_event {
                    DragDropEvent::Drop { paths, .. } => {
                        let path_list: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
                        let _ = window.emit("tauri://file-drop", serde_json::to_string(&path_list).unwrap_or_default());
                        log::info!("Files dropped: {:?}", path_list);
                    }
                    DragDropEvent::Enter { .. } => {
                        let _ = window.emit("tauri://file-drop-hover", true);
                    }
                    DragDropEvent::Leave => {
                        let _ = window.emit("tauri://file-drop-hover", false);
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
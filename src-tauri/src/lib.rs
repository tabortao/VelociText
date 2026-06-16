mod commands;
mod config;
mod engine;
mod errors;
mod models;

#[cfg(test)]
mod tests;

use config::app_config::AppConfig;
use config::dictionary_config::DictionaryConfig;
use engine::ocr::OcrEngine;
use engine::recognizer_factory::RecognizerFactory;
use engine::transcriber::Transcriber;
use engine::transcription_pipeline::{SegmentResult, VadSettings};
use sherpa_onnx::OfflineRecognizer;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8};
use std::sync::{Arc, Mutex};
use tauri::webview::PageLoadEvent;
use tauri::{Emitter, Manager, WindowEvent};
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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
    pub active_model: Arc<Mutex<String>>, // "sense-voice-small" | "paraformer" | "qwen3-asr"
    pub dictionary_config: Arc<Mutex<DictionaryConfig>>,
    pub hotwords_file_path: Arc<Mutex<Option<String>>>,
    pub active_ocr_model: Arc<Mutex<String>>, // "ppocr-v4" | "ppocr-v5" | "ppocr-v6"
    /// OCR engine instance (session reuse for performance).
    /// References snow-shot's OcrService pattern.
    pub ocr_engine: Arc<Mutex<Option<OcrEngine>>>,
    /// Pending screenshot data for the screenshot selection window.
    /// Stored by `start_screenshot_selection`, retrieved by `get_screenshot_data`.
    pub pending_screenshot: Arc<Mutex<Option<serde_json::Value>>>,
}

/// Build the ASR recognizer and Silero VAD from the configured model path.
/// Returns (recognizer, vad, num_threads, model_dir_name) on success.
fn build_models(
    model_path: &str,
    settings: &VadSettings,
    preferred_model: Option<&str>,
    hotwords_file: Option<String>,
) -> Result<
    (
        OfflineRecognizer,
        sherpa_onnx::VoiceActivityDetector,
        u32,
        String,
    ),
    String,
> {
    // Find available model
    let available = RecognizerFactory::list_available(model_path);

    // Determine model type: use preferred if available, otherwise auto-detect
    let model_type = if let Some(preferred) = preferred_model {
        if available.contains(&preferred.to_string()) {
            match preferred {
                "sense-voice-small" => engine::recognizer_factory::ModelType::SenseVoice,
                "paraformer" => engine::recognizer_factory::ModelType::Paraformer,
                "qwen3-asr" => engine::recognizer_factory::ModelType::Qwen3Asr,
                _ => {
                    log::warn!(
                        "[build_models] unknown preferred model: {preferred}, auto-detecting"
                    );
                    return build_models(model_path, settings, None, hotwords_file);
                }
            }
        } else {
            log::warn!("[build_models] preferred model {preferred} not available, auto-detecting");
            return build_models(model_path, settings, None, hotwords_file);
        }
    } else if available.contains(&"sense-voice-small".to_string()) {
        engine::recognizer_factory::ModelType::SenseVoice
    } else if available.contains(&"paraformer".to_string()) {
        engine::recognizer_factory::ModelType::Paraformer
    } else if available.contains(&"qwen3-asr".to_string()) {
        engine::recognizer_factory::ModelType::Qwen3Asr
    } else {
        return Err(format!(
            "No model found in {model_path}. Available: {available:?}"
        ));
    };

    let model_dir = Path::new(model_path).join(model_type.dir_name());
    let model_dir_str = model_dir.to_string_lossy().to_string();

    // Auto-fix: if Paraformer tokens.txt is in JSON format, convert it
    if matches!(
        model_type,
        engine::recognizer_factory::ModelType::Paraformer
    ) {
        let tokens_path = model_dir.join("tokens.txt");
        if tokens_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&tokens_path) {
                if content.trim_start().starts_with('[') {
                    log::warn!(
                        "[build_models] Paraformer tokens.txt is in JSON format, converting..."
                    );
                    if let Ok(tokens) = serde_json::from_str::<Vec<String>>(&content) {
                        let mut file = std::fs::File::create(&tokens_path).unwrap();
                        for (i, token) in tokens.iter().enumerate() {
                            let _ = writeln!(file, "{} {}", token, i);
                        }
                        log::info!(
                            "[build_models] tokens.txt converted with {} tokens",
                            tokens.len()
                        );
                    }
                }
            }
        }
    }

    // Adjust VAD settings based on model type
    let effective_settings = match model_type {
        engine::recognizer_factory::ModelType::Paraformer => {
            // Paraformer handles longer utterances better
            let mut s = settings.clone();
            if s.max_speech_duration < 30.0 {
                s.max_speech_duration = 30.0;
            }
            s
        }
        _ => settings.clone(),
    };

    let config = engine::recognizer_factory::RecognizerConfig {
        model_dir: model_dir_str.clone(),
        num_threads: effective_settings.num_threads as u32,
        hotwords_file: hotwords_file.clone(),
        hotwords_score: 1.5,
        use_itn: true,
    };

    log::info!(
        "[build_models] model_type={}, model_dir={:?}, num_threads={}",
        model_type.display_name(),
        model_dir,
        effective_settings.num_threads
    );

    let (recognizer, actual_dir_name) =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine::recognizer_factory::RecognizerFactory::create(&model_type, &config)
        })) {
            Ok(Ok(r)) => {
                let name = if config.model_dir.contains("paraformer") {
                    "paraformer"
                } else if config.model_dir.contains("qwen3-asr") {
                    "qwen3-asr"
                } else {
                    "sense-voice-small"
                };
                (r, name.to_string())
            }
            Ok(Err(e)) => {
                log::error!(
                    "[build_models] failed to create {} recognizer: {e}",
                    model_type.display_name()
                );
                // Try to fall back to another available model
                let fallback_type = match model_type {
                    engine::recognizer_factory::ModelType::Paraformer => {
                        if available.contains(&"sense-voice-small".to_string()) {
                            log::warn!("[build_models] falling back to SenseVoice-Small");
                            Some(engine::recognizer_factory::ModelType::SenseVoice)
                        } else if available.contains(&"qwen3-asr".to_string()) {
                            log::warn!("[build_models] falling back to Qwen3-ASR");
                            Some(engine::recognizer_factory::ModelType::Qwen3Asr)
                        } else {
                            None
                        }
                    }
                    engine::recognizer_factory::ModelType::Qwen3Asr => {
                        if available.contains(&"sense-voice-small".to_string()) {
                            log::warn!("[build_models] falling back to SenseVoice-Small");
                            Some(engine::recognizer_factory::ModelType::SenseVoice)
                        } else if available.contains(&"paraformer".to_string()) {
                            log::warn!("[build_models] falling back to Paraformer");
                            Some(engine::recognizer_factory::ModelType::Paraformer)
                        } else {
                            None
                        }
                    }
                    engine::recognizer_factory::ModelType::SenseVoice
                        if available.contains(&"paraformer".to_string()) =>
                    {
                        log::warn!("[build_models] falling back to Paraformer");
                        Some(engine::recognizer_factory::ModelType::Paraformer)
                    }
                    engine::recognizer_factory::ModelType::SenseVoice
                        if available.contains(&"qwen3-asr".to_string()) =>
                    {
                        log::warn!("[build_models] falling back to Qwen3-ASR");
                        Some(engine::recognizer_factory::ModelType::Qwen3Asr)
                    }
                    _ => None,
                };
                match fallback_type {
                    Some(ft) => {
                        let fb_dir = Path::new(model_path).join(ft.dir_name());
                        // Don't pass hotwords to fallback — avoid cascading failure
                        let fb_config = engine::recognizer_factory::RecognizerConfig {
                            model_dir: fb_dir.to_string_lossy().to_string(),
                            num_threads: settings.num_threads as u32,
                            hotwords_file: None,
                            hotwords_score: 1.5,
                            use_itn: true,
                        };
                        let fb_name = ft.dir_name().to_string();
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            engine::recognizer_factory::RecognizerFactory::create(&ft, &fb_config)
                        })) {
                            Ok(Ok(r)) => (r, fb_name),
                            Ok(Err(fe)) => return Err(format!("Fallback also failed: {fe}")),
                            Err(_) => {
                                return Err(format!("Fallback model panicked during creation"))
                            }
                        }
                    }
                    None => {
                        return Err(format!(
                            "Failed to create {} recognizer: {e}",
                            model_type.display_name()
                        ))
                    }
                }
            }
            Err(panic_info) => {
                log::error!(
                    "[build_models] {} recognizer creation panicked: {:?}",
                    model_type.display_name(),
                    panic_info
                );
                // Try to fall back to another available model
                let fallback_type = match model_type {
                    engine::recognizer_factory::ModelType::Paraformer => {
                        if available.contains(&"sense-voice-small".to_string()) {
                            Some(engine::recognizer_factory::ModelType::SenseVoice)
                        } else if available.contains(&"qwen3-asr".to_string()) {
                            Some(engine::recognizer_factory::ModelType::Qwen3Asr)
                        } else {
                            None
                        }
                    }
                    engine::recognizer_factory::ModelType::Qwen3Asr => {
                        if available.contains(&"sense-voice-small".to_string()) {
                            Some(engine::recognizer_factory::ModelType::SenseVoice)
                        } else if available.contains(&"paraformer".to_string()) {
                            Some(engine::recognizer_factory::ModelType::Paraformer)
                        } else {
                            None
                        }
                    }
                    engine::recognizer_factory::ModelType::SenseVoice
                        if available.contains(&"paraformer".to_string()) =>
                    {
                        Some(engine::recognizer_factory::ModelType::Paraformer)
                    }
                    engine::recognizer_factory::ModelType::SenseVoice
                        if available.contains(&"qwen3-asr".to_string()) =>
                    {
                        Some(engine::recognizer_factory::ModelType::Qwen3Asr)
                    }
                    _ => None,
                };
                match fallback_type {
                    Some(ft) => {
                        let fb_dir = Path::new(model_path).join(ft.dir_name());
                        // Don't pass hotwords to fallback — avoid cascading failure
                        let fb_config = engine::recognizer_factory::RecognizerConfig {
                            model_dir: fb_dir.to_string_lossy().to_string(),
                            num_threads: settings.num_threads as u32,
                            hotwords_file: None,
                            hotwords_score: 1.5,
                            use_itn: true,
                        };
                        let fb_name = ft.dir_name().to_string();
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            engine::recognizer_factory::RecognizerFactory::create(&ft, &fb_config)
                        })) {
                            Ok(Ok(r)) => {
                                log::warn!(
                                    "[build_models] recovered from panic, using fallback model {}",
                                    fb_name
                                );
                                (r, fb_name)
                            }
                            Ok(Err(fe)) => {
                                return Err(format!("Fallback after panic also failed: {fe}"))
                            }
                            Err(_) => return Err("Fallback model also panicked".into()),
                        }
                    }
                    None => {
                        return Err(format!(
                            "{} model creation panicked and no fallback available",
                            model_type.display_name()
                        ))
                    }
                }
            }
        };

    log::info!("[build_models] recognizer created, actual_model={actual_dir_name}");

    // Create Silero VAD
    let vad_model_path = Path::new(model_path).join("silero-vad").join("model.onnx");
    let vad_model_str = vad_model_path.to_string_lossy().to_string();

    if !vad_model_path.exists() {
        return Err(format!("VAD model not found at {vad_model_str}"));
    }

    // Create VAD with effective settings (may be adjusted for Paraformer)
    let vad = create_silero_vad_with_settings(&vad_model_str, &effective_settings)?;
    log::info!("[build_models] VAD created");

    Ok((
        recognizer,
        vad,
        effective_settings.num_threads as u32,
        actual_dir_name.to_string(),
    ))
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

/// Parse a shortcut string like "Ctrl+Shift+O" into (Modifiers, Code).
fn parse_shortcut_string(s: &str) -> (Modifiers, Code) {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    let mut modifiers = Modifiers::empty();

    for &part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            "super" | "cmd" | "command" | "win" => modifiers |= Modifiers::SUPER,
            _ => {}
        }
    }

    let key = parts.last().map(|p| p.to_uppercase()).unwrap_or_default();
    let code = match key.as_str() {
        "A" => Code::KeyA, "B" => Code::KeyB, "C" => Code::KeyC, "D" => Code::KeyD,
        "E" => Code::KeyE, "F" => Code::KeyF, "G" => Code::KeyG, "H" => Code::KeyH,
        "I" => Code::KeyI, "J" => Code::KeyJ, "K" => Code::KeyK, "L" => Code::KeyL,
        "M" => Code::KeyM, "N" => Code::KeyN, "O" => Code::KeyO, "P" => Code::KeyP,
        "Q" => Code::KeyQ, "R" => Code::KeyR, "S" => Code::KeyS, "T" => Code::KeyT,
        "U" => Code::KeyU, "V" => Code::KeyV, "W" => Code::KeyW, "X" => Code::KeyX,
        "Y" => Code::KeyY, "Z" => Code::KeyZ,
        "0" => Code::Digit0, "1" => Code::Digit1, "2" => Code::Digit2,
        "3" => Code::Digit3, "4" => Code::Digit4, "5" => Code::Digit5,
        "6" => Code::Digit6, "7" => Code::Digit7, "8" => Code::Digit8,
        "9" => Code::Digit9,
        "F1" => Code::F1, "F2" => Code::F2, "F3" => Code::F3, "F4" => Code::F4,
        "F5" => Code::F5, "F6" => Code::F6, "F7" => Code::F7, "F8" => Code::F8,
        "F9" => Code::F9, "F10" => Code::F10, "F11" => Code::F11, "F12" => Code::F12,
        "SPACE" => Code::Space,
        "TAB" => Code::Tab,
        "ENTER" | "RETURN" => Code::Enter,
        "ESCAPE" | "ESC" => Code::Escape,
        "BACKSPACE" => Code::Backspace,
        _ => Code::KeyO, // fallback
    };

    (modifiers, code)
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
    let mut initial_config = AppConfig::load();

    // Crash recovery: if a `.loading` marker file exists, the previous model load crashed the app.
    // Fall back to the other available model.
    let crash_marker_path =
        std::path::PathBuf::from(&initial_config.model_path).join(".model_loading");
    if crash_marker_path.exists() {
        if let Ok(crashed_model) = std::fs::read_to_string(&crash_marker_path) {
            log::warn!(
                "[init] crash detected: model '{}' caused crash on last load, falling back",
                crashed_model
            );
            // Try fallback models in priority order (SenseVoice is most reliable)
            let fallbacks = match crashed_model.as_str() {
                "paraformer" => vec!["sense-voice-small", "qwen3-asr"],
                "qwen3-asr" => vec!["sense-voice-small", "paraformer"],
                _ => vec!["sense-voice-small", "paraformer", "qwen3-asr"],
            };
            let mut switched = false;
            for fallback in &fallbacks {
                let fallback_dir = std::path::Path::new(&initial_config.model_path).join(fallback);
                let available = if *fallback == "paraformer" {
                    engine::model_manager::is_paraformer_installed_at(&fallback_dir)
                } else if *fallback == "qwen3-asr" {
                    engine::model_manager::is_qwen3_asr_installed_at(&fallback_dir)
                } else {
                    engine::model_manager::is_model_installed_at(&fallback_dir)
                };
                if available {
                    initial_config.active_model = fallback.to_string();
                    let _ = AppConfig::save(&initial_config);
                    log::info!("[init] switched to fallback model: {fallback}");
                    switched = true;
                    break;
                }
            }
            if !switched {
                log::warn!("[init] no fallback model available, will try original model anyway");
            }
        }
        let _ = std::fs::remove_file(&crash_marker_path);
    }

    // Build streaming state
    let dictionary_config = Arc::new(Mutex::new(DictionaryConfig::load()));
    let hotwords_file_path: Arc<Mutex<Option<String>>> = {
        let dc = dictionary_config.lock().unwrap();
        if dc.hotwords.is_empty() {
            Arc::new(Mutex::new(None))
        } else {
            match dc.generate_hotwords_file() {
                Ok(path) => Arc::new(Mutex::new(Some(path))),
                Err(e) => {
                    log::warn!("[init] failed to generate hotwords file: {e}");
                    Arc::new(Mutex::new(None))
                }
            }
        }
    };
    let recognizer = Arc::new(Mutex::new(None::<OfflineRecognizer>));
    let vad_detector = Arc::new(Mutex::new(None::<sherpa_onnx::VoiceActivityDetector>));
    let init_status = Arc::new(AtomicU8::new(3)); // 3 = released (lazy loading, not loaded yet)
    let init_error = Arc::new(Mutex::new(String::new()));
    let num_threads = Arc::new(AtomicU32::new(0));
    let vad_settings = Arc::new(Mutex::new(VadSettings::default()));
    let active_model: Arc<Mutex<String>> =
        Arc::new(Mutex::new(initial_config.active_model.clone()));

    // ASR models are now lazy-loaded via `ensure_asr_models` command
    // when the user navigates to the Transcribe page.
    // They are released after 5 minutes of inactivity via `release_asr_models`.
    // This reduces startup memory usage from ~500MB to ~50MB.

    let active_ocr_model = initial_config.active_ocr_model.clone();
    // Extract shortcut before initial_config is moved into AppState
    let screenshot_shortcut = if initial_config.ocr_screenshot_shortcut.is_empty() {
        "Ctrl+Shift+O".to_string()
    } else {
        initial_config.ocr_screenshot_shortcut.clone()
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // When a second instance is launched, show and focus the main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
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
            active_model,
            dictionary_config,
            hotwords_file_path,
            active_ocr_model: Arc::new(Mutex::new(active_ocr_model)),
            ocr_engine: Arc::new(Mutex::new(None)),
            pending_screenshot: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            // 转录命令 (旧)
            commands::transcribe::transcribe_file,
            commands::transcribe::transcribe_batch,
            commands::transcribe::export_result,
            commands::transcribe::export_to_file,
            commands::transcribe::write_text_file,
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
            commands::transcribe::ensure_asr_models,
            commands::transcribe::release_asr_models,
            // 模型命令
            commands::model::list_models,
            commands::model::get_model_path,
            commands::model::download_model,
            commands::model::download_specific_model,
            commands::model::get_active_model,
            commands::model::set_active_model,
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
            // 词典命令
            commands::dictionary::get_dictionary_config,
            commands::dictionary::save_hotwords,
            commands::dictionary::save_replacements,
            commands::dictionary::get_hotwords_file_path,
            // OCR 命令
            commands::ocr::ocr_recognize,
            commands::ocr::ocr_recognize_bytes,
            commands::ocr::capture_all_monitors,
            commands::ocr::ocr_screenshot_region,
            commands::ocr::start_screenshot_selection,
            commands::ocr::get_screenshot_data,
            commands::ocr::close_screenshot_window,
            commands::ocr::screenshot_ocr_done,
            commands::ocr::copy_text_to_clipboard,
            commands::ocr::ocr_get_active_model,
            commands::ocr::ocr_set_active_model,
            commands::ocr::ocr_release,
            commands::ocr::pdf_get_page_count,
            commands::ocr::pdf_render_page,
            commands::ocr::ocr_recognize_pdf,
        ])
        .setup(move |app| {
            // System tray — references snow-shot's tray implementation
            let show = MenuItemBuilder::with_id("show", "显示 VelociText")
                .build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出")
                .build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show, &quit])
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("VelociText")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Register global shortcut for screenshot OCR (Rust-side, more reliable than JS)
            {
                let app_handle = app.handle().clone();

                // Parse shortcut string (e.g. "Ctrl+Shift+O") into Modifiers + Code
                let (modifiers, code) = parse_shortcut_string(&screenshot_shortcut);

                let shortcut = Shortcut::new(Some(modifiers), code);

                // Register with handler
                app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        log::info!("[global-shortcut] screenshot OCR shortcut pressed");
                        // Trigger screenshot via invoke
                        let app = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            // Only show window if it's currently visible (not minimized to tray)
                            // When minimized to tray, do screenshot silently without showing the window
                            if let Some(window) = app.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.set_focus();
                                }
                            }
                            // Emit event to frontend to trigger screenshot
                            let _ = app.emit("trigger-screenshot-ocr", ());
                        });
                    }
                })?;

                log::info!("[global-shortcut] registered shortcut: {}", screenshot_shortcut);
            }

            Ok(())
        })
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                log::info!("VelociText main webview loaded");
                let _ = webview.window().show();
            }
        })
        .on_window_event(|window, event| {
            // Close to tray instead of quitting (only for main window)
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                    return;
                }
                // Screenshot window and other windows close normally
            }
            if let WindowEvent::DragDrop(taura_drop_event) = event {
                use tauri::DragDropEvent;
                match taura_drop_event {
                    DragDropEvent::Drop { paths, .. } => {
                        let path_list: Vec<String> = paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();
                        let _ = window.emit(
                            "tauri://file-drop",
                            serde_json::to_string(&path_list).unwrap_or_default(),
                        );
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

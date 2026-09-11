mod commands;
mod config;
mod engine;
mod errors;
mod models;

#[cfg(test)]
mod tests;

use config::app_config::AppConfig;
use config::dictionary_config::DictionaryConfig;
use engine::recognizer_factory::RecognizerFactory;
use engine::transcription_pipeline::{SegmentResult, VadSettings};
use sherpa_onnx::OfflineRecognizer;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicU64};
use std::sync::{Arc, Mutex};
use tauri::webview::PageLoadEvent;
use tauri::{Emitter, Manager, WindowEvent};
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub config: Mutex<AppConfig>,

    // Streaming transcription state
    pub recognizer: Arc<Mutex<Option<OfflineRecognizer>>>,
    pub vad_detector: Arc<Mutex<Option<sherpa_onnx::VoiceActivityDetector>>>,
    pub running: Arc<AtomicBool>,
    pub cancelled: Arc<AtomicBool>,
    pub progress: Arc<AtomicU32>,
    pub status: Arc<Mutex<String>>, // "idle" | "processing" | "done" | "cancelled" | "error:..."
    pub segments: Arc<Mutex<Vec<SegmentResult>>>,
    pub audio_path: Arc<Mutex<String>>,
    pub init_status: Arc<AtomicU8>, // 0=pending, 1=ready, 2=error, 3=released
    pub init_error: Arc<Mutex<String>>,
    pub num_threads: Arc<AtomicU32>,
    pub vad_settings: Arc<Mutex<VadSettings>>,
    pub elapsed_secs: Arc<Mutex<f32>>,
    pub audio_duration_secs: Arc<Mutex<f32>>,
    pub active_model: Arc<Mutex<String>>, // "sense-voice-small" | "paraformer" | "qwen3-asr" | "qwen3-asr-1.7b"
    pub dictionary_config: Arc<Mutex<DictionaryConfig>>,
    pub hotwords_file_path: Arc<Mutex<Option<String>>>,
    /// Token used to cancel a pending deferred model release.
    /// Bumped by `ensure_asr_models` (models needed again) and by each new
    /// `release_asr_models` request, so only the latest scheduled release fires.
    pub release_token: Arc<AtomicU64>,

    // Audio conversion state (video → audio via FFmpeg)
    pub audio_convert_cancel: Arc<AtomicBool>,
    pub audio_convert_child: Arc<Mutex<Option<std::process::Child>>>,
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
            match engine::recognizer_factory::ModelType::from_dir_name(preferred) {
                Some(mt) => mt,
                None => {
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
    } else if available.contains(&"qwen3-asr-1.7b".to_string()) {
        engine::recognizer_factory::ModelType::Qwen3Asr1_7b
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
                        match std::fs::File::create(&tokens_path) {
                            Ok(mut file) => {
                                for (i, token) in tokens.iter().enumerate() {
                                    let _ = writeln!(file, "{} {}", token, i);
                                }
                                log::info!(
                                    "[build_models] tokens.txt converted with {} tokens",
                                    tokens.len()
                                );
                            }
                            Err(e) => {
                                log::error!(
                                    "[build_models] Failed to create tokens.txt: {e}"
                                );
                            }
                        }
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
            Ok(Ok(r)) => (r, model_type.dir_name().to_string()),
            Ok(Err(e)) => {
                log::error!(
                    "[build_models] failed to create {} recognizer: {e}",
                    model_type.display_name()
                );
                match create_fallback_recognizer(model_path, settings, &model_type, &available) {
                    Ok(res) => res,
                    Err(fe) => {
                        return Err(format!(
                            "Failed to create {} recognizer: {e} ({fe})",
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
                match create_fallback_recognizer(model_path, settings, &model_type, &available) {
                    Ok(res) => {
                        log::warn!(
                            "[build_models] recovered from panic, using fallback model {}",
                            res.1
                        );
                        res
                    }
                    Err(fe) => {
                        return Err(format!(
                            "{} model creation panicked and no fallback available ({fe})",
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

/// Pick a fallback ASR model when the preferred one fails to load.
/// Priority: SenseVoice (most reliable) → Paraformer → Qwen3-ASR (1.7B) → Qwen3-ASR (0.6B).
fn pick_fallback_model(
    failed: &engine::recognizer_factory::ModelType,
    available: &[String],
) -> Option<engine::recognizer_factory::ModelType> {
    const FALLBACK_PRIORITY: [&str; 4] = [
        "sense-voice-small",
        "paraformer",
        "qwen3-asr-1.7b",
        "qwen3-asr",
    ];
    FALLBACK_PRIORITY
        .iter()
        .filter(|name| **name != failed.dir_name())
        .find(|name| available.contains(&name.to_string()))
        .and_then(|name| engine::recognizer_factory::ModelType::from_dir_name(name))
}

/// Create a recognizer from the best available fallback model after `failed_type`
/// failed to load (returned Err or panicked).
fn create_fallback_recognizer(
    model_path: &str,
    settings: &VadSettings,
    failed_type: &engine::recognizer_factory::ModelType,
    available: &[String],
) -> Result<(OfflineRecognizer, String), String> {
    let Some(fallback_type) = pick_fallback_model(failed_type, available) else {
        return Err("no fallback model available".to_string());
    };
    log::warn!(
        "[build_models] falling back to {}",
        fallback_type.display_name()
    );

    let fb_dir = Path::new(model_path).join(fallback_type.dir_name());
    // Don't pass hotwords to fallback — avoid cascading failure
    let fb_config = engine::recognizer_factory::RecognizerConfig {
        model_dir: fb_dir.to_string_lossy().to_string(),
        num_threads: settings.num_threads as u32,
        hotwords_file: None,
        hotwords_score: 1.5,
        use_itn: true,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine::recognizer_factory::RecognizerFactory::create(&fallback_type, &fb_config)
    })) {
        Ok(Ok(r)) => Ok((r, fallback_type.dir_name().to_string())),
        Ok(Err(fe)) => Err(format!("fallback {} also failed: {fe}", fallback_type.display_name())),
        Err(_) => Err(format!(
            "fallback {} panicked during creation",
            fallback_type.display_name()
        )),
    }
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
                "paraformer" => vec!["sense-voice-small", "qwen3-asr", "qwen3-asr-1.7b"],
                "qwen3-asr" => vec!["sense-voice-small", "paraformer", "qwen3-asr-1.7b"],
                "qwen3-asr-1.7b" => vec!["sense-voice-small", "paraformer", "qwen3-asr"],
                _ => vec!["sense-voice-small", "paraformer", "qwen3-asr", "qwen3-asr-1.7b"],
            };
            let mut switched = false;
            for fallback in &fallbacks {
                let fallback_dir = std::path::Path::new(&initial_config.model_path).join(fallback);
                let available = if *fallback == "paraformer" {
                    engine::model_manager::is_paraformer_installed_at(&fallback_dir)
                } else if *fallback == "qwen3-asr" || *fallback == "qwen3-asr-1.7b" {
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
    // when the user navigates to the Transcribe / Subtitle page.
    // Leaving those pages schedules a deferred release (`release_asr_models`)
    // after 5 minutes of inactivity — switching between the two pages shares
    // the same loaded models without a reload.
    // This reduces startup memory usage from ~500MB to ~50MB.

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
            release_token: Arc::new(AtomicU64::new(0)),
            audio_convert_cancel: Arc::new(AtomicBool::new(false)),
            audio_convert_child: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            // 转录命令 (流式)
            commands::transcribe::recognize_file,
            commands::transcribe::get_recognition_progress,
            commands::transcribe::cancel_recognition,
            commands::transcribe::save_segment_as_wav,
            commands::transcribe::export_to_file,
            commands::transcribe::check_ffmpeg,
            commands::transcribe::get_vad_settings,
            commands::transcribe::apply_vad_settings,
            // 音频转换命令 (视频 → 音频)
            commands::audio_convert::convert_to_audio,
            commands::audio_convert::cancel_audio_convert,
            commands::audio_convert::get_audio_formats,
            // App init
            commands::transcribe::get_init_status,
            commands::transcribe::ensure_asr_models,
            commands::transcribe::release_asr_models,
            // 模型命令
            commands::model::list_models,
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

            Ok(())
        })
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                log::info!("VelociText main webview loaded");
                let _ = webview.window().show();
            }
        })
        .on_window_event(|window, event| {
            // Respect the user-configured close behavior for the main window:
            // "tray" = hide to system tray (default), "exit" = quit the app.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let behavior = window
                        .app_handle()
                        .try_state::<AppState>()
                        .and_then(|state| {
                            state.config.lock().ok().map(|c| c.close_behavior.clone())
                        })
                        .unwrap_or_else(|| "tray".to_string());

                    if behavior == "exit" {
                        window.app_handle().exit(0);
                        return;
                    }

                    // Default: minimize to tray
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

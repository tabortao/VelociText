use crate::engine::model_manager::{is_model_installed_at, is_paraformer_installed_at, is_qwen3_asr_installed_at, is_silero_vad_installed_at, ModelInfo, ModelManager};
use crate::AppState;
use std::path::Path;
use tauri::Emitter;

/// 列出所有可用模型
#[tauri::command]
pub async fn list_models(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModelInfo>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let manager = ModelManager::new(config.model_path.clone());
    Ok(manager.list_models())
}

/// 获取模型路径
#[tauri::command]
pub async fn get_model_path(
    model_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let manager = ModelManager::new(config.model_path.clone());
    if manager.is_model_installed(&model_name) {
        Ok(Some(manager.get_model_path(&model_name).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

/// 获取当前活跃的 ASR 模型名称
#[tauri::command]
pub async fn get_active_model(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let active = state.active_model.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
}

/// 切换活跃 ASR 模型（保存配置后重启应用）
#[tauri::command]
pub async fn set_active_model(
    model_name: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Validate model name
    if model_name != "sense-voice-small" && model_name != "paraformer" && model_name != "qwen3-asr" {
        return Err(format!("Unknown model: {model_name}"));
    }

    // Check model is installed
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    let model_dir = Path::new(&model_path).join(&model_name);
    let installed = if model_name == "paraformer" {
        is_paraformer_installed_at(&model_dir)
    } else if model_name == "qwen3-asr" {
        is_qwen3_asr_installed_at(&model_dir)
    } else {
        is_model_installed_at(&model_dir)
    };

    if !installed {
        return Err(format!("Model {model_name} is not installed"));
    }

    // Save to config file
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.active_model = model_name.clone();
        crate::config::app_config::AppConfig::save(&config).map_err(|e| e.to_string())?;
    }

    log::info!("[set_active_model] config saved, active_model={model_name}, restarting app...");

    // Restart the application (never returns)
    app_handle.restart();
}

/// 下载模型
///
/// 同时下载 SenseVoice-Small ASR 模型和 Silero VAD 模型。
/// 从 ModelScope 下载，通过事件 `model-download-progress` 推送进度。
#[tauri::command]
pub async fn download_model(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    // Check if both models are already installed
    let sense_voice_dir = Path::new(&model_path).join("sense-voice-small");
    let silero_vad_dir = Path::new(&model_path).join("silero-vad");
    let sense_installed = is_model_installed_at(&sense_voice_dir);
    let silero_installed = is_silero_vad_installed_at(&silero_vad_dir);

    if sense_installed && silero_installed {
        return Ok("All models installed".into());
    }

    let app_handle_clone = app_handle.clone();
    let model_path_clone = model_path.clone();

    // Download in a background thread
    let result = tokio::task::spawn_blocking(move || {
        let manager = ModelManager::new(model_path_clone);

        // Download SenseVoice-Small if not installed
        if !is_model_installed_at(&sense_voice_dir) {
            manager.download_sense_voice_small(&|progress| {
                let _ = app_handle_clone.emit("model-download-progress", progress.clone());
            })?;
        }

        // Download Silero VAD if not installed
        if is_silero_vad_installed_at(&silero_vad_dir) {
            return Ok("All models installed".into());
        }

        manager.download_silero_vad(&|progress| {
            let _ = app_handle_clone.emit("model-download-progress", progress.clone());
        })?;
        Ok("All models installed".into())
    })
    .await
    .map_err(|e| format!("Download task failed: {}", e))?;

    result.map_err(|e: crate::errors::AppError| e.to_string())
}

/// 下载指定模型
#[tauri::command]
pub async fn download_specific_model(
    model_name: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    let app_handle_clone = app_handle.clone();
    let model_path_clone = model_path.clone();

    let result = tokio::task::spawn_blocking(move || {
        let manager = ModelManager::new(model_path_clone);

        match model_name.as_str() {
            "sense-voice-small" => {
                let dir = Path::new(&model_path).join("sense-voice-small");
                if is_model_installed_at(&dir) {
                    return Ok("Model already installed".into());
                }
                manager.download_sense_voice_small(&|progress| {
                    let _ = app_handle_clone.emit("model-download-progress", progress.clone());
                }).map_err(|e| e.to_string())?;
            }
            "paraformer" => {
                let dir = Path::new(&model_path).join("paraformer");
                // Check if properly installed (not just files existing, but correct format)
                let needs_download = if is_paraformer_installed_at(&dir) {
                    // Verify tokens.txt is not in JSON format
                    let tokens_path = dir.join("tokens.txt");
                    if let Ok(content) = std::fs::read_to_string(&tokens_path) {
                        content.trim_start().starts_with('[') // JSON format = broken, needs re-download
                    } else {
                        true
                    }
                } else {
                    true
                };
                if !needs_download {
                    return Ok("Model already installed".into());
                }
                manager.download_paraformer_large(&|progress| {
                    let _ = app_handle_clone.emit("model-download-progress", progress.clone());
                }).map_err(|e| e.to_string())?;
            }
            "silero-vad" => {
                let dir = Path::new(&model_path).join("silero-vad");
                if is_silero_vad_installed_at(&dir) {
                    return Ok("Model already installed".into());
                }
                manager.download_silero_vad(&|progress| {
                    let _ = app_handle_clone.emit("model-download-progress", progress.clone());
                }).map_err(|e| e.to_string())?;
            }
            "qwen3-asr" => {
                let dir = Path::new(&model_path).join("qwen3-asr");
                if is_qwen3_asr_installed_at(&dir) {
                    return Ok("Model already installed".into());
                }
                manager.download_qwen3_asr(&|progress| {
                    let _ = app_handle_clone.emit("model-download-progress", progress.clone());
                }).map_err(|e| e.to_string())?;
            }
            _ => return Err(format!("Unknown model: {model_name}")),
        }

        Ok(format!("Model {model_name} downloaded"))
    })
    .await
    .map_err(|e| format!("Download task failed: {}", e))?;

    result
}
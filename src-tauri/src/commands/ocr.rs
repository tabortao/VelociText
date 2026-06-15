use crate::engine::ocr::{OcrEngine, types::OcrResult};
use crate::engine::model_manager::is_ppocr_installed_at;
use crate::AppState;
use std::path::Path;

/// Run OCR on an image file using the specified model version.
///
/// Uses session reuse: the ONNX session is kept alive in AppState between calls
/// (references snow-shot's OcrService pattern). On first call or model switch,
/// a new engine is created and cached. Subsequent calls reuse the cached engine
/// for ~10x faster initialization.
#[tauri::command]
pub async fn ocr_recognize(
    image_path: String,
    model_version: String,
    state: tauri::State<'_, AppState>,
) -> Result<OcrResult, String> {
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    let model_dir = Path::new(&model_path).join(&model_version);

    if !is_ppocr_installed_at(&model_dir) {
        return Err(format!(
            "OCR model {} is not installed. Please download it from Model Settings.",
            model_version
        ));
    }

    let engine_arc = state.ocr_engine.clone();

    tokio::task::spawn_blocking(move || {
        let mut guard = engine_arc.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut eng) = *guard {
            // Reuse existing engine — fast path
            eng.recognize_from_path(Path::new(&image_path)).map_err(|e| e.to_string())
        } else {
            // First call: create and cache engine
            let mut eng = OcrEngine::new_with_memory(&model_dir, false)
                .map_err(|e| e.to_string())?;
            let result = eng.recognize_from_path(Path::new(&image_path)).map_err(|e| e.to_string());
            *guard = Some(eng);
            result
        }
    })
    .await
    .map_err(|e| format!("OCR task failed: {e}"))?
}

/// Run OCR on raw image bytes (PNG/JPEG/etc.) using the specified model version.
#[tauri::command]
pub async fn ocr_recognize_bytes(
    image_data: Vec<u8>,
    model_version: String,
    state: tauri::State<'_, AppState>,
) -> Result<OcrResult, String> {
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    let model_dir = Path::new(&model_path).join(&model_version);

    if !is_ppocr_installed_at(&model_dir) {
        return Err(format!(
            "OCR model {} is not installed. Please download it from Model Settings.",
            model_version
        ));
    }

    let engine_arc = state.ocr_engine.clone();

    tokio::task::spawn_blocking(move || {
        let mut guard = engine_arc.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut eng) = *guard {
            eng.recognize_from_bytes(&image_data).map_err(|e| e.to_string())
        } else {
            let mut eng = OcrEngine::new_with_memory(&model_dir, false)
                .map_err(|e| e.to_string())?;
            let result = eng.recognize_from_bytes(&image_data).map_err(|e| e.to_string());
            *guard = Some(eng);
            result
        }
    })
    .await
    .map_err(|e| format!("OCR task failed: {e}"))?
}

/// Get the currently active OCR model version.
#[tauri::command]
pub async fn ocr_get_active_model(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let active = state.active_ocr_model.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
}

/// Set the active OCR model version (saves to config, releases cached engine).
/// The engine will be re-created on the next OCR call with the new model.
#[tauri::command]
pub async fn ocr_set_active_model(
    model_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Validate model name
    let valid_models = ["ppocr-v4", "ppocr-v5", "ppocr-v6"];
    if !valid_models.contains(&model_name.as_str()) {
        return Err(format!("Unknown OCR model: {model_name}"));
    }

    // Save to config
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.active_ocr_model = model_name.clone();
        crate::config::app_config::AppConfig::save(&config).map_err(|e| e.to_string())?;
    }

    // Update in-memory state
    {
        let mut active = state.active_ocr_model.lock().map_err(|e| e.to_string())?;
        *active = model_name.clone();
    }

    // Release cached engine (will be re-created with new model on next OCR call)
    {
        let mut engine = state.ocr_engine.lock().map_err(|e| e.to_string())?;
        *engine = None;
    }

    log::info!("[ocr_set_active_model] switched to {model_name}, engine released");
    Ok(())
}

/// Release the cached OCR engine to free ONNX Runtime resources.
/// References snow-shot's `ocr_release` command.
#[tauri::command]
pub async fn ocr_release(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut engine = state.ocr_engine.lock().map_err(|e| e.to_string())?;
    *engine = None;
    log::info!("[ocr_release] engine released");
    Ok(())
}
use crate::engine::model_manager::is_ppocr_installed_at;
use crate::engine::ocr::{types::OcrResult, OcrEngine};
use crate::AppState;
use std::path::Path;

/// Result of screen capture — returns the image path for frontend region selection.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotCapture {
    pub image_path: String,
    pub width: u32,
    pub height: u32,
}

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
            eng.recognize_from_path(Path::new(&image_path))
                .map_err(|e| e.to_string())
        } else {
            // First call: create and cache engine
            let mut eng =
                OcrEngine::new_with_memory(&model_dir, false).map_err(|e| e.to_string())?;
            let result = eng
                .recognize_from_path(Path::new(&image_path))
                .map_err(|e| e.to_string());
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
            eng.recognize_from_bytes(&image_data)
                .map_err(|e| e.to_string())
        } else {
            let mut eng =
                OcrEngine::new_with_memory(&model_dir, false).map_err(|e| e.to_string())?;
            let result = eng
                .recognize_from_bytes(&image_data)
                .map_err(|e| e.to_string());
            *guard = Some(eng);
            result
        }
    })
    .await
    .map_err(|e| format!("OCR task failed: {e}"))?
}

/// Capture the primary monitor and save screenshot to a temp file.
/// Returns the image path and dimensions for the frontend to display a region selection overlay.
#[tauri::command]
pub async fn capture_screenshot() -> Result<ScreenshotCapture, String> {
    tokio::task::spawn_blocking(move || {
        let monitors =
            xcap::Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {e}"))?;
        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary())
            .ok_or_else(|| "No primary monitor found".to_string())?;
        let image = primary
            .capture_image()
            .map_err(|e| format!("Failed to capture screen: {e}"))?;

        let width = image.width();
        let height = image.height();

        let temp_path = std::env::temp_dir().join("velocitext_screenshot.png");
        image
            .save(&temp_path)
            .map_err(|e| format!("Failed to save screenshot: {e}"))?;

        Ok(ScreenshotCapture {
            image_path: temp_path.to_string_lossy().to_string(),
            width,
            height,
        })
    })
    .await
    .map_err(|e| format!("Screenshot capture failed: {e}"))?
}

/// Run OCR on a cropped region of a captured screenshot.
/// The frontend provides the crop coordinates after user selection.
#[tauri::command]
pub async fn ocr_screenshot_region(
    image_path: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
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
        // Load the full screenshot
        let img = image::open(&image_path)
            .map_err(|e| format!("Failed to open screenshot: {e}"))?;

        // Crop the selected region
        let cropped = img.crop_imm(x, y, width, height);

        let mut guard = engine_arc.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut eng) = *guard {
            eng.recognize_from_image(&cropped, 1.0)
                .map_err(|e| e.to_string())
        } else {
            let mut eng =
                OcrEngine::new_with_memory(&model_dir, false).map_err(|e| e.to_string())?;
            let result = eng
                .recognize_from_image(&cropped, 1.0)
                .map_err(|e| e.to_string());
            *guard = Some(eng);
            result
        }
    })
    .await
    .map_err(|e| format!("Screenshot OCR failed: {e}"))?
}

/// Copy text to the system clipboard.
/// Uses `arboard` directly to avoid the "Document is not focused" error
/// that occurs with the browser clipboard API.
#[tauri::command]
pub async fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| format!("Failed to open clipboard: {e}"))?;
        clipboard
            .set_text(&text)
            .map_err(|e| format!("Failed to set clipboard text: {e}"))
    })
    .await
    .map_err(|e| format!("Clipboard task failed: {e}"))?
}

/// Get the currently active OCR model version.
#[tauri::command]
pub async fn ocr_get_active_model(state: tauri::State<'_, AppState>) -> Result<String, String> {
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
pub async fn ocr_release(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut engine = state.ocr_engine.lock().map_err(|e| e.to_string())?;
    *engine = None;
    log::info!("[ocr_release] engine released");
    Ok(())
}
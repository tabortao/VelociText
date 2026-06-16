use crate::engine::model_manager::is_ppocr_installed_at;
use crate::engine::ocr::{types::OcrResult, OcrEngine};
use crate::AppState;
use std::path::Path;
use tauri::{Emitter, Manager};

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

/// Capture all monitors and stitch them into a single screenshot.
/// Returns the image path, dimensions, and bounding box for positioning the screenshot window.
/// References snow-shot's `capture_all_monitors` command.
#[tauri::command]
pub async fn capture_all_monitors() -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || capture_all_monitors_inner())
        .await
        .map_err(|e| format!("Screenshot capture failed: {e}"))?
}

/// Run OCR on a cropped region of a captured screenshot.
/// The frontend provides the crop coordinates after user selection.
/// Also saves the cropped region as a temp PNG for preview display.
#[tauri::command]
pub async fn ocr_screenshot_region(
    image_path: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    model_version: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
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

        // Save cropped region to temp file for preview
        let cropped_path = std::env::temp_dir().join("velocitext_screenshot_crop.png");
        cropped
            .save(&cropped_path)
            .map_err(|e| format!("Failed to save cropped screenshot: {e}"))?;

        let cropped_path_str = cropped_path.to_string_lossy().to_string();

        let mut guard = engine_arc.lock().map_err(|e| e.to_string())?;
        let result = if let Some(ref mut eng) = *guard {
            eng.recognize_from_image(&cropped, 1.0)
                .map_err(|e| e.to_string())?
        } else {
            let mut eng =
                OcrEngine::new_with_memory(&model_dir, false).map_err(|e| e.to_string())?;
            let result = eng
                .recognize_from_image(&cropped, 1.0)
                .map_err(|e| e.to_string());
            *guard = Some(eng);
            result?
        };

        // Return OcrResult along with the cropped image path
        Ok(serde_json::json!({
            "ocrResult": result,
            "croppedImagePath": cropped_path_str,
        }))
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

/// Start screenshot selection by creating a transparent fullscreen window.
/// References snow-shot's `create_draw_window` — transparent, frameless, always-on-top
/// window covering all monitors with screenshot displayed at 1:1 scale.
///
/// The screenshot data is stored in AppState and retrieved by the screenshot window
/// via `get_screenshot_data` command, avoiding event timing issues.
#[tauri::command]
pub async fn start_screenshot_selection(
    app: tauri::AppHandle,
    model_version: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    // Step 1: Capture all monitors
    let capture_result = capture_all_monitors_inner()?;

    let bbox = capture_result
        .get("boundingBox")
        .ok_or("Missing boundingBox")?;
    let min_x = bbox["minX"].as_i64().ok_or("Missing minX")? as i32;
    let min_y = bbox["minY"].as_i64().ok_or("Missing minY")? as i32;
    let bbox_width = bbox["width"].as_u64().ok_or("Missing bbox width")? as u32;
    let bbox_height = bbox["height"].as_u64().ok_or("Missing bbox height")? as u32;

    // Step 2: Store screenshot data in AppState for the screenshot window to retrieve
    {
        let mut pending = state.pending_screenshot.lock().map_err(|e| e.to_string())?;
        *pending = Some(serde_json::json!({
            "imagePath": capture_result["imagePath"],
            "width": capture_result["width"],
            "height": capture_result["height"],
            "modelVersion": model_version,
        }));
    }

    // Step 3: Close any existing screenshot window
    if let Some(existing) = app.get_webview_window("screenshot") {
        let _ = existing.close();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Step 4: Create transparent fullscreen window covering all monitors
    let _window = tauri::WebviewWindowBuilder::new(&app, "screenshot", tauri::WebviewUrl::App("screenshot.html".into()))
        .title("VelociText Screenshot")
        .inner_size(bbox_width as f64, bbox_height as f64)
        .position(min_x as f64, min_y as f64)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .build()
        .map_err(|e| format!("Failed to create screenshot window: {e}"))?;

    log::info!(
        "[start_screenshot_selection] window created, size={}x{}, pos=({},{})",
        bbox_width,
        bbox_height,
        min_x,
        min_y
    );

    Ok(capture_result)
}

/// Get the pending screenshot data. Called by the screenshot window after it finishes loading.
/// Returns the data once and clears it from AppState.
#[tauri::command]
pub async fn get_screenshot_data(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let mut pending = state.pending_screenshot.lock().map_err(|e| e.to_string())?;
    pending
        .take()
        .ok_or("No pending screenshot data".to_string())
}

/// Close the screenshot window. Called from the screenshot window itself after
/// region selection or ESC cancel. References snow-shot's `closeWindowAfterDelay`.
#[tauri::command]
pub async fn close_screenshot_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("screenshot") {
        window
            .close()
            .map_err(|e| format!("Failed to close screenshot window: {e}"))?;
        log::info!("[close_screenshot_window] screenshot window closed");
    }
    Ok(())
}

/// Called from the screenshot window after OCR completes.
/// Emits the result to the main window so it can display the result.
/// Includes the cropped screenshot path for preview display.
/// When the main window is hidden (minimized to tray), shows a desktop toast notification.
#[tauri::command]
pub async fn screenshot_ocr_done(
    app: tauri::AppHandle,
    text: String,
    time_ms: u64,
    cropped_image_path: Option<String>,
    ocr_result: Option<serde_json::Value>,
) -> Result<(), String> {
    // Check if main window is visible
    let main_visible = app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);

    // Emit to the main window specifically
    if let Some(main_window) = app.get_webview_window("main") {
        main_window
            .emit(
                "screenshot-ocr-result",
                serde_json::json!({
                    "text": text,
                    "timeMs": time_ms,
                    "croppedImagePath": cropped_image_path,
                    "ocrResult": ocr_result,
                }),
            )
            .map_err(|e| format!("Failed to emit screenshot-ocr-result: {e}"))?;
    }

    // If main window is not visible (minimized to tray), show a desktop toast
    if !main_visible && !text.is_empty() {
        show_desktop_toast(&app, "文本复制成功");
    }

    Ok(())
}

/// Show a desktop toast notification by creating a small transparent overlay window.
/// The window auto-closes after 2 seconds via a Rust-side timer.
fn show_desktop_toast(app: &tauri::AppHandle, message: &str) {
    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
  background: transparent;
  overflow: hidden;
  -webkit-user-select: none;
  user-select: none;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
}}
.toast {{
  background: #dcfce7;
  color: #166534;
  padding: 10px 24px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  font-family: system-ui, -apple-system, sans-serif;
  box-shadow: 0 4px 16px rgba(0,0,0,0.12);
  border: 1px solid #bbf7d0;
  text-align: center;
  white-space: nowrap;
  animation: fadeIn 0.3s ease;
}}
@keyframes fadeIn {{ from {{ opacity: 0; }} to {{ opacity: 1; }} }}
</style></head>
<body><div class="toast">{message}</div></body></html>"#
    );

    // Save HTML to temp file
    let temp_html = std::env::temp_dir().join("velocitext_toast.html");
    if std::fs::write(&temp_html, &html_content).is_ok() {
        // Get primary monitor dimensions for positioning
        let monitors = xcap::Monitor::all().ok();
        let (screen_w, screen_h) = if let Some(ref mons) = monitors {
            if let Some(primary) = mons.first() {
                (primary.width() as f64, primary.height() as f64)
            } else {
                (1920.0, 1080.0)
            }
        } else {
            (1920.0, 1080.0)
        };

        // Estimate toast size
        let toast_w = 300.0;
        let toast_h = 50.0;
        let pos_x = (screen_w - toast_w) / 2.0;
        let pos_y = screen_h * 0.18;

        let url = tauri::WebviewUrl::External(
            format!("file:///{}", temp_html.to_string_lossy())
                .parse()
                .unwrap(),
        );

        if let Ok(_toast_window) = tauri::WebviewWindowBuilder::new(app, "toast", url)
            .title("VelociText Toast")
            .inner_size(toast_w, toast_h)
            .position(pos_x, pos_y)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .build()
        {
            log::info!("[show_desktop_toast] toast window created");

            // Auto-close after 2.3 seconds via Rust timer
            let app_clone = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(2300));
                if let Some(window) = app_clone.get_webview_window("toast") {
                    let _ = window.close();
                    log::info!("[show_desktop_toast] toast window auto-closed");
                }
            });
        }
    }
}

/// Inner implementation of capture_all_monitors (reusable without tauri::State).
fn capture_all_monitors_inner() -> Result<serde_json::Value, String> {
    let monitors =
        xcap::Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {e}"))?;

    if monitors.is_empty() {
        return Err("No monitors found".to_string());
    }

    // Calculate bounding box of all monitors
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for m in &monitors {
        min_x = min_x.min(m.x());
        min_y = min_y.min(m.y());
        max_x = max_x.max(m.x() + m.width() as i32);
        max_y = max_y.max(m.y() + m.height() as i32);
    }

    let bbox_width = (max_x - min_x) as u32;
    let bbox_height = (max_y - min_y) as u32;

    // Capture all monitors in parallel
    let captures: Vec<_> = monitors
        .iter()
        .map(|m| {
            let img = m.capture_image();
            (m.x(), m.y(), img)
        })
        .collect();

    // Stitch images into a single canvas
    let mut canvas = image::RgbaImage::new(bbox_width, bbox_height);

    for (x, y, cap) in captures {
        let cap = cap.map_err(|e| format!("Failed to capture monitor: {e}"))?;
        let offset_x = (x - min_x) as u32;
        let offset_y = (y - min_y) as u32;
        image::imageops::overlay(&mut canvas, &cap, offset_x as i64, offset_y as i64);
    }

    let temp_path = std::env::temp_dir().join("velocitext_screenshot_all.png");
    canvas
        .save(&temp_path)
        .map_err(|e| format!("Failed to save screenshot: {e}"))?;

    Ok(serde_json::json!({
        "imagePath": temp_path.to_string_lossy(),
        "width": bbox_width,
        "height": bbox_height,
        "boundingBox": {
            "minX": min_x,
            "minY": min_y,
            "maxX": max_x,
            "maxY": max_y,
            "width": bbox_width,
            "height": bbox_height,
        }
    }))
}
//! Dictionary management Tauri commands.
//!
//! Provides CRUD operations for hotwords and text replacements,
//! with automatic model rebuilding when hotwords change.

use crate::config::dictionary_config::DictionaryConfig;
use crate::AppState;
use tauri::State;

/// Returns the full dictionary configuration.
#[tauri::command]
pub fn get_dictionary_config(state: State<'_, AppState>) -> Result<DictionaryConfig, String> {
    let config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

/// Save hotwords and rebuild the ASR model.
#[tauri::command]
pub fn save_hotwords(
    state: State<'_, AppState>,
    hotwords: Vec<crate::config::dictionary_config::HotwordEntry>,
) -> Result<(), String> {
    log::info!("[save_hotwords] saving {} hotword entries", hotwords.len());

    // Snapshot old config for rollback
    let old_config = {
        let config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
        config.clone()
    };

    // Update in-memory config
    {
        let mut config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
        config.hotwords = hotwords;
        config.save()?;
    }

    // Generate hotwords file (pass None to build_models if empty)
    let hotwords_path: Option<String> = {
        let config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
        if config.hotwords.is_empty() {
            None
        } else {
            Some(config.generate_hotwords_file()?)
        }
    };

    // If hotwords are empty, just clear the file and skip rebuild
    if hotwords_path.is_none() {
        // Delete old hotwords file if it exists
        DictionaryConfig::remove_hotwords_file();
        // Don't need to rebuild model — just use None for hotwords when model next loads
        let mut hwp = state.hotwords_file_path.lock().map_err(|e| e.to_string())?;
        *hwp = None;
        log::info!("[save_hotwords] hotwords cleared");
        return Ok(());
    }

    let hotwords_path = hotwords_path.unwrap();

    // Update hotwords file path in state
    {
        let mut hwp = state.hotwords_file_path.lock().map_err(|e| e.to_string())?;
        *hwp = Some(hotwords_path.clone());
    }

    // Rebuild the recognizer with new hotwords
    if let Err(e) = rebuild_recognizer(&state, &hotwords_path) {
        // Rollback: restore old config, clear hotwords file path
        log::error!("[save_hotwords] rebuild failed, rolling back config: {e}");
        {
            let mut config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
            *config = old_config;
            config.save()?;
        }
        {
            let mut hwp = state.hotwords_file_path.lock().map_err(|e| e.to_string())?;
            *hwp = None;
        }
        return Err(format!(
            "模型重建失败: {e}。热词配置已回滚，请检查模型兼容性。"
        ));
    }

    log::info!("[save_hotwords] model rebuilt with hotwords");
    Ok(())
}

/// Save text replacements (no model rebuild needed).
#[tauri::command]
pub fn save_replacements(
    state: State<'_, AppState>,
    replacements: Vec<crate::config::dictionary_config::ReplacementEntry>,
) -> Result<(), String> {
    log::info!(
        "[save_replacements] saving {} replacements",
        replacements.len()
    );

    let mut config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
    config.replacements = replacements;
    config.save()?;

    log::info!("[save_replacements] replacements saved");
    Ok(())
}

/// Get the current hotwords file path.
#[tauri::command]
pub fn get_hotwords_file_path(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let hwp = state.hotwords_file_path.lock().map_err(|e| e.to_string())?;
    Ok(hwp.clone())
}

/// Rebuild the recognizer with updated hotwords.
///
/// This is similar to `apply_vad_settings` but only changes the hotwords file,
/// keeping VAD settings unchanged.
fn rebuild_recognizer(state: &State<'_, AppState>, hotwords_path: &str) -> Result<(), String> {
    let model_path = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.model_path.clone()
    };

    let active_model = state
        .active_model
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let vad_settings = state
        .vad_settings
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    log::info!(
        "[rebuild_recognizer] model={}, hotwords={}",
        active_model,
        hotwords_path
    );

    // Build new recognizer with updated hotwords
    let preferred = if active_model.is_empty() {
        None
    } else {
        Some(active_model.as_str())
    };
    let (rec, _vad, _threads, model_name) = crate::build_models(
        &model_path,
        &vad_settings,
        preferred,
        Some(hotwords_path.to_string()),
    )?;

    // Update state
    {
        let mut r = state.recognizer.lock().map_err(|e| e.to_string())?;
        *r = Some(rec);
    }
    {
        let mut a = state.active_model.lock().map_err(|e| e.to_string())?;
        *a = model_name;
    }

    log::info!("[rebuild_recognizer] recognizer updated successfully");
    Ok(())
}

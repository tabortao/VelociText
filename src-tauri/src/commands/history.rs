//! History commands - JSON file-based persistence

use crate::models::history::HistoryEntry;
use crate::AppState;
use std::fs;
use std::path::PathBuf;
use tauri::State;

/// Get the history file path from app data directory
fn history_file_path(config: &crate::config::app_config::AppConfig) -> PathBuf {
    let base = std::path::Path::new(&config.model_path).parent().unwrap_or(std::path::Path::new("."));
    base.join("history.json")
}

/// Load history from JSON file
fn load_history_file(path: &std::path::Path) -> Vec<HistoryEntry> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// Save history to JSON file
fn save_history_file(path: &std::path::Path, entries: &[HistoryEntry]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get all history entries
#[tauri::command]
pub async fn get_history(
    state: State<'_, AppState>,
) -> Result<Vec<HistoryEntry>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let path = history_file_path(&config);
    Ok(load_history_file(&path))
}

/// Save a history entry
#[tauri::command]
pub async fn save_history(
    state: State<'_, AppState>,
    entry: HistoryEntry,
) -> Result<(), String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let path = history_file_path(&config);
    let mut entries = load_history_file(&path);

    // Deduplicate by id
    entries.retain(|e| e.id != entry.id);
    entries.insert(0, entry);

    // Keep last 100 entries
    if entries.len() > 100 {
        entries.truncate(100);
    }

    save_history_file(&path, &entries)
}

/// Delete a history entry by id
#[tauri::command]
pub async fn delete_history(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let path = history_file_path(&config);
    let mut entries = load_history_file(&path);
    entries.retain(|e| e.id != id);
    save_history_file(&path, &entries)
}

/// Clear all history
#[tauri::command]
pub async fn clear_history(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let path = history_file_path(&config);
    save_history_file(&path, &[])
}
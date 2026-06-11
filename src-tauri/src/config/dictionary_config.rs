//! Dictionary configuration — manages hotwords and text replacements.
//!
//! Provides a unified hotwords manager for all ASR model types
//! (SenseVoice-Small, Paraformer, Qwen3-ASR).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A hotword entry for ASR boosting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotwordEntry {
    pub word: String,
    pub weight: f32,
}

/// A text replacement entry for post-processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementEntry {
    pub original: String,
    pub replacement: String,
}

/// Complete dictionary configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryConfig {
    pub hotwords: Vec<HotwordEntry>,
    pub replacements: Vec<ReplacementEntry>,
}

impl DictionaryConfig {
    /// Returns the path to the dictionary config file.
    fn config_file_path() -> PathBuf {
        app_data_dir().join("dictionary.json")
    }

    /// Returns the path to the generated hotwords file.
    fn hotwords_file_path() -> PathBuf {
        app_data_dir().join("hotwords.txt")
    }

    /// Load config from disk. Returns default if file doesn't exist or is invalid.
    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(config) => {
                            log::info!("[DictionaryConfig] loaded from {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            log::warn!("[DictionaryConfig] failed to parse: {e}, using defaults");
                        }
                    }
                }
                Err(e) => {
                    log::warn!("[DictionaryConfig] failed to read: {e}, using defaults");
                }
            }
        }
        Self::default()
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize: {e}"))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write config: {e}"))?;
        log::info!("[DictionaryConfig] saved to {}", path.display());
        Ok(())
    }

    /// Generate the hotwords file for sherpa-onnx.
    ///
    /// Format: one `word weight` pair per line.
    pub fn generate_hotwords_file(&self) -> Result<String, String> {
        let path = Self::hotwords_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir: {e}"))?;
        }

        if self.hotwords.is_empty() {
            // Write empty file on empty hotwords
            std::fs::write(&path, "")
                .map_err(|e| format!("Failed to write hotwords file: {e}"))?;
        } else {
            let mut lines = String::new();
            for entry in &self.hotwords {
                lines.push_str(&format!("{} {}\n", entry.word, entry.weight));
            }
            std::fs::write(&path, &lines)
                .map_err(|e| format!("Failed to write hotwords file: {e}"))?;
        }

        log::info!(
            "[DictionaryConfig] generated hotwords file at {} ({} entries)",
            path.display(),
            self.hotwords.len()
        );
        Ok(path.to_string_lossy().to_string())
    }

    /// Apply text replacements to a string.
    pub fn apply_replacements(&self, text: &str) -> String {
        if self.replacements.is_empty() {
            return text.to_string();
        }
        let mut result = text.to_string();
        for entry in &self.replacements {
            result = result.replace(&entry.original, &entry.replacement);
        }
        result
    }

    /// Remove the hotwords file (cleanup).
    pub fn remove_hotwords_file() {
        let path = Self::hotwords_file_path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("VelociText")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".local").join("share").join("VelociText")
    }
}
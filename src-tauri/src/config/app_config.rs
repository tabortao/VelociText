use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// 模型存储路径
    pub model_path: String,
    /// 默认语言
    pub default_language: String,
    /// 导出格式: txt, srt, vtt
    pub export_format: String,
    /// 是否启用 VAD
    pub use_vad: bool,
    /// FFmpeg 路径 (可选)
    pub ffmpeg_path: Option<String>,
    /// 活跃 ASR 模型: "sense-voice-small" | "paraformer"
    #[serde(default = "default_active_model")]
    pub active_model: String,
    /// 侧边栏是否收起
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// 活跃 OCR 模型: "ppocr-v4" | "ppocr-v5" | "ppocr-v6"
    #[serde(default = "default_active_ocr_model")]
    pub active_ocr_model: String,
}

fn default_active_ocr_model() -> String {
    "ppocr-v5".to_string()
}

fn default_active_model() -> String {
    "sense-voice-small".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        let app_data = app_data_dir();
        let model_path = app_data.join("models").to_string_lossy().to_string();
        Self {
            model_path,
            default_language: "zh".into(),
            export_format: "txt".into(),
            use_vad: true,
            ffmpeg_path: None,
            active_model: default_active_model(),
            sidebar_collapsed: false,
            active_ocr_model: default_active_ocr_model(),
        }
    }
}

impl AppConfig {
    /// Returns the path to the config file.
    fn config_file_path() -> PathBuf {
        app_data_dir().join("config.json")
    }

    /// Load config from disk. Returns default if file doesn't exist or is invalid.
    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        log::info!("[AppConfig] loaded from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        log::warn!("[AppConfig] failed to parse config: {e}, using defaults");
                    }
                },
                Err(e) => {
                    log::warn!("[AppConfig] failed to read config: {e}, using defaults");
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
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(&path, content).map_err(|e| format!("Failed to write config: {e}"))?;
        log::info!("[AppConfig] saved to {}", path.display());
        Ok(())
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

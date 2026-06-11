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
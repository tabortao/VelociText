use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 转录任务状态 (为历史记录功能预留)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Pending,
    ExtractingAudio,
    Transcribing,
    Completed,
    Failed,
    Cancelled,
}

/// 转录结果分段
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeSegment {
    /// 开始时间 (秒)
    pub start: f64,
    /// 结束时间 (秒)
    pub end: f64,
    /// 识别文本
    pub text: String,
}

/// 转录任务 (为历史记录功能预留)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeTask {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub status: TaskStatus,
    pub progress: f64,
    pub segments: Vec<TranscribeSegment>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// 转录选项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeOptions {
    pub file_path: String,
    pub language: Option<String>,
    pub use_vad: Option<bool>,
    /// 模型类型 (默认 SenseVoice)
    pub model_type: Option<String>,
    /// 热词文件路径
    pub hotwords_file: Option<String>,
}

/// 转录完成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeResult {
    /// 转录分段
    pub segments: Vec<TranscribeSegment>,
    /// 音频总时长 (秒)
    pub audio_duration: f64,
    /// 转录耗时 (毫秒)
    pub elapsed_ms: u64,
    /// 原始文件名
    pub file_name: String,
    /// 使用的模型类型
    pub model_type: String,
}

/// 批量转录中单个文件的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFileResult {
    pub file_name: String,
    pub file_path: String,
    pub success: bool,
    pub segments: Vec<TranscribeSegment>,
    pub audio_duration: f64,
    pub elapsed_ms: u64,
    pub model_type: String,
    pub error: Option<String>,
}

/// 批量转录总结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub results: Vec<BatchFileResult>,
    pub total_files: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub total_elapsed_ms: u64,
    pub total_audio_duration: f64,
}
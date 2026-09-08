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



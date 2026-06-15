use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 历史记录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub duration_secs: f64,
    pub text: String,
    pub language: String,
    pub model_type: String,
    pub segment_count: u32,
    pub created_at: DateTime<Utc>,
}

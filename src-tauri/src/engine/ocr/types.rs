use serde::{Deserialize, Serialize};

/// OCR result returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub text_blocks: Vec<TextBlockInfo>,
    pub total_time_ms: u64,
}

/// Single text block detected and recognized by OCR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBlockInfo {
    pub text: String,
    pub confidence: f32,
    /// 4 corner points of the bounding box (clockwise from top-left).
    pub box_points: Vec<[f32; 2]>,
}

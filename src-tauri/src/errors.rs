use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("音频提取失败: {0}")]
    AudioExtraction(String),

    #[error("模型加载失败: {0}")]
    ModelLoad(String),

    #[error("转录失败: {0}")]
    Transcription(String),

    #[error("导出失败: {0}")]
    Export(String),

    #[error("IO 错误: {0}")]
    Io(#[from] io::Error),

    #[error("FFmpeg 未找到，请安装 FFmpeg 并确保其在 PATH 中")]
    FfmpegNotFound,

    #[error("不支持的格式: {0}")]
    UnsupportedFormat(String),

    #[error("模型未下载: {0}")]
    ModelNotDownloaded(String),

    #[error("模型下载失败: {0}")]
    ModelDownload(String),

    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("OCR 错误: {0}")]
    Ocr(String),
}

pub type AppResult<T> = Result<T, AppError>;
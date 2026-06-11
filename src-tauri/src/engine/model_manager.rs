//! 模型管理器 - 负责模型下载和缓存管理

use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub name: String,
    pub display_name: String,
    pub size: String,
    pub installed: bool,
    pub path: Option<String>,
}

/// 模型下载进度
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_name: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
    pub stage: String, // "downloading" | "extracting" | "completed" | "error"
}

/// 模型文件候选名
const MODEL_CANDIDATES: &[&str] = &["model.onnx", "model.int8.onnx", "model_q8.onnx"];
const SILERO_VAD_FILE: &str = "model.onnx";

/// Check if a silero-vad model is installed at the given directory.
pub fn is_silero_vad_installed_at(dir: &Path) -> bool {
    dir.join(SILERO_VAD_FILE).exists()
}

/// Check if the Paraformer model is installed at the given directory.
/// Requires `model.int8.onnx` (or `model.onnx`) and `tokens.txt`.
pub fn is_paraformer_installed_at(dir: &Path) -> bool {
    let has_model = dir.join("model.int8.onnx").exists() || dir.join("model.onnx").exists();
    let has_tokens = dir.join("tokens.txt").exists();
    has_model && has_tokens
}

fn find_model_file(dir: &Path) -> Option<std::path::PathBuf> {
    MODEL_CANDIDATES
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
}

pub fn is_model_installed_at(dir: &Path) -> bool {
    find_model_file(dir).is_some() && dir.join("tokens.txt").exists()
}

/// 模型管理器
pub struct ModelManager {
    models_dir: String,
}

impl ModelManager {
    pub fn new(models_dir: String) -> Self {
        Self { models_dir }
    }

    /// 获取所有可用模型列表
    pub fn list_models(&self) -> Vec<ModelInfo> {
        let sense_voice_path = Path::new(&self.models_dir).join("sense-voice-small");
        let sense_voice_installed = is_model_installed_at(&sense_voice_path);

        let paraformer_path = Path::new(&self.models_dir).join("paraformer");
        let paraformer_installed = is_paraformer_installed_at(&paraformer_path);

        let silero_vad_path = Path::new(&self.models_dir).join("silero-vad");
        let silero_vad_installed = is_silero_vad_installed_at(&silero_vad_path);

        vec![
            ModelInfo {
                name: "sense-voice-small".into(),
                display_name: "SenseVoice-Small".into(),
                size: "~230MB (q8)".into(),
                installed: sense_voice_installed,
                path: if sense_voice_installed {
                    Some(sense_voice_path.to_string_lossy().to_string())
                } else {
                    None
                },
            },
            ModelInfo {
                name: "paraformer".into(),
                display_name: "Paraformer-Large".into(),
                size: "~238MB (int8)".into(),
                installed: paraformer_installed,
                path: if paraformer_installed {
                    Some(paraformer_path.to_string_lossy().to_string())
                } else {
                    None
                },
            },
            ModelInfo {
                name: "silero-vad".into(),
                display_name: "Silero VAD".into(),
                size: "~2.7MB".into(),
                installed: silero_vad_installed,
                path: if silero_vad_installed {
                    Some(silero_vad_path.to_string_lossy().to_string())
                } else {
                    None
                },
            },
        ]
    }

    /// 检查指定模型是否已安装
    pub fn is_model_installed(&self, model_name: &str) -> bool {
        let model_path = Path::new(&self.models_dir).join(model_name);
        is_model_installed_at(&model_path)
    }

    /// 获取模型路径
    pub fn get_model_path(&self, model_name: &str) -> AppResult<String> {
        let model_path = Path::new(&self.models_dir).join(model_name);
        if !self.is_model_installed(model_name) {
            return Err(AppError::ModelNotDownloaded(model_name.into()));
        }
        Ok(model_path.to_string_lossy().to_string())
    }

    /// 下载模型文件
    ///
    /// 从 ModelScope 下载 SenseVoice-Small q8 模型。
    /// 通过回调报告进度。
    pub fn download_sense_voice_small(
        &self,
        on_progress: &dyn Fn(DownloadProgress),
    ) -> AppResult<String> {
        let model_dir = Path::new(&self.models_dir).join("sense-voice-small");
        std::fs::create_dir_all(&model_dir)?;

        let base_url = "https://www.modelscope.cn/models/xiaowangge/sherpa-onnx-sense-voice-small/resolve/master";
        let files: &[(&str, u64)] = &[
            ("model_q8.onnx", 228), // ~228 MB
            ("tokens.txt", 1),       // ~316 KB
            ("README.md", 0),        // optional
        ];

        for (file_name, size_mb) in files {
            let url = format!("{}/{}", base_url, file_name);
            let dest = model_dir.join(file_name);

            // Skip if already exists and correct size
            if dest.exists() {
                let existing = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                if *size_mb > 0 && existing > size_mb * 1024 * 1024 / 2 {
                    // Rough check: file is at least half expected size
                    continue;
                }
            }

            // Download
            let resp = ureq::get(&url)
                .call()
                .map_err(|e| AppError::ModelDownload(format!("HTTP 请求失败: {}", e)))?;

            let total = resp
                .header("Content-Length")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            let mut reader = resp.into_reader();
            let mut buf = [0u8; 8192];
            let mut downloaded: u64 = 0;
            let mut file = std::fs::File::create(&dest)
                .map_err(|e| AppError::ModelDownload(format!("创建文件失败: {}", e)))?;

            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| AppError::ModelDownload(format!("下载中断: {}", e)))?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut file, &buf[..n])
                    .map_err(|e| AppError::ModelDownload(format!("写入文件失败: {}", e)))?;
                downloaded += n as u64;

                if total > 0 {
                    let percentage = (downloaded as f64 / total as f64) * 100.0;
                    on_progress(DownloadProgress {
                        model_name: "sense-voice-small".into(),
                        downloaded,
                        total,
                        percentage,
                        stage: format!("下载 {}...", file_name),
                    });
                }
            }
        }

        // 验证
        if !is_model_installed_at(&model_dir) {
            return Err(AppError::ModelDownload(
                "下载完成但模型文件校验失败".into(),
            ));
        }

        on_progress(DownloadProgress {
            model_name: "sense-voice-small".into(),
            downloaded: 100,
            total: 100,
            percentage: 100.0,
            stage: "completed".into(),
        });

        Ok(model_dir.to_string_lossy().to_string())
    }

    /// Download silero-vad ONNX model from ModelScope (xiaowangge/sherpa-onnx-sense-voice-small repository).
    /// The model is ~2.7MB.
    pub fn download_silero_vad(&self, on_progress: &dyn Fn(DownloadProgress)) -> AppResult<String> {
        let model_dir = Path::new(&self.models_dir).join("silero-vad");
        std::fs::create_dir_all(&model_dir)?;

        let base_url = "https://www.modelscope.cn/models/xiaowangge/sherpa-onnx-sense-voice-small/resolve/master/silero-vad";
        let files: &[(&str, u64)] = &[
            ("model.onnx", 3), // ~2.7 MB
        ];

        for (file_name, size_mb) in files {
            let url = format!("{}/{}", base_url, file_name);
            let dest = model_dir.join(file_name);

            if dest.exists() {
                let existing = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                if *size_mb > 0 && existing > size_mb * 1024 * 1024 / 2 {
                    continue;
                }
            }

            // Download
            let resp = ureq::get(&url)
                .call()
                .map_err(|e| AppError::ModelDownload(format!("HTTP request failed: {}", e)))?;

            let total = resp
                .header("Content-Length")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            let mut reader = resp.into_reader();
            let mut buf = [0u8; 8192];
            let mut downloaded: u64 = 0;
            let mut file = std::fs::File::create(&dest)
                .map_err(|e| AppError::ModelDownload(format!("Create file failed: {}", e)))?;

            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| AppError::ModelDownload(format!("Download interrupted: {}", e)))?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut file, &buf[..n])
                    .map_err(|e| AppError::ModelDownload(format!("Write file failed: {}", e)))?;
                downloaded += n as u64;

                if total > 0 {
                    let percentage = (downloaded as f64 / total as f64) * 100.0;
                    on_progress(DownloadProgress {
                        model_name: "silero-vad".into(),
                        downloaded,
                        total,
                        percentage,
                        stage: format!("Downloading {}...", file_name),
                    });
                }
            }
        }

        if !is_silero_vad_installed_at(&model_dir) {
            return Err(AppError::ModelDownload(
                "Download completed but model file check failed".into(),
            ));
        }

        on_progress(DownloadProgress {
            model_name: "silero-vad".into(),
            downloaded: 100,
            total: 100,
            percentage: 100.0,
            stage: "completed".into(),
        });

        Ok(model_dir.to_string_lossy().to_string())
    }

    /// Download Paraformer-Large ASR model from ModelScope.
    ///
    /// Downloads from `iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx`.
    /// The ModelScope model uses `model_quant.onnx` and `tokens.json`, which are
    /// renamed to `model.int8.onnx` and `tokens.txt` respectively for sherpa-onnx compatibility.
    pub fn download_paraformer_large(
        &self,
        on_progress: &dyn Fn(DownloadProgress),
    ) -> AppResult<String> {
        let model_dir = Path::new(&self.models_dir).join("paraformer");
        std::fs::create_dir_all(&model_dir)?;

        let base_url = "https://www.modelscope.cn/models/iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx/resolve/master";

        // ModelScope file name → local file name (rename for sherpa-onnx compatibility)
        let files: &[(&str, &str, u64)] = &[
            ("model_quant.onnx", "model.int8.onnx", 238), // ~238 MB
            ("tokens.json", "tokens.txt", 1),             // ~94 KB
        ];

        for (remote_name, local_name, size_mb) in files {
            let url = format!("{}/{}", base_url, remote_name);
            let dest = model_dir.join(local_name);

            // Skip if already exists and correct size
            if dest.exists() {
                let existing = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                if *size_mb > 0 && existing > size_mb * 1024 * 1024 / 2 {
                    continue;
                }
            }

            // Download
            let resp = ureq::get(&url)
                .call()
                .map_err(|e| AppError::ModelDownload(format!("HTTP request failed: {}", e)))?;

            let total = resp
                .header("Content-Length")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            let mut reader = resp.into_reader();
            let mut buf = [0u8; 8192];
            let mut downloaded: u64 = 0;
            let mut file = std::fs::File::create(&dest)
                .map_err(|e| AppError::ModelDownload(format!("Create file failed: {}", e)))?;

            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| AppError::ModelDownload(format!("Download interrupted: {}", e)))?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut file, &buf[..n])
                    .map_err(|e| AppError::ModelDownload(format!("Write file failed: {}", e)))?;
                downloaded += n as u64;

                if total > 0 {
                    let percentage = (downloaded as f64 / total as f64) * 100.0;
                    on_progress(DownloadProgress {
                        model_name: "paraformer".into(),
                        downloaded,
                        total,
                        percentage,
                        stage: format!("Downloading {}...", remote_name),
                    });
                }
            }
        }

        if !is_paraformer_installed_at(&model_dir) {
            return Err(AppError::ModelDownload(
                "Download completed but model file check failed".into(),
            ));
        }

        on_progress(DownloadProgress {
            model_name: "paraformer".into(),
            downloaded: 100,
            total: 100,
            percentage: 100.0,
            stage: "completed".into(),
        });

        Ok(model_dir.to_string_lossy().to_string())
    }
}
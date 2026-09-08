//! 模型管理器 - 负责模型下载和缓存管理

use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

/// Download a file from URL to destination, reporting progress.
fn download_file(
    url: &str,
    dest: &Path,
    model_name: &str,
    on_progress: &dyn Fn(DownloadProgress),
) -> AppResult<()> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| AppError::ModelDownload(format!("HTTP request failed: {}", e)))?;

    // Check HTTP status code — bail early on non-2xx to avoid writing error pages as zip files
    let status = resp.status();
    if !(200..300).contains(&status) {
        return Err(AppError::ModelDownload(format!(
            "Server returned HTTP {} for URL: {}. The model file may not exist on the server yet.",
            status, url
        )));
    }

    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;
    let mut file = std::fs::File::create(dest)
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
                model_name: model_name.into(),
                downloaded,
                total,
                percentage,
                stage: "Downloading...".to_string(),
            });
        }
    }

    Ok(())
}

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

/// Check if Qwen3-ASR model is installed at the given directory.
pub fn is_qwen3_asr_installed_at(dir: &Path) -> bool {
    let has_conv_frontend = dir.join("conv_frontend.onnx").exists();
    let has_encoder = dir.join("encoder.int8.onnx").exists() || dir.join("encoder.onnx").exists();
    let has_decoder = dir.join("decoder.int8.onnx").exists() || dir.join("decoder.onnx").exists();
    let has_tokenizer = dir.join("tokenizer").exists();
    has_conv_frontend && has_encoder && has_decoder && has_tokenizer
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

        let qwen3_asr_path = Path::new(&self.models_dir).join("qwen3-asr");
        let qwen3_asr_installed = is_qwen3_asr_installed_at(&qwen3_asr_path);

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
                display_name: "Paraformer (Trilingual)".into(),
                size: "~233MB (int8)".into(),
                installed: paraformer_installed,
                path: if paraformer_installed {
                    Some(paraformer_path.to_string_lossy().to_string())
                } else {
                    None
                },
            },
            ModelInfo {
                name: "qwen3-asr".into(),
                display_name: "Qwen3-ASR (0.6B)".into(),
                size: "~450MB (int8)".into(),
                installed: qwen3_asr_installed,
                path: if qwen3_asr_installed {
                    Some(qwen3_asr_path.to_string_lossy().to_string())
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
            ("tokens.txt", 1),      // ~316 KB
            ("README.md", 0),       // optional
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
            return Err(AppError::ModelDownload("下载完成但模型文件校验失败".into()));
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

    /// Download Paraformer Trilingual ASR model (sherpa-onnx official).
    ///
    /// Downloads `paraformer.zip` from gitcode.com releases and extracts
    /// `model.int8.onnx` and `tokens.txt` to the local model directory.
    ///
    /// Users can also manually place model files in the `paraformer/` directory:
    /// - `model.int8.onnx` (or `model.onnx`) — the ONNX model
    /// - `tokens.txt` — vocabulary file
    pub fn download_paraformer_large(
        &self,
        on_progress: &dyn Fn(DownloadProgress),
    ) -> AppResult<String> {
        let model_dir = Path::new(&self.models_dir).join("paraformer");
        std::fs::create_dir_all(&model_dir)?;

        // Check if already installed with correct files
        if is_paraformer_installed_at(&model_dir) {
            // Verify tokens.txt is not in JSON format (legacy broken download)
            let tokens_path = model_dir.join("tokens.txt");
            if let Ok(content) = std::fs::read_to_string(&tokens_path) {
                if !content.trim_start().starts_with('[') {
                    let model_int8 = model_dir.join("model.int8.onnx");
                    let model_plain = model_dir.join("model.onnx");
                    let model_file = if model_int8.exists() {
                        &model_int8
                    } else {
                        &model_plain
                    };
                    if model_file.exists() {
                        let model_size_ok = std::fs::metadata(model_file)
                            .map(|m| m.len() > 50_000_000)
                            .unwrap_or(false);

                        if model_size_ok {
                            on_progress(DownloadProgress {
                                model_name: "paraformer".into(),
                                downloaded: 100,
                                total: 100,
                                percentage: 100.0,
                                stage: "completed".into(),
                            });
                            return Ok(model_dir.to_string_lossy().to_string());
                        }
                    }
                }
            }
            // Old/broken model files exist — clean them up before re-downloading
            log::warn!(
                "[download_paraformer] cleaning up old/broken model files in {:?}",
                model_dir
            );
            let _ = std::fs::remove_dir_all(&model_dir);
            std::fs::create_dir_all(&model_dir)?;
        }

        // Download zip from gitcode.com (fast in China)
        let archive_url =
            "https://gitcode.com/tabortao/VelociText/releases/download/v0.1.2/paraformer.zip";

        on_progress(DownloadProgress {
            model_name: "paraformer".into(),
            downloaded: 0,
            total: 0,
            percentage: 0.0,
            stage: "Downloading model archive...".into(),
        });

        // Download to temp file
        let temp_dir = std::env::temp_dir();
        let archive_path = temp_dir.join("paraformer.zip");

        download_file(archive_url, &archive_path, "paraformer", on_progress)?;

        on_progress(DownloadProgress {
            model_name: "paraformer".into(),
            downloaded: 100,
            total: 100,
            percentage: 90.0,
            stage: "Extracting...".into(),
        });

        // Extract zip
        let file = std::fs::File::open(&archive_path)
            .map_err(|e| AppError::ModelDownload(format!("Open archive failed: {}", e)))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::ModelDownload(format!("Read zip archive failed: {}", e)))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::ModelDownload(format!("Read zip entry failed: {}", e)))?;

            let name = entry.name().to_string();
            // Extract filename from path (handle both "model.int8.onnx" and "subdir/model.int8.onnx")
            let filename = std::path::Path::new(&name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if filename == "model.int8.onnx" || filename == "tokens.txt" {
                let dest = model_dir.join(&filename);
                let mut file_content = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut file_content)
                    .map_err(|e| AppError::ModelDownload(format!("Read entry failed: {}", e)))?;
                std::fs::write(&dest, &file_content)
                    .map_err(|e| AppError::ModelDownload(format!("Write file failed: {}", e)))?;
                log::info!("[download_paraformer] extracted {}", filename);
            }
        }

        // Clean up archive
        let _ = std::fs::remove_file(&archive_path);

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

    /// Download Qwen3-ASR 0.6B int8 model.
    ///
    /// Downloads from gitcode.com and extracts `conv_frontend.onnx`, `encoder.int8.onnx`,
    /// `decoder.int8.onnx`, and the `tokenizer/` directory to `qwen3-asr/`.
    ///
    /// Users can also manually place model files in the `qwen3-asr/` directory:
    /// - `conv_frontend.onnx` — convolutional frontend
    /// - `encoder.int8.onnx` — encoder model
    /// - `decoder.int8.onnx` — decoder model
    /// - `tokenizer/` — tokenizer directory
    pub fn download_qwen3_asr(&self, on_progress: &dyn Fn(DownloadProgress)) -> AppResult<String> {
        let model_dir = Path::new(&self.models_dir).join("qwen3-asr");
        std::fs::create_dir_all(&model_dir)?;

        // Check if already installed
        if is_qwen3_asr_installed_at(&model_dir) {
            on_progress(DownloadProgress {
                model_name: "qwen3-asr".into(),
                downloaded: 100,
                total: 100,
                percentage: 100.0,
                stage: "completed".into(),
            });
            return Ok(model_dir.to_string_lossy().to_string());
        }

        // Download zip from gitcode.com
        let archive_url = "https://gitcode.com/tabortao/VelociText/releases/download/model/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.zip";

        on_progress(DownloadProgress {
            model_name: "qwen3-asr".into(),
            downloaded: 0,
            total: 0,
            percentage: 0.0,
            stage: "Downloading Qwen3-ASR model archive...".into(),
        });

        let temp_dir = std::env::temp_dir();
        let archive_path = temp_dir.join("qwen3-asr.zip");

        download_file(archive_url, &archive_path, "qwen3-asr", on_progress)?;

        on_progress(DownloadProgress {
            model_name: "qwen3-asr".into(),
            downloaded: 100,
            total: 100,
            percentage: 90.0,
            stage: "Extracting...".into(),
        });

        // Extract zip
        let file = std::fs::File::open(&archive_path)
            .map_err(|e| AppError::ModelDownload(format!("Open archive failed: {}", e)))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::ModelDownload(format!("Read zip archive failed: {}", e)))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::ModelDownload(format!("Read zip entry failed: {}", e)))?;

            let name = entry.name().to_string();
            // Skip directories
            if entry.is_dir() {
                continue;
            }

            // Extract filename from path, preserving subdirectory structure for tokenizer/
            let entry_path = std::path::Path::new(&name);
            let filename = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // We need: conv_frontend.onnx, encoder.int8.onnx, decoder.int8.onnx, and tokenizer/* files
            let is_model_file = filename == "conv_frontend.onnx"
                || filename == "encoder.int8.onnx"
                || filename == "encoder.onnx"
                || filename == "decoder.int8.onnx"
                || filename == "decoder.onnx";

            let is_tokenizer_file = name.contains("tokenizer/") || name.contains("tokenizer\\");

            if is_model_file || is_tokenizer_file {
                // For tokenizer files, preserve relative path inside tokenizer/
                let dest = if is_tokenizer_file {
                    // Strip leading directory prefix to get relative path under tokenizer/
                    let relative = if let Some(idx) = name.find("tokenizer") {
                        &name[idx..] // "tokenizer/..." or "subdir/tokenizer/..."
                    } else {
                        &filename
                    };
                    model_dir.join(relative)
                } else {
                    model_dir.join(&filename)
                };

                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        AppError::ModelDownload(format!("Create dir failed: {}", e))
                    })?;
                }

                let mut file_content = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut file_content)
                    .map_err(|e| AppError::ModelDownload(format!("Read entry failed: {}", e)))?;
                std::fs::write(&dest, &file_content)
                    .map_err(|e| AppError::ModelDownload(format!("Write file failed: {}", e)))?;
                log::info!(
                    "[download_qwen3_asr] extracted {} -> {}",
                    name,
                    dest.display()
                );
            }
        }

        // Clean up archive
        let _ = std::fs::remove_file(&archive_path);

        if !is_qwen3_asr_installed_at(&model_dir) {
            return Err(AppError::ModelDownload(
                "Download completed but model file check failed".into(),
            ));
        }

        on_progress(DownloadProgress {
            model_name: "qwen3-asr".into(),
            downloaded: 100,
            total: 100,
            percentage: 100.0,
            stage: "completed".into(),
        });

        Ok(model_dir.to_string_lossy().to_string())
    }
}

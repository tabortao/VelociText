use crate::config::app_config::AppConfig;
use crate::models::language::Language;
use crate::AppState;
use serde::Serialize;
use std::io::Read;
use std::path::Path;
use tauri::{Emitter, State};

/// 获取应用配置
#[tauri::command]
pub async fn get_app_config(
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

/// 更新应用配置
#[tauri::command]
pub async fn set_app_config(
    new_config: AppConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    *config = new_config;
    log::info!("app config updated");
    Ok(())
}

/// 获取支持的语言列表
#[tauri::command]
pub async fn get_languages() -> Result<Vec<Language>, String> {
    Ok(Language::supported())
}

/// FFmpeg 下载进度事件
#[derive(Debug, Clone, Serialize)]
pub struct FfmpegDownloadProgress {
    pub stage: String,       // "downloading" | "extracting" | "completed" | "error"
    pub percentage: f64,
    pub message: String,
}

const FFMPEG_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.1-essentials_build.7z";
const FFMPEG_DIR_NAME: &str = "ffmpeg-8.1.1-essentials_build";

/// 下载并解压 FFmpeg，自动配置为项目 ffmpeg 路径
#[tauri::command]
pub async fn download_ffmpeg(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // 获取 app data 目录
    let app_data = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        Path::new(&config.model_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    };

    let ffmpeg_dir = app_data.join(FFMPEG_DIR_NAME);
    let ffmpeg_exe = ffmpeg_dir.join("bin").join("ffmpeg.exe");

    // 如果已经存在，直接返回路径
    if ffmpeg_exe.exists() {
        let exe_path = ffmpeg_exe.to_string_lossy().to_string();
        {
            let mut config = state.config.lock().map_err(|e| e.to_string())?;
            config.ffmpeg_path = Some(exe_path.clone());
        }
        let _ = app_handle.emit("ffmpeg-download-progress", FfmpegDownloadProgress {
            stage: "completed".into(),
            percentage: 100.0,
            message: "FFmpeg already installed".into(),
        });
        return Ok(exe_path);
    }

    let archive_path = app_data.join("ffmpeg.7z");

    // 下载阶段
    {
        let _ = app_handle.emit("ffmpeg-download-progress", FfmpegDownloadProgress {
            stage: "downloading".into(),
            percentage: 0.0,
            message: "Downloading FFmpeg...".into(),
        });

        let resp = ureq::get(FFMPEG_URL)
            .call()
            .map_err(|e| format!("Failed to download FFmpeg: {}", e))?;

        let total = resp
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let mut reader = resp.into_reader();
        let mut buf = [0u8; 65536];
        let mut downloaded: u64 = 0;
        let mut file = std::fs::File::create(&archive_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("Download interrupted: {}", e))?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n])
                .map_err(|e| format!("Write failed: {}", e))?;
            downloaded += n as u64;

            if total > 0 {
                let pct = (downloaded as f64 / total as f64) * 100.0;
                let _ = app_handle.emit("ffmpeg-download-progress", FfmpegDownloadProgress {
                    stage: "downloading".into(),
                    percentage: pct,
                    message: format!("Downloading... {:.1} MB / {:.1} MB",
                        downloaded as f64 / 1048576.0, total as f64 / 1048576.0),
                });
            }
        }
    }

    // 解压阶段
    {
        let _ = app_handle.emit("ffmpeg-download-progress", FfmpegDownloadProgress {
            stage: "extracting".into(),
            percentage: 0.0,
            message: "Extracting FFmpeg...".into(),
        });

        std::fs::create_dir_all(&ffmpeg_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        sevenz_rust::decompress_file(&archive_path, &app_data)
            .map_err(|e| format!("Failed to extract archive: {}", e))?;

        // Clean up archive
        let _ = std::fs::remove_file(&archive_path);
    }

    // 验证 ffmpeg.exe 存在
    if !ffmpeg_exe.exists() {
        return Err(format!(
            "Extraction completed but ffmpeg.exe not found at: {}",
            ffmpeg_exe.display()
        ));
    }

    // 自动配置 ffmpeg 路径
    let exe_path = ffmpeg_exe.to_string_lossy().to_string();
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.ffmpeg_path = Some(exe_path.clone());
    }

    let _ = app_handle.emit("ffmpeg-download-progress", FfmpegDownloadProgress {
        stage: "completed".into(),
        percentage: 100.0,
        message: "FFmpeg installed successfully!".into(),
    });

    Ok(exe_path)
}
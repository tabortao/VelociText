use crate::errors::{AppError, AppResult};
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows CREATE_NO_WINDOW flag - 防止弹出命令行窗口
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 支持的音频/视频格式
const SUPPORTED_FORMATS: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "aac", "m4a", "aiff", "caf",
    "mp4", "mov", "mkv", "webm",
];

/// 为 Command 设置后台运行（Windows 不弹黑窗）
fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd; // 非 Windows 不做任何事
}

/// 检查 FFmpeg 是否可用
pub fn check_ffmpeg() -> AppResult<String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-version");
    hide_window(&mut cmd);
    let output = cmd.output()
        .map_err(|_| AppError::FfmpegNotFound)?;

    if !output.status.success() {
        return Err(AppError::FfmpegNotFound);
    }

    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("ffmpeg")
        .to_string();
    Ok(version)
}

/// 检查文件格式是否支持
pub fn is_supported_format(file_path: &str) -> bool {
    let path = Path::new(file_path);
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        SUPPORTED_FORMATS.contains(&ext.as_str())
    } else {
        false
    }
}

/// 从音频/视频文件中提取音频并转为 16kHz Mono WAV
///
/// # Arguments
/// * `input_path` - 输入文件路径
/// * `output_path` - 输出 WAV 文件路径
pub fn extract_audio(input_path: &str, output_path: &str) -> AppResult<f64> {
    check_ffmpeg()?;

    // 获取音频时长
    let duration = get_audio_duration(input_path)?;

    // 使用 FFmpeg 转码为 16kHz mono WAV
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-i",
        input_path,
        "-vn",                 // 不要视频
        "-acodec",
        "pcm_s16le",           // 16-bit PCM
        "-ar",
        "16000",               // 16kHz 采样率
        "-ac",
        "1",                   // Mono
        "-f",
        "wav",
        output_path,
    ]);
    hide_window(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| AppError::AudioExtraction(format!("FFmpeg 执行失败: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::AudioExtraction(format!(
            "音频提取失败: {}",
            stderr
        )));
    }

    Ok(duration)
}

/// 获取音频文件时长（秒）
fn get_audio_duration(file_path: &str) -> AppResult<f64> {
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
        file_path,
    ]);
    hide_window(&mut cmd);
    let output = cmd.output()
        .map_err(|_| {
            // ffprobe 可能和 ffmpeg 一起安装，如果找不到则返回 0
            log::warn!("ffprobe not found, using 0 as duration");
        });

    match output {
        Ok(out) if out.status.success() => {
            let dur_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            dur_str.parse::<f64>().map_err(|_| {
                AppError::AudioExtraction("无法解析音频时长".into())
            })
        }
        _ => Ok(0.0), // 无法获取时长时返回 0
    }
}
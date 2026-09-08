use crate::errors::{AppError, AppResult};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows CREATE_NO_WINDOW flag - 防止弹出命令行窗口
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
    let output = cmd.output().map_err(|_| AppError::FfmpegNotFound)?;

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

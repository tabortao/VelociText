use crate::engine::audio_extractor::hide_window;
use crate::AppState;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

/// Supported output audio formats: (id, file extension, FFmpeg encoder args).
const AUDIO_FORMATS: &[(&str, &str, &[&str])] = &[
    ("mp3", "mp3", &["-c:a", "libmp3lame", "-q:a", "2"]),
    ("wav", "wav", &["-c:a", "pcm_s16le"]),
    ("flac", "flac", &["-c:a", "flac"]),
    ("m4a", "m4a", &["-c:a", "aac", "-b:a", "192k"]),
    ("ogg", "ogg", &["-c:a", "libvorbis", "-q:a", "5"]),
    ("opus", "opus", &["-c:a", "libopus", "-b:a", "128k"]),
    ("wma", "wma", &["-c:a", "wmav2", "-b:a", "192k"]),
];

/// Progress event payload sent to the frontend while a conversion runs.
#[derive(Debug, Clone, Serialize)]
pub struct AudioConvertProgress {
    /// Source file path this progress update belongs to.
    pub path: String,
    /// "probing" | "converting" | "done"
    pub stage: String,
    /// 0-100, or -1 while the total duration is still unknown.
    pub percentage: f64,
    pub message: String,
}

/// Convert a video/audio file to the given audio format with FFmpeg.
/// Emits `audio-convert-progress` events while running and returns the
/// output file path on success.
#[tauri::command]
pub async fn convert_to_audio(
    path: String,
    format: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (_, ext, encoder_args) = AUDIO_FORMATS
        .iter()
        .find(|(id, _, _)| *id == format)
        .ok_or_else(|| format!("Unsupported audio format: {format}"))?;

    if !Path::new(&path).is_file() {
        return Err(format!("File not found: {path}"));
    }

    // Resolve the FFmpeg executable: explicit config path first, then PATH.
    let ffmpeg_exe = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config
            .ffmpeg_path
            .clone()
            .filter(|p| !p.is_empty() && Path::new(p).is_file())
            .unwrap_or_else(|| "ffmpeg".to_string())
    };

    // Output: same folder and base name as the source, new extension.
    let input = Path::new(&path);
    let parent = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let output = parent.join(format!("{stem}.{ext}"));
    let output_str = output.to_string_lossy().to_string();

    if path_eq_ignore_case(&path, &output_str) {
        return Err("Output format is the same as the input file".to_string());
    }

    // Reset cancellation state and clear any stale child handle.
    state.audio_convert_cancel.store(false, Ordering::SeqCst);
    if let Ok(mut slot) = state.audio_convert_child.lock() {
        if let Some(old) = slot.as_mut() {
            let _ = old.kill();
        }
        *slot = None;
    }

    let cancel = Arc::clone(&state.audio_convert_cancel);
    let child_slot = Arc::clone(&state.audio_convert_child);
    let encoder: Vec<String> = encoder_args.iter().map(|s| s.to_string()).collect();
    let input_owned = path.clone();

    tokio::task::spawn_blocking(move || {
        run_conversion(
            &input_owned,
            &output_str,
            &ffmpeg_exe,
            &encoder,
            &cancel,
            &child_slot,
            &app_handle,
        )
    })
    .await
    .map_err(|e| format!("Conversion task failed: {e}"))?
}

/// Kill the running conversion (if any) and mark it cancelled.
#[tauri::command]
pub fn cancel_audio_convert(state: State<'_, AppState>) {
    state.audio_convert_cancel.store(true, Ordering::SeqCst);
    if let Ok(mut slot) = state.audio_convert_child.lock() {
        if let Some(child) = slot.as_mut() {
            let _ = child.kill();
        }
    }
}

/// List supported output audio format ids.
#[tauri::command]
pub fn get_audio_formats() -> Vec<String> {
    AUDIO_FORMATS
        .iter()
        .map(|(id, _, _)| id.to_string())
        .collect()
}

/// Blocking conversion worker: spawns FFmpeg, tracks progress via
/// `-progress pipe:1` and forwards it as events until the process exits.
fn run_conversion(
    input: &str,
    output: &str,
    ffmpeg_exe: &str,
    encoder_args: &[String],
    cancel: &Arc<AtomicBool>,
    child_slot: &Arc<Mutex<Option<Child>>>,
    app_handle: &AppHandle,
) -> Result<String, String> {
    let emit = |stage: &str, pct: f64, msg: &str| {
        let _ = app_handle.emit(
            "audio-convert-progress",
            AudioConvertProgress {
                path: input.to_string(),
                stage: stage.to_string(),
                percentage: pct,
                message: msg.to_string(),
            },
        );
    };

    emit("probing", -1.0, "");

    let mut cmd = Command::new(ffmpeg_exe);
    cmd.arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-vn")
        .args(encoder_args)
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg(output);
    hide_window(&mut cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg: {e}. Install FFmpeg or set its path in Settings."))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Expose the child so `cancel_audio_convert` can kill it.
    {
        let mut slot = child_slot.lock().map_err(|e| e.to_string())?;
        *slot = Some(child);
    }

    // Shared progress state, filled by the pipe reader threads.
    let duration_us = Arc::new(AtomicU64::new(0));
    let out_time_us = Arc::new(AtomicU64::new(0));
    let stdout_done = Arc::new(AtomicBool::new(false));
    let stderr_tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let mut readers = Vec::new();

    // stdout carries the `-progress` key=value stream.
    if let Some(pipe) = stdout {
        let out_time = Arc::clone(&out_time_us);
        let done = Arc::clone(&stdout_done);
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines() {
                let Ok(line) = line else { break };
                if let Some(us) = parse_out_time_line(&line) {
                    out_time.store(us, Ordering::Relaxed);
                }
            }
            done.store(true, Ordering::SeqCst);
        }));
    } else {
        stdout_done.store(true, Ordering::SeqCst);
    }

    // stderr carries the total duration and error details.
    if let Some(pipe) = stderr {
        let duration = Arc::clone(&duration_us);
        let tail = Arc::clone(&stderr_tail);
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines() {
                let Ok(line) = line else { break };
                if duration.load(Ordering::Relaxed) == 0 {
                    if let Some(us) = parse_duration_line(&line) {
                        duration.store(us, Ordering::Relaxed);
                    }
                }
                if let Ok(mut t) = tail.lock() {
                    t.push(line);
                    let excess = t.len().saturating_sub(30);
                    t.drain(..excess);
                }
            }
        }));
    }

    // Poll the shared progress state and forward it to the frontend.
    let mut last_pct = -1.0f64;
    loop {
        if stdout_done.load(Ordering::SeqCst) {
            break;
        }
        if cancel.load(Ordering::SeqCst) {
            if let Ok(mut slot) = child_slot.lock() {
                if let Some(c) = slot.as_mut() {
                    let _ = c.kill();
                }
            }
            break;
        }
        let dur = duration_us.load(Ordering::Relaxed);
        let cur = out_time_us.load(Ordering::Relaxed);
        if dur > 0 && cur > 0 {
            let pct = ((cur as f64 / dur as f64) * 100.0).clamp(0.0, 100.0);
            if (pct - last_pct).abs() >= 0.5 {
                emit("converting", pct, "");
                last_pct = pct;
            }
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    // Reap the process and collect its exit status.
    let status = {
        let mut slot = child_slot.lock().map_err(|e| e.to_string())?;
        match slot.take() {
            Some(mut c) => c
                .wait()
                .map_err(|e| format!("Failed to wait for FFmpeg: {e}"))?,
            None => return Err("Conversion process state lost".to_string()),
        }
    };

    for handle in readers {
        let _ = handle.join();
    }

    if !status.success() {
        if cancel.load(Ordering::SeqCst) {
            // Remove the partial output written before cancellation.
            let _ = std::fs::remove_file(output);
            return Err("cancelled".to_string());
        }
        let tail = stderr_tail.lock().map(|t| t.join("\n")).unwrap_or_default();
        return Err(format!(
            "FFmpeg failed (exit code {}):\n{}",
            status.code().unwrap_or(-1),
            tail
        ));
    }

    emit("done", 100.0, output);
    Ok(output.to_string())
}

/// Parse FFmpeg stderr lines like `  Duration: 00:03:45.23, start: ...`
/// into microseconds.
fn parse_duration_line(line: &str) -> Option<u64> {
    let rest = line.split("Duration:").nth(1)?;
    parse_hms(rest.split(',').next()?.trim())
}

/// Parse `-progress` stdout lines: prefer `out_time_us=`, fall back to
/// `out_time=HH:MM:SS.frac`.
fn parse_out_time_line(line: &str) -> Option<u64> {
    if let Some(v) = line.strip_prefix("out_time_us=") {
        return v.trim().parse::<u64>().ok();
    }
    if let Some(v) = line.strip_prefix("out_time=") {
        return parse_hms(v.trim());
    }
    None
}

/// Parse `HH:MM:SS.frac` into microseconds.
fn parse_hms(s: &str) -> Option<u64> {
    let mut parts = s.split(':');
    let h: f64 = parts.next()?.trim().parse().ok()?;
    let m: f64 = parts.next()?.trim().parse().ok()?;
    let sec: f64 = parts.next()?.trim().parse().ok()?;
    Some(((h * 3600.0 + m * 60.0 + sec) * 1_000_000.0) as u64)
}

fn path_eq_ignore_case(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.eq_ignore_ascii_case(b)
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

use crate::errors::AppResult;
use crate::models::task::TranscribeSegment;

/// 导出管理器 - 支持多种格式导出转录结果
///
/// SRT/VTT 直接使用 VAD 检测的原始分段 [start, end] 时间戳，
/// 保证字幕时间轴与音视频内容严格对应（不做二次拆句）。
pub struct ExportManager;

impl ExportManager {
    /// 导出为纯文本（分句分段，便于阅读）
    pub fn to_txt(segments: &[TranscribeSegment]) -> String {
        let mut result = String::new();
        for (i, seg) in segments.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            // 添加时间戳标注
            result.push_str(&format!(
                "[{:02}:{:02}.{:02}] {}\n",
                (seg.start as u32) / 60,
                (seg.start as u32) % 60,
                ((seg.start - seg.start.floor()) * 100.0) as u32,
                seg.text
            ));
        }
        result
    }

    /// 导出为 SRT 字幕格式（每个 VAD 分段独立时间段，时间轴与音视频一致）
    pub fn to_srt(segments: &[TranscribeSegment]) -> String {
        segments
            .iter()
            .enumerate()
            .map(|(i, seg)| {
                format!(
                    "{}\n{} --> {}\n{}\n",
                    i + 1,
                    Self::format_srt_time(seg.start),
                    Self::format_srt_time(seg.end),
                    seg.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 导出为 VTT 字幕格式（每个 VAD 分段独立时间段，时间轴与音视频一致）
    pub fn to_vtt(segments: &[TranscribeSegment]) -> String {
        let body = segments
            .iter()
            .map(|seg| {
                format!(
                    "{} --> {}\n{}",
                    Self::format_vtt_time(seg.start),
                    Self::format_vtt_time(seg.end),
                    seg.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        format!("WEBVTT\n\n{}", body)
    }

    /// 根据格式导出
    pub fn export(segments: &[TranscribeSegment], format: &str) -> AppResult<String> {
        match format.to_lowercase().as_str() {
            "txt" => Ok(Self::to_txt(segments)),
            "srt" => Ok(Self::to_srt(segments)),
            "vtt" => Ok(Self::to_vtt(segments)),
            _ => Err(crate::errors::AppError::Export(format!(
                "不支持的导出格式: {}",
                format
            ))),
        }
    }

    /// 格式化 SRT 时间戳: HH:MM:SS,mmm
    fn format_srt_time(seconds: f64) -> String {
        let hours = (seconds / 3600.0) as u32;
        let minutes = ((seconds % 3600.0) / 60.0) as u32;
        let secs = (seconds % 60.0) as u32;
        let millis = ((seconds - seconds.floor()) * 1000.0) as u32;
        format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
    }

    /// 格式化 VTT 时间戳: HH:MM:SS.mmm
    fn format_vtt_time(seconds: f64) -> String {
        let hours = (seconds / 3600.0) as u32;
        let minutes = ((seconds % 3600.0) / 60.0) as u32;
        let secs = (seconds % 60.0) as u32;
        let millis = ((seconds - seconds.floor()) * 1000.0) as u32;
        format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_time_format() {
        assert_eq!(ExportManager::format_srt_time(0.0), "00:00:00,000");
        assert_eq!(ExportManager::format_srt_time(3661.5), "01:01:01,500");
    }

    #[test]
    fn test_vtt_time_format() {
        assert_eq!(ExportManager::format_vtt_time(0.0), "00:00:00.000");
    }

    #[test]
    fn test_export_txt() {
        let segments = vec![
            TranscribeSegment {
                start: 1.5,
                end: 2.5,
                text: "你好".into(),
            },
            TranscribeSegment {
                start: 3.0,
                end: 5.0,
                text: "世界".into(),
            },
        ];
        let txt = ExportManager::to_txt(&segments);
        assert!(txt.contains("[00:01.50] 你好"));
        assert!(txt.contains("[00:03.00] 世界"));
    }

    #[test]
    fn test_export_srt() {
        let segments = vec![TranscribeSegment {
            start: 0.0,
            end: 1.5,
            text: "你好世界".into(),
        }];
        let srt = ExportManager::to_srt(&segments);
        assert!(srt.contains("00:00:00,000 --> 00:00:01,500"));
        assert!(srt.contains("你好世界"));
    }

    #[test]
    fn test_srt_uses_original_segment_timestamps() {
        // 两个分段的时间戳原样输出，不做拆句
        let segments = vec![
            TranscribeSegment {
                start: 0.0,
                end: 6.0,
                text: "第一句话。第二句话。".into(),
            },
            TranscribeSegment {
                start: 7.5,
                end: 9.0,
                text: "第三句话。".into(),
            },
        ];
        let srt = ExportManager::to_srt(&segments);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:06,000\n第一句话。第二句话。"));
        assert!(srt.contains("2\n00:00:07,500 --> 00:00:09,000\n第三句话。"));
    }

    #[test]
    fn test_export_vtt() {
        let segments = vec![TranscribeSegment {
            start: 1.0,
            end: 2.5,
            text: "Hello".into(),
        }];
        let vtt = ExportManager::to_vtt(&segments);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.500\nHello"));
    }
}

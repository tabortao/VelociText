use crate::errors::AppResult;
use crate::models::task::TranscribeSegment;

/// 智能分句 - 基于标点符号将长文本拆分为句子
///
/// 支持中英文标点：。！？；，、 . ! ? ; ,
/// 对于纯文本（无时间信息），均匀分配时间跨度
pub fn segment_text(text: &str, total_duration: f64) -> Vec<TranscribeSegment> {
    if text.trim().is_empty() {
        return vec![];
    }

    let mut segments = Vec::new();
    let mut current = String::new();
    let mut char_count = 0;
    let total_chars = text.chars().count() as f64;
    let mut segment_start_char = 0usize;

    for ch in text.chars() {
        current.push(ch);
        char_count += 1;

        // 句子结束标点
        let is_sentence_end = matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '\n');
        // 短停顿标点（逗号等），如果积累够了也分段
        let is_clause_end = matches!(ch, '；' | '，' | ',' | ';');

        let should_split = is_sentence_end
            || (is_clause_end && current.chars().count() >= 15);

        if should_split {
            let text = current.trim().to_string();
            if !text.is_empty() {
                let start = (segment_start_char as f64 / total_chars) * total_duration;
                let end = (char_count as f64 / total_chars) * total_duration;
                segments.push(TranscribeSegment { start, end, text });
            }
            segment_start_char = char_count;
            current.clear();
        }
    }

    // 剩余内容
    let remaining = current.trim().to_string();
    if !remaining.is_empty() {
        let start = (segment_start_char as f64 / total_chars) * total_duration;
        segments.push(TranscribeSegment {
            start,
            end: total_duration,
            text: remaining,
        });
    }

    // 如果分句后没有结果（全是无标点连续文本），返回整段
    if segments.is_empty() {
        segments.push(TranscribeSegment {
            start: 0.0,
            end: total_duration,
            text: text.trim().to_string(),
        });
    }

    segments
}

/// 导出管理器 - 支持多种格式导出转录结果
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

    /// 导出为 SRT 字幕格式
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

    /// 导出为 VTT 字幕格式
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
            TranscribeSegment { start: 1.5, end: 2.5, text: "你好".into() },
            TranscribeSegment { start: 3.0, end: 5.0, text: "世界".into() },
        ];
        let txt = ExportManager::to_txt(&segments);
        assert!(txt.contains("[00:01.50] 你好"));
        assert!(txt.contains("[00:03.00] 世界"));
    }

    #[test]
    fn test_export_srt() {
        let segments = vec![
            TranscribeSegment { start: 0.0, end: 1.5, text: "你好世界".into() },
        ];
        let srt = ExportManager::to_srt(&segments);
        assert!(srt.contains("00:00:00,000 --> 00:00:01,500"));
        assert!(srt.contains("你好世界"));
    }
}
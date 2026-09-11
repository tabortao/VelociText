use crate::errors::AppResult;
use crate::models::task::TranscribeSegment;

/// 句末标点（中英文），用于将分段文本拆分为单句字幕
const SENTENCE_ENDS: &[char] = &['。', '！', '？', '；', '…', '!', '?', ';', '】', '」', '』'];
/// 句末标点后的收尾字符（引号/括号等），拆句时跟随前一句
const TRAILING_CLOSE: &[char] = &['"', '\'', '”', '’', '）', ')', '〉', '》', '】', '」', '』'];

/// 判断字符是否为句末标点。
/// 英文句点仅在后面跟空白或结尾时视为句末（避免拆开小数、缩写）。
fn is_sentence_end(chars: &[char], idx: usize) -> bool {
    let c = chars[idx];
    if SENTENCE_ENDS.contains(&c) {
        return true;
    }
    if c == '.' {
        return idx + 1 >= chars.len() || chars[idx + 1].is_whitespace();
    }
    false
}

/// 单句字符权重（用于时间比例分配）：全角字符按 1.0 计，半角字符按 0.5 计
fn sentence_weight(text: &str) -> f64 {
    text.chars()
        .map(|c| if c.is_ascii() { 0.5 } else { 1.0 })
        .sum()
}

/// 将一个分段按句末标点拆分为多个单句分段。
///
/// 时间按字符权重比例分配（中文全角字符权重是 ASCII 的两倍），
/// 每句话获得独立的 [start, end] 时间段。无句末标点时原样返回。
fn split_segment_by_sentence(seg: &TranscribeSegment) -> Vec<TranscribeSegment> {
    let text = seg.text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        current.push(c);

        if is_sentence_end(&chars, i) {
            // 吸收紧随其后的更多句末标点（如 "？！"、"。。。"）和收尾引号/括号
            let mut j = i + 1;
            while j < chars.len()
                && (SENTENCE_ENDS.contains(&chars[j])
                    || chars[j] == '.'
                    || TRAILING_CLOSE.contains(&chars[j]))
            {
                current.push(chars[j]);
                j += 1;
            }
            // 英文句点后的空格也吸收进当前句，避免下一句以空格开头
            if j < chars.len() && chars[j].is_whitespace() && current.ends_with('.') {
                current.push(chars[j]);
                j += 1;
            }
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            current.clear();
            i = j;
        } else {
            i += 1;
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }

    if sentences.len() <= 1 {
        return vec![TranscribeSegment {
            start: seg.start,
            end: seg.end,
            text: text.to_string(),
        }];
    }

    // 按字符权重比例分配时间段
    let total_weight: f64 = sentences.iter().map(|s| sentence_weight(s)).sum();
    let duration = (seg.end - seg.start).max(0.0);
    let mut result = Vec::with_capacity(sentences.len());
    let mut cursor = seg.start;

    for (i, sentence) in sentences.iter().enumerate() {
        let is_last = i == sentences.len() - 1;
        let end = if is_last {
            seg.end
        } else {
            cursor + duration * (sentence_weight(sentence) / total_weight)
        };
        result.push(TranscribeSegment {
            start: cursor,
            end,
            text: sentence.clone(),
        });
        cursor = end;
    }

    result
}

/// 将所有分段拆分为句子级分段（用于字幕导出）
pub fn split_into_sentences(segments: &[TranscribeSegment]) -> Vec<TranscribeSegment> {
    segments
        .iter()
        .flat_map(split_segment_by_sentence)
        .collect()
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

    /// 导出为 SRT 字幕格式（每句话独立时间段）
    pub fn to_srt(segments: &[TranscribeSegment]) -> String {
        let sentences = split_into_sentences(segments);
        sentences
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

    /// 导出为 VTT 字幕格式（每句话独立时间段）
    pub fn to_vtt(segments: &[TranscribeSegment]) -> String {
        let sentences = split_into_sentences(segments);
        let body = sentences
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
    fn test_split_by_sentence_chinese() {
        let segments = vec![TranscribeSegment {
            start: 0.0,
            end: 10.0,
            text: "今天天气很好。我们一起去公园吧！好吗？".into(),
        }];
        let split = split_into_sentences(&segments);
        assert_eq!(split.len(), 3);
        assert_eq!(split[0].text, "今天天气很好。");
        assert_eq!(split[1].text, "我们一起去公园吧！");
        assert_eq!(split[2].text, "好吗？");
        // 时间段首尾相接、覆盖整个分段
        assert_eq!(split[0].start, 0.0);
        assert_eq!(split[2].end, 10.0);
        assert_eq!(split[0].end, split[1].start);
        assert_eq!(split[1].end, split[2].start);
    }

    #[test]
    fn test_split_by_sentence_english() {
        let segments = vec![TranscribeSegment {
            start: 5.0,
            end: 15.0,
            text: "Hello world. This is a test! The price is 3.14 dollars.".into(),
        }];
        let split = split_into_sentences(&segments);
        assert_eq!(split.len(), 3);
        assert_eq!(split[0].text, "Hello world.");
        assert_eq!(split[1].text, "This is a test!");
        // 3.14 不应被拆开
        assert_eq!(split[2].text, "The price is 3.14 dollars.");
        assert_eq!(split[0].start, 5.0);
        assert_eq!(split[2].end, 15.0);
    }

    #[test]
    fn test_split_by_sentence_no_punctuation() {
        let segments = vec![TranscribeSegment {
            start: 1.0,
            end: 2.0,
            text: "没有句末标点的文本".into(),
        }];
        let split = split_into_sentences(&segments);
        assert_eq!(split.len(), 1);
        assert_eq!(split[0].text, "没有句末标点的文本");
        assert_eq!(split[0].start, 1.0);
        assert_eq!(split[0].end, 2.0);
    }

    #[test]
    fn test_split_by_sentence_consecutive_punct() {
        let segments = vec![TranscribeSegment {
            start: 0.0,
            end: 4.0,
            text: "真的吗？！我不信。".into(),
        }];
        let split = split_into_sentences(&segments);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].text, "真的吗？！");
        assert_eq!(split[1].text, "我不信。");
    }

    #[test]
    fn test_srt_exports_sentence_level() {
        let segments = vec![TranscribeSegment {
            start: 0.0,
            end: 6.0,
            text: "第一句话。第二句话。".into(),
        }];
        let srt = ExportManager::to_srt(&segments);
        // 两句各自独立的序号与时间段
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:03,000\n第一句话。"));
        assert!(srt.contains("2\n00:00:03,000 --> 00:00:06,000\n第二句话。"));
    }
}

use crate::engine::audio_extractor::extract_audio;
use crate::engine::progress::ProgressTracker;
use crate::engine::recognizer::Recognizer;
use crate::errors::AppResult;
use crate::models::task::TranscribeSegment;
use tempfile::NamedTempFile;

/// 转录器 - 协调音频提取和语音识别
pub struct Transcriber {
    recognizer: Recognizer,
}

impl Transcriber {
    pub fn new() -> Self {
        Self {
            recognizer: Recognizer::new(),
        }
    }

    /// 确保模型已加载
    pub fn ensure_model(&self, model_path: &str) -> AppResult<()> {
        if !self.recognizer.is_loaded() {
            self.recognizer.load_sense_voice(model_path)?;
        }
        Ok(())
    }

    /// 转录音频/视频文件
    ///
    /// 流程：提取音频 -> 识别 -> 返回分段结果
    pub fn transcribe_file(
        &self,
        file_path: &str,
        progress: &ProgressTracker,
    ) -> AppResult<(Vec<TranscribeSegment>, f64)> {
        // 1. 提取音频为 16kHz WAV
        let temp_wav = NamedTempFile::new()?;
        let wav_path = temp_wav.path().to_string_lossy().to_string();

        let duration = extract_audio(file_path, &wav_path)?;

        // 2. 执行语音识别
        let text = self.recognizer.recognize_wav(&wav_path)?;

        // 3. 构建结果分段（简化为单段，后续可集成 VAD 进行精细分段）
        let segments = vec![TranscribeSegment {
            start: 0.0,
            end: duration,
            text,
        }];

        progress.set_progress(100);
        Ok((segments, duration))
    }

    pub fn recognizer(&self) -> &Recognizer {
        &self.recognizer
    }
}

impl Default for Transcriber {
    fn default() -> Self {
        Self::new()
    }
}

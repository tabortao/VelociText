use crate::errors::{AppError, AppResult};
use sherpa_onnx::{
    OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    Wave,
};
use std::path::Path;
use std::sync::Mutex;

/// Recognizer 封装 - 管理 sherpa-onnx 识别器实例
pub struct Recognizer {
    inner: Mutex<Option<OfflineRecognizer>>,
}

impl Recognizer {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// 加载 SenseVoice-Small 模型
    ///
    /// 模型目录中应包含:
    /// - model.onnx 或 model.int8.onnx (模型文件)
    /// - tokens.txt (词表文件)
    pub fn load_sense_voice(&self, model_path: &str) -> AppResult<()> {
        let base = Path::new(model_path);

        // 支持多种模型文件名: model.onnx, model.int8.onnx, model_q8.onnx
        let model_file = ["model.onnx", "model.int8.onnx", "model_q8.onnx"]
            .iter()
            .map(|name| base.join(name))
            .find(|p| p.exists())
            .ok_or_else(|| {
                AppError::ModelLoad(format!(
                    "模型文件不存在 (需要 model.onnx 或 model.int8.onnx): {}",
                    base.display()
                ))
            })?;

        let tokens_file = base.join("tokens.txt");
        if !tokens_file.exists() {
            return Err(AppError::ModelLoad(format!(
                "tokens 文件不存在: {}",
                tokens_file.display()
            )));
        }

        let config = OfflineRecognizerConfig {
            model_config: OfflineModelConfig {
                sense_voice: OfflineSenseVoiceModelConfig {
                    model: Some(model_file.to_string_lossy().to_string()),
                    ..Default::default()
                },
                tokens: Some(tokens_file.to_string_lossy().to_string()),
                num_threads: 4,
                ..Default::default()
            },
            ..Default::default()
        };

        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| AppError::ModelLoad("无法创建识别器".into()))?;

        let mut inner = self
            .inner
            .lock()
            .map_err(|e| AppError::ModelLoad(format!("锁失败: {}", e)))?;
        *inner = Some(recognizer);

        log::info!("SenseVoice-Small 模型加载成功");
        Ok(())
    }

    /// 对 WAV 音频进行识别
    pub fn recognize_wav(&self, wav_path: &str) -> AppResult<String> {
        let inner = self
            .inner
            .lock()
            .map_err(|e| AppError::Transcription(format!("锁失败: {}", e)))?;

        let recognizer = inner
            .as_ref()
            .ok_or_else(|| AppError::Transcription("识别器未初始化".into()))?;

        let audio = Wave::read(wav_path)
            .ok_or_else(|| AppError::Transcription(format!("无法读取音频文件: {}", wav_path)))?;

        let stream = recognizer.create_stream();
        stream.accept_waveform(audio.sample_rate(), audio.samples());

        // 离线识别：接受完音频后，调用一次 decode 即可得到结果
        recognizer.decode(&stream);

        let result = stream.get_result().map(|r| r.text).unwrap_or_default();

        Ok(result)
    }

    /// 检查模型是否已加载
    pub fn is_loaded(&self) -> bool {
        self.inner
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}

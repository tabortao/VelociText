use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineModelConfig, OfflineParaformerModelConfig, OfflineRecognizer,
    OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    OfflineTransducerModelConfig, OfflineZipformerCtcModelConfig,
};
use std::path::Path;

/// Supported model types for the multi-model architecture.
///
/// New model types can be added here to extend recognition support
/// without changing the core pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelType {
    /// SenseVoice series (Small, Large) — multilingual ASR
    SenseVoice,
    /// Paraformer — Mandarin ASR
    Paraformer,
    /// Zipformer CTC — English/Multilingual ASR with CTC
    ZipformerCtc,
    /// Transducer (e.g., Zipformer transducer, Conformer) — general ASR
    Transducer,
}

impl ModelType {
    /// Returns the model directory name used in the filesystem.
    pub fn dir_name(&self) -> &str {
        match self {
            ModelType::SenseVoice => "sense-voice-small",
            ModelType::Paraformer => "paraformer",
            ModelType::ZipformerCtc => "zipformer-ctc",
            ModelType::Transducer => "transducer",
        }
    }

    /// Returns candidate model file names to search for.
    pub fn model_file_candidates(&self) -> &[&str] {
        match self {
            ModelType::SenseVoice => &["model.onnx", "model.int8.onnx", "model_q8.onnx"],
            ModelType::Paraformer => &["model.onnx", "model.int8.onnx"],
            ModelType::ZipformerCtc => &["model.onnx", "model.int8.onnx"],
            ModelType::Transducer => &["encoder.onnx", "decoder.onnx", "joiner.onnx"],
        }
    }

    /// Human-readable display name.
    pub fn display_name(&self) -> &str {
        match self {
            ModelType::SenseVoice => "SenseVoice-Small",
            ModelType::Paraformer => "Paraformer",
            ModelType::ZipformerCtc => "Zipformer CTC",
            ModelType::Transducer => "Transducer",
        }
    }
}

/// Configuration for creating a recognizer.
pub struct RecognizerConfig {
    /// Path to the model directory (contains model files + tokens.txt).
    pub model_dir: String,
    /// Number of CPU threads for inference.
    pub num_threads: u32,
    /// Optional hotwords file path for boosting specific terms.
    pub hotwords_file: Option<String>,
    /// Hotwords boosting score (default: 1.5).
    pub hotwords_score: f32,
    /// Whether to enable ITN (Inverse Text Normalization).
    pub use_itn: bool,
}

impl Default for RecognizerConfig {
    fn default() -> Self {
        Self {
            model_dir: String::new(),
            num_threads: 4,
            hotwords_file: None,
            hotwords_score: 1.5,
            use_itn: true,
        }
    }
}

/// Factory for creating sherpa-onnx recognizer instances.
///
/// Supports multiple model architectures via the `ModelType` enum.
/// This is the central abstraction for the P2 multi-model architecture.
pub struct RecognizerFactory;

impl RecognizerFactory {
    /// Create a recognizer for the given model type with the provided config.
    ///
    /// Automatically discovers model files based on the model type's candidates.
    pub fn create(model_type: &ModelType, config: &RecognizerConfig) -> AppResult<OfflineRecognizer> {
        let base = Path::new(&config.model_dir);

        // Find the first matching model file
        let model_file = model_type
            .model_file_candidates()
            .iter()
            .map(|name| base.join(name))
            .find(|p| p.exists())
            .ok_or_else(|| {
                AppError::ModelLoad(format!(
                    "{} model file not found in: {}",
                    model_type.display_name(),
                    base.display()
                ))
            })?;

        let tokens_file = base.join("tokens.txt");
        if !tokens_file.exists() {
            return Err(AppError::ModelLoad(format!(
                "tokens.txt not found in: {}",
                base.display()
            )));
        }

        log::info!(
            "Creating recognizer: type={}, model={}, tokens={}",
            model_type.display_name(),
            model_file.display(),
            tokens_file.display()
        );

        let model_config = Self::build_model_config(model_type, &model_file, config);

        let recognizer_config = OfflineRecognizerConfig {
            model_config,
            hotwords_file: config.hotwords_file.clone(),
            hotwords_score: config.hotwords_score,
            ..Default::default()
        };

        OfflineRecognizer::create(&recognizer_config)
            .ok_or_else(|| AppError::ModelLoad(format!(
                "Failed to create {} recognizer",
                model_type.display_name()
            )))
    }

    /// Build the model-specific configuration.
    fn build_model_config(model_type: &ModelType, model_file: &Path, config: &RecognizerConfig) -> OfflineModelConfig {
        let model_path = model_file.to_string_lossy().to_string();
        let parent = model_file.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let tokens_path = Path::new(&parent).join("tokens.txt").to_string_lossy().to_string();

        match model_type {
            ModelType::SenseVoice => OfflineModelConfig {
                sense_voice: OfflineSenseVoiceModelConfig {
                    model: Some(model_path),
                    use_itn: config.use_itn,
                    ..Default::default()
                },
                tokens: Some(tokens_path),
                num_threads: config.num_threads as i32,
                ..Default::default()
            },

            ModelType::Paraformer => OfflineModelConfig {
                paraformer: OfflineParaformerModelConfig {
                    model: Some(model_path),
                    ..Default::default()
                },
                tokens: Some(tokens_path),
                num_threads: config.num_threads as i32,
                ..Default::default()
            },

            ModelType::ZipformerCtc => OfflineModelConfig {
                zipformer_ctc: OfflineZipformerCtcModelConfig {
                    model: Some(model_path),
                    ..Default::default()
                },
                tokens: Some(tokens_path),
                num_threads: config.num_threads as i32,
                ..Default::default()
            },

            ModelType::Transducer => {
                // Transducer uses encoder/decoder/joiner triplet
                let base = model_file.parent().unwrap_or(Path::new("."));
                OfflineModelConfig {
                    transducer: OfflineTransducerModelConfig {
                        encoder: Some(base.join("encoder.onnx").to_string_lossy().to_string()),
                        decoder: Some(base.join("decoder.onnx").to_string_lossy().to_string()),
                        joiner: Some(base.join("joiner.onnx").to_string_lossy().to_string()),
                        ..Default::default()
                    },
                    tokens: Some(tokens_path),
                    num_threads: config.num_threads as i32,
                    ..Default::default()
                }
            },
        }
    }

    /// List available model directories by scanning the base models path.
    pub fn list_available(base_models_dir: &str) -> Vec<String> {
        let dir = Path::new(base_models_dir);
        if !dir.exists() {
            return vec![];
        }

        let all_types = [
            ModelType::SenseVoice,
            ModelType::Paraformer,
            ModelType::ZipformerCtc,
            ModelType::Transducer,
        ];

        all_types
            .iter()
            .filter(|mt| {
                let model_dir = dir.join(mt.dir_name());
                model_dir.exists()
                    && mt.model_file_candidates()
                        .iter()
                        .any(|name| model_dir.join(name).exists())
            })
            .map(|mt| mt.dir_name().to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_dir_names() {
        assert_eq!(ModelType::SenseVoice.dir_name(), "sense-voice-small");
        assert_eq!(ModelType::Paraformer.dir_name(), "paraformer");
        assert_eq!(ModelType::ZipformerCtc.dir_name(), "zipformer-ctc");
        assert_eq!(ModelType::Transducer.dir_name(), "transducer");
    }

    #[test]
    fn test_model_type_display_names() {
        assert_eq!(ModelType::SenseVoice.display_name(), "SenseVoice-Small");
        assert_eq!(ModelType::Paraformer.display_name(), "Paraformer");
    }

    #[test]
    fn test_config_defaults() {
        let config = RecognizerConfig::default();
        assert_eq!(config.num_threads, 4);
        assert_eq!(config.hotwords_score, 1.5);
        assert!(config.use_itn);
        assert!(config.hotwords_file.is_none());
    }
}
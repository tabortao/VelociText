//! Integration tests for the transcription and OCR pipeline.
//!
//! Run with: cargo test --lib -- transcribe_integration --nocapture
//! Run OCR:   cargo test --lib -- ocr_integration --nocapture

#[cfg(test)]
mod transcribe_integration {
    use crate::engine::model_manager::is_model_installed_at;
    use crate::engine::recognizer_factory::{ModelType, RecognizerConfig, RecognizerFactory};
    use crate::engine::transcription_pipeline::run_recognition;
    use sherpa_onnx::{SileroVadModelConfig, VadModelConfig};
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicU32};
    use std::sync::{Arc, Mutex};

    /// Test the full streaming pipeline (symphonia decode → VAD → ASR),
    /// mirroring the production `recognize_file` code path.
    #[test]
    fn test_streaming_transcribe_mp3_with_sense_voice() {
        // Locate test file
        let test_mp3 = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/test2.mp3"));

        if !test_mp3.exists() {
            eprintln!("SKIP: test2.mp3 not found at {:?}", test_mp3);
            return;
        }

        // Locate model
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let models_dir = Path::new(&appdata).join("VelociText/models");
        let model_dir = models_dir.join("sense-voice-small");

        if !is_model_installed_at(&model_dir) {
            eprintln!("SKIP: SenseVoice-Small model not installed at {:?}", model_dir);
            return;
        }

        // 1. Create recognizer via factory
        let factory_config = RecognizerConfig {
            model_dir: model_dir.to_string_lossy().to_string(),
            num_threads: 4,
            hotwords_file: None,
            hotwords_score: 1.5,
            use_itn: true,
        };

        let recognizer = RecognizerFactory::create(&ModelType::SenseVoice, &factory_config)
            .expect("Failed to create SenseVoice recognizer");

        // 2. Create Silero VAD (same config as production)
        let vad_model_path = models_dir.join("silero-vad").join("model.onnx");
        if !vad_model_path.exists() {
            eprintln!("SKIP: silero-vad model not installed at {:?}", vad_model_path);
            return;
        }
        let vad_config = VadModelConfig {
            silero_vad: SileroVadModelConfig {
                model: Some(vad_model_path.to_string_lossy().to_string()),
                threshold: 0.2,
                min_silence_duration: 0.2,
                min_speech_duration: 0.2,
                window_size: 512,
                max_speech_duration: 10.0,
                ..Default::default()
            },
            sample_rate: 16000,
            num_threads: 1,
            ..Default::default()
        };
        let vad = sherpa_onnx::VoiceActivityDetector::create(&vad_config, 120.0)
            .expect("Failed to create Silero VAD");

        // 3. Run the streaming pipeline on the original mp3
        let recognizer = Arc::new(Mutex::new(Some(recognizer)));
        let vad = Arc::new(Mutex::new(Some(vad)));
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU32::new(0));
        let segments = Arc::new(Mutex::new(Vec::new()));

        let duration = run_recognition(
            test_mp3.to_str().unwrap(),
            &recognizer,
            &vad,
            &cancelled,
            &progress,
            &segments,
        )
        .expect("Streaming recognition failed");

        assert!(duration > 0.0, "Audio duration should be > 0, got {duration}");

        let results = segments.lock().unwrap();
        let text: String = results.iter().map(|s| s.text.as_str()).collect();

        println!("  Duration: {:.2}s, segments: {}", duration, results.len());
        println!("  Recognized: {}", text);

        assert!(
            !text.trim().is_empty(),
            "Recognition returned empty text — model or audio may be invalid"
        );

        println!("  PASS: Full streaming transcription pipeline works!");
    }
}

#[cfg(test)]
mod ocr_integration {
    use crate::engine::ocr::OcrEngine;
    use std::path::Path;

    /// Test OCR on docs/demo.png with each model version.
    /// Expected text: "根据项目初设阶段图纸情况得分情况，本项目预评价得分为72.3分；"
    #[test]
    fn test_ocr_demo_v4() {
        test_ocr_demo("ppocr-v4");
    }

    #[test]
    fn test_ocr_demo_v5() {
        test_ocr_demo("ppocr-v5");
    }

    #[test]
    fn test_ocr_demo_v6() {
        test_ocr_demo("ppocr-v6");
    }

    fn test_ocr_demo(model_version: &str) {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let model_dir = Path::new(&appdata)
            .join("VelociText/models")
            .join(model_version);
        let demo_image = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/demo.png"));

        if !model_dir.exists() {
            eprintln!("SKIP [{model_version}]: model not installed at {model_dir:?}");
            return;
        }
        if !demo_image.exists() {
            eprintln!("SKIP [{model_version}]: demo.png not found at {demo_image:?}");
            return;
        }

        let mut engine = OcrEngine::new_with_memory(&model_dir, false)
            .expect(&format!("[{model_version}] Failed to init OCR engine"));

        let result = engine
            .recognize_from_path(demo_image)
            .expect(&format!("[{model_version}] OCR recognition failed"));

        let full_text: String = result
            .text_blocks
            .iter()
            .map(|b| b.text.clone())
            .collect::<Vec<_>>()
            .join("");

        println!("[{model_version}] Recognized text: {}", full_text);
        println!("[{model_version}] Blocks: {}", result.text_blocks.len());

        for (i, block) in result.text_blocks.iter().enumerate() {
            println!(
                "  [{model_version}] block[{i}]: text=\"{}\", confidence={:.4}",
                block.text, block.confidence
            );
        }

        assert!(
            !full_text.trim().is_empty(),
            "[{model_version}] OCR returned empty text"
        );

        // Check for expected keywords
        let has_keywords = full_text.contains("项目") || full_text.contains("预评价");
        if !has_keywords {
            eprintln!(
                "[{model_version}] WARNING: Expected text contains '项目' or '预评价', got: {}",
                full_text
            );
        }
    }
}

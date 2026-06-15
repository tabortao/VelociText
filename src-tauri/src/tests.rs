//! Integration tests for the transcription and OCR pipeline.
//!
//! Run with: cargo test --lib -- transcribe_integration --nocapture
//! Run OCR:   cargo test --lib -- ocr_integration --nocapture

#[cfg(test)]
mod transcribe_integration {
    use crate::engine::audio_extractor::extract_audio;
    use crate::engine::model_manager::is_model_installed_at;
    use crate::engine::recognizer_factory::{ModelType, RecognizerConfig, RecognizerFactory};
    use std::path::Path;

    #[test]
    fn test_transcribe_mp3_with_sense_voice() {
        // Locate test file
        let test_mp3 = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/test2.mp3"));

        if !test_mp3.exists() {
            eprintln!("SKIP: test2.mp3 not found at {:?}", test_mp3);
            return;
        }

        // Locate model
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let model_dir = Path::new(&appdata).join("VelociText/models/sense-voice-small");

        if !is_model_installed_at(&model_dir) {
            eprintln!(
                "SKIP: SenseVoice-Small model not installed at {:?}",
                model_dir
            );
            return;
        }

        // 1. Extract audio via FFmpeg
        let temp_wav = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        let wav_path = temp_wav.path().to_string_lossy().to_string();

        let duration = extract_audio(test_mp3.to_str().unwrap(), &wav_path)
            .expect("FFmpeg audio extraction failed");

        assert!(
            duration > 0.0,
            "Audio duration should be > 0, got {}",
            duration
        );
        println!("  Duration: {:.2}s", duration);

        // 2. Create recognizer via factory
        let factory_config = RecognizerConfig {
            model_dir: model_dir.to_string_lossy().to_string(),
            num_threads: 4,
            hotwords_file: None,
            hotwords_score: 1.5,
            use_itn: true,
        };

        let recognizer = RecognizerFactory::create(&ModelType::SenseVoice, &factory_config)
            .expect("Failed to create SenseVoice recognizer");
        println!("  Recognizer created successfully");

        // 3. Run recognition
        let audio = sherpa_onnx::Wave::read(&wav_path).expect("Failed to read WAV file");
        let stream = recognizer.create_stream();
        stream.accept_waveform(audio.sample_rate(), audio.samples());
        recognizer.decode(&stream);

        let text = stream.get_result().map(|r| r.text).unwrap_or_default();
        println!("  Recognized: {}", text);

        assert!(
            !text.trim().is_empty(),
            "Recognition returned empty text — model or audio may be invalid"
        );

        // 4. Test VAD segmentation on the same file
        let model_path =
            Path::new(&appdata).join("VelociText/models/sense-voice-small/silero_vad.onnx");
        let segments =
            crate::engine::vad::detect_speech_segments(&wav_path, model_path.to_str().unwrap())
                .expect("VAD detection failed");
        println!("  VAD segments detected: {}", segments.0.len());

        println!("  PASS: Full transcription pipeline works!");
    }

    #[test]
    fn test_vad_config_defaults() {
        // Verify Silero VAD model creation works with default config
        let result = crate::engine::vad::create_silero_vad("nonexistent.onnx");
        // Should fail because model file doesn't exist, but the config is valid
        assert!(result.is_err(), "Should fail with nonexistent model file");
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

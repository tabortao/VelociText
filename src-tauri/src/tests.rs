//! Integration tests for the transcription pipeline.
//!
//! Run with: cargo test --lib -- transcribe_integration --nocapture

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
            eprintln!("SKIP: SenseVoice-Small model not installed at {:?}", model_dir);
            return;
        }

        // 1. Extract audio via FFmpeg
        let temp_wav = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        let wav_path = temp_wav.path().to_string_lossy().to_string();

        let duration = extract_audio(
            test_mp3.to_str().unwrap(),
            &wav_path,
        )
        .expect("FFmpeg audio extraction failed");

        assert!(duration > 0.0, "Audio duration should be > 0, got {}", duration);
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
        let audio = sherpa_onnx::Wave::read(&wav_path)
            .expect("Failed to read WAV file");
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
        let vad_config = crate::engine::vad::VadConfig::default();
        let segments = crate::engine::vad::detect_speech_segments(&wav_path, &vad_config)
            .expect("VAD detection failed");
        println!("  VAD segments detected: {}", segments.len());

        println!("  PASS: Full transcription pipeline works!");
    }

    #[test]
    fn test_vad_with_synthetic_data() {
        // Test VAD module with basic assertions
        let config = crate::engine::vad::VadConfig::default();
        assert!((config.window_ms - 30.0).abs() < 0.01);
        assert!(config.min_speech_duration > 0.0);
        assert!(config.min_silence_duration > 0.0);
        assert!((config.threshold_ratio - 0.03).abs() < 0.001);
    }
}
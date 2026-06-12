# Changelog

All notable changes to VelociText will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.1.2] - 2026-06-11

### Added
- **Dictionary Panel**: unified hotwords and text replacement management for all ASR models (SenseVoice-Small, Paraformer-Large, Qwen3-ASR)
  - **Hotwords tab**: add proper nouns (names, places, brands) with adjustable weights (0.0–10.0) to boost ASR recognition accuracy. Changes trigger automatic model rebuild via sherpa-onnx `hotwords_file` + `hotwords_score`
  - **Replacements tab**: define post-processing text replacement rules (original → replacement) to correct commonly misrecognized words. Applied at the `get_recognition_progress` level so all downstream consumers (display, copy, export) receive corrected text
  - Dictionary configuration persisted to `AppData\Roaming\VelociText\dictionary.json`, hotwords file generated as `hotwords.txt` in standard `word weight` format
  - **Dedicated dictionary page** in the left sidebar navigation (below "转录"), with full i18n support (Chinese and English)
- **Sidebar state persistence**: collapsed/expanded state is now saved to `config.json` (`sidebarCollapsed` field) and restored on next launch
- **Qwen3-ASR 0.6B int8 model support** (`sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25`): 30+ languages including Chinese, English, Cantonese, Japanese, Korean, Arabic, German, French, Spanish, and more. Downloaded from gitcode.com (`https://gitcode.com/tabortao/VelociText/releases/download/model/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.zip`), ~450MB int8 quantized. Requires `conv_frontend.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, and `tokenizer/` directory
- **Model download from gitcode.com**: all Paraformer and Qwen3-ASR models now download from gitcode.com for fast access in China
- **Full transcription feature parity with sherpa-onnx official Tauri example**: `non-streaming-speech-recognition-from-file`
- Pure Rust audio/video decoding via `symphonia` crate — no FFmpeg required for basic transcription
  - Supports all major formats: MP3, FLAC, AAC, OGG, WAV, MP4, MKV, WebM, AIFF, M4A
- **Streaming incremental pipeline**: packet-by-packet decode → VAD segmentation → ASR recognition without buffering the entire file
- Built-in HTML5 `<video>` player with live subtitle overlay synchronized via `requestAnimationFrame` (~60fps)
  - Sticky player that stays visible when scrolling through segments
  - Player preview available during transcription processing
  - Proper `assetProtocol` configuration for local media playback via `convertFileSrc`
- **Click-to-seek**: click any segment row in the results table to jump playback to that segment
- **Segment save**: export individual speech segments as standalone 16kHz WAV files
- **VAD parameter configuration modal** (runtime adjustable via UI):
  - Threshold (0.0–1.0)
  - Minimum silence duration (seconds)
  - Minimum speech duration (seconds)
  - Maximum speech duration (seconds)
  - Recognizer threads (1–16)
- **Cancellation support**: cancel recognition mid-processing and keep partial results
- Copy actions:
  - Copy plain text
  - Copy with timestamps (SRT-style format)
- Export actions:
  - Export SRT subtitle file
  - Export TXT text file
- Background model initialization — models loaded on app startup so first transcription is faster
- Progress polling with incremental segment updates — results appear as they're recognized
- Real-time RTF (Real-Time Factor) calculation and speed display after completion
- **Flash/toast notifications** for copy and save confirmations (2.5s auto-dismiss)
- Dynamic status text showing segment count and character count during processing

### Changed
- Transcription page completely refactored to the new streaming architecture
- Transcription progress updated incrementally instead of waiting for full completion
- VAD now uses Sherpa-ONNX Silero VAD directly on streaming audio instead of post-processing RMS energy
- Segments recognized by ASR immediately after VAD detection, results incrementally added to UI
- Table-based segment display with save button per-segment
- Player URL now properly computed via `convertFileSrc` from `@tauri-apps/api/core` instead of fragile global access
- Enabled `protocol-asset` Tauri feature with scope `["**"]` for local media file playback

### Added
- Toast notification on transcription completion showing audio duration and elapsed time (4s auto-dismiss)
- `transcribe.completedToast` i18n key for completion toast message (Chinese and English)
- **Paraformer-Large ASR model support**: download from GitHub releases (`sherpa-onnx-paraformer-zh-2023-03-28.tar.bz2`), higher accuracy Chinese speech recognition
- **Model switching UI**: switch between SenseVoice-Small and Paraformer-Large in model management; active model highlighted with "Active" badge
- **Config persistence**: `AppConfig` now saves to/loads from `{app_data_dir}/config.json`, preserving `activeModel` and other settings across restarts
- **Model switching via restart**: `set_active_model` now saves config to JSON and calls `app_handle.restart()` instead of hot-swapping models (which caused crashes). On startup, `AppConfig::load()` reads the config and `build_models` uses the `activeModel` field to load the correct model
- **Paraformer tokens.txt auto-fix**: ModelScope provides `tokens.json` (JSON array format), but sherpa-onnx requires `tokens.txt` (plain text, one token per line). Download now converts JSON to correct format. Additionally, `build_models` auto-detects and fixes incorrectly formatted `tokens.txt` on startup
- **Model load fallback**: if the preferred model fails to load, `build_models` automatically falls back to another available model instead of crashing
- **Per-model download**: individual download buttons for each model (SenseVoice-Small, Paraformer-Large, Silero VAD)
- `download_specific_model`, `get_active_model`, `set_active_model` Tauri commands for model management
- `activeModel` field in `AppConfig` for persisting the selected ASR model across restarts
- Paraformer i18n keys (`models.paraformerDesc`, `models.activeModel`, `models.switchModel`, `models.switching`) in Chinese and English

### Changed
- Updated supported format descriptions to accurately reflect symphonia capabilities: removed AVI and FLV (not supported by symphonia), added FLAC, OGG, AAC, M4A, AIFF. Updated in i18n strings, file dialog filters, and Rust `SUPPORTED_FORMATS` list
- **File drag-and-drop**: replaced HTML5 `onDrop` with Tauri `DragDropEvent` listener. The Rust backend already emits `tauri://file-drop` and `tauri://file-drop-hover` events; the frontend now listens for these instead of relying on `DataTransfer.path` which is unavailable in Tauri v2
- **Sidebar width**: reduced from `72 * spacing` to `52 * spacing` for a more compact layout
- **Version display**: sidebar footer and About page now read version dynamically from `tauri.conf.json` via `getVersion()` API instead of hardcoding
- Removed History page from navigation and app routing
- Updated README.md and README-zh.md: removed FFmpeg dependency, added Paraformer-Large and symphonia, updated tech stack and roadmap
- **VAD optimization for Paraformer**: `max_speech_duration` automatically adjusted to 30s when Paraformer is active (vs 10s for SenseVoice)
- `build_models()` now accepts `preferred_model` parameter and returns the active model name
- Fixed `build_models` dir_name mismatch: was checking `"paraformer-large"` but `ModelType::Paraformer.dir_name()` returns `"paraformer"`

### Fixed
- **Dictionary hotwords model rebuild crash**: fixed cascading failure when model creation with hotwords fails — fallback models no longer inherit the hotwords file, preventing double failure. Added rollback mechanism: if `rebuild_recognizer` fails, dictionary config is reverted to previous state and the hotwords file path is cleared
- **Cannot clear/delete hotwords**: fixed `disabled` condition on Save button that prevented saving empty hotwords list; clearing all hotwords now correctly skips model rebuild. Frontend now reloads dictionary config from backend after both successful and failed saves, ensuring UI stays in sync with persisted state
- **Paraformer model crash on load**: switched download source from ModelScope iic (FunASR native ONNX, incompatible with sherpa-onnx) to official sherpa-onnx GitHub releases (`sherpa-onnx-paraformer-trilingual-zh-cantonese-en.tar.bz2`). FunASR native ONNX models lack the metadata and input/output names that sherpa-onnx expects, causing ONNX Runtime to abort the process
- **Model upgraded to Trilingual Paraformer** (`csukuangfj/sherpa-onnx-paraformer-trilingual-zh-cantonese-en`): supports Chinese, English, and Cantonese (粤语), int8 quantized ~233MB. Downloaded from gitcode.com (`https://gitcode.com/tabortao/VelociText/releases/download/v0.1.2/paraformer.zip`) for fast download in China; users can also manually download and extract `model.int8.onnx` + `tokens.txt` to the `paraformer/` directory
- **Crash recovery mechanism**: added `.model_loading` marker file — if the app crashes during model initialization, the next startup detects the marker and automatically falls back to the other available model
- **Old Paraformer model cleanup**: when re-downloading Paraformer, old/broken model files (e.g., `model_quant.onnx` from ModelScope, JSON-format `tokens.txt`) are now automatically cleaned up before downloading the correct sherpa-onnx model
- **`download_specific_model` Paraformer check**: now verifies `tokens.txt` format (not just file existence) before skipping download, ensuring broken models get re-downloaded
- **Tar entry borrow checker error (E0505)**: fixed by extracting path string into a separate scope before moving the entry for content reading
- **`config_arc` undefined variable**: replaced with direct `Mutex::new(initial_config)` since config fields are cloned before the init thread
- `set_active_model` return type: changed from `Result<String, String>` to `Result<(), String>` since `app_handle.restart()` never returns
- **GitHub Actions release workflow**: fixed PowerShell string parsing error in release summary step by specifying `shell: bash` for the heredoc block
- Updated About page tech stack description: replaced "FFmpeg for audio/video decoding" with "symphonia for pure Rust audio/video decoding"
- **Video file transcription**: fixed symphonia selecting video codec track instead of audio track in video files. Restricted symphonia to audio-only features (mp3, aac, flac, vorbis, wav, ogg, isomp4, mkv, pcm, adpcm, aiff, caf) matching the official sherpa-onnx Tauri example. Removed overly strict `sample_rate`/`channels` presence check that incorrectly skipped audio tracks in MP4/MKV containers where these fields are not populated during probe phase.
- **Video file audio sample rate detection**: fixed incorrect sample rate and channel count for video files. MP4/MKV containers often do not report `sample_rate` and `channels` in `codec_params` during the probe phase, causing the pipeline to default to 16000 Hz / 1 channel. This meant the resampler was never created for 44100/48000 Hz audio, resulting in garbled or empty ASR output. Now the actual sample rate and channel count are determined from the first decoded `AudioBufferRef` frame, and the resampler is created lazily with the correct rate.
- Added detailed logging for track selection and first-frame audio metadata to aid future debugging
- **Qwen3-ASR model switching**: fixed bug where switching to Qwen3-ASR and restarting would revert to SenseVoice-Small. Root cause: `build_models` catch_unwind success handler hardcoded model name detection to only check "paraformer" vs "sense-voice-small", missing "qwen3-asr". Also updated fallback chains (both error and panic paths) to include Qwen3-ASR as a candidate

- Removed unused `numThreads` TypeScript variable causing build error
- Fixed `convertFileSrc` not working properly by adding `assetProtocol` configuration
- **Updated application icons**: regenerated using `tauri icon` command from `source-icon.png`, creating complete icon sets for Windows, macOS, iOS, and Android platforms

## [v0.1.1] - 2026-06-10

### Added
- VAD (Voice Activity Detection) using RMS energy-based silence detection for automatic speech segmentation
- VAD toggle in Settings page — enable to split audio at silence points and recognize each segment independently
- Batch transcription: select multiple files at once and transcribe them sequentially with shared model loading
- Batch results summary panel in transcription page — click individual results to switch between files
- `transcribe_batch` Tauri command for backend batch processing
- `BatchResult` and `BatchFileResult` types for batch transcription results
- `HistoryEntry` TypeScript type for history record display
- FFmpeg startup detection with warning banner in main view when not installed
- `export_to_file` Rust command for direct file writing (bypasses Tauri fs plugin scope restrictions)
- `open_file_with_system` Rust command for cross-platform file opening with system default application
- Acknowledgments section in README.md and README-zh.md crediting sherpa-onnx, SenseVoice, Silero VAD, Tauri, React, shadcn/ui, FFmpeg, and ModelScope

### Changed
- `RecognizerFactory` now respects `num_threads` and `use_itn` from `RecognizerConfig`
- VAD setting in transcription page reads from app config instead of being hardcoded to false
- Play button now opens file via Rust `open_file_with_system` command instead of `tauri-plugin-opener`
- Export writes files via Rust `export_to_file` command instead of frontend `writeTextFile` (fs plugin)
- Export functions now display error messages to user when export fails
- Merged TXT/SRT/VTT export buttons into a single "Export" button with save-as dialog supporting all three formats

### Fixed
- Missing `HistoryEntry` type in TypeScript type definitions
- Unused `batchActiveFile` state causing `tsc` build failure
- Smart sentence segmentation not applied within VAD segments (text was distributed as raw blocks)
- TXT/SRT/VTT export silently failing without user-facing error feedback
- Removed unused `VadSegment::samples` field and `write_wav_segment` function
- Model management page header showing raw i18n key `header.model-settings` instead of translated title
- Play button not opening files with system default player
- Export save dialog failing to write files due to Tauri fs plugin scope restrictions

## [v0.1.0] - 2026-06-09

### Added
- Initial project scaffolding based on Tauri v2 + React 19 + TypeScript + shadcn/ui
- Left sidebar navigation: Transcription, History, Settings, Model Management
- Core speech recognition engine using sherpa-onnx framework with SenseVoice-Small model (int8 quantized)
- Multi-model architecture (`RecognizerFactory`) supporting SenseVoice, Paraformer, Zipformer CTC, and Transducer model types
- FFmpeg-based audio extraction to 16kHz Mono WAV with hidden console window on Windows
- Native file drag-and-drop support via Rust `DragDropEvent`
- Real-time progress reporting via `tokio::spawn_blocking` + `mpsc::channel` with frontend event listening
- Elapsed time and audio duration display after transcription
- Smart sentence segmentation based on Chinese/English punctuation marks with proportional timestamps
- Multi-format export: TXT (with timestamps), SRT, VTT subtitle files using original filename
- Model management page with one-click download of SenseVoice-Small from ModelScope.cn
- FFmpeg auto-detection with version display in Settings page
- Settings page: language selection, export format, VAD toggle
- File-based history persistence (JSON) with save, load, delete, and clear operations
- Audio player (Play/Pause) in transcription results
- "New" button to transcribe another file after completion without clearing results
- Custom hotwords support via `hotwords_file` and `hotwords_score` config
- Application binary renamed to `velocitext.exe`
- Version display in sidebar footer matches ChangeLog version
- Cleaned up old tauri-ui template files (nav-*, greet, data-table, chart, debug-panel, theme-provider, etc.)
- bun as package manager

### Fixed
- Missing `tokens` field in `OfflineModelConfig` causing recognizer creation failure
- `@tauri-apps/plugin-dialog` dynamic import resolution
- Model file path resolution to support `model_q8.onnx` filename
- FFmpeg console window popup on Windows by using `CREATE_NO_WINDOW` flag
- Binary name from `tauri-native` to `velocitext`
- Missing `Progress` shadcn/ui component
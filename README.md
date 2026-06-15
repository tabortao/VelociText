# VelociText

> **The Ultimate Offline Video & Audio to Text Transcription & OCR Tool**

VelociText is a blazing-fast, cross-platform desktop application for offline speech recognition and text OCR. Powered by [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) with SenseVoice-Small and Paraformer-Large ASR models, plus PaddleOCR (V4/V5/V6) for text recognition — all processing happens entirely on your device. **No internet required, your data stays private.**

## Features

### Speech Recognition (ASR)
- **Fully Offline** — All processing happens locally. No cloud, no API keys, no data leaks.
- **Multi-format Support** — MP3, WAV, FLAC, OGG, AAC, M4A, AIFF, MP4, MOV, MKV, WebM and more (pure Rust decoding via symphonia, no FFmpeg required).
- **Dual ASR Models** — Switch between SenseVoice-Small (multilingual) and Paraformer-Large (higher accuracy Chinese) in the model management UI.
- **Qwen3-ASR Model** — Additional multilingual ASR model option with high accuracy.
- **Smart VAD** — Silero VAD (Voice Activity Detection) intelligently detects speech segments with millisecond precision.
- **Streaming Pipeline** — Incremental audio decoding + VAD + ASR for real-time progress and low memory usage.
- **Multi-format Export** — Export results as TXT (with timestamps), SRT, or VTT subtitle files.

### Text Recognition (OCR)
- **PaddleOCR Models** — Support for PP-OCR V4, V5, and V6 ONNX models with one-click download.
- **Image OCR** — Drag-and-drop or file picker to load images (PNG, JPG, BMP, WEBP, TIFF) for text recognition.
- **Screenshot OCR** — Press a global shortcut (default `Ctrl+Shift+O`) to capture any screen region, recognize text, and copy to clipboard automatically. Supports multi-monitor setups.
- **Dictionary Correction** — Custom hotword dictionary for post-OCR text correction.

### Application
- **System Tray** — Closing the window minimizes to the system tray. Left-click to restore, right-click to quit.
- **Single Instance** — Only one instance can run at a time; launching again activates the existing window.
- **Global Shortcuts** — Screenshot OCR shortcut works even when the app is minimized or in the tray.
- **Multi-language UI** — Interface available in English and Chinese.
- **Model Management** — One-click model download with progress tracking; switch active model at any time.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | [Tauri v2](https://v2.tauri.app) (Rust backend) |
| Frontend | React 19 + TypeScript + [shadcn/ui](https://ui.shadcn.com) |
| ASR Engine | [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) v1.13 |
| OCR Engine | [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) via [paddle-ocr-rs](https://github.com/mg-chao/paddle-ocr-rs) |
| Speech Models | SenseVoice-Small (q8, ~230MB) + Paraformer-Large (int8, ~238MB) + Qwen3-ASR |
| OCR Models | PP-OCR V4/V5/V6 ONNX (~25MB each) |
| VAD Model | Silero VAD ONNX (~2.7MB) |
| Audio/Video Decoding | [symphonia](https://github.com/pdeljanov/Symphonia) (pure Rust, no FFmpeg needed) |
| Screenshot Capture | [xcap](https://github.com/nicepkg/xcap) (multi-monitor support) |
| Build Tool | [Bun](https://bun.sh) + Vite |

## Roadmap

- [x] Core speech recognition with SenseVoice-Small
- [x] VAD smart speech segmentation
- [x] SRT/VTT subtitle export
- [x] Paraformer-Large ONNX — Higher accuracy Mandarin ASR model
- [x] Model switching UI (SenseVoice ↔ Paraformer ↔ Qwen3-ASR)
- [x] Pure Rust audio/video decoding (symphonia, no FFmpeg dependency)
- [x] OCR text recognition with PaddleOCR (V4/V5/V6)
- [x] Screenshot OCR with global shortcut and multi-monitor support
- [x] System tray and single instance
- [ ] Speaker diarization (speaker identification)
- [ ] Custom vocabulary / hotwords UI

## Quick Start

### Prerequisites

- [Bun](https://bun.sh) (package manager)
- [Rust](https://rustup.rs) (for Tauri backend compilation)

### Development

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev

# Build for production
bun run tauri build
```

### Models

ASR and OCR models can be downloaded from within the app via **Settings → Model Management → Download**.

| Model | Size | Description |
|-------|------|-------------|
| SenseVoice-Small | ~230MB (q8) | Multilingual: Chinese, English, Cantonese, Japanese, Korean |
| Paraformer-Large | ~238MB (int8) | Higher accuracy Chinese ASR |
| Qwen3-ASR | ~400MB | Multilingual ASR with high accuracy |
| Silero VAD | ~2.7MB | Voice activity detection for speech segmentation |
| PP-OCR V4/V5/V6 | ~25MB each | Text recognition (OCR) models |

Models are stored in a configurable local directory. The default path is `{app_data_dir}/models/`.

## Contributing

VelociText is under active development. Contributions, issues, and feature requests are welcome.

## License

MIT

## Acknowledgments

VelociText is built on the shoulders of giants. Special thanks to these outstanding open-source projects:

- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) — The core ASR runtime engine powering offline speech recognition
- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) — Outstanding multilingual OCR toolkit
- [paddle-ocr-rs](https://github.com/mg-chao/paddle-ocr-rs) — Rust bindings for PaddleOCR ONNX inference
- [SenseVoice](https://github.com/FunAudioLLM/SenseVoice) — Multilingual speech recognition model by FunAudioLLM
- [Paraformer](https://www.modelscope.cn/models/iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx) — High-accuracy Chinese ASR model by Alibaba DAMO Academy
- [Silero VAD](https://github.com/snakers4/silero-vad) — Voice Activity Detection model for speech segmentation
- [symphonia](https://github.com/pdeljanov/Symphonia) — Pure Rust audio decoding library
- [xcap](https://github.com/nicepkg/xcap) — Cross-platform screen capture library
- [Tauri](https://tauri.app/) — Cross-platform desktop application framework
- [React](https://react.dev/) — Frontend UI library
- [shadcn/ui](https://ui.shadcn.com/) — Beautifully designed UI components

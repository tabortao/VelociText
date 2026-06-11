# VelociText

> **The Ultimate Offline Video & Audio to Text Transcription Tool**

VelociText is a blazing-fast, cross-platform desktop application for offline speech recognition. Powered by [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) with SenseVoice-Small and Paraformer-Large models, it transcribes audio and video files entirely on your device — **no internet required, your data stays private**.

## Features

- **Fully Offline** — All processing happens locally. No cloud, no API keys, no data leaks.
- **Multi-format Support** — MP3, WAV, FLAC, OGG, AAC, M4A, AIFF, MP4, MOV, MKV, WebM and more (pure Rust decoding via symphonia, no FFmpeg required).
- **Dual ASR Models** — Switch between SenseVoice-Small (multilingual) and Paraformer-Large (higher accuracy Chinese) in the model management UI.
- **Smart VAD** — Silero VAD (Voice Activity Detection) intelligently detects speech segments with millisecond precision.
- **Streaming Pipeline** — Incremental audio decoding + VAD + ASR for real-time progress and low memory usage.
- **Multi-format Export** — Export results as TXT (with timestamps), SRT, or VTT subtitle files.
- **Multi-language UI** — Interface available in English and Chinese; SenseVoice supports Chinese, English, Cantonese, Japanese, and Korean.
- **Model Management** — One-click model download from ModelScope.cn with progress tracking; switch active model at any time.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | [Tauri v2](https://v2.tauri.app) (Rust backend) |
| Frontend | React 19 + TypeScript + [shadcn/ui](https://ui.shadcn.com) |
| ASR Engine | [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) v1.13 |
| Speech Models | SenseVoice-Small (q8, ~230MB) + Paraformer-Large (int8, ~238MB) |
| VAD Model | Silero VAD ONNX (~2.7MB) |
| Audio/Video Decoding | [symphonia](https://github.com/pdeljanov/Symphonia) (pure Rust, no FFmpeg needed) |
| Build Tool | [Bun](https://bun.sh) + Vite |

## Roadmap

- [x] Core speech recognition with SenseVoice-Small
- [x] VAD smart speech segmentation
- [x] SRT/VTT subtitle export
- [x] **Paraformer-Large ONNX** — Higher accuracy Mandarin ASR model
- [x] Model switching UI (SenseVoice ↔ Paraformer)
- [x] Pure Rust audio/video decoding (symphonia, no FFmpeg dependency)
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

ASR models and the Silero VAD model can be downloaded from within the app via **Settings → Models → Download**.

| Model | Size | Description |
|-------|------|-------------|
| SenseVoice-Small | ~230MB (q8) | Multilingual: Chinese, English, Cantonese, Japanese, Korean |
| Paraformer-Large | ~238MB (int8) | Higher accuracy Chinese ASR |
| Silero VAD | ~2.7MB | Voice activity detection for speech segmentation |

Models are stored in a configurable local directory. The default path is `{app_data_dir}/models/`.

## Contributing

VelociText is under active development. Contributions, issues, and feature requests are welcome.

## License

MIT

## Acknowledgments

VelociText is built on the shoulders of giants. Special thanks to these outstanding open-source projects:

- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) — The core ASR runtime engine powering offline speech recognition
- [SenseVoice](https://github.com/FunAudioLLM/SenseVoice) — Multilingual speech recognition model by FunAudioLLM
- [Paraformer](https://www.modelscope.cn/models/iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx) — High-accuracy Chinese ASR model by Alibaba DAMO Academy
- [Silero VAD](https://github.com/snakers4/silero-vad) — Voice Activity Detection model for speech segmentation
- [symphonia](https://github.com/pdeljanov/Symphonia) — Pure Rust audio decoding library
- [Tauri](https://tauri.app/) — Cross-platform desktop application framework
- [React](https://react.dev/) — Frontend UI library
- [shadcn/ui](https://ui.shadcn.com/) — Beautifully designed UI components
- [ModelScope](https://modelscope.cn/) — Model hosting and distribution platform

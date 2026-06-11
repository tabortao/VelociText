# VelociText

> **The Ultimate Offline Video & Audio to Text Transcription Tool**

VelociText is a blazing-fast, cross-platform desktop application for offline speech recognition. Powered by [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) and the SenseVoice-Small model, it transcribes audio and video files entirely on your device — **no internet required, your data stays private**.

## Features

- **Fully Offline** — All processing happens locally. No cloud, no API keys, no data leaks.
- **Multi-format Support** — MP3, WAV, MP4, AVI, MOV, MKV, FLV, WEBM, AAC, FLAC, OGG, WMA, M4A, and more.
- **Smart VAD** — Silero VAD (Voice Activity Detection) intelligently detects speech segments with millisecond precision.
- **Parallel Chunked Transcription** — Long audio files (>60s) are automatically split into chunks and transcribed in parallel for maximum speed.
- **Batch Processing** — Select multiple files and transcribe them in one go with shared model loading.
- **Multi-format Export** — Export results as TXT (with timestamps), SRT, or VTT subtitle files.
- **Multi-language Support** — UI available in English and Chinese; ASR supports Chinese, English, Cantonese, Japanese, and Korean.
- **Model Management** — One-click model download from ModelScope.cn with progress tracking.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | [Tauri v2](https://v2.tauri.app) (Rust backend) |
| Frontend | React 19 + TypeScript + [shadcn/ui](https://ui.shadcn.com) |
| ASR Engine | [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) v1.13 |
| Speech Model | SenseVoice-Small (q8 quantized, ~230MB) |
| VAD Model | Silero VAD ONNX (~2.7MB) |
| Audio Processing | FFmpeg (16kHz mono WAV extraction) |
| Build Tool | [Bun](https://bun.sh) + Vite |

## Roadmap

- [x] Core speech recognition with SenseVoice-Small
- [x] VAD smart speech segmentation
- [x] Batch file transcription
- [x] SRT/VTT subtitle export
- [x] Parallel chunked transcription for long audio
- [ ] **Paraformer Large ONNX** — Higher accuracy Mandarin ASR model (in development)
- [ ] Speaker diarization (speaker identification)
- [ ] Custom vocabulary / hotwords UI

## Quick Start

### Prerequisites

- [Bun](https://bun.sh) (package manager)
- [Rust](https://rustup.rs) (for Tauri backend compilation)
- [FFmpeg](https://ffmpeg.org) (required for audio extraction — auto-detected on startup)

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

The SenseVoice-Small ASR model (~230MB) and Silero VAD model (~2.7MB) can be downloaded from within the app via **Settings → Models → Download**.

Models are stored in a configurable local directory. The default path is `{app_data_dir}/models/`.

## Contributing

VelociText is under active development. Contributions, issues, and feature requests are welcome.

## License

MIT

## Acknowledgments

VelociText is built on the shoulders of giants. Special thanks to these outstanding open-source projects:

- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) — The core ASR runtime engine powering offline speech recognition
- [SenseVoice-Small](https://github.com/FunAudioLLM/SenseVoice) — Multilingual speech recognition model by FunAudioLLM
- [Silero VAD](https://github.com/snakers4/silero-vad) — Voice Activity Detection model for speech segmentation
- [Tauri](https://tauri.app/) — Cross-platform desktop application framework
- [React](https://react.dev/) — Frontend UI library
- [shadcn/ui](https://ui.shadcn.com/) — Beautifully designed UI components
- [FFmpeg](https://ffmpeg.org/) — Universal audio/video processing toolkit
- [ModelScope](https://modelscope.cn/) — Model hosting and distribution platform
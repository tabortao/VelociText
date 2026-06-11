# VelociText

> **极致离线视频/音频转文字工具**

VelociText 是一款极速、跨平台的离线语音识别桌面应用。基于 [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 引擎，支持 SenseVoice-Small 和 Paraformer-Large 双模型，完全在本地转录音视频文件 — **无需联网，数据隐私无忧**。

## 功能特性

- **完全离线** — 所有处理均在本地完成，无需云端、无需 API Key、数据不会泄露。
- **多格式支持** — MP3、WAV、FLAC、OGG、AAC、M4A、AIFF、MP4、MOV、MKV、WebM 等（纯 Rust 解码，无需 FFmpeg）。
- **双 ASR 模型** — 在模型管理界面一键切换 SenseVoice-Small（多语言）和 Paraformer-Large（更高精度中文）。
- **智能 VAD** — Silero VAD 语音活动检测，毫秒级精度识别语音段落。
- **流式管线** — 增量音频解码 + VAD + ASR，实时进度显示，低内存占用。
- **多格式导出** — 支持导出为 TXT（带时间戳）、SRT、VTT 字幕文件。
- **多语言界面** — 支持中文和英文界面切换；SenseVoice 支持中文、英文、粤语、日语、韩语。
- **模型管理** — 一键从 ModelScope.cn 下载模型，带进度显示；随时切换活跃模型。

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | [Tauri v2](https://v2.tauri.app)（Rust 后端） |
| 前端 | React 19 + TypeScript + [shadcn/ui](https://ui.shadcn.com) |
| ASR 引擎 | [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) v1.13 |
| 语音模型 | SenseVoice-Small（q8 量化，约 230MB）+ Paraformer-Large（int8 量化，约 238MB） |
| VAD 模型 | Silero VAD ONNX（约 2.7MB） |
| 音视频解码 | [symphonia](https://github.com/pdeljanov/Symphonia)（纯 Rust，无需 FFmpeg） |
| 构建工具 | [Bun](https://bun.sh) + Vite |

## 路线图

- [x] SenseVoice-Small 离线语音识别
- [x] VAD 智能语音分段
- [x] SRT/VTT 字幕导出
- [x] **Paraformer-Large ONNX** — 更高精度的中文语音识别模型
- [x] 模型切换 UI（SenseVoice ↔ Paraformer）
- [x] 纯 Rust 音视频解码（symphonia，无 FFmpeg 依赖）
- [ ] 说话人分离（Speaker Diarization）
- [ ] 自定义热词 UI 管理

## 快速开始

### 环境要求

- [Bun](https://bun.sh)（包管理器）
- [Rust](https://rustup.rs)（用于编译 Tauri 后端）

### 开发

```bash
# 安装依赖
bun install

# 开发模式运行
bun run tauri dev

# 构建生产版本
bun run tauri build
```

### 模型下载

ASR 模型和 Silero VAD 模型可在应用内通过 **设置 → 模型管理 → 下载** 获取。

| 模型 | 大小 | 说明 |
|------|------|------|
| SenseVoice-Small | 约 230MB (q8) | 多语言：中文、英文、粤语、日语、韩语 |
| Paraformer-Large | 约 238MB (int8) | 更高精度的中文语音识别 |
| Silero VAD | 约 2.7MB | 语音活动检测，用于语音分段 |

模型存储在可配置的本地目录中，默认路径为 `{应用数据目录}/models/`。

## 贡献

VelociText 正在积极开发中，欢迎提交 Issue 和 Pull Request。

## 许可证

MIT

## 鸣谢

VelociText 的构建得益于以下优秀开源项目：

- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) — 核心 ASR 推理引擎，驱动离线语音识别
- [SenseVoice](https://github.com/FunAudioLLM/SenseVoice) — FunAudioLLM 多语言语音识别模型
- [Paraformer](https://www.modelscope.cn/models/iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx) — 阿里达摩院高精度中文语音识别模型
- [Silero VAD](https://github.com/snakers4/silero-vad) — 语音活动检测模型，用于语音分段
- [symphonia](https://github.com/pdeljanov/Symphonia) — 纯 Rust 音频解码库
- [Tauri](https://tauri.app/) — 跨平台桌面应用框架
- [React](https://react.dev/) — 前端 UI 库
- [shadcn/ui](https://ui.shadcn.com/) — 精美设计的 UI 组件
- [ModelScope](https://modelscope.cn/) — 模型托管与分发平台

# VelociText

> **极致离线视频/音频转文字工具**

VelociText 是一款极速、跨平台的离线语音识别桌面应用。基于 [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 模型引擎，完全在本地转录音视频文件 — **无需联网，数据隐私无忧**。

## 功能特性

- **完全离线** — 所有处理均在本地完成，无需云端、无需 API Key、数据不会泄露。
- **多格式支持** — MP3、WAV、MP4、AVI、MOV、MKV、FLV、WEBM、AAC、FLAC、OGG、WMA、M4A 等。
- **智能 VAD** — Silero VAD 语音活动检测，毫秒级精度识别语音段落。
- **并行分段转录** — 长音频（>60秒）自动切分为片段并行转录，大幅提升识别速度。
- **批量处理** — 一次选择多个文件，共享模型加载，依次转录。
- **多格式导出** — 支持导出为 TXT（带时间戳）、SRT、VTT 字幕文件。
- **多语言界面** — 支持中文和英文界面切换；语音识别支持中文、英文、粤语、日语、韩语。
- **模型管理** — 一键从 ModelScope.cn 下载模型，带进度显示。

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | [Tauri v2](https://v2.tauri.app)（Rust 后端） |
| 前端 | React 19 + TypeScript + [shadcn/ui](https://ui.shadcn.com) |
| ASR 引擎 | [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) v1.13 |
| 语音模型 | SenseVoice-Small（q8 量化，约 230MB） |
| VAD 模型 | Silero VAD ONNX（约 2.7MB） |
| 音频处理 | FFmpeg（16kHz 单声道 WAV 提取） |
| 构建工具 | [Bun](https://bun.sh) + Vite |

## 路线图

- [x] SenseVoice-Small 离线语音识别
- [x] VAD 智能语音分段
- [x] 批量文件转录
- [x] SRT/VTT 字幕导出
- [x] 长音频并行分段转录
- [ ] **Paraformer Large ONNX** — 更高精度的中文语音识别模型（开发中）
- [ ] 说话人分离（Speaker Diarization）
- [ ] 自定义热词 UI 管理

## 快速开始

### 环境要求

- [Bun](https://bun.sh)（包管理器）
- [Rust](https://rustup.rs)（用于编译 Tauri 后端）
- [FFmpeg](https://ffmpeg.org)（音频提取必需，启动时自动检测）

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

SenseVoice-Small 语音识别模型（约 230MB）和 Silero VAD 模型（约 2.7MB）可在应用内通过 **设置 → 模型管理 → 下载** 获取。

模型存储在可配置的本地目录中，默认路径为 `{应用数据目录}/models/`。

## 贡献

VelociText 正在积极开发中，欢迎提交 Issue 和 Pull Request。

## 许可证

MIT

## 鸣谢

VelociText 的构建得益于以下优秀开源项目：

- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) — 核心 ASR 推理引擎，驱动离线语音识别
- [SenseVoice-Small](https://github.com/FunAudioLLM/SenseVoice) — FunAudioLLM 多语言语音识别模型
- [Silero VAD](https://github.com/snakers4/silero-vad) — 语音活动检测模型，用于语音分段
- [Tauri](https://tauri.app/) — 跨平台桌面应用框架
- [React](https://react.dev/) — 前端 UI 库
- [shadcn/ui](https://ui.shadcn.com/) — 精美设计的 UI 组件
- [FFmpeg](https://ffmpeg.org/) — 通用音视频处理工具集
- [ModelScope](https://modelscope.cn/) — 模型托管与分发平台
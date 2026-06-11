# 转录功能优化 v2 — 对标 sherpa-onnx 官方 Tauri 示例（收尾计划）

## 概要

参考 `docs/Reference/tauri-examples/non-streaming-speech-recognition-from-file` 对 VelociText 转录功能进行优化。**核心代码工作已在上一次会话中完成**（后端 Rust 流水线 + 前端 React 新 UI），本次计划仅聚焦于**修复剩余构建问题** + **验证** + **ChangeLog 更新**。

## 已完成功能回顾（上一会话）

### Backend (Rust) — 全部完成 ✅
| 文件 | 内容 |
|------|------|
| `Cargo.toml` | 添加 `symphonia` 依赖（纯 Rust 音视频解码） |
| `engine/audio_decoder.rs` | `open_audio_file`, `decode_to_mono_f32`, `create_resampler`, `write_wav`, `decode_time_range` |
| `engine/transcription_pipeline.rs` | `run_recognition`, `recognize_segment`, `SegmentResult`, `VadSettings` |
| `engine/mod.rs` | 注册 audio_decoder + transcription_pipeline 模块 |
| `lib.rs` | AppState 扩展（recognizer, vad_detector, running, cancelled, segments 等）+ 后台模型加载 + `build_models` + 命令注册 |
| `commands/transcribe.rs` | 新流式命令（recognize_file, get_recognition_progress, cancel_recognition, save_segment_as_wav, get_vad_settings, apply_vad_settings, get_init_status）+ 保留旧命令 |

### Frontend (TypeScript/React) — 全部完成 ✅
| 文件 | 内容 |
|------|------|
| `types/index.ts` | 新增 `StreamingSegment`, `ProcessingState`, `InitStatus`, `VadSettings` |
| `lib/app-context.tsx` | 新增 18 个翻译键（中/英），含 `speedLabel`, `loadingModels`, `copyTimed`, `saveSegment`, `vadSettings` 等 |
| `app/transcribe/page.tsx` | 全面重构：模型初始化轮询、流式转录 + 进度轮询、内置 `<video>` 播放器 + rAF 字幕同步（60fps）、结果表格 + 点击跳转、分段 WAV 保存、VAD 设置模态框（threshold/min_silence/min_speech/max_speech/threads）、取消支持、复制文本/时间戳、导出 SRT/TXT |

### 对标参考示例的功能完成度

| 参考示例功能 | 状态 |
|-------------|------|
| 读取音视频文件（symphonia 解码） | ✅ |
| VAD + ASR 流式分段识别 | ✅ |
| 模型后台加载 + 初始化轮询 | ✅ |
| 进度条 + 百分比显示 | ✅ |
| Cancel 取消识别 | ✅ |
| 内置播放器（video 元素）+ 字幕覆盖层 | ✅ |
| rAF 字幕同步 + 表格行高亮 | ✅ |
| 点击表格行 → seek + 播放 | ✅ |
| 分段保存为 WAV | ✅ |
| Copy Text / Copy with Timestamps | ✅ |
| Export SRT | ✅ |
| VAD 设置弹窗（参数可调 + 模型重建） | ✅ |
| RTF 速度显示 | ✅ |
| 多语言支持（中/英） | ✅ |

---

## 待修复/完成的任务

### 1. 修复 TypeScript 构建错误

**文件**: [src/app/transcribe/page.tsx](file:///d:/Code/Rust/VelociText/src/app/transcribe/page.tsx)

**问题**: `numThreads` 变量在第 58 行声明 `const [numThreads, setNumThreads] = useState(2)` 并在第 100 行赋值 `setNumThreads(res.numThreads)`，但从不被读取，导致 TypeScript 构建错误。

**原因**: `numThreads` 从 `get_init_status` 获取但未在任何地方展示。VAD 设置的线程数由 `get_vad_settings` 单独加载到设置弹窗表单中，不依赖此变量。

**修复方案**: 删除 `numThreads` 相关的两行代码。

**需要删除的行**:
- 第 58 行: `const [numThreads, setNumThreads] = useState(2)`
- 第 100 行: `setNumThreads(res.numThreads)`

### 2. 构建验证

**步骤**:
1. `cargo check` — 验证 Rust 编译通过
2. `bun run tauri build` — 完整构建（生成 MSI + Setup.exe）

**预期结果**: 无错误，成功生成 `VelociText_0.1.1_x64_en-US.msi` 和 `VelociText_0.1.1_x64-setup.exe`

### 3. 更新 ChangeLog

**文件**: [docs/ChangeLog.md](file:///d:/Code/Rust/VelociText/docs/ChangeLog.md)

按照 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) 规范（英文），记录本次转录功能优化。

---

## 验证步骤

1. `cargo check` — Rust 编译通过
2. `bun run tauri build` — 前端 + 后端全量构建通过
3. 启动应用 → 显示 "正在加载模型..." → 加载完成后按钮可用
4. 选择音视频文件 → 进度条实时更新 → 分段结果逐段出现在表格
5. 转录完成后 → 内置播放器可用 → 字幕同步高亮
6. 点击表格行 → 播放器跳转到对应时间并自动播放
7. 点击取消 → 转录中断 → 保留已完成结果
8. VAD 设置调整 → 验证 + 模型重新加载 → 影响后续识别
9. 复制纯文本 / 带时间戳文本
10. 导出 SRT / TXT
11. 保存单个分段为 WAV
12. 语言切换（中/英）正常

---

## 实施顺序

| # | 任务 | 文件 | 预计影响 |
|---|------|------|---------|
| 1 | 移除未使用的 `numThreads` | `src/app/transcribe/page.tsx` | 2 行删除 |
| 2 | 构建验证 | `cargo check` + `bun run tauri build` | 验证所有代码 |
| 3 | 更新 ChangeLog | `docs/ChangeLog.md` | 英文文档 |
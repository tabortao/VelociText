# VelociText 音频/视频转文字软件开发计划

> **状态更新**：MVP 核心架构已基本完成。本文档反映当前实际实现状态，并规划剩余工作。

***

## 1. 项目概述与目标

### 1.1 项目背景

VelociText 是一款基于 Tauri v2 的跨平台桌面应用，专注于将音频和视频文件高效转换为文字。依托 sherpa-onnx 框架在本地执行语音识别推理，无需联网，保护用户隐私。

### 1.2 项目目标

| 版本             | 目标                                                         | 状态            |
| -------------- | ---------------------------------------------------------- | ------------- |
| **V1.0 (MVP)** | 基于 SenseVoice-Small 的离线语音识别核心流程，支持常见音视频格式导入，导出 TXT/SRT/VTT | 🟡 核心完成，待联调打磨 |
| **V1.1**       | 流式实时识别、说话人分离、标点增强、历史记录持久化                                  | ⬜ 待开始         |
| **V2.0**       | 多模型切换架构、GPU 加速（DirectML/CUDA）、批量处理                         | ⬜ 待开始         |

### 1.3 技术栈

| 层级    | 技术                                                      | 版本       |
| ----- | ------------------------------------------------------- | -------- |
| 桌面框架  | Tauri v2                                                | 2.x      |
| 前端    | React + TypeScript + Vite + shadcn/ui + Tailwind CSS v4 | React 19 |
| 包管理器  | bun                                                     | latest   |
| 语音识别  | sherpa-onnx (Rust crate)                                | 1.13.x   |
| 初始模型  | SenseVoice-Small (ONNX)                                 | -        |
| 音视频解码 | FFmpeg (通过 std::process 调用)                             | 系统依赖     |

***

## 2. 技术架构设计

### 2.1 整体架构（已实现）

```
┌─────────────────────────────────────────────────────┐
│                    Frontend (React)                  │
│  ┌──────────┐  ┌──────────────────────────────────┐ │
│  │ Sidebar  │  │        Content Area              │ │
│  │  - 转录   │  │  state-based view switch        │ │
│  │  - 历史   │  │  (Transcribe/History/Settings)  │ │
│  │  - 设置   │  │                                 │ │
│  │  - 模型   │  │                                 │ │
│  └──────────┘  └──────────────────────────────────┘ │
│                        │ Tauri invoke()              │
├────────────────────────┼────────────────────────────┤
│              Rust Backend (src-tauri/)               │
│  ┌─────────────────────┼──────────────────────────┐ │
│  │  Commands (已实现)   │                         │ │
│  │  - transcribe_file  │  - export_result        │ │
│  │  - check_ffmpeg     │  - check_file_format    │ │
│  │  - list_models      │  - get_model_path       │ │
│  │  - get_app_config   │  - set_app_config       │ │
│  │  - get_languages    │  - get_history / delete │ │
│  ├─────────────────────┼──────────────────────────┤ │
│  │  Engine Layer (已实现)                        │ │
│  │  - AudioExtractor  │  FFmpeg 提取 16kHz WAV   │ │
│  │  - Recognizer      │  sherpa-onnx 封装        │ │
│  │  - Transcriber     │  转录调度                │ │
│  │  - ExportManager   │  TXT/SRT/VTT 导出       │ │
│  │  - ModelManager    │  模型列表/路径管理       │ │
│  │  - ProgressTracker │  进度跟踪                │ │
│  └─────────────────────┴──────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

### 2.2 Rust 后端模块

#### 当前目录结构

```
src-tauri/src/
├── main.rs                       # ✅ 入口
├── lib.rs                        # ✅ Builder + plugins + commands 注册
├── commands/
│   ├── mod.rs                    # ✅
│   ├── transcribe.rs             # ✅ transcribe_file, export_result, check_ffmpeg, check_file_format
│   ├── model.rs                  # ✅ list_models, get_model_path
│   ├── config.rs                 # ✅ get_app_config, set_app_config, get_languages
│   └── history.rs                # ✅ get_history, delete_history (stub)
├── engine/
│   ├── mod.rs                    # ✅
│   ├── audio_extractor.rs        # ✅ FFmpeg 提取音频为 16kHz Mono WAV
│   ├── recognizer.rs             # ✅ sherpa-onnx SenseVoice-Small 封装
│   ├── transcriber.rs            # ✅ 协调音频提取 → 识别
│   ├── export.rs                 # ✅ TXT/SRT/VTT 导出 + 测试
│   ├── model_manager.rs          # ✅ 模型列表/路径管理
│   └── progress.rs               # ✅ 原子进度跟踪
├── config/
│   ├── mod.rs                    # ✅
│   └── app_config.rs             # ✅ 配置结构体 + 默认值
├── models/
│   ├── mod.rs                    # ✅
│   ├── task.rs                   # ✅ TranscribeSegment, TranscribeOptions 等
│   ├── history.rs                # ✅ 历史记录模型
│   └── language.rs               # ✅ Language 模型
└── errors.rs                     # ✅ AppError + AppResult
```

#### Cargo.toml 依赖

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-log = "2"
tauri-plugin-opener = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
log = "0.4"
sherpa-onnx = "1.13"
tokio = { version = "1", features = ["full"] }
tempfile = "3"
anyhow = "1"
thiserror = "2"
chrono = { version = "0.4", features = ["serde"] }
```

### 2.3 前端架构

#### 当前目录结构

```
src/
├── main.tsx                      # ✅ 入口
├── App.tsx                       # ✅ 根组件（SidebarProvider + 页面切换）
├── index.css                     # ✅ 全局样式
├── app/
│   ├── transcribe/
│   │   └── page.tsx              # ✅ 转录页面（文件选择/进度/结果/导出）
│   ├── history/
│   │   └── page.tsx              # ✅ 历史记录（占位 - 暂无记录）
│   └── settings/
│       ├── page.tsx              # ✅ 设置页面（配置项表单）
│       └── model-settings.tsx    # ✅ 模型管理页面
├── components/
│   ├── ui/                       # ✅ shadcn/ui 组件库（25个组件）
│   ├── app-sidebar.tsx           # ✅ 左侧导航（转录/历史/设置/模型）
│   ├── site-header.tsx           # ✅ 顶部标题栏
│   └── ... (旧模板组件待清理)
├── hooks/
│   └── use-mobile.ts             # ✅
├── lib/
│   ├── tauri.ts                  # ✅ Tauri 调用封装
│   └── utils.ts                  # ✅
└── types/
    └── index.ts                  # ✅ 类型定义
```

#### 页面路由（状态切换方式）

```
"transcribe"      → TranscribePage   (转录)
"history"         → HistoryPage      (历史记录)
"settings"        → SettingsPage     (设置)
"model-settings"  → ModelSettingsPage (模型管理)
```

### 2.4 数据流（核心流程）

```
选择文件 → FileUpload → invoke("transcribe_file")
                              ↓
                         AudioExtractor (FFmpeg → 16kHz WAV)
                              ↓
                         Recognizer (sherpa-onnx SenseVoice-Small)
                              ↓
                         返回 Vec<TranscribeSegment>
                              ↓
                         前端展示 + 复制/导出
```

***

## 3. 功能模块划分

### 3.1 已完成模块

| 模块             | 功能                                | 实现文件                                                                                           |
| -------------- | --------------------------------- | ---------------------------------------------------------------------------------------------- |
| 音视频处理          | FFmpeg 提取音频为 16kHz Mono WAV       | [audio\_extractor.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/audio_extractor.rs) |
| 语音识别           | sherpa-onnx SenseVoice-Small 离线推理 | [recognizer.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/recognizer.rs)            |
| 转录调度           | 协调音频提取与识别流程                       | [transcriber.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/transcriber.rs)          |
| 结果导出           | TXT / SRT / VTT 三种格式              | [export.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/export.rs)                    |
| 模型管理           | 模型列表查询、路径管理                       | [model\_manager.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/model_manager.rs)     |
| 进度跟踪           | 原子进度计数器 + 取消支持                    | [progress.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/progress.rs)                |
| 配置管理           | 模型路径、语言、导出格式等                     | [app\_config.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/config/app_config.rs)           |
| 转录页面           | 文件上传 + 进度 + 结果展示 + 导出             | [transcribe/page.tsx](file:///d:/Code/Rust/VelociText/src/app/transcribe/page.tsx)             |
| 设置页面           | 配置项表单 + 模型状态展示                    | [settings/page.tsx](file:///d:/Code/Rust/VelociText/src/app/settings/page.tsx)                 |
| 侧边导航           | 四页面切换导航                           | [app-sidebar.tsx](file:///d:/Code/Rust/VelociText/src/components/app-sidebar.tsx)              |
| Tauri Commands | 前端-后端接口层                          | [commands/](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/)                           |
| 错误处理           | 统一错误类型 (thiserror)                | [errors.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/errors.rs)                           |

### 3.2 待完成模块（优先级排序）

#### P0 - 必须完成（阻塞 MVP 交付）

| 任务               | 描述                                                        | 预估工作量 |
| ---------------- | --------------------------------------------------------- | ----- |
| **模型下载**         | 从 GitHub Releases 自动下载 SenseVoice-Small，含进度 + 断点续传        | 2天    |
| **进度实时推送**       | 当前 transcribe\_file 一次性返回结果。需改造为分段流式返回 + Tauri event 推送进度 | 1天    |
| **前后端联调**        | 在真实 Tauri 环境中调通整个流程（目前各模块已独立完成，但未在 app 中完整跑通）             | 1天    |
| **FFmpeg 检测 UI** | 启动时检测 FFmpeg 可用性，不可用时弹窗提示安装引导                             | 0.5天  |
| **错误处理完善**       | 各页面错误状态 UI、用户友好提示、异常恢复                                    | 1天    |
| **旧模板文件清理**      | 删除原 tauri-ui 模板中不再需要的前端组件                                 | 0.5天  |
| **打包配置**         | 配置 Windows .msi 打包参数，验证 `cargo tauri build` 成功            | 1天    |
| **ChangeLog**    | 在 docs/ 目录创建 ChangeLog.md                                 | 0.5天  |

#### P1 - 增强功能（V1.1）

| 任务             | 描述                                       |
| -------------- | ---------------------------------------- |
| **历史记录持久化**    | 将转录任务保存到本地 SQLite/JSON 文件，支持查看、删除        |
| **VAD 语音活动检测** | 集成 sherpa-onnx VAD 实现自动分段，替代当前的单段模式      |
| **拖拽上传**       | 在 Tauri 中实现真正的文件拖拽上传（利用 tauri-plugin-fs） |
| **音频播放器**      | 转录结果页面集成音频播放，点击分段跳转对应时间                  |
| **说话人分离**      | 集成 sherpa-onnx Speaker Diarization       |
| **实时进度条**      | 分段识别时实时更新进度百分比和当前识别的文本                   |

#### P2 - 架构扩展（V2.0）

| 任务         | 描述                                                          |
| ---------- | ----------------------------------------------------------- |
| **多模型工厂**  | RecognizerFactory 抽象层，支持 SenseVoice/Paraformer/Zipformer 切换 |
| **GPU 加速** | DirectML (Windows) / CUDA (NVIDIA) 推理加速                     |
| **批量处理**   | 多文件队列批量转录                                                   |
| **翻译集成**   | 转录结果自动翻译为其他语言                                               |
| **自定义热词**  | 支持自定义词汇表提升特定领域识别率                                           |

***

## 4. 资源需求

### 4.1 开发环境

* Windows 10/11 x64（主要开发平台）

* macOS / Linux（辅助测试）

* Rust 1.80+

* bun latest

* FFmpeg 7.x（系统安装，确保 ffmpeg 和 ffprobe 在 PATH 中）

* sherpa-onnx 依赖的 C++ 工具链（Windows: Visual Studio Build Tools）

### 4.2 模型资源

| 模型                      | 大小      | 来源                                                                            |
| ----------------------- | ------- | ----------------------------------------------------------------------------- |
| SenseVoice-Small ONNX   | \~300MB | [k2-fsa/sherpa-onnx releases](https://github.com/k2-fsa/sherpa-onnx/releases) |
| SenseVoice-Small tokens | \~300KB | 同上                                                                            |

**模型存储策略**：

* 开发阶段：手动下载到 `%APPDATA%/VelociText/models/sense-voice-small/`

* 发布阶段：首次启动自动下载（待实现）

* 配置项：允许用户在设置中修改模型路径

### 4.3 外部依赖

* **FFmpeg**：音视频解码。用户需自行安装。未来可考虑 portable 打包方案。

***

## 5. 测试策略

### 5.1 Rust 后端测试

| 范围                      | 方式                  | 状态                      |
| ----------------------- | ------------------- | ----------------------- |
| engine/export.rs        | `cargo test` 单元测试   | ✅ 已有（SRT/VTT 格式 + 导出内容） |
| engine/audio\_extractor | 手动验证 FFmpeg 转码      | ⬜ 需补自动化测试               |
| engine/recognizer       | 端到端推理测试（需要模型文件）     | ⬜ 需补测试用例                |
| commands                | 集成测试（mock AppState） | ⬜ 待添加                   |

### 5.2 前端测试

| 范围            | 方式                             | 状态      |
| ------------- | ------------------------------ | ------- |
| TypeScript 类型 | `bun run typecheck`            | ✅ 通过    |
| 代码规范          | `bun run lint`                 | ⬜ 需运行验证 |
| 组件测试          | Vitest + React Testing Library | ⬜ 待添加   |

### 5.3 手动测试用例

* [ ] 上传 MP3 文件 → 转录 → 导出 TXT

* [ ] 上传 MP4 视频 → 提取音频 → 转录 → 导出 SRT

* [ ] 设置页面修改配置 → 保存 → 重启应用配置持久化

* [ ] 未安装 FFmpeg 时的错误提示

* [ ] 未下载模型时的错误提示

* [ ] 长音频（>10分钟）转录稳定性

* [ ] 不同语言（中文/英文/粤语）识别

### 5.4 CI/CD

* GitHub Actions：

  * PR 触发：`cargo test` + `bun run lint` + `bun run typecheck`

  * Tag 触发：Release Build（Windows/macOS/Linux，`cargo tauri build`）

***

## 6. 交付标准

### 6.1 MVP 验收清单

* [ ] 用户通过 UI 选择音频/视频文件（MP3/WAV/MP4 等）

* [ ] 系统自动调用 FFmpeg 提取并转码音频为 16kHz WAV

* [ ] SenseVoice-Small 模型成功加载并执行离线 ASR 推理

* [ ] 转录完成后展示完整文本，支持编辑

* [ ] 导出为 .txt 纯文本文件

* [ ] 导出为 .srt 字幕文件（含时间戳）

* [ ] 导出为 .vtt 字幕文件

* [ ] 左侧导航栏正常切换四个页面（转录/历史/设置/模型管理）

* [ ] 设置页面可配置模型路径、语言、导出格式、VAD、FFmpeg 路径

* [ ] 启动时自动检测 FFmpeg 可用性，不可用时给出明确提示

* [ ] 模型未下载时有明确的安装引导

* [ ] 应用打包为 Windows .msi 安装包

* [ ] `bun run typecheck` 无错误

* [ ] `bun run lint` 无错误

* [ ] `cargo test` 全部通过

* [ ] docs/ChangeLog.md 存在且内容完整

### 6.2 质量指标

| 指标                         | 目标              |
| -------------------------- | --------------- |
| 中文识别准确率 (SenseVoice-Small) | 95%+ (安静环境)     |
| 10 分钟音频推理时间                | < 30 秒（4 核 CPU） |
| 内存占用（推理时）                  | < 800MB         |
| 应用包大小                      | < 200MB（不含模型）   |
| 首次启动到可用                    | < 30 秒（含模型加载）   |

***

## 7. 实施路线图

### 当前状态：核心架构已完成

Rust 后端引擎层、Commands 层、前端页面层、类型系统均已完成代码编写。以下为剩余工作路线图：

```
Phase A: 闭环联调 (Week 1)
├── 清理旧模板文件 (未使用的组件)
├── 前后端在真实 Tauri 环境联调
├── 进度实时推送改造 (事件驱动)
├── FFmpeg 启动检测 + UI 提示
├── 错误处理完善
├── 运行 typecheck + lint 验证
└── 更新 ChangeLog

Phase B: 模型自动下载 (Week 2)
├── 实现 model_manager 下载功能
├── 下载进度 UI
├── 断点续传支持
└── 打包验证 (cargo tauri build)

Phase C: 历史 + 打磨 (Week 3)
├── 历史记录持久化 (SQLite)
├── 历史记录 UI 完善
├── 集成测试 + 手动测试
└── 文档完善 (README/用户手册)

Phase D: V1.1 增强 (Week 4-5)
├── VAD 分段集成
├── 拖拽上传支持
├── 音频播放器
└── 说话人分离
```

### 立即下一步（P0 优先）

| # | 任务                                         | 文件                                                                                                                                                                   |
| - | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 | 清理旧模板组件                                    | 删除 `src/components/nav-*.tsx`, `greet.tsx`, `data-table.tsx`, `chart-*.tsx`, `section-cards.tsx`, `debug-panel.tsx`, `theme-provider.tsx`, `external-link-guard.tsx` |
| 2 | 更新 ChangeLog                               | 创建 `docs/ChangeLog.md`                                                                                                                                               |
| 3 | 运行 `bun run typecheck` + `bun run lint` 验证 | -                                                                                                                                                                    |
| 4 | 运行 `cargo test` 验证                         | -                                                                                                                                                                    |
| 5 | 真实 Tauri 环境联调                              | `bun run tauri dev`                                                                                                                                                  |

***

## 8. 关键风险与缓解

| 风险                                  | 影响 | 缓解措施                                                 |
| ----------------------------------- | -- | ---------------------------------------------------- |
| sherpa-onnx Rust crate Windows 编译失败 | 高  | 验证 Windows 编译链完整；已通过 SHERPA\_ONNX\_ARCHIVE\_DIR 手动配置 |
| SenseVoice-Small 模型下载缓慢/失败          | 中  | 支持国内镜像下载；提供手动下载指引                                    |
| FFmpeg 环境差异（用户未安装）                  | 中  | 启动检测 + 引导安装 UI；未来提供 portable 打包                      |
| 长音频内存溢出                             | 中  | 分段处理（每 30s 一段）；stream 模式                             |
| ASR 结果没有精确时间戳                       | 中  | 当前为单段返回。VAD 集成后可获得分段时间戳                              |
| token 文件加载路径问题                      | 低  | Recognizer 中已验证文件存在性                                 |

***

## 9. 附录

### 9.1 关键 API 参考（已实现的 sherpa-onnx 接口）

```rust
// 创建识别器
let config = OfflineRecognizerConfig {
    model_config: OfflineModelConfig {
        sense_voice: OfflineSenseVoiceModelConfig {
            model: Some(model_path.into()),
            ..Default::default()
        },
        ..Default::default()
    },
    ..Default::default()
};
let recognizer = OfflineRecognizer::create(&config);

// 离线识别
let audio = Wave::read(wav_path);
let stream = recognizer.create_stream();
stream.accept_waveform(audio.sample_rate(), audio.samples());
recognizer.decode(&stream);
let result = stream.get_result().map(|r| r.text).unwrap_or_default();
```

### 9.2 补充文件（已完成，从 summary 重建）

以下文件在会话丢失后的恢复中已重新实现：

* [recognizer.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/recognizer.rs)

* [transcriber.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/transcriber.rs)

* [transcribe.rs (commands)](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/transcribe.rs)

* [model.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/model.rs)

* [config.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/config.rs)

* [history.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/history.rs)

* [progress.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/progress.rs)

* [model\_manager.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/model_manager.rs)

* [task.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/models/task.rs)

* [history.rs (models)](file:///d:/Code/Rust/VelociText/src-tauri/src/models/history.rs)

* [language.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/models/language.rs)

* [App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx)

* [app-sidebar.tsx](file:///d:/Code/Rust/VelociText/src/components/app-sidebar.tsx)

* [transcribe/page.tsx](file:///d:/Code/Rust/VelociText/src/app/transcribe/page.tsx)

* [settings/page.tsx](file:///d:/Code/Rust/VelociText/src/app/settings/page.tsx)

* [settings/model-settings.tsx](file:///d:/Code/Rust/VelociText/src/app/settings/model-settings.tsx)

* [history/page.tsx](file:///d:/Code/Rust/VelociText/src/app/history/page.tsx)

* [types/index.ts](file:///d:/Code/Rust/VelociText/src/types/index.ts)

* [textarea.tsx](file:///d:/Code/Rust/VelociText/src/components/ui/textarea.tsx)


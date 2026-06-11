# 转录功能优化 — 对标 sherpa-onnx 官方 Tauri 示例

## 概要

参考 `docs/Reference/tauri-examples/non-streaming-speech-recognition-from-file`（sherpa-onnx 官方 Tauri v2 桌面应用示例），对 VelociText 转录功能进行全面优化，实现**流式 VAD+ASR 分段识别**、**内置播放器+字幕同步**、**渐进式结果推送**、**取消支持**、**分段 WAV 导出**、**VAD 参数可调**等完整功能。

## 当前实现状态

### 已完成 (Backend — Rust)
- ✅ `Cargo.toml`: 添加 symphonia 依赖
- ✅ `engine/audio_decoder.rs`: 纯 Rust 音频解码 (open_audio_file, decode_to_mono_f32, create_resampler, write_wav, decode_time_range)
- ✅ `engine/transcription_pipeline.rs`: 流式转录流水线 (run_recognition, recognize_segment, SegmentResult, VadSettings)
- ✅ `engine/mod.rs`: 注册 audio_decoder + transcription_pipeline 模块
- ✅ `lib.rs`: AppState 扩展 (recognizer, vad_detector, running, cancelled, progress, status, segments, audio_path, init_status, num_threads, vad_settings, elapsed/audio_duration_secs) + 后台模型加载 + build_models + 所有命令注册
- ✅ `commands/transcribe.rs`: 新流式命令 (recognize_file, get_recognition_progress, cancel_recognition, save_segment_as_wav, get_vad_settings, apply_vad_settings, get_init_status) + 保留旧命令 (transcribe_file, transcribe_batch, export_to_file 等)

### 待完成 (Frontend — TypeScript/React)
- ❌ `src/types/index.ts`: 新增流式转录类型
- ❌ `src/lib/app-context.tsx`: 新增翻译键
- ❌ `src/app/transcribe/page.tsx`: 重构为流式转录 UI (播放器+渐进结果+表格+设置+取消)

---

## 变更计划

### 1. 更新 types/index.ts — 新增流式类型

**文件**: [src/types/index.ts](file:///d:/Code/Rust/VelociText/src/types/index.ts)

在前端类型文件中追加以下新类型（对应 Rust 端的 `SegmentResult`、`ProcessingState`、`InitStatus`、`VadSettings`）：

```ts
/** 流式转录分段结果 */
export interface StreamingSegment {
  start: number;
  end: number;
  text: string;
}

/** 轮询获取的处理状态 */
export interface ProcessingState {
  percent: number;
  status: string;        // "idle" | "processing" | "done" | "cancelled" | "error:..."
  segments: StreamingSegment[];
  elapsedSecs: number;
  audioDurationSecs: number;
}

/** 模型初始化状态 */
export interface InitStatus {
  status: number;        // 0=pending, 1=ready, 2=error
  error: string;
  numThreads: number;
}

/** VAD 可调参数 */
export interface VadSettings {
  threshold: number;
  minSilenceDuration: number;
  minSpeechDuration: number;
  maxSpeechDuration: number;
  numThreads: number;
}
```

### 2. 更新 app-context.tsx — 新增翻译键

**文件**: [src/lib/app-context.tsx](file:///d:/Code/Rust/VelociText/src/lib/app-context.tsx)

在 `dict.zh` 和 `dict.en` 中新增以下键：

| Key | 中文 | English |
|-----|------|---------|
| `transcribe.loadingModels` | 正在加载模型... | Loading models... |
| `transcribe.initFailed` | 模型加载失败 | Model initialization failed |
| `transcribe.copyTimed` | 复制带时间戳 | Copy with Timestamps |
| `transcribe.copyText` | 复制文本 | Copy Text |
| `transcribe.saveSegment` | 保存分段 | Save Segment |
| `transcribe.cancel` | 取消 | Cancel |
| `transcribe.cancelled` | 已取消 | Cancelled |
| `transcribe.vadSettings` | VAD 设置 | VAD Settings |
| `transcribe.vadThreshold` | 阈值 | Threshold |
| `transcribe.vadMinSilence` | 最小静音时长 (s) | Min Silence (s) |
| `transcribe.vadMinSpeech` | 最小语音时长 (s) | Min Speech (s) |
| `transcribe.vadMaxSpeech` | 最大语音时长 (s) | Max Speech (s) |
| `transcribe.vadThreads` | 识别线程数 | Recognizer Threads |
| `transcribe.vadApply` | 应用 | Apply |
| `transcribe.rtf` | RTF | RTF |
| `transcribe.settingsReloading` | 重新加载模型... | Reloading models... |
| `transcribe.decoding` | 正在解码... | Decoding audio... |
| `transcribe.done` | 完成 | Done |
| `transcribe.segmentsWithCount` | {count} 个分段 | {count} segment(s) |

### 3. 重构 transcribe/page.tsx — 流式转录 UI

**文件**: [src/app/transcribe/page.tsx](file:///d:/Code/Rust/VelociText/src/app/transcribe/page.tsx)

重构整个转录页面，对标参考示例 `main.js`，分为以下功能模块：

#### 3a. 模型初始化状态轮询

启动时轮询 `get_init_status`（每 300ms），显示 "Loading models..." 直到 `status === 1`：

```tsx
// 组件挂载时
useEffect(() => {
  const poll = setInterval(async () => {
    const res = await invoke<InitStatus>("get_init_status");
    if (res.status === 1) {
      setModelsReady(true);
      clearInterval(poll);
    } else if (res.status === 2) {
      setModelError(res.error);
      clearInterval(poll);
    }
  }, 300);
  return () => clearInterval(poll);
}, []);
```

`selectFile` 按钮在 `modelsReady` 前为 disabled。

#### 3b. 非阻塞转录 + 轮询进度

点击选择文件后：
1. 调用 `invoke("recognize_file", { path })` — 立即返回
2. 启动 200ms 间隔轮询 `invoke<ProcessingState>("get_recognition_progress")`
3. 增量追加新 segments（根据 `lastSegments.length` 判断新增）
4. 更新进度条

```tsx
// 轮询逻辑
const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);

const startPolling = () => {
  pollTimer.current = setInterval(async () => {
    const state = await invoke<ProcessingState>("get_recognition_progress");
    setProgress(state.percent);
    
    const prevLen = segmentsRef.current.length;
    if (state.segments.length > prevLen) {
      setSegments(prev => [...prev, ...state.segments.slice(prevLen)]);
    }
    segmentsRef.current = state.segments;
    
    if (state.status === "done") {
      clearInterval(pollTimer.current!);
      setTranscribeState("completed");
      setElapsedSecs(state.elapsedSecs);
      setAudioDurationSecs(state.audioDurationSecs);
    } else if (state.status === "cancelled") {
      clearInterval(pollTimer.current!);
      setTranscribeState("cancelled");
    } else if (state.status.startsWith("error:")) {
      clearInterval(pollTimer.current!);
      setError(state.status.slice(6));
      setTranscribeState("error");
    }
  }, 200);
};
```

#### 3c. 内置播放器 + 字幕同步

转录完成后，使用 `<video>` 元素加载本地文件：

```tsx
// 使用 Tauri convertFileSrc 获取本地文件 URL
const playerUrl = convertFileSrc(filePath);

// player-wrapper
<div className="relative">
  <video ref={playerRef} src={playerUrl} controls 
    onPlay={startSync} onPause={stopSync} onEnded={stopSync} />
  <div className="subtitle-overlay">
    {activeSubtitle}
  </div>
</div>
```

**字幕同步**：使用 `requestAnimationFrame` 驱动（~60fps），二分查找当前播放时间对应 segment：

```tsx
const tick = () => {
  if (!playerRef.current || playerRef.current.paused) return;
  const t = playerRef.current.currentTime;
  const idx = findSegmentIndex(t); // 二分查找
  setActiveSubtitle(idx >= 0 ? segments[idx].text : "");
  // 高亮对应表格行
  setActiveRowIdx(idx);
  rafId.current = requestAnimationFrame(tick);
};
```

#### 3d. 结果表格 + 点击跳转 + 分段保存

以表格形式展示所有 segment，对标参考示例：

```tsx
<table>
  <thead>
    <tr><th>Start</th><th>End</th><th>Text</th><th></th></tr>
  </thead>
  <tbody>
    {segments.map((seg, i) => (
      <tr key={i} className={i === activeRowIdx ? "active" : ""}
          onClick={() => seekTo(seg.start - 0.3)}>
        <td>{seg.start.toFixed(2)}s</td>
        <td>{seg.end.toFixed(2)}s</td>
        <td>{seg.text}</td>
        <td>
          <button onClick={(e) => { e.stopPropagation(); saveSegment(i); }}>
            💾
          </button>
        </td>
      </tr>
    ))}
  </tbody>
</table>
```

点击行 → 暂停播放 → seek 到 `seg.start - 0.3s` → 播放。
`saveSegment(i)` → 调用 `save` dialog → `invoke("save_segment_as_wav", { path, start, end })`

#### 3e. 操作按钮组

转录完成后显示：
- **Copy Text** — 纯文本合并 → `navigator.clipboard.writeText()`
- **Copy with Timestamps** — `[HH:MM:SS,ms --> HH:MM:SS,ms] text` 格式
- **Export SRT** — 调用 `invoke("export_to_file", { segments, format: "srt", savePath })`
- **Export TXT/VTT** — 复用现有 export 逻辑

#### 3f. VAD 设置弹窗

使用 shadcn/ui `Dialog` 组件 + `Input` 组件，对标参考示例的 Settings Modal：

- VAD Threshold (0.0–1.0, step 0.05)
- Min Silence Duration (s)
- Min Speech Duration (s)
- Max Speech Duration (s)
- Recognizer Threads (1–16)

Apply 按钮：
1. 前端验证
2. `invoke("apply_vad_settings", newSettings)` — 触发后端重建模型
3. 弹窗关闭，显示 "Reloading models..."
4. 重新轮询 `get_init_status`

```tsx
<Dialog>
  <DialogTrigger asChild>
    <Button variant="ghost" size="icon" disabled={!modelsReady || isProcessing}>
      <SettingsIcon />
    </Button>
  </DialogTrigger>
  <DialogContent>
    <DialogHeader>
      <DialogTitle>{t("transcribe.vadSettings")}</DialogTitle>
    </DialogHeader>
    <div className="space-y-4">
      <label>{t("transcribe.vadThreshold")}
        <Input type="number" min={0} max={1} step={0.05} ... />
      </label>
      {/* ... other fields ... */}
    </div>
    <DialogFooter>
      <Button variant="outline" onClick={...}>{t("common.cancel")}</Button>
      <Button onClick={handleApplySettings}>{t("transcribe.vadApply")}</Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
```

#### 3g. 取消转录

转录进行中显示红色取消按钮：

```tsx
{isProcessing && (
  <Button variant="destructive" onClick={handleCancel}>
    <XIcon /> {t("transcribe.cancel")}
  </Button>
)}
```

`handleCancel` → `invoke("cancel_recognition")` → 轮询收到 `status === "cancelled"` → 显示已完成的部分结果。

#### 3h. 完成状态显示

转录完成后显示：
- 分段数量 `{count} segments`
- Audio duration 秒数
- Elapsed time 秒数
- RTF 值 `(=elapsed/audio_duration)` + 速度倍率

### 4. CSS 样式

**文件**: 修改 `src/app/transcribe/page.tsx` 中组件内联样式 或 新增 CSS module

对标参考示例的样式：
- `.player-wrapper`: sticky top, z-index 10
- `.player-container`: relative
- `.subtitle-overlay`: absolute bottom, 居中, 半透明黑底白字
- 表格行 hover 高亮 + active 行蓝色背景
- 进度条动画

由于项目使用 Tailwind CSS + shadcn/ui，部分样式用 Tailwind 类名替代：
- 字幕覆盖层: `absolute bottom-10 left-1/2 -translate-x-1/2 bg-black/70 text-white px-3 py-1 rounded text-center`
- 表格激活行: 通过 className 条件判断 + shadcn Table 组件

---

## 实施顺序

| # | 任务 | 文件 | 说明 |
|---|------|------|------|
| 1 | 新增类型 | `src/types/index.ts` | 追加 StreamingSegment, ProcessingState, InitStatus, VadSettings |
| 2 | 新增翻译 | `src/lib/app-context.tsx` | 追加 ~15 个翻译键 |
| 3 | 重构转录页面 | `src/app/transcribe/page.tsx` | 全部功能：模型轮询、流式转录、播放器+字幕、表格+VAD设置+取消 |
| 4 | 构建验证 | `cargo check` → `bun run tauri build` | 确保编译通过 |
| 5 | 更新 ChangeLog | `docs/ChangeLog.md` | 记录本次优化 |

---

## 假设与决策

1. **后端已完成** — 所有 Rust 端的流式转录命令和流水线已实现，无需修改
2. **前端架构不变** — 使用 React + shadcn/ui + Tailwind CSS
3. **Dialog 使用 shadcn/ui 组件** — 替代参考示例的原生 modal
4. **保留旧 `transcribe_file` / `transcribe_batch` 命令** — 作为 fallback，暂不移除
5. **播放器使用 HTML5 `<video>`** — 对标参考示例，支持音视频播放 + 字幕叠加
6. **shadcn/ui Table 可用** — 当前项目已安装 shadcn/ui

---

## 验证步骤

1. `cargo check` Rust 编译通过
2. `bun run tauri build` 全项目构建通过
3. 启动应用 → 显示 "正在加载模型..." → 加载完成后按钮可用
4. 选择音频文件 → 进度条实时更新 → 结果逐段出现在表格
5. 转录完成后 → 内置播放器可用 → 字幕与视频/音频同步
6. 点击表格行 → 播放器跳转到对应时间
7. 点击取消 → 转录中断 → 显示已识别结果
8. VAD 设置调整 → 模型重新加载 → 影响后续转录
9. 复制纯文本 / 带时间戳文本正常
10. 导出 SRT / TXT 正常
11. 保存单个分段 WAV 正常
12. 生成安装程序可正常运行
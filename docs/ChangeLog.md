# Changelog

All notable changes to VelociText will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.1.8] - 2026-09-10

### 新增
- **"转音频"页面**：功能区新增独立页面（位于"转字幕"下方）。选择一个或多个视频文件，选择目标音频格式（默认 MP3，另支持 WAV、FLAC、M4A、OGG、OPUS、WMA），点击"转音频"即可调用 FFmpeg 批量提取音频并转换格式，输出文件保存到源文件所在文件夹（同名换扩展名）。支持拖拽添加文件、文件列表管理（移除/清空）、逐文件实时转换进度（基于 FFmpeg `-progress` 解析的百分比）、整体进度条与批量完成汇总提示；处理过程中可随时停止，停止会终止 FFmpeg 进程并自动清理未完成的输出文件。FFmpeg 优先使用设置页配置的路径；未检测到 FFmpeg 时页面内提供一键下载。中英文界面均已适配。

### 修复
- **含音乐的视频转录不完整**：修复"朗诵 + 背景歌曲"交替的音视频（如 2 分 13 秒的参考视频）只能转录到约 65 秒、后半段全部丢失的问题。根因有二，均已修复：
  - **Silero VAD LSTM 状态污染**：VAD 是有状态模型（LSTM），持续的音乐输入会将其隐状态推入异常区域，导致后续真正的语音完全检测不到。现新增"能量门控的陈旧状态重置"：当音频可听（RMS > 0.01，即音乐而非静音）且连续 2 秒未检测到语音时，自动重置 VAD 状态并维护采样偏移量保证时间戳正确；纯静音不重置，正常语音分段行为不变。
  - **歌曲歌词对 VAD 不可见**：Silero VAD 对带背景音乐的演唱/说唱不敏感，歌词段会被整段静默丢弃。现新增"长间隙回填"：主流程结束后，对检测到的语音段之间超过 3 秒的间隙重新解码音频，按 12 秒固定块直接送 ASR 识别（低于模型最大输入长度），无文本产出的纯伴奏块自动丢弃，回填结果按开始时间排序合并。
  - 新增回归测试 `test_streaming_transcribe_reference_mp4`（断言转录覆盖 >90% 音频时长），参考视频覆盖率由 49% 提升至 97.7%。
- **FFmpeg 检测忽略已配置路径**：`check_ffmpeg` 命令现优先检测设置页配置的 FFmpeg 路径（例如通过"下载 FFmpeg"安装到应用数据目录的副本），检测不到时才回退到系统 PATH。修复下载安装 FFmpeg 后设置页仍显示"未找到"的问题。
- **音频转换进度读取的潜在死循环**：FFmpeg 进度管道的行读取由 `lines().flatten()` 改为显式错误检查（`Err` 即退出循环），避免 I/O 错误时迭代器反复产生 `Err` 导致的 Clippy 警告及潜在无限循环。

## [v0.1.7] - 2026-09-08

### 新增
- **关闭按钮行为设置**：设置页面新增"关闭按钮行为"配置项，可选"最小化到托盘"（默认，保持原有行为）或"退出应用"。选择"退出应用"后点击窗口右上角关闭按钮将直接退出程序；旧配置文件自动兼容（缺省按托盘处理）。中英文界面均已适配。

### 优化
- **转录/转字幕模型共享，不再重复加载**："转录"与"转字幕"页面使用同一套 ASR 模型。模型释放由"离开页面立即释放"改为**延迟释放**机制：离开页面后模型保留在内存中，5 分钟内无使用才真正释放；期间切换到另一页面（或返回）会自动取消挂起的释放，模型原地复用，无需重新加载（此前页面切换会立即卸载再重新加载模型，耗时数秒）。识别运行中的释放请求会被自动跳过。
- **识别完成后自动回收结果缓冲**：单次识别结束后 60 秒，后端会自动清空并收缩识别结果缓冲区（`segments`），前端已在轮询完成时持有自己的副本，用户无感知；若期间开始了新的识别则自动跳过回收。
- **修复 Clippy 诊断问题**：修复 `uninit_vec` 错误（OCR 的 RGBA→RGB 转换改用安全的 `par_chunks_exact_mut` 并行实现）及约 20 项 Clippy 警告（不必要的类型转换、冗余闭包、`clamp`/`saturating_sub`/`Range::contains` 惯用法、无效的 `..Default::default()`、文档缩进等），当前 `cargo clippy` 全量零警告。

### 移除
- **OCR 功能全量删除**（前端页面已于 v0.1.6 移除，本次清除后端全部残留）：
  - Rust 端：`engine/ocr` 模块（PaddleOCR 引擎）、`commands/ocr.rs` 全部命令（识别/截图/PDF/剪贴板等 15 个）、`AppState` 中的 `ocr_engine`/`active_ocr_model`/`pending_screenshot` 状态、`AppConfig` 的 `active_ocr_model` 字段（旧配置文件中的该字段会被自动忽略）、`AppError::Ocr` 错误变体、模型管理中的 PaddleOCR 模型（v4/v5/v6）下载与列表项、OCR 集成测试。
  - 前端：截图 OCR 窗口入口（`screenshot.html`、`src/screenshot-main.ts`）、vite 构建的 screenshot 入口、`@tauri-apps/plugin-clipboard-manager` 依赖及对应 capabilities 权限。
  - 依赖清理：移除 `paddle-ocr-rs`、`ort`、`pdfium-render`、`xcap`、`arboard`、`image`、`rayon`、`num_cpus`、`tempfile`、`tauri-plugin-clipboard-manager` 共 10 项。安装包体积由 36.8MB 减至 23.7MB（MSI，-35%），NSIS 安装器由 10.3MB 减至 5.3MB（-48%）。
- **未使用的旧转录命令**：移除 `transcribe_file` 与 `transcribe_batch` 命令（前端早已改用流式 `recognize_file` 架构，旧命令每次调用都会临时创建约 500MB 的独立识别器实例，不再有任何调用方）。
- **后端死代码清理**：删除 `Transcriber`/`Recognizer`/`ProgressTracker`、`segment_text`、`extract_audio`、`detect_speech_segments`、`write_text_file`/`open_file_with_system`/`export_result`/`check_file_format`/`get_model_types`/`download_model`/`get_model_path` 等无调用方的命令与函数。集成测试改为直接走真实流式管线（symphonia 解码 → Silero VAD → ASR）。

## [v0.1.6] - 2026-09-07

### 修复
- **主内容区无法滚动**：应用主区域（`SidebarInset`）现为固定视口高度（`h-svh`）的布局，页面内容超出可视区域时可在头部下方的独立滚动容器中上下滚动。修复"转录"页面生成文本较多时无法下拉查看的问题。页面内的吸顶播放器（sticky）随滚动容器正常吸附。
- **"转字幕"页面文件列表过长**：文件列表区域新增内部滚动（最大高度 50vh，表头吸顶），"开始转字幕 / 添加文件 / 清除"等操作按钮始终保持在可视范围内，无需滚动即可操作。

### 新增
- **"转字幕"页面**：功能区内新增独立页面（位于"转录"下方）。用户可选择一个或多个音视频文件，点击"开始转字幕"后，逐个文件执行语音识别并在源文件所在文件夹中生成同名 `.srt` 字幕文件。支持拖拽添加文件、文件列表管理（移除/清空）、逐文件状态展示（等待中/转录进度/写入字幕/已生成/失败/已停止）、整体进度条与批量完成汇总提示，处理过程中可随时停止。字幕时间段基于 Silero VAD 语音分段生成，词典替换规则同样生效。页面进入时懒加载 ASR 模型、离开时自动释放内存，与转录页行为一致。中英文界面均已适配。

### 移除
- **左侧"功能"区"文本识别"（OCR）页面**：侧边栏不再显示该入口，`src/app/ocr/page.tsx` 页面及其相关 i18n 词条、OCR 类型定义一并删除。
- **模型管理页面中的 OCR 相关功能**：移除 OCR 模型（ppocr-v4/v5/v6）选择、下载与安装状态展示，页面仅保留 ASR 模型管理，并清理了对应的无用翻译词条。
- **截图 OCR 快捷键功能**：移除设置页面的"截图 OCR 快捷键"配置项（含快捷键录入、冲突检测逻辑及相关翻译）；同步移除 Rust 端全局快捷键注册、`tauri-plugin-global-shortcut` 插件依赖（前后端）、capabilities 权限声明及 `AppConfig` 中的 `ocrScreenshotShortcut` 配置字段（旧配置文件中的该字段会被自动忽略，不影响加载）。

## [v0.1.5] - 2026-06-16

### Added
- **Audio waveform visualization**: the Transcribe page now displays an interactive audio waveform (powered by wavesurfer.js) for audio files, while video files (mp4, avi, mov, mkv, flv, webm) still use the native `<video>` element for video playback. The waveform shows a moving playback cursor during audio playback, supports click-to-seek, and includes play/pause controls with time display. Subtitle overlay is rendered on top of the waveform for synchronized display. The file type is detected from the extension and the appropriate player is rendered automatically.

### Changed
- **ASR model immediate release**: ASR models are now released immediately when the user navigates away from the Transcribe page (e.g., to the OCR page or other pages), instead of the previous 5-minute delayed release. This frees memory (~400MB) instantly when the models are no longer needed. The `window.__velocitext_release_timer` global variable has been removed.
- **OCR model automatic release**: the OCR engine is now automatically released when the user leaves the OCR page, freeing ~400MB of ONNX Runtime memory. Previously, the OCR engine remained in memory indefinitely after the first OCR operation, causing memory usage to stay at ~1.3GB even after leaving the OCR page. The `ocr_release` command is now called in the page's cleanup effect.
- **OCR image processing optimization**: `recognize_from_image` now only clones the input image when resizing is actually needed (images outside the 960-1920px range). Images in the sweet spot are borrowed directly, avoiding unnecessary multi-MB memory allocations.
- **Batch OCR recognition**: the OCR page now supports selecting or dragging multiple images for batch recognition. Images are automatically processed upon selection — no manual "Start OCR" click needed. Each image shows its own status (processing, completed, error) with expandable detail view. Batch export writes one `_ocr.txt` file per image. "Copy All" copies all recognized text to clipboard. The file dialog now allows multiple selection (`multiple: true`). A "Retry Failed" button is available for re-processing failed items.
- **PDF OCR support**: the OCR page now supports selecting or dragging PDF files. Each PDF page is rendered to an image (at 200 DPI) using `fop-pdf-renderer` (pure Rust, no external DLL dependency), then OCR is performed on each page independently. Results are displayed as separate items per page with the rendered page as preview. Added `pdf_get_page_count`, `pdf_render_page`, and `ocr_recognize_pdf` Tauri commands.
- **Large image OCR speed optimization**: images with a long side exceeding 1920px are now automatically downscaled to 1920px before OCR processing. Previously, all images were upscaled to 1.5x regardless of size, causing unnecessary computation on large images. Small images (< 960px) still get the 1.5x upscale for better accuracy. Images between 960-1920px are used as-is.
- **Screenshot OCR result display**: when the app window is visible, screenshot OCR results now appear in the OCR page's batch list with the same expandable detail view as image-based OCR, including the cropped screenshot preview and individual text blocks with confidence scores.
- **Desktop toast notification for screenshot OCR**: when the app is minimized to the system tray, pressing the screenshot OCR shortcut shows a desktop-level toast notification ("文本复制成功") at 18% screen height with light green background and dark green text. The toast auto-closes after 2.3 seconds. The main window stays hidden.
- **Silent screenshot OCR when minimized**: when the app is minimized to the system tray, pressing the screenshot OCR shortcut no longer shows the main window. The screenshot is taken silently, OCR is performed, and the result is copied to the clipboard.
- **Auto-start OCR on image selection**: selecting or dragging images into the OCR page automatically starts recognition immediately, eliminating the need for a manual "Start OCR" button click.
- **GitHub Actions release workflow**: `.github/workflows/release.yml` now only builds for Windows. macOS (Apple Silicon / Intel) and Linux matrix entries are preserved but commented out for easy future re-enable. The Linux system dependencies step and Rust multi-targets step are also commented out.
- **Eliminated INEFFECTIVE_DYNAMIC_IMPORT warnings**: all `await import("@tauri-apps/api/core")`, `await import("@tauri-apps/api/event")`, and `await import("@tauri-apps/plugin-dialog")` dynamic imports across `ocr/page.tsx`, `settings/page.tsx`, `settings/model-settings.tsx`, `transcribe/page.tsx`, and `history/page.tsx` have been converted to static top-level imports. Since these Tauri API modules are already statically imported by other components (e.g., `App.tsx`, `lib/tauri.ts`), the dynamic imports provided no code-splitting benefit and caused Vite build warnings.

### Fixed
- **Drag-and-drop OCR not working**: the `addFiles` callback captured a stale `modelInstalled` value (always `false` on first render) in the drag-drop event listener closure. Fixed by using `useRef` to track `modelInstalled` and `startBatchOCRForPaths`, ensuring the callback always uses the latest values.
- **OCR engine memory leak**: the OCR engine (ONNX Runtime session, ~400MB) was never released after leaving the OCR page. Fixed by calling `ocr_release` in the page cleanup effect.
- **Potential panic in `build_models`**: `File::create(&tokens_path).unwrap()` could panic if the Paraformer tokens.txt path was not writable. Changed to handle the error gracefully with a log warning.
- **Unsafe `convert_rgba_to_rgb` missing validation**: added `debug_assert!` to verify RGBA data length is divisible by 4 before unsafe operations.
- **Repeated `segments.reduce()` in render**: the Transcribe page called `segments.reduce()` twice per render to compute the total character count. Replaced with a single `useMemo` that recalculates only when `segments` changes.

## [v0.1.4] - 2026-06-15

### Changed
- **ASR model lazy loading**: ASR models (SenseVoice-Small, Paraformer-Large, Qwen3-ASR) are no longer loaded at application startup. Instead, they are loaded on demand when the user navigates to the Transcribe page via the new `ensure_asr_models` command. This reduces startup memory usage from ~500MB to ~50MB. After the user leaves the Transcribe page, a 5-minute inactivity timer starts; if the user does not return within 5 minutes, the ASR models are released from memory via the `release_asr_models` command. Returning to the Transcribe page cancels the timer and reloads models if needed. The `init_status` field now supports value `3` (released) in addition to `0` (pending), `1` (ready), and `2` (error).

### Fixed
- **PP-OCR garbled recognition output for all models**: `paddle-ocr-rs`'s `read_keys_from_file` (used by `init_models_with_dict`) loads `dict.txt` without the blank token `#` at index 0 or space ` ` at the end, unlike `get_keys()` which reads from model metadata and adds both. This caused CTC decoding index offset of 1, producing completely garbled Chinese text. Added `prepare_ocr_dict()` function that generates a corrected `dict_ocr.txt` cache file with `#` prepended and ` ` appended, matching the PaddleOCR Python runtime behavior (`["blank"] + character_str + [" "]`). The original `dict.txt` is never modified.
- **PP-OCR V4 incorrect recognition results**: V4 model now also uses external `dict.txt` (PaddleOCR standard `ppocr_keys_v1.txt`, 6623 characters). The model-embedded `character` metadata field was unreliable, causing completely wrong character mappings. All three models (V4/V5/V6) now use external dictionary files.
- **PP-OCR V5/V6 crash on recognition**: V5 and V6 models use external `dict.txt` for character dictionary instead of the model-embedded `character` metadata field. The `OcrEngine::new_with_memory` and `init_session` methods now check for `dict.txt` in the model directory and use `init_models_with_dict` when available, falling back to the existing metadata-based dictionary loading for compatibility.
- **OCR export TXT error "missing required key segments"**: the OCR page was calling `export_to_file` (which expects `segments, format, save_path` parameters for transcription) instead of the new `write_text_file` command (which accepts `path, content` parameters). Added a generic `write_text_file` Tauri command.
- **Model management page removed "ModelScope.cn" download source text**: the i18n translations for `models.desc.management` and `models.downloadHint` no longer reference ModelScope, since models are downloaded from GitCode.
- **PaddleOCR model download to use correct GitCode path**: download URL changed from `releases/download/ocr/` to `releases/download/model/` to match the actual GitCode release layout.
- **OCR page allowing uninstalled models**: the OCR page now tracks which models are actually installed and shows an amber warning banner when an uninstalled model is selected. Selecting a model without installation no longer triggers OCR recognition (previously it would try and fail silently or produce confusing errors).
- **Missing `detection.onnx` in ppocr-v4.zip and ppocr-v5.zip**: rebuilt both model archives to include the required `detection.onnx` file (from `ch_PP-OCRv4_det_infer.onnx`). Previously the zips only contained `recognition.onnx` and `cls.onnx`, so `is_ppocr_installed_at` would never detect them as installed even after extraction.
- **PaddleOCR model download HTTP status check**: `download_file` now validates HTTP response status codes (2xx required) before writing the response body. Previously, a 404 Not Found response from the server was silently written as a corrupted zip file, causing confusing "Read zip archive failed" errors.

### Changed
- **OCR engine session reuse for ~10x faster subsequent calls**: the ONNX session is now kept alive in `AppState` (references snow-shot's `OcrService` pattern). First OCR call initializes the engine (~1-2s), subsequent calls reuse the cached session (~100-300ms). Model switching releases the old engine and creates a new one. Added `ocr_release` command for explicit resource management.
- **Model settings page redesigned with dropdown selection**: ASR and OCR models are now selected via dropdown menus instead of individual cards. Each dropdown shows model installation status and provides context-aware action buttons (Download / Switch & Restart / Active badge). The layout is cleaner and more compact.
- **PP-OCR V4/V5/V6 model archives now include `dict.txt`**: the zip files for all three models include character dictionary files. V4 uses PaddleOCR standard `ppocr_keys_v1.txt` (6623 chars), V5/V6 dictionaries sourced from [OnnxOCR](https://github.com/jingsongliujing/OnnxOCR). The model downloader also extracts `dict.txt` alongside the ONNX model files.
- **`zip` crate upgraded from 0.6 to 2.x**: with `deflate` and `xz` features to support xz-compressed zip archives (e.g., snow-shot's `rapid_ocr.zip` using method 95). No breaking API changes since only `ZipArchive::new` was used for reading.
- **Sidebar navigation reorganized**: "Dictionary" moved from the Features section to the Settings section (above "About"), grouping it with Settings, Model Management, and About as a configuration-related item.

### Added
- **System tray support**: closing the application window now minimizes to the system tray instead of quitting. Left-click the tray icon to restore the window, right-click to access a context menu with "Show VelociText" and "Quit" options. Uses Tauri's `tray-icon` feature with `TrayIconBuilder` and `MenuBuilder` for cross-platform tray functionality.
- **Single instance application**: only one instance of VelociText can run at a time. If the user tries to launch a second instance, the existing window is shown and focused instead. Uses `tauri-plugin-single-instance`.
- **Global shortcut for screenshot OCR**: the screenshot OCR shortcut (default `Ctrl+Shift+O`, configurable in Settings) now works globally via `tauri-plugin-global-shortcut`, even when the application is minimized to the system tray. The shortcut is registered on the Rust side during app startup (using `GlobalShortcutExt::on_shortcut`), which is more reliable than JS-side registration. When triggered, it emits a `trigger-screenshot-ocr` event to the frontend.
- **Multi-monitor screenshot capture with transparent fullscreen window**: the screenshot OCR feature now creates a separate transparent, frameless, always-on-top window covering all monitors (references snow-shot's `create_draw_window` architecture). Screenshots from each monitor are stitched into a single canvas at 1:1 scale, and mouse coordinates map directly to image pixels — no scaling or offset calculations needed. The `start_screenshot_selection` Rust command handles window creation, and the `screenshot.html` / `screenshot-main.ts` frontend handles region selection. After selection, the screenshot window performs OCR via `invoke("ocr_screenshot_region")`, copies text to clipboard via `invoke("copy_text_to_clipboard")`, notifies the main window via `invoke("screenshot_ocr_done")`, and closes itself via `invoke("close_screenshot_window")`. Vite is configured for multi-page build (`index.html` + `screenshot.html`).
- **PP-OCRv6 ONNX model downloader script** (`tools/download_ppocr_v6.py`): Python script to download pre-converted PP-OCRv6 ONNX models directly from ModelScope (魔搭社区, `https://www.modelscope.cn/collections/PaddlePaddle/PP-OCRv6`). Downloads `PP-OCRv6_small_det_onnx` (detection, 9.4MB) and `PP-OCRv6_small_rec_onnx` (recognition, 20.2MB) from official PaddlePaddle ModelScope repositories. Classification model (`cls.onnx`) reused from PP-OCRv4/V5. Note: the `inference.yml` file shipped alongside the ModelScope ONNX model is not needed by `paddle-ocr-rs` (it uses its own built-in preprocessing config).
- **Screenshot OCR**: capture the entire primary monitor, then select a region by dragging for OCR. Available via a "Screenshot OCR" button in the OCR page toolbar or a configurable keyboard shortcut (default `Ctrl+Shift+O`). The screenshot overlay supports drag-to-select region, ESC to cancel, and a processing indicator. OCR results are automatically copied to the clipboard.
- **Screenshot OCR shortcut configuration**: the keyboard shortcut for screenshot OCR is now configurable in the Settings page with an interactive key recorder. Click the shortcut box to enter recording mode, then press the desired key combination. Built-in conflict detection warns when the shortcut conflicts with system shortcuts (Ctrl+C, Ctrl+V, Alt+Tab, etc.). The shortcut is persisted in `AppConfig.ocrScreenshotShortcut`.
- **OCR completion Toast**: displays a toast notification after OCR recognition completes, showing the number of text blocks detected and the total processing time in seconds. Uses `total_time_ms` field in `OcrResult`.
- **Screenshot OCR success Toast**: after screenshot OCR completes and text is copied to clipboard, a green toast notification ("文本复制成功") appears at the top-center of the screen for 2 seconds with a fade-out animation.
- **Raw RGBA OCR optimization**: `recognize_from_raw_rgba` method on `OcrEngine` bypasses PNG encoding/decoding for screenshot OCR. The `ocr_screenshot` command now passes raw RGBA pixel data directly from `xcap` capture to the OCR engine, avoiding the overhead of PNG compression and decompression. References snow-shot's SharedBuffer zero-copy approach.
- **OCR optimization summary document**: `docs/OCR优化总结.md` documents all OCR performance optimizations in Chinese, covering model loading, ONNX inference, image preprocessing, dictionary correction, and feature optimizations.
- **Rust clipboard command**: `copy_text_to_clipboard` command uses `arboard` crate directly to write text to the system clipboard, avoiding the "Document is not focused" error that occurs with the browser Clipboard API. Applied to both screenshot OCR auto-copy and manual copy button.

### Changed
- **OCR page renamed to "Text Recognition" (文本识别)**: sidebar navigation label and page header changed from "OCR" to "Text Recognition" in both Chinese and English locales.
- **OCR completion time now displayed in seconds**: the completion toast and result display now show time in seconds (e.g., "1.2s") instead of milliseconds. The i18n translations for `ocr.completedToast` have been updated from "ms" to "秒" (Chinese) and "s" (English).

## [v0.1.3] - 2026-06-14

### Added
- **OCR page**: Optical Character Recognition using ONNX-format PaddleOCR models (V4/V5/V6)
  - Drag-and-drop or file picker to load images (PNG, JPG, JPEG, BMP, WEBP, TIFF)
  - Model version selector (V4/V5/V6) with runtime switching (no restart required)
  - Image preview with OCR results panel showing extracted text and confidence scores
  - Copy all text to clipboard or export as TXT file
  - `paddle-ocr-rs` crate for PaddleOCR ONNX inference via ONNX Runtime (`ort`)
  - OCR models (detection.onnx + recognition.onnx + cls.onnx) downloadable from Model Settings page
  - Full i18n support (Chinese and English) for all OCR UI strings
- **OCR engine improvements** (referencing [snow-shot](https://github.com/mg-chao/snow-shot)):
  - `detect_angle_rollback` with 0.9 rollback threshold to reduce false angle corrections on screenshots
  - Parallel RGBA→RGB conversion via `rayon` for faster screenshot processing
  - Model in-memory loading (`new_with_memory`) for faster initialization
  - Scale factor handling with Lanczos3 resizing for low-resolution images
  - Session lifecycle management (`release_session`/`init_session`) to free ONNX Runtime resources
  - `ocr_recognize_bytes` Tauri command for direct byte-array OCR (no temp file needed)
- **OCR model management**: PaddleOCR V4/V5/V6 entries in Model Settings page
  - Download from gitcode.com (`https://gitcode.com/tabortao/VelociText/releases/download/model/ppocr-v*.zip`)
  - Install status tracking and download progress
  - Active OCR model persisted in `AppConfig.activeOcrModel`

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
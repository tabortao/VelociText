# OCR 优化总结

本文档总结了 VelociText 项目中 OCR（光学字符识别）功能的所有性能优化措施，参考了 [snow-shot](https://github.com/mg-chao/snow-shot) 项目的实现。

---

## 一、模型加载优化

### 1.1 ONNX 模型内存预加载 (`new_with_memory`)

**原理：** 在引擎初始化时，使用 `std::fs::read` 将检测模型 (`detection.onnx`)、识别模型 (`recognition.onnx`)、方向分类模型 (`cls.onnx`) 的三个 ONNX 文件一次性读取到内存中，后续初始化 ONNX Session 时直接从内存加载，避免磁盘 I/O。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `OcrEngine::new_with_memory()`

**效果：** 首次初始化约 1-2 秒，后续重建 Session 无需重新读取磁盘。

### 1.2 ONNX Session 复用 (`AppState` 缓存)

**原理：** 将 `OcrEngine` 实例保存在 `AppState.ocr_engine: Arc<Mutex<Option<OcrEngine>>>` 中。首次 OCR 调用创建引擎并缓存，后续调用直接复用，避免每次 OCR 都重新初始化 ONNX Session。

**参考：** snow-shot 的 `OcrService` 模式。

**实现位置：** `src-tauri/src/commands/ocr.rs` — 所有 OCR 命令

**效果：** 后续 OCR 调用从 ~1-2s 降至 ~100-300ms（约 10 倍提速）。

### 1.3 Session 生命周期管理

**原理：** 提供 `release_session()` 释放 ONNX 资源，`init_session()` 从预加载的内存数据重建 Session。模型切换时释放旧引擎并创建新引擎。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `OcrEngine::release_session()` / `OcrEngine::init_session()`

---

## 二、ONNX 推理优化

### 2.1 线程配置 (`build_ocr_session`)

**原理：** 使用 `num_cpus::get_physical()` 获取物理核心数，同时设置 `inter_threads`（操作间并行度）和 `intra_threads`（操作内并行度），使用 `GraphOptimizationLevel::Level3` 级别的 ONNX 图优化。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `build_ocr_session()`

**效果：** 充分利用 CPU 多核并行能力，加速推理。

### 2.2 角度检测阈值优化 (`detect_angle_rollback`)

**原理：** 使用 `detect_angle_rollback` 方法，设置 `rollback_threshold = 0.9`。截图和文档场景中文字排版多为横向，减少不必要的角度旋转校正（仅当角度分类置信度 > 0.9 时才执行旋转），避免误判。

**参考：** snow-shot 的 `ocr_detect_core`。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `recognize_from_image()` 和 `recognize_from_raw_rgba()`

---

## 三、图像预处理优化

### 3.1 并行 RGBA → RGB 转换 (`convert_rgba_to_rgb`)

**原理：** 使用 `rayon` 的 `into_par_iter()` 并行处理像素数据，结合 `unsafe` 的 `ptr::copy_nonoverlapping` 进行零拷贝的逐像素 RGBA→RGB 转换（跳过 Alpha 通道）。

**参考：** snow-shot 的 `convert_rgba_to_rgb`。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `convert_rgba_to_rgb()`

**效果：** 对大分辨率截图（如 4K 屏幕），并行转换显著减少预处理时间。

### 3.2 Lanczos3 高质量缩放

**原理：** 当图像 `scale_factor < 1.5` 时，使用 Lanczos3 滤波算法将图像放大到等效 1.5x 缩放。Lanczos3 比双线性/双三次插值保留更多细节，有利于小文字识别。

**参考：** snow-shot 的 `ocr_detect_core`。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `recognize_from_image()`

### 3.3 截图 RGBA 直传（避免 PNG 编解码）

**原理：** 截图 OCR 场景中，`xcap` 直接返回 RGBA 像素数据。传统流程需要 `RGBA → PNG 编码 → 传输 → PNG 解码 → RGB`，现在新增 `recognize_from_raw_rgba()` 方法直接接收 RGBA 原始数据，跳过 PNG 编解码环节。

**参考：** snow-shot 的 SharedBuffer 零拷贝方案。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `recognize_from_raw_rgba()` / `src-tauri/src/commands/ocr.rs` — `ocr_screenshot`

**效果：** 消除 PNG 压缩/解压的 CPU 开销，对大分辨率屏幕效果显著。

---

## 四、字符字典优化

### 4.1 字典文件自动修正 (`prepare_ocr_dict`)

**原理：** `paddle-ocr-rs` 的 `read_keys_from_file` 直接加载 `dict.txt`，但 CTC 解码要求索引 0 为空白符 `#`、末尾为空格 ` `。`prepare_ocr_dict()` 生成修正后的 `dict_ocr.txt` 缓存文件，在不修改原始文件的前提下满足 CTC 解码要求。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `prepare_ocr_dict()`

---

## 五、功能优化

### 5.1 截图 OCR 快速识别

**原理：** 用户可通过按钮或快捷键（默认 `Ctrl+Shift+O`，可在设置中自定义）一键截取屏幕、OCR 识别并自动复制结果到剪贴板。

**实现位置：**
- 后端：`src-tauri/src/commands/ocr.rs` — `ocr_screenshot`
- 前端：`src/app/ocr/page.tsx` — `handleScreenshotOCR`

### 5.2 识别耗时统计

**原理：** 在 OCR 方法入口记录 `Instant::now()`，完成后计算 `elapsed` 并填入 `OcrResult.total_time_ms`，前端通过 Toast 展示。

**实现位置：** `src-tauri/src/engine/ocr/mod.rs` — `recognize_from_image()` / `recognize_from_raw_rgba()`

---

## 六、优化效果总结

| 优化项 | 类型 | 效果 |
|--------|------|------|
| Session 复用 | 模型加载 | 后续 OCR 调用 ~10x 提速 |
| 内存预加载 | 模型加载 | 减少磁盘 I/O，加速 Session 重建 |
| 线程配置 | 推理 | 充分利用多核 CPU |
| 角度阈值 | 推理 | 减少误判旋转，提升准确率 |
| 并行 RGBA→RGB | 预处理 | 大图预处理显著加速 |
| Lanczos3 缩放 | 预处理 | 小文字识别精度提升 |
| RGBA 直传 | 数据传输 | 消除 PNG 编解码开销 |
| 字典修正 | 精度 | 修复 CTC 解码索引偏移，消除乱码 |
| 截图 OCR | 功能 | 一键截图 + 识别 + 复制 |
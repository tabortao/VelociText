# VelociText 截图 OCR 实现原理

本文档描述 VelociText 项目中「截图 OCR」功能的完整实现方案，参考了 snow-shot 项目的截图架构。

## 1. 整体架构

截图 OCR 涉及两个独立的 Tauri Webview 窗口：

| 窗口 | 标签 | 入口 HTML | 入口 JS | 职责 |
|------|------|-----------|---------|------|
| 主窗口 | `main` | `index.html` | `main.tsx` → App.tsx | 提供 UI、响应用户操作、显示 OCR 结果 |
| 截图窗口 | `screenshot` | `screenshot.html` | `screenshot-main.ts` | 覆盖全屏显示截图、接收鼠标选区、执行 OCR |

```
用户操作（点击按钮 / 按快捷键）
    │
    ▼
主窗口 App.tsx  ── invoke("start_screenshot_selection") ──▶  Rust 后端
    │                                                              │
    │                                                              ├─ 调用 xcap 捕获所有显示器
    │                                                              ├─ 拼接为一张大图存到临时文件
    │                                                              ├─ 将截图数据存入 AppState
    │                                                              └─ 创建透明全屏窗口 (screenshot.html)
    │
    ▼
截图窗口 (screenshot.html + screenshot-main.ts)
    │
    ├─ 启动后 invoke("get_screenshot_data") 拉取截图数据
    ├─ 1:1 原始尺寸显示截图
    ├─ 监听 mousedown/mousemove/mouseup → 绘制选区
    ├─ 松开鼠标 → invoke("ocr_screenshot_region") 裁剪 + OCR
    ├─ invoke("copy_text_to_clipboard") 复制到剪贴板
    ├─ invoke("screenshot_ocr_done") 通知主窗口结果
    └─ invoke("close_screenshot_window") 关闭自身
    │
    ▼
主窗口收到 screenshot-ocr-result 事件 → 显示绿色 Toast + OCR 结果
```

## 2. 关键依赖

| 依赖 | 用途 |
|------|------|
| `xcap` | 跨平台屏幕捕获，支持多显示器枚举和并行截图 |
| `image` (Rust crate) | 图像拼接（`imageops::overlay`）、裁剪（`crop_imm`） |
| `arboard` | 系统剪贴板写入（避免浏览器 Clipboard API 的「Document is not focused」错误） |
| `tauri-plugin-global-shortcut` | 全局键盘快捷键（即使应用在托盘也能触发） |
| `tauri-plugin-single-instance` | 单实例限制 |
| `paddle-ocr-rs` + `ort` | OCR 推理核心 |

Tauri 权限（`capabilities/default.json`）需要为 `main` 窗口配置：
```json
"global-shortcut:allow-register",
"global-shortcut:allow-unregister",
"global-shortcut:allow-unregister-all",
"global-shortcut:allow-is-registered"
```

## 3. 核心流程详解

### 3.1 触发入口

截图可以通过两种方式触发：

**方式 A — 点击按钮**（主窗口 UI）

[App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx) 中 `triggerScreenshot()` 调用：
```typescript
await invoke("start_screenshot_selection", { modelVersion })
```

**方式 B — 全局快捷键**（Rust 端注册）

[lib.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/lib.rs) 的 `setup` 阶段注册：
```rust
let shortcut = Shortcut::new(Some(modifiers), code);
app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
    if event.state() == ShortcutState::Pressed {
        // 显示主窗口（如果在托盘）+ emit trigger-screenshot-ocr 事件
        app.emit("trigger-screenshot-ocr", ());
    }
})?;
```

主窗口 [App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx) 监听此事件后同样调用 `triggerScreenshot()`。

快捷键字符串（如 `Ctrl+Shift+O`）通过 [parse_shortcut_string](file:///d:/Code/Rust/VelociText/src-tauri/src/lib.rs#L378-L418) 解析为 `Modifiers` + `Code`。

### 3.2 屏幕捕获与拼接

Rust 命令 [start_screenshot_selection](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/ocr.rs#L237-L294) 内部调用 [capture_all_monitors_inner](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/ocr.rs#L344-L406)：

```
xcap::Monitor::all()          → 枚举所有显示器
并行 capture_image()          → 每个显示器截一张图
计算 boundingBox              → 所有显示器的联合外接矩形
image::RgbaImage::new(W, H)  → 创建合并画布
对每张子图执行 overlay()     → 按物理坐标偏移叠加
保存 PNG 到临时目录           → %TEMP%\velocitext_screenshot_all.png
```

**关键点**：xcap 返回的每个 Monitor 都有物理坐标 `x()` / `y()`，比如双显示器可能是 `[0,0] 1920×1080` 和 `[1920,0] 1920×1080`。拼接时用 `offset_x = x - min_x` 把物理坐标转为画布内的偏移。

返回数据结构：
```json
{
  "imagePath": "...png 路径...",
  "width": 3840,
  "height": 1080,
  "boundingBox": { "minX": 0, "minY": 0, "maxX": 3840, "maxY": 1080, "width": 3840, "height": 1080 }
}
```

### 3.3 截图窗口创建

同样在 `start_screenshot_selection` 中：

```rust
tauri::WebviewWindowBuilder::new(app, "screenshot",
    tauri::WebviewUrl::App("screenshot.html".into()))
    .title("VelociText Screenshot")
    .inner_size(bbox_width, bbox_height)      // 精确等于所有显示器的总像素
    .position(min_x, min_y)                   // 定位到最左上角显示器的左上角
    .transparent(true)                         // 透明，让图片之外的部分透出
    .decorations(false)                       // 无标题栏
    .always_on_top(true)                       // 置顶，不被其他窗口遮挡
    .skip_taskbar(true)                        // 不显示在任务栏
    .resizable(false)
    .shadow(false)
    .build()
```

关键配置：窗口的 `inner_size` 和 `position` 必须精确匹配所有显示器的总尺寸和物理坐标原点，这样窗口的 1 CSS 像素 = 1 物理像素，鼠标坐标 `clientX`/`clientY` 就等于图片像素坐标。

### 3.4 数据传递机制（重要）

截图数据先存到 `AppState.pending_screenshot`（`Arc<Mutex<Option<Value>>>`），截图窗口加载完成后**主动 `invoke("get_screenshot_data")`** 拉取。

为什么不用事件推送？因为如果 Rust 创建窗口后立即 `emit`，JS 可能还没加载完，事件会丢失（时序问题）。让截图窗口主动 pull 就彻底避免了这个问题。

### 3.5 前端选区交互

[screenshot-main.ts](file:///d:/Code/Rust/VelociText/src/screenshot-main.ts) 做了三件事：

**1. 加载截图**
```typescript
const data = await invoke("get_screenshot_data")
img.src = convertFileSrc(data.imagePath)
img.style.width = data.width + "px"    // 1:1 原始尺寸
img.style.height = data.height + "px"
```

**2. 鼠标事件** — 只监听 `document` 的 mousedown/mousemove/mouseup。用 `Math.min(startX, e.clientX)` + `Math.abs` 处理反方向拖拽。最小选区 10×10 像素以下视为误操作。

**3. 执行 OCR 并清理**
```
松开鼠标
  → invoke("ocr_screenshot_region", { imagePath, x, y, width, height, modelVersion })
  → invoke("copy_text_to_clipboard", { text })
  → invoke("screenshot_ocr_done", { text, timeMs })
  → invoke("close_screenshot_window")
```

ESC 键取消：直接 `close_screenshot_window`。

### 3.6 OCR 命令

[ocr_screenshot_region](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/ocr.rs#L110-L160) 的流程：
```
image::open(image_path)  → 加载完整截图
crop_imm(x, y, w, h)     → 裁剪选区
OcrEngine.recognize_from_image(cropped, 1.0)
```

`OcrEngine` 缓存在 `AppState.ocr_engine` 中（`Arc<Mutex<Option<OcrEngine>>>`），首次调用创建，后续调用复用 ONNX Session（约 10× 提速）。切换模型时在 `ocr_set_active_model` 中 release。

### 3.7 剪贴板

[copy_text_to_clipboard](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/ocr.rs#L162-L176) 使用 `arboard::Clipboard::set_text` 直接写入系统剪贴板。不用浏览器的 `navigator.clipboard.writeText`，因为在非聚焦窗口（截图窗口、系统托盘状态）浏览器 Clipboard API 会抛「Document is not focused」错误。

### 3.8 结果回传

`screenshot_ocr_done` 命令 emit 到 `main` 窗口：
```rust
main_window.emit("screenshot-ocr-result", { text, timeMs })
```

主窗口 App.tsx 收到后：
1. 导航到 OCR 页面
2. 通过 `CustomEvent("velocitext:screenshot-ocr-result")` 传给 OCR 页面组件
3. 屏幕上方 18% 位置显示绿色 Toast「文本复制成功」

### 3.9 Vite 多页面构建

[vite.config.ts](file:///d:/Code/Rust/VelociText/vite.config.ts) 配置了两个入口：
```typescript
build: {
  rollupOptions: {
    input: {
      main: path.resolve(__dirname, "src/index.html"),       // 主窗口
      screenshot: path.resolve(__dirname, "src/screenshot.html")  // 截图窗口
    }
  }
}
```

`tauri.conf.json` 的 `frontendDist` 指向 `../dist`，Tauri 会根据 WebviewUrl 加载对应的 HTML。

## 4. 关键设计决策

| 问题 | 决策 | 理由 |
|------|------|------|
| 如何覆盖多显示器？ | 创建独立的透明全屏窗口，尺寸等于所有显示器的联合外接矩形 | 这样 CSS 像素 = 物理像素，选区坐标与图片坐标一一对应，无需缩放计算 |
| 如何传递截图数据？ | AppState 缓存 + 截图窗口主动 invoke 拉取 | 避免 Rust 先 emit 但 JS 还没加载的时序问题 |
| 如何触发全局快捷键？ | Rust 端注册（`GlobalShortcutExt::on_shortcut`） | 更可靠，不依赖前端是否加载完成 |
| 如何写剪贴板？ | Rust 端 `arboard` 库 | 浏览器 Clipboard API 在非聚焦窗口会失败 |
| 如何缓存 OCR 引擎？ | AppState 中 `Arc<Mutex<Option<OcrEngine>>>` | ONNX Session 创建耗时 1-2 秒，复用后提速 10 倍 |
| 如何关闭窗口？ | 让截图窗口自己 invoke `close_screenshot_window` | 截图窗口没有 @tauri-apps/npm 包，用 `window.__TAURI__` 全局 API |

## 5. 文件清单

| 文件 | 角色 |
|------|------|
| [src-tauri/src/commands/ocr.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/ocr.rs) | 截图 + OCR + 剪贴板全部 Tauri 命令 |
| [src-tauri/src/lib.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/lib.rs) | 插件注册、托盘、全局快捷键、单实例、AppState |
| [src-tauri/src/engine/ocr/mod.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/ocr/mod.rs) | OcrEngine（PaddleOCR ONNX 封装、Session 复用、词典修正） |
| [src/screenshot.html](file:///d:/Code/Rust/VelociText/src/screenshot.html) | 截图窗口 HTML 入口 |
| [src/screenshot-main.ts](file:///d:/Code/Rust/VelociText/src/screenshot-main.ts) | 截图窗口前端逻辑（选区 + OCR + 关闭） |
| [src/App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx) | 主窗口：监听全局快捷键事件、触发截图、显示结果 Toast |
| [vite.config.ts](file:///d:/Code/Rust/VelociText/vite.config.ts) | Vite 多页面构建配置 |
| [src-tauri/capabilities/default.json](file:///d:/Code/Rust/VelociText/src-tauri/capabilities/default.json) | Tauri 权限配置 |

## 6. snow-shot 对照

本项目的截图实现参考了 [snow-shot](https://github.com/mg-chao/snow-shot) 的以下设计：

- `xcap` + `image` crate 做屏幕捕获和多显示器拼接
- `create_draw_window` 架构：透明无边框置顶窗口覆盖所有显示器
- OCR 引擎 Session 复用模式（`OcrService`）
- 全局快捷键在 Rust 端注册

不同之处：
- snow-shot 使用 Qt 进行图像预处理，本项目全部用纯 Rust `image` crate
- snow-shot 通过 WebSocket 让 Qt 后端与 JS 通信，本项目直接用 Tauri `invoke` + 事件
- snow-shot 有 SharedBuffer 零拷贝传递原始 RGBA，本项目走 PNG 临时文件（未来可优化）

# Paraformer-large ASR 模型集成 & 词典功能 开发计划

## 概要

为 VelociText 新增 Paraformer-large 中文语音识别模型支持（ONNX 格式），并新增「词典」功能模块替代「历史记录」，词典包含「替换词典」（结果后处理）和「专有名词」（热词增强）。不影响现有 SenseVoice-Small 模型。

***

## 当前状态分析

### 已有架构 (Phase 1 探索结果)

**Rust 后端 — 模型层**

* [recognizer\_factory.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/recognizer_factory.rs): `ModelType` 枚举已有 `ParaformerLarge` 变体，`RecognizerFactory::create()` 已支持 `OfflineParaformerModelConfig`，热词通过 `RecognizerConfig.hotwords_file` 传入 ✓ **已完成**

* [model\_manager.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/model_manager.rs): `ModelManager` 已有 `download_paraformer_large()` 方法，支持从 ModelScope 下载并自动转换 `tokens.json` → `tokens.txt` ✓ **已完成**

* [commands/model.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/model.rs): 已有 `download_paraformer_large` Tauri 命令 ✓ **已完成**

* [vad.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/engine/vad.rs): 使用 sherpa-onnx Silero VAD，已实现通用语音分段（读取 WAV → Silero VAD → 返回 `VadSegment` 列表），与模型类型无关 ✓ **已有**

* [commands/transcribe.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/transcribe.rs): `transcribe_file` 已支持通过 `options.model_type` 选择模型类型，VAD+ASR 混合流水线已就绪，`ParaformerLarge` 已添加 ✓ **已完成**

* [lib.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/lib.rs): `download_paraformer_large` 命令已注册 ✓ **已完成**

**前端**

* [App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx): `Page` 类型为 `"transcribe" | "history" | "settings" | "model-settings" | "about"` ⚠ 需要更新

* [app-sidebar.tsx](file:///d:/Code/Rust/VelociText/src/components/app-sidebar.tsx): 导航项：转录、历史记录、设置(含模型管理)、关于 ⚠ 需要替换历史记录为词典

* [model-settings.tsx](file:///d:/Code/Rust/VelociText/src/app/settings/model-settings.tsx): 模型列表，显示 SenseVoice-Small 和 Silero VAD ⚠ 需要添加 Paraformer-Large 卡片和独立下载处理

* [app-context.tsx](file:///d:/Code/Rust/VelociText/src/lib/app-context.tsx): 中英文翻译字典 ⚠ 需要添加词典相关翻译

* [types/index.ts](file:///d:/Code/Rust/VelociText/src/types/index.ts): TypeScript 类型定义 ⚠ 需要添加词典类型

* [app\_config.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/config/app_config.rs): 应用配置 ⚠ 需要扩展支持默认模型类型

**关键发现**

* sherpa-onnx `OfflineRecognizerConfig` 已支持 `hotwords_file` 和 `hotwords_score`（当前 `RecognizerFactory::create()` 已传入这些参数）

* fbank 特征提取由 sherpa-onnx 内部处理，无需手动实现

* tokens.json → tokens.txt 转换：ModelScope 官方模型使用 `tokens.json`（含 id/token 的 JSON 数组），sherpa-onnx 需要 `tokens.txt`（每行一个 token 文本）

* 后端已自动完成 `model_quant.onnx` → `model.onnx` 重命名，兼容识别器工厂的候选文件查找逻辑

***

## 变更计划

### 1. 前端模型设置页 — Paraformer-Large 支持 (Frontend)

**涉及文件**: [model-settings.tsx](file:///d:/Code/Rust/VelociText/src/app/settings/model-settings.tsx)

**变更**:

* `modelDescriptions` 添加 `paraformer-large` 描述键

* `handleDownload` 需要改为按模型分发：`paraformer-large` → 调用 `download_paraformer_large`，其它 → `download_model`

* 渲染时，每个模型独立调用对应的下载处理，而不是全局一个 `handleDownload`

### 2. 词典功能 — 后端 (Rust)

**涉及文件**:

* 新建 `src-tauri/src/engine/dictionary.rs` — 词典数据结构和管理

* 新建 `src-tauri/src/commands/dictionary.rs` — 词典 Tauri 命令

* [lib.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/lib.rs) — 注册新命令

* [errors.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/errors.rs) — 新增错误类型（可选）

**数据模型**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dictionary {
    /// 替换词典: 错误词→正确词
    pub replacements: Vec<ReplacementEntry>,
    /// 专有名词 (热词)
    pub hotwords: Vec<HotwordEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementEntry {
    pub wrong: String,
    pub correct: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotwordEntry {
    pub word: String,
    pub score: f64, // 默认 1.5
}
```

**存储**: JSON 文件 `{app_data_dir}/VelociText/dictionary.json`

**命令**:

* `get_dictionary()` → 返回 `Dictionary`

* `save_dictionary(dictionary: Dictionary)` → 保存到 JSON 文件

* `generate_hotwords_file()` → 将专有名词导出为临时 `hotwords.txt`（每行一个词），返回文件路径

### 3. 词典功能 — 前端类型

**涉及文件**: [types/index.ts](file:///d:/Code/Rust/VelociText/src/types/index.ts)

**变更**: 新增对应 TypeScript 接口：

```ts
export interface ReplacementEntry {
  wrong: string;
  correct: string;
}

export interface HotwordEntry {
  word: string;
  score: number;
}

export interface Dictionary {
  replacements: ReplacementEntry[];
  hotwords: HotwordEntry[];
}
```

### 4. 词典功能 — 前端页面

**涉及文件**:

* 新建 `src/app/dictionary/page.tsx` — 词典页面组件

* [App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx) — 新增 `dictionary` 到 `Page` 类型，导入组件，添加路由 case

* [app-context.tsx](file:///d:/Code/Rust/VelociText/src/lib/app-context.tsx) — 新增翻译键（见下文）

**页面布局**:

* 使用 shadcn/ui `Tabs` 组件，两个 Tab：

  1. **替换词典**: 表格形式，每行：错误词输入框 → 正确词输入框 → 删除按钮，底部「添加」按钮和「保存」按钮

  2. **专有名词**: 表格形式，每行：词语输入框 → 权重(score)数字输入 → 删除按钮，底部「添加」按钮和「保存」按钮

* 页面标题：中文「词典」/ English「Dictionary」

* 图标：使用 `BookOpenIcon` (lucide-react)

### 5. AppConfig 扩展 (Rust + TypeScript)

**涉及文件**:

* [app\_config.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/config/app_config.rs): 新增字段 `default_model_type: String`（默认 `"sense-voice"`）

* [types/index.ts](file:///d:/Code/Rust/VelociText/src/types/index.ts): `AppConfig` 接口同步新增 `defaultModelType: string`

### 6. 侧边栏调整 (Frontend)

**涉及文件**:

* [app-sidebar.tsx](file:///d:/Code/Rust/VelociText/src/components/app-sidebar.tsx): 移除 `history` 导航项，新增 `dictionary`，替换 `ClockIcon` 为 `BookOpenIcon`，更新 labels 翻译

* [App.tsx](file:///d:/Code/Rust/VelociText/src/App.tsx): `Page` 类型移除 `"history"` → 新增 `"dictionary"`，`renderPage()` 添加 `dictionary` case

* **决策**: 保留 `HistoryPage` 组件和后端历史命令代码不删除，仅从侧边栏移除导航入口（用户要求"去除历史记录"指 UI 层面）

### 7. 翻译更新 (app-context.tsx)

**涉及文件**: [app-context.tsx](file:///d:/Code/Rust/VelociText/src/lib/app-context.tsx)

新增键（中英文都要）:

| Key                           | 中文                                         | English                                                        |
| ----------------------------- | ------------------------------------------ | -------------------------------------------------------------- |
| `features.dictionary`         | 词典                                         | Dictionary                                                     |
| `header.dictionary`           | 词典                                         | Dictionary                                                     |
| `dictionary.title`            | 词典                                         | Dictionary                                                     |
| `dictionary.desc`             | 管理替换词典和专有名词                                | Manage replacement dictionary and proper nouns                 |
| `dictionary.replacements`     | 替换词典                                       | Replacement Dictionary                                         |
| `dictionary.hotwords`         | 专有名词                                       | Proper Nouns                                                   |
| `dictionary.replacementsDesc` | 将识别结果中的错误词自动替换为正确词                         | Auto-replace incorrect words in recognition results            |
| `dictionary.hotwordsDesc`     | 添加人名、地名、品牌等专有名词以提高识别准确率                    | Add names, places, brands to improve recognition accuracy      |
| `dictionary.wrongWord`        | 错误词                                        | Wrong Word                                                     |
| `dictionary.correctWord`      | 正确词                                        | Correct Word                                                   |
| `dictionary.word`             | 词语                                         | Word                                                           |
| `dictionary.score`            | 权重                                         | Score                                                          |
| `dictionary.add`              | 添加                                         | Add                                                            |
| `dictionary.save`             | 保存                                         | Save                                                           |
| `dictionary.saved`            | 已保存                                        | Saved                                                          |
| `dictionary.empty`            | 暂无词条                                       | No entries yet                                                 |
| `models.paraformerLargeDesc`  | Paraformer-Large 中文高精度模型 · \~238MB (quant) | Paraformer-Large High-accuracy Chinese Model · \~238MB (quant) |

### 8. 转录集成 — 热词+替换词典 (Rust)

**涉及文件**: [commands/transcribe.rs](file:///d:/Code/Rust/VelociText/src-tauri/src/commands/transcribe.rs)

**流程**:

1. 在 `transcribe_file` 开始处：

   * 如果有专有名词，调用 `dictionary.generate_hotwords_file()` 生成临时热词文件

   * 将热词文件路径传入 `RecognizerConfig.hotwords_file`

2. ASR 识别完成得到 `segments` 后：

   * 应用替换词典：遍历每个 segment，对 `text` 执行所有替换规则（按词典顺序替换）

3. 返回替换后的 segments

### 9. 版本信息更新

**涉及文件**:

* [app-sidebar.tsx](file:///d:/Code/Rust/VelociText/src/components/app-sidebar.tsx): 版本号更新为 `v0.2.0-20260611`

* [docs/ChangeLog.md](file:///d:/Code/Rust/VelociText/docs/ChangeLog.md): 新增 v0.2.0-20260611 版本，记录本次变更

***

## 假设与决策

1. **tokens.json 格式**: 假设 ModelScope 的 `tokens.json` 格式为 `[{"id": 0, "token": "<s>"}, ...]`，转换时取 `token` 字段 ✓ 后端已实现

2. **am.mvn**: 下载保留，若 sherpa-onnx `OfflineParaformerModelConfig` 需要 CMVN 归一化参数，该文件已存在 ✓ 后端已下载

3. **热词 scores**: 默认 1.5，用户可在 UI 中自定义

4. **替换词典后处理**: 在 ASR 完成后、返回结果前执行，对每个 segment 的 text 做字符串替换

5. **历史记录保留**: 虽从侧边栏移除导航，后端 Rust `history` 命令和前端 `HistoryPage` 组件不删除（代码保留，方便以后恢复）

6. **Paraformer-large 独立下载**: 不与 SenseVoice-Small 捆绑，用户在模型管理页单独点击下载 ✓ 后端已实现

7. **临时热词文件**: 每次转录生成新的临时文件，使用 `tempfile` crate 管理，自动清理

8. **词典存储**: 保存在应用数据目录，独立于模型，所有模型共享词典

***

## 验证步骤

1. `cargo check` Rust 编译通过

2. 模型管理页出现 Paraformer-Large 卡片，可点击下载

3. 下载进度正常显示，下载完成后显示"已安装"状态

4. 词典页：两个 Tab 可正常添加/编辑/保存/删除条目

5. 使用 `docs/test2.mp3` 测试 Paraformer-Large 转录，识别结果正常

6. 专有名词热词：添加热词后重复转录，热词应被优先识别

7. 替换词典：添加替换规则后转录，结果中的错误词应被替换

8. VAD 分段正常工作（与 SenseVoice 相同）

9. `bun run tauri build` 全项目构建通过

10. 生成的安装程序可正常运行，侧边栏无历史记录，有词典入口


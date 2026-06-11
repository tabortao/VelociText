# 词典页面 & 统一热词管理器 实现计划

## Summary

在转录页面下方增加"词典"面板，包含两个 Tab：

1. **替换词典**：后处理文本替换（orig → repl），解决 ASR 输出中专有名词识别不准确的问题
2. **专有名词（热词）**：sherpa-onnx hotwords 机制，提升特定词汇的 ASR 识别准确率

通过统一的 Rust 后端管理器（`HotwordsManager`），为 Paraformer、Qwen3-ASR、SenseVoice-Small 三种模型统一提供热词支持，修改后立即重建模型生效。

***

## Current State Analysis

### 现有架构

**Rust 后端**：

* `src-tauri/src/engine/recognizer_factory.rs` — `RecognizerConfig` 已有 `hotwords_file: Option<String>` 和 `hotwords_score: f32` 字段，`OfflineRecognizerConfig` 顶层支持 `hotwords_file`/`hotwords_score`，对所有模型类型统一生效

* `src-tauri/src/lib.rs` — `build_models()` 在 line 119-125 创建 `RecognizerConfig`，当前 `hotwords_file: None` 硬编码

* `src-tauri/src/config/app_config.rs` — `AppConfig` 持久化到 `{APPDATA}/VelociText/config.json`

* `src-tauri/src/commands/transcribe.rs` — 转录命令入口

* `src-tauri/Cargo.toml` — 依赖 serde, serde\_json 已就绪

**前端**：

* `src/app/transcribe/page.tsx` — 转录页面布局：`<div className="space-y-4">` > `<Card>` > VAD Modal。页面底部无其他内容

* `src/lib/app-context.tsx` — i18n 上下文，中英文键值模式

* `src/app/settings/model-settings.tsx` — 模型管理页面，已有的表单/保存模式参考

### 技术基础

sherpa-onnx v1.13 的 `OfflineRecognizerConfig` 提供了：

* `hotwords_file: Option<String>` — 热词文件路径，每行 `word weight` 格式

* `hotwords_score: f32` — 热词加权分数（默认 1.5）

* `hr: HomophoneReplacerConfig` — 同音词替换器（需要 dict + lexicon + .fst 文件，实现复杂度高，本计划暂不采用，改用简单文本后处理）

### 关键约束

* 热词修改后需要**立即重建模型**（用户选择），类似 VAD 设置修改流程

* 热词对所有 ASR 模型统一生效

* 配置持久化到 JSON 文件

***

## Proposed Changes

### 1. Rust: 词典配置模块 (NEW)

**文件**: `src-tauri/src/config/dictionary_config.rs`

```rust
/// 热词条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotwordEntry {
    pub word: String,
    pub weight: f32,  // 1.0-5.0
}

/// 替换条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementEntry {
    pub original: String,
    pub replacement: String,
}

/// 词典配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DictionaryConfig {
    pub hotwords: Vec<HotwordEntry>,
    pub replacements: Vec<ReplacementEntry>,
}
```

**功能**：

* load/save 到 `{app_data_dir}/dictionary.json`

* `generate_hotwords_file()` → 生成临时 `hotwords.txt`，每行 `word weight`

* `apply_replacements(text: &str)` → 逐条应用替换规则到文本

***

### 2. Rust: 字典管理命令 (NEW)

**文件**: `src-tauri/src/commands/dictionary.rs`

新增 Tauri commands：

| Command                  | 功能                          |
| ------------------------ | --------------------------- |
| `get_dictionary_config`  | 返回完整 `DictionaryConfig`     |
| `save_hotwords`          | 保存热词列表，生成 hotwords.txt，重建模型 |
| `save_replacements`      | 保存替换词典列表                    |
| `get_hotwords_file_path` | 返回当前 hotwords.txt 路径        |

**模型重建流程**（类似 VAD 设置 `apply_vad_settings`）：

1. 保存 `DictionaryConfig` 到磁盘
2. 调用 `generate_hotwords_file()` 生成临时文件
3. 调用 `build_models()` 重建识别器（传入新 hotwords\_file 路径）
4. 更新 `AppState` 中的 recognizer

***

### 3. Rust: lib.rs 修改

**文件**: `src-tauri/src/lib.rs`

**AppState 新增字段**：

```rust
pub dictionary_config: Arc<Mutex<DictionaryConfig>>,
pub hotwords_file_path: Arc<Mutex<Option<String>>>,
```

**初始化**：

* 启动时调用 `DictionaryConfig::load()` 加载配置

* 如果有热词，调用 `generate_hotwords_file()` 生成文件

* 将路径传入 `build_models()`

**build\_models 修改**（line 119-125）：

```rust
// Before:
hotwords_file: None,

// After:
hotwords_file: hotwords_file_path.clone(),
```

**register\_commands**：注册新的 dictionary commands

***

### 4. Rust: transcription\_pipeline.rs 修改

**文件**: `src-tauri/src/engine/transcription_pipeline.rs`

在 `SegmentResult.text` 赋值后应用替换词典：

```rust
// After ASR recognition:
segment.text = apply_replacements(&dictionary_config.replacements, &segment.text);
```

***

### 5. Rust: 模块注册

**文件**: `src-tauri/src/config/mod.rs`

* 添加 `pub mod dictionary_config;`

**文件**: `src-tauri/src/commands/mod.rs`

* 添加 `pub mod dictionary;`

***

### 6. 前端: 词典面板组件 (NEW)

**文件**: `src/app/transcribe/dictionary-panel.tsx`

组件结构：

```tsx
<Card>
  <CardHeader>
    <CardTitle>{t("dictionary.title")}</CardTitle>
    <CardDescription>{t("dictionary.desc")}</CardDescription>
  </CardHeader>
  <CardContent>
    <Tabs defaultValue="replacements">
      <TabsList>
        <TabsTrigger value="replacements">替换词典</TabsTrigger>
        <TabsTrigger value="hotwords">专有名词(热词)</TabsTrigger>
      </TabsList>
      
      {/* Tab 1: 替换词典 */}
      <TabsContent value="replacements">
        <Table>
          <TableHeader><TableRow><TableHead>原文</TableHead><TableHead>替换为</TableHead><TableHead /></TableRow></TableHeader>
          <TableBody>
            {replacements.map((entry, i) => (
              <TableRow key={i}>
                <TableCell>{entry.original}</TableCell>
                <TableCell>{entry.replacement}</TableCell>
                <TableCell><Button onClick={() => removeReplacement(i)}>删除</Button></TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        <div className="flex gap-2 mt-2">
          <Input placeholder="原文" value={newOrig} onChange={...} />
          <Input placeholder="替换为" value={newRepl} onChange={...} />
          <Button onClick={addReplacement}>添加</Button>
        </div>
      </TabsContent>
      
      {/* Tab 2: 专有名词(热词) */}
      <TabsContent value="hotwords">
        <!-- Same pattern with word + weight (1.0-5.0) -->
      </TabsContent>
    </Tabs>
  </CardContent>
</Card>
```

**功能**：

* 从 Rust 加载 `DictionaryConfig`

* 替换词典：增/删/改原文→替换词对

* 热词：增/删/改词汇→权重（1.0-5.0）

* 保存时调用 `save_hotwords` / `save_replacements`

* 热词保存触发模型重建（显示 loading 状态）

***

### 7. 前端: transcribe/page.tsx 修改

在现有 `<Card>`（转录卡片）下方添加词典面板：

```tsx
// Line 918, before </div> (closing of space-y-4)
<div className="px-4 lg:px-6 space-y-4">
  <Card>...</Card>  {/* 现有转录卡片 */}
  <DictionaryPanel />  {/* NEW: 词典面板 */}
  {showSettings && ...}  {/* VAD 设置弹窗 */}
</div>
```

***

### 8. 前端: i18n 键值

**文件**: `src/lib/app-context.tsx`

新增键值：

```typescript
// 中文
"dictionary.title": "词典",
"dictionary.desc": "管理替换词典和专有名词热词，提高识别准确率",
"dictionary.replacements": "替换词典",
"dictionary.replacementsDesc": "将识别结果中的特定文本替换为正确内容",
"dictionary.hotwords": "专有名词（热词）",
"dictionary.hotwordsDesc": "提高人名、地名、品牌等专有名词的识别准确率",
"dictionary.original": "原文",
"dictionary.replacement": "替换为",
"dictionary.word": "词汇",
"dictionary.weight": "权重",
"dictionary.add": "添加",
"dictionary.delete": "删除",
"dictionary.saving": "保存并重建模型...",

// 英文
"dictionary.title": "Dictionary",
"dictionary.desc": "Manage replacement dictionary and hotwords to improve recognition accuracy",
"dictionary.replacements": "Replacement Dictionary",
"dictionary.replacementsDesc": "Replace specific text in recognition results with correct content",
"dictionary.hotwords": "Proper Nouns (Hotwords)",
"dictionary.hotwordsDesc": "Improve recognition accuracy for names, places, brands, etc.",
"dictionary.original": "Original",
"dictionary.replacement": "Replacement",
"dictionary.word": "Word",
"dictionary.weight": "Weight",
"dictionary.add": "Add",
"dictionary.delete": "Delete",
"dictionary.saving": "Saving & rebuilding model...",
```

***

### 9. 前端: TypeScript 类型

**文件**: `src/types/index.ts`（或新建 `src/types/dictionary.ts`）

```typescript
export interface HotwordEntry {
  word: string
  weight: number
}

export interface ReplacementEntry {
  original: string
  replacement: string
}

export interface DictionaryConfig {
  hotwords: HotwordEntry[]
  replacements: ReplacementEntry[]
}
```

***

## Assumptions & Decisions

| 决定                                    | 理由                                                                                                              |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| 替换词典采用简单文本后处理                         | homophone-replacer 需要 dict.tar.bz2 + lexicon.txt + OpenFST 生成 `.fst`，引入大量额外依赖（\~10MB+ 工具链），MVP 阶段采用简单文本替换，后续可升级 |
| 热词文件生成到 `{app_data_dir}/hotwords.txt` | 与 `config.json` 同目录，统一管理                                                                                        |
| 热词保存后立即重建模型                           | 用户明确选择"保存后立即重建模型"，复用 VAD 设置重建流程                                                                                 |
| 替换词典应用到 `SegmentResult.text`          | 在 Rust 管道中统一处理，确保所有输出格式（TXT/SRT/Copy）一致                                                                         |
| 使用 shadcn/ui Tabs 组件                  | 项目已使用 shadcn/ui（Card, Button, Badge, Separator, Input, Label, Skeleton 等），Tabs 组件一致                             |

***

## Verification Steps

1. `cargo check` — Rust 编译无错误
2. `bun run tauri build` — 完整构建成功
3. 启动应用 → 转录页面下方出现"词典"面板
4. 切换到"专有名词"Tab → 添加热词 "OpenAI 2.0" → 保存 → 模型重建
5. 转录音频 → 验证热词生效（OpenAI 被识别为 "OpenAI" 而非其他变体）
6. 切换到"替换词典"Tab → 添加替换 "苹果" → "Apple" → 保存
7. 转录音频 → 验证替换生效（输出中 "苹果" 被替换为 "Apple"）
8. 切换模型（SenseVoice → Paraformer → Qwen3-ASR）→ 验证热词在所有模型中生效


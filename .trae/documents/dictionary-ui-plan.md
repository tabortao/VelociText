# Dictionary Panel Implementation Plan

## Summary

Add a dictionary panel below the transcription page with two tabs:
- **替换词典 (Replacements)**: Post-processing text replacement rules (original → replacement)
- **专有名词 (Hotwords)**: ASR hotwords with weights to boost recognition accuracy

The Rust backend (config, commands, model rebuild) is already implemented. This plan covers the remaining work: applying replacements in the pipeline and building the frontend UI.

---

## Current State Analysis

### Rust Backend (DONE)
- `src-tauri/src/config/dictionary_config.rs`: `DictionaryConfig` struct with `HotwordEntry` / `ReplacementEntry`, load/save from `AppData\Roaming\VelociText\dictionary.json`, `generate_hotwords_file()` produces `hotwords.txt` in `word weight` format, `apply_replacements()` performs text substitution
- `src-tauri/src/commands/dictionary.rs`: 4 Tauri commands (`get_dictionary_config`, `save_hotwords`, `save_replacements`, `get_hotwords_file_path`), `save_hotwords` rebuilds recognizer automatically
- `src-tauri/src/lib.rs`: `AppState` has `dictionary_config` and `hotwords_file_path` fields; `build_models()` accepts `hotwords_file: Option<String>`; all 4 commands registered in `invoke_handler`
- `src-tauri/src/config/mod.rs`: `pub mod dictionary_config;`
- `src-tauri/src/commands/mod.rs`: `pub mod dictionary;`

### Missing: Replacements Not Applied in Streaming Pipeline
- `recognize_segment()` in `transcription_pipeline.rs` stores raw ASR text without applying replacements
- `get_recognition_progress()` returns raw segments without replacements
- `export_to_file()` exports raw segments without replacements
- Copy functions on frontend copy raw text

### Missing: Frontend
- No TypeScript types for dictionary entries
- No i18n keys for dictionary UI
- No dictionary panel component
- Transcription page doesn't include dictionary panel

---

## Proposed Changes

### 1. Rust: Apply replacements in transcription flow

**File:** `src-tauri/src/commands/transcribe.rs`

**Change:** In `get_recognition_progress()`, apply replacements to segments before returning them.

**Why:** The simplest single-point approach — all consumers (polling frontend display, copy, export) get replaced text. This avoids modifying the pipeline internals or the export function separately.

**How:**
```rust
pub fn get_recognition_progress(state: State<'_, AppState>) -> Result<ProcessingState, String> {
    let percent = state.progress.load(Ordering::Relaxed);
    let status = state.status.lock().map_err(|e| e.to_string())?.clone();
    let raw_segments = state.segments.lock().map_err(|e| e.to_string())?.clone();
    let elapsed_secs = *state.elapsed_secs.lock().map_err(|e| e.to_string())?;
    let audio_duration_secs = *state.audio_duration_secs.lock().map_err(|e| e.to_string())?;

    // Apply text replacements
    let dict_config = state.dictionary_config.lock().map_err(|e| e.to_string())?;
    let segments: Vec<SegmentResult> = raw_segments
        .into_iter()
        .map(|s| SegmentResult {
            text: dict_config.apply_replacements(&s.text),
            ..s
        })
        .collect();
    drop(dict_config);

    Ok(ProcessingState { percent, status, segments, elapsed_secs, audio_duration_secs })
}
```

**Note:** The old `export_to_file` command receives segments from the frontend (already replaced), so no change needed there.

---

### 2. Frontend: TypeScript types

**File:** `src/types/index.ts`

**Change:** Add dictionary-related type definitions at the end of the file.

**New types:**
```typescript
/** 热词条目 */
export interface HotwordEntry {
  word: string;
  weight: number;
}

/** 替换条目 */
export interface ReplacementEntry {
  original: string;
  replacement: string;
}

/** 词典配置 */
export interface DictionaryConfig {
  hotwords: HotwordEntry[];
  replacements: ReplacementEntry[];
}
```

---

### 3. Frontend: i18n keys

**File:** `src/lib/app-context.tsx`

**Change:** Add dictionary-related i18n keys in both `zh` and `en` dictionaries.

**New keys (zh):**
```typescript
// Dictionary
"dictionary.title": "词典",
"dictionary.desc": "添加专有名词和替换规则以提高识别准确率",
"dictionary.hotwords": "专有名词",
"dictionary.hotwordsDesc": "添加人名、地名、品牌等专有名词，提高语音识别准确率。每行一个词，可设置权重。",
"dictionary.replacements": "替换词典",
"dictionary.replacementsDesc": "识别结果中的文本替换规则。如将识别错误的词替换为正确写法。",
"dictionary.word": "词语",
"dictionary.weight": "权重",
"dictionary.original": "原文",
"dictionary.replacement": "替换为",
"dictionary.add": "添加",
"dictionary.remove": "删除",
"dictionary.save": "保存",
"dictionary.saving": "保存中...",
"dictionary.saved": "已保存",
"dictionary.rebuilding": "正在重建模型...",
"dictionary.empty": "暂无条目",
"dictionary.emptyHotwords": "暂无专有名词，点击上方添加",
"dictionary.emptyReplacements": "暂无替换规则，点击上方添加",
"dictionary.weightHint": "权重 (0.0-10.0)，越高越优先",
"dictionary.wordPlaceholder": "输入专有名词",
"dictionary.originalPlaceholder": "输入待替换文本",
"dictionary.replacementPlaceholder": "输入替换后文本",
"dictionary.hotwordSaved": "热词已保存，模型正在重建...",
"dictionary.replacementSaved": "替换规则已保存",
```

**New keys (en):**
```typescript
// Dictionary
"dictionary.title": "Dictionary",
"dictionary.desc": "Add proper nouns and replacement rules to improve recognition accuracy",
"dictionary.hotwords": "Hotwords",
"dictionary.hotwordsDesc": "Add proper nouns like names, places, brands to boost recognition accuracy. One word per line, with adjustable weight.",
"dictionary.replacements": "Replacements",
"dictionary.replacementsDesc": "Post-processing text replacement rules. Replace incorrectly recognized words with correct spellings.",
"dictionary.word": "Word",
"dictionary.weight": "Weight",
"dictionary.original": "Original",
"dictionary.replacement": "Replacement",
"dictionary.add": "Add",
"dictionary.remove": "Remove",
"dictionary.save": "Save",
"dictionary.saving": "Saving...",
"dictionary.saved": "Saved",
"dictionary.rebuilding": "Rebuilding model...",
"dictionary.empty": "No entries",
"dictionary.emptyHotwords": "No hotwords yet, add one above",
"dictionary.emptyReplacements": "No replacement rules yet, add one above",
"dictionary.weightHint": "Weight (0.0-10.0), higher = more priority",
"dictionary.wordPlaceholder": "Enter a proper noun",
"dictionary.originalPlaceholder": "Enter text to replace",
"dictionary.replacementPlaceholder": "Enter replacement text",
"dictionary.hotwordSaved": "Hotwords saved, rebuilding model...",
"dictionary.replacementSaved": "Replacement rules saved",
```

---

### 4. Frontend: Dictionary panel component

**File:** `src/components/dictionary-panel.tsx` (NEW)

**What:** A card component with two tabs using shadcn/ui `Tabs` component.

**Structure:**
```
<Card>
  <CardHeader>
    <CardTitle>词典</CardTitle>
    <CardDescription>添加专有名词和替换规则...</CardDescription>
  </CardHeader>
  <CardContent>
    <Tabs defaultValue="hotwords">
      <TabsList>
        <TabsTrigger value="hotwords">专有名词</TabsTrigger>
        <TabsTrigger value="replacements">替换词典</TabsTrigger>
      </TabsList>

      <TabsContent value="hotwords">
        <!-- Add hotword form: word input + weight input + Add button -->
        <!-- Table: word | weight | delete button -->
        <!-- Save button (triggers model rebuild) -->
      </TabsContent>

      <TabsContent value="replacements">
        <!-- Add replacement form: original input + replacement input + Add button -->
        <!-- Table: original | replacement | delete button -->
        <!-- Save button (no rebuild needed) -->
      </TabsContent>
    </Tabs>
  </CardContent>
</Card>
```

**State management:**
- `hotwords: HotwordEntry[]` — loaded from `get_dictionary_config` on mount
- `replacements: ReplacementEntry[]` — loaded from `get_dictionary_config` on mount
- `saving: boolean` — saving state indicator
- `rebuilding: boolean` — model rebuilding indicator (hotwords only)

**Key behaviors:**
- Load config from backend on component mount via `invoke("get_dictionary_config")`
- Add/remove entries locally (optimistic UI)
- Save button calls `invoke("save_hotwords", { hotwords })` or `invoke("save_replacements", { replacements })`
- For hotwords: saving triggers model rebuild, show "Rebuilding..." status
- For replacements: saving is instant, show "Saved" flash
- While model is rebuilding (after hotwords save), disable the Start Transcription button
- Input validation: weight must be 0.0-10.0, word must not be empty

---

### 5. Frontend: Integrate dictionary panel into transcribe page

**File:** `src/app/transcribe/page.tsx`

**Change:** Add the `<DictionaryPanel />` component below the main transcription card, inside the `space-y-4` container.

**How:**
```tsx
import { DictionaryPanel } from "@/components/dictionary-panel"

// ... inside the JSX, after the closing </Card> of the transcription card:
<DictionaryPanel />
```

**Also:** While the dictionary panel is saving hotwords (rebuilding), the transcription should be blocked. The `DictionaryPanel` can expose a callback or use the `modelsReady` state. Simplest approach: when hotwords save triggers model rebuild, the dictionary panel can emit a "rebuilding" state that the parent page uses to disable the file selection UI.

---

### 6. Build & Changelog

**Commands:**
```
bun run tauri build
```

**File:** `docs/ChangeLog.md`

**Change:** Add entry for v0.1.2-20260611 (or current date version) documenting the dictionary feature.

---

## Assumptions & Decisions

1. **Replacements applied in `get_recognition_progress`**: Single point of application, all downstream consumers (display, copy, export) get replaced text. The stored raw segments remain unchanged, but this is acceptable since the dictionary config is available at query time.

2. **Model rebuild on hotwords save**: Reuses the existing `rebuild_recognizer` logic in `commands/dictionary.rs`. The frontend should show a loading state while the model rebuilds.

3. **No model rebuild on replacements save**: Replacements are post-processing only, no ASR model change needed.

4. **UI uses shadcn/ui Tabs**: Already available in `src/components/ui/tabs.tsx` (Radix-based).

5. **Dictionary panel is collapsible**: The `Card` component naturally takes only the space it needs. No special collapse logic needed.

6. **State shared via props/events**: The dictionary panel and transcription page communicate via the shared `modelsReady` state. When hotwords save triggers rebuild, `modelsReady` becomes `false` temporarily.

---

## Verification

1. `cargo check` to verify Rust changes compile
2. `bun run tauri build` to verify full build succeeds
3. Manual test: add a hotword, save, verify model rebuilds and hotword is recognized
4. Manual test: add a replacement rule, save, verify text is replaced in transcription results
5. Manual test: switch between tabs, verify data persists
6. Manual test: verify i18n works in both Chinese and English
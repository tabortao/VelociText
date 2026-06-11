# Plan: Add Paraformer Model Support & Update README

## Summary

Two tasks:

1. Update README.md and README-zh.md to reflect current project state
2. Add Paraformer-large ONNX model support with ModelScope download, model switching, and VAD optimization

## Current State Analysis

### Model Architecture

* **RecognizerFactory** (`src-tauri/src/engine/recognizer_factory.rs`): Already has `ModelType::Paraformer` enum variant with `dir_name = "paraformer"`, `model_file_candidates = ["model.onnx", "model.int8.onnx"]`

* **ModelManager** (`src-tauri/src/engine/model_manager.rs`): Only lists/downloads SenseVoice-Small + Silero VAD; no Paraformer download

* **build\_models** (`src-tauri/src/lib.rs`): Falls back to `"paraformer-large"` dir but `ModelType::Paraformer.dir_name()` returns `"paraformer"` — **mismatch bug**

* **Model Settings UI** (`src/app/settings/model-settings.tsx`): Single download button for all models; no model selection UI

* **Commands** (`src-tauri/src/commands/model.rs`): `download_model` downloads SenseVoice + VAD only; `download_paraformer_large` is commented out

### ModelScope Paraformer Model

* Repository: `iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx`

* Files: `model_quant.onnx` (238MB), `tokens.json` (94KB), `am.mvn`, `config.yaml`

* **Key issue**: sherpa-onnx expects `model.int8.onnx` + `tokens.txt`, but ModelScope provides `model_quant.onnx` + `tokens.json`

* **Solution**: Download `model_quant.onnx` and rename to `model.int8.onnx`; download `tokens.json` and rename to `tokens.txt`

### Reference Example Paraformer Config

```rust
config.model_config.paraformer = OfflineParaformerModelConfig {
    model: p("model.int8.onnx"),
};
config.model_config.tokens = p("tokens.txt");
config.model_config.model_type = Some("paraformer".into());
config.model_config.num_threads = 2;
config.rule_fsts = p("itn_zh_number.fst"); // ITN FST for Chinese number normalization
```

### VAD Considerations

* Paraformer is Chinese-only, so VAD can be more aggressive (shorter min\_silence\_duration)

* Current VAD settings: threshold=0.2, min\_silence=0.2, min\_speech=0.2, max\_speech=10.0

* Reference example uses the same VAD for all models, but Paraformer benefits from slightly different settings

* The `max_speech_duration` for Paraformer should be larger (e.g., 30s) since it handles long utterances better than SenseVoice

## Proposed Changes

### Task 1: Update README.md and README-zh.md

**Files**: `README.md`, `README-zh.md`

Update both files to reflect:

* Remove FFmpeg dependency (now using symphonia)

* Update supported formats (remove AVI/FLV, add actual supported formats)

* Add Paraformer-Large to tech stack and roadmap (mark as supported)

* Update model management description (multiple models)

* Remove FFmpeg from prerequisites and acknowledgments

* Add symphonia to acknowledgments

### Task 2: Add Paraformer Model Support

#### 2.1 Fix dir\_name mismatch bug

**File**: `src-tauri/src/lib.rs` (line 56)

Change `"paraformer-large"` to `"paraformer"` in `build_models()` to match `ModelType::Paraformer.dir_name()`.

#### 2.2 Add Paraformer download to ModelManager

**File**: `src-tauri/src/engine/model_manager.rs`

* Add `download_paraformer_large()` method

* Download from ModelScope: `https://www.modelscope.cn/models/iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx/resolve/master/{filename}`

* Files to download:

  * `model_quant.onnx` → rename to `model.int8.onnx` (238MB)

  * `tokens.json` → rename to `tokens.txt` (94KB)

* Store in `{models_dir}/paraformer/` directory

* Add `is_paraformer_installed_at()` check function

* Update `list_models()` to include Paraformer entry

#### 2.3 Add model\_type field to Paraformer config

**File**: `src-tauri/src/engine/recognizer_factory.rs`

In `build_model_config()` for `ModelType::Paraformer`, add:

```rust
model_type: Some("paraformer".into()),
```

This matches the reference example and ensures sherpa-onnx correctly identifies the model type.

#### 2.4 Add model switching support

**File**: `src-tauri/src/lib.rs`

* Modify `build_models()` to accept a `model_type` parameter (or detect from config)

* Store active model type in `AppState`

* Add `active_model` field to `AppState` (e.g., `Arc<Mutex<String>>`)

**File**: `src-tauri/src/commands/model.rs`

* Add `switch_model` command that rebuilds recognizer with the selected model type

* Update `download_model` to also support downloading Paraformer

* Add `download_paraformer` command

* Add `get_active_model` command

* Add `set_active_model` command

#### 2.5 Update model settings UI

**File**: `src/app/settings/model-settings.tsx`

* Show all available models (SenseVoice-Small, Paraformer-Large, Silero VAD)

* Add individual download buttons per model

* Add model selection (radio button or dropdown) to choose active ASR model

* Show which model is currently active

* Add model descriptions for Paraformer

**File**: `src/lib/app-context.tsx`

* Add i18n keys for Paraformer model name and description

#### 2.6 VAD optimization for Paraformer

**File**: `src-tauri/src/engine/recognizer_factory.rs`

* Paraformer handles longer utterances better; adjust default VAD `max_speech_duration` when Paraformer is active

* The VAD settings are already configurable via `apply_vad_settings`, so no hardcoded changes needed

* When switching to Paraformer, suggest optimized VAD settings (max\_speech\_duration=30.0)

**File**: `src-tauri/src/lib.rs`

* When building models with Paraformer, use `max_speech_duration: 30.0` as default (vs 10.0 for SenseVoice)

#### 2.7 Update types

**File**: `src/types/index.ts`

* Add `activeModel` field to relevant types if needed

## Implementation Order

1. Fix `build_models` dir\_name mismatch
2. Add Paraformer download to ModelManager
3. Add `model_type` to Paraformer config in RecognizerFactory
4. Add model switching commands (switch\_model, get\_active\_model, set\_active\_model)
5. Update model settings UI with model selection and individual downloads
6. Add i18n keys for Paraformer
7. VAD optimization for Paraformer
8. Update README.md and README-zh.md
9. Build and verify
10. Update ChangeLog.md

## Assumptions & Decisions

1. **Model file renaming**: `model_quant.onnx` → `model.int8.onnx`, `tokens.json` → `tokens.txt`. This is safe because sherpa-onnx only cares about the file content, not the original filename.
2. **No ITN FST**: The reference example uses `itn_zh_number.fst` for Chinese number normalization, but this file is not available on the ModelScope Paraformer repository. We skip it for now — Paraformer still works without it, just without number normalization (e.g., "一百二十三" won't be converted to "123").
3. **Model storage**: Paraformer stored in `{models_dir}/paraformer/` matching `ModelType::Paraformer.dir_name()`.
4. **Active model persistence**: The selected model type will be stored in `AppConfig` and persisted across restarts.
5. **VAD sharing**: Both models share the same Silero VAD instance. VAD settings are adjusted when switching models.

## Verification Steps

1. Download Paraformer model via UI → verify files appear in `paraformer/` directory
2. Switch to Paraformer model → verify transcription works with Chinese audio
3. Switch back to SenseVoice → verify it still works
4. Verify model persistence across app restarts
5. Verify build succeeds with `bun run tauri build`


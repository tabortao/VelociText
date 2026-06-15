# OCR Feature Plan: PaddleOCR ONNX Integration

## Summary

Add an OCR page to VelociText supporting PaddleOCR V4/V5/V6 ONNX models. Users can drag-and-drop or open images for text recognition. Models can be downloaded from the Model Settings page.

## Current Status

Most code has been written in a prior session. The following files are already in place and need verification/fixes:

**Rust Backend (complete, needs compilation verification):**
- `src-tauri/Cargo.toml` — dependencies added (`paddle-ocr-rs = "0.6"`, `ort = "2.0.0-rc.12"`)
- `src-tauri/src/engine/ocr/mod.rs` — `OcrEngine` wrapper around `paddle_ocr_rs::OcrLite`
- `src-tauri/src/engine/ocr/types.rs` — `OcrResult` / `TextBlockInfo` serializable types
- `src-tauri/src/commands/ocr.rs` — `ocr_recognize`, `ocr_get_active_model`, `ocr_set_active_model` commands
- `src-tauri/src/engine/mod.rs` — `pub mod ocr` registered
- `src-tauri/src/commands/mod.rs` — `pub mod ocr` registered
- `src-tauri/src/engine/model_manager.rs` — OCR model download/install check (`download_ppocr`, `is_ppocr_installed_at`)
- `src-tauri/src/commands/model.rs` — `download_specific_model` handles `ppocr-v4/v5/v6`
- `src-tauri/src/errors.rs` — `Ocr(String)` variant added
- `src-tauri/src/config/app_config.rs` — `active_ocr_model` field with default `ppocr-v5`
- `src-tauri/src/lib.rs` — `active_ocr_model` in `AppState`, OCR commands registered in `invoke_handler`

**Frontend (complete, may need minor fixes):**
- `src/app/ocr/page.tsx` — OCR page with drag-drop, image preview, results panel, model selector
- `src/App.tsx` — `"ocr"` page type and routing
- `src/components/app-sidebar.tsx` — OCR nav item with `ScanTextIcon`
- `src/types/index.ts` — `OcrTextBlock` / `OcrResult` types
- `src/lib/app-context.tsx` — OCR i18n strings (zh/en)
- `src/app/settings/model-settings.tsx` — OCR model entries in model list

## Remaining Tasks

### Task 1: Fix Compilation Issues

**Problem:** Previous session encountered compilation errors with `paddle-ocr-rs` related to `ndarray` trait bounds and `ort` version compatibility.

**Analysis:**
- `paddle-ocr-rs` 0.6.1 depends on `ort` ^2.0.0-rc.10 and `ndarray` ^0.16
- Our `Cargo.toml` specifies `ort = "2.0.0-rc.12"` — this is compatible (^2.0.0-rc.10 means >=2.0.0-rc.10, <3.0.0)
- The `download-binaries` feature on `ort` is additive and should not cause issues
- If compilation still fails, possible solutions:
  - Remove explicit `ort` dependency and let `paddle-ocr-rs` pull its own version (lose `download-binaries` auto-DLL)
  - Use `ort` feature `download-binaries` on the pulled-in version via a patch or feature unification
  - Pin `ndarray` version if there's a conflict

**Files to modify (if needed):**
- `src-tauri/Cargo.toml` — adjust ort/ndarray versions

**Verification:** `cargo check` in `src-tauri/` directory

### Task 2: ONNX Runtime DLL Bundling

**Problem:** The `ort` crate's `download-binaries` feature downloads `onnxruntime.dll` (Windows) during build, but Tauri's release build does not automatically bundle it next to the executable.

**Solution:** Add `onnxruntime.dll` to Tauri's `resources` configuration so it's bundled with the app.

**File to modify:** `src-tauri/tauri.conf.json`

```json
"bundle": {
  "resources": {
    "resources/onnxruntime*": "./"
  }
}
```

Also need to create a build script or Tauri resource config that copies the DLL from `ort`'s build output to the resources directory. The `ort` crate downloads the DLL to `$OUT_DIR` during build. We can:

1. Add a `build.rs` script that copies the DLL from `ort`'s output to `src-tauri/resources/`
2. Or use Tauri's `resources` config with a glob pattern

**Alternative approach:** Use `ort` without `download-binaries` feature, and include the ONNX Runtime DLL as a pre-downloaded resource. This avoids the auto-download complexity.

**Files to modify:**
- `src-tauri/tauri.conf.json` — add resources config
- `src-tauri/build.rs` (new or modify) — copy ONNX Runtime DLL to resources

### Task 3: Build & Integration Test

Run `bun run tauri build` to verify the full build pipeline works.

**Verification steps:**
1. `cargo check` passes in `src-tauri/`
2. `bun run tauri build` succeeds
3. Application launches with OCR page accessible
4. OCR model can be downloaded from Model Settings
5. OCR recognition works on a test image

### Task 4: Update Changelog

**File to modify:** `docs/ChangeLog.md`

Add entry for v0.1.3 with the OCR feature.

## Files Changed Summary

| Status | File | Purpose |
|--------|------|---------|
| DONE | `src-tauri/Cargo.toml` | Add paddle-ocr-rs, ort deps |
| DONE | `src-tauri/src/engine/ocr/mod.rs` | OCR engine wrapper |
| DONE | `src-tauri/src/engine/ocr/types.rs` | OCR result types |
| DONE | `src-tauri/src/commands/ocr.rs` | Tauri OCR commands |
| DONE | `src-tauri/src/engine/mod.rs` | Register ocr module |
| DONE | `src-tauri/src/commands/mod.rs` | Register ocr commands |
| DONE | `src-tauri/src/engine/model_manager.rs` | OCR model download/check |
| DONE | `src-tauri/src/commands/model.rs` | download_specific_model for ppocr |
| DONE | `src-tauri/src/errors.rs` | Ocr error variant |
| DONE | `src-tauri/src/config/app_config.rs` | active_ocr_model field |
| DONE | `src-tauri/src/lib.rs` | AppState + command reg |
| DONE | `src/app/ocr/page.tsx` | OCR page UI |
| DONE | `src/App.tsx` | OCR page routing |
| DONE | `src/components/app-sidebar.tsx` | OCR nav item |
| DONE | `src/types/index.ts` | OCR type definitions |
| DONE | `src/lib/app-context.tsx` | OCR i18n strings |
| DONE | `src/app/settings/model-settings.tsx` | OCR model entries |
| TODO | `src-tauri/tauri.conf.json` | Resources config for ONNX Runtime DLL |
| TODO | `src-tauri/build.rs` | Copy ONNX Runtime DLL to resources |
| TODO | `docs/ChangeLog.md` | Add OCR feature entry |

## Assumptions & Decisions

1. **OCR Library**: `paddle-ocr-rs` v0.6.1 (Apache-2.0), wraps `ort` for ONNX Runtime inference
2. **Model Hosting**: gitcode.com (consistent with ASR model hosting pattern)
3. **Model Versions**: V4/V5/V6 each as independent model directories
4. **Default Model**: PP-OCRv5
5. **Image Formats**: PNG, JPG, JPEG, BMP, WEBP, TIFF (via `paddle-ocr-rs` / `image` crate)
6. **CPU-only**: No GPU acceleration (consistent with ASR approach)
7. **OCR Model Switching**: No restart required (models loaded on demand per recognition)
8. **ONNX Runtime**: Bundled with app via `ort`'s `download-binaries` feature + Tauri resources
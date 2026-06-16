//! OCR engine — wraps paddle-ocr-rs for PaddleOCR ONNX inference.
//!
//! Reference implementation: snow-shot (https://github.com/mg-chao/snow-shot)
//!
//! Key improvements adopted from snow-shot:
//! - `detect_angle_rollback` with 0.9 rollback threshold (screenshots are mostly horizontal)
//! - Parallel RGBA→RGB conversion via rayon
//! - Model in-memory loading for faster initialization
//! - Scale factor handling with Lanczos3 resizing
//! - Session lifecycle management (release/reinit)

pub mod types;

use crate::errors::{AppError, AppResult};
use ort::session::builder::SessionBuilder;
use paddle_ocr_rs::ocr_lite::OcrLite;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::io::Write;
use std::path::{Path, PathBuf};
use types::{OcrResult, TextBlockInfo};

/// Build an optimized ONNX Runtime session.
/// References snow-shot's `OcrService::build_session`.
fn build_ocr_session(builder: SessionBuilder) -> Result<SessionBuilder, ort::Error> {
    let num_threads = num_cpus::get_physical();
    builder
        .with_inter_threads(num_threads)?
        .with_intra_threads(num_threads)?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
}

/// Convert RGBA image data to RGB using parallel processing.
/// References snow-shot's `convert_rgba_to_rgb`.
fn convert_rgba_to_rgb(image: &[u8]) -> Vec<u8> {
    let pixel_count = image.len() / 4;
    debug_assert!(image.len() % 4 == 0, "RGBA image data must be divisible by 4");
    let mut rgb_data = Vec::with_capacity(pixel_count * 3);

    unsafe {
        rgb_data.set_len(pixel_count * 3);

        let image_ptr = image.as_ptr() as usize;
        let rgb_ptr = rgb_data.as_mut_ptr() as usize;

        (0..pixel_count).into_par_iter().for_each(|i| {
            let image_base = i * 4;
            let rgb_base = i * 3;
            std::ptr::copy_nonoverlapping(
                (image_ptr as *const u8).add(image_base),
                (rgb_ptr as *mut u8).add(rgb_base),
                3,
            );
        });
    }

    rgb_data
}

/// Prepare a corrected dictionary file for `paddle-ocr-rs`.
///
/// `paddle-ocr-rs`'s `read_keys_from_file` (used by `init_models_with_dict`)
/// loads dict.txt as-is, but CTC decoding expects index 0 to be the blank token `#`
/// and the last index to be ` ` (space). The standard PaddleOCR Python code handles
/// this by prepending "blank" and appending " " at runtime.
///
/// This function generates a `dict_ocr.txt` that mirrors what `get_keys()` produces
/// from model metadata: `#` at the start, ` ` at the end.
///
/// The original `dict.txt` is never modified.
fn prepare_ocr_dict(original_dict: &Path) -> AppResult<PathBuf> {
    let corrected_path = original_dict.with_file_name("dict_ocr.txt");

    // Regenerate if corrected file is missing or older than original
    let needs_regenerate = if corrected_path.exists() {
        let orig_modified = std::fs::metadata(original_dict)
            .and_then(|m| m.modified())
            .ok();
        let corrected_modified = std::fs::metadata(&corrected_path)
            .and_then(|m| m.modified())
            .ok();
        match (orig_modified, corrected_modified) {
            (Some(orig), Some(corr)) => orig > corr,
            _ => true,
        }
    } else {
        true
    };

    if needs_regenerate {
        let content = std::fs::read_to_string(original_dict)
            .map_err(|e| AppError::Ocr(format!("Failed to read dict.txt: {e}")))?;

        let mut file = std::fs::File::create(&corrected_path)
            .map_err(|e| AppError::Ocr(format!("Failed to create dict_ocr.txt: {e}")))?;

        // Prepend blank token `#` (CTC index 0) and append space character
        writeln!(file, "#")?;
        file.write_all(content.as_bytes())?;
        // Ensure trailing newline before the space line
        if !content.ends_with('\n') {
            writeln!(file)?;
        }
        writeln!(file, " ")?;
    }

    Ok(corrected_path)
}

/// OCR engine that wraps paddle-ocr-rs OcrLite.
pub struct OcrEngine {
    inner: OcrLite,
    /// Whether hot-start mode is enabled (session is kept alive).
    #[allow(dead_code)]
    hot_start: bool,
    /// Cached model data for in-memory re-initialization.
    #[allow(dead_code)]
    det_model_data: Option<Vec<u8>>,
    #[allow(dead_code)]
    cls_model_data: Option<Vec<u8>>,
    #[allow(dead_code)]
    rec_model_data: Option<Vec<u8>>,
    /// Model directory (for re-init with dict.txt).
    model_dir: PathBuf,
}

impl OcrEngine {
    /// Create a new OCR engine from model files in the given directory.
    ///
    /// The directory should contain:
    /// - `detection.onnx` — text detection model (DBNet)
    /// - `recognition.onnx` — text recognition model (CRNN/SVTR)
    /// - `cls.onnx` (optional) — text orientation classifier
    #[allow(dead_code)]
    pub fn new(model_dir: &Path) -> AppResult<Self> {
        let (det_path, rec_path, cls_path) = Self::model_paths(model_dir);

        let det_str = det_path.to_string_lossy().to_string();
        let rec_str = rec_path.to_string_lossy().to_string();
        let cls_str = if cls_path.exists() {
            cls_path.to_string_lossy().to_string()
        } else {
            String::new()
        };

        let mut ocr = OcrLite::new();
        ocr.init_models_custom(&det_str, &cls_str, &rec_str, build_ocr_session)
            .map_err(|e| AppError::Ocr(format!("Failed to init OCR models: {e}")))?;

        Ok(Self {
            inner: ocr,
            hot_start: false,
            det_model_data: None,
            cls_model_data: None,
            rec_model_data: None,
            model_dir: model_dir.to_path_buf(),
        })
    }

    /// Create a new OCR engine with models pre-loaded into memory.
    /// This enables faster re-initialization via `release_session` + `init_session`.
    ///
    /// References snow-shot's `OcrService::init_models` with `model_write_to_memory`.
    ///
    /// When `dict.txt` exists in the model directory (required for V5/V6 models),
    /// uses `init_models_with_dict` to load the character dictionary from file.
    /// Otherwise falls back to reading the dictionary from model metadata (V4).
    pub fn new_with_memory(model_dir: &Path, hot_start: bool) -> AppResult<Self> {
        let (det_path, rec_path, cls_path) = Self::model_paths(model_dir);
        let dict_path = model_dir.join("dict.txt");

        let det_data = std::fs::read(&det_path)
            .map_err(|e| AppError::Ocr(format!("Failed to read det model: {e}")))?;
        let rec_data = std::fs::read(&rec_path)
            .map_err(|e| AppError::Ocr(format!("Failed to read rec model: {e}")))?;
        let cls_data = if cls_path.exists() {
            Some(
                std::fs::read(&cls_path)
                    .map_err(|e| AppError::Ocr(format!("Failed to read cls model: {e}")))?,
            )
        } else {
            None
        };

        let mut ocr = OcrLite::new();

        if dict_path.exists() {
            // Models with external dict.txt (model metadata lacks "character" field).
            // Generate a corrected dict_ocr.txt with blank token `#` at index 0
            // and space ` ` at the end, matching what `get_keys()` produces from metadata.
            let corrected_dict = prepare_ocr_dict(&dict_path)?;

            let det_str = det_path.to_string_lossy().to_string();
            let rec_str = rec_path.to_string_lossy().to_string();
            let cls_str = if cls_path.exists() {
                cls_path.to_string_lossy().to_string()
            } else {
                String::new()
            };
            let dict_str = corrected_dict.to_string_lossy().to_string();
            ocr.init_models_with_dict(
                &det_str,
                &cls_str,
                &rec_str,
                &dict_str,
                num_cpus::get_physical(),
            )
            .map_err(|e| AppError::Ocr(format!("Failed to init OCR models with dict: {e}")))?;
        } else if let Some(ref cls_bytes) = cls_data {
            ocr.init_models_from_memory_custom(&det_data, cls_bytes, &rec_data, build_ocr_session)
                .map_err(|e| {
                    AppError::Ocr(format!("Failed to init OCR models from memory: {e}"))
                })?;
        } else {
            ocr.init_models_from_memory_custom(&det_data, &[], &rec_data, build_ocr_session)
                .map_err(|e| {
                    AppError::Ocr(format!("Failed to init OCR models from memory: {e}"))
                })?;
        }

        Ok(Self {
            inner: ocr,
            hot_start,
            det_model_data: Some(det_data),
            cls_model_data: cls_data,
            rec_model_data: Some(rec_data),
            model_dir: model_dir.to_path_buf(),
        })
    }

    /// Resolve model file paths for detection, classification, and recognition.
    fn model_paths(model_dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let det_path = model_dir.join("detection.onnx");
        let rec_path = model_dir.join("recognition.onnx");
        let cls_path = model_dir.join("cls.onnx");
        (det_path, rec_path, cls_path)
    }

    /// Re-initialize the ONNX session from cached model data.
    /// References snow-shot's `OcrService::init_session`.
    #[allow(dead_code)]
    pub fn init_session(&mut self) -> AppResult<()> {
        let dict_path = self.model_dir.join("dict.txt");

        if dict_path.exists() {
            // Generate corrected dict with blank token and space
            let corrected_dict = prepare_ocr_dict(&dict_path)?;

            let (det_path, rec_path, cls_path) = Self::model_paths(&self.model_dir);
            let det_str = det_path.to_string_lossy().to_string();
            let rec_str = rec_path.to_string_lossy().to_string();
            let cls_str = if cls_path.exists() {
                cls_path.to_string_lossy().to_string()
            } else {
                String::new()
            };
            let dict_str = corrected_dict.to_string_lossy().to_string();
            let mut ocr = OcrLite::new();
            ocr.init_models_with_dict(
                &det_str,
                &cls_str,
                &rec_str,
                &dict_str,
                num_cpus::get_physical(),
            )
            .map_err(|e| AppError::Ocr(format!("Failed to re-init OCR session: {e}")))?;
            self.inner = ocr;
        } else {
            // V4: re-init from cached model data
            let det_data = self
                .det_model_data
                .as_ref()
                .ok_or_else(|| AppError::Ocr("Det model data not cached".into()))?;
            let rec_data = self
                .rec_model_data
                .as_ref()
                .ok_or_else(|| AppError::Ocr("Rec model data not cached".into()))?;

            let mut ocr = OcrLite::new();
            if let Some(ref cls_bytes) = self.cls_model_data {
                ocr.init_models_from_memory_custom(det_data, cls_bytes, rec_data, build_ocr_session)
            } else {
                ocr.init_models_from_memory_custom(det_data, &[], rec_data, build_ocr_session)
            }
            .map_err(|e| AppError::Ocr(format!("Failed to re-init OCR session: {e}")))?;

            self.inner = ocr;
        }
        Ok(())
    }

    /// Release the current ONNX session (free GPU/memory resources).
    /// References snow-shot's `OcrService::release_session`.
    pub fn release_session(&mut self) {
        // Drop the current OcrLite to release ONNX sessions
        self.inner = OcrLite::new();
    }

    /// Run OCR on an image file path.
    pub fn recognize_from_path(&mut self, image_path: &Path) -> AppResult<OcrResult> {
        let img = image::open(image_path)
            .map_err(|e| AppError::Ocr(format!("Failed to open image: {e}")))?;
        self.recognize_from_image(&img, 1.0)
    }

    /// Run OCR on an in-memory image (DynamicImage) with optional scale factor.
    ///
    /// Preprocessing strategy:
    /// - Small images (long side < 960px): resize up to 1.5x for better OCR accuracy
    /// - Large images (long side >= 960px): resize down to max 1920px for faster processing
    /// - PaddleOCR internally limits detection input to `max_side_len`, so pre-resizing
    ///   avoids unnecessary computation on oversized images.
    /// References snow-shot's `ocr_detect_core` scale factor handling.
    pub fn recognize_from_image(
        &mut self,
        img: &image::DynamicImage,
        scale_factor: f32,
    ) -> AppResult<OcrResult> {
        let start = std::time::Instant::now();

        let long_side = img.width().max(img.height());

        // PaddleOCR optimal detection size is around 960px.
        // For small images, scale up for better accuracy.
        // For large images, scale down to avoid excessive computation.
        const OCR_TARGET_SIZE: u32 = 960;
        const OCR_MAX_SIZE: u32 = 1920;

        // Only clone the image when scaling is actually needed.
        // Using a conditional owned image avoids unnecessary multi-MB allocations
        // for images in the 960-1920px sweet spot.
        let need_resize = long_side < OCR_TARGET_SIZE || long_side > OCR_MAX_SIZE;
        let owned_image;
        let image: &image::DynamicImage = if need_resize {
            let mut tmp = img.clone();
            if long_side < OCR_TARGET_SIZE {
                // Small image: scale up to 1.5x for better OCR accuracy
                let target_scale_factor = 1.5;
                if scale_factor < target_scale_factor && scale_factor > 0.0 {
                    let resize_factor = target_scale_factor / scale_factor;
                    tmp = tmp.resize(
                        (tmp.width() as f32 * resize_factor) as u32,
                        (tmp.height() as f32 * resize_factor) as u32,
                        image::imageops::FilterType::Lanczos3,
                    );
                }
            } else {
                // Large image: scale down to OCR_MAX_SIZE for faster processing
                let scale = OCR_MAX_SIZE as f32 / long_side as f32;
                tmp = tmp.resize(
                    (tmp.width() as f32 * scale) as u32,
                    (tmp.height() as f32 * scale) as u32,
                    image::imageops::FilterType::Lanczos3,
                );
            }
            owned_image = tmp;
            &owned_image
        } else {
            // Images between 960-1920px: use as-is (already in a good range)
            img
        };

        let max_size = image.height().max(image.width());

        // Convert to RGB — using parallel conversion for RGBA, standard for RGB
        let image_buffer = match image {
            image::DynamicImage::ImageRgb8(rgb) => rgb.clone(),
            image::DynamicImage::ImageRgba8(rgba) => {
                let rgb_data = convert_rgba_to_rgb(rgba.as_raw());
                image::RgbImage::from_raw(rgba.width(), rgba.height(), rgb_data)
                    .ok_or_else(|| AppError::Ocr("Failed to convert RGBA to RGB".into()))?
            }
            _ => return Err(AppError::Ocr("Unsupported image format".into())),
        };

        // Use detect_angle_rollback with 0.9 rollback threshold.
        // Screenshots are mostly horizontal, so we reduce false angle corrections.
        // References snow-shot's `ocr_detect_core`.
        let result = self
            .inner
            .detect_angle_rollback(
                &image_buffer,
                50,       // padding
                max_size, // max side len
                0.5,      // box score threshold
                0.3,      // box threshold
                1.6,      // unclip ratio
                true,     // do angle classification
                false,    // most angle (only 0/180)
                0.9,      // rollback threshold (snow-shot: 0.9)
            )
            .map_err(|e| AppError::Ocr(format!("OCR recognition failed: {e}")))?;

        let text_blocks: Vec<TextBlockInfo> = result
            .text_blocks
            .iter()
            .map(|block| TextBlockInfo {
                text: block.text.clone(),
                confidence: block.text_score,
                box_points: block
                    .box_points
                    .iter()
                    .map(|p| [p.x as f32, p.y as f32])
                    .collect(),
            })
            .collect();

        Ok(OcrResult {
            text_blocks,
            total_time_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Run OCR on raw image bytes (PNG/JPEG/etc.).
    pub fn recognize_from_bytes(&mut self, data: &[u8]) -> AppResult<OcrResult> {
        let img = image::load_from_memory(data)
            .map_err(|e| AppError::Ocr(format!("Failed to decode image: {e}")))?;
        self.recognize_from_image(&img, 1.0)
    }

    /// Run OCR on raw RGBA pixel data (bypasses PNG encode/decode).
    /// Optimized for screenshot OCR where raw pixel data is already available.
    /// References snow-shot's SharedBuffer zero-copy approach.
    pub fn recognize_from_raw_rgba(
        &mut self,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> AppResult<OcrResult> {
        let start = std::time::Instant::now();

        let rgb_data = convert_rgba_to_rgb(rgba_data);
        let image_buffer = image::RgbImage::from_raw(width, height, rgb_data)
            .ok_or_else(|| AppError::Ocr("Failed to create RGB image from raw data".into()))?;

        let max_size = height.max(width);

        let result = self
            .inner
            .detect_angle_rollback(
                &image_buffer,
                50,
                max_size,
                0.5,
                0.3,
                1.6,
                true,
                false,
                0.9,
            )
            .map_err(|e| AppError::Ocr(format!("OCR recognition failed: {e}")))?;

        let text_blocks: Vec<TextBlockInfo> = result
            .text_blocks
            .iter()
            .map(|block| TextBlockInfo {
                text: block.text.clone(),
                confidence: block.text_score,
                box_points: block
                    .box_points
                    .iter()
                    .map(|p| [p.x as f32, p.y as f32])
                    .collect(),
            })
            .collect();

        Ok(OcrResult {
            text_blocks,
            total_time_ms: start.elapsed().as_millis() as u64,
        })
    }
}

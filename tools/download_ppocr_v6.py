#!/usr/bin/env python3
"""
PP-OCRv6 ONNX Model Downloader

Downloads pre-converted ONNX models from ModelScope (魔搭社区).
PP-OCRv6 models are officially provided by PaddlePaddle with pre-converted ONNX format.
No paddle2onnx conversion needed — just download and rename.

Sources:
  - Detection:  https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv6_small_det_onnx
  - Recognition: https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv6_small_rec_onnx
  - Classification: Uses PP-OCRv2 cls model (same as V4/V5)

Usage:
    python download_ppocr_v6.py

Output:
    ppocr-v6/
      detection.onnx    (PP-OCRv6_small_det)
      recognition.onnx  (PP-OCRv6_small_rec)
      cls.onnx          (ch_ppocr_mobile_v2.0_cls — same as V4/V5)
"""

import os
import sys
import urllib.request
import shutil

# ============================================================
# Configuration
# ============================================================

OUTPUT_DIR = "ppocr-v6"
os.makedirs(OUTPUT_DIR, exist_ok=True)

# PP-OCRv6 ONNX models from ModelScope (official PaddlePaddle)
MODEL_DET_URL = (
    "https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv6_small_det_onnx"
    "/resolve/master/inference.onnx"
)
MODEL_REC_URL = (
    "https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv6_small_rec_onnx"
    "/resolve/master/inference.onnx"
)

# Classification model — PP-OCRv6 doesn't have its own cls model.
# Use the same PP-OCRv2 classification model as V4/V5.
# Downloaded from legacy PaddleOCR CDN.
MODEL_CLS_URL = (
    "https://paddleocr.bj.bcebos.com/dygraph_v2.0/"
    "ch_ppocr_mobile_v2.0_cls_infer.tar"
)
# Fallback: use snow-shot's cls model from rapid_ocr.zip if available
SNOWSHOT_RAPID_ZIP = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "../docs/ocr-models/rapid_ocr.zip"
)


def download_file(url, dest_path):
    """Download a file with progress reporting."""
    print(f"  Downloading: {url}")
    try:
        urllib.request.urlretrieve(url, dest_path)
        size_mb = os.path.getsize(dest_path) / (1024 * 1024)
        print(f"  Downloaded: {size_mb:.1f} MB -> {dest_path}")
        return True
    except Exception as e:
        print(f"  FAILED: {e}")
        return False


def extract_cls_from_snowshot():
    """Extract cls.onnx from snow-shot's rapid_ocr.zip."""
    if not os.path.exists(SNOWSHOT_RAPID_ZIP):
        return False

    print(f"  Extracting cls model from snow-shot's rapid_ocr.zip...")
    try:
        import zipfile
        with zipfile.ZipFile(SNOWSHOT_RAPID_ZIP, 'r') as zf:
            cls_name = "ch_ppocr_mobile_v2.0_cls_infer.onnx"
            if cls_name in zf.namelist():
                zf.extract(cls_name, OUTPUT_DIR)
                # Rename
                src = os.path.join(OUTPUT_DIR, cls_name)
                dst = os.path.join(OUTPUT_DIR, "cls.onnx")
                if os.path.exists(dst):
                    os.remove(dst)
                os.rename(src, dst)
                print(f"  [OK] cls.onnx extracted from rapid_ocr.zip")
                return True
            else:
                print(f"  {cls_name} not found in rapid_ocr.zip")
                return False
    except Exception as e:
        print(f"  Failed to extract cls from rapid_ocr.zip: {e}")
        return False


def download_cls_model():
    """Download and extract the PP-OCRv2 classification model."""
    import tarfile
    import tempfile

    print(f"\n  Downloading classification model...")

    # Method 1: Try downloading the .tar and extracting
    with tempfile.TemporaryDirectory() as tmpdir:
        tar_path = os.path.join(tmpdir, "cls.tar")
        if download_file(MODEL_CLS_URL, tar_path):
            try:
                with tarfile.open(tar_path, "r") as tar:
                    tar.extractall(path=tmpdir)

                # Find the ONNX file
                for root, dirs, files in os.walk(tmpdir):
                    for f in files:
                        if f.endswith(".onnx"):
                            src = os.path.join(root, f)
                            dst = os.path.join(OUTPUT_DIR, "cls.onnx")
                            if os.path.exists(dst):
                                os.remove(dst)
                            shutil.copy2(src, dst)
                            size_mb = os.path.getsize(dst) / (1024 * 1024)
                            print(f"  [OK] cls.onnx created ({size_mb:.1f} MB)")
                            return True
                print(f"  WARNING: No ONNX file found in tar")
            except Exception as e:
                print(f"  Failed to extract tar: {e}")

    return False


def main():
    print("=" * 60)
    print("PP-OCRv6 ONNX Model Downloader")
    print("=" * 60)
    print()
    print("Source: ModelScope (魔搭社区) — official PaddlePaddle")
    print("Detection:  PP-OCRv6_small_det_onnx")
    print("Recognition: PP-OCRv6_small_rec_onnx")
    print("Classification: ch_ppocr_mobile_v2.0_cls (from V2, shared with V4/V5)")
    print()

    # Download detection model
    print("--- Detection Model ---")
    det_dst = os.path.join(OUTPUT_DIR, "detection.onnx")
    if download_file(MODEL_DET_URL, det_dst):
        print(f"  [OK] detection.onnx ({os.path.getsize(det_dst) / (1024*1024):.1f} MB)")
    else:
        print(f"  [FAIL] detection.onnx download failed")
        sys.exit(1)

    # Download recognition model
    print(f"\n--- Recognition Model ---")
    rec_dst = os.path.join(OUTPUT_DIR, "recognition.onnx")
    if download_file(MODEL_REC_URL, rec_dst):
        print(f"  [OK] recognition.onnx ({os.path.getsize(rec_dst) / (1024*1024):.1f} MB)")
    else:
        print(f"  [FAIL] recognition.onnx download failed")
        sys.exit(1)

    # Get classification model
    print(f"\n--- Classification Model ---")
    cls_dst = os.path.join(OUTPUT_DIR, "cls.onnx")
    if os.path.exists(cls_dst):
        print(f"  cls.onnx already exists, skipping")
    elif extract_cls_from_snowshot():
        pass  # Already handled in function
    elif download_cls_model():
        pass  # Already handled in function
    else:
        print(f"  [WARN] cls model not available. OCR will work without text orientation correction.")

    # Summary
    print(f"\n{'=' * 60}")
    print("  Download complete!")
    print(f"  Output directory: {os.path.abspath(OUTPUT_DIR)}/")
    for f in sorted(os.listdir(OUTPUT_DIR)):
        path = os.path.join(OUTPUT_DIR, f)
        size_mb = os.path.getsize(path) / (1024 * 1024)
        print(f"    {f}: {size_mb:.1f} MB")
    print(f"\n  Next: Compress ppocr-v6/ into ppocr-v6.zip and upload to GitCode.")
    print(f"  Upload path: https://gitcode.com/tabortao/VelociText/releases/download/ocr/ppocr-v6.zip")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
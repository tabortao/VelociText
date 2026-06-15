/// Quick helper to extract snow-shot's rapid_ocr.zip (uses xz compression)
/// and create proper ppocr-v4.zip and ppocr-v5.zip for VelociText.
///
/// Usage: cargo run --example rebuild_ocr_zips
fn main() {
    let base = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/ocr-models"));
    let rapid_zip = base.join("rapid_ocr.zip");
    let temp = base.join("temp_extract");

    // Clean and create temp dir
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    // Read rapid_ocr.zip
    let file = std::fs::File::open(&rapid_zip).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    println!("Found {} entries in rapid_ocr.zip", archive.len());

    // Extract all files to temp
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        if entry.is_dir() {
            continue;
        }
        let out_path = temp.join(&name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut out_file = std::fs::File::create(&out_path).unwrap();
        std::io::copy(&mut entry, &mut out_file).unwrap();
        println!("  Extracted: {} ({} bytes)", name, out_path.metadata().unwrap().len());
    }

    // File paths in extracted directory
    let rapid_dir = temp.join("rapid_ocr");
    let det_v4 = rapid_dir.join("ch_PP-OCRv4_det_infer.onnx");
    let rec_v4 = rapid_dir.join("ch_PP-OCRv4_rec_infer.onnx");
    let rec_v5 = rapid_dir.join("ch_PP-OCRv5_rec_mobile_infer.onnx");
    let cls = rapid_dir.join("ch_ppocr_mobile_v2.0_cls_infer.onnx");

    // Build ppocr-v4.zip: detection.onnx, recognition.onnx, cls.onnx
    {
        let v4_path = base.join("ppocr-v4.zip");
        let out = std::fs::File::create(&v4_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // detection.onnx (V4 det)
        zip_writer.start_file("detection.onnx", options).unwrap();
        std::io::copy(&mut std::fs::File::open(&det_v4).unwrap(), &mut zip_writer).unwrap();
        println!("  Added detection.onnx (from ch_PP-OCRv4_det_infer.onnx)");

        // recognition.onnx (V4 rec)
        zip_writer.start_file("recognition.onnx", options).unwrap();
        std::io::copy(&mut std::fs::File::open(&rec_v4).unwrap(), &mut zip_writer).unwrap();
        println!("  Added recognition.onnx (from ch_PP-OCRv4_rec_infer.onnx)");

        // cls.onnx
        zip_writer.start_file("cls.onnx", options).unwrap();
        std::io::copy(&mut std::fs::File::open(&cls).unwrap(), &mut zip_writer).unwrap();
        println!("  Added cls.onnx");

        zip_writer.finish().unwrap();
        let size = v4_path.metadata().unwrap().len();
        println!("Created ppocr-v4.zip ({:.1} MB)", size as f64 / 1_048_576.0);
    }

    // Build ppocr-v5.zip: detection.onnx, recognition.onnx, cls.onnx
    {
        let v5_path = base.join("ppocr-v5.zip");
        let out = std::fs::File::create(&v5_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // detection.onnx (same V4 det model — V5 uses the same detection)
        zip_writer.start_file("detection.onnx", options).unwrap();
        std::io::copy(&mut std::fs::File::open(&det_v4).unwrap(), &mut zip_writer).unwrap();
        println!("  Added detection.onnx (from ch_PP-OCRv4_det_infer.onnx)");

        // recognition.onnx (V5 rec)
        zip_writer.start_file("recognition.onnx", options).unwrap();
        std::io::copy(&mut std::fs::File::open(&rec_v5).unwrap(), &mut zip_writer).unwrap();
        println!("  Added recognition.onnx (from ch_PP-OCRv5_rec_mobile_infer.onnx)");

        // cls.onnx
        zip_writer.start_file("cls.onnx", options).unwrap();
        std::io::copy(&mut std::fs::File::open(&cls).unwrap(), &mut zip_writer).unwrap();
        println!("  Added cls.onnx");

        zip_writer.finish().unwrap();
        let size = v5_path.metadata().unwrap().len();
        println!("Created ppocr-v5.zip ({:.1} MB)", size as f64 / 1_048_576.0);
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp);
    println!("\nDone! ppocr-v4.zip and ppocr-v5.zip are ready.");
    println!("Upload these to: https://gitcode.com/tabortao/VelociText/releases/download/ocr/");
}
//! スパイク用: 画像のディレクトリを OCR して `<ファイル名>.txt` と所要時間を出す。
//! 使い方: cargo run --release --features onnx --example ocr_dump -- ppocr ORT_LIB DET REC IN_DIR OUT_DIR [threads] [ALT_REC]
//!        cargo run --release --example ocr_dump -- tesseract EXE TESSDATA_DIR IN_DIR OUT_DIR
use factory_core::ocr::{tesseract::TesseractOcr, Ocr, OcrImage};
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (ocr, in_dir, out_dir): (Box<dyn Ocr>, &str, &str) = match a[1].as_str() {
        #[cfg(feature = "onnx")]
        "ppocr" => {
            use factory_core::ocr::ppocr::{PpOcr, PpOcrConfig};
            let threads = a.get(7).and_then(|s| s.parse().ok()).unwrap_or(4);
            let o = PpOcr::new(&PpOcrConfig { onnxruntime_lib: a[2].clone().into(), det_model: a[3].clone().into(), rec_model: a[4].clone().into(), dict: None, alt_rec_model: a.get(8).map(|s| s.into()), threads }).unwrap();
            (Box::new(o), &a[5], &a[6])
        }
        "tesseract" => {
            let mut t = TesseractOcr::new(&a[2]);
            if a[3] != "-" { t.tessdata_dir = Some(a[3].clone().into()); }
            (Box::new(t), &a[4], &a[5])
        }
        _ => panic!("engine"),
    };
    std::fs::create_dir_all(out_dir).unwrap();
    let mut files: Vec<_> = std::fs::read_dir(in_dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.sort();
    let mut times = vec![];
    for p in files {
        let Ok(bytes) = std::fs::read(&p) else { continue };
        let Ok(img) = OcrImage::decode(&bytes) else { continue };
        let t = Instant::now();
        let page = ocr.recognize(&img).unwrap();
        let dt = t.elapsed().as_secs_f64();
        times.push(dt);
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        std::fs::write(format!("{out_dir}/{name}.txt"), page.to_text()).unwrap();
        std::fs::write(format!("{out_dir}/{name}.alt.txt"), page.to_alt_text()).unwrap();
        println!("{name} {dt:.2}s lines={} conf={:.2}", page.lines.len(), page.mean_confidence());
    }
    times.sort_by(|a, b| a.total_cmp(b));
    if let Ok(st) = std::fs::read_to_string("/proc/self/status") {
        for l in st.lines().filter(|l| l.starts_with("VmHWM")) { println!("{l}"); }
    }
    println!("n={} median={:.2}s max={:.2}s", times.len(), times[times.len() / 2], times[times.len() - 1]);
}

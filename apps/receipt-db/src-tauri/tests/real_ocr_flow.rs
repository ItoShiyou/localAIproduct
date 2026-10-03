//! 実OCR(ONNX Runtime 上の PP-OCR)で、画面と同じコマンド経路(取り込み → 読み取り → 一覧)を通す結合テスト。
//! `--features onnx` と、モデルの場所(FACTORY_OCR_ORT_LIB / FACTORY_OCR_DET / FACTORY_OCR_REC / 任意で FACTORY_OCR_ALT_REC)が
//! 揃っているときだけ動く。揃っていなければ何もせずに通る(CI やモデルの無い環境向け)。
//! 実行例: FACTORY_OCR_...=... cargo test --features onnx --test real_ocr_flow -- --nocapture

use base64::Engine;
use receipt_db::commands::{AppState, ImportFileDto, SearchDto};
use std::path::Path;

#[test]
fn 実ocrで_テストセットを取り込み_読み取り_正解と比べる() {
    let (ocr, kind) = receipt_db::build_ocr();
    if kind != "real" {
        eprintln!("実OCRのモデルが未設定のため省略");
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset");
    let labels: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("labels.json")).unwrap()).unwrap();
    let data = std::env::temp_dir().join(format!("rdb-real-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    let s = AppState::new(data.clone(), ocr, kind).unwrap();

    let files: Vec<ImportFileDto> = labels
        .as_object()
        .unwrap()
        .keys()
        .map(|name| {
            let bytes = std::fs::read(dir.join("files").join(name)).unwrap();
            let mime = if name.ends_with(".pdf") { "application/pdf" } else { "image/png" };
            ImportFileDto { name: name.clone(), data_url: format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }
        })
        .collect();
    let n = files.len();
    let res = s.import_files(files);
    assert!(res.iter().all(|r| r.outcome == "imported"), "{res:?}");

    let t0 = std::time::Instant::now();
    s.run_jobs().unwrap();
    let secs = t0.elapsed().as_secs_f64();
    let p = s.progress().unwrap();
    eprintln!("読み取り {n} 件: {secs:.1} 秒(1件あたり {:.2} 秒)、失敗 {}", secs / n as f64, p.failed);
    assert_eq!(p.failed, 0, "{:?}", p.errors);

    let rows = s.list(SearchDto::default()).unwrap();
    assert!(rows.iter().all(|r| r.status == "draft"), "自動では確定しない");
    let (mut date, mut total, mut vendor) = (0, 0, 0);
    for r in &rows {
        let l = &labels[&r.original_name];
        date += (r.date.as_deref() == l["date"].as_str()) as u32;
        total += (r.total == l["total"].as_i64()) as u32;
        vendor += (r.vendor.as_deref() == l["vendor"].as_str()) as u32;
    }
    eprintln!("正解との一致({} 行): 日付 {date}、金額 {total}、支払先 {vendor}", rows.len());
    std::fs::remove_dir_all(&data).ok();
}

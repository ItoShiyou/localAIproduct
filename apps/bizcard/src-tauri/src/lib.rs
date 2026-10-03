//! 名刺管理: OCR出力(core::ocr)の後ろ側のロジック(ルール抽出・重複検知・保存と検索・CSV・vCard)。
pub mod commands;
pub mod csv_export;
pub mod dedup;
pub mod eval;
pub mod extract;
pub mod ocr_input;
pub mod store;
pub mod vcard;
#[cfg(feature = "tauri")]
pub mod tauri_glue;

/// `core::ocr` の実装を選ぶ。
/// `FACTORY_OCR_ORT_LIB` / `FACTORY_OCR_DET` / `FACTORY_OCR_REC`(任意で `FACTORY_OCR_ALT_REC`、
/// `FACTORY_OCR_THREADS`)が揃っていれば実OCR(ONNX Runtime 上の PP-OCR)、無ければ `FakeOcr`
/// (常に空のページを返す。手打ちでの確認画面の動作確認用)で起動する。
#[cfg(feature = "tauri")]
fn build_ocr() -> Box<dyn factory_core::ocr::Ocr + Send + Sync> {
    #[cfg(feature = "onnx")]
    {
        if let (Ok(lib), Ok(det), Ok(rec)) = (
            std::env::var("FACTORY_OCR_ORT_LIB"),
            std::env::var("FACTORY_OCR_DET"),
            std::env::var("FACTORY_OCR_REC"),
        ) {
            use factory_core::ocr::ppocr::{PpOcr, PpOcrConfig};
            let alt = std::env::var("FACTORY_OCR_ALT_REC").ok().map(std::path::PathBuf::from);
            let threads: usize = std::env::var("FACTORY_OCR_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(4);
            match PpOcr::new(&PpOcrConfig {
                onnxruntime_lib: lib.into(),
                det_model: det.into(),
                rec_model: rec.into(),
                dict: None,
                alt_rec_model: alt,
                threads,
            }) {
                Ok(o) => return Box::new(o),
                Err(e) => eprintln!("実OCRを初期化できません({e})。架空のOCR(FakeOcr)で起動します"),
            }
        }
    }
    Box::new(factory_core::ocr::FakeOcr { page: factory_core::ocr::OcrPage { width: 0.0, height: 0.0, lines: vec![] } })
}

#[cfg(feature = "tauri")]
pub fn run() {
    use commands::AppState;
    use std::sync::Mutex;
    use tauri::Manager;

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            std::fs::create_dir_all(&data_dir)?;
            let store = store::Store::open(&data_dir.join("bizcard.sqlite3"))?;
            app.manage(AppState { store: Mutex::new(store), ocr: build_ocr(), image_dir: data_dir.join("images") });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_glue::import_card,
            tauri_glue::confirm_card,
            tauri_glue::discard_draft,
            tauri_glue::search,
        ])
        .run(tauri::generate_context!())
        .expect("tauri アプリの起動に失敗しました");
}

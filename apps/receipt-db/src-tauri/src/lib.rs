//! 領収書DB のアプリ固有ロジックと、Tauri 2 の殻(feature `tauri`)。
//! 共通部品(CSV・ライセンスなど)は `factory-core`(`../core/engine`)を使う。

pub mod app_settings;
pub mod commands;
pub mod extract;
pub mod import;
pub mod journal;
pub mod receipt;
pub mod rules;
pub mod store;
pub mod vendor;
#[cfg(feature = "tauri")]
pub mod tauri_glue;

/// `core::ocr` の実装を選ぶ。
/// `FACTORY_OCR_ORT_LIB` / `FACTORY_OCR_DET` / `FACTORY_OCR_REC`(任意で `FACTORY_OCR_ALT_REC`、`FACTORY_OCR_THREADS`)が
/// 揃っていれば実OCR(ONNX Runtime 上の PP-OCR)、無ければ `FakeOcr`(常に空のページ。テキスト層のあるPDFは読める。
/// 画像は確認画面で手入力)で起動する。
pub fn build_ocr() -> (Box<dyn factory_core::ocr::Ocr + Send + Sync>, &'static str) {
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
            match PpOcr::new(&PpOcrConfig { onnxruntime_lib: lib.into(), det_model: det.into(), rec_model: rec.into(), dict: None, alt_rec_model: alt, threads }) {
                Ok(o) => return (Box::new(o), "real"),
                Err(e) => eprintln!("実OCRを初期化できません({e})。架空のOCR(FakeOcr)で起動します"),
            }
        }
    }
    (Box::new(factory_core::ocr::FakeOcr { page: factory_core::ocr::OcrPage { width: 0.0, height: 0.0, lines: vec![] } }), "fake")
}

#[cfg(feature = "tauri")]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let (ocr, kind) = build_ocr();
            let state = commands::AppState::new(data_dir, ocr, kind)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_glue::import_files,
            tauri_glue::run_jobs,
            tauri_glue::cancel_jobs,
            tauri_glue::retry_failed,
            tauri_glue::progress,
            tauri_glue::list,
            tauri_glue::get_receipt,
            tauri_glue::original,
            tauri_glue::edit,
            tauri_glue::undo,
            tauri_glue::confirm,
            tauri_glue::unconfirm,
            tauri_glue::delete_receipt,
            tauri_glue::export_csv,
            tauri_glue::export_sqlite,
            tauri_glue::settings,
            tauri_glue::set_update_check,
            tauri_glue::delete_all,
        ])
        .run(tauri::generate_context!())
        .expect("tauri アプリの起動に失敗しました");
}

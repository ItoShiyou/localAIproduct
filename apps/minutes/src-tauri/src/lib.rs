//! 議事録: 録音の読み込み・ノイズ除去・文字起こし・確認・書き出しのロジックと、Tauri 2 の殻(feature `tauri`)。

pub mod asr;
pub mod audio;
pub mod commands;
pub mod export;
pub mod pipeline;
pub mod store;
#[cfg(feature = "tauri")]
pub mod tauri_glue;

/// 文字起こしエンジンを選ぶ。`MINUTES_WHISPER_MODEL`(whisper.cpp の ggml 形式のモデル)があれば whisper、
/// 無ければ `MissingAsr`(画面は動くが、文字起こしは「モデルが未設定」で失敗する)。`MINUTES_THREADS` でスレッド数(既定 6)。
pub fn build_asr() -> Box<dyn asr::Asr> {
    #[cfg(feature = "whisper")]
    {
        if let Ok(model) = std::env::var("MINUTES_WHISPER_MODEL") {
            let threads = std::env::var("MINUTES_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
            match asr::WhisperAsr::new(std::path::Path::new(&model), threads) {
                Ok(a) => return Box::new(a),
                Err(e) => return Box::new(asr::MissingAsr { reason: e }),
            }
        }
    }
    Box::new(asr::MissingAsr { reason: "文字起こしのモデルが設定されていません".into() })
}

#[cfg(feature = "tauri")]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            app.manage(commands::AppState::new(data_dir, build_asr())?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_glue::pick_and_import,
            tauri_glue::import_paths,
            tauri_glue::run_jobs,
            tauri_glue::cancel_jobs,
            tauri_glue::retry,
            tauri_glue::progress,
            tauri_glue::meetings,
            tauri_glue::detail,
            tauri_glue::audio_path,
            tauri_glue::update_meta,
            tauri_glue::edit_text,
            tauri_glue::set_speaker,
            tauri_glue::merge_next,
            tauri_glue::split,
            tauri_glue::revert_segment,
            tauri_glue::undo,
            tauri_glue::reapply_glossary,
            tauri_glue::confirm,
            tauri_glue::unconfirm,
            tauri_glue::delete_meeting,
            tauri_glue::export,
            tauri_glue::export_denoised,
            tauri_glue::search,
            tauri_glue::glossary,
            tauri_glue::add_glossary,
            tauri_glue::delete_glossary,
            tauri_glue::settings,
            tauri_glue::set_flag,
            tauri_glue::delete_all,
        ])
        .run(tauri::generate_context!())
        .expect("tauri アプリの起動に失敗しました");
}

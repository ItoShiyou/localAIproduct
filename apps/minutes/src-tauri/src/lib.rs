//! 議事録: 録音の読み込み・ノイズ除去・文字起こし・確認・書き出しのロジックと、Tauri 2 の殻(feature `tauri`)。

pub mod asr;
pub mod audio;
pub mod commands;
pub mod diarize;
pub mod export;
pub mod pipeline;
pub mod plan;
pub mod recorder;
pub mod store;
#[cfg(feature = "tauri")]
pub mod tauri_glue;

/// モデルの場所から文字起こしエンジンを作る(whisper.cpp)。`MINUTES_THREADS` でスレッド数(既定 6)。
pub fn whisper_loader() -> commands::AsrLoader {
    Box::new(|path: &std::path::Path| -> Result<Box<dyn asr::Asr>, String> {
        #[cfg(feature = "whisper")]
        {
            let threads = std::env::var("MINUTES_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
            Ok(Box::new(asr::WhisperAsr::new(path, threads)?))
        }
        #[cfg(not(feature = "whisper"))]
        {
            let _ = path;
            Err("このビルドには文字起こしのエンジンが含まれていません".into())
        }
    })
}

/// 話者の判別(WeSpeaker、ONNX Runtime)。場所は 開発用の環境変数(MINUTES_ORT_LIB / MINUTES_SPK_MODEL)→ アプリに同梱 の順。
pub fn embedder_loader(resources: Option<std::path::PathBuf>) -> commands::EmbedderLoader {
    Box::new(move || -> Result<Box<dyn diarize::Embedder>, String> {
        #[cfg(feature = "diarize")]
        {
            let pick = |env: &str, rel: &[&str]| -> Option<std::path::PathBuf> {
                std::env::var(env).ok().map(std::path::PathBuf::from).filter(|p| p.exists()).or_else(|| {
                    resources.as_ref().map(|r| rel.iter().fold(r.clone(), |p, s| p.join(s))).filter(|p| p.exists())
                })
            };
            let lib_name = if cfg!(windows) { "onnxruntime.dll" } else { "libonnxruntime.dylib" };
            let lib = pick("MINUTES_ORT_LIB", &["onnxruntime", lib_name]).ok_or("話者の判別に使う onnxruntime が見つかりません")?;
            let model = pick("MINUTES_SPK_MODEL", &["models", "voxceleb_resnet34_LM.onnx"]).ok_or("話者の判別のモデルが見つかりません")?;
            let threads = std::env::var("MINUTES_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(4);
            Ok(Box::new(diarize::OnnxEmbedder::new(&lib, &model, threads)?))
        }
        #[cfg(not(feature = "diarize"))]
        {
            let _ = &resources;
            Err("このビルドには話者の判別が含まれていません".into())
        }
    })
}

/// 開発用: 環境変数 `MINUTES_WHISPER_MODEL` でモデルの場所を指定できる(指定が無ければ、設定画面から取得したもの)。
pub fn env_model() -> Option<std::path::PathBuf> {
    std::env::var("MINUTES_WHISPER_MODEL").ok().map(std::path::PathBuf::from)
}

#[cfg(feature = "tauri")]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            // モデルの場所: 開発用の環境変数 → アプリに同梱(resources/models/)の順。どちらも無ければ設定画面から取得
            let mut fixed = Vec::new();
            if let Some(p) = env_model() {
                fixed.push((p, "env"));
            }
            let res = app.path().resource_dir().ok().map(|r| r.join("resources"));
            if let Some(r) = &res {
                fixed.push((r.join("models").join(commands::WHISPER_MODEL.file_name), "bundled"));
            }
            let state = commands::AppState::new(data_dir, None, whisper_loader(), fixed)?;
            state.set_embedder_loader(embedder_loader(res.clone()));
            // 録音中の仮の文字に使う小さなモデル(開発用の環境変数 → 同梱)
            let mut live = Vec::new();
            if let Ok(p) = std::env::var("MINUTES_LIVE_MODEL") {
                live.push(std::path::PathBuf::from(p));
            }
            if let Some(r) = &res {
                live.push(r.join("models").join("ggml-small-q5_1.bin"));
            }
            state.set_live_models(live);
            // 無料版の使った量の記録: データフォルダ・別の場所・OS の資格情報ストア(どれか消されても戻る)
            let id = app.config().identifier.clone();
            let mut slots: Vec<Box<dyn plan::Slot>> = vec![Box::new(plan::FileSlot(app.path().app_data_dir().unwrap_or_default().join("usage.dat")))];
            if let Ok(home) = app.path().home_dir() {
                let other = if cfg!(target_os = "macos") {
                    home.join("Library").join("Preferences").join(format!("{id}.usage"))
                } else {
                    home.join("AppData").join("Local").join(format!("{id}-usage")).join("usage.dat")
                };
                slots.push(Box::new(plan::FileSlot(other)));
            }
            slots.push(Box::new(plan::KeychainSlot { service: id }));
            state.set_ledger(plan::Ledger::new(slots));
            *state.notices_path.lock().unwrap() = res.map(|r| r.join("THIRD_PARTY_NOTICES.txt"));
            app.manage(state);
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
            tauri_glue::model_status,
            tauri_glue::download_model,
            tauri_glue::cancel_model_download,
            tauri_glue::delete_model,
            tauri_glue::third_party_notices,
            tauri_glue::all_tags,
            tauri_glue::set_tags,
            tauri_glue::update_notes,
            tauri_glue::rediarize,
            tauri_glue::rename_speaker,
            tauri_glue::find_in,
            tauri_glue::replace_in,
            tauri_glue::reprocess,
            tauri_glue::print_page,
            tauri_glue::export_glossary,
            tauri_glue::import_glossary,
            tauri_glue::record_start,
            tauri_glue::record_push,
            tauri_glue::record_status,
            tauri_glue::record_stop,
            tauri_glue::record_discard,
            tauri_glue::plan,
            tauri_glue::waveform,
        ])
        .run(tauri::generate_context!())
        .expect("tauri アプリの起動に失敗しました");
}

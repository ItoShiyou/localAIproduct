//! Tauri 2 の `#[tauri::command]` の薄い包み。feature `tauri` のときだけビルドする。中身は `commands.rs`。
//! 重い処理(取り込み・文字起こし・書き出し)は async コマンドにして、画面のスレッドを止めない。

use crate::commands::*;
use crate::store::{GlossaryEntry, Meeting, SearchHit};
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub name: String,
    pub id: Option<i64>,
    pub error: Option<String>,
}

fn import_many(state: &AppState, paths: Vec<PathBuf>, denoise: bool) -> Vec<ImportResult> {
    paths
        .into_iter()
        .map(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match state.import_audio(&p, denoise) {
                Ok(id) => ImportResult { name, id: Some(id), error: None },
                Err(e) => ImportResult { name, id: None, error: Some(e) },
            }
        })
        .collect()
}

/// OS のダイアログで録音ファイルを選んで取り込む(複数可)。取り消したら空。
#[tauri::command]
pub async fn pick_and_import(app: tauri::AppHandle, state: State<'_, AppState>, denoise: bool) -> Result<Vec<ImportResult>, String> {
    let Some(files) = app.dialog().file().add_filter("録音・動画", &AUDIO_EXTS).blocking_pick_files() else { return Ok(vec![]) };
    let paths = files.into_iter().filter_map(|f| f.into_path().ok()).collect();
    Ok(import_many(&state, paths, denoise))
}

/// ドラッグ&ドロップされたファイルを取り込む。
#[tauri::command]
pub async fn import_paths(state: State<'_, AppState>, paths: Vec<String>, denoise: bool) -> Result<Vec<ImportResult>, String> {
    Ok(import_many(&state, paths.into_iter().map(PathBuf::from).collect(), denoise))
}

#[tauri::command]
pub async fn run_jobs(state: State<'_, AppState>) -> Result<usize, String> {
    state.run_jobs()
}

#[tauri::command]
pub fn cancel_jobs(state: State<'_, AppState>) {
    state.cancel_jobs()
}

#[tauri::command]
pub async fn retry(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.retry(id)
}

#[tauri::command]
pub async fn progress(state: State<'_, AppState>) -> Result<ProgressDto, String> {
    state.progress()
}

#[tauri::command]
pub async fn meetings(state: State<'_, AppState>) -> Result<Vec<Meeting>, String> {
    state.meetings()
}

#[tauri::command]
pub async fn detail(state: State<'_, AppState>, id: i64) -> Result<DetailDto, String> {
    state.detail(id)
}

#[tauri::command]
pub async fn audio_path(state: State<'_, AppState>, id: i64) -> Result<Option<String>, String> {
    state.audio_path(id)
}

#[tauri::command]
pub async fn update_meta(state: State<'_, AppState>, id: i64, title: String, held_on: Option<String>, participants: String) -> Result<DetailDto, String> {
    state.update_meta(id, &title, held_on, &participants)
}

#[tauri::command]
pub async fn edit_text(state: State<'_, AppState>, segment_id: i64, text: String) -> Result<DetailDto, String> {
    state.edit_text(segment_id, &text)
}

#[tauri::command]
pub async fn set_speaker(state: State<'_, AppState>, segment_id: i64, speaker: String, following: bool) -> Result<DetailDto, String> {
    state.set_speaker(segment_id, &speaker, following)
}

#[tauri::command]
pub async fn merge_next(state: State<'_, AppState>, segment_id: i64) -> Result<DetailDto, String> {
    state.merge_next(segment_id)
}

#[tauri::command]
pub async fn split(state: State<'_, AppState>, segment_id: i64, at: usize) -> Result<DetailDto, String> {
    state.split(segment_id, at)
}

#[tauri::command]
pub async fn revert_segment(state: State<'_, AppState>, segment_id: i64) -> Result<DetailDto, String> {
    state.revert_segment(segment_id)
}

#[tauri::command]
pub async fn undo(state: State<'_, AppState>, id: i64) -> Result<DetailDto, String> {
    state.undo(id)
}

#[tauri::command]
pub async fn reapply_glossary(state: State<'_, AppState>, id: i64) -> Result<(usize, DetailDto), String> {
    state.reapply_glossary(id)
}

#[tauri::command]
pub async fn confirm(state: State<'_, AppState>, id: i64) -> Result<DetailDto, String> {
    state.confirm(id)
}

#[tauri::command]
pub async fn unconfirm(state: State<'_, AppState>, id: i64) -> Result<DetailDto, String> {
    state.unconfirm(id)
}

#[tauri::command]
pub async fn delete_meeting(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.delete_meeting(id)
}

/// 保存先を OS のダイアログで選んで書き出す(確定した議事録だけ)。取り消したら None。
#[tauri::command]
pub async fn export(app: tauri::AppHandle, state: State<'_, AppState>, id: i64, format: String) -> Result<Option<String>, String> {
    let name = state.export_name(id, &format)?;
    let Some(dest) = app.dialog().file().set_file_name(&name).blocking_save_file() else { return Ok(None) };
    let dest = dest.into_path().map_err(|e| e.to_string())?;
    state.export(id, &format, &dest)?;
    Ok(Some(dest.to_string_lossy().into()))
}

#[tauri::command]
pub async fn export_denoised(app: tauri::AppHandle, state: State<'_, AppState>, id: i64) -> Result<Option<String>, String> {
    let Some(dest) = app.dialog().file().set_file_name("noise-reduced.wav").add_filter("WAV", &["wav"]).blocking_save_file() else { return Ok(None) };
    let dest = dest.into_path().map_err(|e| e.to_string())?;
    state.export_denoised(id, &dest)?;
    Ok(Some(dest.to_string_lossy().into()))
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> Result<Vec<SearchHit>, String> {
    state.search(&query)
}

#[tauri::command]
pub async fn glossary(state: State<'_, AppState>) -> Result<Vec<GlossaryEntry>, String> {
    state.glossary()
}

#[tauri::command]
pub async fn add_glossary(state: State<'_, AppState>, wrong: String, right: String) -> Result<Vec<GlossaryEntry>, String> {
    state.add_glossary(&wrong, &right)
}

#[tauri::command]
pub async fn delete_glossary(state: State<'_, AppState>, id: i64) -> Result<Vec<GlossaryEntry>, String> {
    state.delete_glossary(id)
}

#[tauri::command]
pub fn settings(state: State<'_, AppState>) -> SettingsDto {
    state.settings()
}

#[tauri::command]
pub fn set_flag(state: State<'_, AppState>, key: String, on: bool) -> Result<SettingsDto, String> {
    state.set_flag(&key, on)
}

#[tauri::command]
pub async fn delete_all(state: State<'_, AppState>) -> Result<usize, String> {
    state.delete_all()
}

#[tauri::command]
pub async fn model_status(state: State<'_, AppState>) -> Result<ModelDto, String> {
    Ok(state.model_status())
}

/// モデルを取得する(利用者が設定画面で押したときだけ)。終わるまで返らない。進み具合は model_status で見る。
#[tauri::command]
pub async fn download_model(state: State<'_, AppState>) -> Result<ModelDto, String> {
    state.download_model()
}

#[tauri::command]
pub fn cancel_model_download(state: State<'_, AppState>) {
    state.cancel_model_download()
}

#[tauri::command]
pub async fn delete_model(state: State<'_, AppState>) -> Result<ModelDto, String> {
    state.delete_model()
}

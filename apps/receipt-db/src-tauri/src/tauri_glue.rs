//! Tauri 2 の `#[tauri::command]` の薄い包み。feature `tauri` のときだけビルドする。中身は `commands.rs`。
//! 重い処理(取り込み・読み取り・書き出し)は async コマンドにして、画面のスレッドを止めない。

use crate::commands::*;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn import_files(state: State<'_, AppState>, files: Vec<ImportFileDto>) -> Result<Vec<ImportResultDto>, String> {
    Ok(state.import_files(files))
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
pub fn retry_failed(state: State<'_, AppState>) -> Result<usize, String> {
    state.retry_failed()
}

#[tauri::command]
pub async fn progress(state: State<'_, AppState>) -> Result<ProgressDto, String> {
    state.progress()
}

#[tauri::command]
pub async fn list(state: State<'_, AppState>, query: SearchDto) -> Result<Vec<ReceiptDto>, String> {
    state.list(query)
}

#[tauri::command]
pub async fn get_receipt(state: State<'_, AppState>, id: i64) -> Result<ReceiptDto, String> {
    state.get(id)
}

#[tauri::command]
pub async fn original(state: State<'_, AppState>, document_id: i64) -> Result<String, String> {
    state.original(document_id)
}

#[tauri::command]
pub async fn edit(state: State<'_, AppState>, id: i64, field: String, value: Option<String>) -> Result<ReceiptDto, String> {
    state.edit(id, &field, value)
}

#[tauri::command]
pub async fn undo(state: State<'_, AppState>, id: i64) -> Result<ReceiptDto, String> {
    state.undo(id)
}

#[tauri::command]
pub async fn confirm(state: State<'_, AppState>, ids: Vec<i64>) -> Result<Vec<RejectedDto>, String> {
    state.confirm(ids)
}

#[tauri::command]
pub async fn unconfirm(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.unconfirm(id)
}

#[tauri::command]
pub async fn delete_receipt(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.delete(id)
}

/// 保存先を OS のダイアログで選んでもらい、書き出す。取り消したら None。
#[tauri::command]
pub async fn export_csv(app: tauri::AppHandle, state: State<'_, AppState>, encoding: String) -> Result<Option<String>, String> {
    let Some(dest) = app.dialog().file().set_file_name("receipts.csv").add_filter("CSV", &["csv"]).blocking_save_file() else { return Ok(None) };
    let dest = dest.into_path().map_err(|e| e.to_string())?;
    state.export_csv(&encoding, &dest)?;
    Ok(Some(dest.to_string_lossy().into()))
}

#[tauri::command]
pub async fn export_sqlite(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let Some(dest) = app.dialog().file().set_file_name("receipts-export.sqlite3").blocking_save_file() else { return Ok(None) };
    let dest = dest.into_path().map_err(|e| e.to_string())?;
    state.export_sqlite(&dest)?;
    Ok(Some(dest.to_string_lossy().into()))
}

#[tauri::command]
pub fn settings(state: State<'_, AppState>) -> SettingsDto {
    state.settings()
}

#[tauri::command]
pub fn set_update_check(state: State<'_, AppState>, on: bool) -> Result<SettingsDto, String> {
    state.set_update_check(on)
}

#[tauri::command]
pub async fn delete_all(state: State<'_, AppState>) -> Result<usize, String> {
    state.delete_all()
}

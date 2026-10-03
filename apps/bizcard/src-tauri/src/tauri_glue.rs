//! Tauri 2 の `#[tauri::command]` の薄い包み。feature `tauri` のときだけビルドする。
//! **この環境(Linux クラウド)では tauri をビルドできないため、未検証**(コンパイルも確認していない)。
//! 殻(`tauri.conf.json`、`main.rs`、`Builder::manage(AppState)`)は未作成。
//! 使い方の想定: `tauri::Builder::default().manage(AppState{..}).invoke_handler(tauri::generate_handler![import_card, confirm_card, discard_draft, search])`

use crate::commands::{AppState, ConfirmInput, ReadResultDto, SearchHitDto};
use tauri::State;

#[tauri::command]
pub fn import_card(state: State<'_, AppState>, image_data_url: String, side: String) -> Result<ReadResultDto, String> {
    state.import_card(&image_data_url, &side)
}

#[tauri::command]
pub fn confirm_card(state: State<'_, AppState>, input: ConfirmInput) -> Result<(), String> {
    state.confirm_card(input)
}

#[tauri::command]
pub fn discard_draft(state: State<'_, AppState>, person_id: i64) -> Result<(), String> {
    state.discard_draft(person_id).map(|_| ())
}

#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String) -> Result<Vec<SearchHitDto>, String> {
    state.search(&query)
}

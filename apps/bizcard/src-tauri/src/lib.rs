//! 名刺管理: OCR出力(core::ocr)の後ろ側のロジック(ルール抽出・重複検知・保存と検索・CSV・vCard)。
pub mod csv_export;
pub mod dedup;
pub mod eval;
pub mod extract;
pub mod ocr_input;
pub mod store;
pub mod vcard;

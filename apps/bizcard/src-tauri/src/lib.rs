//! 名刺管理: OCRに依存しないロジック(ルール抽出・重複検知・CSV)。
pub mod csv_export;
pub mod dedup;
pub mod eval;
pub mod extract;
pub mod ocr_input;

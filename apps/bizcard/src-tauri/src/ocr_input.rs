//! OCRの出力型は `core::ocr`(`factory_core::ocr`)のものをそのまま使う。
//! 以前ここにあった暫定の型(`OcrCard`)は廃止した。`OcrPage` に `id`/`side` は無いので、
//! 面(表・裏)は呼び出し側(取り込み処理)が持つ。
pub use factory_core::ocr::{BBox, Ocr, OcrImage, OcrLine, OcrPage};

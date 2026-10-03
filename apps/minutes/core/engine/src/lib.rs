//! ローカルAI買い切りアプリの共通コア(純ロジック部分)。
//!
//! 画面・OCRエンジン・推論エンジン本体は含まない。それらは `trait` の向こう側に置き、
//! ここでは「読み取り結果の検証」「仕訳候補」「CSV書き出し」「ライセンス判定」
//! 「モデル呼び出しの再試行」だけを持つ。外部通信なしでテストできる範囲を広く取っている。

pub mod db;
pub mod error;
pub mod export;
pub mod journal;
pub mod license;
pub mod llm;
pub mod ocr;
pub mod receipt;
pub mod rules;

pub use error::CoreError;

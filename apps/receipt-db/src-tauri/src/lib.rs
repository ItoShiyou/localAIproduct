//! 領収書DB のアプリ固有ロジック。画面(Tauri 2 の殻)は未接続で、`cargo test` で検証できる部分から作る。
//! 共通部品(CSV・ライセンスなど)は `factory-core`(`../core/engine`)を使う。

pub mod journal;
pub mod receipt;
pub mod rules;
pub mod vendor;

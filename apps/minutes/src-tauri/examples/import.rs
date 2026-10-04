//! 開発用: 録音ファイルをアプリのデータフォルダに取り込み、処理の待ちに入れる(画面を操作せずに確認するため)。
//! アプリを起動すると、待ちの処理が自動で始まる。
//! 使い方: cargo run --example import --no-default-features -- <データフォルダ> <録音ファイル...>
//! macOS のデータフォルダ: ~/Library/Application Support/dev.localaiproduct.minutes
use minutes::asr::MissingAsr;
use minutes::commands::AppState;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, files) = args.split_first().expect("使い方: import <データフォルダ> <録音ファイル...>");
    let s = AppState::new(PathBuf::from(dir), Box::new(MissingAsr { reason: "取り込み専用".into() })).unwrap();
    let denoise = s.settings().denoise_default;
    for f in files {
        match s.import_audio(Path::new(f), denoise) {
            Ok(id) => println!("{f}: 取り込みました(id {id})"),
            Err(e) => println!("{f}: {e}"),
        }
    }
}

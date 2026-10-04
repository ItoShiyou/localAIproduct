fn main() {
    // Tauri の殻(feature `tauri`)をビルドするときだけ。ロジックだけのテスト(--no-default-features)では呼ばない
    if std::env::var_os("CARGO_FEATURE_TAURI").is_some() {
        ensure_summarizer_placeholder();
        tauri_build::build();
    }
}

/// 要約のサイドカー(`bundle.externalBin`)は、`tools/build_summarizer.sh` で作って `binaries/` に置く。
/// 開発用(debug)のビルドでは、まだ作っていなくても `cargo build` が通るよう、空の仮ファイルを置く
/// (要約を使うときは、サイドカーを作るか、MINUTES_SUMMARIZER_BIN で指定する)。
/// 配布用(release)のビルドでは仮ファイルを置かない: 無ければ Tauri がエラーにするので、空のまま配布してしまうことがない。
fn ensure_summarizer_placeholder() {
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    let ext = if target.contains("windows") { ".exe" } else { "" };
    let dir = std::path::Path::new("binaries");
    let path = dir.join(format!("minutes-summarizer-{target}{ext}"));
    if !path.exists() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&path, b"");
    }
}

fn main() {
    // Tauri の殻(feature `tauri`)をビルドするときだけ。ロジックだけのテスト(--no-default-features)では呼ばない
    if std::env::var_os("CARGO_FEATURE_TAURI").is_some() {
        tauri_build::build();
    }
}

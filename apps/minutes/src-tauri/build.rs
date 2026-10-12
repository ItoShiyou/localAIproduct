fn main() {
    if std::env::var_os("CARGO_FEATURE_MAC_APP_STORE").is_some() { build_storekit(); }
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

fn build_storekit() {
    use std::{env, path::PathBuf, process::Command};
    let target = env::var("TARGET").unwrap();
    assert!(target == "aarch64-apple-darwin", "MAS currently supports Apple Silicon only");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let status = Command::new("xcrun").args(["swiftc", "-module-cache-path"]).arg(out.join("swift-cache"))
        .args(["-swift-version", "5", "-parse-as-library", "-emit-library", "-static", "-module-name", "MinutesStoreKit", "-target", "arm64-apple-macosx12.0", "apple/StoreKitBridge.swift", "-o"])
        .arg(out.join("libMinutesStoreKit.a")).status().expect("Xcode Swift compiler required");
    assert!(status.success(), "StoreKit bridge compilation failed");
    println!("cargo:rerun-if-changed=apple/StoreKitBridge.swift");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=MinutesStoreKit");
    println!("cargo:rustc-link-search=native=/usr/lib/swift");
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    let toolchain = Command::new("xcode-select").arg("-p").output().unwrap();
    let root = String::from_utf8(toolchain.stdout).unwrap();
    println!("cargo:rustc-link-search=native={}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx", root.trim());
    println!("cargo:rustc-link-lib=framework=StoreKit");
    println!("cargo:rustc-link-lib=framework=Foundation");
}

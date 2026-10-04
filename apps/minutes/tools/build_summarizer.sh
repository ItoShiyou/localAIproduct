#!/usr/bin/env bash
# 要約のサイドカー(llama.cpp。apps/minutes/summarizer/)をビルドして、Tauri の externalBin の場所に置く。
#   src-tauri/binaries/minutes-summarizer-<ターゲット>[.exe]
# tauri build(配布用)の前に1回実行する。tauri dev / cargo build(debug)では無くても動く(仮のファイルが置かれる)。
# 必要なもの: Rust、cmake、C/C++ コンパイラ(macOS は Xcode の Command Line Tools、Windows は MSVC)。
# 本体(whisper.cpp)とは別の実行ファイルにしているのは、両方が持つ ggml の記号が衝突するのを避けるため。
set -euo pipefail
APP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
EXT=""
[[ "$TARGET" == *windows* ]] && EXT=".exe"
# macOS は、アプリの最低対応(tauri.conf.json の minimumSystemVersion)に合わせる
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}"
# CPU だけで動かす(GPU 用の部品を入れない)。llama-cpp-sys-2 は GGML_ で始まる環境変数を CMake にそのまま渡す
export GGML_METAL=OFF
cd "$APP/summarizer"
cargo build --release --target "$TARGET"
mkdir -p "$APP/src-tauri/binaries"
cp "target/$TARGET/release/minutes-summarizer$EXT" "$APP/src-tauri/binaries/minutes-summarizer-$TARGET$EXT"
echo "サイドカーを置きました: src-tauri/binaries/minutes-summarizer-$TARGET$EXT ($(du -h "$APP/src-tauri/binaries/minutes-summarizer-$TARGET$EXT" | cut -f1))"

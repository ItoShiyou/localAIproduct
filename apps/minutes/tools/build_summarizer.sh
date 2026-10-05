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
# Windows(MSVC): cmake クレートが CMAKE_C_FLAGS_RELEASE を上書きして /O2 が消え、最適化なしになる。CMake に渡る環境変数で戻す(CI は workflow の env でも指定)
if [[ "$TARGET" == *windows-msvc ]]; then
  export CMAKE_C_FLAGS_RELEASE="${CMAKE_C_FLAGS_RELEASE:--O2 -Ob2 -DNDEBUG}"
  export CMAKE_CXX_FLAGS_RELEASE="${CMAKE_CXX_FLAGS_RELEASE:--O2 -Ob2 -DNDEBUG}"
fi
cd "$APP/summarizer"
cargo build --release --target "$TARGET"
mkdir -p "$APP/src-tauri/binaries"
# 置き先に空の仮ファイル(src-tauri/build.rs が debug のビルドで置く。実行権限が無い)が既にあると、cp は権限を引き継がず上書きするだけで、
# できあがった .app の中のサイドカーが実行できなくなる(実際に起きた: Permission denied)。消してから、実行権限を付けて置く
DEST="$APP/src-tauri/binaries/minutes-summarizer-$TARGET$EXT"
rm -f "$DEST"
install -m 755 "target/$TARGET/release/minutes-summarizer$EXT" "$DEST"
[[ "$EXT" == ".exe" || -x "$DEST" ]] || { echo "サイドカーに実行権限がありません: $DEST" >&2; exit 1; }
echo "サイドカーを置きました: src-tauri/binaries/minutes-summarizer-$TARGET$EXT ($(du -h "$APP/src-tauri/binaries/minutes-summarizer-$TARGET$EXT" | cut -f1))"

#!/usr/bin/env bash
# 配布物(と tauri dev)に同梱するものを用意する。tauri build / tauri dev の前に1回実行する。
#   1) 文字起こしのモデルを公式の配布元から取得し、SHA-256 を照合して src-tauri/resources/models/ に置く(取得済みなら何もしない)
#   2) 話者の判別の部品(onnxruntime の公式配布、WeSpeaker のモデル)を取得し、SHA-256 を照合して置く
#   3) 第三者ライセンス表記 src-tauri/resources/THIRD_PARTY_NOTICES.txt を作り直す
set -euo pipefail
EDITION="${1:-legacy}"
case "$EDITION" in free|pro|legacy) ;; *) echo 'Select free or pro' >&2; exit 1 ;; esac
APP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$APP/../.." && pwd)"
cd "$APP/src-tauri"
# macOS は shasum、Windows(Git Bash)は sha256sum を使う
sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
PY="$(command -v python3 || command -v python)"
export PYTHONUTF8=1  # Windows の既定(cp1252)だと日本語の出力で落ちる
if [[ "$EDITION" != free ]]; then
  cargo run -q --release --no-default-features --example fetch_model -- "$APP/src-tauri/resources/models"
fi
rm -f resources/models/*.part

fetch() { # <URL> <保存先> <SHA-256>
  if [[ -f "$2" ]] && [[ "$(sha256 "$2")" == "$3" ]]; then return; fi
  mkdir -p "$(dirname "$2")"
  curl -sSfL -o "$2.part" "$1"
  [[ "$(sha256 "$2.part")" == "$3" ]] || { echo "ハッシュが一致しません: $1" >&2; rm -f "$2.part"; exit 1; }
  mv "$2.part" "$2"
}
# 録音中の仮の文字に使う小さなモデル(Whisper small、MIT)
fetch https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin \
  resources/models/ggml-small-q5_1.bin ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb
if [[ "$EDITION" != free ]]; then
fetch https://huggingface.co/Wespeaker/wespeaker-voxceleb-resnet34-LM/resolve/main/voxceleb_resnet34_LM.onnx \
  resources/models/voxceleb_resnet34_LM.onnx 7bb2f06e9df17cdf1ef14ee8a15ab08ed28e8d0ef5054ee135741560df2ec068
if [[ "$(uname -s)" == "Darwin" ]]; then
  TGZ="$(mktemp -d)/ort.tgz"
  fetch https://github.com/microsoft/onnxruntime/releases/download/v1.30.0/onnxruntime-osx-arm64-1.30.0.tgz "$TGZ" \
    6ebb5062a934537c352937821f9fe9718e7de1a2db1122a93dd363ffd53a7012
  tar xzf "$TGZ" -C "$(dirname "$TGZ")"
  mkdir -p resources/onnxruntime
  cp "$(dirname "$TGZ")/onnxruntime-osx-arm64-1.30.0/lib/libonnxruntime.1.30.0.dylib" resources/onnxruntime/libonnxruntime.dylib
else
  case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*)
    ZIP="$(mktemp -d)/ort.zip"
    fetch https://github.com/microsoft/onnxruntime/releases/download/v1.30.0/onnxruntime-win-x64-1.30.0.zip "$ZIP" \
      c6ba983baf5681af108599675d2a89c2d145512d02de28aed0bff177cd0ba949
    mkdir -p resources/onnxruntime
    # dll だけ取り出す(zip には 400MB の pdb も入っている)
    "$PY" -c "import sys,zipfile;z=zipfile.ZipFile(sys.argv[1]);open(sys.argv[2],'wb').write(z.read('onnxruntime-win-x64-1.30.0/lib/onnxruntime.dll'))" \
      "$ZIP" resources/onnxruntime/onnxruntime.dll ;;
  esac
fi
fi
if [[ "$EDITION" == pro ]]; then
  cargo run -q --release --no-default-features --example fetch_summary_model -- "$APP/src-tauri/resources/models"
fi
(cd "$APP/ui" && npm ci --silent)
NOTICE_FEATURES=tauri,whisper
[[ "$EDITION" == free ]] || NOTICE_FEATURES=tauri,whisper,diarize
"$PY" "$ROOT/tools/gen_notices.py" "$APP/src-tauri" "$APP/ui" "$APP/src-tauri/resources/THIRD_PARTY_NOTICES.txt" \
  --features "$NOTICE_FEATURES" --extra "$APP/legal/extra.json"
echo "準備できました: $(du -sh resources | cut -f1)"

# --- 要約のサイドカー(別の実行ファイル)を作って src-tauri/binaries/ に置く(bundle.externalBin)。tools/build_summarizer.sh ---
if [[ "$EDITION" != free ]]; then
  bash "$APP/tools/build_summarizer.sh"
fi

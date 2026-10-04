#!/usr/bin/env bash
# 配布物(と tauri dev)に同梱するものを用意する。tauri build / tauri dev の前に1回実行する。
#   1) 文字起こしのモデルを公式の配布元から取得し、SHA-256 を照合して src-tauri/resources/models/ に置く(取得済みなら何もしない)
#   2) 話者の判別の部品(onnxruntime の公式配布、WeSpeaker のモデル)を取得し、SHA-256 を照合して置く
#   3) 第三者ライセンス表記 src-tauri/resources/THIRD_PARTY_NOTICES.txt を作り直す
set -euo pipefail
APP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$APP/../.." && pwd)"
cd "$APP/src-tauri"
cargo run -q --release --no-default-features --example fetch_model -- "$APP/src-tauri/resources/models"
rm -f resources/models/*.part

fetch() { # <URL> <保存先> <SHA-256>
  if [[ -f "$2" ]] && [[ "$(shasum -a 256 "$2" | cut -d' ' -f1)" == "$3" ]]; then return; fi
  mkdir -p "$(dirname "$2")"
  curl -sSfL -o "$2.part" "$1"
  [[ "$(shasum -a 256 "$2.part" | cut -d' ' -f1)" == "$3" ]] || { echo "ハッシュが一致しません: $1" >&2; rm -f "$2.part"; exit 1; }
  mv "$2.part" "$2"
}
# 録音中の仮の文字に使う小さなモデル(Whisper small、MIT)
fetch https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin \
  resources/models/ggml-small-q5_1.bin ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb
fetch https://huggingface.co/Wespeaker/wespeaker-voxceleb-resnet34-LM/resolve/main/voxceleb_resnet34_LM.onnx \
  resources/models/voxceleb_resnet34_LM.onnx 7bb2f06e9df17cdf1ef14ee8a15ab08ed28e8d0ef5054ee135741560df2ec068
if [[ "$(uname -s)" == "Darwin" ]]; then
  TGZ="$(mktemp -d)/ort.tgz"
  fetch https://github.com/microsoft/onnxruntime/releases/download/v1.30.0/onnxruntime-osx-arm64-1.30.0.tgz "$TGZ" \
    6ebb5062a934537c352937821f9fe9718e7de1a2db1122a93dd363ffd53a7012
  tar xzf "$TGZ" -C "$(dirname "$TGZ")"
  mkdir -p resources/onnxruntime
  cp "$(dirname "$TGZ")/onnxruntime-osx-arm64-1.30.0/lib/libonnxruntime.1.30.0.dylib" resources/onnxruntime/libonnxruntime.dylib
fi
(cd "$APP/ui" && npm ci --silent)
python3 "$ROOT/tools/gen_notices.py" "$APP/src-tauri" "$APP/ui" "$APP/src-tauri/resources/THIRD_PARTY_NOTICES.txt" \
  --features tauri,whisper,diarize --extra "$APP/legal/extra.json"
echo "準備できました: $(du -sh resources | cut -f1)"

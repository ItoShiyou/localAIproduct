#!/usr/bin/env bash
# 配布物(と tauri dev)に同梱するものを用意する。tauri build / tauri dev の前に1回実行する。
#   1) 文字起こしのモデルを公式の配布元から取得し、SHA-256 を照合して src-tauri/resources/models/ に置く(取得済みなら何もしない)
#   2) 第三者ライセンス表記 src-tauri/resources/THIRD_PARTY_NOTICES.txt を作り直す
set -euo pipefail
APP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$APP/../.." && pwd)"
cd "$APP/src-tauri"
cargo run -q --release --no-default-features --example fetch_model -- "$APP/src-tauri/resources/models"
rm -f resources/models/*.part
(cd "$APP/ui" && npm ci --silent)
python3 "$ROOT/tools/gen_notices.py" "$APP/src-tauri" "$APP/ui" "$APP/src-tauri/resources/THIRD_PARTY_NOTICES.txt" \
  --features tauri,whisper --extra "$APP/legal/extra.json"
echo "準備できました: $(du -sh resources | cut -f1)"

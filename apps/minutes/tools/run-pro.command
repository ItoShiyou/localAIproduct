#!/bin/bash
# 開発用: 手元の議事録アプリに、自分用のライセンスを入れて起動し直す(Finder でダブルクリック)。
# 配布用のビルドは環境変数 MINUTES_TIER を無視するため(2026-10-05〜)、有料版にするにはライセンスが要る。
# 使い方: ライセンスファイルの場所を引数に渡す。省略時は ~/.config/minutes-license/owner.license
#   例) license_issuer issue --licensee "自分" --out ~/.config/minutes-license/owner.license
# (製品用の公開鍵が埋め込まれたビルドで使える。手順は docs/license.md)
APP="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/target/release/bundle/macos/minutes.app"
LIC="${1:-$HOME/.config/minutes-license/owner.license}"
DATA="$HOME/Library/Application Support/dev.localaiproduct.minutes"
if [ ! -d "$APP" ]; then echo "アプリが見つかりません: $APP(先に tauri build してください)"; read -r; exit 1; fi
if [ ! -f "$LIC" ]; then echo "ライセンスファイルが見つかりません: $LIC(docs/license.md の手順で発行してください)"; read -r; exit 1; fi
pkill -f "minutes.app/Contents/MacOS/minutes" 2>/dev/null; sleep 1
mkdir -p "$DATA" && cp "$LIC" "$DATA/license.key"
nohup "$APP/Contents/MacOS/minutes" >/dev/null 2>&1 &
echo "ライセンスを入れて起動しました。この画面は閉じて大丈夫です。"

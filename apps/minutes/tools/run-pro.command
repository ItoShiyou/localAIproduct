#!/bin/bash
# 開発用: 議事録アプリを「有料版」として起動する(ライセンス認証をつなぐまでの仮の方法)。
# Finder でダブルクリックすると、開いているアプリを終了してから、有料版で起動し直す。
APP="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/target/release/bundle/macos/minutes.app"
if [ ! -d "$APP" ]; then echo "アプリが見つかりません: $APP(先に tauri build してください)"; read -r; exit 1; fi
pkill -f "minutes.app/Contents/MacOS/minutes" 2>/dev/null; sleep 1
MINUTES_TIER=pro nohup "$APP/Contents/MacOS/minutes" >/dev/null 2>&1 &
echo "有料版として起動しました。この画面は閉じて大丈夫です。"

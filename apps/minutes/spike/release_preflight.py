"""Read-only checks of known release blockers; not a certification of readiness.

Run from any directory: python3 apps/minutes/spike/release_preflight.py
Missing production configuration intentionally exits 1. Payment, contract and
hardware checks remain manual even if these local checks pass.
"""
import json
import plistlib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def checks(config, key_source, notices):
    match = re.search(r'PRODUCTION_PUBLIC_KEY_HEX:\s*&str\s*=\s*"([^"]*)"', key_source)
    key = match.group(1) if match else ""
    owner = config.get("bundle", {}).get("copyright", "")
    return {
        "Pro本番公開鍵の形式（署名・交付は別途確認）": bool(re.fullmatch(r"[0-9a-fA-F]{64}", key)),
        "販売者の著作権表示": bool(owner.strip()) and not any(word in owner for word in ("販売前", "記入", "仮称")),
        "画面タイトル": all("仮称" not in window.get("title", "") for window in config.get("app", {}).get("windows", [])),
        "第三者ライセンス同梱": len(notices) > 1000 and all(name in notices for name in ("Whisper", "WeSpeaker", "Qwen", "MPL")),
    }


def mac_permissions(config, entitlements):
    mac = config.get('bundle', {}).get('macOS', {})
    return (mac.get('hardenedRuntime') is True
            and mac.get('entitlements') == 'Entitlements.plist'
            and entitlements.get('com.apple.security.device.audio-input') is True)


def main():
    config = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())
    key_source = (ROOT / "src-tauri/src/license.rs").read_text()
    notice_path = ROOT / "src-tauri/resources/THIRD_PARTY_NOTICES.txt"
    result = checks(config, key_source, notice_path.read_text() if notice_path.exists() else "")
    try:
        with (ROOT / 'src-tauri/Entitlements.plist').open('rb') as source:
            entitlements = plistlib.load(source)
    except (OSError, plistlib.InvalidFileException):
        entitlements = {}
    result['署名時のマイク権限・Hardened Runtime設定'] = mac_permissions(config, entitlements)
    for name, passed in result.items():
        print(f"{'確認済み' if passed else '未完了'}: {name}")
    print("別途必須: 配布署名・公証、実購入→キー交付→登録、販売表記・規約・返金条件、実機マイク・機器抜去試験")
    return 0 if all(result.values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())

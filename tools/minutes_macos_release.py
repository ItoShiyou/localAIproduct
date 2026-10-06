"""Developer-ID packaging plan. --execute signs/uploads only with owner credentials.
Secrets are read by Apple's tools from Keychain, never through this script.
"""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys

REPO = Path(__file__).resolve().parents[1]
APP_ROOT = REPO / 'apps/minutes'
sys.path.insert(0, str(APP_ROOT / 'spike'))
from release_preflight import checks, mac_permissions

def bundle_info(app):
    if not app.is_dir() or app.name != 'minutes.app':
        raise ValueError('minutes.appを指定してください')
    with (app / 'Contents/Info.plist').open('rb') as source:
        info = plistlib.load(source)
    if info.get('CFBundleIdentifier') != 'dev.localaiproduct.minutes':
        raise ValueError('minutes以外のアプリには署名しません')
    version = info.get('CFBundleShortVersionString', '')
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?', version):
        raise ValueError('アプリの版情報が不正です')
    for path in app.rglob('*'):
        if path.is_symlink() and (path.readlink().is_absolute() or app not in path.resolve().parents):
            raise ValueError('アプリ外を参照するリンクがあります')
    return version

def execute(app, output, identity, profile, run=None):
    version = bundle_info(app)
    if app == output or app in output.parents:
        raise ValueError('元アプリの中へ配布物を作ることはできません')
    if not identity.startswith('Developer ID Application:'):
        raise ValueError('Developer ID Applicationの署名IDを指定してください')
    if not profile.strip():
        raise ValueError('Keychainの公証プロファイル名が必要です')
    if output.exists():
        raise ValueError('出力先は新しいフォルダーを指定してください。上書きしません')
    config = json.loads((APP_ROOT / 'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))
    notices = APP_ROOT / 'src-tauri/resources/THIRD_PARTY_NOTICES.txt'
    result = checks(config, (APP_ROOT / 'src-tauri/src/license.rs').read_text(encoding='utf-8'), notices.read_text(encoding='utf-8') if notices.exists() else '')
    if not all(result.values()):
        raise ValueError('販売前設定が未完了です: ' + ', '.join(k for k,v in result.items() if not v))
    with (APP_ROOT / 'src-tauri/Entitlements.plist').open('rb') as source:
        permissions = plistlib.load(source)
    if not mac_permissions(config, permissions):
        raise ValueError('署名時のマイク権限またはHardened Runtime設定が不足しています')
    if run is None:
        def run(arguments):
            completed = subprocess.run(arguments, check=True, capture_output=True, text=True, encoding='utf-8')
            return completed.stdout
    output.mkdir(parents=True)
    staged = output / 'staging/minutes.app'
    staged.parent.mkdir()
    shutil.copytree(app, staged, symlinks=True)
    magic = {bytes.fromhex(value) for value in ['feedface','feedfacf','cefaedfe','cffaedfe','cafebabe','bebafeca','cafebabf','bfbafeca']}
    for path in sorted(staged.rglob('*'), key=lambda p:len(p.parts), reverse=True):
        if path.is_file() and not path.is_symlink():
            with path.open('rb') as file:
                is_macho = file.read(4) in magic
            if is_macho:
                run(['codesign','--force','--timestamp','--options','runtime','--sign',identity,str(path)])
    entitlement = APP_ROOT / 'src-tauri/Entitlements.plist'
    run(['codesign','--force','--timestamp','--options','runtime','--entitlements',str(entitlement),'--sign',identity,str(staged)])
    run(['codesign','--verify','--deep','--strict',str(staged)])
    upload = output / 'notary-submission.zip'
    run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(staged),str(upload)])
    response = json.loads(run(['xcrun','notarytool','submit',str(upload),'--keychain-profile',profile,'--wait','--output-format','json']))
    if response.get('status') != 'Accepted':
        raise ValueError('Appleが公証を受理していません。配布ファイルを作りません')
    run(['xcrun','stapler','staple',str(staged)])
    run(['xcrun','stapler','validate',str(staged)])
    run(['spctl','--assess','--type','execute','--verbose=2',str(staged)])
    final = output / f'minutes-{version}-macos-arm64.zip'
    run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(staged),str(final)])
    digest = hashlib.sha256()
    with final.open('rb') as file:
        for chunk in iter(lambda:file.read(1024*1024), b''):
            digest.update(chunk)
    report = {'version':version,'file':final.name,'sha256':digest.hexdigest(),'bytes':final.stat().st_size,
        'notarization_id':response.get('id'), 'note':'実ダウンロード・マイク試験と契約／決済確認は別途必須。自動公開はしない。'}
    (output / 'release-manifest.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    return report

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--identity', required=True)
    parser.add_argument('--notary-profile', required=True)
    parser.add_argument('--execute', action='store_true', help='Sign the copy and upload the app to Apple for notarization')
    args = parser.parse_args()
    app, output = args.app.resolve(), args.output.resolve()
    version = bundle_info(app)
    if not args.execute:
        print(f'計画のみ: minutes {version}。元アプリを保持し、コピーの署名→Apple公証→チケット付与→配布ZIPとSHA-256を作成します。')
        print('実署名・通信・ファイル変更はしていません。所有者が準備してから --execute を指定してください。')
        return
    if sys.platform != 'darwin':
        parser.error('実署名はmacOSで実行してください')
    print(json.dumps(execute(app,output,args.identity,args.notary_profile),ensure_ascii=False,indent=2))

if __name__ == '__main__':
    try:
        main()
    except (ValueError,OSError,subprocess.CalledProcessError) as error:
        raise SystemExit(f'配布準備を停止しました: {error}')

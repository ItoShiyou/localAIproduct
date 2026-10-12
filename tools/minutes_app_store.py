"""Validate/sign a COPY for Mac App Store. Never publish or access private keys directly."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import plistlib
import re
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TAURI = ROOT / 'apps/minutes/src-tauri'
HASHES = {
 'ggml-small-q5_1.bin':'ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb',
 'ggml-large-v3-turbo-q5_0.bin':'394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2',
 'Qwen3-4B-Instruct-2507-Q4_K_M.gguf':'3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597',
 'voxceleb_resnet34_LM.onnx':'7bb2f06e9df17cdf1ef14ee8a15ab08ed28e8d0ef5054ee135741560df2ec068',
}
def validate(app):
    info = plistlib.loads((app/'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier') != 'dev.localaiproduct.minutes' or info.get('MinutesDistributionChannel') != 'mac-app-store':
        raise ValueError('Mac App Store専用ビルドだけを受け付けます')
    if info.get('LSApplicationCategoryType') != 'public.app-category.productivity':
        raise ValueError('Mac App Store用の仕事効率化カテゴリがありません')
    if not info.get('NSMicrophoneUsageDescription'):
        raise ValueError('マイク使用説明がありません')
    privacy = plistlib.loads((app/'Contents/Resources/PrivacyInfo.xcprivacy').read_bytes())
    if privacy.get('NSPrivacyTracking') is not False or privacy.get('NSPrivacyTrackingDomains') != [] or privacy.get('NSPrivacyCollectedDataTypes') != []:
        raise ValueError('PrivacyInfo.xcprivacyが現在の収集しない方針と一致しません')
    models = app/'Contents/Resources/resources/models'
    bundled = list(models.iterdir())
    if {path.name for path in bundled} != set(HASHES) or any(path.is_symlink() or not path.is_file() for path in bundled):
        raise ValueError('同梱モデルは商用配布を確認した4ファイルのみ許可します')
    for name, expected in HASHES.items():
        h = hashlib.sha256()
        with (models/name).open('rb') as source:
            for chunk in iter(lambda: source.read(1024*1024), b''): h.update(chunk)
        if h.hexdigest() != expected: raise ValueError('モデルのハッシュ不一致: '+name)
    if not (app/'Contents/Resources/resources/THIRD_PARTY_NOTICES.txt').is_file():
        raise ValueError('ライセンス表記がありません')
    helper = app/'Contents/MacOS/minutes-summarizer'
    if not helper.is_file() or helper.stat().st_size < 1000:
        raise ValueError('要約エンジンがありません')
    for path in app.rglob('*'):
        if path.is_symlink() and app.resolve() not in path.resolve().parents:
            raise ValueError('バンドル外への参照があります')
    runtime = app/'Contents/Resources/resources/onnxruntime/libonnxruntime.dylib'
    def version(value): return tuple(int(n) for n in value.split('.'))
    for engine in [app/'Contents/MacOS'/info['CFBundleExecutable'], helper, runtime]:
        architectures = subprocess.check_output(['lipo','-archs',str(engine)],text=True).split()
        if architectures != ['arm64']:
            raise ValueError('Apple Silicon専用エンジンが必要です: '+engine.name)
        builds = subprocess.check_output(['vtool','-show-build',str(engine)],text=True)
        minimums = re.findall(r'minos\s+([0-9.]+)',builds)
        if not minimums:
            raise ValueError('エンジンの最低OSを検証できません: '+engine.name)
        if version(info.get('LSMinimumSystemVersion','0.0')) < max(map(version,minimums)):
            raise ValueError('最低対応OSが同梱エンジンの必要OSより古いです。エンジンを再構築してください: '+engine.name)
    return info

def validate_distribution_profile(document, bundle_id, now=None):
    """Reject an expired or development/device-limited profile before copying/signing."""
    grant = document.get('Entitlements', {})
    appid = grant.get('com.apple.application-identifier', '')
    if not isinstance(appid, str) or not appid.endswith('.'+bundle_id) or '*' in appid:
        raise ValueError('プロファイルとBundle IDが一致しません')
    expiry = document.get('ExpirationDate')
    if not isinstance(expiry, datetime):
        raise ValueError('プロファイルの有効期限を確認できません')
    if expiry.tzinfo is None: expiry = expiry.replace(tzinfo=timezone.utc)
    now = now or datetime.now(timezone.utc)
    if expiry <= now:
        raise ValueError('配布プロファイルの有効期限が切れています')
    if grant.get('get-task-allow') or grant.get('com.apple.security.get-task-allow'):
        raise ValueError('開発用プロファイルはApp Store配布に使えません')
    if document.get('ProvisionedDevices') or document.get('ProvisionsAllDevices'):
        raise ValueError('デバイス限定・Developer ID用プロファイルはApp Store配布に使えません')
    team = grant.get('com.apple.developer.team-identifier')
    if not team or team not in document.get('TeamIdentifier', []):
        raise ValueError('プロファイルのTeam IDが一致しません')
    return grant

def prepare(app, output, identity, installer=None, profile=None, release_approved=False):
    info = validate(app)
    production = identity != '-'
    if production:
        copyright_text = info.get('NSHumanReadableCopyright','')
        if not copyright_text or '権利者名' in copyright_text:
            raise ValueError('正式な権利者の著作権表示をビルド構成に記入してください')
        if not identity.startswith(('Apple Distribution:', '3rd Party Mac Developer Application:')):
            raise ValueError('App Store用Application署名証明書が必要です（Developer ID不可）')
        if not installer or not installer.startswith('3rd Party Mac Developer Installer:'):
            raise ValueError('App Store用Installer証明書が必要です')
        if not profile: raise ValueError('App Store配布用プロビジョニングプロファイルが必要です')
        if not release_approved:
            raise ValueError('価格・無料上限・モデル権利・実機QAを確定した承認記録が必要です')
    if output.exists(): raise ValueError('出力先は新しいディレクトリを指定してください')
    if app.resolve() == output.resolve() or app.resolve() in output.resolve().parents:
        raise ValueError('元アプリ内に出力できません')
    ent = plistlib.loads((TAURI/'Entitlements.mas.plist').read_bytes())
    if production:
        decoded = subprocess.check_output(['security','cms','-D','-i',str(profile)])
        grant = validate_distribution_profile(plistlib.loads(decoded), info['CFBundleIdentifier'])
        for key in ['com.apple.application-identifier','com.apple.developer.team-identifier','keychain-access-groups']:
            if key in grant: ent[key] = grant[key]
    output.mkdir(parents=True)
    staged = output/app.name
    shutil.copytree(app, staged, symlinks=True)
    if production: shutil.copy2(profile, staged/'Contents/embedded.provisionprofile')
    entitlement = output/'app-entitlements.plist'; entitlement.write_bytes(plistlib.dumps(ent))
    magic = {bytes.fromhex(x) for x in ['feedface','feedfacf','cefaedfe','cffaedfe','cafebabe','bebafeca','cafebabf','bfbafeca']}
    for path in sorted(staged.rglob('*'), key=lambda p: len(p.parts), reverse=True):
        if path.is_file() and not path.is_symlink():
            with path.open('rb') as source: macho = source.read(4) in magic
            if macho:
                args=['codesign','--force','--sign',identity]
                if path.name.startswith('minutes-summarizer'):
                    args += ['--entitlements',str(TAURI/'Entitlements.sidecar.plist')]
                subprocess.run(args+[str(path)],check=True)
    subprocess.run(['codesign','--force','--sign',identity,'--entitlements',str(entitlement),str(staged)],check=True)
    subprocess.run(['codesign','--verify','--deep','--strict',str(staged)],check=True)
    if production:
        subprocess.run(['productbuild','--component',str(staged),'/Applications','--sign',installer,str(output/'minutes.pkg')],check=True)
        subprocess.run(['pkgutil','--check-signature',str(output/'minutes.pkg')],check=True)
    report={'version':info['CFBundleShortVersionString'],'build':info.get('CFBundleVersion'),'channel':'mac-app-store','distribution_signed':production,
            'app':str(staged),'uploaded':False,'note':('App Store用署名済み。アップロード・TestFlight・審査・公開は別工程です' if production else '開発用署名では購入・TestFlight・Apple審査の成功を証明しません')}
    (output/'manifest.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    return report

if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',required=True,type=Path);p.add_argument('--output',required=True,type=Path)
    p.add_argument('--identity',default='-');p.add_argument('--installer-identity');p.add_argument('--profile',type=Path)
    p.add_argument('--release-approved',action='store_true');p.add_argument('--execute',action='store_true')
    a=p.parse_args()
    try:
        validate(a.app)
        if a.execute: print(json.dumps(prepare(a.app,a.output,a.identity,a.installer_identity,a.profile,a.release_approved),ensure_ascii=False))
        else: print('検証のみ完了。署名・アップロード・公開は行っていません。')
    except (ValueError,OSError,subprocess.CalledProcessError) as error: raise SystemExit(str(error))

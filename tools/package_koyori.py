"""Portable, allowlisted brand-kit build. No app/user data are included."""
import argparse
import hashlib
import json
import posixpath
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlsplit
from zipfile import ZIP_DEFLATED, ZipFile

def build(output):
    repo = Path(__file__).resolve().parents[1]
    paths = ['web/minutes/KOYORI_BRAND_GUIDE.md', 'web/minutes/minutes.css',
             'web/minutes/brand.css', 'web/minutes/bird.css', 'web/minutes/koyori.css',
             'web/minutes/koyori.js', 'web/minutes/bird-motion.mjs', 'web/minutes/motion.js',
             'web/minutes/character/index.html', 'web/minutes/character/character.css']
    paths += [f'web/assets/minutes-bird{pose}.png' for pose in ['', '-wave', '-blink', '-tilt']]
    payload = {path.removeprefix('web/'): (repo / path).read_bytes() for path in paths}
    html = payload['minutes/character/index.html'].decode()
    # Only product navigation goes online; every animation asset remains local.
    for local, remote in [('../#beta', 'https://itoshiyou.github.io/localAIproduct/minutes/#beta'),
                          ('../privacy/', 'https://itoshiyou.github.io/localAIproduct/minutes/privacy/'),
                          ('../', 'https://itoshiyou.github.io/localAIproduct/minutes/')]:
        html = html.replace(f'href="{local}"', f'href="{remote}"')
    payload['minutes/character/index.html'] = html.encode()
    class References(HTMLParser):
        def handle_starttag(self, tag, attributes):
            for key, ref in attributes:
                if key not in ('href', 'src') or not ref:
                    continue
                url = urlsplit(ref)
                if url.scheme or url.netloc or not url.path:
                    continue
                target = posixpath.normpath(posixpath.join('minutes/character', url.path))
                if url.path.endswith('/'):
                    target += '/index.html'
                assert target in payload, f'Missing brand-kit resource: {target}'
    References().feed(html)
    payload['README.md'] = '''# こより / KOYORI 素材セット

minutesの制作担当者向け。第三者への自由な再配布・改変を許諾するライセンスではありません。
商標の正式調査は未完了です。使用方針は minutes/KOYORI_BRAND_GUIDE.md を参照。

透過PNG4枚、Webの6しぐさ、実演ページ、停止ボタン、ブランド資料、ハッシュ一覧を収録。
GIF・動画・Lottie・印刷用ベクターは含みません。秘密鍵、録音、アプリ本体も含みません。

展開したkoyoriフォルダーで `python3 -m http.server 8090 --bind 127.0.0.1` を実行し、
http://127.0.0.1:8090/minutes/character/ を開くと実演できます。終了はCtrl+C。
ファイルを直接開くと、ブラウザーの制限でアニメーションが実行されない場合があります。
動きと画像はネット接続不要。製品ページへのリンクを押した場合だけ外部サイトを開きます。
OSの動き制限、画面下部の停止ボタン、画面外・別タブでの停止に対応します。
'''.encode()
    manifest = {name: hashlib.sha256(data).hexdigest() for name, data in payload.items()}
    payload['SHA256.json'] = json.dumps(manifest, ensure_ascii=False, indent=2).encode()
    with ZipFile(output, 'w', ZIP_DEFLATED) as archive:
        for name, data in payload.items():
            archive.writestr('koyori/' + name, data)
    with ZipFile(output) as archive:
        assert archive.testzip() is None
        for name, digest in manifest.items():
            assert hashlib.sha256(archive.read('koyori/' + name)).hexdigest() == digest
    print(f'PASS {len(payload)} files; hashes verified; {output}')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path, help='Output ZIP (outside source tree recommended)')
    build(parser.parse_args().output)

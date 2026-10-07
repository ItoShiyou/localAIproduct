"""Package an unsigned Windows Pro ZIP candidate; never publishes or signs.
The caller must build edition-pro first. Not a certification of sales readiness.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import zipfile
from minutes_macos_release import MODEL_HASHES

def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda:source.read(1024*1024), b''):
            result.update(chunk)
    return result.hexdigest()

def package(exe, resources, sidecar, output):
    exe, resources, sidecar, output = (p.resolve() for p in (exe, resources, sidecar, output))
    if output.exists():
        raise ValueError('Use a new output directory; existing files are never replaced')
    if any(output == p or output in p.parents or p in output.parents for p in (exe, resources, sidecar)):
        raise ValueError('Output must be separate from build inputs')
    pairs = [(exe, 'minutes.exe'), (sidecar, 'minutes-summarizer.exe'),
             (resources / 'onnxruntime/onnxruntime.dll', 'resources/onnxruntime/onnxruntime.dll'),
             (resources / 'THIRD_PARTY_NOTICES.txt', 'resources/THIRD_PARTY_NOTICES.txt')]
    pairs += [(resources / 'models' / name, 'resources/models/' + name) for name in MODEL_HASHES]
    for source, relative in pairs:
        if not source.is_file() or source.is_symlink() or source.stat().st_size == 0:
            raise ValueError('Missing or invalid build input: ' + relative)
        if relative.endswith(('.exe', '.dll')):
            with source.open('rb') as file:
                if file.read(2) != b'MZ':
                    raise ValueError('Not a Windows executable: ' + relative)
        name = source.name
        if name in MODEL_HASHES and digest(source) != MODEL_HASHES[name]:
            raise ValueError('Model hash mismatch: ' + name)
    output.mkdir(parents=True)
    staged = output / 'minutes Pro'
    staged.mkdir()
    entries = []
    for source, relative in pairs:
        target = staged / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        entries.append({'path':relative, 'bytes':target.stat().st_size, 'sha256':digest(target)})
    (staged / 'TEST-CANDIDATE.txt').write_text(
        'Windows x64 Pro 検証候補・未署名・販売用ではありません。\n'
        '必要な処理データは同梱しています。フォルダー全体を展開し、minutes.exeを起動します。\n'
        'WebView2 RuntimeとMicrosoft Visual C++ x64 Runtimeの準備が別途必要な場合があります。\n'
        '開発用ランナーでの試験は、クリーンな実機のインストール・マイク・SmartScreen試験の代わりではありません。\n'
        '無料版とProを同時に起動しないでください。大切な記録はバックアップしてください。\n', encoding='utf-8')
    manifest = {'edition':'pro', 'platform':'windows-x64', 'signed':False,
                'salesReady':False, 'files':entries}
    (staged / 'candidate-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    archive = output / 'minutes-pro-windows-x64-candidate.zip'
    # ZIP64 and stored entries avoid the single-executable NSIS size limit.
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_STORED, allowZip64=True) as zip:
        for path in sorted(staged.rglob('*')):
            if path.is_file():
                zip.write(path, path.relative_to(output))
    return {'file':archive.name, 'bytes':archive.stat().st_size, 'sha256':digest(archive), **manifest}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'resources', 'sidecar', 'output'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(package(args.exe, args.resources, args.sidecar, args.output), ensure_ascii=False, indent=2))

if __name__ == '__main__':
    main()

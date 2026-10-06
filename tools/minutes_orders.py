"""Offline, operator-confirmed order fulfillment. No payment verification or email."""
import argparse
import json
import os
from pathlib import Path
import re
import sqlite3
import subprocess

def protected_path(path):
    path = Path(path).expanduser().resolve()
    if any((parent / '.git').exists() for parent in [path.parent, *path.parents]):
        raise ValueError('注文台帳はGitリポジトリの外に置いてください')
    path.parent.mkdir(parents=True, exist_ok=True)
    # Only secure the dedicated file, never chmod a broad existing parent.
    fd = os.open(path, os.O_CREAT | os.O_RDWR, 0o600)
    os.close(fd)
    if os.name == 'posix':
        path.chmod(0o600)
    return path

def fulfill(path, order_id, request, issue, payment_confirmed=False):
    if not payment_confirmed:
        raise ValueError('販売者が支払いを確認してから --payment-confirmed を指定してください')
    if not re.fullmatch(r'[A-Za-z0-9._:-]{1,200}', order_id):
        raise ValueError('注文IDはサービス名を含む英数字・.-_:で指定してください')
    if not isinstance(request.get('licensee'), str) or not 1 <= len(request['licensee'].strip()) <= 256:
        raise ValueError('購入者名が不正です')
    if request.get('edition') not in ('pro', 'pro_offline'):
        raise ValueError('ライセンス版が不正です')
    encoded = json.dumps(request, ensure_ascii=False, sort_keys=True)
    db = sqlite3.connect(protected_path(path), timeout=30)
    try:
        # Do not create customer-bearing WAL sidecars with weaker permissions.
        db.execute('PRAGMA journal_mode=DELETE')
        db.execute('PRAGMA synchronous=FULL')
        db.execute('CREATE TABLE IF NOT EXISTS orders (id TEXT PRIMARY KEY, request TEXT NOT NULL, license TEXT NOT NULL, blocked INTEGER NOT NULL DEFAULT 0)')
        db.commit()
        db.execute('BEGIN IMMEDIATE')
        previous = db.execute('SELECT request, license, blocked FROM orders WHERE id=?', (order_id,)).fetchone()
        if previous:
            if previous[2]:
                raise ValueError('この注文は返金・保留のため再交付できません')
            if previous[0] != encoded:
                raise ValueError('同じ注文IDに異なる購入者・版を指定できません')
            db.commit()
            return previous[1], False
        license_text = issue(request)
        if not license_text.startswith('MNT1-'):
            raise ValueError('発行コマンドから有効な形式のキーが得られません')
        db.execute('INSERT INTO orders(id,request,license) VALUES (?,?,?)', (order_id, encoded, license_text))
        db.commit()
        return license_text, True
    finally:
        db.close()

def block_order(path, order_id):
    path = Path(path).resolve()
    if not path.is_file():
        raise ValueError('台帳がありません')
    db = sqlite3.connect(protected_path(path), timeout=30)
    try:
        with db:
            result = db.execute('UPDATE orders SET blocked=1 WHERE id=?', (order_id,))
            if result.rowcount != 1:
                raise ValueError('注文が見つかりません')
    finally:
        db.close()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ledger', required=True, type=Path)
    parser.add_argument('--order-id', required=True)
    parser.add_argument('--block', action='store_true', help='Stop re-delivery; does not revoke an offline installed key')
    parser.add_argument('--payment-confirmed', action='store_true')
    parser.add_argument('--licensee')
    parser.add_argument('--edition', choices=['pro', 'pro_offline'], default='pro_offline')
    parser.add_argument('--issuer', type=Path)
    parser.add_argument('--key', type=Path)
    parser.add_argument('--test-mode', action='store_true', help='Allow a publicly known development key for isolated tests only')
    args = parser.parse_args()
    if args.block:
        block_order(args.ledger, args.order_id)
        print('再交付を停止しました。オフラインの既存キーは即時失効しません。')
        return
    if not args.issuer or not args.key:
        parser.error('--issuer と --key が必要です')
    issuer = str(args.issuer.resolve())
    key = args.key.expanduser().resolve()
    if any((parent / '.git').exists() for parent in key.parents):
        parser.error('秘密鍵はGitリポジトリの外に置いてください')
    def invoke(arguments):
        result = subprocess.run([issuer, *arguments], capture_output=True, text=True, encoding='utf-8', timeout=30)
        if result.returncode:
            raise ValueError('発行・署名検証コマンドが失敗しました。キーや購入者情報をログへ貼り付けないでください')
        return result.stdout
    public = invoke(['pubkey', '--key', str(key)]).strip()
    if not args.test_mode and public == invoke(['pubkey', '--dev']).strip():
        parser.error('開発用の鍵では製品ライセンスを交付できません')
    if os.name == 'posix' and key.stat().st_mode & 0o077:
        parser.error('秘密鍵のファイル権限を所有者だけが読める状態にしてください（600）')
    def issue(request):
        output = invoke(['issue', '--key', str(key), '--licensee', request['licensee'], '--edition', request['edition']])
        token = next((line.strip() for line in output.splitlines() if line.startswith('MNT1-')), '')
        verified = invoke(['verify', token, '--pubkey', public])
        payload = json.loads(verified[verified.index('{'):])
        if payload['licensee'] != request['licensee'] or payload['edition'] != request['edition']:
            raise ValueError('署名検証後の購入者または版が一致しません')
        return token
    token, created = fulfill(args.ledger, args.order_id,
        {'licensee': args.licensee, 'edition': args.edition, 'public_key': public}, issue, args.payment_confirmed)
    print('新規交付' if created else '既存キーを再交付')
    print(token)  # Explicit operator output; not an email/send operation.

if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, sqlite3.Error, subprocess.TimeoutExpired) as error:
        raise SystemExit(f'エラー: {error}')

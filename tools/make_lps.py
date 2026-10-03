#!/usr/bin/env python3
"""ideas.json の各案から、試験用の静的LPを dist/<slug>/index.html に生成する。

使い方:
  python3 tools/make_lps.py              # 全案を生成
  python3 tools/make_lps.py --only pii-mask receipt-ledger
  python3 tools/make_lps.py --serve      # 生成後 http://localhost:8000 で確認

標準ライブラリのみ。form_action(登録の送信先)は config.json に設定する。
未設定のままだと、各ページ上部に赤いバナーが出る(公開前に必ず設定)。
"""
import argparse
import html
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def esc(s: str) -> str:
    return html.escape(s, quote=True)


def render(idea: dict, cfg: dict, tpl: str) -> str:
    form_action = cfg.get("form_action", "").strip()
    dev = not form_action
    banner = (
        '<div class="dev">FORM_ACTION 未設定: このページは登録を受け付けません。'
        "config.json を設定してから公開してください。</div>"
        if dev
        else ""
    )
    banner_css = (
        ".dev{background:#b3261e;color:#fff;padding:8px 16px;text-align:center;font-size:14px}"
        if dev
        else ""
    )
    contact = (
        f' ・ <a href="mailto:{esc(cfg["contact_email"])}">お問い合わせ</a>'
        if cfg.get("contact_email")
        else ""
    )
    privacy = (
        f' ・ <a href="{esc(cfg["privacy_url"])}">プライバシー</a>'
        if cfg.get("privacy_url")
        else ""
    )
    features = "\n".join(f"<li>{esc(f)}</li>" for f in idea["features"])
    os_opts = "".join(f'<option>{esc(o)}</option>' for o in cfg["os_options"])
    price_opts = "".join(f'<option>{esc(o)}</option>' for o in cfg["price_options"])

    repl = {
        "{{NAME}}": esc(idea["name"]),
        "{{STUDIO}}": esc(cfg["studio_name"]),
        "{{TAGLINE}}": esc(idea["tagline"]),
        "{{PAIN}}": esc(idea["pain"]),
        "{{AUDIENCE}}": esc(idea["audience"]),
        "{{FEATURES}}": features,
        "{{LOCAL_REASON}}": esc(idea["local_reason"]),
        "{{NOTE}}": esc(idea["note"]),
        "{{SLUG}}": esc(idea["slug"]),
        "{{FORM_ACTION}}": esc(form_action or "#"),
        "{{FORM_METHOD}}": esc(cfg.get("form_method", "POST")),
        "{{SUPPORTED_OS}}": esc(cfg.get("supported_os_text", "")),
        "{{OS_OPTIONS}}": os_opts,
        "{{PRICE_OPTIONS}}": price_opts,
        "{{CONTACT}}": contact,
        "{{PRIVACY}}": privacy,
        "{{DEVBANNER}}": banner,
        "{{DEVBANNER_CSS}}": banner_css,
    }
    out = tpl
    for k, v in repl.items():
        out = out.replace(k, v)
    leftover = [t for t in ("{{",) if t in out]
    if leftover:
        raise ValueError(f"未置換のプレースホルダが残っています: {idea['slug']}")
    return out


def index_page(ideas: list, cfg: dict) -> str:
    rows = "\n".join(
        f'<li><a href="{esc(i["slug"])}/">{esc(i["name"])}</a> — {esc(i["tagline"])}</li>'
        for i in ideas
    )
    return (
        '<!doctype html><meta charset="utf-8"><meta name="robots" content="noindex">'
        f"<title>試験LP一覧</title><h1>試験LP一覧（{esc(cfg['studio_name'])}）</h1><ul>{rows}</ul>"
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*", help="生成する slug を限定")
    ap.add_argument("--serve", action="store_true", help="生成後にローカルで配信")
    args = ap.parse_args()

    cfg = json.loads((ROOT / "config.json").read_text(encoding="utf-8"))
    ideas = json.loads((ROOT / "ideas.json").read_text(encoding="utf-8"))
    tpl = (ROOT / "templates" / "lp.html").read_text(encoding="utf-8")

    slugs = [i["slug"] for i in ideas]
    if len(slugs) != len(set(slugs)):
        print("ideas.json の slug が重複しています", file=sys.stderr)
        return 1
    if args.only:
        unknown = set(args.only) - set(slugs)
        if unknown:
            print(f"未知の slug: {', '.join(sorted(unknown))}", file=sys.stderr)
            return 1
        ideas = [i for i in ideas if i["slug"] in args.only]

    dist = ROOT / "dist"
    for idea in ideas:
        d = dist / idea["slug"]
        d.mkdir(parents=True, exist_ok=True)
        (d / "index.html").write_text(render(idea, cfg, tpl), encoding="utf-8")
        print(f"生成: dist/{idea['slug']}/index.html")
    (dist / "index.html").write_text(index_page(ideas, cfg), encoding="utf-8")

    if not cfg.get("form_action", "").strip():
        print("\n注意: form_action が未設定です。公開前に config.json を設定してください。")

    if args.serve:
        import functools
        import http.server
        import socketserver

        handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(dist))
        with socketserver.TCPServer(("", 8000), handler) as srv:
            print("http://localhost:8000 で配信中(Ctrl+C で終了)")
            try:
                srv.serve_forever()
            except KeyboardInterrupt:
                pass
    return 0


if __name__ == "__main__":
    sys.exit(main())

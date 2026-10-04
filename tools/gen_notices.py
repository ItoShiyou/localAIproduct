#!/usr/bin/env python3
"""配布物に同梱する第三者ライセンス表記(THIRD_PARTY_NOTICES.txt)を作る。

使い方:
  python3 tools/gen_notices.py <アプリの src-tauri> <アプリの ui> <出力ファイル> [--features a,b] [--extra 追加の表記ファイル.json]

- Rust: `cargo metadata` の依存グラフから、通常の依存(build / dev 依存を除く。配布物に入るもの)を、
  macOS(aarch64)と Windows(x86_64)の両方の対象でたどり、各クレートの LICENSE / COPYING / NOTICE などの全文を入れる。
  クレートに全文が無い場合は、SPDX の標準テキスト(tools/licenses/)を入れ、作者・リポジトリを添える。
- npm: 画面の JS に入る依存(package.json の dependencies とその依存)の LICENSE を入れる。
- MPL-2.0 の部品は、ソースコードの入手先を明記する(MPL-2.0 第3.2節)。
- --extra: Rust / npm の外にある部品(モデルの重みなど)。[{"name","version","license","url","text_file"}] の JSON。
"""
import json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.abspath(__file__))
SPDX_DIR = os.path.join(ROOT, "licenses")
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT|UNLICENSE)([-._].*)?$", re.I)
TARGETS = ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"]


def spdx_ids(expr):
    return sorted(set(t for t in re.split(r"[\s()/]+|\bOR\b|\bAND\b|\bWITH\b", expr or "") if t and t not in ("OR", "AND", "WITH")))


def spdx_text(i):
    p = os.path.join(SPDX_DIR, f"{i}.txt")
    return open(p, encoding="utf-8").read() if os.path.exists(p) else None


def license_files(d, depth=0):
    out = []
    try:
        names = sorted(os.listdir(d))
    except OSError:
        return out
    for n in names:
        p = os.path.join(d, n)
        if os.path.isfile(p) and LICENSE_FILE.match(n):
            out.append(p)
    return out


def rust_packages(src_tauri, features):
    pkgs = {}
    for target in TARGETS:
        cmd = ["cargo", "metadata", "--format-version", "1", "--filter-platform", target]
        if features is not None:
            cmd += ["--no-default-features", "--features", features]
        meta = json.loads(subprocess.check_output(cmd, cwd=src_tauri))
        by_id = {p["id"]: p for p in meta["packages"]}
        nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
        root = meta["resolve"]["root"]
        stack, seen = [root], set()
        while stack:
            i = stack.pop()
            if i in seen:
                continue
            seen.add(i)
            for d in nodes[i]["deps"]:
                if any(k["kind"] is None for k in d["dep_kinds"]):  # 通常の依存だけ(build / dev は配布物に入らない)
                    stack.append(d["pkg"])
        for i in seen:
            p = by_id[i]
            if p["source"] is None:  # 自作(アプリ・core)
                continue
            pkgs[(p["name"], p["version"])] = p
    return [pkgs[k] for k in sorted(pkgs)]


def npm_packages(ui):
    out = subprocess.check_output(["npm", "ls", "--omit=dev", "--all", "--json"], cwd=ui, shell=(os.name == "nt"))
    tree = json.loads(out)
    found = {}

    def walk(deps):
        for name, info in (deps or {}).items():
            key = (name, info.get("version", ""))
            if key not in found:
                found[key] = os.path.join(ui, "node_modules", name)
                walk(info.get("dependencies"))

    walk(tree.get("dependencies"))
    res = []
    for (name, ver), d in sorted(found.items()):
        pj = json.load(open(os.path.join(d, "package.json"), encoding="utf-8"))
        res.append({"name": name, "version": ver, "license": pj.get("license", ""), "dir": d, "repository": (pj.get("repository") or {}).get("url", "") if isinstance(pj.get("repository"), dict) else pj.get("repository", "")})
    return res


def main():
    args = sys.argv[1:]
    features = None
    extra = None
    if "--features" in args:
        i = args.index("--features"); features = args[i + 1]; del args[i:i + 2]
    if "--extra" in args:
        i = args.index("--extra"); extra = args[i + 1]; del args[i:i + 2]
    src_tauri, ui, out_path = args
    sections, summary, mpl = [], [], []
    missing = []

    def add(name, ver, lic, url, texts, note=""):
        summary.append(f"- {name} {ver} — {lic}")
        body = "\n\n".join(t.strip() for t in texts)
        head = f"{name} {ver}\nLicense: {lic}\n" + (f"Source: {url}\n" if url else "") + (f"{note}\n" if note else "")
        sections.append(head + "-" * 70 + "\n" + body + "\n")

    if extra:
        for e in json.load(open(extra, encoding="utf-8")):
            base = os.path.dirname(os.path.abspath(extra))
            add(e["name"], e.get("version", ""), e["license"], e.get("url", ""), [open(os.path.join(base, e["text_file"]), encoding="utf-8").read()], e.get("note", ""))

    for p in rust_packages(src_tauri, features):
        d = os.path.dirname(p["manifest_path"])
        files = license_files(d)
        url = p.get("repository") or f"https://crates.io/crates/{p['name']}/{p['version']}"
        texts = [open(f, encoding="utf-8", errors="replace").read() for f in files]
        note = ""
        # 同梱しているネイティブのソース(whisper.cpp / SQLite)の表記
        bundled = []
        for sub in ("whisper.cpp", "sqlite3"):
            sub_files = license_files(os.path.join(d, sub))
            if sub_files:
                bundled.append(sub)
            texts += [open(f, encoding="utf-8", errors="replace").read() for f in sub_files]
        if not texts:
            ids = spdx_ids(p.get("license"))
            std = [(i, spdx_text(i)) for i in ids]
            if any(t is None for _, t in std):
                missing.append(f"{p['name']} {p['version']} ({p.get('license')})")
            authors = ", ".join(p.get("authors") or []) or "the authors"
            texts = [f"Copyright (c) {authors}\n(このクレートに全文の同梱が無いため、SPDX の標準テキストを掲載)"] + [t for _, t in std if t]
        lic = p.get("license") or "(see text)"
        if bundled:
            note = f"Includes the bundled source of {', '.join(bundled)}; its license text is reproduced below."
        if "MPL" in lic:
            mpl.append(f"- {p['name']} {p['version']}: {url}(crates.io: https://crates.io/crates/{p['name']}/{p['version']})")
            note = (note + " " if note else "") + "This component is used without modification. Its source code is available at the URL above (MPL-2.0 §3.2)."
        add(p["name"], p["version"], lic, url, texts, note)

    for n in npm_packages(ui):
        files = license_files(n["dir"])
        texts = [open(f, encoding="utf-8", errors="replace").read() for f in files]
        if not texts:
            std = [spdx_text(i) for i in spdx_ids(n["license"])]
            if not any(std):
                missing.append(f"npm {n['name']} {n['version']} ({n['license']})")
            texts = [t for t in std if t]
        add(n["name"], n["version"], n["license"], n["repository"], texts)

    head = [
        "THIRD-PARTY SOFTWARE NOTICES AND INFORMATION / 第三者のソフトウェアとモデルのライセンス表記",
        "",
        "このアプリには、以下の第三者のソフトウェア・モデルが含まれています。各ライセンスの条件に従い、著作権表示と許諾文を掲載します。",
        "This application includes the third-party software and models listed below. Their copyright notices and license texts are reproduced here as required by their licenses.",
        "",
    ]
    if mpl:
        head += [
            "Mozilla Public License 2.0 の部品(改変せずに使用。ソースコードは次の場所から入手できます)",
            "Components licensed under MPL-2.0 (used unmodified; source code is available at):",
            *mpl,
            "",
        ]
    head += ["一覧 / Summary", *summary, "", "=" * 70, ""]
    text = "\n".join(head) + ("\n" + "=" * 70 + "\n\n").join(sections)
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    open(out_path, "w", encoding="utf-8").write(text)
    print(f"{out_path}: {len(sections)} 件, {len(text) // 1024} KB")
    if missing:
        print("全文を用意できなかったもの(要確認):", *missing, sep="\n  ")
        sys.exit(2)


if __name__ == "__main__":
    main()

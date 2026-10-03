#!/usr/bin/env bash
# 新しいアプリの作業場所を apps/<slug>/ に作る。
#   ./tools/new_app.sh <slug>
# ideas.json の同じ slug の内容を SPEC.md / KICKOFF.md / CLAUDE.md に流し込み、
# core/ のコピーとチェックリストを入れて、アプリごとに独立した git リポジトリにする。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SLUG="${1:-}"

if [[ -z "$SLUG" ]]; then
  echo "使い方: $0 <slug>   (slug は ideas.json から選ぶ)" >&2
  python3 - "$ROOT" <<'PY' >&2
import json, sys, pathlib
root = pathlib.Path(sys.argv[1])
print("候補:")
for i in json.loads((root / "ideas.json").read_text(encoding="utf-8")):
    print(f"  {i['slug']:<22}{i['name']}")
PY
  exit 1
fi

DEST="$ROOT/apps/$SLUG"
if [[ -e "$DEST" ]]; then
  echo "既にあります: $DEST (上書きしません)" >&2
  exit 1
fi

mkdir -p "$DEST"
cp -R "$ROOT/templates/app/." "$DEST/"
cp -R "$ROOT/templates/checklists" "$DEST/checklists"
if [[ -d "$ROOT/docs" ]]; then
  cp -R "$ROOT/docs" "$DEST/docs"
fi
mkdir -p "$DEST/core"
if [[ -n "$(ls -A "$ROOT/core" 2>/dev/null)" ]]; then
  cp -R "$ROOT/core/." "$DEST/core/"
fi

python3 - "$ROOT" "$SLUG" "$DEST" <<'PY'
import json, pathlib, sys
root, slug, dest = pathlib.Path(sys.argv[1]), sys.argv[2], pathlib.Path(sys.argv[3])
ideas = {i["slug"]: i for i in json.loads((root / "ideas.json").read_text(encoding="utf-8"))}
if slug not in ideas:
    sys.exit(f"ideas.json に slug '{slug}' がありません")
i = ideas[slug]
repl = {
    "{{NAME}}": i["name"],
    "{{SLUG}}": slug,
    "{{TAGLINE}}": i["tagline"],
    "{{AUDIENCE}}": i["audience"],
    "{{PAIN}}": i["pain"],
    "{{LOCAL_REASON}}": i["local_reason"],
    "{{NOTE}}": i["note"],
    "{{FEATURES_MD}}": "\n".join(f"- [ ] {f}" for f in i["features"]),
}
for name in ("CLAUDE.md", "SPEC.md", "KICKOFF.md"):
    p = dest / name
    t = p.read_text(encoding="utf-8")
    for k, v in repl.items():
        t = t.replace(k, v)
    p.write_text(t, encoding="utf-8")
PY

# specs/<slug>/ があれば、その SPEC.md / KICKOFF.md(と参考ファイル)で雛形を置き換える
if [[ -d "$ROOT/specs/$SLUG" ]]; then
  cp -R "$ROOT/specs/$SLUG/." "$DEST/"
  echo "specs/$SLUG の仕様書で上書きしました"
fi

chmod +x "$DEST/run.sh"

(
  cd "$DEST"
  git init -q
  git add -A
  git -c user.name="${GIT_AUTHOR_NAME:-factory}" -c user.email="${GIT_AUTHOR_EMAIL:-factory@localhost}" \
    commit -qm "chore: 工場の雛形からスキャフォールド ($SLUG)"
)

echo "作成: apps/$SLUG"
echo "次: ./tools/parallel.sh start $SLUG"

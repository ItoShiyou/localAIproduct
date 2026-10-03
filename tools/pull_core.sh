#!/usr/bin/env bash
# 最初のアプリ(receipt-db)で作った共通コアを、factory/core/ に取り込む。
#   ./tools/pull_core.sh <slug>
# apps/<slug>/core/ の内容で factory/core/ を更新する(target/ と node_modules/ は除く)。
# そのあと ./tools/sync_core.sh <他のslug>... で、ほかのアプリへ配る。
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SLUG="${1:-}"
[[ -n "$SLUG" && -d "$ROOT/apps/$SLUG/core" ]] || { echo "使い方: $0 <slug> (apps/<slug>/core が必要)" >&2; exit 1; }
if [[ -d "$ROOT/apps/$SLUG/.git" && -n "$(git -C "$ROOT/apps/$SLUG" status --porcelain -- core)" ]]; then
  echo "中止: apps/$SLUG の core/ に未コミットの変更があります。先にコミットしてください" >&2
  exit 1
fi
mkdir -p "$ROOT/core"
( cd "$ROOT/apps/$SLUG/core" && tar --exclude='./engine/target' --exclude='node_modules' -cf - . ) | ( cd "$ROOT/core" && tar -xf - )
echo "取り込み: apps/$SLUG/core → core/"
echo "次: ./tools/sync_core.sh <他のslug>..."

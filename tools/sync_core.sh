#!/usr/bin/env bash
# factory/core/ の最新版を、各アプリの core/ に取り込む。
#   ./tools/sync_core.sh <slug>...     指定したアプリだけ
#   ./tools/sync_core.sh --all         全アプリ
# アプリ側の core/ は上書きされる。アプリ側で直接コアを直した場合は先にコミットしておくこと。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
targets=()
if [[ "${1:-}" == "--all" ]]; then
  for d in "$ROOT"/apps/*/; do [[ -d "$d" ]] && targets+=("$(basename "$d")"); done
else
  targets=("$@")
fi
[[ ${#targets[@]} -ge 1 ]] || { echo "使い方: $0 <slug>... | --all" >&2; exit 1; }

for slug in "${targets[@]}"; do
  dir="$ROOT/apps/$slug"
  [[ -d "$dir" ]] || { echo "ありません: apps/$slug" >&2; continue; }
  if [[ -d "$dir/.git" && -n "$(git -C "$dir" status --porcelain -- core)" ]]; then
    echo "スキップ: apps/$slug の core/ に未コミットの変更があります" >&2
    continue
  fi
  rm -rf "$dir/core"
  mkdir -p "$dir/core"
  cp -R "$ROOT/core/." "$dir/core/"
  echo "同期: apps/$slug"
done

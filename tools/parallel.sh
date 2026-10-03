#!/usr/bin/env bash
# 複数アプリの開発を、tmux のセッション1つ=アプリ1本で同時に回す。
#
#   ./tools/parallel.sh start  <slug>...   各アプリのセッションを起動(Claude Code が KICKOFF.md を読んで開始)
#   ./tools/parallel.sh status             全アプリの進み具合を一覧表示
#   ./tools/parallel.sh attach <slug>      セッションに入る(抜けるときは Ctrl+b → d)
#   ./tools/parallel.sh stop   <slug>...   セッションを止める(slug 省略で全部)
#
# 環境変数:
#   CLAUDE_CMD     起動するコマンド(既定: claude)
#   MAX_PARALLEL   同時に動かす上限(既定: 3)。超える場合は FORCE=1 を付ける
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLAUDE_CMD="${CLAUDE_CMD:-claude}"
MAX_PARALLEL="${MAX_PARALLEL:-3}"
PREFIX="fac-"

need_tmux() {
  command -v tmux >/dev/null 2>&1 || { echo "tmux が必要です (brew install tmux / apt install tmux)" >&2; exit 1; }
}

running_slugs() {
  tmux list-sessions -F '#S' 2>/dev/null | grep "^${PREFIX}" | sed "s/^${PREFIX}//" || true
}

cmd_start() {
  need_tmux
  [[ $# -ge 1 ]] || { echo "slug を指定してください" >&2; exit 1; }
  local current new_count
  current=$(running_slugs | grep -c . || true)
  new_count=0
  for slug in "$@"; do
    tmux has-session -t "${PREFIX}${slug}" 2>/dev/null || new_count=$((new_count + 1))
  done
  if (( current + new_count > MAX_PARALLEL )) && [[ "${FORCE:-0}" != "1" ]]; then
    echo "同時実行の上限 ${MAX_PARALLEL} を超えます(現在 ${current}、追加 ${new_count})。" >&2
    echo "確認とサポートはあなた1人に集まります。増やす場合は FORCE=1 を付けてください。" >&2
    exit 1
  fi
  for slug in "$@"; do
    local dir="$ROOT/apps/$slug"
    if [[ ! -d "$dir" ]]; then
      echo "apps/$slug がありません。先に ./tools/new_app.sh $slug" >&2
      continue
    fi
    if tmux has-session -t "${PREFIX}${slug}" 2>/dev/null; then
      echo "起動済み: $slug"
      continue
    fi
    tmux new-session -d -s "${PREFIX}${slug}" -c "$dir" "env CLAUDE_CMD='$CLAUDE_CMD' bash '$dir/run.sh'"
    echo "起動: $slug  (入る: ./tools/parallel.sh attach $slug)"
  done
}

cmd_status() {
  need_tmux
  printf '%-22s %-6s %-9s %-6s %s\n' "アプリ" "実行" "SPEC進捗" "未保存" "最新コミット"
  shopt -s nullglob
  local any=0
  for dir in "$ROOT"/apps/*/; do
    any=1
    local slug; slug="$(basename "$dir")"
    local up="-"
    tmux has-session -t "${PREFIX}${slug}" 2>/dev/null && up="稼働"
    local done_n=0 total=0
    if [[ -f "$dir/SPEC.md" ]]; then
      done_n=$(grep -c '^- \[x\]' "$dir/SPEC.md" || true)
      total=$(grep -cE '^- \[( |x)\]' "$dir/SPEC.md" || true)
    fi
    local dirty="-" last="-"
    if [[ -d "$dir/.git" ]]; then
      dirty=$(git -C "$dir" status --porcelain | wc -l | tr -d ' ')
      last=$(git -C "$dir" log -1 --format='%cd %s' --date=short 2>/dev/null || echo "-")
    fi
    printf '%-22s %-6s %-9s %-6s %s\n' "$slug" "$up" "${done_n}/${total}" "$dirty" "$last"
  done
  (( any )) || echo "apps/ にアプリがありません。./tools/new_app.sh <slug> で作成します。"
}

cmd_attach() {
  need_tmux
  [[ $# -eq 1 ]] || { echo "使い方: attach <slug>" >&2; exit 1; }
  exec tmux attach -t "${PREFIX}$1"
}

cmd_stop() {
  need_tmux
  local targets=("$@")
  if [[ ${#targets[@]} -eq 0 ]]; then
    mapfile -t targets < <(running_slugs)
  fi
  for slug in "${targets[@]}"; do
    if tmux has-session -t "${PREFIX}${slug}" 2>/dev/null; then
      tmux kill-session -t "${PREFIX}${slug}"
      echo "停止: $slug"
    fi
  done
}

case "${1:-}" in
  start)  shift; cmd_start "$@" ;;
  status) cmd_status ;;
  attach) shift; cmd_attach "$@" ;;
  stop)   shift; cmd_stop "$@" ;;
  *) sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
